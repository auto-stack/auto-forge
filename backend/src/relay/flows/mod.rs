//! Built-in Flow Specifications
//!
//! Pre-defined flow specs for common development workflows.
//! Also supports loading custom flows from `.autoforge/flows/*.yml`.

use crate::relay::flow::{FlowSpec, ValidationIssue, ValidationSeverity, ExitRouting};
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

// ─── Flow Validation ─────────────────────────────────────────────────────────

/// Validate a flow spec against the available professions and tools.
pub fn validate_flow(
    flow: &FlowSpec,
    professions: &crate::relay::profession::ProfessionRegistry,
    tools: &crate::forge::tools::ToolRegistry,
) -> Vec<ValidationIssue> {
    use std::collections::{HashSet, VecDeque};
    let mut issues = Vec::new();
    let step_ids: HashSet<&str> = flow.steps.iter().map(|s| s.id.as_str()).collect();

    // Rule 1: step.id uniqueness
    let mut seen = HashSet::new();
    for step in &flow.steps {
        if !seen.insert(&step.id) {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                message: format!("Duplicate step id '{}'", step.id),
                step_id: Some(step.id.clone()),
            });
        }
    }

    // Rule 2: profession_id exists
    for step in &flow.steps {
        if professions.get(&step.profession_id).is_none() {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                message: format!("Unknown profession_id '{}'", step.profession_id),
                step_id: Some(step.id.clone()),
            });
        }
    }

    // Rules 3 & 4: routing targets exist
    for step in &flow.steps {
        match &step.exit {
            ExitRouting::Next => {}
            ExitRouting::Branch { arms, default, .. } => {
                for (value, target) in arms {
                    if !step_ids.contains(target.as_str()) {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Error,
                            message: format!(
                                "Branch arm '{}' targets unknown step '{}'",
                                value, target
                            ),
                            step_id: Some(step.id.clone()),
                        });
                    }
                }
                if !step_ids.contains(default.as_str()) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        message: format!(
                            "Branch default targets unknown step '{}'",
                            default
                        ),
                        step_id: Some(step.id.clone()),
                    });
                }
            }
            ExitRouting::Loop { target_step_id, .. } => {
                if !step_ids.contains(target_step_id.as_str()) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        message: format!(
                            "Loop target '{}' does not exist",
                            target_step_id
                        ),
                        step_id: Some(step.id.clone()),
                    });
                }
            }
            ExitRouting::Condition { true_branch, false_branch, .. } => {
                // Validate both branches recursively
                for branch in [true_branch.as_ref(), false_branch.as_ref()] {
                    match branch {
                        ExitRouting::Next => {}
                        ExitRouting::Branch { arms, default, .. } => {
                            for (value, target) in arms {
                                if !step_ids.contains(target.as_str()) {
                                    issues.push(ValidationIssue {
                                        severity: ValidationSeverity::Error,
                                        message: format!(
                                            "Condition branch arm '{}' targets unknown step '{}'",
                                            value, target
                                        ),
                                        step_id: Some(step.id.clone()),
                                    });
                                }
                            }
                            if !step_ids.contains(default.as_str()) {
                                issues.push(ValidationIssue {
                                    severity: ValidationSeverity::Error,
                                    message: format!(
                                        "Condition branch default targets unknown step '{}'",
                                        default
                                    ),
                                    step_id: Some(step.id.clone()),
                                });
                            }
                        }
                        ExitRouting::Loop { target_step_id, .. } => {
                            if !step_ids.contains(target_step_id.as_str()) {
                                issues.push(ValidationIssue {
                                    severity: ValidationSeverity::Error,
                                    message: format!(
                                        "Condition branch loop target '{}' does not exist",
                                        target_step_id
                                    ),
                                    step_id: Some(step.id.clone()),
                                });
                            }
                        }
                        ExitRouting::Condition { .. } => {
                            issues.push(ValidationIssue {
                                severity: ValidationSeverity::Warning,
                                message: "Nested conditions are allowed but can be hard to reason about".to_string(),
                                step_id: Some(step.id.clone()),
                            });
                        }
                    }
                }
            }
        }
    }

    // Rule 5: unreachable steps (BFS)
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();
    if let Some(first) = flow.steps.first() {
        queue.push_back(0usize);
        reachable.insert(first.id.as_str());
    }
    while let Some(idx) = queue.pop_front() {
        let step = &flow.steps[idx];
        let nexts: Vec<usize> = match &step.exit {
            ExitRouting::Next => {
                if idx + 1 < flow.steps.len() {
                    vec![idx + 1]
                } else {
                    vec![]
                }
            }
            ExitRouting::Branch { arms, default, .. } => {
                let mut targets = Vec::new();
                for tid in arms.values() {
                    if let Some(i) = flow.get_step_index(tid) {
                        targets.push(i);
                    }
                }
                if let Some(i) = flow.get_step_index(default) {
                    targets.push(i);
                }
                targets
            }
            ExitRouting::Loop { target_step_id, .. } => {
                flow.get_step_index(target_step_id).into_iter().collect()
            }
            ExitRouting::Condition { true_branch, false_branch, .. } => {
                let mut targets = Vec::new();
                for branch in [true_branch.as_ref(), false_branch.as_ref()] {
                    match branch {
                        ExitRouting::Next => {
                            if idx + 1 < flow.steps.len() {
                                targets.push(idx + 1);
                            }
                        }
                        ExitRouting::Branch { arms, default, .. } => {
                            for tid in arms.values() {
                                if let Some(i) = flow.get_step_index(tid) {
                                    targets.push(i);
                                }
                            }
                            if let Some(i) = flow.get_step_index(default) {
                                targets.push(i);
                            }
                        }
                        ExitRouting::Loop { target_step_id, .. } => {
                            if let Some(i) = flow.get_step_index(target_step_id) {
                                targets.push(i);
                            }
                        }
                        ExitRouting::Condition { true_branch: tb, false_branch: fb, .. } => {
                            // Flatten one level of nested condition for BFS
                            for b in [tb.as_ref(), fb.as_ref()] {
                                match b {
                                    ExitRouting::Next => {
                                        if idx + 1 < flow.steps.len() {
                                            targets.push(idx + 1);
                                        }
                                    }
                                    ExitRouting::Branch { arms, default, .. } => {
                                        for tid in arms.values() {
                                            if let Some(i) = flow.get_step_index(tid) {
                                                targets.push(i);
                                            }
                                        }
                                        if let Some(i) = flow.get_step_index(default) {
                                            targets.push(i);
                                        }
                                    }
                                    ExitRouting::Loop { target_step_id, .. } => {
                                        if let Some(i) = flow.get_step_index(target_step_id) {
                                            targets.push(i);
                                        }
                                    }
                                    ExitRouting::Condition { .. } => {
                                        // Deeper nesting ignored for BFS — validator already warns
                                    }
                                }
                            }
                        }
                    }
                }
                targets
            }
        };
        for nidx in nexts {
            if reachable.insert(flow.steps[nidx].id.as_str()) {
                queue.push_back(nidx);
            }
        }
    }
    for step in &flow.steps {
        if !reachable.contains(step.id.as_str()) {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                message: format!("Step '{}' is unreachable from the start", step.id),
                step_id: Some(step.id.clone()),
            });
        }
    }

    // Rule 6: infinite loop without cap
    for step in &flow.steps {
        if let ExitRouting::Loop { max_iterations, .. } = &step.exit {
            if *max_iterations == 0 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    message: format!(
                        "Loop on step '{}' has max_iterations=0 (infinite)",
                        step.id
                    ),
                    step_id: Some(step.id.clone()),
                });
            }
        }
    }

    // Rule 7: tool_guard references only known tools
    for step in &flow.steps {
        if let Some(guard) = &step.tool_guard {
            let mut referenced = HashSet::new();
            for t in &guard.required_first {
                referenced.insert(t.as_str());
            }
            for t in &guard.always_allowed {
                referenced.insert(t.as_str());
            }
            for t in &guard.forbidden {
                referenced.insert(t.as_str());
            }
            for list in guard.unlocks.values() {
                for t in list {
                    referenced.insert(t.as_str());
                }
            }
            for t in referenced {
                if tools.get(t).is_none() {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        message: format!(
                            "Tool '{}' referenced in tool_guard does not exist",
                            t
                        ),
                        step_id: Some(step.id.clone()),
                    });
                }
            }
        }
    }

    issues
}

