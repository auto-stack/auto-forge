//! Built-in Flow Specifications
//!
//! Pre-defined flow specs for common development workflows.
//! Also supports loading custom flows from `.autoforge/flows/*.yml`.

use crate::relay::flow::FlowSpec;
use std::collections::HashMap;
use std::sync::Mutex;

// ─── Built-in Flow YAMLs ─────────────────────────────────────────────────────

const BUILTIN_FLOWS: &[(&str, &str)] = &[
    ("standard-spec-driven-development", include_str!("builtin/standard-spec.yml")),
    ("fast-track", include_str!("builtin/fast-track.yml")),
    ("auto-discovery", include_str!("builtin/auto-discovery.yml")),
    ("post-discovery", include_str!("builtin/post-discovery.yml")),
    ("bug-fix", include_str!("builtin/bug-fix.yml")),
    ("goal-discovery", include_str!("builtin/goal-discovery.yml")),
    ("doc-patch", include_str!("builtin/doc-patch.yml")),
    ("spec-tweak", include_str!("builtin/spec-tweak.yml")),
    ("superpower", include_str!("builtin/superpower.yml")),
];

// ─── Flow Registry ───────────────────────────────────────────────────────────

/// Global registry of all available flows (built-in + YAML-loaded).
pub struct FlowRegistry {
    flows: HashMap<String, FlowSpec>,
}

impl FlowRegistry {
    /// Create a new registry and load all flows.
    pub fn new(data_dir: &std::path::Path) -> Self {
        let mut registry = Self {
            flows: HashMap::new(),
        };
        registry.load_builtin();
        registry.load_from_yaml(data_dir);
        registry
    }

    /// Load only built-in flows (useful for tests).
    pub fn load_builtins_only() -> Self {
        let mut registry = Self {
            flows: HashMap::new(),
        };
        registry.load_builtin();
        registry
    }

    /// Get a flow by ID. Returns built-in if YAML override not found.
    pub fn get(&self, flow_id: &str) -> Option<FlowSpec> {
        self.flows.get(flow_id).cloned()
    }

    /// List all available flow IDs.
    pub fn list(&self) -> Vec<String> {
        self.flows.keys().cloned().collect()
    }

    fn load_builtin(&mut self) {
        for (_id, yaml) in BUILTIN_FLOWS {
            match serde_yaml::from_str::<FlowSpec>(yaml) {
                Ok(flow) => {
                    self.flows.insert(flow.id.clone(), flow);
                }
                Err(e) => {
                    tracing::error!("Failed to parse built-in flow YAML: {}", e);
                }
            }
        }
    }

    fn load_from_yaml(&mut self, data_dir: &std::path::Path) {
        let flows_dir = data_dir.join(".autoforge").join("flows");
        if !flows_dir.is_dir() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&flows_dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "yml" && ext != "yaml" {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_yaml::from_str::<FlowSpec>(&content) {
                        Ok(flow) => {
                            tracing::info!("Loaded flow '{}' from {:?}", flow.id, path);
                            self.flows.insert(flow.id.clone(), flow);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse flow YAML {:?}: {}", path, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read flow YAML {:?}: {}", path, e);
                }
            }
        }
    }
}

// ─── Global Registry ─────────────────────────────────────────────────────────

/// Lazy-initialized global flow registry.
static FLOW_REGISTRY: Mutex<Option<FlowRegistry>> = Mutex::new(None);

/// Initialize the global flow registry from the current project path.
/// Call once at startup or after opening a project.
pub fn init_flow_registry() {
    if let Some(project_path) = crate::forge::current_project_path() {
        let path = std::path::PathBuf::from(project_path);
        let mut guard = FLOW_REGISTRY.lock().unwrap();
        *guard = Some(FlowRegistry::new(&path));
    }
}

