//! Skill Registry
//!
//! Defines reusable capability packages ("skills") that can be equipped to
//! agent configs. Each skill grants additional tools and injects behavior
//! instructions into the system prompt.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A skill is a reusable capability package that extends an agent's abilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Tool names this skill grants access to.
    pub granted_tools: Vec<String>,
    /// Markdown instructions injected into the system prompt.
    pub prompt_fragment: String,
    /// Optional: max extra turns this skill consumes.
    #[serde(default)]
    pub extra_turns: u32,
    /// Optional: extra token budget.
    #[serde(default)]
    pub extra_token_budget: u64,
}

#[derive(Debug, Clone)]
pub enum SkillError {
    IoError(String),
    ParseError(String),
}

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillError::IoError(s) => write!(f, "IO error: {}", s),
            SkillError::ParseError(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl std::error::Error for SkillError {}

fn config_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge")
}

fn skills_path() -> PathBuf {
    config_dir().join("skills.json")
}

/// Load skills from disk.
pub fn load_skills() -> Vec<SkillDefinition> {
    let path = skills_path();
    if !path.exists() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to read skills.json: {}", e);
            return Vec::new();
        }
    };
    match serde_json::from_str(&content) {
        Ok(skills) => skills,
        Err(e) => {
            eprintln!("Warning: failed to parse skills.json: {}", e);
            Vec::new()
        }
    }
}

/// Save skills to disk.
pub fn save_skills(skills: &[SkillDefinition]) -> Result<(), SkillError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| SkillError::IoError(format!("create dir {}: {}", dir.display(), e)))?;
    let path = skills_path();
    let content = serde_json::to_string_pretty(skills)
        .map_err(|e| SkillError::ParseError(format!("serialize: {}", e)))?;
    std::fs::write(&path, content)
        .map_err(|e| SkillError::IoError(format!("write {}: {}", path.display(), e)))?;
    Ok(())
}

/// Generate default skill definitions.
pub fn generate_default_skills() -> Vec<SkillDefinition> {
    vec![
        SkillDefinition {
            id: "code_review".into(),
            name: "Code Review".into(),
            description: "Read and analyze code with precision.".into(),
            granted_tools: vec!["read_file".into(), "search".into()],
            prompt_fragment: "You are thorough at reading code and searching for issues. Always cite line numbers. Focus on correctness, performance, and maintainability.".into(),
            extra_turns: 0,
            extra_token_budget: 0,
        },
        SkillDefinition {
            id: "shell_ops".into(),
            name: "Shell Operations".into(),
            description: "Run shell commands for exploration and automation.".into(),
            granted_tools: vec!["shell".into()],
            prompt_fragment: "You may run shell commands. Prefer `find` and `grep` for searching. Always explain what a command does before running it. Be cautious with destructive operations.".into(),
            extra_turns: 0,
            extra_token_budget: 0,
        },
        SkillDefinition {
            id: "spec_writer".into(),
            name: "Spec Writer".into(),
            description: "Write and edit spec sections in Auto format.".into(),
            granted_tools: vec!["write_specs".into(), "read_specs".into(), "list_specs".into()],
            prompt_fragment: "You write concise spec sections in Auto format. Preserve existing structure. Use clear headings and bullet points.".into(),
            extra_turns: 0,
            extra_token_budget: 0,
        },
        SkillDefinition {
            id: "guidelines_think".into(),
            name: "Think Before Acting".into(),
            description: "Universal behavioral rule: state assumptions, surface tradeoffs, ask when uncertain.".into(),
            granted_tools: vec![],
            prompt_fragment: "## Behavioral Guideline: Think Before Acting\n\n\
                - State your assumptions explicitly. If uncertain, ask.\n\
                - If multiple interpretations exist, present them — don't pick silently.\n\
                - If a simpler approach exists, say so. Push back when warranted.\n\
                - If something is unclear, stop. Name what's confusing. Ask.\n".into(),
            extra_turns: 0,
            extra_token_budget: 0,
        },
        SkillDefinition {
            id: "guidelines_simple".into(),
            name: "Simplicity First".into(),
            description: "Universal behavioral rule: minimum solution, no speculative features.".into(),
            granted_tools: vec![],
            prompt_fragment: "## Behavioral Guideline: Simplicity First\n\n\
                - Minimum code that solves the problem. Nothing speculative.\n\
                - No features beyond what was asked.\n\
                - No abstractions for single-use code.\n\
                - No 'flexibility' or 'configurability' that wasn't requested.\n\
                - If you write 200 lines and it could be 50, rewrite it.\n".into(),
            extra_turns: 0,
            extra_token_budget: 0,
        },
        SkillDefinition {
            id: "guidelines_surgical".into(),
            name: "Surgical Changes".into(),
            description: "For file-modifying agents: touch only what you must.".into(),
            granted_tools: vec![],
            prompt_fragment: "## Behavioral Guideline: Surgical Changes\n\n\
                - Touch only what you must. Don't 'improve' adjacent code, comments, or formatting.\n\
                - Don't refactor things that aren't broken.\n\
                - Match existing style, even if you'd do it differently.\n\
                - Remove imports/variables/functions that YOUR changes made unused.\n\
                - Don't remove pre-existing dead code unless asked.\n\
                - Every changed line should trace directly to the request.\n".into(),
            extra_turns: 0,
            extra_token_budget: 0,
        },
        SkillDefinition {
            id: "executing_plans".into(),
            name: "Executing Plans".into(),
            description: "Follow a written plan step by step and verify each task before handing off.".into(),
            granted_tools: vec![],
            prompt_fragment: "## Plan Execution Discipline\n\n\
                You are executing a pre-written plan. The plan is the single source of truth for what must be done.\n\n\
                1. Read the plan file at the start of your turn.\n\
                2. Extract every concrete task/checkbox from the plan and keep a mental checklist.\n\
                3. Execute tasks **strictly in order**. Do not skip a task because it looks optional.\n\
                4. After each task, run the verification command specified in the plan (e.g., `cargo check`, `pnpm vue-tsc --noEmit`).\n\
                5. If a verification fails, fix it before moving to the next task.\n\
                6. If a task requires changes in multiple stacks (backend + frontend + specs + i18n), touch **all** of them.\n\
                7. Before handing off, review your checklist: every task must be either completed or explicitly called out as blocked with a reason.\n\
                8. In your final response include a `## Task Completion` section listing every task and its status (`DONE` / `BLOCKED`).\n\
                9. If anything from the plan is missing or incomplete, you MUST flag it as `STATUS: INCOMPLETE` in your handoff.".into(),
            extra_turns: 0,
            extra_token_budget: 0,
        },
    ]
}

