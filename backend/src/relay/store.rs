//! Relay Run Store
//!
//! In-memory store for active and completed pipeline runs.
//! Provides the bridge between the deterministic PipelineEngine and HTTP APIs.

use crate::relay::flow::FlowSpec;
use crate::relay::handoff::{HandoffDocument, ReportReference};
use crate::relay::pipeline::{AdvanceResult, GateDecision, PipelineEngine, PipelineStatus, StepRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Shared in-memory store for all relay runs.
pub type RunStore = Arc<Mutex<HashMap<String, RunEntry>>>;

/// An entry in the run store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEntry {
    pub run_id: String,
    pub engine: PipelineEngine,
    pub created_at: u64,
    pub updated_at: u64,
    /// Serialized events for SSE replay.
    pub events: Vec<RunEvent>,
    /// Optional metadata for tracking run origin and notifications
    #[serde(default)]
    pub metadata: RunMetadata,
}

/// Metadata about a relay run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunMetadata {
    /// The chat session that spawned this run (for notifications)
    #[serde(default)]
    pub originating_chat_session: Option<String>,
    /// Auto-generated title for display
    #[serde(default)]
    pub title: Option<String>,
}

/// A run event for SSE streaming and history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    StepStarted { step_id: String, profession_id: String },
    StepCompleted { step_id: String, handoff_summary: String },
    GateWaiting { step_id: String, gate: String },
    GateResolved { step_id: String, decision: String },
    RunCompleted,
    RunFailed { error: String },
    TokenSpend { cumulative: u64, step_tokens: u64 },
    RelayCompleteNotification {
        run_id: String,
        status: String,
        title: String,
        summary: String,
        report_link: Option<ReportLink>,
        timestamp: u64,
    },
    // ─── Turn events (for session log persistence) ───
    TurnDelta { profession_id: String, text: String },
    TurnToolCall { profession_id: String, tool_id: String, tool_name: String, arguments: serde_json::Value },
    TurnToolResult { profession_id: String, tool_id: String, result: String },
    TurnComplete { profession_id: String },
    TurnError { profession_id: String, message: String },
    TurnBudgetWarning { profession_id: String, remaining: u64 },
    TurnBudgetExceeded { profession_id: String },
}

/// Summary of a run for listing.
#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub run_id: String,
    pub status: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub current_profession: Option<String>,
    pub cumulative_tokens: u64,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Detailed run state for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct RunState {
    pub run_id: String,
    pub status: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub steps: Vec<StepState>,
    pub step_history: Vec<StepRecord>,
    pub cumulative_tokens: u64,
    pub budget_limit: u64,
    pub budget_remaining: u64,
    pub waiting_for_gate: Option<GateState>,
    pub parallel_estimate: u64,
    pub savings: u64,
    pub savings_ratio: f64,
    pub events: Vec<RunEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepState {
    pub id: String,
    pub profession_id: String,
    pub status: String, // "pending", "running", "completed", "failed"
    pub gate: String,   // "auto", "human"
}

#[derive(Debug, Clone, Serialize)]
pub struct GateState {
    pub step_id: String,
    pub profession_id: String,
    pub since: u64,
}

/// Report link for relay completion notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportLink {
    pub url: String,
    pub report_id: String,
    pub report_title: String,
}

/// Directory where runs are persisted.
fn persistence_dir() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("autoforge")
        .join("runs");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Save a run entry to disk.
pub fn save_run(entry: &RunEntry) {
    let dir = persistence_dir().join(&entry.run_id);
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("run.json");
    if let Ok(json) = serde_json::to_string_pretty(entry) {
        let _ = fs::write(&path, json);
    }
}

/// Load all persisted runs from disk.
pub fn load_all_runs() -> RunStore {
    let store = Arc::new(Mutex::new(HashMap::new()));
    let dir = persistence_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return store;
    };

    for entry in entries.flatten() {
        let path = entry.path().join("run.json");
        if path.exists() {
            if let Ok(data) = fs::read_to_string(&path) {
                if let Ok(run_entry) = serde_json::from_str::<RunEntry>(&data) {
                    let mut map = store.lock().unwrap();
                    map.insert(run_entry.run_id.clone(), run_entry);
                }
            }
        }
    }
    store
}

/// Create a new shared run store, loading persisted runs.
pub fn new_run_store() -> RunStore {
    load_all_runs()
}

/// Start a new run with the given flow spec.
pub fn start_run(store: &RunStore, flow: FlowSpec, run_id: impl Into<String>) -> Result<RunState, String> {
    let run_id = run_id.into();
    let mut map = store.lock().unwrap();
    if map.contains_key(&run_id) {
        return Err(format!("Run {} already exists", run_id));
    }

    let mut engine = PipelineEngine::new(flow.clone(), &run_id);

    // Set per-step token budgets from profession configurations
    let registry = crate::relay::ProfessionRegistry::new();
    for step in &flow.steps {
        if let Some(profession) = registry.get(&step.profession_id) {
            let budget = crate::relay::budget::TokenBudget::new(profession.token_budget);
            engine.budget_tracker.set_step_budget(&step.profession_id, budget);
        }
    }

    let now = now_secs();
    let entry = RunEntry {
        run_id: run_id.clone(),
        engine,
        created_at: now,
        updated_at: now,
        events: Vec::new(),
        metadata: RunMetadata::default(),
    };

    let state = build_run_state(&entry);
    save_run(&entry);
    map.insert(run_id, entry);
    Ok(state)
}

