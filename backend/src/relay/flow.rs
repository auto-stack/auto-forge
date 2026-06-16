//! Flow Specification
//!
//! Declarative flow definitions that the PipelineEngine executes.
//! Flows are deterministic — the orchestrator is pure code, not an LLM.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Validation Types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub message: String,
    pub step_id: Option<String>,
}

/// A flow is an ordered list of steps with routing logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSpec {
    pub id: String,
    pub steps: Vec<FlowStep>,
}

impl FlowSpec {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            steps: Vec::new(),
        }
    }

    pub fn add_step(&mut self, step: FlowStep) -> &mut Self {
        self.steps.push(step);
        self
    }

    pub fn get_step(&self, step_id: &str) -> Option<&FlowStep> {
        self.steps.iter().find(|s| s.id == step_id)
    }

    pub fn get_step_index(&self, step_id: &str) -> Option<usize> {
        self.steps.iter().position(|s| s.id == step_id)
    }

    /// Resolve a profession_id to the first step that uses it.
    pub fn step_for_profession(&self, profession_id: &str) -> Option<usize> {
        self.steps.iter().position(|s| s.profession_id == profession_id)
    }
}

/// A single step in a flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowStep {
    pub id: String,
    pub profession_id: String,
    /// Optional agent config to use instead of the default for this profession.
    pub agent_config_id: Option<String>,
    pub gate: GateType,
    /// Max LLM turns before forced handoff (overrides profession default).
    pub max_turns: Option<u32>,
    /// How to route after this step completes.
    pub exit: ExitRouting,
    /// Validators that check the step's handoff before proceeding.
    #[serde(default)]
    pub validators: Vec<StepValidator>,
    /// Tool guard enforcing step-level tool call sequencing.
    #[serde(default)]
    pub tool_guard: Option<ToolGuard>,
}

impl FlowStep {
    pub fn new(id: impl Into<String>, profession_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            profession_id: profession_id.into(),
            agent_config_id: None,
            gate: GateType::Auto,
            max_turns: None,
            exit: ExitRouting::Next,
            validators: Vec::new(),
            tool_guard: None,
        }
    }

    pub fn with_gate(mut self, gate: GateType) -> Self {
        self.gate = gate;
        self
    }

    pub fn with_exit(mut self, exit: ExitRouting) -> Self {
        self.exit = exit;
        self
    }

    pub fn with_agent_config(mut self, config_id: Option<String>) -> Self {
        self.agent_config_id = config_id;
        self
    }

    pub fn with_validators(mut self, validators: Vec<StepValidator>) -> Self {
        self.validators = validators;
        self
    }

    pub fn with_tool_guard(mut self, guard: ToolGuard) -> Self {
        self.tool_guard = Some(guard);
        self
    }
}

/// Gate type controlling whether a step needs human approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateType {
    /// Proceed automatically.
    Auto,
    /// Pause for human approval before executing.
    Human,
}

/// Routing logic after a step completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitRouting {
    /// Go to the next step in sequence.
    Next,
    /// Branch based on a field in the handoff.
    Branch {
        /// Name of the handoff field to branch on (e.g., "intent").
        on: String,
        /// Map of field value → target step_id.
        arms: HashMap<String, String>,
        /// Fallback step_id if no arm matches.
        default: String,
    },
    /// Loop back to a target step.
    Loop {
        /// Step to return to.
        target_step_id: String,
        /// Max iterations before breaking to next.
        max_iterations: u32,
    },
    /// Conditional routing based on runtime state.
    Condition {
        condition: RoutingCondition,
        true_branch: Box<ExitRouting>,
        false_branch: Box<ExitRouting>,
    },
}

