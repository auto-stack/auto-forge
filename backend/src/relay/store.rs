//! Relay Run Store
//!
//! In-memory store for active and completed pipeline runs.
//! Provides the bridge between the deterministic PipelineEngine and HTTP APIs.

use crate::relay::flow::FlowSpec;
use crate::relay::flows::get_flow;
use crate::relay::handoff::{HandoffDocument, ReportReference};
use crate::relay::pipeline::{AdvanceResult, GateDecision, PipelineEngine, PipelineStatus, StepRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Maximum length of a tool result string stored in events.
const MAX_TOOL_RESULT_LEN: usize = 4000;
/// Maximum events returned in API responses (prevents multi-GB JSON).
const MAX_EVENTS_IN_RESPONSE: usize = 1000;
/// Maximum events kept in disk storage (older events are dropped).
const MAX_EVENTS_IN_STORAGE: usize = 1000;

/// Truncate a tool result string to a reasonable length.
pub fn truncate_tool_result(result: &str) -> String {
    if result.len() <= MAX_TOOL_RESULT_LEN {
        result.to_string()
    } else {
        let mut truncated = result.chars().take(MAX_TOOL_RESULT_LEN).collect::<String>();
        truncated.push_str("\n\n...[truncated: ");
        truncated.push_str(&result.len().to_string());
        truncated.push_str(" chars total]");
        truncated
    }
}

/// Trim in-memory events if they exceed a threshold.
/// Call this after pushing new events to prevent unbounded growth.
pub fn maybe_trim_events_in_memory(events: &mut Vec<RunEvent>) {
    if events.len() > MAX_EVENTS_IN_STORAGE * 2 {
        *events = trim_events_for_storage(events);
    }
}

/// Trim events for storage: keep only the last N and truncate tool results.
fn trim_events_for_storage(events: &[RunEvent]) -> Vec<RunEvent> {
    let slice = if events.len() > MAX_EVENTS_IN_STORAGE {
        &events[events.len() - MAX_EVENTS_IN_STORAGE..]
    } else {
        events
    };
    slice
        .iter()
        .map(|e| match e {
            RunEvent::TurnToolResult { timestamp, profession_id, tool_id, result } => {
                RunEvent::TurnToolResult {
                    timestamp: *timestamp,
                    profession_id: profession_id.clone(),
                    tool_id: tool_id.clone(),
                    result: truncate_tool_result(result),
                }
            }
            _ => e.clone(),
        })
        .collect()
}

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
    /// Original task description used to spawn the run.
    #[serde(default)]
    pub initial_task: Option<String>,
    /// Project path this run belongs to, if any.
    #[serde(default)]
    pub project_path: Option<String>,
    /// TaskPlan this run belongs to, if any.
    #[serde(default)]
    pub task_plan_id: Option<String>,
    /// Name of the run inside the TaskPlan.
    #[serde(default)]
    pub task_run_name: Option<String>,
    /// Name of the phase this run belongs to.
    #[serde(default)]
    pub phase_name: Option<String>,
    /// Index of the phase within the TaskPlan.
    #[serde(default)]
    pub phase_index: Option<usize>,
    /// Parent run ID for nested TaskPlans.
    #[serde(default)]
    pub parent_run_id: Option<String>,
    /// Root run ID of the TaskPlan tree.
    #[serde(default)]
    pub root_run_id: Option<String>,
}