/// Get the current state of a run.
pub fn get_run(store: &RunStore, run_id: &str) -> Option<RunState> {
    let map = store.lock().unwrap();
    map.get(run_id).map(build_run_state)
}

/// List all runs.
pub fn list_runs(store: &RunStore) -> Vec<RunSummary> {
    let map = store.lock().unwrap();
    map.values().map(|e| RunSummary {
        run_id: e.run_id.clone(),
        status: e.engine.status.to_status_str(),
        current_step: e.engine.current_step,
        total_steps: e.engine.flow.steps.len(),
        current_profession: e.engine.current_profession_id().map(|s| s.to_string()),
        cumulative_tokens: e.engine.cumulative_tokens,
        created_at: e.created_at,
        updated_at: e.updated_at,
        title: e.metadata.title.clone(),
    }).collect()
}

/// Advance a run by one step.
pub fn advance_run(store: &RunStore, run_id: &str) -> Option<AdvanceResult> {
    let mut map = store.lock().unwrap();
    let entry = map.get_mut(run_id)?;
    let result = entry.engine.advance();
    entry.updated_at = now_secs();

    match &result {
        AdvanceResult::ExecuteStep { step_id, profession_id, .. } => {
            entry.events.push(RunEvent::StepStarted {
                step_id: step_id.clone(),
                profession_id: profession_id.clone(),
            });
        }
        AdvanceResult::WaitForHuman { step_id, .. } => {
            entry.events.push(RunEvent::GateWaiting {
                step_id: step_id.clone(),
                gate: "human".into(),
            });
        }
        AdvanceResult::Completed => {
            entry.events.push(RunEvent::RunCompleted);
        }
        AdvanceResult::Failed { error } => {
            entry.events.push(RunEvent::RunFailed { error: error.clone() });
        }
    }

    save_run(entry);
    Some(result.clone())
}

/// Submit a handoff for the current step.
pub fn submit_handoff(store: &RunStore, run_id: &str, handoff: HandoffDocument) -> Option<AdvanceResult> {
    let mut map = store.lock().unwrap();
    let entry = map.get_mut(run_id)?;
    let result = entry.engine.submit_handoff(handoff.clone());
    entry.updated_at = now_secs();

    let step_tokens = handoff.token_usage.step_input + handoff.token_usage.step_output;
    entry.events.push(RunEvent::TokenSpend {
        cumulative: entry.engine.cumulative_tokens,
        step_tokens,
    });

    match &result {
        AdvanceResult::ExecuteStep { step_id, .. } => {
            entry.events.push(RunEvent::StepCompleted {
                step_id: step_id.clone(),
                handoff_summary: handoff.summary.clone(),
            });
        }
        AdvanceResult::Completed => {
            entry.events.push(RunEvent::RunCompleted);
        }
        AdvanceResult::Failed { error } => {
            entry.events.push(RunEvent::RunFailed { error: error.clone() });
        }
        _ => {}
    }

    save_run(entry);
    Some(result.clone())
}

/// Delete a run from the store and remove its persisted file.
pub fn delete_run(store: &RunStore, run_id: &str) -> bool {
    let mut map = store.lock().unwrap();
    let removed = map.remove(run_id).is_some();
    if removed {
        let path = persistence_dir().join(run_id).join("run.json");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(path.parent().unwrap_or(&path));
    }
    removed
}

/// Resolve a human gate for a run.
pub fn resolve_gate(store: &RunStore, run_id: &str, decision: GateDecision) -> Option<AdvanceResult> {
    let mut map = store.lock().unwrap();
    let entry = map.get_mut(run_id)?;
    let result = entry.engine.resolve_gate(decision.clone());
    entry.updated_at = now_secs();

    let decision_str = match decision {
        GateDecision::Approve => "approve",
        GateDecision::Reject { .. } => "reject",
        GateDecision::Edit { .. } => "edit",
    };

    if let Some(step_id) = entry.engine.current_step_id() {
        entry.events.push(RunEvent::GateResolved {
            step_id: step_id.to_string(),
            decision: decision_str.into(),
        });
    }

    save_run(entry);
    Some(result.clone())
}

