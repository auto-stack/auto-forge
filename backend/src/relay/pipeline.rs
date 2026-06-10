//! Pipeline Engine
//!
//! The deterministic state machine that executes Flow specs.
//! Pure Rust code — zero LLM tokens spent on orchestration.

use crate::relay::budget::{BudgetTracker, TokenBudget};
use crate::relay::flow::{ExitRouting, FlowSpec, GateType};
use crate::relay::handoff::HandoffDocument;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Execution mode controlling human gate behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayMode {
    /// Get Shit Done — autonomous execution. Only GoalGate requires human approval.
    GSD,
    /// Human reviews every configured gate.
    Check,
}

impl Default for RelayMode {
    fn default() -> Self {
        RelayMode::GSD
    }
}

/// Result of advancing the pipeline — tells the caller what to do next.
#[derive(Debug, Clone, PartialEq)]
pub enum AdvanceResult {
    /// Execute the given step by running its agent.
    ExecuteStep {
        step_id: String,
        profession_id: String,
        /// If set, use this agent config instead of the default for the profession.
        agent_config_id: Option<String>,
    },
    /// Pause for human approval at a gate.
    WaitForHuman {
        gate: GateType,
        step_id: String,
    },
    /// Flow completed successfully.
    Completed,
    /// Flow failed with an error.
    Failed {
        error: String,
    },
}

/// Decision from a human at a gate.
#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    /// Approve and continue.
    Approve,
    /// Reject and redraft — routes back to the same step.
    Reject {
        feedback: String,
    },
    /// Approve with edits — continues but includes edit notes in context.
    Edit {
        changes: String,
    },
}

/// Record of a completed step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRecord {
    pub step_id: String,
    pub profession_id: String,
    pub handoff: Option<HandoffDocument>,
    pub started_at: u64,
    pub completed_at: u64,
    pub iteration: u32,
}

/// The pipeline engine state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineEngine {
    pub flow: FlowSpec,
    /// Index into flow.steps of the current (or next) step.
    pub current_step: usize,
    pub status: PipelineStatus,
    pub run_id: String,
    /// History of completed steps.
    pub step_history: Vec<StepRecord>,
    /// Loop iteration counters per step_id.
    pub loop_counters: HashMap<String, u32>,
    /// Pending human gate (if status is WaitingForHuman).
    pub pending_gate: Option<PendingGate>,
    /// Feedback from rejected gates, keyed by step_id.
    pub gate_feedback: HashMap<String, Vec<String>>,
    /// Tracks which step had its gate resolved for the current attempt.
    pub gate_resolved_for_step: Option<String>,
    /// Accumulated token usage across all steps.
    pub cumulative_tokens: u64,
    /// Budget tracker for runaway cost prevention and analytics.
    pub budget_tracker: BudgetTracker,
    /// Execution mode: GSD (autonomous, default) or Check (human reviews all gates).
    pub mode: RelayMode,
}

/// Current state of the pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PipelineStatus {
    /// Flow loaded, ready to start.
    Idle,
    /// A step is currently executing.
    Running {
        step_id: String,
        profession_id: String,
        started_at: u64,
    },
    /// Paused waiting for human approval.
    WaitingForHuman {
        gate: GateType,
        step_id: String,
        since: u64,
    },
    /// All steps completed.
    Completed,
    /// Unrecoverable failure.
    Failed {
        error: String,
    },
    /// Explicitly paused (not via gate).
    Paused {
        at_step: usize,
    },
}

impl PipelineStatus {
    /// Return a clean, human-readable status string.
    pub fn to_status_str(&self) -> String {
        match self {
            PipelineStatus::Idle => "idle".to_string(),
            PipelineStatus::Running { .. } => "running".to_string(),
            PipelineStatus::WaitingForHuman { .. } => "waiting_approval".to_string(),
            PipelineStatus::Completed => "completed".to_string(),
            PipelineStatus::Failed { error } => format!("failed"),
            PipelineStatus::Paused { .. } => "paused".to_string(),
        }
    }
}

/// Information about a gate that is awaiting human resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingGate {
    pub step_id: String,
    pub gate: GateType,
    pub since: u64,
}

impl PipelineEngine {
    /// Create a new pipeline from a flow spec.
    pub fn new(flow: FlowSpec, run_id: impl Into<String>) -> Self {
        Self::with_budget(flow, run_id, TokenBudget::new(10_000_000))
    }

    /// Create a new pipeline with a custom run budget.
    pub fn with_budget(flow: FlowSpec, run_id: impl Into<String>, run_budget: TokenBudget) -> Self {
        Self {
            flow,
            current_step: 0,
            status: PipelineStatus::Idle,
            run_id: run_id.into(),
            step_history: Vec::new(),
            loop_counters: HashMap::new(),
            pending_gate: None,
            gate_feedback: HashMap::new(),
            gate_resolved_for_step: None,
            cumulative_tokens: 0,
            budget_tracker: BudgetTracker::new(run_budget),
            mode: RelayMode::GSD,
        }
    }

