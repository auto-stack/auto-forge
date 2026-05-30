//! Agent Turn Engine
//!
//! Extracts the ReAct loop from the Forge chat handler into a reusable,
//! parameterized component. Each AgentTurn runs one agent's step in the
//! relay pipeline: it holds the baton, executes tools, and produces a
//! result that can be turned into a HandoffDocument.

use crate::provider::{ChatMessage, ContentBlock, ToolChatEvent, ToolChatRequest, ClaudeProviderState};
use crate::forge::tools::{set_current_profession, ToolDefinition, ToolRegistry};
use crate::relay::agent::AgentInstance;
use crate::relay::budget::{BudgetAction, BudgetTracker};
use crate::relay::flow::{ToolAction, ToolGuard};
use crate::relay::handoff::{HandoffDocument, SpecUpdate};
use serde_json::Value;

/// Events emitted during an agent turn.
#[derive(Debug, Clone)]
pub enum TurnEvent {
    /// A text delta from the LLM.
    TextDelta { text: String },
    /// The agent wants to use a tool.
    ToolCall { id: String, name: String, arguments: Value },
    /// Tool execution completed.
    ToolResult { id: String, result: String },
    /// The turn completed normally (no more tool calls).
    Complete,
    /// An error occurred.
    Error { message: String },
    /// Budget warning fired.
    BudgetWarning { remaining: u64 },
    /// Budget hard-stopped the turn.
    BudgetExceeded,
}

/// Result of a completed agent turn.
#[derive(Debug, Clone)]
pub struct TurnResult {
    /// Full assistant text produced during the turn.
    pub assistant_text: String,
    /// Tool calls made during the turn.
    pub tool_calls: Vec<ToolCallRecord>,
    /// Input tokens consumed this turn.
    pub input_tokens: u64,
    /// Output tokens consumed this turn.
    pub output_tokens: u64,
    /// Whether the agent explicitly called the `handoff` tool.
    pub handoff_requested: bool,
    /// Decisions extracted from the turn text.
    pub decisions: Vec<String>,
    /// Open questions extracted from the turn text.
    pub open_questions: Vec<String>,
    /// Files touched during this turn, with action type (read/write/edit).
    pub files_touched: Vec<(String, ToolAction)>,
    /// Spec sections updated during this turn.
    pub spec_updates: Vec<SpecUpdate>,
}

/// Record of a single tool invocation.
#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub result: String,
}

/// Reusable agent turn engine.
pub struct AgentTurn {
    pub agent: AgentInstance,
    /// Filtered tool definitions for this profession.
    pub tool_definitions: Vec<ToolDefinition>,
    /// Full tool registry for execution.
    pub tool_registry: ToolRegistry,
    /// Conversation history (mutable during the turn).
    pub messages: Vec<ChatMessage>,
    /// Max LLM turns.
    pub max_turns: u32,
    /// Budget tracker for this run.
    pub budget_tracker: Option<BudgetTracker>,
    /// Optional tool guard enforcing step-level sequencing rules.
    pub tool_guard: Option<ToolGuard>,
    /// Optional run_id for logging context.
    pub run_id: Option<String>,
}

impl AgentTurn {
    /// Create a new AgentTurn from an agent instance.
    /// Filters tools to only those the profession is allowed to use.
    pub fn new(
        agent: AgentInstance,
        registry: ToolRegistry,
        messages: Vec<ChatMessage>,
    ) -> Self {
        // Use the global cached filter instead of re-scanning every time.
        let tool_definitions = ToolRegistry::global()
            .definitions_for_profession(&agent.profession, &agent.skill_tools);

        Self {
            agent,
            tool_definitions,
            tool_registry: registry,
            messages,
            max_turns: 40,
            budget_tracker: None,
            tool_guard: None,
            run_id: None,
        }
    }

