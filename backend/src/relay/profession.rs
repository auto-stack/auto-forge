//! Profession Registry
//!
//! Defines agent professions — what each agent can and cannot do.
//! Each profession specifies owned spec sections, available tools,
//! handoff rules, and token budgets.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::forge::SectionType;
use crate::relay::config::ModelTier;

/// A profession defines an agent's role, scope, and constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profession {
    pub id: String,
    pub name: String,
    pub phase: ForgePhase,
    /// Sections this profession can write to.
    pub owned_sections: Vec<SectionType>,
    /// Sections this profession can read for context.
    pub readable_sections: Vec<SectionType>,
    /// Tool names this profession is allowed to use.
    pub allowed_tools: Vec<String>,
    /// Professions that may receive handoffs from this one.
    pub handoff_to: Vec<String>,
    /// Professions that may be dispatched to as errand agents from this one.
    pub dispatchable_to: Vec<String>,
    /// Human approval is required before handing off to these professions.
    pub approval_gates: Vec<String>,
    /// Max LLM turns before forced handoff.
    pub max_turns: u32,
    /// Default token budget for this profession.
    pub token_budget: u64,
    /// Enable Claude extended thinking mode for this profession.
    #[serde(default)]
    pub thinking_enabled: bool,
    /// Thinking budget in tokens (e.g. 1024, 2048). Only used when thinking_enabled is true.
    #[serde(default)]
    pub thinking_budget: u32,
    /// Base skills that all agents of this profession receive.
    #[serde(default)]
    pub base_skills: Vec<String>,
    /// Minimum model tier this profession can use.
    #[serde(default)]
    pub min_tier: ModelTier,
    /// Maximum model tier this profession can use.
    #[serde(default)]
    pub max_tier: ModelTier,
}

/// Lifecycle phase of the spec-driven workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgePhase {
    Intake,
    Discovery,
    GoalGate,
    Design,
    Planning,
    Execution,
    Verification,
    Report,
    Errand,
}

impl ForgePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            ForgePhase::Intake => "intake",
            ForgePhase::Discovery => "discovery",
            ForgePhase::GoalGate => "goal_gate",
            ForgePhase::Design => "design",
            ForgePhase::Planning => "planning",
            ForgePhase::Execution => "execution",
            ForgePhase::Verification => "verification",
            ForgePhase::Report => "report",
            ForgePhase::Errand => "errand",
        }
    }
}

/// Registry of built-in and custom professions.
pub struct ProfessionRegistry {
    professions: HashMap<String, Profession>,
}

fn config_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge")
}

fn professions_path() -> PathBuf {
    config_dir().join("professions.json")
}

/// Load professions from disk.
pub fn load_professions() -> Vec<Profession> {
    let path = professions_path();
    if !path.exists() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to read professions.json: {}", e);
            return Vec::new();
        }
    };
    match serde_json::from_str(&content) {
        Ok(professions) => professions,
        Err(e) => {
            eprintln!("Warning: failed to parse professions.json: {}", e);
            Vec::new()
        }
    }
}

/// Save professions to disk.
pub fn save_professions(professions: &[Profession]) -> Result<(), ProfessionError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| ProfessionError::LoadError(format!("create dir {}: {}", dir.display(), e)))?;
    let path = professions_path();
    let content = serde_json::to_string_pretty(professions)
        .map_err(|e| ProfessionError::ParseError(format!("serialize: {}", e)))?;
    std::fs::write(&path, content)
        .map_err(|e| ProfessionError::LoadError(format!("write {}: {}", path.display(), e)))?;
    Ok(())
}