    /// Advance the pipeline by one logical action.
    ///
    /// Returns what the caller should do next:
    /// - `ExecuteStep` → run the agent for this step, then call `submit_handoff()`
    /// - `WaitForHuman` → pause and wait for `resolve_gate()`
    /// - `Completed` or `Failed` → terminal states
    pub fn advance(&mut self) -> AdvanceResult {
        tracing::info!("pipeline_advance: current_step={}, status={:?}", self.current_step, self.status);
        match &self.status {
            PipelineStatus::Completed => return AdvanceResult::Completed,
            PipelineStatus::Failed { error } => {
                tracing::info!("pipeline_advance: returning Failed because status is Failed");
                return AdvanceResult::Failed { error: error.clone() };
            }
            PipelineStatus::WaitingForHuman { .. } => {
                return AdvanceResult::Failed {
                    error: "Cannot advance while waiting for human gate. Call resolve_gate() first.".into(),
                };
            }
            _ => {}
        }

        // Check if we've exhausted all steps
        if self.current_step >= self.flow.steps.len() {
            self.status = PipelineStatus::Completed;
            return AdvanceResult::Completed;
        }

        let step = &self.flow.steps[self.current_step];
        let now = now_secs();

        // Check gate — GSD mode only pauses at Advisor→Architect (GoalGate)
        if step.gate == GateType::Human
            && self.gate_resolved_for_step.as_ref() != Some(&step.id)
            && self.mode == RelayMode::Check
        {
            // Check mode: pause at every human gate
            self.status = PipelineStatus::WaitingForHuman {
                gate: GateType::Human,
                step_id: step.id.clone(),
                since: now,
            };
            self.pending_gate = Some(PendingGate {
                step_id: step.id.clone(),
                gate: GateType::Human,
                since: now,
            });
            return AdvanceResult::WaitForHuman {
                gate: GateType::Human,
                step_id: step.id.clone(),
            };
        }

        // GSD mode: only Advisor→Architect handoff requires human approval (GoalGate)
        if step.gate == GateType::Human
            && self.gate_resolved_for_step.as_ref() != Some(&step.id)
            && self.mode == RelayMode::GSD
            && step.profession_id == "advisor"
        {
            self.status = PipelineStatus::WaitingForHuman {
                gate: GateType::Human,
                step_id: step.id.clone(),
                since: now,
            };
            self.pending_gate = Some(PendingGate {
                step_id: step.id.clone(),
                gate: GateType::Human,
                since: now,
            });
            return AdvanceResult::WaitForHuman {
                gate: GateType::Human,
                step_id: step.id.clone(),
            };
        }

        // Transition to Running
        self.status = PipelineStatus::Running {
            step_id: step.id.clone(),
            profession_id: step.profession_id.clone(),
            started_at: now,
        };

        AdvanceResult::ExecuteStep {
            step_id: step.id.clone(),
            profession_id: step.profession_id.clone(),
            agent_config_id: step.agent_config_id.clone(),
        }
    }