    /// Run the ReAct loop until completion, error, or budget exhaustion.
    /// Events are sent via `tx` so callers can observe progress in real time.
    pub async fn run(
        &mut self,
        provider: ClaudeProviderState,
        tx: tokio::sync::mpsc::UnboundedSender<TurnEvent>,
    ) -> TurnResult {
        // Set current profession for tools that need it (e.g., dispatch validation)
        set_current_profession(&self.agent.profession.id);

        let mut result = TurnResult {
            assistant_text: String::new(),
            tool_calls: Vec::new(),
            input_tokens: 0,
            output_tokens: 0,
            handoff_requested: false,
            decisions: Vec::new(),
            open_questions: Vec::new(),
            files_touched: Vec::new(),
            spec_updates: Vec::new(),
        };

        let system_prompt = self.agent.render_system_prompt();
        let mut turn_count = 0;

        while turn_count < self.max_turns {
            turn_count += 1;
            self.agent.context.turns_taken = turn_count;
            let t_chat_turn = std::time::Instant::now();
            let mut tools_elapsed_ms: u64 = 0;

            // Budget check before turn
            if let Some(ref tracker) = self.budget_tracker {
                match tracker.check(&self.agent.profession.id) {
                    BudgetAction::Warning { remaining } => {
                        let _ = tx.send(TurnEvent::BudgetWarning { remaining });
                    }
                    BudgetAction::HardStop => {
                        let _ = tx.send(TurnEvent::BudgetExceeded);
                        break;
                    }
                    _ => {}
                }
            }

            let request = ToolChatRequest {
                messages: self.messages.clone(),
                tools: self.tool_definitions.clone(),
                system_prompt: Some(system_prompt.clone()),
                thinking_budget: if self.agent.thinking_enabled {
                    Some(self.agent.thinking_budget)
                } else {
                    None
                },
                max_tokens: Some(self.agent.model.max_tokens),
            };

            let (turn_tx, mut turn_rx) = tokio::sync::mpsc::unbounded_channel::<ToolChatEvent>();
            let provider_clone = provider.clone();
            let turn_task = tokio::spawn(async move {
                provider_clone.chat_turn(request, turn_tx).await
            });

            let mut got_tool_use = false;
            let mut turn_text = String::new();
            let mut turn_tools: Vec<ToolCallRecord> = Vec::new();

            while let Some(event) = turn_rx.recv().await {
                match event {
                    ToolChatEvent::TextDelta { text } => {
                        turn_text.push_str(&text);
                        let _ = tx.send(TurnEvent::TextDelta { text: text.clone() });
                    }
                    ToolChatEvent::ThinkingDelta { .. } => {}
                    ToolChatEvent::ToolUse { id, name, input } => {
                        got_tool_use = true;
                        let t_tool = std::time::Instant::now();
                        let _ = tx.send(TurnEvent::ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: input.clone(),
                        });

                        // Collect names of tools already called this turn for guard checks
                        let tools_called: Vec<String> = result.tool_calls.iter()
                            .chain(turn_tools.iter())
                            .map(|t| t.name.clone())
                            .collect();

                        // Apply tool guard if configured
                        let guard_result = if let Some(ref guard) = self.tool_guard {
                            guard.check(&name, &tools_called)
                        } else {
                            Ok(())
                        };

                        // Execute the tool (or return guard error)
                        let exec_result = match guard_result {
                            Err(guard_err) => guard_err,
                            Ok(_) => {
                                if let Some(tool) = self.tool_registry.get(&name) {
                                    let mut allowed: Vec<String> = self.agent.profession.allowed_tools.clone();
                                    for tool_name in &self.agent.skill_tools {
                                        if !allowed.contains(tool_name) {
                                            allowed.push(tool_name.clone());
                                        }
                                    }
                                    if !allowed.is_empty() && !allowed.contains(&name) {
                                        format!("Tool '{}' is not available for profession '{}'", name, self.agent.profession.id)
                                    } else {
                                        // Heavy tools (shell, search, list_symbols) can block for seconds.
                                        // Run them on the blocking thread pool so tokio workers stay responsive.
                                        let is_heavy = matches!(name.as_str(), "shell" | "search" | "list_symbols");
                                        let tool_result = if is_heavy {
                                            let tool_name = name.clone();
                                            let tool_input = input.clone();
                                            let tool_name_err = tool_name.clone();
                                            tokio::task::spawn_blocking(move || {
                                                crate::forge::tools::ToolRegistry::global()
                                                    .get(&tool_name)
                                                    .map(|t| t.execute(tool_input))
                                            })
                                            .await
                                            .ok()
                                            .flatten()
                                            .unwrap_or_else(|| Err(crate::forge::tools::ToolError::ExecutionFailed(
                                                format!("Tool '{}' failed or panicked", tool_name_err)
                                            )))
                                        } else {
                                            tool.execute(input.clone())
                                        };
                                        match tool_result {
                                            Ok(r) => r,
                                            Err(e) => format!("Error: {}", e),
                                        }
                                    }
                                } else {
                                    format!("Tool '{}' not found or not allowed for profession '{}'", name, self.agent.profession.id)
                                }
                            }
                        };

                        let _ = tx.send(TurnEvent::ToolResult {
                            id: id.clone(),
                            result: exec_result.clone(),
                        });

                        let tool_elapsed_ms = t_tool.elapsed().as_millis() as u64;
                        tools_elapsed_ms += tool_elapsed_ms;
                        tracing::info!(
                            run_id = %self.run_id.as_deref().unwrap_or("unknown"),
                            profession_id = %self.agent.profession.id,
                            turn = turn_count,
                            tool_name = %name,
                            elapsed_ms = tool_elapsed_ms,
                            "AgentTurn: tool_execute"
                        );
                        tracing::debug!("AgentTurn tool call: id={}, name={}, profession={}, result={}", id, name, self.agent.profession.id, exec_result.chars().take(100).collect::<String>());
                        // Track special tools
                        if name == "handoff" || name == "bring_in" || name == "spawn_relay" {
                            result.handoff_requested = true;
                        }
                        let action = match name.as_str() {
                            "read_file" => Some(ToolAction::Read),
                            "write_file" => Some(ToolAction::Write),
                            "edit_file" => Some(ToolAction::Edit),
                            _ => None,
                        };
                        if let Some(action) = action {
                            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                                if !result.files_touched.iter().any(|(p, _)| p == path) {
                                    result.files_touched.push((path.to_string(), action));
                                }
                            }
                        }
                        if name == "write_specs" {
                            if let Some(section_id) = input.get("section_id").and_then(|v| v.as_str()) {
                                let item_id = input.get("item_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let description = format!("Updated section '{}' via write_specs", section_id);
                                result.spec_updates.push(SpecUpdate {
                                    section_id: section_id.to_string(),
                                    item_id,
                                    change_type: "modified".to_string(),
                                    description,
                                });
                            }
                        }
                        if name == "write_goals" {
                            result.spec_updates.push(SpecUpdate {
                                section_id: "goals".to_string(),
                                item_id: None,
                                change_type: "modified".to_string(),
                                description: "Updated goals section via write_goals".to_string(),
                            });
                        }
                        if name == "update_spec" {
                            if let Some(section_id) = input.get("section_id").and_then(|v| v.as_str()) {
                                let item_id = input.get("item_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("upsert");
                                let description = format!("{} item '{}' in section '{}' via update_spec", action, item_id.as_deref().unwrap_or("unknown"), section_id);
                                result.spec_updates.push(SpecUpdate {
                                    section_id: section_id.to_string(),
                                    item_id,
                                    change_type: action.to_string(),
                                    description,
                                });
                            }
                        }

                        turn_tools.push(ToolCallRecord {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: input,
                            result: exec_result.clone(),
                        });

                        self.messages.push(ChatMessage::tool_result(&id, &exec_result));
                    }
                    ToolChatEvent::Usage { input_tokens, output_tokens } => {
                        result.input_tokens += input_tokens as u64;
                        result.output_tokens += output_tokens as u64;
                    }
                    ToolChatEvent::Done => break,
                    ToolChatEvent::Error { message } => {
                        let _ = tx.send(TurnEvent::Error { message: message.clone() });
                        result.assistant_text = turn_text;
                        result.tool_calls = turn_tools;
                        return result;
                    }
                }
            }

            // Check for turn-level errors from the provider
            match turn_task.await {
                Ok(Some(err)) => {
                    let _ = tx.send(TurnEvent::Error { message: err });
                    result.assistant_text = turn_text;
                    result.tool_calls = turn_tools;
                    return result;
                }
                Err(join_err) => {
                    let _ = tx.send(TurnEvent::Error { message: format!("Turn task panicked: {}", join_err) });
                    result.assistant_text = turn_text;
                    result.tool_calls = turn_tools;
                    return result;
                }
                Ok(None) => {}
            }

            let chat_turn_elapsed_ms = t_chat_turn.elapsed().as_millis() as u64;
            let llm_elapsed_ms = chat_turn_elapsed_ms.saturating_sub(tools_elapsed_ms);
            tracing::info!(
                run_id = %self.run_id.as_deref().unwrap_or("unknown"),
                profession_id = %self.agent.profession.id,
                turn = turn_count,
                chat_turn_elapsed_ms,
                llm_elapsed_ms,
                tools_elapsed_ms,
                tool_calls_this_turn = turn_tools.len(),
                "AgentTurn: chat_turn_complete"
            );

            // Persist assistant message for next turn
            if !turn_text.is_empty() || !turn_tools.is_empty() {
                if got_tool_use {
                    let mut blocks = vec![ContentBlock::text(&turn_text)];
                    for call in &turn_tools {
                        blocks.push(ContentBlock::ToolUse {
                            id: call.id.clone(),
                            name: call.name.clone(),
                            input: call.arguments.clone(),
                        });
                    }
                    self.messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: blocks,
                    });
                } else {
                    self.messages.push(ChatMessage::assistant_text(&turn_text));
                }
            }