/// Generate the 9 default built-in professions.
pub fn generate_default_professions() -> Vec<Profession> {
    vec![
        Profession {
            id: String::from("assistant"),
            name: String::from("Assistant"),
            phase: ForgePhase::Intake,
            owned_sections: vec![],
            readable_sections: vec![],
            allowed_tools: vec![
                String::from("bring_in"),
                String::from("dispatch"),
                String::from("spawn_relay"),
                String::from("shell"),
                String::from("query_wiki"),
                String::from("list_wiki"),
            ],
            handoff_to: vec![
                String::from("advisor"),
                String::from("coder"),
            ],
            approval_gates: vec![],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 12,
            token_budget: 2_000_000,
            thinking_enabled: false,
            thinking_budget: 0,
            base_skills: Vec::new(),
            min_tier: ModelTier::Min,
            max_tier: ModelTier::Mid,
        },
        Profession {
            id: String::from("advisor"),
            name: String::from("Advisor"),
            phase: ForgePhase::Discovery,
            owned_sections: vec![SectionType::Goals],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
            ],
            allowed_tools: vec![
                String::from("read_specs"),
                String::from("write_specs"),
                String::from("update_spec"),
                String::from("write_goals"),
                String::from("list_specs"),
                String::from("read_file"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("bring_in"),
                String::from("dispatch"),
                String::from("spawn_relay"),
            ],
            handoff_to: vec![String::from("architect")],
            approval_gates: vec![String::from("architect")],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 40,
            token_budget: 8_000_000,
            thinking_enabled: true,
            thinking_budget: 1024,
            base_skills: Vec::new(),
            min_tier: ModelTier::Mid,
            max_tier: ModelTier::Max,
        },
        Profession {
            id: String::from("architect"),
            name: String::from("Architect"),
            phase: ForgePhase::Design,
            owned_sections: vec![
                SectionType::Architecture,
                SectionType::Designs,
            ],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
            ],
            allowed_tools: vec![
                String::from("read_specs"),
                String::from("write_specs"),
                String::from("update_spec"),
                String::from("list_specs"),
                String::from("read_file"),
                String::from("write_file"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("bring_in"),
                String::from("spawn_relay"),
            ],
            handoff_to: vec![String::from("planner")],
            approval_gates: vec![],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 40,
            token_budget: 12_000_000,
            thinking_enabled: true,
            thinking_budget: 2048,
            base_skills: Vec::new(),
            min_tier: ModelTier::Mid,
            max_tier: ModelTier::Max,
        },
        Profession {
            id: String::from("planner"),
            name: String::from("Planner"),
            phase: ForgePhase::Planning,
            owned_sections: vec![SectionType::Plans],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
            ],
            allowed_tools: vec![
                String::from("read_specs"),
                String::from("write_specs"),
                String::from("update_spec"),
                String::from("list_specs"),
                String::from("read_file"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("bring_in"),
            ],
            handoff_to: vec![String::from("tester")],
            approval_gates: vec![],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 40,
            token_budget: 8_000_000,
            thinking_enabled: true,
            thinking_budget: 1024,
            base_skills: Vec::new(),
            min_tier: ModelTier::Mid,
            max_tier: ModelTier::Pro,
        },
        Profession {
            id: String::from("tester"),
            name: String::from("Tester"),
            phase: ForgePhase::Planning,
            owned_sections: vec![SectionType::Tests],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
            ],
            allowed_tools: vec![
                String::from("read_specs"),
                String::from("write_specs"),
                String::from("update_spec"),
                String::from("list_specs"),
                String::from("read_file"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("bring_in"),
            ],
            handoff_to: vec![String::from("coder")],
            approval_gates: vec![],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 40,
            token_budget: 8_000_000,
            thinking_enabled: true,
            thinking_budget: 1024,
            base_skills: Vec::new(),
            min_tier: ModelTier::Min,
            max_tier: ModelTier::Mid,
        },
        Profession {
            id: String::from("coder"),
            name: String::from("Coder"),
            phase: ForgePhase::Execution,
            owned_sections: vec![],
            readable_sections: vec![
                SectionType::Plans,
                SectionType::Designs,
                SectionType::Tests,
            ],
            allowed_tools: vec![
                String::from("read_file"),
                String::from("write_file"),
                String::from("edit_file"),
                String::from("shell"),
                String::from("search"),
                String::from("read_specs"),
                String::from("list_specs"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("create_wiki_page"),
                String::from("update_wiki_page"),
                String::from("dispatch"),
            ],
            handoff_to: vec![
                String::from("tester"),
                String::from("architect"),
            ],
            approval_gates: vec![],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 30,
            token_budget: 20_000_000,
            thinking_enabled: true,
            thinking_budget: 2048,
            base_skills: Vec::new(),
            min_tier: ModelTier::Mid,
            max_tier: ModelTier::Max,
        },
        Profession {
            id: String::from("reviewer"),
            name: String::from("Reviewer"),
            phase: ForgePhase::Verification,
            owned_sections: vec![SectionType::Reviews],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
                SectionType::Reviews,
                SectionType::Reports,
            ],
            allowed_tools: vec![
                String::from("read_file"),
                String::from("write_file"),
                String::from("edit_file"),
                String::from("shell"),
                String::from("search"),
                String::from("read_specs"),
                String::from("write_specs"),
                String::from("update_spec"),
                String::from("list_specs"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("dispatch"),
            ],
            handoff_to: vec![String::from("documenter")],
            approval_gates: vec![],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 40,
            token_budget: 15_000_000,
            thinking_enabled: true,
            thinking_budget: 1024,
            base_skills: Vec::new(),
            min_tier: ModelTier::Mid,
            max_tier: ModelTier::Max,
        },
        Profession {
            id: String::from("documenter"),
            name: String::from("Documenter"),
            phase: ForgePhase::Report,
            owned_sections: vec![SectionType::Reports],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
                SectionType::Reviews,
                SectionType::Reports,
            ],
            allowed_tools: vec![
                String::from("read_file"),
                String::from("read_specs"),
                String::from("write_specs"),
                String::from("list_specs"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("create_wiki_page"),
                String::from("update_wiki_page"),
                String::from("get_relay_state"),
                String::from("get_checkpoint_diff"),
            ],
            handoff_to: vec![],
            approval_gates: vec![],
            dispatchable_to: vec![],
            max_turns: 20,
            token_budget: 4_000_000,
            thinking_enabled: false,
            thinking_budget: 0,
            base_skills: Vec::new(),
            min_tier: ModelTier::Min,
            max_tier: ModelTier::Mid,
        },
        Profession {
            id: String::from("gofer"),
            name: String::from("Gofer"),
            phase: ForgePhase::Errand,
            owned_sections: vec![],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
            ],
            allowed_tools: vec![
                String::from("shell"),
                String::from("read_file"),
                String::from("search"),
                String::from("list_specs"),
                String::from("read_specs"),
                String::from("query_wiki"),
                String::from("list_wiki"),
            ],
            handoff_to: vec![],
            approval_gates: vec![],
            dispatchable_to: vec![],
            max_turns: 20,
            token_budget: 4_000_000,
            thinking_enabled: false,
            thinking_budget: 0,
            base_skills: Vec::new(),
            min_tier: ModelTier::Min,
            max_tier: ModelTier::Lite,
        },
        // ─── Superpower Professions ──────────────────────────────────────────────
        Profession {
            id: String::from("super-advisor"),
            name: String::from("Super Advisor"),
            phase: ForgePhase::Planning,
            owned_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
            ],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
                SectionType::Reviews,
                SectionType::Reports,
            ],
            allowed_tools: vec![
                String::from("read_specs"),
                String::from("write_specs"),
                String::from("update_spec"),
                String::from("write_goals"),
                String::from("list_specs"),
                String::from("read_file"),
                String::from("write_file"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("bring_in"),
                String::from("dispatch"),
                String::from("spawn_relay"),
            ],
            handoff_to: vec![String::from("super-coder")],
            approval_gates: vec![String::from("super-coder")],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 50,
            token_budget: 15_000_000,
            thinking_enabled: true,
            thinking_budget: 4096,
            base_skills: Vec::new(),
            min_tier: ModelTier::Mid,
            max_tier: ModelTier::Max,
        },
        Profession {
            id: String::from("super-coder"),
            name: String::from("Super Coder"),
            phase: ForgePhase::Execution,
            owned_sections: vec![],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
            ],
            allowed_tools: vec![
                String::from("read_file"),
                String::from("write_file"),
                String::from("edit_file"),
                String::from("shell"),
                String::from("search"),
                String::from("read_specs"),
                String::from("list_specs"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("create_wiki_page"),
                String::from("update_wiki_page"),
                String::from("dispatch"),
            ],
            handoff_to: vec![String::from("super-tester")],
            approval_gates: vec![],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 30,
            token_budget: 20_000_000,
            thinking_enabled: true,
            thinking_budget: 2048,
            base_skills: Vec::new(),
            min_tier: ModelTier::Mid,
            max_tier: ModelTier::Max,
        },
        Profession {
            id: String::from("super-tester"),
            name: String::from("Super Tester"),
            phase: ForgePhase::Report,
            owned_sections: vec![
                SectionType::Reviews,
                SectionType::Reports,
            ],
            readable_sections: vec![
                SectionType::Goals,
                SectionType::Architecture,
                SectionType::Designs,
                SectionType::Plans,
                SectionType::Tests,
                SectionType::Reviews,
                SectionType::Reports,
            ],
            allowed_tools: vec![
                String::from("read_file"),
                String::from("shell"),
                String::from("search"),
                String::from("read_specs"),
                String::from("write_specs"),
                String::from("update_spec"),
                String::from("list_specs"),
                String::from("query_wiki"),
                String::from("list_wiki"),
                String::from("create_wiki_page"),
                String::from("update_wiki_page"),
                String::from("dispatch"),
            ],
            handoff_to: vec![],
            approval_gates: vec![],
            dispatchable_to: vec![String::from("gofer")],
            max_turns: 40,
            token_budget: 15_000_000,
            thinking_enabled: true,
            thinking_budget: 2048,
            base_skills: Vec::new(),
            min_tier: ModelTier::Mid,
            max_tier: ModelTier::Max,
        },
    ]
}