    /// Submit the result of an agent turn to continue the pipeline.
    ///
    /// The handoff's `to` field and `routing_key` determine next routing.
    pub fn submit_handoff(&mut self, mut handoff: HandoffDocument) -> AdvanceResult {
        let now = now_secs();

        // Record the completed step
        let (step_id, started_at) = match &self.status {
            PipelineStatus::Running { step_id, started_at, .. } => (step_id.clone(), *started_at),
            _ => {
                self.status = PipelineStatus::Failed {
                    error: "submit_handoff called but no step is running".into(),
                };
                return self.advance();
            }
        };

        // Consume the gate resolution — next attempt at this step needs re-approval
        self.gate_resolved_for_step = None;

        let profession_id = self.flow.steps[self.current_step].profession_id.clone();

        // ─── Handoff target validation & auto-correction ────────────────────────
        let exit = self.flow.steps[self.current_step].exit.clone();
        let expected_prof = match &exit {
            ExitRouting::Next => {
                let next_idx = self.current_step + 1;
                if next_idx < self.flow.steps.len() {
                    Some(self.flow.steps[next_idx].profession_id.clone())
                } else {
                    None
                }
            }
            ExitRouting::Loop { target_step_id, .. } => {
                self.flow.get_step_index(target_step_id)
                    .map(|idx| self.flow.steps[idx].profession_id.clone())
            }
            ExitRouting::Branch { .. } => {
                // Branch routing depends on handoff fields; skip auto-correction
                None
            }
            ExitRouting::Condition { .. } => {
                // Condition routing depends on runtime state; skip auto-correction
                None
            }
        };

        if let Some(expected) = expected_prof {
            if handoff.to != expected {
                tracing::warn!(
                    "Handoff target '{}' does not match flow routing expected '{}'. Correcting to '{}'.",
                    handoff.to, expected, expected
                );
                self.gate_feedback
                    .entry(step_id.clone())
                    .or_default()
                    .push(format!(
                        "[AUTO-CORRECTION] Handoff target was '{}' but flow routing expects '{}'. Corrected automatically.",
                        handoff.to, expected
                    ));
                handoff.to = expected;
            }
        }

        self.step_history.push(StepRecord {
            step_id: step_id.clone(),
            profession_id: profession_id.clone(),
            handoff: Some(handoff.clone()),
            started_at,
            completed_at: now,
            iteration: *self.loop_counters.get(&step_id).unwrap_or(&0),
        });

        // Update cumulative tokens
        let step_tokens = handoff.token_usage.step_input + handoff.token_usage.step_output;
        self.cumulative_tokens += step_tokens;

        // Track in budget tracker
        self.budget_tracker.record(&profession_id, handoff.token_usage.step_input, handoff.token_usage.step_output);

        // Check budget enforcement
        match self.budget_tracker.check(&profession_id) {
            crate::relay::budget::BudgetAction::HardStop => {
                self.status = PipelineStatus::Failed {
                    error: format!("Budget exceeded: {} tokens spent vs {} limit", self.budget_tracker.cumulative, self.budget_tracker.run_budget.limit),
                };
                return AdvanceResult::Failed {
                    error: match &self.status {
                        PipelineStatus::Failed { error } => error.clone(),
                        _ => unreachable!(),
                    },
                };
            }
            _ => {} // Warning and None are non-fatal at this point
        }

        // ─── Auto-validation: check step produced valid output ─────────────────
        if let Some(fail_reason) = self.validate_step(&step_id, &handoff) {
            // Coder gets 2 self-retries (3 total attempts), then escalation to design
            // Other professions get 3 self-retries (4 total attempts), then hard stop
            let max_retries = if step_id == "code" { 2 } else { 3 };

            let retry_count = {
                let count = self.loop_counters.entry(step_id.clone()).or_insert(0);
                *count += 1;
                *count
            };

            if step_id == "code" && retry_count == max_retries {
                // Coder escalation: route back to design for re-architecture
                self.gate_feedback
                    .entry("design".to_string())
                    .or_default()
                    .push(format!(
                        "[ESCALATION FROM CODER] Code failed to compile after 3 attempts: {}\n\n\
                         The implementation cannot be built. Please revisit the design/architecture \
                         to ensure it is feasible.",
                        fail_reason
                    ));
                tracing::warn!(
                    "Coder failed auto-validation after 3 attempts. Escalating to design: {}",
                    fail_reason
                );
                // Reset code retry counter so future coder steps can retry again
                self.loop_counters.remove("code");
                // Route to design step
                if let Some(design_idx) = self.flow.step_for_profession("architect") {
                    self.current_step = design_idx;
                    return self.advance();
                }
            }

            if retry_count <= max_retries {
                // Auto-retry: feed error back as gate feedback and re-run same step
                self.gate_feedback
                    .entry(step_id.clone())
                    .or_default()
                    .push(format!(
                        "[AUTO-VALIDATION FAILED] {}\n\nPlease fix this issue before proceeding. This is attempt {}/{}.",
                        fail_reason, retry_count, max_retries
                    ));
                tracing::warn!(
                    "Step '{}' failed auto-validation (attempt {}/{}). Retrying with feedback: {}",
                    step_id, retry_count, max_retries, fail_reason
                );
                return AdvanceResult::ExecuteStep {
                    step_id: step_id.clone(),
                    profession_id: profession_id.clone(),
                    agent_config_id: None,
                };
            } else {
                // Max retries exceeded — hard stop
                let error = format!(
                    "Step '{}' failed auto-validation after {} retries: {}",
                    step_id, max_retries, fail_reason
                );
                tracing::error!("{}", error);
                self.status = PipelineStatus::Failed { error: error.clone() };
                return AdvanceResult::Failed { error };
            }
        }

        // Determine next step based on exit routing
        let step_id = self.flow.steps[self.current_step].id.clone();
        let exit = self.flow.steps[self.current_step].exit.clone();
        let next_index = self.resolve_next_step(&step_id, &exit, &handoff);

        match next_index {
            NextStep::Index(idx) => {
                self.current_step = idx;
                self.advance()
            }
            NextStep::Complete => {
                self.current_step = self.flow.steps.len();
                self.status = PipelineStatus::Completed;
                AdvanceResult::Completed
            }
            NextStep::Error(msg) => {
                self.status = PipelineStatus::Failed { error: msg };
                AdvanceResult::Failed {
                    error: match &self.status {
                        PipelineStatus::Failed { error } => error.clone(),
                        _ => unreachable!(),
                    },
                }
            }
        }
    }

    /// Resolve a human gate decision.
    pub fn resolve_gate(&mut self, decision: GateDecision) -> AdvanceResult {
        let pending = match self.pending_gate.take() {
            Some(g) => g,
            None => {
                return AdvanceResult::Failed {
                    error: "No pending gate to resolve".into(),
                };
            }
        };

        match decision {
            GateDecision::Approve | GateDecision::Edit { .. } => {
                // Mark gate as resolved for this step attempt
                self.gate_resolved_for_step = Some(pending.step_id.clone());
                self.status = PipelineStatus::Idle;
                self.advance()
            }
            GateDecision::Reject { feedback } => {
                // Store feedback and redraft: stay on same step
                self.gate_feedback
                    .entry(pending.step_id.clone())
                    .or_default()
                    .push(feedback);
                // Also mark resolved so we can re-enter the step
                self.gate_resolved_for_step = Some(pending.step_id.clone());
                self.status = PipelineStatus::Idle;
                self.advance()
            }
        }
    }

    /// Pause the pipeline at the current position.
    pub fn pause(&mut self) {
        if matches!(self.status, PipelineStatus::Running { .. }) {
            self.status = PipelineStatus::Paused {
                at_step: self.current_step,
            };
        }
    }

    /// Resume from a paused state.
    pub fn resume(&mut self) {
        if matches!(self.status, PipelineStatus::Paused { .. }) {
            self.status = PipelineStatus::Idle;
        }
    }