            result.assistant_text.push_str(&turn_text);
            result.tool_calls.extend(turn_tools);

            // Auto-extract goals from advisor text and write to specs
            if self.agent.profession.id == "advisor" {
                if let Some(goals_content) = extract_goals_from_text(&turn_text) {
                    match write_goals_to_specs(&goals_content) {
                        Ok(section_id) => {
                            result.spec_updates.push(SpecUpdate {
                                section_id,
                                item_id: None,
                                change_type: "modified".to_string(),
                                description: "Goals extracted from advisor text and auto-written to specs".to_string(),
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Failed to auto-write goals from advisor text: {}", e);
                        }
                    }
                }
            }

            // If no tool_use was requested, we're done
            if !got_tool_use {
                break;
            }

            // If handoff was explicitly requested, stop
            if result.handoff_requested {
                break;
            }
        }

        // Extract decisions and questions from text (simple heuristics)
        result.decisions = extract_section(&result.assistant_text, "Decisions Made");
        result.open_questions = extract_section(&result.assistant_text, "Open Questions");

        let _ = tx.send(TurnEvent::Complete);

        // Clear profession context to prevent leakage between steps
        set_current_profession("");

        result
    }

    /// Generate a HandoffDocument from this turn's result.
    pub fn to_handoff(
        &self,
        result: &TurnResult,
        to_profession: &str,
        run_id: &str,
        checkpoint_id: u64,
    ) -> HandoffDocument {
        let mut handoff = HandoffDocument::new(
            &self.agent.profession.id,
            to_profession,
            run_id,
            checkpoint_id,
        );
        handoff.summary = format!(
            "{} completed their work in {} turns. Produced {} tool calls.",
            self.agent.profession.name,
            self.agent.context.turns_taken,
            result.tool_calls.len()
        );
        for d in &result.decisions {
            handoff.decisions.push(crate::relay::handoff::Decision {
                id: format!("D-{}", handoff.decisions.len() + 1),
                title: d.clone(),
                status: "made".to_string(),
                rationale: String::new(),
            });
        }
        for q in &result.open_questions {
            handoff.open_questions.push(crate::relay::handoff::Question {
                id: format!("Q-{}", handoff.open_questions.len() + 1),
                text: q.clone(),
                status: "open".to_string(),
                assigned_to: None,
            });
        }
        for u in &result.spec_updates {
            handoff.spec_updates.push(u.clone());
        }
        for (path, action) in &result.files_touched {
            handoff.work_product.push(crate::relay::handoff::WorkProduct {
                path: path.clone(),
                description: match action {
                    ToolAction::Read => "(read)".to_string(),
                    ToolAction::Write => "(written)".to_string(),
                    ToolAction::Edit => "(edited)".to_string(),
                },
                lines: None,
            });
        }
        handoff.token_usage = crate::relay::handoff::TokenUsage {
            step_input: result.input_tokens,
            step_output: result.output_tokens,
            cumulative: result.input_tokens + result.output_tokens,
            budget_remaining: self
                .agent
                .profession
                .token_budget
                .saturating_sub(result.input_tokens + result.output_tokens),
        };

        // Note: report generation tracking can be added here if needed

        handoff
    }
}