/// Get a flow from the global registry.
/// Auto-initializes on first call if a project is open.
pub fn get_flow(flow_id: &str) -> Option<FlowSpec> {
    // Support both hyphen and underscore variants (e.g. "post_discovery" ↔ "post-discovery")
    let alt_id = if flow_id.contains('_') {
        flow_id.replace('_', "-")
    } else {
        flow_id.replace('-', "_")
    };
    {
        let guard = FLOW_REGISTRY.lock().unwrap();
        if let Some(ref registry) = *guard {
            return registry.get(flow_id).or_else(|| registry.get(&alt_id));
        }
    }
    // Auto-initialize if not yet loaded
    init_flow_registry();
    let guard = FLOW_REGISTRY.lock().unwrap();
    guard.as_ref()?.get(flow_id).or_else(|| guard.as_ref()?.get(&alt_id))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::flow::GateType;

    fn builtin(flow_id: &str) -> FlowSpec {
        FlowRegistry::load_builtins_only()
            .get(flow_id)
            .unwrap_or_else(|| panic!("Built-in flow '{}' not found", flow_id))
    }

    #[test]
    fn test_standard_flow_has_nine_steps() {
        let flow = builtin("standard-spec-driven-development");
        assert_eq!(flow.steps.len(), 9);
        assert_eq!(flow.steps[0].profession_id, "assistant");
        assert_eq!(flow.steps[1].profession_id, "advisor");
        assert_eq!(flow.steps[2].profession_id, "architect");
        assert_eq!(flow.steps[3].profession_id, "planner");
        assert_eq!(flow.steps[4].profession_id, "tester");
        assert_eq!(flow.steps[5].profession_id, "coder");
        assert_eq!(flow.steps[6].profession_id, "tester");
        assert_eq!(flow.steps[7].profession_id, "reviewer");
        assert_eq!(flow.steps[8].profession_id, "documenter");
    }

    #[test]
    fn test_standard_flow_has_auto_gate_at_advisor() {
        let flow = builtin("standard-spec-driven-development");
        assert_eq!(flow.steps[1].gate, GateType::Auto);
        assert_eq!(flow.steps[2].gate, GateType::Auto);
        assert_eq!(flow.steps[3].gate, GateType::Auto);
        assert_eq!(flow.steps[4].gate, GateType::Auto);
    }

    #[test]
    fn test_advisor_step_has_tool_guard() {
        let flow = builtin("standard-spec-driven-development");
        let advisor_step = flow.get_step("discover").unwrap();
        assert!(advisor_step.tool_guard.is_some());
        let guard = advisor_step.tool_guard.as_ref().unwrap();
        assert_eq!(guard.required_first, vec!["write_specs", "write_goals"]);
        assert!(guard.always_allowed.contains(&"list_specs".to_string()));
    }

    #[test]
    fn test_fast_track_flow_has_two_steps() {
        let flow = builtin("fast-track");
        assert_eq!(flow.steps.len(), 2);
        assert_eq!(flow.steps[0].profession_id, "assistant");
        assert_eq!(flow.steps[1].profession_id, "coder");
    }

    #[test]
    fn test_post_discovery_flow_has_seven_steps() {
        let flow = builtin("post-discovery");
        assert_eq!(flow.steps.len(), 7);
        assert_eq!(flow.steps[0].profession_id, "architect");
        assert_eq!(flow.steps[1].profession_id, "planner");
        assert_eq!(flow.steps[2].profession_id, "tester");
        assert_eq!(flow.steps[3].profession_id, "coder");
        assert_eq!(flow.steps[4].profession_id, "tester");
        assert_eq!(flow.steps[5].profession_id, "reviewer");
        assert_eq!(flow.steps[6].profession_id, "documenter");
    }

    #[test]
    fn test_bug_fix_flow_has_loop() {
        let flow = builtin("bug-fix");
        assert_eq!(flow.steps.len(), 4);
        match &flow.steps[2].exit {
            crate::relay::flow::ExitRouting::Loop { target_step_id, max_iterations } => {
                assert_eq!(target_step_id, "code");
                assert_eq!(*max_iterations, 3);
            }
            _ => panic!("Expected Loop exit on tester step"),
        }
    }

    #[test]
    fn test_doc_patch_flow_has_two_steps() {
        let flow = builtin("doc-patch");
        assert_eq!(flow.steps.len(), 2);
        assert_eq!(flow.steps[0].profession_id, "assistant");
        assert_eq!(flow.steps[1].profession_id, "documenter");
    }

    #[test]
    fn test_spec_tweak_flow_has_two_steps() {
        let flow = builtin("spec-tweak");
        assert_eq!(flow.steps.len(), 2);
        assert_eq!(flow.steps[0].profession_id, "assistant");
        assert_eq!(flow.steps[1].profession_id, "advisor");
    }

    #[test]
    fn test_superpower_flow_has_three_steps() {
        let flow = builtin("superpower");
        assert_eq!(flow.steps.len(), 3);
        assert_eq!(flow.steps[0].profession_id, "super-advisor");
        assert_eq!(flow.steps[1].profession_id, "super-coder");
        assert_eq!(flow.steps[2].profession_id, "super-tester");
    }

    #[test]
    fn test_superpower_design_step_has_human_gate() {
        let flow = builtin("superpower");
        assert_eq!(flow.steps[0].gate, GateType::Human);
        assert_eq!(flow.steps[1].gate, GateType::Auto);
        assert_eq!(flow.steps[2].gate, GateType::Auto);
    }

    #[test]
    fn test_superpower_tester_step_has_loop_exit() {
        let flow = builtin("superpower");
        match &flow.steps[2].exit {
            crate::relay::flow::ExitRouting::Loop { target_step_id, max_iterations } => {
                assert_eq!(target_step_id, "implement");
                assert_eq!(*max_iterations, 3);
            }
            _ => panic!("Expected Loop exit on super-tester step"),
        }
    }

    #[test]
    fn test_spec_tweak_requires_read_specs_first() {
        let flow = builtin("spec-tweak");
        let tweak_step = flow.get_step("tweak").unwrap();
        assert!(tweak_step.tool_guard.is_some());
        let guard = tweak_step.tool_guard.as_ref().unwrap();
        assert_eq!(guard.required_first, vec!["read_specs"]);
        assert!(guard.always_allowed.contains(&"write_specs".to_string()));
    }
}
