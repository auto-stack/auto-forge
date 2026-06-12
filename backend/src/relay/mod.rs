//! AutoForge Relay — Multi-Agent Orchestration
//!
//! The Relay module implements spec-driven serial cooperation of
//! profession-bearing AI agents. Core principles:
//!
//! 1. **Token efficiency** — Handoff documents compress context; no chat history.
//! 2. **Quality over speed** — Human approval gates at spec boundaries.
//! 3. **Relay mode** — One agent holds the baton at a time.
//! 4. **Profession-based** — Each agent has a Soul, Profession, and Model.
//! 5. **Spec-driven** — Agents cooperate via the Ledger, not via chat.

pub mod agent;
pub mod api;
pub mod budget;
pub mod checkpoint;
pub mod config;
pub mod driver;
pub mod flow;
pub mod flows;
pub mod handoff;
pub mod handoff_store;
pub mod pipeline;
pub mod profession;
pub mod skills;
pub mod soul;
pub mod store;
pub mod task_plan;
pub mod task_plan_parser;
pub mod task_plan_registry;
pub mod title;
pub mod turn;

pub use agent::{AgentContext, AgentInstance, ModelConfig, Provider};
pub use budget::{BudgetAction, BudgetStrategy, BudgetTracker, CostReport, TokenBudget};
pub use checkpoint::{Checkpoint, CheckpointError, FileState};
pub use config::{AgentConfig, ApiSource, ConfigError, ConnectionTestResult, ModelDefinition, ModelTier};
pub use flow::{ExitRouting, FlowSpec, FlowStep, GateType};
pub use handoff::{ContextPointers, Decision, HandoffDocument, Question, SpecUpdate, TokenUsage, WorkProduct};
pub use pipeline::{AdvanceResult, GateDecision, PipelineEngine, PipelineStatus, StepRecord};
pub use profession::{ForgePhase, Profession, ProfessionError, ProfessionRegistry};
pub use skills::{SkillDefinition, SkillError, SkillRegistry};
pub use soul::{SoulConfig, SoulError};
pub use store::{RunStore, new_run_store, start_run, get_run, list_runs, advance_run, submit_handoff, resolve_gate, RunEntry, RunEvent, RunSummary, RunState, StepState, GateState};

use std::collections::HashMap;
use std::sync::LazyLock;

/// Global singleton — initialized once on first access.
static GLOBAL_REGISTRY: LazyLock<RelayRegistry> = LazyLock::new(RelayRegistry::new);

/// Global registry of Souls, Professions, and API Sources.
pub struct RelayRegistry {
    pub professions: ProfessionRegistry,
    /// Cached souls loaded from `.autoforge/souls/`.
    pub souls: HashMap<String, SoulConfig>,
    /// Configured API sources (LLM providers).
    pub api_sources: Vec<ApiSource>,
    /// Configured agent bindings (profession + soul + api source + tier).
    pub agent_configs: Vec<AgentConfig>,
    pub skills: SkillRegistry,
    souls_dir: std::path::PathBuf,
}

impl RelayRegistry {
    pub fn new() -> Self {
        let souls_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".autoforge")
            .join("souls");