/// A run event for SSE streaming and history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    StepStarted { #[serde(default)] timestamp: u64, step_id: String, profession_id: String },
    StepCompleted { #[serde(default)] timestamp: u64, step_id: String, handoff_summary: String },
    GateWaiting { #[serde(default)] timestamp: u64, step_id: String, gate: String },
    GateResolved { #[serde(default)] timestamp: u64, step_id: String, decision: String },
    RunCompleted { #[serde(default)] timestamp: u64 },
    RunFailed { #[serde(default)] timestamp: u64, error: String },
    TokenSpend { #[serde(default)] timestamp: u64, cumulative: u64, step_tokens: u64 },
    RelayUpdate { #[serde(default)] timestamp: u64, step_id: String, profession_id: String, status: String },
    RelayCompleteNotification {
        run_id: String,
        status: String,
        title: String,
        summary: String,
        report_link: Option<ReportLink>,
        timestamp: u64,
    },
    // ─── Turn events (for session log persistence) ───
    TurnDelta { #[serde(default)] timestamp: u64, profession_id: String, text: String },
    TurnToolCall { #[serde(default)] timestamp: u64, profession_id: String, tool_id: String, tool_name: String, arguments: serde_json::Value },
    TurnToolResult { #[serde(default)] timestamp: u64, profession_id: String, tool_id: String, result: String },
    TurnComplete { #[serde(default)] timestamp: u64, profession_id: String },
    TurnError { #[serde(default)] timestamp: u64, profession_id: String, message: String },
    TurnBudgetWarning { #[serde(default)] timestamp: u64, profession_id: String, remaining: u64 },
    TurnBudgetExceeded { #[serde(default)] timestamp: u64, profession_id: String },
    TurnThinking { #[serde(default)] timestamp: u64, profession_id: String, thinking: String },
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_run_id: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_step_started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_run_id: Option<String>,
    /// Per-profession token totals aggregated from TokenSpend events.
    /// Key = profession_id, Value = cumulative tokens.
    #[serde(default)]
    pub profession_tokens: std::collections::HashMap<String, u64>,
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

/// Synchronous disk write (used by the background persistence task).
fn save_run_sync(entry: &RunEntry) {
    let dir = persistence_dir().join(&entry.run_id);
    if let Err(e) = fs::create_dir_all(&dir) {
        tracing::error!("save_run: failed to create dir {}: {}", dir.display(), e);
        return;
    }
    let path = dir.join("run.json");
    match serde_json::to_string_pretty(entry) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, &json) {
                tracing::error!("save_run: failed to write {} for run {}: {}", path.display(), entry.run_id, e);
            } else {
                tracing::debug!("save_run: persisted run {} to {}", entry.run_id, path.display());
            }
        }
        Err(e) => {
            tracing::error!("save_run: failed to serialize run {}: {}", entry.run_id, e);
        }
    }
}

/// Background async persistence queue.
/// Calling `save_run()` only clones the entry and sends it to a channel —
/// the actual disk I/O happens on a blocking thread pool without blocking callers.
static SAVE_RUN_TX: std::sync::Mutex<Option<mpsc::UnboundedSender<RunEntry>>> = std::sync::Mutex::new(None);

fn ensure_save_run_tx() -> Option<mpsc::UnboundedSender<RunEntry>> {
    let mut tx_opt = SAVE_RUN_TX.lock().unwrap();
    if let Some(ref tx) = *tx_opt {
        return Some(tx.clone());
    }
    // Only spawn the background task if a tokio runtime is available.
    if tokio::runtime::Handle::try_current().is_ok() {
        let (tx, mut rx) = mpsc::unbounded_channel::<RunEntry>();
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                let _ = tokio::task::spawn_blocking(move || {
                    save_run_sync(&entry);
                }).await;
            }
        });
        *tx_opt = Some(tx.clone());
        Some(tx)
    } else {
        None
    }
}

/// Queue a run entry for async persistence.
/// This is non-blocking when a tokio runtime is present.
/// Falls back to synchronous disk write in test contexts without a runtime.
pub fn save_run(entry: &RunEntry) {
    // Trim events before cloning to avoid multi-GB channel sends.
    let trimmed = RunEntry {
        run_id: entry.run_id.clone(),
        engine: entry.engine.clone(),
        created_at: entry.created_at,
        updated_at: entry.updated_at,
        events: trim_events_for_storage(&entry.events),
        metadata: entry.metadata.clone(),
    };
    if let Some(tx) = ensure_save_run_tx() {
        let _ = tx.send(trimmed);
    } else {
        save_run_sync(&trimmed);
    }
}

/// Save an entry and also trim its in-memory events if they have grown too large.
/// Use this in hot paths (e.g. driver event loops) to prevent unbounded memory growth.
pub fn save_and_trim(entry: &mut RunEntry) {
    maybe_trim_events_in_memory(&mut entry.events);
    save_run(entry);
}

/// Load all persisted runs from disk.
pub fn load_all_runs() -> RunStore {
    let store = Arc::new(Mutex::new(HashMap::new()));
    let dir = persistence_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("load_all_runs: cannot read dir {}: {}", dir.display(), e);
            return store;
        }
    };

    let mut loaded = 0;
    let mut failed = 0;
    for entry in entries.flatten() {
        let path = entry.path().join("run.json");
        if !path.exists() {
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(data) => {
                match serde_json::from_str::<RunEntry>(&data) {
                    Ok(run_entry) => {
                        let mut map = store.lock().unwrap();
                        map.insert(run_entry.run_id.clone(), run_entry);
                        loaded += 1;
                    }
                    Err(e) => {
                        tracing::error!("load_all_runs: failed to parse {}: {}", path.display(), e);
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                tracing::error!("load_all_runs: failed to read {}: {}", path.display(), e);
                failed += 1;
            }
        }
    }
    if loaded > 0 || failed > 0 {
        tracing::info!("load_all_runs: loaded {} runs, {} failed from {}", loaded, failed, dir.display());
    }
    store
}

/// Create a new shared run store, loading persisted runs.
pub fn new_run_store() -> RunStore {
    load_all_runs()
}

/// Start a new run with the given flow spec.
pub fn start_run(
    store: &RunStore,
    flow: FlowSpec,
    run_id: impl Into<String>,
    project_path: Option<String>,
) -> Result<RunState, String> {
    let metadata = RunMetadata {
        project_path,
        ..RunMetadata::default()
    };
    start_run_with_metadata(store, flow, run_id, metadata)
}

/// Start a new run with full metadata.
pub fn start_run_with_metadata(
    store: &RunStore,
    flow: FlowSpec,
    run_id: impl Into<String>,
    metadata: RunMetadata,
) -> Result<RunState, String> {
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
        metadata,
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

/// Normalize a project path for comparison.
/// Converts backslashes to forward slashes so Windows and URL-encoded
/// variants compare equal.
fn normalize_project_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// List runs, optionally filtered by project path.
///
/// If `project_path` is provided, returns runs that belong to that project.
/// Runs with no project_path (legacy runs) are included as a fallback so
/// existing data remains visible.
pub fn list_runs(store: &RunStore, project_path: Option<String>) -> Vec<RunSummary> {
    let map = store.lock().unwrap();
    map.values()
        .filter(|e| match &project_path {
            Some(p) => {
                let run_path = e.metadata.project_path.as_deref().unwrap_or("");
                run_path.is_empty() || normalize_project_path(run_path) == normalize_project_path(p)
            }
            None => true,
        })
        .map(|e| RunSummary {
            run_id: e.run_id.clone(),
            status: e.engine.status.to_status_str(),
            current_step: e.engine.current_step,
            total_steps: e.engine.flow.steps.len(),
            current_profession: e.engine.current_profession_id().map(|s| s.to_string()),
            cumulative_tokens: e.engine.cumulative_tokens,
            created_at: e.created_at,
            updated_at: e.updated_at,
            title: e.metadata.title.clone(),
            project_path: e.metadata.project_path.clone(),
            task_plan_id: e.metadata.task_plan_id.clone(),
            parent_run_id: e.metadata.parent_run_id.clone(),
            root_run_id: e.metadata.root_run_id.clone(),
        })
        .collect()
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
                timestamp: now_secs(),
                step_id: step_id.clone(),
                profession_id: profession_id.clone(),
            });
        }
        AdvanceResult::WaitForHuman { step_id, .. } => {
            entry.events.push(RunEvent::GateWaiting {
                timestamp: now_secs(),
                step_id: step_id.clone(),
                gate: "human".into(),
            });
        }
        AdvanceResult::Completed => {
            entry.events.push(RunEvent::RunCompleted { timestamp: now_secs() });
        }
        AdvanceResult::Failed { error } => {
            entry.events.push(RunEvent::RunFailed { timestamp: now_secs(), error: error.clone() });
        }
        AdvanceResult::Paused { step_id, reason } => {
            entry.events.push(RunEvent::RelayUpdate {
                timestamp: now_secs(),
                step_id: step_id.clone(),
                profession_id: entry.engine.current_profession_id().unwrap_or("").to_string(),
                status: format!("paused: {}", reason),
            });
        }
    }

    save_run(entry);
    Some(result.clone())
}

/// Resume a paused run.
/// Resets loop counters and advances from the paused step.
pub fn resume_run(store: &RunStore, run_id: &str) -> Option<AdvanceResult> {
    let mut map = store.lock().unwrap();
    let entry = map.get_mut(run_id)?;
    // Refresh flow configuration from the latest registry so that validator
    // and routing changes take effect on resumed runs.
    if let Some(latest_flow) = get_flow(&entry.engine.flow.id) {
        entry.engine.flow = latest_flow;
    }
    let result = entry.engine.resume()?;
    entry.updated_at = now_secs();

    match &result {
        AdvanceResult::ExecuteStep { step_id, profession_id, .. } => {
            entry.events.push(RunEvent::StepStarted {
                timestamp: now_secs(),
                step_id: step_id.clone(),
                profession_id: profession_id.clone(),
            });
        }
        AdvanceResult::Completed => {
            entry.events.push(RunEvent::RunCompleted { timestamp: now_secs() });
        }
        AdvanceResult::Failed { error } => {
            entry.events.push(RunEvent::RunFailed { timestamp: now_secs(), error: error.clone() });
        }
        _ => {}
    }

    save_run(entry);
    Some(result.clone())
}

/// Rerun a failed run from the current failed step.
/// Resets retry counters and clears gate feedback, then advances.
pub fn rerun_run(store: &RunStore, run_id: &str) -> Option<AdvanceResult> {
    let mut map = store.lock().unwrap();
    let entry = map.get_mut(run_id)?;
    // Refresh flow configuration from the latest registry so that validator
    // and routing changes take effect on reruns.
    if let Some(latest_flow) = get_flow(&entry.engine.flow.id) {
        entry.engine.flow = latest_flow;
    }
    let result = entry.engine.rerun()?;
    entry.updated_at = now_secs();

    match &result {
        AdvanceResult::ExecuteStep { step_id, profession_id, .. } => {
            entry.events.push(RunEvent::StepStarted {
                timestamp: now_secs(),
                step_id: step_id.clone(),
                profession_id: profession_id.clone(),
            });
        }
        AdvanceResult::Completed => {
            entry.events.push(RunEvent::RunCompleted { timestamp: now_secs() });
        }
        AdvanceResult::Failed { error } => {
            entry.events.push(RunEvent::RunFailed { timestamp: now_secs(), error: error.clone() });
        }
        _ => {}
    }

    save_run(entry);
    Some(result.clone())
}

/// Submit a handoff for the current step.
pub fn submit_handoff(store: &RunStore, run_id: &str, handoff: HandoffDocument) -> Option<AdvanceResult> {
    let mut map = store.lock().unwrap();
    let entry = map.get_mut(run_id)?;
    // Capture the just-completed step ID before the engine advances
    let completed_step_id = entry.engine.flow.steps.get(entry.engine.current_step)
        .map(|s| s.id.clone())
        .unwrap_or_default();
    let result = entry.engine.submit_handoff(handoff.clone());
    entry.updated_at = now_secs();

    // Note: per-turn TokenSpend events are already pushed by the
    // TurnEvent::Usage handler in the driver. We do not add another
    // TokenSpend here to avoid double-counting. The engine's
    // cumulative_tokens is updated once by engine.submit_handoff().

    match &result {
        AdvanceResult::ExecuteStep { .. } => {
            entry.events.push(RunEvent::StepCompleted {
                timestamp: now_secs(),
                step_id: completed_step_id,
                handoff_summary: handoff.summary.clone(),
            });
        }
        AdvanceResult::Completed => {
            entry.events.push(RunEvent::RunCompleted { timestamp: now_secs() });
        }
        AdvanceResult::Failed { error } => {
            entry.events.push(RunEvent::RunFailed { timestamp: now_secs(), error: error.clone() });
        }
        AdvanceResult::Paused { step_id, reason } => {
            entry.events.push(RunEvent::RelayUpdate {
                timestamp: now_secs(),
                step_id: step_id.clone(),
                profession_id: entry.engine.current_profession_id().unwrap_or("").to_string(),
                status: format!("paused: {}", reason),
            });
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

/// Update the metadata of a run.
///
/// If `title` is `Some("")`, the custom title is cleared and the run will
/// fall back to its auto-generated title. If `title` is `Some("text")`,
/// it sets a custom title. If `None`, no update is performed.
pub fn update_run_metadata(
    store: &RunStore,
    run_id: &str,
    title: Option<String>,
) -> Option<RunState> {
    let mut map = store.lock().unwrap();
    let entry = map.get_mut(run_id)?;

    // Update title if provided
    if let Some(t) = title {
        entry.metadata.title = if t.trim().is_empty() {
            None // Clear custom title, will fall back to auto-generated
        } else {
            Some(t.trim().to_string())
        };
    }

    // Update timestamp
    entry.updated_at = now_secs();

    // Build state response before saving
    let state = build_run_state(entry);

    // Persist changes
    save_run(entry);

    Some(state)
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
            timestamp: now_secs(),
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
        } else if idx == engine.current_step && matches!(engine.status, PipelineStatus::Paused { .. }) {
            "paused"
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

    let current_step_started_at = match &engine.status {
        PipelineStatus::Running { started_at, .. } => Some(*started_at),
        _ => None,
    };

    // Only return the most recent events in API responses to prevent
    // multi-GB JSON payloads that crash the browser.
    let events_for_response = {
        let slice = if entry.events.len() > MAX_EVENTS_IN_RESPONSE {
            &entry.events[entry.events.len() - MAX_EVENTS_IN_RESPONSE..]
        } else {
            &entry.events[..]
        };
        slice
            .iter()
            .map(|e| match e {
                RunEvent::TurnToolResult { timestamp, profession_id, tool_id, result } => {
                    RunEvent::TurnToolResult {
                        timestamp: *timestamp,
                        profession_id: profession_id.clone(),
                        tool_id: tool_id.clone(),
                        result: truncate_tool_result(result),
                    }
                }
                _ => e.clone(),
            })
            .collect()
    };

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
        events: events_for_response,
        title: entry.metadata.title.clone(),
        current_step_started_at,
        task_plan_id: entry.metadata.task_plan_id.clone(),
        parent_run_id: entry.metadata.parent_run_id.clone(),
        root_run_id: entry.metadata.root_run_id.clone(),
        profession_tokens: build_profession_tokens(entry),
    }
}

/// Aggregate per-profession token totals from step history handoffs.
/// Each StepRecord has a profession_id and handoff.token_usage; we sum
/// step_input + step_output per profession for the cost breakdown chart.
fn build_profession_tokens(entry: &RunEntry) -> std::collections::HashMap<String, u64> {
    let mut map = std::collections::HashMap::new();
    for rec in &entry.engine.step_history {
        if let Some(ref handoff) = rec.handoff {
            let total = handoff.token_usage.step_input + handoff.token_usage.step_output;
            *map.entry(rec.profession_id.clone()).or_insert(0) += total;
        }
    }
    map
}

pub fn now_secs() -> u64 {
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
        let state = start_run(&store, flow, &run_id, None).unwrap();
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
        start_run(&store, flow, &run_id, None).unwrap();

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
        start_run(&store, flow, &run_id, None).unwrap();
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
        start_run(&store, flow, &run_id, None).unwrap();
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
