//! Errand Runner — lightweight side-agent task execution.
//!
//! Any relay profession can `dispatch` a task to a gofer, who runs it in an
//! isolated context with a cheap model. Only the summary returns to the caller;
//! the full errand log is persisted for audit.

use crate::forge::tools::{ToolError, ToolRegistry};
use crate::forge::ForgeStreamEvent;
use crate::provider::types::*;
use crate::provider::ClaudeProvider;
use axum::response::sse::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// Alias for the SSE event sender used by forge_stream.
pub type SseEventSender = tokio::sync::mpsc::UnboundedSender<Result<Event, std::convert::Infallible>>;

/// Status of an errand session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrandStatus {
    Running,
    Completed { result: String },
    Failed { error: String },
    Truncated { result: String, reason: String },
}

/// An isolated errand session spawned by the `dispatch` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrandSession {
    pub id: String,
    pub parent_session_id: String,
    pub profession_id: String,
    pub task: String,
    pub context: Option<String>,
    pub max_turns: u32,
    /// ID of the dispatch tool_call that spawned this errand.
    pub tool_call_id: String,
    /// Messages exchanged during the errand (isolated from master session).
    pub messages: Vec<ChatMessage>,
    pub status: ErrandStatus,
    pub token_usage: u64,
    pub started_at: u64,
    pub completed_at: Option<u64>,
}

impl ErrandSession {
    /// Create a new errand session.
    pub fn new(
        parent_session_id: String,
        profession_id: String,
        task: String,
        context: Option<String>,
        max_turns: u32,
        tool_call_id: String,
    ) -> Self {
        Self {
            id: format!("e-{}", uuid::Uuid::new_v4()),
            parent_session_id,
            profession_id,
            task: task.clone(),
            context,
            max_turns,
            tool_call_id,
            messages: vec![ChatMessage::user(&task)],
            status: ErrandStatus::Running,
            token_usage: 0,
            started_at: crate::forge::now_secs(),
            completed_at: None,
        }
    }

