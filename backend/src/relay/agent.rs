//! Agent Instance
//!
//! Combines Soul + Profession + Model into a runnable agent identity.
//! Handles context assembly and system prompt rendering.

use crate::provider::{ChatMessage, ToolChatRequest};
use crate::forge::tools::ToolDefinition;
use crate::relay::profession::Profession;
use crate::relay::soul::SoulConfig;
use serde::{Deserialize, Serialize};

/// Cognitive substrate configuration for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: Provider,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub reasoning_budget: Option<u32>,
    /// Ordered list of fallback model names if the primary fails.
    pub fallback_chain: Vec<String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            provider: Provider::Anthropic,
            model: String::from("claude-3-5-sonnet-20241022"),
            temperature: 0.3,
            max_tokens: 4096,
            reasoning_budget: None,
            fallback_chain: vec![
                String::from("claude-3-5-sonnet-20241022"),
                String::from("gpt-4o"),
            ],
        }
    }
}

impl ModelConfig {
    pub fn cheap() -> Self {
        Self {
            provider: Provider::Anthropic,
            model: String::from("claude-3-5-haiku-20241022"),
            temperature: 0.2,
            max_tokens: 2048,
            reasoning_budget: None,
            fallback_chain: vec![String::from("claude-3-5-haiku-20241022")],
        }
    }

    pub fn standard() -> Self {
        Self {
            provider: Provider::Anthropic,
            model: String::from("claude-3-5-sonnet-20241022"),
            temperature: 0.3,
            max_tokens: 4096,
            reasoning_budget: None,
            fallback_chain: vec![
                String::from("claude-3-5-sonnet-20241022"),
                String::from("gpt-4o"),
            ],
        }
    }