/// Simple heuristic: extract bullet items under a heading.
fn extract_section(text: &str, heading: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case(&format!("## {}", heading))
            || trimmed.eq_ignore_ascii_case(&format!("### {}", heading))
        {
            in_section = true;
            continue;
        }
        if in_section {
            if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
                break;
            }
            if trimmed.starts_with("-") || trimmed.starts_with("*") {
                results.push(trimmed[1..].trim().to_string());
            }
        }
    }
    results
}

/// Extract goals written in plain text from advisor output.
/// Looks for lines starting with `## G{N}` and collects everything until
/// the next `## ` heading or the end of text.
fn extract_goals_from_text(text: &str) -> Option<String> {
    let mut goals_lines: Vec<&str> = Vec::new();
    let mut in_goals = false;
    let goal_heading_re = regex::Regex::new(r"^##\s+G\d+\b").unwrap();

    for line in text.lines() {
        let trimmed = line.trim();
        if goal_heading_re.is_match(trimmed) {
            in_goals = true;
            goals_lines.push(line);
            continue;
        }
        if in_goals {
            // Stop at next major heading (## or # not part of goal content)
            if trimmed.starts_with("## ") && !trimmed.starts_with("## G") {
                break;
            }
            goals_lines.push(line);
        }
    }

    if goals_lines.is_empty() {
        None
    } else {
        Some(goals_lines.join("\n"))
    }
}