    /// Rerun from the current failed step.
    /// Resets loop counters and gate feedback for the current step,
    /// then transitions to Idle so the next advance() will re-execute it.
    pub fn rerun(&mut self) -> Option<AdvanceResult> {
        match &self.status {
            PipelineStatus::Failed { .. } => {
                let step_id = self.flow.steps.get(self.current_step)?.id.clone();
                // Reset retry counter so the step gets a fresh set of attempts
                self.loop_counters.insert(step_id.clone(), 0);
                // Clear accumulated gate feedback for this step
                self.gate_feedback.remove(&step_id);
                self.gate_resolved_for_step = None;
                // Move to Idle so advance() can pick it up
                self.status = PipelineStatus::Idle;
                Some(self.advance())
            }
            _ => None,
        }
    }

    /// Resolve the next step index from exit routing.
    fn resolve_next_step(&mut self, step_id: &str, exit: &ExitRouting, handoff: &HandoffDocument) -> NextStep {
        match exit {
            ExitRouting::Next => {
                let next = self.current_step + 1;
                if next >= self.flow.steps.len() {
                    NextStep::Complete
                } else {
                    NextStep::Index(next)
                }
            }
            ExitRouting::Branch { on, arms, default } => {
                let key = match on.as_str() {
                    "intent" | "to" => handoff.to.clone(),
                    "from" => handoff.from.clone(),
                    "classification" => {
                        // Use first decision status as classification, or summary hash
                        handoff.decisions.first()
                            .map(|d| d.status.clone())
                            .unwrap_or_else(|| "default".to_string())
                    }
                    _ => {
                        // Try to extract from handoff context or fall back to summary
                        extract_branch_key(handoff, on)
                    }
                };
                let target_id = arms.get(&key).unwrap_or(default);
                match self.flow.get_step_index(target_id) {
                    Some(idx) => NextStep::Index(idx),
                    None => NextStep::Error(format!("Branch target '{}' not found", target_id)),
                }
            }
            ExitRouting::Loop {
                target_step_id,
                max_iterations,
            } => {
                let count = self.loop_counters.entry(step_id.to_string()).or_insert(0);
                *count += 1;
                if *count >= *max_iterations {
                    // Break loop, go to next step
                    let next = self.current_step + 1;
                    if next >= self.flow.steps.len() {
                        NextStep::Complete
                    } else {
                        NextStep::Index(next)
                    }
                } else {
                    match self.flow.get_step_index(target_step_id) {
                        Some(idx) => NextStep::Index(idx),
                        None => NextStep::Error(format!(
                            "Loop target '{}' not found",
                            target_step_id
                        )),
                    }
                }
            }
            ExitRouting::Condition { condition, true_branch, false_branch } => {
                let result = self.evaluate_condition(condition, step_id, handoff);
                tracing::info!("Condition evaluated to {} for step '{}'", result, step_id);
                let branch = if result { true_branch } else { false_branch };
                self.resolve_next_step(step_id, branch, handoff)
            }
        }
    }

    /// Evaluate a routing condition against current pipeline state and handoff.
    fn evaluate_condition(&self, condition: &crate::relay::flow::RoutingCondition, step_id: &str, handoff: &HandoffDocument) -> bool {
        use crate::relay::flow::RoutingCondition;
        match condition {
            RoutingCondition::ValidatorFailed => {
                // Re-run validators for this step against the handoff.
                // In the current architecture this is most useful when the flow
                // is configured to route on validation failure instead of auto-retry.
                self.validate_step(step_id, handoff).is_some()
            }
            RoutingCondition::HandoffFieldEquals { field, value } => {
                let actual = match field.as_str() {
                    "to" => &handoff.to,
                    "from" => &handoff.from,
                    "run_id" => &handoff.run_id,
                    "summary" => &handoff.summary,
                    _ => {
                        // Try to match against a decision status
                        return handoff.decisions.iter().any(|d| {
                            d.status == *value && field.to_lowercase() == d.title.to_lowercase()
                        });
                    }
                };
                actual == value
            }
            RoutingCondition::TokenUsageCumulativeExceeds { limit } => {
                self.cumulative_tokens > *limit
            }
            RoutingCondition::TokenUsageStepExceeds { limit } => {
                let step_tokens = handoff.token_usage.step_input + handoff.token_usage.step_output;
                step_tokens > *limit
            }
            RoutingCondition::WorkProductExists { glob } => {
                let pattern = glob.replace("*", "");
                handoff.work_product.iter().any(|wp| {
                    wp.path.contains(&pattern)
                })
            }
            RoutingCondition::All(conditions) => {
                conditions.iter().all(|c| self.evaluate_condition(c, step_id, handoff))
            }
            RoutingCondition::Any(conditions) => {
                conditions.iter().any(|c| self.evaluate_condition(c, step_id, handoff))
            }
            RoutingCondition::Not(inner) => {
                !self.evaluate_condition(inner, step_id, handoff)
            }
        }
    }

    /// Convenience: which profession is currently/next expected.
    pub fn current_profession_id(&self) -> Option<&str> {
        self.flow.steps.get(self.current_step).map(|s| s.profession_id.as_str())
    }

    /// Convenience: current step ID.
    pub fn current_step_id(&self) -> Option<&str> {
        self.flow.steps.get(self.current_step).map(|s| s.id.as_str())
    }