/// Load skills, merging missing defaults with existing ones.
pub fn load_or_generate_skills() -> Vec<SkillDefinition> {
    let existing = load_skills();
    let defaults = generate_default_skills();

    if existing.is_empty() {
        let _ = save_skills(&defaults);
        return defaults;
    }

    let mut merged = existing;
    let mut added = false;
    for default in &defaults {
        if !merged.iter().any(|s| s.id == default.id) {
            merged.push(default.clone());
            added = true;
        }
    }

    if added {
        let _ = save_skills(&merged);
    }
    merged
}

/// Global registry of skill definitions.
pub struct SkillRegistry {
    skills: HashMap<String, SkillDefinition>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            skills: HashMap::new(),
        };
        for skill in load_or_generate_skills() {
            registry.skills.insert(skill.id.clone(), skill);
        }
        registry
    }

    pub fn get(&self, id: &str) -> Option<&SkillDefinition> {
        self.skills.get(id)
    }

    pub fn list(&self) -> Vec<&SkillDefinition> {
        let mut values: Vec<_> = self.skills.values().collect();
        values.sort_by_key(|s| &s.name);
        values
    }

    pub fn register(&mut self, skill: SkillDefinition) {
        self.skills.insert(skill.id.clone(), skill);
    }

    pub fn remove(&mut self, id: &str) -> Option<SkillDefinition> {
        self.skills.remove(id)
    }

    pub fn save(&self) -> Result<(), SkillError> {
        let skills: Vec<_> = self.skills.values().cloned().collect();
        save_skills(&skills)
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_registry_defaults() {
        let registry = SkillRegistry::new();
        assert!(registry.get("code_review").is_some());
        assert!(registry.get("shell_ops").is_some());
        assert!(registry.get("spec_writer").is_some());
    }

    #[test]
    fn test_skill_granted_tools() {
        let registry = SkillRegistry::new();
        let review = registry.get("code_review").unwrap();
        assert!(review.granted_tools.contains(&"read_file".into()));
        assert!(review.granted_tools.contains(&"search".into()));
    }
}