        let mut registry = Self {
            professions: ProfessionRegistry::new(),
            souls: HashMap::new(),
            api_sources: Vec::new(),
            agent_configs: Vec::new(),
            skills: SkillRegistry::new(),
            souls_dir,
        };
        registry.api_sources = config::load_or_detect_api_sources();
        registry.agent_configs = config::load_or_generate_agent_configs(&registry.api_sources);
        registry.load_builtin_souls();
        let _ = registry.load_custom_souls();
        registry
    }

    /// Load built-in default souls embedded in the binary.
    fn load_builtin_souls(&mut self) {
        let defaults: [(&str, &str); 11] = [
            ("assistant", include_str!("souls/assistant.md")),
            ("advisor", include_str!("souls/advisor.md")),
            ("planner", include_str!("souls/planner.md")),
            ("architect", include_str!("souls/architect.md")),
            ("coder", include_str!("souls/coder.md")),
            ("tester", include_str!("souls/tester.md")),
            ("reviewer", include_str!("souls/reviewer.md")),
            ("documenter", include_str!("souls/documenter.md")),
            ("super-advisor", include_str!("souls/super-advisor.md")),
            ("super-coder", include_str!("souls/super-coder.md")),
            ("super-tester", include_str!("souls/super-tester.md")),
        ];
        for (id, markdown) in defaults {
            if let Ok(soul) = SoulConfig::parse(id, markdown) {
                self.souls.insert(id.to_string(), soul);
            }
        }
    }

    /// Load custom souls from `.autoforge/souls/` directory.
    fn load_custom_souls(&mut self) -> Result<(), SoulError> {
        let Ok(entries) = std::fs::read_dir(&self.souls_dir) else { return Ok(()) };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() == Some("md".as_ref()) {
                let id = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let soul = SoulConfig::load(&id, &self.souls_dir)?;
                self.souls.insert(id, soul);
            }
        }
        Ok(())
    }

    pub fn get_soul(&self, id: &str) -> Option<&SoulConfig> {
        self.souls.get(id)
    }

    pub fn get_profession(&self, id: &str) -> Option<&Profession> {
        self.professions.get(id)
    }

    /// Access the global singleton registry.
    pub fn global() -> &'static RelayRegistry {
        &GLOBAL_REGISTRY
    }

    /// Spawn an agent instance with the given profession and soul.
    pub fn spawn_agent(&self, profession_id: &str, soul_id: &str, model: ModelConfig) -> Option<AgentInstance> {
        let profession = self.professions.get(profession_id)?.clone();
        let base_skills = profession.base_skills.clone();
        let soul = self.souls.get(soul_id)?.clone();
        let agent = AgentInstance::spawn(profession, soul, model)
            .with_skills(&self.skills, &base_skills);
        Some(agent)
    }

    /// Spawn an agent from an AgentConfig, resolving model from the linked ApiSource.
    pub fn spawn_agent_from_config(&self, config: &AgentConfig) -> Option<AgentInstance> {
        let model = config::resolve_model(config, &self.api_sources)?;
        let profession = self.professions.get(&config.profession_id)?.clone();
        let soul = self.souls.get(&config.soul_id)?.clone();
        let mut agent = AgentInstance::spawn_named(profession, soul, model, config.name.clone())
            .with_skills(&self.skills, &config.equipped_skills);
        // Override thinking config from AgentConfig if set
        agent.thinking_enabled = config.thinking_enabled;
        if let Some(budget) = config.thinking_budget {
            agent.thinking_budget = budget;
        }
        Some(agent)
    }

    /// Find the default agent config for a given profession.
    pub fn default_agent_for(&self, profession_id: &str) -> Option<&AgentConfig> {
        self.agent_configs
            .iter()
            .find(|c| c.profession_id == profession_id && c.is_default)
    }
}

impl Default for RelayRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_registry_loads_builtin_souls() {
        let registry = RelayRegistry::new();
        assert!(registry.get_soul("planner").is_some());
        assert!(registry.get_soul("architect").is_some());
        assert!(registry.get_soul("coder").is_some());
    }

    #[test]
    fn test_spawn_agent() {
        let registry = RelayRegistry::new();
        let agent = registry.spawn_agent("planner", "planner", ModelConfig::standard());
        assert!(agent.is_some());
        let agent = agent.unwrap();
        assert_eq!(agent.profession.id, "planner");
        assert_eq!(agent.soul.id, "planner");
    }

    #[test]
    fn test_spawn_agent_unknown_profession() {
        let registry = RelayRegistry::new();
        let agent = registry.spawn_agent("nonexistent", "planner", ModelConfig::standard());
        assert!(agent.is_none());
    }
}