    /// Auto-validate a step's handoff. Returns `Some(reason)` if validation fails.
    fn validate_step(&self, step_id: &str, handoff: &HandoffDocument) -> Option<String> {
        // Prefer step-specific validators from flow configuration
        if let Some(step) = self.flow.get_step(step_id) {
            if !step.validators.is_empty() {
                for validator in &step.validators {
                    if let Some(reason) = validator.check(handoff) {
                        return Some(reason);
                    }
                }
                return None;
            }
        }

        // Fallback to hardcoded validators for backward compatibility
        match step_id {
            "discover" => {
                if handoff.work_product.is_empty() && handoff.spec_updates.is_empty() {
                    return Some("Advisor produced no work_product. Use write_specs to create or update goals.".into());
                }
            }
            "design" => {
                let has_meaningful_work = handoff.work_product.iter().any(|wp| {
                    !wp.path.ends_with("README.md") && !wp.path.is_empty()
                });
                if !has_meaningful_work && handoff.spec_updates.is_empty() {
                    return Some("Architect produced no meaningful spec work. Use write_specs to update architecture and designs.".into());
                }
            }
            "plan" => {
                let has_plan_work = handoff.work_product.iter().any(|wp| {
                    wp.path.contains("plan") || wp.path.ends_with(".ad")
                });
                if !has_plan_work && handoff.work_product.len() < 2 && handoff.spec_updates.is_empty() {
                    return Some("Planner produced no plans. Use write_specs to create or update plans.".into());
                }
            }
            "code" => {
                let has_code_changes = handoff.work_product.iter().any(|wp| {
                    wp.path.ends_with(".rs") || wp.path.ends_with(".vue") || wp.path.ends_with(".ts")
                });
                if !has_code_changes {
                    return Some("Coder produced no code changes. Use write_file or edit_file to modify source files.".into());
                }
            }
            "review" => {
                if handoff.work_product.is_empty() && handoff.decisions.is_empty() && handoff.spec_updates.is_empty() {
                    return Some("Reviewer produced no review output. Use write_specs to update reviews section.".into());
                }
            }
            "report" => {
                if handoff.work_product.is_empty() && handoff.spec_updates.is_empty() {
                    return Some("Documenter produced no report. Use write_specs to update reports and finalize spec statuses.".into());
                }
            }
            _ => {}
        }
        None
    }
}