// ─── Flow Registry ───────────────────────────────────────────────────────────

/// Global registry of all available flows (built-in + YAML-loaded).
pub struct FlowRegistry {
    flows: HashMap<String, FlowSpec>,
}

impl FlowRegistry {
    /// Create a new registry and load all flows.
    pub fn new(data_dir: &std::path::Path) -> Self {
        let professions = crate::relay::profession::ProfessionRegistry::new();
        let tools = crate::forge::tools::ToolRegistry::new();
        let mut registry = Self {
            flows: HashMap::new(),
        };
        registry.load_builtin(&professions, &tools);
        registry.load_from_yaml(data_dir, &professions, &tools);
        registry
    }

    /// Load only built-in flows (useful for tests).
    pub fn load_builtins_only() -> Self {
        let professions = crate::relay::profession::ProfessionRegistry::new();
        let tools = crate::forge::tools::ToolRegistry::new();
        let mut registry = Self {
            flows: HashMap::new(),
        };
        registry.load_builtin(&professions, &tools);
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

    /// Insert or overwrite a flow in the registry.
    pub fn insert(&mut self, flow: FlowSpec) {
        self.flows.insert(flow.id.clone(), flow);
    }

    /// Remove a flow from the registry. Returns the removed flow if any.
    pub fn remove(&mut self, flow_id: &str) -> Option<FlowSpec> {
        self.flows.remove(flow_id)
    }

    /// Check whether a flow ID corresponds to a built-in flow.
    pub fn is_builtin(&self, flow_id: &str) -> bool {
        BUILTIN_FLOWS.iter().any(|(id, _)| *id == flow_id)
    }

    fn load_builtin(
        &mut self,
        professions: &crate::relay::profession::ProfessionRegistry,
        tools: &crate::forge::tools::ToolRegistry,
    ) {
        for (_id, yaml) in BUILTIN_FLOWS {
            match serde_yaml::from_str::<FlowSpec>(yaml) {
                Ok(flow) => {
                    let issues = validate_flow(&flow, professions, tools);
                    let has_errors = issues
                        .iter()
                        .any(|i| matches!(i.severity, ValidationSeverity::Error));
                    for issue in &issues {
                        let level = match issue.severity {
                            ValidationSeverity::Error => "error",
                            ValidationSeverity::Warning => "warning",
                        };
                        tracing::error!(
                            "Built-in flow '{}' validation {}: {}",
                            flow.id, level, issue.message
                        );
                    }
                    if has_errors {
                        panic!(
                            "Built-in flow '{}' has validation errors — fix the YAML",
                            flow.id
                        );
                    }
                    self.flows.insert(flow.id.clone(), flow);
                }
                Err(e) => {
                    tracing::error!("Failed to parse built-in flow YAML: {}", e);
                }
            }
        }
    }

    fn load_from_yaml(
        &mut self,
        data_dir: &std::path::Path,
        professions: &crate::relay::profession::ProfessionRegistry,
        tools: &crate::forge::tools::ToolRegistry,
    ) {
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
                            let issues = validate_flow(&flow, professions, tools);
                            let has_errors = issues.iter().any(|i| {
                                matches!(i.severity, ValidationSeverity::Error)
                            });
                            for issue in &issues {
                                let level = match issue.severity {
                                    ValidationSeverity::Error => "error",
                                    ValidationSeverity::Warning => "warning",
                                };
                                if has_errors {
                                    tracing::error!(
                                        "User flow '{}' validation {}: {}",
                                        flow.id, level, issue.message
                                    );
                                } else {
                                    tracing::warn!(
                                        "User flow '{}' validation {}: {}",
                                        flow.id, level, issue.message
                                    );
                                }
                            }
                            if has_errors {
                                tracing::error!(
                                    "Skipping user flow '{}' due to validation errors",
                                    flow.id
                                );
                            } else {
                                tracing::info!("Loaded flow '{}' from {:?}", flow.id, path);
                                self.flows.insert(flow.id.clone(), flow);
                            }
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
pub(crate) static FLOW_REGISTRY: Mutex<Option<FlowRegistry>> = Mutex::new(None);

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

    // ─── Validation Tests ──────────────────────────────────────────────────────

    use crate::relay::flow::{FlowStep, ExitRouting, ValidationSeverity};
    use crate::relay::profession::ProfessionRegistry;
    use crate::forge::tools::ToolRegistry;

    fn test_professions() -> ProfessionRegistry {
        ProfessionRegistry::new()
    }

    fn test_tools() -> ToolRegistry {
        ToolRegistry::new()
    }

    #[test]
    fn test_validate_duplicate_step_id() {
        let mut flow = FlowSpec::new("test-dup");
        flow.add_step(FlowStep::new("s1", "assistant"));
        flow.add_step(FlowStep::new("s1", "coder"));
        let issues = validate_flow(&flow, &test_professions(), &test_tools());
        let dup_errors: Vec<_> = issues.iter().filter(|i| i.message.contains("Duplicate")).collect();
        assert_eq!(dup_errors.len(), 1);
        assert_eq!(dup_errors[0].severity, ValidationSeverity::Error);
    }

    #[test]
    fn test_validate_unknown_profession() {
        let mut flow = FlowSpec::new("test-prof");
        flow.add_step(FlowStep::new("s1", "nonexistent-profession"));
        let issues = validate_flow(&flow, &test_professions(), &test_tools());
        let prof_errors: Vec<_> = issues.iter().filter(|i| i.message.contains("Unknown profession_id")).collect();
        assert_eq!(prof_errors.len(), 1);
        assert_eq!(prof_errors[0].severity, ValidationSeverity::Error);
    }

    #[test]
    fn test_validate_unknown_branch_target() {
        let mut flow = FlowSpec::new("test-branch");
        flow.add_step(FlowStep::new("s1", "assistant"));
        flow.add_step(
            FlowStep::new("s2", "coder")
                .with_exit(ExitRouting::Branch {
                    on: "to".into(),
                    arms: {
                        let mut m = std::collections::HashMap::new();
                        m.insert("reviewer".into(), "nonexistent".into());
                        m
                    },
                    default: "s1".into(),
                }),
        );
        let issues = validate_flow(&flow, &test_professions(), &test_tools());
        let branch_errors: Vec<_> = issues.iter().filter(|i| i.message.contains("Branch arm")).collect();
        assert_eq!(branch_errors.len(), 1);
        assert_eq!(branch_errors[0].severity, ValidationSeverity::Error);
    }

    #[test]
    fn test_validate_unknown_loop_target() {
        let mut flow = FlowSpec::new("test-loop");
        flow.add_step(FlowStep::new("s1", "assistant"));
        flow.add_step(
            FlowStep::new("s2", "coder")
                .with_exit(ExitRouting::Loop {
                    target_step_id: "ghost".into(),
                    max_iterations: 3,
                }),
        );
        let issues = validate_flow(&flow, &test_professions(), &test_tools());
        let loop_errors: Vec<_> = issues.iter().filter(|i| i.message.contains("Loop target")).collect();
        assert_eq!(loop_errors.len(), 1);
        assert_eq!(loop_errors[0].severity, ValidationSeverity::Error);
    }

    #[test]
    fn test_validate_unreachable_step() {
        let mut flow = FlowSpec::new("test-unreachable");
        flow.add_step(FlowStep::new("s1", "assistant"));
        flow.add_step(FlowStep::new("s2", "coder"));
        flow.add_step(FlowStep::new("s3", "tester"));
        // s2 branches back to s1, so s3 is never reached
        flow.steps[1].exit = ExitRouting::Branch {
            on: "to".into(),
            arms: {
                let mut m = std::collections::HashMap::new();
                m.insert("back".into(), "s1".into());
                m
            },
            default: "s1".into(),
        };
        let issues = validate_flow(&flow, &test_professions(), &test_tools());
        let warn: Vec<_> = issues.iter().filter(|i| i.message.contains("unreachable")).collect();
        assert_eq!(warn.len(), 1);
        assert_eq!(warn[0].severity, ValidationSeverity::Warning);
        assert_eq!(warn[0].step_id.as_ref().unwrap(), "s3");
    }

    #[test]
    fn test_validate_unknown_tool_in_guard() {
        let mut flow = FlowSpec::new("test-tool");
        let mut step = FlowStep::new("s1", "assistant");
        step.tool_guard = Some(crate::relay::flow::ToolGuard {
            required_first: vec!["nonexistent_tool".into()],
            unlocks: std::collections::HashMap::new(),
            always_allowed: vec![],
            forbidden: vec![],
        });
        flow.add_step(step);
        let issues = validate_flow(&flow, &test_professions(), &test_tools());
        let tool_errors: Vec<_> = issues.iter().filter(|i| i.message.contains("tool_guard")).collect();
        assert_eq!(tool_errors.len(), 1);
        assert_eq!(tool_errors[0].severity, ValidationSeverity::Error);
    }

    #[test]
    fn test_validate_builtin_flows_have_no_errors() {
        let professions = test_professions();
        let tools = test_tools();
        for (_, yaml) in super::BUILTIN_FLOWS {
            let flow: FlowSpec = serde_yaml::from_str(yaml).expect("built-in YAML must parse");
            let issues = validate_flow(&flow, &professions, &tools);
            let errors: Vec<_> = issues.iter().filter(|i| matches!(i.severity, ValidationSeverity::Error)).collect();
            assert!(errors.is_empty(), "Built-in flow '{}' has validation errors: {:?}", flow.id, errors);
        }
    }
}