/// Runtime condition for conditional routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "content")]
pub enum RoutingCondition {
    /// True if any validator failed for the current step.
    ValidatorFailed,
    /// True if the handoff's field matches a value.
    HandoffFieldEquals { field: String, value: String },
    /// True if cumulative token usage exceeds threshold.
    TokenUsageCumulativeExceeds { limit: u64 },
    /// True if step token usage exceeds threshold.
    TokenUsageStepExceeds { limit: u64 },
    /// True if a work_product file matching pattern was produced.
    WorkProductExists { glob: String },
    /// Logical AND of multiple conditions.
    All(Vec<RoutingCondition>),
    /// Logical OR of multiple conditions.
    Any(Vec<RoutingCondition>),
    /// Logical NOT of a condition.
    Not(Box<RoutingCondition>),
}

// ─── Step Validators ─────────────────────────────────────────────────────────

/// Content-aware validators for step handoffs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "content")]
pub enum StepValidator {
    /// Must have non-empty spec_updates containing at least one of these sections.
    SpecUpdatesNonEmpty { sections: Vec<String> },
    /// Must have non-empty work_product with at least one file matching these extensions.
    WorkProductHasExtensions { exts: Vec<String> },
    /// Must have non-empty decisions.
    DecisionsNonEmpty,
    /// Must not have any decision containing the given pattern (case-insensitive).
    DecisionsNotContain { pattern: String },
    /// Must have non-empty open_questions.
    OpenQuestionsNonEmpty,
    /// Custom check: spec_updates contain items with IDs following a sequential pattern.
    SequentialIds { section: String, prefix: String },
    /// Composite: all of the above must pass.
    All(Vec<StepValidator>),
    /// Composite: any of the above must pass.
    Any(Vec<StepValidator>),
}

impl StepValidator {
    /// Check this validator against a handoff. Returns `Some(reason)` on failure.
    pub fn check(&self, handoff: &crate::relay::handoff::HandoffDocument) -> Option<String> {
        use crate::relay::handoff::HandoffDocument;
        match self {
            StepValidator::SpecUpdatesNonEmpty { sections } => {
                let has = handoff.spec_updates.iter().any(|u| sections.contains(&u.section_id));
                if !has {
                    return Some(format!(
                        "Step must produce spec updates for at least one of: {}. Use write_specs or update_spec to create or update specs.",
                        sections.join(", ")
                    ));
                }
                None
            }
            StepValidator::WorkProductHasExtensions { exts } => {
                let has = handoff.work_product.iter().any(|wp| {
                    exts.iter().any(|ext| wp.path.ends_with(ext))
                });
                if !has {
                    return Some(format!(
                        "Step must produce work products with one of these extensions: {}. Use write_file or edit_file to modify source files.",
                        exts.join(", ")
                    ));
                }
                None
            }
            StepValidator::DecisionsNonEmpty => {
                if handoff.decisions.is_empty() {
                    return Some("Step must produce at least one decision. Document your design choices in the handoff.".into());
                }
                None
            }
            StepValidator::DecisionsNotContain { pattern } => {
                let lower = pattern.to_lowercase();
                for d in &handoff.decisions {
                    if d.title.to_lowercase().contains(&lower) {
                        return Some(format!(
                            "Review marked the implementation as '{}'. The previous step must be re-run to fix the issues.",
                            pattern
                        ));
                    }
                }
                None
            }
            StepValidator::OpenQuestionsNonEmpty => {
                if handoff.open_questions.is_empty() {
                    return Some("Step must list at least one open question for the next agent.".into());
                }
                None
            }
            StepValidator::SequentialIds { section, prefix } => {
                let updates: Vec<_> = handoff.spec_updates.iter()
                    .filter(|u| &u.section_id == section)
                    .collect();
                if updates.is_empty() {
                    return None; // no updates for this section, let SpecUpdatesNonEmpty catch it
                }
                // TODO: implement sequential ID check when we have access to SpecsStore
                None
            }
            StepValidator::All(validators) => {
                for v in validators {
                    if let Some(reason) = v.check(handoff) {
                        return Some(reason);
                    }
                }
                None
            }
            StepValidator::Any(validators) => {
                let mut reasons = Vec::new();
                for v in validators {
                    match v.check(handoff) {
                        None => return None,
                        Some(r) => reasons.push(r),
                    }
                }
                Some(format!("None of the alternative checks passed: {}", reasons.join("; ")))
            }
        }
    }
}

