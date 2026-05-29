//! Built-in Flow Specifications
//!
//! Pre-defined flow specs for common development workflows.
//! Also supports loading custom flows from `.autoforge/flows/*.yml`.

use crate::relay::flow::{FlowSpec, FlowStep, GateType, StepValidator, ToolGuard};
use std::collections::HashMap;
use std::sync::Mutex;

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

    /// Get a flow by ID. Returns built-in if YAML override not found.
    pub fn get(&self, flow_id: &str) -> Option<FlowSpec> {
        self.flows.get(flow_id).cloned()
    }

    /// List all available flow IDs.
    pub fn list(&self) -> Vec<String> {
        self.flows.keys().cloned().collect()
    }

    fn load_builtin(&mut self) {
        let builtins = vec![
            standard_spec_flow(),
            fast_track_flow(),
            auto_discovery_flow(),
            post_discovery_flow(),
            bug_fix_flow(),
            goal_discovery_flow(),
            doc_patch_flow(),
            spec_tweak_flow(),
        ];
        for flow in builtins {
            self.flows.insert(flow.id.clone(), flow);
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

/// Default validators for the discover (advisor) step.
fn discover_validators() -> Vec<StepValidator> {
    vec![
        StepValidator::Any(vec![
            StepValidator::SpecUpdatesNonEmpty {
                sections: vec!["goals".to_string()],
            },
            StepValidator::WorkProductHasExtensions {
                exts: vec![".ad".to_string()],
            },
        ]),
    ]
}

/// Default tool guard for the discover (advisor) step.
/// Advisor must call write_specs before reading files or dispatching.
fn discover_tool_guard() -> ToolGuard {
    ToolGuard {
        required_first: vec!["write_specs".to_string(), "write_goals".to_string()],
        unlocks: HashMap::new(),
        always_allowed: vec!["list_specs".to_string(), "read_specs".to_string()],
        forbidden: vec![],
    }
}

/// Default validators for the design (architect) step.
fn design_validators() -> Vec<StepValidator> {
    vec![
        StepValidator::Any(vec![
            StepValidator::SpecUpdatesNonEmpty {
                sections: vec!["architecture".to_string(), "designs".to_string()],
            },
            StepValidator::DecisionsNonEmpty,
        ]),
    ]
}

/// Default tool guard for the design (architect) step.
fn design_tool_guard() -> ToolGuard {
    ToolGuard {
        required_first: vec!["update_spec".to_string()],
        unlocks: HashMap::new(),
        always_allowed: vec!["list_specs".to_string(), "read_specs".to_string(), "read_file".to_string(), "dispatch".to_string()],
        forbidden: vec!["write_specs".to_string()],
    }
}

/// Default validators for the plan (planner) step.
fn plan_validators() -> Vec<StepValidator> {
    vec![
        StepValidator::SpecUpdatesNonEmpty {
            sections: vec!["plans".to_string()],
        },
    ]
}

/// Default tool guard for the plan (planner) step.
fn plan_tool_guard() -> ToolGuard {
    ToolGuard {
        required_first: vec!["write_specs".to_string(), "update_spec".to_string()],
        unlocks: HashMap::new(),
        always_allowed: vec!["list_specs".to_string(), "read_specs".to_string(), "read_file".to_string()],
        forbidden: vec![],
    }
}

/// Default validators for the code (coder) step.
fn code_validators() -> Vec<StepValidator> {
    vec![
        StepValidator::WorkProductHasExtensions {
            exts: vec![".rs".to_string(), ".vue".to_string(), ".ts".to_string(), ".js".to_string()],
        },
    ]
}

/// Default validators for the test (tester) step.
fn test_validators() -> Vec<StepValidator> {
    vec![
        StepValidator::Any(vec![
            StepValidator::SpecUpdatesNonEmpty {
                sections: vec!["tests".to_string()],
            },
            StepValidator::WorkProductHasExtensions {
                exts: vec![".rs".to_string(), ".ts".to_string(), ".vue".to_string()],
            },
        ]),
    ]
}

/// Default validators for the review (reviewer) step.
fn review_validators() -> Vec<StepValidator> {
    vec![
        StepValidator::Any(vec![
            StepValidator::DecisionsNonEmpty,
            StepValidator::SpecUpdatesNonEmpty {
                sections: vec!["reviews".to_string()],
            },
            StepValidator::WorkProductHasExtensions {
                exts: vec![".md".to_string(), ".ad".to_string(), ".txt".to_string()],
            },
        ]),
    ]
}

/// Default validators for the report (documenter) step.
fn report_validators() -> Vec<StepValidator> {
    vec![
        StepValidator::Any(vec![
            StepValidator::SpecUpdatesNonEmpty {
                sections: vec!["reports".to_string()],
            },
            StepValidator::WorkProductHasExtensions {
                exts: vec![".md".to_string(), ".ad".to_string()],
            },
        ]),
    ]
}

/// The standard spec-driven development flow (v2).
///
/// Assistant → Advisor → Architect → Planner → Tester → Coder → Tester → Reviewer → Documenter
///
/// With human gate at Advisor→Architect boundary (GoalGate).
/// In GSD mode, only the Advisor gate pauses. In Check mode, all gates pause.
pub fn standard_spec_flow() -> FlowSpec {
    let mut flow = FlowSpec::new("standard-spec-driven-development");
    flow.add_step(FlowStep::new("intake", "assistant"));
    flow.add_step(
        FlowStep::new("discover", "advisor")
            .with_gate(GateType::Human)
            .with_validators(discover_validators())
            .with_tool_guard(discover_tool_guard()),
    );
    flow.add_step(
        FlowStep::new("design", "architect")
            .with_validators(design_validators())
            .with_tool_guard(design_tool_guard()),
    );
    flow.add_step(
        FlowStep::new("plan", "planner")
            .with_validators(plan_validators())
            .with_tool_guard(plan_tool_guard()),
    );
    flow.add_step(FlowStep::new("draft-tests", "tester"));
    flow.add_step(
        FlowStep::new("code", "coder")
            .with_validators(code_validators()),
    );
    flow.add_step(
        FlowStep::new("run-tests", "tester")
            .with_validators(test_validators())
            .with_exit(crate::relay::flow::ExitRouting::Branch {
                on: "to".into(),
                arms: {
                    let mut m = std::collections::HashMap::new();
                    m.insert("coder".into(), "code".into());
                    m.insert("reviewer".into(), "review".into());
                    m
                },
                default: "review".into(),
            }),
    );
    flow.add_step(
        FlowStep::new("review", "reviewer")
            .with_validators(review_validators())
            .with_tool_guard(ToolGuard {
                required_first: vec!["write_specs".to_string(), "update_spec".to_string(), "write_file".to_string(), "edit_file".to_string()],
                forbidden: vec!["dispatch".to_string()],
                always_allowed: vec!["read_file".to_string(), "read_specs".to_string(), "list_specs".to_string(), "search".to_string(), "shell".to_string()],
                ..ToolGuard::new()
            }),
    );
    flow.add_step(
        FlowStep::new("report", "documenter")
            .with_validators(report_validators()),
    );
    flow
}

/// A fast-track flow for small, well-understood tasks.
///
/// Assistant classifies as DIRECT → Coder only.
/// Falls back to full flow if classification is COMPLEX.
pub fn fast_track_flow() -> FlowSpec {
    let mut flow = FlowSpec::new("fast-track");
    flow.add_step(
        FlowStep::new("intake", "assistant"),
    );
    flow.add_step(
        FlowStep::new("code", "coder")
            .with_validators(code_validators()),
    );
    flow
}

/// Auto-discovery flow: user gives a raw goal, advisor auto-analyzes it,
/// then proceeds through the full pipeline without human gates.
///
/// Advisor → Architect → Planner → Tester → Coder → Tester → Reviewer → Documenter
pub fn auto_discovery_flow() -> FlowSpec {
    let mut flow = FlowSpec::new("auto-discovery");
    flow.add_step(
        FlowStep::new("discover", "advisor")
            .with_validators(discover_validators())
            .with_tool_guard(discover_tool_guard()),
    );
    flow.add_step(
        FlowStep::new("design", "architect")
            .with_validators(design_validators())
            .with_tool_guard(design_tool_guard()),
    );
    flow.add_step(
        FlowStep::new("plan", "planner")
            .with_validators(plan_validators())
            .with_tool_guard(plan_tool_guard()),
    );
    flow.add_step(FlowStep::new("draft-tests", "tester"));
    flow.add_step(
        FlowStep::new("code", "coder")
            .with_validators(code_validators()),
    );
    flow.add_step(
        FlowStep::new("run-tests", "tester")
            .with_validators(test_validators())
            .with_exit(crate::relay::flow::ExitRouting::Loop {
                target_step_id: "code".into(),
                max_iterations: 3,
            }),
    );
    flow.add_step(
        FlowStep::new("review", "reviewer")
            .with_validators(review_validators()),
    );
    flow.add_step(
        FlowStep::new("report", "documenter")
            .with_validators(report_validators()),
    );
    flow
}

/// Post-discovery flow: skips intake and advisor since chat already did discovery.
///
/// Architect → Planner → Tester → Coder → Tester → Reviewer → Documenter
pub fn post_discovery_flow() -> FlowSpec {
    let mut flow = FlowSpec::new("post-discovery");
    flow.add_step(
        FlowStep::new("design", "architect")
            .with_validators(design_validators())
            .with_tool_guard(design_tool_guard()),
    );
    flow.add_step(
        FlowStep::new("plan", "planner")
            .with_validators(plan_validators())
            .with_tool_guard(plan_tool_guard()),
    );
    flow.add_step(FlowStep::new("draft-tests", "tester"));
    flow.add_step(
        FlowStep::new("code", "coder")
            .with_validators(code_validators()),
    );
    flow.add_step(
        FlowStep::new("run-tests", "tester")
            .with_validators(test_validators())
            .with_exit(crate::relay::flow::ExitRouting::Loop {
                target_step_id: "code".into(),
                max_iterations: 3,
            }),
    );
    flow.add_step(
        FlowStep::new("review", "reviewer")
            .with_validators(review_validators()),
    );
    flow.add_step(
        FlowStep::new("report", "documenter")
            .with_validators(report_validators()),
    );
    flow
}

/// A bug-fix flow with tester-review loop.
///
/// Coder → Tester → Reviewer, with loop back to Coder if tests fail.
pub fn bug_fix_flow() -> FlowSpec {
    let mut flow = FlowSpec::new("bug-fix");
    flow.add_step(FlowStep::new("intake", "assistant"));
    flow.add_step(
        FlowStep::new("code", "coder")
            .with_validators(code_validators()),
    );
    flow.add_step(
        FlowStep::new("test", "tester")
            .with_validators(test_validators())
            .with_exit(crate::relay::flow::ExitRouting::Loop {
                target_step_id: "code".into(),
                max_iterations: 3,
            }),
    );
    flow.add_step(
        FlowStep::new("review", "reviewer")
            .with_validators(review_validators()),
    );
    flow
}

/// Goal-discovery flow: runs only the Advisor step to produce goals.
///
/// Useful for quickly testing whether the Advisor can successfully
/// analyze a task and write new goals before committing to a full pipeline.
pub fn goal_discovery_flow() -> FlowSpec {
    let mut flow = FlowSpec::new("goal-discovery");
    flow.add_step(
        FlowStep::new("discover", "advisor")
            .with_validators(discover_validators())
            .with_tool_guard(discover_tool_guard()),
    );
    flow
}

/// Doc-patch flow: for quick documentation or wiki updates without code changes.
///
/// Assistant → Documenter. The documenter updates reports, wiki pages,
/// or other documentation artifacts. No code is written or modified.
pub fn doc_patch_flow() -> FlowSpec {
    let mut flow = FlowSpec::new("doc-patch");
    flow.add_step(FlowStep::new("intake", "assistant"));
    flow.add_step(
        FlowStep::new("doc-update", "documenter")
            .with_validators(vec![StepValidator::Any(vec![
                StepValidator::SpecUpdatesNonEmpty {
                    sections: vec!["reports".to_string()],
                },
                StepValidator::WorkProductHasExtensions {
                    exts: vec![".md".to_string(), ".ad".to_string()],
                },
            ])]),
    );
    flow
}

/// Spec-tweak flow: for updating specs (goals, architecture, designs, plans, tests)
/// without executing any code.
///
/// Assistant → Advisor. The advisor reads existing specs and produces targeted
/// updates to any spec section. No compilation, testing, or code changes occur.
pub fn spec_tweak_flow() -> FlowSpec {
    let mut flow = FlowSpec::new("spec-tweak");
    flow.add_step(FlowStep::new("intake", "assistant"));
    flow.add_step(
        FlowStep::new("tweak", "advisor")
            .with_validators(vec![StepValidator::SpecUpdatesNonEmpty {
                sections: vec![
                    "goals".to_string(),
                    "architecture".to_string(),
                    "designs".to_string(),
                    "plans".to_string(),
                    "tests".to_string(),
                ],
            }])
            .with_tool_guard(ToolGuard {
                required_first: vec!["read_specs".to_string()],
                unlocks: HashMap::new(),
                always_allowed: vec![
                    "list_specs".to_string(),
                    "read_specs".to_string(),
                    "write_specs".to_string(),
                    "write_goals".to_string(),
                    "read_file".to_string(),
                    "query_wiki".to_string(),
                    "list_wiki".to_string(),
                ],
                forbidden: vec![],
            }),
    );
    flow
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::flow::GateType;

    #[test]
    fn test_standard_flow_has_nine_steps() {
        let flow = standard_spec_flow();
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
    fn test_standard_flow_has_human_gate_at_advisor() {
        let flow = standard_spec_flow();
        assert_eq!(flow.steps[1].gate, GateType::Human); // advisor → architect (GoalGate)
        assert_eq!(flow.steps[2].gate, GateType::Auto);  // architect → planner
        assert_eq!(flow.steps[3].gate, GateType::Auto);  // planner → tester
        assert_eq!(flow.steps[4].gate, GateType::Auto);  // tester → coder
    }

    #[test]
    fn test_advisor_step_has_tool_guard() {
        let flow = standard_spec_flow();
        let advisor_step = flow.get_step("discover").unwrap();
        assert!(advisor_step.tool_guard.is_some());
        let guard = advisor_step.tool_guard.as_ref().unwrap();
        assert_eq!(guard.required_first, vec!["write_specs", "write_goals"]);
        assert!(guard.always_allowed.contains(&"list_specs".to_string()));
    }

    #[test]
    fn test_fast_track_flow_has_two_steps() {
        let flow = fast_track_flow();
        assert_eq!(flow.steps.len(), 2);
        assert_eq!(flow.steps[0].profession_id, "assistant");
        assert_eq!(flow.steps[1].profession_id, "coder");
    }

    #[test]
    fn test_post_discovery_flow_has_seven_steps() {
        let flow = post_discovery_flow();
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
        let flow = bug_fix_flow();
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
        let flow = doc_patch_flow();
        assert_eq!(flow.steps.len(), 2);
        assert_eq!(flow.steps[0].profession_id, "assistant");
        assert_eq!(flow.steps[1].profession_id, "documenter");
    }

    #[test]
    fn test_spec_tweak_flow_has_two_steps() {
        let flow = spec_tweak_flow();
        assert_eq!(flow.steps.len(), 2);
        assert_eq!(flow.steps[0].profession_id, "assistant");
        assert_eq!(flow.steps[1].profession_id, "advisor");
    }

    #[test]
    fn test_spec_tweak_requires_read_specs_first() {
        let flow = spec_tweak_flow();
        let tweak_step = flow.get_step("tweak").unwrap();
        assert!(tweak_step.tool_guard.is_some());
        let guard = tweak_step.tool_guard.as_ref().unwrap();
        assert_eq!(guard.required_first, vec!["read_specs"]);
        assert!(guard.always_allowed.contains(&"write_specs".to_string()));
    }
}