/// Write extracted goals content directly to the specs store.
pub fn write_goals_to_specs(content: &str) -> Result<String, String> {
    let project = crate::forge::tools::current_project();
    if project.is_empty() {
        return Err("No project context set".to_string());
    }
    let project_name = std::path::Path::new(&project)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or(project.clone());

    let mut store = crate::forge::specs().lock().map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let doc = store.get_or_default(&project_name);
    if let Some(section) = doc.sections.iter_mut().find(|s| s.id == "goals") {
        section.content = content.to_string();
        section.status = crate::forge::Status::InProgress;
        section.last_modified = now;
        if let Some(parsed) = crate::forge::SpecsStore::parse_ad_file(
            "goals",
            "goals",
            "Goals",
            &format!("# Goals\n{}", content),
        ) {
            for new_item in parsed.items {
                if let Some(existing) = section.items.iter_mut().find(|i| i.id == new_item.id) {
                    *existing = new_item;
                } else {
                    section.items.push(new_item);
                }
            }
            section.content = String::new();
        }
    } else {
        doc.sections.push(crate::forge::SpecsSection {
            id: "goals".to_string(),
            section_type: crate::forge::SectionType::Goals,
            title: "Goals".to_string(),
            items: vec![],
            content: content.to_string(),
            status: crate::forge::Status::InProgress,
            depends_on: vec![],
            last_modified: now,
            last_verified: None,
        });
    }

    let doc = store.get(&project_name).ok_or("Project not found")?;
    store.save_ad_format(doc, &project_name);
    Ok("goals".to_string())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::agent::AgentInstance;
    use crate::relay::profession::{ForgePhase, Profession};
    use crate::relay::soul::SoulConfig;
    use std::collections::HashMap;

    fn make_test_agent() -> AgentInstance {
        let profession = Profession {
            id: "tester".to_string(),
            name: "Tester".to_string(),
            phase: ForgePhase::Execution,
            owned_sections: vec![],
            readable_sections: vec![],
            allowed_tools: vec!["read_file".to_string()],
            handoff_to: vec![],
            dispatchable_to: vec![],
            approval_gates: vec![],
            max_turns: 20,
            token_budget: 10_000,
            thinking_enabled: false,
            thinking_budget: 0,
            base_skills: Vec::new(),
            min_tier: crate::relay::config::ModelTier::Lite,
            max_tier: crate::relay::config::ModelTier::Max,
        };
        let soul = SoulConfig::parse("tester", "# Soul of the Tester\n\n## Core Values\n- Test everything\n").unwrap();
        AgentInstance::spawn(profession, soul, crate::relay::agent::ModelConfig::cheap())
    }

    #[test]
    fn test_agent_turn_filters_tools() {
        let agent = make_test_agent();
        let registry = ToolRegistry::new();
        let turn = AgentTurn::new(agent, registry, vec![]);

        // Only read_file is allowed for the tester profession
        let names: Vec<String> = turn.tool_definitions.iter().map(|d| d.name.clone()).collect();
        assert!(names.contains(&"read_file".to_string()));
        assert!(!names.contains(&"write_file".to_string()));
    }

    #[test]
    fn test_extract_section() {
        let text = r#"## Decisions Made
- Use JWT instead of sessions
- Add refresh token rotation

## Open Questions
- Should we support OAuth1?
"#;
        let decisions = extract_section(text, "Decisions Made");
        assert_eq!(decisions.len(), 2);
        assert!(decisions[0].contains("JWT"));

        let questions = extract_section(text, "Open Questions");
        assert_eq!(questions.len(), 1);
        assert!(questions[0].contains("OAuth1"));
    }

    #[test]
    fn test_tool_guard_blocks_before_required() {
        let guard = ToolGuard {
            required_first: vec!["write_specs".to_string()],
            unlocks: HashMap::new(),
            always_allowed: vec![],
            forbidden: vec![],
        };
        assert!(guard.check("read_file", &[]).is_err());
        assert!(guard.check("write_specs", &[]).is_ok());
        assert!(guard.check("read_file", &["write_specs".to_string()]).is_ok());
    }

    #[test]
    fn test_tool_guard_forbidden() {
        let guard = ToolGuard {
            required_first: vec![],
            unlocks: HashMap::new(),
            always_allowed: vec![],
            forbidden: vec!["dispatch".to_string()],
        };
        assert!(guard.check("dispatch", &[]).is_err());
        assert!(guard.check("read_file", &[]).is_ok());
    }

    #[test]
    fn test_tool_guard_unlocks() {
        let mut unlocks = HashMap::new();
        unlocks.insert("edit_file".to_string(), vec!["read_file".to_string()]);
        let guard = ToolGuard {
            required_first: vec![],
            unlocks,
            always_allowed: vec![],
            forbidden: vec![],
        };
        assert!(guard.check("edit_file", &[]).is_err());
        assert!(guard.check("edit_file", &["read_file".to_string()]).is_ok());
    }
}