// ─── Tool Guard ──────────────────────────────────────────────────────────────

/// Enforces step-level tool call sequencing rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolGuard {
    /// Tools that MUST be called before any other tool (except themselves).
    #[serde(default)]
    pub required_first: Vec<String>,
    /// After calling key tool, these tools become available.
    #[serde(default)]
    pub unlocks: HashMap<String, Vec<String>>,
    /// Tools always allowed (bypass required_first after satisfied).
    #[serde(default)]
    pub always_allowed: Vec<String>,
    /// Tools that are NEVER allowed.
    #[serde(default)]
    pub forbidden: Vec<String>,
}

impl ToolGuard {
    pub fn new() -> Self {
        Self {
            required_first: Vec::new(),
            unlocks: HashMap::new(),
            always_allowed: Vec::new(),
            forbidden: Vec::new(),
        }
    }

    /// Check if a tool call is permitted given previously called tools.
    pub fn check(&self, tool_name: &str, tools_called: &[String]) -> Result<(), String> {
        // Check forbidden
        if self.forbidden.contains(&tool_name.to_string()) {
            return Err(format!("Tool '{}' is forbidden in this step.", tool_name));
        }

        // Check required_first
        if !self.required_first.is_empty() {
            let has_called_required = tools_called.iter().any(|t| self.required_first.contains(t));
            let is_required = self.required_first.contains(&tool_name.to_string());
            let is_always = self.always_allowed.contains(&tool_name.to_string());

            if !has_called_required && !is_required && !is_always {
                return Err(format!(
                    "This step requires calling one of [{}] before using '{}'.",
                    self.required_first.join(", "),
                    tool_name
                ));
            }
        }

        // Check unlocks
        if let Some(required_predecessors) = self.unlocks.get(tool_name) {
            let has_unlocked = required_predecessors.iter().any(|t| tools_called.contains(t));
            if !has_unlocked {
                return Err(format!(
                    "Tool '{}' is locked. Call one of [{}] first.",
                    tool_name,
                    required_predecessors.join(", ")
                ));
            }
        }

        Ok(())
    }
}

impl Default for ToolGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Action type for a tool call, used to distinguish read vs write operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAction {
    Read,
    Write,
    Edit,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_spec_builder() {
        let mut flow = FlowSpec::new("test-flow");
        flow.add_step(FlowStep::new("step-1", "planner"));
        flow.add_step(FlowStep::new("step-2", "architect"));

        assert_eq!(flow.steps.len(), 2);
        assert_eq!(flow.get_step("step-1").unwrap().profession_id, "planner");
        assert_eq!(flow.get_step_index("step-2"), Some(1));
    }

    #[test]
    fn test_branch_routing() {
        let mut arms = HashMap::new();
        arms.insert("DIRECT".to_string(), "coder-step".to_string());
        arms.insert("COMPLEX".to_string(), "planner-step".to_string());

        let exit = ExitRouting::Branch {
            on: "intent".to_string(),
            arms,
            default: "planner-step".to_string(),
        };

        match exit {
            ExitRouting::Branch { on, arms, default } => {
                assert_eq!(on, "intent");
                assert_eq!(arms.get("DIRECT"), Some(&"coder-step".to_string()));
                assert_eq!(default, "planner-step");
            }
            _ => panic!("Expected Branch"),
        }
    }

    #[test]
    fn test_loop_routing() {
        let exit = ExitRouting::Loop {
            target_step_id: "step-1".to_string(),
            max_iterations: 3,
        };

        match exit {
            ExitRouting::Loop { target_step_id, max_iterations } => {
                assert_eq!(target_step_id, "step-1");
                assert_eq!(max_iterations, 3);
            }
            _ => panic!("Expected Loop"),
        }
    }
}