/// Build a RunState from a RunEntry.
fn build_run_state(entry: &RunEntry) -> RunState {
    let engine = &entry.engine;
    let steps: Vec<StepState> = engine.flow.steps.iter().enumerate().map(|(idx, step)| {
        let status = if idx < engine.current_step {
            "completed"
        } else if idx == engine.current_step && matches!(engine.status, PipelineStatus::Running { .. }) {
            "running"
        } else if idx == engine.current_step && matches!(engine.status, PipelineStatus::WaitingForHuman { .. }) {
            "waiting_gate"
        } else {
            "pending"
        };
        StepState {
            id: step.id.clone(),
            profession_id: step.profession_id.clone(),
            status: status.into(),
            gate: match step.gate {
                crate::relay::flow::GateType::Auto => "auto",
                crate::relay::flow::GateType::Human => "human",
            }.into(),
        }
    }).collect();

    let waiting_for_gate = if let PipelineStatus::WaitingForHuman { step_id, since, .. } = &engine.status {
        engine.flow.get_step(step_id).map(|step| GateState {
            step_id: step_id.clone(),
            profession_id: step.profession_id.clone(),
            since: *since,
        })
    } else {
        None
    };

    let (savings, savings_ratio) = engine.budget_tracker.savings_vs_parallel(
        engine.flow.steps.len() as u32,
        5000, // avg_context heuristic
        3,    // rounds heuristic
    );

    RunState {
        run_id: entry.run_id.clone(),
        status: engine.status.to_status_str(),
        current_step: engine.current_step,
        total_steps: engine.flow.steps.len(),
        steps,
        step_history: engine.step_history.clone(),
        cumulative_tokens: engine.cumulative_tokens,
        budget_limit: engine.budget_tracker.run_budget.limit,
        budget_remaining: engine.budget_tracker.run_budget.limit.saturating_sub(engine.budget_tracker.cumulative),
        waiting_for_gate,
        parallel_estimate: engine.budget_tracker.estimate_parallel_cost(engine.flow.steps.len() as u32, 5000, 3),
        savings,
        savings_ratio,
        events: entry.events.clone(),
        title: entry.metadata.title.clone(),
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::flow::{FlowSpec, FlowStep, GateType};
    use crate::relay::handoff::{HandoffDocument, TokenUsage};

    #[test]
    fn test_run_store_start_and_get() {
        let store = new_run_store();
        let mut flow = FlowSpec::new("test");
        flow.add_step(FlowStep::new("s1", "planner"));
        flow.add_step(FlowStep::new("s2", "coder"));

        let run_id = format!("run-1-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        let state = start_run(&store, flow, &run_id).unwrap();
        assert_eq!(state.run_id, run_id);
        assert_eq!(state.total_steps, 2);
        assert_eq!(state.status, "idle");

        let fetched = get_run(&store, &run_id).unwrap();
        assert_eq!(fetched.run_id, run_id);
    }

    #[test]
    fn test_run_store_advance_and_handoff() {
        let store = new_run_store();
        let mut flow = FlowSpec::new("test");
        flow.add_step(FlowStep::new("s1", "planner"));

        let run_id = format!("run-adv-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        start_run(&store, flow, &run_id).unwrap();

        let r = advance_run(&store, &run_id).unwrap();
        assert!(matches!(r, AdvanceResult::ExecuteStep { .. }));

        let h = HandoffDocument::new("planner", "done", &run_id, 0);
        let r2 = submit_handoff(&store, &run_id, h).unwrap();
        assert_eq!(r2, AdvanceResult::Completed);

        let state = get_run(&store, &run_id).unwrap();
        assert_eq!(state.status, "completed");
        assert_eq!(state.current_step, 1);
    }

    #[test]
    fn test_run_store_gate_waiting() {
        let store = new_run_store();
        let mut flow = FlowSpec::new("test");
        flow.add_step(FlowStep::new("s1", "advisor").with_gate(GateType::Human));

        let run_id = format!("run-gate-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        start_run(&store, flow, &run_id).unwrap();
        let r = advance_run(&store, &run_id).unwrap();
        assert!(matches!(r, AdvanceResult::WaitForHuman { .. }));

        let state = get_run(&store, &run_id).unwrap();
        assert!(state.waiting_for_gate.is_some());
        assert_eq!(state.steps[0].status, "waiting_gate");

        // Resolve gate
        let r2 = resolve_gate(&store, &run_id, GateDecision::Approve).unwrap();
        assert!(matches!(r2, AdvanceResult::ExecuteStep { .. }));

        let state2 = get_run(&store, &run_id).unwrap();
        assert!(state2.waiting_for_gate.is_none());
    }

    #[test]
    fn test_run_store_budget_tracking() {
        let store = new_run_store();
        let mut flow = FlowSpec::new("test");
        flow.add_step(FlowStep::new("s1", "planner"));

        let run_id = format!("run-budget-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        start_run(&store, flow, &run_id).unwrap();
        advance_run(&store, &run_id);

        let mut h = HandoffDocument::new("planner", "done", &run_id, 0);
        h.token_usage = TokenUsage { step_input: 1000, step_output: 500, cumulative: 1500, budget_remaining: 9_998_500 };
        submit_handoff(&store, &run_id, h);

        let state = get_run(&store, &run_id).unwrap();
        assert_eq!(state.cumulative_tokens, 1500);
        assert_eq!(state.budget_limit, 10_000_000);
        assert_eq!(state.budget_remaining, 10_000_000 - 1500);
    }
}