/// Load professions, merging missing defaults with existing ones.
pub fn load_or_generate_professions() -> Vec<Profession> {
    let existing = load_professions();
    let defaults = generate_default_professions();

    if existing.is_empty() {
        let _ = save_professions(&defaults);
        return defaults;
    }

    let mut merged = existing;
    let mut changed = false;

    for default in &defaults {
        if let Some(idx) = merged.iter().position(|p| p.id == default.id) {
            // Sync fields that may change via code updates
            if merged[idx].token_budget != default.token_budget {
                merged[idx].token_budget = default.token_budget;
                changed = true;
            }
            if merged[idx].max_turns != default.max_turns {
                merged[idx].max_turns = default.max_turns;
                changed = true;
            }
            if merged[idx].thinking_enabled != default.thinking_enabled {
                merged[idx].thinking_enabled = default.thinking_enabled;
                changed = true;
            }
            if merged[idx].thinking_budget != default.thinking_budget {
                merged[idx].thinking_budget = default.thinking_budget;
                changed = true;
            }
            if merged[idx].min_tier != default.min_tier {
                merged[idx].min_tier = default.min_tier;
                changed = true;
            }
            if merged[idx].max_tier != default.max_tier {
                merged[idx].max_tier = default.max_tier;
                changed = true;
            }
            if merged[idx].allowed_tools != default.allowed_tools {
                merged[idx].allowed_tools = default.allowed_tools.clone();
                changed = true;
            }
            if merged[idx].owned_sections != default.owned_sections {
                merged[idx].owned_sections = default.owned_sections.clone();
                changed = true;
            }
            if merged[idx].readable_sections != default.readable_sections {
                merged[idx].readable_sections = default.readable_sections.clone();
                changed = true;
            }
            if merged[idx].handoff_to != default.handoff_to {
                merged[idx].handoff_to = default.handoff_to.clone();
                changed = true;
            }
            if merged[idx].dispatchable_to != default.dispatchable_to {
                merged[idx].dispatchable_to = default.dispatchable_to.clone();
                changed = true;
            }
            if merged[idx].approval_gates != default.approval_gates {
                merged[idx].approval_gates = default.approval_gates.clone();
                changed = true;
            }
            if merged[idx].base_skills != default.base_skills {
                merged[idx].base_skills = default.base_skills.clone();
                changed = true;
            }
        } else {
            // Add missing default profession
            merged.push(default.clone());
            changed = true;
        }
    }

    if changed {
        let _ = save_professions(&merged);
    }
    merged
}