/// Internal result of next-step resolution.
#[derive(Debug)]
enum NextStep {
    Index(usize),
    Complete,
    Error(String),
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Extract a branch routing key from a handoff document based on a field name.
fn extract_branch_key(handoff: &HandoffDocument, field: &str) -> String {
    match field {
        "summary" => handoff.summary.clone(),
        "run_id" => handoff.run_id.clone(),
        _ => {
            // Heuristic: check if any decision title contains the field name
            for d in &handoff.decisions {
                if d.title.to_lowercase().contains(&field.to_lowercase()) {
                    return d.status.clone();
                }
            }
            // Default fallback
            "default".to_string()
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::flow::{FlowStep, RoutingCondition};
    use crate::relay::handoff::HandoffDocument;
    use std::collections::HashMap;

    fn make_handoff(from: &str, to: &str) -> HandoffDocument {
        HandoffDocument::new(from, to, "test-run", 1)
    }

    // ── S2.1: Sequential execution ───────────────────────────────────────────

    #[test]
    fn test_pipeline_executes_steps_in_order() {
        let mut flow = FlowSpec::new("test-seq");
        flow.add_step(FlowStep::new("s1", "planner"));
        flow.add_step(FlowStep::new("s2", "architect"));
        flow.add_step(FlowStep::new("s3", "coder"));

        let mut engine = PipelineEngine::new(flow, "run-1");

        // Step 1
        let r1 = engine.advance();
        assert_eq!(
            r1,
            AdvanceResult::ExecuteStep {
                step_id: "s1".into(),
                profession_id: "planner".into(),
                agent_config_id: None,
            }
        );

        let h1 = make_handoff("planner", "architect");
        let r2 = engine.submit_handoff(h1);
        assert_eq!(
            r2,
            AdvanceResult::ExecuteStep {
                step_id: "s2".into(),
                profession_id: "architect".into(),
                agent_config_id: None,
            }
        );

        // Step 2
        let h2 = make_handoff("architect", "coder");
        let r3 = engine.submit_handoff(h2);
        assert_eq!(
            r3,
            AdvanceResult::ExecuteStep {
                step_id: "s3".into(),
                profession_id: "coder".into(),
                agent_config_id: None,
            }
        );

        // Step 3 → Complete
        let h3 = make_handoff("coder", "tester");
        let r4 = engine.submit_handoff(h3);
        assert_eq!(r4, AdvanceResult::Completed);
        assert_eq!(engine.status, PipelineStatus::Completed);
        assert_eq!(engine.step_history.len(), 3);
    }

    // ── S2.2: Branching routes DIRECT intent ─────────────────────────────────

    #[test]
    fn test_pipeline_branch_direct_skips_advisor() {
        let mut flow = FlowSpec::new("test-branch");

        let mut arms = HashMap::new();
        arms.insert("coder".to_string(), "coder-step".to_string());
        arms.insert("advisor".to_string(), "advisor-step".to_string());

        flow.add_step(
            FlowStep::new("assistant-step", "assistant").with_exit(ExitRouting::Branch {
                on: "intent".to_string(),
                arms,
                default: "advisor-step".to_string(),
            }),
        );
        flow.add_step(FlowStep::new("advisor-step", "advisor"));
        flow.add_step(FlowStep::new("coder-step", "coder"));

        let mut engine = PipelineEngine::new(flow, "run-2");

        // Assistant runs
        let r1 = engine.advance();
        assert_eq!(r1, AdvanceResult::ExecuteStep { step_id: "assistant-step".into(), profession_id: "assistant".into(), agent_config_id: None });

        // Assistant classifies as DIRECT → handoff to coder
        let mut h = make_handoff("assistant", "coder");
        h.to = "coder".to_string();
        let r2 = engine.submit_handoff(h);
        assert_eq!(
            r2,
            AdvanceResult::ExecuteStep {
                step_id: "coder-step".into(),
                profession_id: "coder".into(),
                agent_config_id: None,
            }
        );

        // Verify advisor was skipped
        let professions_run: Vec<&str> = engine
            .step_history
            .iter()
            .map(|r| r.profession_id.as_str())
            .collect();
        assert_eq!(professions_run, vec!["assistant"]);

        // Finish coder
        let h2 = make_handoff("coder", "tester");
        let r3 = engine.submit_handoff(h2);
        assert_eq!(r3, AdvanceResult::Completed);
    }

    // ── S6.1: Human gate pauses until approval ───────────────────────────────

    #[test]
    fn test_human_gate_pauses_and_approves() {
        let mut flow = FlowSpec::new("test-gate");
        flow.add_step(FlowStep::new("s1", "advisor").with_gate(GateType::Human));
        flow.add_step(FlowStep::new("s2", "architect"));

        let mut engine = PipelineEngine::new(flow, "run-gate");

        // First advance hits the gate (advisor in GSD mode)
        let r1 = engine.advance();
        assert_eq!(
            r1,
            AdvanceResult::WaitForHuman {
                gate: GateType::Human,
                step_id: "s1".into(),
            }
        );
        assert!(matches!(engine.status, PipelineStatus::WaitingForHuman { .. }));

        // Cannot advance while waiting
        let r_err = engine.advance();
        assert!(matches!(r_err, AdvanceResult::Failed { .. }));

        // Approve the gate
        let r2 = engine.resolve_gate(GateDecision::Approve);
        assert_eq!(
            r2,
            AdvanceResult::ExecuteStep {
                step_id: "s1".into(),
                profession_id: "advisor".into(),
                agent_config_id: None,
            }
        );

        // Submit handoff → architect
        let h = make_handoff("advisor", "architect");
        let r3 = engine.submit_handoff(h);
        assert_eq!(
            r3,
            AdvanceResult::ExecuteStep {
                step_id: "s2".into(),
                profession_id: "architect".into(),
                agent_config_id: None,
            }
        );
    }

    #[test]
    fn test_human_gate_reject_redrafts() {
        let mut flow = FlowSpec::new("test-reject");
        flow.add_step(FlowStep::new("s1", "advisor").with_gate(GateType::Human));

        let mut engine = PipelineEngine::new(flow, "run-reject");

        // Hit gate
        let _ = engine.advance();
        assert!(matches!(engine.status, PipelineStatus::WaitingForHuman { .. }));

        // Reject with feedback
        let r = engine.resolve_gate(GateDecision::Reject {
            feedback: "Need more detail on error handling".into(),
        });

        // Should re-enter the same step
        assert_eq!(
            r,
            AdvanceResult::ExecuteStep {
                step_id: "s1".into(),
                profession_id: "advisor".into(),
                agent_config_id: None,
            }
        );

        // Feedback stored
        assert_eq!(engine.gate_feedback.get("s1").unwrap().len(), 1);
    }

    // ── S7.2: GSD mode auto-approves all gates except Advisor ──────────────────

    #[test]
    fn test_gsd_mode_only_pauses_at_advisor_gate() {
        let mut flow = FlowSpec::new("test-gsd");
        flow.add_step(FlowStep::new("s1", "advisor").with_gate(GateType::Human));
        flow.add_step(FlowStep::new("s2", "architect").with_gate(GateType::Human));
        flow.add_step(FlowStep::new("s3", "planner").with_gate(GateType::Human));

        let mut engine = PipelineEngine::new(flow, "run-gsd");
        engine.mode = RelayMode::GSD;

        // Advisor gate → pause
        let r1 = engine.advance();
        assert!(
            matches!(r1, AdvanceResult::WaitForHuman { step_id, .. } if step_id == "s1")
        );

        // Approve
        let _ = engine.resolve_gate(GateDecision::Approve);
        let h1 = make_handoff("advisor", "architect");
        let r2 = engine.submit_handoff(h1);

        // Architect gate → auto in GSD
        assert!(
            matches!(r2, AdvanceResult::ExecuteStep { step_id, .. } if step_id == "s2")
        );

        let h2 = make_handoff("architect", "planner");
        let r3 = engine.submit_handoff(h2);

        // Planner gate → auto in GSD
        assert!(
            matches!(r3, AdvanceResult::ExecuteStep { step_id, .. } if step_id == "s3")
        );
    }

    // ── S7.3: Check mode pauses at every human-configured gate ─────────────────

    #[test]
    fn test_check_mode_pauses_at_all_gates() {
        let mut flow = FlowSpec::new("test-check");
        flow.add_step(FlowStep::new("s1", "advisor").with_gate(GateType::Human));
        flow.add_step(FlowStep::new("s2", "architect").with_gate(GateType::Human));
        flow.add_step(FlowStep::new("s3", "planner").with_gate(GateType::Human));

        let mut engine = PipelineEngine::new(flow, "run-check");
        engine.mode = RelayMode::Check;

        // Advisor gate → pause
        let r1 = engine.advance();
        assert!(matches!(r1, AdvanceResult::WaitForHuman { .. }));
        let _ = engine.resolve_gate(GateDecision::Approve);
        let r2 = engine.submit_handoff(make_handoff("advisor", "architect"));

        // Architect gate → pause (submit_handoff already advanced and hit the gate)
        assert!(matches!(r2, AdvanceResult::WaitForHuman { .. }));
        let _ = engine.resolve_gate(GateDecision::Approve);
        let r3 = engine.submit_handoff(make_handoff("architect", "planner"));

        // Planner gate → pause
        assert!(matches!(r3, AdvanceResult::WaitForHuman { .. }));
    }

    // ── Budget enforcement ───────────────────────────────────────────────────

    #[test]
    fn test_budget_hardstop_prevents_runaway() {
        use crate::relay::budget::TokenBudget;
        use crate::relay::handoff::TokenUsage;

        let mut flow = FlowSpec::new("test-budget");
        flow.add_step(FlowStep::new("s1", "planner"));
        flow.add_step(FlowStep::new("s2", "architect"));

        // Tight budget: 500 tokens
        let mut engine = PipelineEngine::with_budget(flow, "run-budget", TokenBudget::new(500));

        // Step 1: 300 tokens — under budget
        let _ = engine.advance();
        let mut h1 = make_handoff("planner", "architect");
        h1.token_usage = TokenUsage { step_input: 200, step_output: 100, cumulative: 300, budget_remaining: 200 };
        let r1 = engine.submit_handoff(h1);
        assert!(matches!(r1, AdvanceResult::ExecuteStep { .. }));
        assert_eq!(engine.budget_tracker.cumulative, 300);

        // Step 2: 300 tokens — cumulative 600 > 500 limit → HardStop
        let _ = engine.advance();
        let mut h2 = make_handoff("architect", "coder");
        h2.token_usage = TokenUsage { step_input: 200, step_output: 100, cumulative: 600, budget_remaining: 0 };
        let r2 = engine.submit_handoff(h2);
        assert!(matches!(r2, AdvanceResult::Failed { .. }), "Expected budget hard-stop");
        assert!(matches!(engine.status, PipelineStatus::Failed { .. }));
    }

    #[test]
    fn test_budget_warning_non_fatal() {
        use crate::relay::budget::TokenBudget;
        use crate::relay::handoff::TokenUsage;

        let mut flow = FlowSpec::new("test-budget-warn");
        flow.add_step(FlowStep::new("s1", "planner"));

        // Budget 1000, warning at 700
        let mut engine = PipelineEngine::with_budget(flow, "run-warn", TokenBudget::new(1000));

        let _ = engine.advance();
        let mut h = make_handoff("planner", "done");
        // 800 tokens — above warning (700) but below limit (1000)
        h.token_usage = TokenUsage { step_input: 500, step_output: 300, cumulative: 800, budget_remaining: 200 };
        let r = engine.submit_handoff(h);
        // Should complete normally; warning is advisory
        assert_eq!(r, AdvanceResult::Completed);
    }

    // ── Loop routing ─────────────────────────────────────────────────────────

    #[test]
    fn test_loop_routing_bounded() {
        let mut flow = FlowSpec::new("test-loop");
        flow.add_step(
            FlowStep::new("s1", "tester").with_exit(ExitRouting::Loop {
                target_step_id: "s1".to_string(),
                max_iterations: 3,
            }),
        );
        flow.add_step(FlowStep::new("s2", "reviewer"));

        let mut engine = PipelineEngine::new(flow, "run-loop");

        // Iteration 1
        let _ = engine.advance();
        let h = make_handoff("tester", "tester");
        let r = engine.submit_handoff(h);
        assert_eq!(
            r,
            AdvanceResult::ExecuteStep {
                step_id: "s1".into(),
                profession_id: "tester".into(),
                agent_config_id: None,
            }
        );
        assert_eq!(engine.loop_counters.get("s1"), Some(&1));

        // Iteration 2
        let h = make_handoff("tester", "tester");
        let r = engine.submit_handoff(h);
        assert_eq!(r, AdvanceResult::ExecuteStep { step_id: "s1".into(), profession_id: "tester".into(), agent_config_id: None });
        assert_eq!(engine.loop_counters.get("s1"), Some(&2));

        // Iteration 3 → break to reviewer
        let h = make_handoff("tester", "tester");
        let r = engine.submit_handoff(h);
        assert_eq!(
            r,
            AdvanceResult::ExecuteStep {
                step_id: "s2".into(),
                profession_id: "reviewer".into(),
                agent_config_id: None,
            }
        );
        assert_eq!(engine.loop_counters.get("s1"), Some(&3));
    }

    // ── Edge cases ───────────────────────────────────────────────────────────

    #[test]
    fn test_empty_flow_completes_immediately() {
        let flow = FlowSpec::new("empty");
        let mut engine = PipelineEngine::new(flow, "run-empty");
        let r = engine.advance();
        assert_eq!(r, AdvanceResult::Completed);
    }

    #[test]
    fn test_completed_engine_stays_completed() {
        let mut flow = FlowSpec::new("tiny");
        flow.add_step(FlowStep::new("s1", "assistant"));
        let mut engine = PipelineEngine::new(flow, "run");

        let _ = engine.advance();
        let _ = engine.submit_handoff(make_handoff("assistant", "done"));
        assert_eq!(engine.status, PipelineStatus::Completed);

        let r = engine.advance();
        assert_eq!(r, AdvanceResult::Completed);
    }

    // ── Conditional routing ──────────────────────────────────────────────────

    #[test]
    fn test_condition_routing_handoff_field_equals() {
        use crate::relay::flow::RoutingCondition;
        let mut flow = FlowSpec::new("test-condition");
        flow.add_step(FlowStep::new("s1", "assistant").with_exit(ExitRouting::Condition {
            condition: RoutingCondition::HandoffFieldEquals {
                field: "to".into(),
                value: "coder".into(),
            },
            true_branch: Box::new(ExitRouting::Branch {
                on: "to".into(),
                arms: {
                    let mut m = HashMap::new();
                    m.insert("coder".into(), "coder-step".into());
                    m
                },
                default: "coder-step".into(),
            }),
            false_branch: Box::new(ExitRouting::Next),
        }));
        flow.add_step(FlowStep::new("planner-step", "planner"));
        flow.add_step(FlowStep::new("coder-step", "coder"));

        let mut engine = PipelineEngine::new(flow, "run-condition");

        // s1: assistant
        let _ = engine.advance();

        // handoff.to == "coder" → true branch → coder-step
        let mut h = make_handoff("assistant", "coder");
        h.to = "coder".into();
        let r = engine.submit_handoff(h);
        assert_eq!(
            r,
            AdvanceResult::ExecuteStep {
                step_id: "coder-step".into(),
                profession_id: "coder".into(),
                agent_config_id: None,
            }
        );
    }

    #[test]
    fn test_condition_routing_token_usage_exceeds() {
        use crate::relay::flow::RoutingCondition;
        let mut flow = FlowSpec::new("test-condition-tokens");
        flow.add_step(FlowStep::new("s1", "assistant").with_exit(ExitRouting::Condition {
            condition: RoutingCondition::TokenUsageStepExceeds { limit: 100 },
            true_branch: Box::new(ExitRouting::Next), // planner
            false_branch: Box::new(ExitRouting::Loop {
                target_step_id: "s1".into(),
                max_iterations: 2,
            }),
        }));
        flow.add_step(FlowStep::new("s2", "planner"));

        let mut engine = PipelineEngine::new(flow, "run-tokens");

        // s1: low token usage (< 100) → false branch → loop back to s1
        let _ = engine.advance();
        let mut h = make_handoff("assistant", "planner");
        h.token_usage.step_input = 50;
        h.token_usage.step_output = 30;
        let r = engine.submit_handoff(h);
        assert_eq!(
            r,
            AdvanceResult::ExecuteStep {
                step_id: "s1".into(),
                profession_id: "assistant".into(),
                agent_config_id: None,
            }
        );

        // s1 again: high token usage (> 100) → true branch → next (planner)
        let mut h = make_handoff("assistant", "planner");
        h.token_usage.step_input = 80;
        h.token_usage.step_output = 50;
        let r = engine.submit_handoff(h);
        assert_eq!(
            r,
            AdvanceResult::ExecuteStep {
                step_id: "s2".into(),
                profession_id: "planner".into(),
                agent_config_id: None,
            }
        );
    }

    #[test]
    fn test_condition_routing_any_composite() {
        use crate::relay::flow::RoutingCondition;
        let mut flow = FlowSpec::new("test-condition-any");
        flow.add_step(FlowStep::new("s1", "assistant").with_exit(ExitRouting::Condition {
            condition: RoutingCondition::Any(vec![
                RoutingCondition::HandoffFieldEquals { field: "to".into(), value: "coder".into() },
                RoutingCondition::HandoffFieldEquals { field: "to".into(), value: "tester".into() },
            ]),
            true_branch: Box::new(ExitRouting::Next), // go to s2
            false_branch: Box::new(ExitRouting::Loop {
                target_step_id: "s1".into(),
                max_iterations: 2,
            }),
        }));
        flow.add_step(FlowStep::new("s2", "planner"));

        let mut engine = PipelineEngine::new(flow, "run-any");

        // to == "tester" matches Any → true branch → next
        let _ = engine.advance();
        let mut h = make_handoff("assistant", "tester");
        h.to = "tester".into();
        let r = engine.submit_handoff(h);
        assert_eq!(
            r,
            AdvanceResult::ExecuteStep {
                step_id: "s2".into(),
                profession_id: "planner".into(),
                agent_config_id: None,
            }
        );
    }

    #[test]
    fn test_condition_routing_not_negation() {
        use crate::relay::flow::RoutingCondition;
        let mut flow = FlowSpec::new("test-condition-not");
        flow.add_step(FlowStep::new("s1", "assistant").with_exit(ExitRouting::Condition {
            condition: RoutingCondition::Not(Box::new(
                RoutingCondition::HandoffFieldEquals { field: "to".into(), value: "skip".into() },
            )),
            true_branch: Box::new(ExitRouting::Next), // go to s2
            false_branch: Box::new(ExitRouting::Loop {
                target_step_id: "s1".into(),
                max_iterations: 2,
            }),
        }));
        flow.add_step(FlowStep::new("s2", "planner"));

        let mut engine = PipelineEngine::new(flow, "run-not");

        // to != "skip" → Not(false) = true → next
        let _ = engine.advance();
        let mut h = make_handoff("assistant", "planner");
        h.to = "planner".into();
        let r = engine.submit_handoff(h);
        assert_eq!(
            r,
            AdvanceResult::ExecuteStep {
                step_id: "s2".into(),
                profession_id: "planner".into(),
                agent_config_id: None,
            }
        );
    }
}