    pub fn strong() -> Self {
        Self {
            provider: Provider::Anthropic,
            model: String::from("claude-3-opus-20240229"),
            temperature: 0.2,
            max_tokens: 8192,
            reasoning_budget: Some(4000),
            fallback_chain: vec![
                String::from("claude-3-opus-20240229"),
                String::from("claude-3-5-sonnet-20241022"),
                String::from("gpt-4o"),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Anthropic,
    OpenAI,
    Local { url: String },
}

/// Per-turn mutable state for an agent.
#[derive(Debug, Clone, Default)]
pub struct AgentContext {
    /// Tokens consumed this step.
    pub budget_used: u64,
    /// Number of LLM turns taken this step.
    pub turns_taken: u32,
    /// Files touched during this step.
    pub files_touched: Vec<String>,
    /// Decisions made during this step.
    pub decisions: Vec<String>,
    /// Open questions raised during this step.
    pub open_questions: Vec<String>,
}

/// A runnable agent instance — ephemeral, spawned per step.
pub struct AgentInstance {
    pub id: String,
    pub profession: Profession,
    pub soul: SoulConfig,
    pub model: ModelConfig,
    pub context: AgentContext,
    /// Display name from AgentConfig (e.g. "Nicole"), not the profession or soul title.
    pub display_name: String,
    /// Resolved skill prompt fragments injected into the system prompt.
    pub skill_prompts: Vec<String>,
    /// Resolved skill-granted tool names.
    pub skill_tools: Vec<String>,
    /// When true, this agent is running inside a relay pipeline (not chat).
    /// Affects system prompt and available tools.
    pub relay_mode: bool,
    /// Enable Claude extended thinking mode for this agent.
    pub thinking_enabled: bool,
    /// Thinking budget in tokens (e.g. 1024, 2048). Only used when thinking_enabled is true.
    pub thinking_budget: u32,
}

impl AgentInstance {
    /// Spawn a new agent for a pipeline step.
    pub fn spawn(
        profession: Profession,
        soul: SoulConfig,
        model: ModelConfig,
    ) -> Self {
        let display_name = profession.name.clone();
        Self {
            id: format!("agent-{}", uuid::Uuid::new_v4()),
            profession: profession.clone(),
            soul,
            model,
            context: AgentContext::default(),
            display_name,
            skill_prompts: Vec::new(),
            skill_tools: Vec::new(),
            relay_mode: false,
            thinking_enabled: profession.thinking_enabled,
            thinking_budget: profession.thinking_budget,
        }
    }

    /// Spawn with an explicit display name from AgentConfig.
    pub fn spawn_named(
        profession: Profession,
        soul: SoulConfig,
        model: ModelConfig,
        display_name: String,
    ) -> Self {
        Self {
            id: format!("agent-{}", uuid::Uuid::new_v4()),
            profession: profession.clone(),
            soul,
            model,
            context: AgentContext::default(),
            display_name,
            skill_prompts: Vec::new(),
            skill_tools: Vec::new(),
            relay_mode: false,
            thinking_enabled: profession.thinking_enabled,
            thinking_budget: profession.thinking_budget,
        }
    }

    /// Resolve skills from a registry and attach their prompts/tools to this agent.
    pub fn with_skills(mut self, registry: &crate::relay::skills::SkillRegistry, skill_ids: &[String]) -> Self {
        for skill_id in skill_ids {
            if let Some(skill) = registry.get(skill_id) {
                let fragment = format!(
                    "## Skill: {}\n\n{}\n",
                    skill.name,
                    skill.prompt_fragment
                );
                self.skill_prompts.push(fragment);
                for tool in &skill.granted_tools {
                    if !self.skill_tools.contains(tool) {
                        self.skill_tools.push(tool.clone());
                    }
                }
            }
        }
        self
    }

    /// Render the system prompt from Soul + Profession + constraints.
    pub fn render_system_prompt(&self) -> String {
        let mut parts = Vec::new();

        // Identity — use the configured display name, not the app name
        parts.push(format!(
            "You are {}, an AI coding assistant.\n",
            self.display_name
        ));

        // Soul identity
        parts.push(self.soul.render());

        // Profession scope
        parts.push(format!(
            "## Profession: {}\n\nYour role is {}. Your phase is {}.\n",
            self.profession.name,
            self.profession.name,
            self.profession.phase.as_str()
        ));

        if !self.profession.owned_sections.is_empty() {
            let sections: Vec<String> = self.profession.owned_sections.iter()
                .map(|s| s.as_str().to_string())
                .collect();
            parts.push(format!(
                "You OWN these spec sections and may write to them: {}\n",
                sections.join(", ")
            ));
        }

        if !self.profession.readable_sections.is_empty() {
            let sections: Vec<String> = self.profession.readable_sections.iter()
                .map(|s| s.as_str().to_string())
                .collect();
            parts.push(format!(
                "You may READ these spec sections for context: {}\n",
                sections.join(", ")
            ));
        }

        if !self.profession.allowed_tools.is_empty() {
            parts.push(format!(
                "You may use these tools: {}\n",
                self.profession.allowed_tools.join(", ")
            ));
        }

        // Skill instructions
        for prompt in &self.skill_prompts {
            parts.push(prompt.clone());
        }
        if !self.skill_tools.is_empty() {
            parts.push(format!(
                "Additional tools from your skills: {}\n",
                self.skill_tools.join(", ")
            ));
        }

        // Role-specific execution mandates
        match self.profession.id.as_str() {
            "coder" => {
                parts.push("\n## Execution Mandate\n".to_string());
                parts.push("You MUST write or edit files using `write_file` or `edit_file` tools. ".to_string());
                parts.push("Reading and analyzing code is NOT enough — you must produce ACTUAL file changes. ".to_string());
                parts.push("Before handing off, verify that your changes exist on disk by reading them back.\n".to_string());
            }
            "documenter" => {
                parts.push("\n## Execution Mandate\n".to_string());
                parts.push("When documenting a completed relay run, you MUST update the status of all related spec sections ".to_string());
                parts.push("(plans, tests, designs, etc.) from `in_progress`/`draft` to `done` or the appropriate final status ".to_string());
                parts.push("using `write_specs`. Do NOT leave specs in an intermediate state.\n".to_string());
            }
            "advisor" | "architect" | "planner" => {
                parts.push("\n## Accuracy Mandate\n".to_string());
                parts.push("When referencing files in your handoff work_product, ONLY list files that ACTUALLY exist. ".to_string());
                parts.push("You do NOT have `shell` or `search` directly. To verify file paths, use `dispatch` with `agent=\"gofer\"` ".to_string());
                parts.push("to run `shell`/`search` errands (e.g., list files, grep for patterns). ".to_string());
                parts.push("As a fallback, you can use `read_file` to test a single path. ".to_string());
                parts.push("Do NOT hallucinate file paths (e.g., `.tsx` when the project uses `.vue`, or `README.md` that does not exist).\n".to_string());
            }
            "reviewer" => {
                parts.push("\n## Verification Mandate\n".to_string());
                parts.push("You MUST run `shell cargo check` (or equivalent) to verify code compiles before approving. ".to_string());
                parts.push("You MUST run `shell cargo test` (or equivalent) to verify tests pass before approving. ".to_string());
                parts.push("If compilation or tests fail, REJECT the handoff and list the errors. ".to_string());
                parts.push("A review without compile/test verification is INVALID.\n".to_string());
            }
            _ => {}
        }

        // Relay mode instruction — prevents chat-only tools from being called in pipeline
        if self.relay_mode {
            parts.push("\n## Relay Mode\n".to_string());
            parts.push("You are running inside an autonomous relay pipeline. ".to_string());
            parts.push("Do NOT call `bring_in` or `spawn_relay` — those are chat-mode tools. ".to_string());
            parts.push("When your work is complete, simply stop making tool calls and the pipeline will advance automatically.\n".to_string());
        }

        // Tool usage tips — prevents empty-arg tool calls
        parts.push("\n## Tool Usage Tips\n".to_string());
        parts.push("When calling `write_specs`, you MUST provide both arguments: `section_id` and `content`. Example:\n".to_string());
        parts.push("  {\"section_id\": \"tests\", \"content\": \"# Tests\\n\\n...\"}\n".to_string());
        parts.push("When calling `read_file`, `write_file`, or `edit_file`, always provide the full `path`.\n".to_string());

        // Constraints
        parts.push(format!(
            "\n## Constraints\n- Max turns before handoff: {}\n- Token budget: {}\n",
            self.profession.max_turns,
            self.profession.token_budget
        ));

        parts.join("\n")
    }

    /// Build the initial user message from handoff + relevant specs.
    pub fn render_user_message(&self, handoff_summary: &str, spec_summary: &str) -> Vec<ChatMessage> {
        let mut content = String::new();
        if !handoff_summary.is_empty() {
            content.push_str("## Previous Agent's Handoff\n\n");
            content.push_str(handoff_summary);
            content.push_str("\n\n---\n\n");
        }
        if !spec_summary.is_empty() {
            content.push_str("## Relevant Specs\n\n");
            content.push_str(spec_summary);
            content.push_str("\n\n---\n\n");
        }
        content.push_str("Begin your work now. When you are ready to hand off, call the `handoff` tool.");

        vec![ChatMessage::user(&content)]
    }

    /// Build a complete ToolChatRequest for this agent's turn.
    pub fn build_chat_request(
        &self,
        tools: Vec<ToolDefinition>,
        handoff_summary: &str,
        spec_summary: &str,
    ) -> ToolChatRequest {
        let system = self.render_system_prompt();
        let messages = self.render_user_message(handoff_summary, spec_summary);
        ToolChatRequest {
            messages,
            tools,
            system_prompt: Some(system),
            thinking_budget: if self.thinking_enabled {
                Some(self.thinking_budget)
            } else {
                None
            },
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::profession::ProfessionRegistry;
    use crate::relay::soul::SoulConfig;

    #[test]
    fn test_agent_spawn() {
        let profession = ProfessionRegistry::new().get("planner").unwrap().clone();
        let soul = SoulConfig::parse("planner", "# Soul of the Planner\n\n## Core Values\n- Careful planning\n").unwrap();
        let model = ModelConfig::standard();
        let agent = AgentInstance::spawn(profession, soul, model);
        assert!(agent.id.starts_with("agent-"));
        assert_eq!(agent.profession.id, "planner");
    }

    #[test]
    fn test_render_system_prompt() {
        let profession = ProfessionRegistry::new().get("planner").unwrap().clone();
        let soul = SoulConfig::parse("planner", "# Soul of the Planner\n\n## Core Values\n- Careful planning\n").unwrap();
        let model = ModelConfig::standard();
        let agent = AgentInstance::spawn(profession, soul, model);
        let prompt = agent.render_system_prompt();
        assert!(prompt.contains("Soul of the Planner"));
        assert!(prompt.contains("Profession: Planner"));
        assert!(prompt.contains("You OWN these spec sections"));
        assert!(prompt.contains("goals"));
        assert!(prompt.contains("plans"));
    }

    #[test]
    fn test_model_tiers() {
        let cheap = ModelConfig::cheap();
        assert!(cheap.model.contains("haiku"));

        let standard = ModelConfig::standard();
        assert!(standard.model.contains("sonnet"));

        let strong = ModelConfig::strong();
        assert!(strong.model.contains("opus"));
    }
}