impl ProfessionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            professions: HashMap::new(),
        };
        for profession in load_or_generate_professions() {
            registry.register(profession);
        }
        registry
    }

    pub fn register(&mut self, profession: Profession) {
        self.professions.insert(profession.id.clone(), profession);
    }

    pub fn get(&self, id: &str) -> Option<&Profession> {
        self.professions.get(id)
    }

    pub fn list(&self) -> Vec<&Profession> {
        self.professions.values().collect()
    }

    /// Load custom professions from `.autoforge/professions/{name}.yaml` files.
    pub fn load_custom(&mut self, dir: &std::path::Path) -> Result<(), ProfessionError> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Ok(()) };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() == Some("yaml".as_ref()) || path.extension() == Some("yml".as_ref()) {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| ProfessionError::LoadError(format!("{}: {}", path.display(), e)))?;
                let profession: Profession = serde_yaml::from_str(&content)
                    .map_err(|e| ProfessionError::ParseError(format!("{}: {}", path.display(), e)))?;
                self.register(profession);
            }
        }
        Ok(())
    }
}

impl Default for ProfessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum ProfessionError {
    LoadError(String),
    ParseError(String),
}

impl std::fmt::Display for ProfessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfessionError::LoadError(s) => write!(f, "Load error: {}", s),
            ProfessionError::ParseError(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl std::error::Error for ProfessionError {}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_professions_loaded() {
        let registry = ProfessionRegistry::new();
        assert!(registry.get("assistant").is_some());
        assert!(registry.get("advisor").is_some());
        assert!(registry.get("architect").is_some());
        assert!(registry.get("planner").is_some());
        assert!(registry.get("coder").is_some());
        assert!(registry.get("tester").is_some());
        assert!(registry.get("reviewer").is_some());
        assert!(registry.get("documenter").is_some());
        assert!(registry.get("intaker").is_none());
    }

    #[test]
    fn test_architect_owned_sections() {
        let registry = ProfessionRegistry::new();
        let arch = registry.get("architect").unwrap();
        assert!(arch.owned_sections.contains(&SectionType::Architecture));
        assert!(arch.owned_sections.contains(&SectionType::Designs));
        assert!(!arch.owned_sections.contains(&SectionType::Goals));
    }

    #[test]
    fn test_coder_cannot_write_specs() {
        let registry = ProfessionRegistry::new();
        let coder = registry.get("coder").unwrap();
        assert!(coder.owned_sections.is_empty());
    }

    #[test]
    fn test_advisor_has_approval_gate_for_architect() {
        let registry = ProfessionRegistry::new();
        let advisor = registry.get("advisor").unwrap();
        assert!(advisor.approval_gates.contains(&"architect".to_string()));
    }

    #[test]
    fn test_list_returns_all_builtin() {
        let registry = ProfessionRegistry::new();
        let list = registry.list();
        assert_eq!(list.len(), 12);
    }

    #[test]
    fn test_super_advisor_has_all_design_sections() {
        let registry = ProfessionRegistry::new();
        let sa = registry.get("super-advisor").unwrap();
        assert!(sa.owned_sections.contains(&SectionType::Goals));
        assert!(sa.owned_sections.contains(&SectionType::Architecture));
        assert!(sa.owned_sections.contains(&SectionType::Designs));
        assert!(sa.owned_sections.contains(&SectionType::Plans));
        assert!(sa.owned_sections.contains(&SectionType::Tests));
    }

    #[test]
    fn test_super_tester_has_review_and_report_sections() {
        let registry = ProfessionRegistry::new();
        let st = registry.get("super-tester").unwrap();
        assert!(st.owned_sections.contains(&SectionType::Reviews));
        assert!(st.owned_sections.contains(&SectionType::Reports));
    }

    #[test]
    fn test_super_professions_handoff_chain() {
        let registry = ProfessionRegistry::new();
        let sa = registry.get("super-advisor").unwrap();
        let sc = registry.get("super-coder").unwrap();
        let st = registry.get("super-tester").unwrap();
        assert!(sa.handoff_to.contains(&"super-coder".to_string()));
        assert!(sc.handoff_to.contains(&"super-tester".to_string()));
        assert!(st.handoff_to.is_empty());
    }
}