    /// Run the errand to completion (or max_turns).
    ///
    /// This is a mini ReAct loop: chat → tool_use → execute → tool_result → repeat.
    /// It runs synchronously by blocking the current thread, because the
    /// `Tool::execute` trait method is sync and the caller expects a result.
    ///
    /// If `event_sender` is provided, errand progress events are streamed to the frontend.
    pub fn run_sync(
        &mut self,
        provider: Arc<ClaudeProvider>,
        tool_registry: &ToolRegistry,
        project_path: &str,
        event_sender: Option<SseEventSender>,
    ) -> Result<String, ToolError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.run(provider, tool_registry, project_path, event_sender).await
            })
        })
    }

    /// Async implementation of the errand runner.
    async fn run(
        &mut self,
        provider: Arc<ClaudeProvider>,
        tool_registry: &ToolRegistry,
        project_path: &str,
        event_sender: Option<SseEventSender>,
    ) -> Result<String, ToolError> {
        // Emit errand_start event
        if let Some(ref tx) = event_sender {
            let _ = tx.send(Ok(Event::default().data(
                serde_json::to_string(&ForgeStreamEvent::ErrandStart {
                    errand_id: self.id.clone(),
                    profession_id: self.profession_id.clone(),
                    task: self.task.clone(),
                    tool_call_id: self.tool_call_id.clone(),
                })
                .unwrap(),
            )));
        }

        // Build system prompt from gofer soul
        let system_prompt = build_gofer_system_prompt(&self.task, self.context.as_deref());

        // Build allowed tools for the gofer profession
        let allowed_tools = crate::relay::ProfessionRegistry::new()
            .get(&self.profession_id)
            .map(|p| p.allowed_tools.clone())
            .unwrap_or_default();

        let tools: Vec<ToolDefinition> = tool_registry
            .definitions()
            .into_iter()
            .filter(|t| allowed_tools.contains(&t.name))
            .collect();

        let mut chat_messages = self.messages.clone();
        let mut final_text = String::new();

        for turn in 0..self.max_turns {
            // Emit errand_turn_start event
            if let Some(ref tx) = event_sender {
                let _ = tx.send(Ok(Event::default().data(
                    serde_json::to_string(&ForgeStreamEvent::ErrandTurnStart {
                        errand_id: self.id.clone(),
                        turn: turn + 1,
                        profession_id: self.profession_id.clone(),
                        tool_call_id: self.tool_call_id.clone(),
                    })
                    .unwrap(),
                )));
            }

            let request = ToolChatRequest {
                messages: chat_messages.clone(),
                tools: tools.clone(),
                system_prompt: Some(system_prompt.clone()),
                thinking_budget: None,
                max_tokens: None,
            };

            let (turn_tx, mut turn_rx) =
                tokio::sync::mpsc::unbounded_channel::<ToolChatEvent>();
            let provider_clone = provider.clone();

            let _turn_task = tokio::spawn(async move {
                provider_clone.chat_turn(request, turn_tx).await
            });

            let mut turn_text = String::new();
            let mut tool_use_info: Option<(String, String, Value)> = None;

            while let Some(event) = turn_rx.recv().await {
                match event {
                    ToolChatEvent::TextDelta { text } => {
                        turn_text.push_str(&text);
                        // Emit errand_delta event
                        if let Some(ref tx) = event_sender {
                            let _ = tx.send(Ok(Event::default().data(
                                serde_json::to_string(&ForgeStreamEvent::ErrandDelta {
                                    errand_id: self.id.clone(),
                                    text: text.clone(),
                                    tool_call_id: self.tool_call_id.clone(),
                                })
                                .unwrap(),
                            )));
                        }
                    }
                    ToolChatEvent::ThinkingDelta { .. } => {}
                    ToolChatEvent::ToolUse { id, name, input } => {
                        tool_use_info = Some((id.clone(), name.clone(), input.clone()));
                        // Emit errand_tool_call event
                        if let Some(ref tx) = event_sender {
                            let _ = tx.send(Ok(Event::default().data(
                                serde_json::to_string(&ForgeStreamEvent::ErrandToolCall {
                                    errand_id: self.id.clone(),
                                    id,
                                    name,
                                    arguments: input,
                                    tool_call_id: self.tool_call_id.clone(),
                                })
                                .unwrap(),
                            )));
                        }
                    }
                    ToolChatEvent::Usage { input_tokens, output_tokens } => {
                        self.token_usage += (input_tokens + output_tokens) as u64;
                    }
                    ToolChatEvent::Done => break,
                    ToolChatEvent::Error { message } => {
                        self.status = ErrandStatus::Failed {
                            error: message.clone(),
                        };
                        self.completed_at = Some(crate::forge::now_secs());
                        self.emit_complete(&event_sender);
                        return Err(ToolError::ExecutionFailed(message));
                    }
                }
            }

            final_text = turn_text.clone();

            if let Some((tool_id, tool_name, tool_input)) = tool_use_info {
                // Execute the tool
                crate::forge::tools::set_tool_context(project_path, &self.parent_session_id);
                crate::forge::tools::set_current_profession(&self.profession_id);

                let tool_result = if let Some(tool) = tool_registry.get(&tool_name) {
                    match tool.execute(tool_input.clone()) {
                        Ok(r) => r,
                        Err(e) => format!("Error: {}", e),
                    }
                } else {
                    format!("Tool '{}' not found", tool_name)
                };

                // Emit errand_tool_result event
                if let Some(ref tx) = event_sender {
                    let _ = tx.send(Ok(Event::default().data(
                        serde_json::to_string(&ForgeStreamEvent::ErrandToolResult {
                            errand_id: self.id.clone(),
                            id: tool_id.clone(),
                            result: tool_result.clone(),
                            tool_call_id: self.tool_call_id.clone(),
                        })
                        .unwrap(),
                    )));
                }

                // Build assistant message (text + tool_use)
                let mut blocks = vec![ContentBlock::text(&turn_text)];
                blocks.push(ContentBlock::ToolUse {
                    id: tool_id.clone(),
                    name: tool_name.clone(),
                    input: tool_input.clone(),
                });
                chat_messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: blocks,
                });

                // Add tool result
                chat_messages.push(ChatMessage::tool_result(&tool_id, &tool_result));

                // Record in errand messages
                self.messages = chat_messages.clone();
            } else {
                // No tool use — errand is complete
                let summary = self.generate_tool_summary();
                let result = format!("{}{}", turn_text, summary);
                self.status = ErrandStatus::Completed {
                    result: result.clone(),
                };
                self.completed_at = Some(crate::forge::now_secs());
                self.emit_complete(&event_sender);
                return Ok(result);
            }
        }

        // Max turns reached
        let summary = self.generate_tool_summary();
        let truncated = format!(
            "{final_text}{summary}\n\n[Errand reached maximum turns ({}). Stopping.]",
            self.max_turns
        );
        self.status = ErrandStatus::Truncated {
            result: truncated.clone(),
            reason: format!("max_turns ({}) exceeded", self.max_turns),
        };
        self.completed_at = Some(crate::forge::now_secs());
        self.emit_complete(&event_sender);
        Ok(truncated)
    }

    /// Generate an objective summary of tool calls made during this errand.
    fn generate_tool_summary(&self) -> String {
        use std::collections::HashMap;

        let mut tool_counts: HashMap<String, (u32, u32)> = HashMap::new();
        let mut pending_tool: Option<String> = None;
        let mut assistant_turns = 0u32;

        for msg in &self.messages {
            match msg.role.as_str() {
                "assistant" => {
                    assistant_turns += 1;
                    for block in &msg.content {
                        if let ContentBlock::ToolUse { name, .. } = block {
                            pending_tool = Some(name.clone());
                        }
                    }
                }
                "user" => {
                    if let Some(ref tool_name) = pending_tool {
                        let entry = tool_counts.entry(tool_name.clone()).or_insert((0, 0));
                        entry.0 += 1;
                        for block in &msg.content {
                            let result_text = match block {
                                ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
                                ContentBlock::Text { text } => Some(text.as_str()),
                                _ => None,
                            };
                            if let Some(text) = result_text {
                                if text.starts_with("Error:") {
                                    entry.1 += 1;
                                }
                            }
                        }
                        pending_tool = None;
                    }
                }
                _ => {}
            }
        }

        if tool_counts.is_empty() {
            return String::new();
        }

        let mut parts: Vec<String> = tool_counts
            .iter()
            .map(|(name, (total, errors))| {
                if *errors > 0 {
                    format!("{}×{} ({} failed)", name, total, errors)
                } else {
                    format!("{}×{}", name, total)
                }
            })
            .collect();
        parts.sort();

        format!(
            "\n\n[System Summary] Tools actually used: {} (over {} turns)",
            parts.join(", "),
            assistant_turns
        )
    }

    /// Emit the errand_complete SSE event.
    fn emit_complete(&self, event_sender: &Option<SseEventSender>) {
        if let Some(ref tx) = event_sender {
            let (status_str, result_str) = match &self.status {
                ErrandStatus::Completed { result } => ("completed".to_string(), result.clone()),
                ErrandStatus::Failed { error } => ("failed".to_string(), error.clone()),
                ErrandStatus::Truncated { result, .. } => ("truncated".to_string(), result.clone()),
                ErrandStatus::Running => ("running".to_string(), String::new()),
            };
            let _ = tx.send(Ok(Event::default().data(
                serde_json::to_string(&ForgeStreamEvent::ErrandComplete {
                    errand_id: self.id.clone(),
                    status: status_str,
                    result: result_str,
                    token_usage: self.token_usage,
                    tool_call_id: self.tool_call_id.clone(),
                })
                .unwrap(),
            )));
        }
    }

    /// Persist the errand log to disk.
    pub fn save(&self, base_dir: &std::path::Path) -> std::io::Result<()> {
        let errand_dir = base_dir.join("errands");
        std::fs::create_dir_all(&errand_dir)?;
        let path = errand_dir.join(format!("{}.json", self.id));
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }

    /// Load an errand log from disk.
    pub fn load(errand_id: &str, base_dir: &std::path::Path) -> std::io::Result<Self> {
        let path = base_dir.join("errands").join(format!("{}.json", errand_id));
        let content = std::fs::read_to_string(&path)?;
        let session = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// List all errand IDs persisted for a session.
    pub fn list_for_session(base_dir: &std::path::Path) -> std::io::Result<Vec<String>> {
        let errand_dir = base_dir.join("errands");
        let mut ids = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&errand_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension() == Some("json".as_ref()) {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        ids.push(stem.to_string());
                    }
                }
            }
        }
        Ok(ids)
    }
}

/// Build the gofer's system prompt from soul + task + context.
fn build_gofer_system_prompt(task: &str, context: Option<&str>) -> String {
    let relay = crate::relay::RelayRegistry::new();
    let base_prompt = match relay
        .default_agent_for("gofer")
        .and_then(|cfg| relay.spawn_agent_from_config(cfg))
    {
        Some(agent) => agent.render_system_prompt(),
        None => "You are Gus, an AI research assistant.".to_string(),
    };

    let ctx = context.unwrap_or("None provided.");
    format!(
        "{base_prompt}\n\n---\n\n## Your Task\n{task}\n\n## Caller Context\n{ctx}\n\n---\n\nNow do the task. Be concise."
    )
}
