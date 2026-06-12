//! TaskPlan execution engine.
//!
//! Drives a multi-relay `TaskPlan` by executing phases serially or in parallel,
//! waiting at join gates, and propagating failures. Actual relay runs are
//! performed by a pluggable executor so the engine stays testable without LLM
//! calls.

use crate::provider::ClaudeProviderState;
use crate::relay::api::RunEventBroadcast;
use crate::relay::flows::get_flow;
use crate::relay::handoff::HandoffDocument;
use crate::relay::handoff_store::HandoffStore;
use crate::relay::store::{
    get_run, new_run_store, start_run_with_metadata, RunMetadata, RunStore,
};
use crate::relay::task_plan::{PhaseMode, RunRef, TaskMode, TaskPlan};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Shared context needed to execute a TaskPlan instance.
#[derive(Clone)]
pub struct TaskPlanContext {
    pub run_store: RunStore,
    pub handoff_store: Arc<HandoffStore>,
    pub event_tx: broadcast::Sender<RunEventBroadcast>,
    pub ai_provider: Option<ClaudeProviderState>,
    pub project_path: String,
}

impl TaskPlanContext {
    /// Create a context rooted in a project directory.
    pub fn new(project_path: impl Into<String>) -> Self {
        let path = project_path.into();
        let handoff_store = Arc::new(HandoffStore::new(std::path::PathBuf::from(&path)));
        let (event_tx, _rx) = broadcast::channel(256);
        Self {
            run_store: new_run_store(),
            handoff_store,
            event_tx,
            ai_provider: None,
            project_path: path,
        }
    }

    /// Set the AI provider used by the default relay executor.
    pub fn with_ai_provider(mut self, provider: ClaudeProviderState) -> Self {
        self.ai_provider = Some(provider);
        self
    }
}

/// A request to execute a single relay run inside a TaskPlan.
#[derive(Clone, Debug)]
pub struct RunRequest {
    pub task_plan_id: String,
    pub phase_name: String,
    pub phase_index: usize,
    pub run_ref: RunRef,
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub root_run_id: String,
    pub project_path: String,
    pub task: String,
    pub mode: TaskMode,
}

/// Result of executing a single relay run.
#[derive(Clone, Debug)]
pub struct RunExecutionResult {
    pub run_id: String,
    /// Terminal status string from the run store (e.g. "completed", "failed").
    pub status: String,
    pub handoff: Option<HandoffDocument>,
    pub error: Option<String>,
}

/// Overall status of a TaskPlan execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPlanStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Paused,
}

/// Status of an individual phase within a TaskPlan execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Per-phase execution state.
#[derive(Debug, Clone)]
pub struct PhaseState {
    pub status: PhaseStatus,
    pub run_results: HashMap<String, RunExecutionResult>,
}

/// Per-run execution state tracked by the engine.
#[derive(Debug, Clone)]
pub struct TaskPlanRunState {
    pub run_id: String,
    pub phase_name: String,
    pub run_name: String,
    pub status: String,
    pub handoff: Option<HandoffDocument>,
}

/// Deterministic executor for TaskPlan instances.
pub struct TaskPlanEngine {
    pub plan: TaskPlan,
    pub instance_id: String,
    pub project_path: String,
    pub initial_input: String,
    pub status: TaskPlanStatus,
    pub phase_states: HashMap<String, PhaseState>,
    pub run_states: HashMap<String, TaskPlanRunState>,
}

impl TaskPlanEngine {
    /// Create a new engine for a plan.
    pub fn new(plan: TaskPlan, project_path: impl Into<String>, initial_input: impl Into<String>) -> Self {
        let instance_id = format!("{}-{}", plan.id, uuid::Uuid::new_v4());
        let mut phase_states = HashMap::new();
        for phase in &plan.phases {
            phase_states.insert(
                phase.name.clone(),
                PhaseState {
                    status: PhaseStatus::Pending,
                    run_results: HashMap::new(),
                },
            );
        }
        Self {
            plan,
            instance_id,
            project_path: project_path.into(),
            initial_input: initial_input.into(),
            status: TaskPlanStatus::Pending,
            phase_states,
            run_states: HashMap::new(),
        }
    }

    /// Validate that the plan can be executed.
    pub fn validate(&self) -> Result<(), String> {
        self.plan.validate().map_err(|e| e.to_string())?;

        for phase in &self.plan.phases {
            for run in &phase.runs {
                if get_flow(&run.flow_id).is_none() {
                    return Err(format!(
                        "run '{}' references unknown flow '{}'",
                        run.name, run.flow_id
                    ));
                }
            }
        }
        Ok(())
    }

    /// Execute the plan to completion using the provided run executor.
    pub async fn execute<F, Fut>(&mut self, ctx: &TaskPlanContext, executor: F) -> Result<(), String>
    where
        F: Fn(RunRequest) -> Fut,
        Fut: Future<Output = Result<RunExecutionResult, String>> + Send,
    {
        self.validate()?;

        self.status = TaskPlanStatus::Running;
        self.broadcast(
            ctx,
            "task_plan_started",
            json!({
                "task_plan_id": self.plan.id,
                "instance_id": self.instance_id,
            }),
        );

        let order = self.topological_order()?;
        let mut completed = HashSet::new();

        for phase_name in order {
            let phase = self.plan
                .phases
                .iter()
                .find(|p| p.name == phase_name)
                .cloned()
                .ok_or_else(|| format!("phase '{}' disappeared", phase_name))?;

            self.set_phase_status(&phase.name, PhaseStatus::Running);
            self.broadcast(
                ctx,
                "phase_started",
                json!({
                    "task_plan_id": self.plan.id,
                    "instance_id": self.instance_id,
                    "phase": phase.name,
                    "mode": format!("{:?}", phase.mode).to_lowercase(),
                }),
            );

            let phase_index = self.plan.phases.iter().position(|p| p.name == phase.name).unwrap_or(0);
            let run_refs = phase.runs.clone();
            let results = match phase.mode {
                PhaseMode::Serial => {
                    let mut results = Vec::new();
                    for run_ref in run_refs {
                        let req = self.build_run_request(&ctx.handoff_store, &phase, phase_index, &run_ref);
                        let result = self.run_one(ctx, &executor, req).await?;
                        results.push((run_ref.name.clone(), result));
                    }
                    results
                }
                PhaseMode::Parallel => {
                    let futures: Vec<_> = run_refs
                        .into_iter()
                        .map(|run_ref| {
                            let req = self.build_run_request(&ctx.handoff_store, &phase, phase_index, &run_ref);
                            let fut = self.run_one(ctx, &executor, req);
                            async move { (run_ref.name.clone(), fut.await) }
                        })
                        .collect();

                    let mut results = Vec::new();
                    for fut in futures {
                        let (name, res) = fut.await;
                        results.push((name, res?));
                    }
                    results
                }
            };

            let mut phase_failed = false;
            let mut failure_error = None;
            for (run_name, result) in &results {
                if result.status != "completed" {
                    phase_failed = true;
                    failure_error = Some(
                        result
                            .error
                            .clone()
                            .unwrap_or_else(|| format!("run '{}' ended with status '{}'", run_name, result.status)),
                    );
                }
                if let Some(ref handoff) = result.handoff {
                    if let Err(e) = ctx
                        .handoff_store
                        .save(&self.plan.id, &phase.name, run_name, handoff)
                    {
                        tracing::warn!("Failed to save handoff for {}.{}: {}", phase.name, run_name, e);
                    }
                }
                self.run_states.insert(
                    result.run_id.clone(),
                    TaskPlanRunState {
                        run_id: result.run_id.clone(),
                        phase_name: phase.name.clone(),
                        run_name: run_name.clone(),
                        status: result.status.clone(),
                        handoff: result.handoff.clone(),
                    },
                );
            }

            if let Some(state) = self.phase_states.get_mut(&phase.name) {
                for (run_name, result) in &results {
                    state.run_results.insert(run_name.clone(), result.clone());
                }
            }

            if phase_failed {
                self.set_phase_status(&phase.name, PhaseStatus::Failed);
                let error = failure_error.unwrap_or_default();
                self.broadcast(
                    ctx,
                    "phase_failed",
                    json!({
                        "task_plan_id": self.plan.id,
                        "instance_id": self.instance_id,
                        "phase": phase.name,
                        "error": error,
                    }),
                );
                self.status = TaskPlanStatus::Failed;
                self.broadcast(
                    ctx,
                    "task_plan_failed",
                    json!({
                        "task_plan_id": self.plan.id,
                        "instance_id": self.instance_id,
                        "phase": phase.name,
                        "error": error,
                    }),
                );
                return Err(error);
            }

            self.set_phase_status(&phase.name, PhaseStatus::Completed);
            completed.insert(phase.name.clone());
            self.broadcast(
                ctx,
                "phase_completed",
                json!({
                    "task_plan_id": self.plan.id,
                    "instance_id": self.instance_id,
                    "phase": phase.name,
                }),
            );
        }

        self.status = TaskPlanStatus::Completed;
        self.broadcast(
            ctx,
            "task_plan_completed",
            json!({
                "task_plan_id": self.plan.id,
                "instance_id": self.instance_id,
            }),
        );
        Ok(())
    }

    /// Execute a single run through the supplied executor.
    async fn run_one<F, Fut>(
        &self,
        ctx: &TaskPlanContext,
        executor: &F,
        req: RunRequest,
    ) -> Result<RunExecutionResult, String>
    where
        F: Fn(RunRequest) -> Fut,
        Fut: Future<Output = Result<RunExecutionResult, String>> + Send,
    {
        self.broadcast(
            ctx,
            "run_started",
            json!({
                "task_plan_id": self.plan.id,
                "instance_id": self.instance_id,
                "phase": req.phase_name,
                "run": req.run_ref.name,
                "run_id": req.run_id,
            }),
        );

        let run_name = req.run_ref.name.clone();
        let phase_name = req.phase_name.clone();
        let result = executor(req).await.map_err(|e| format!("executor error: {}", e))?;

        self.broadcast(
            ctx,
            "run_completed",
            json!({
                "task_plan_id": self.plan.id,
                "instance_id": self.instance_id,
                "phase": phase_name,
                "run": run_name,
                "run_id": result.run_id,
                "status": result.status,
            }),
        );

        Ok(result)
    }

    /// Build the execution request for a single run, including resolved inputs.
    fn build_run_request(
        &self,
        handoff_store: &HandoffStore,
        phase: &crate::relay::task_plan::Phase,
        phase_index: usize,
        run_ref: &RunRef,
    ) -> RunRequest {
        let run_id = format!(
            "{}--{}--{}--{}",
            self.instance_id,
            phase.name,
            run_ref.name,
            uuid::Uuid::new_v4()
        );

        let mut task_parts = Vec::new();

        if let Some(input) = &run_ref.input {
            task_parts.push(input.clone());
        } else if phase_index == 0 {
            task_parts.push(self.initial_input.clone());
        }

        for path in &run_ref.input_from {
            let full_path = format!("{}.{}", self.plan.id, path);
            match handoff_store.resolve_path(&full_path) {
                Some(value) => {
                    task_parts.push(format!("## Input from {}\n\n{}", path, value));
                }
                None => {
                    task_parts.push(format!(
                        "## Input from {}\n\n(resolved value missing)",
                        path
                    ));
                }
            }
        }

        if let Some(context) = &run_ref.context {
            task_parts.push(format!("## Context\n\n{}", context));
        }

        let task = task_parts.join("\n\n");
        let mode = run_ref.mode_override.unwrap_or(self.plan.default_mode);

        RunRequest {
            task_plan_id: self.plan.id.clone(),
            phase_name: phase.name.clone(),
            phase_index,
            run_ref: run_ref.clone(),
            run_id,
            parent_run_id: None,
            root_run_id: self.instance_id.clone(),
            project_path: self.project_path.clone(),
            task,
            mode,
        }
    }

    fn set_phase_status(&mut self, phase_name: &str, status: PhaseStatus) {
        if let Some(state) = self.phase_states.get_mut(phase_name) {
            state.status = status;
        }
    }

    fn topological_order(&self) -> Result<Vec<String>, String> {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let mut order = Vec::new();

        for phase in &self.plan.phases {
            if !visited.contains(&phase.name) {
                self.dfs(&phase.name, &mut visited, &mut stack, &mut order)?;
            }
        }
        Ok(order)
    }

    fn dfs(
        &self,
        name: &str,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if stack.contains(name) {
            return Err(format!("cycle detected at phase '{}'", name));
        }
        if visited.contains(name) {
            return Ok(());
        }
        stack.insert(name.to_string());

        let phase = self
            .plan
            .phases
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| format!("phase '{}' not found", name))?;
        for dep in &phase.depends_on {
            self.dfs(dep, visited, stack, order)?;
        }

        stack.remove(name);
        visited.insert(name.to_string());
        order.push(name.to_string());
        Ok(())
    }

    fn broadcast(&self, ctx: &TaskPlanContext, event_type: &str, payload: serde_json::Value) {
        let _ = ctx.event_tx.send(RunEventBroadcast {
            run_id: self.instance_id.clone(),
            event_type: event_type.to_string(),
            payload: Some(payload),
        });
    }
}

/// Default relay executor: starts a run in the run store, drives it with the
/// background relay driver, and waits for it to reach a terminal state.
pub async fn drive_task_plan_run(
    ctx: &TaskPlanContext,
    req: RunRequest,
) -> Result<RunExecutionResult, String> {
    let flow = get_flow(&req.run_ref.flow_id)
        .ok_or_else(|| format!("flow '{}' not found", req.run_ref.flow_id))?;

    let metadata = RunMetadata {
        originating_chat_session: None,
        title: Some(format!(
            "{} / {} / {}",
            req.task_plan_id, req.phase_name, req.run_ref.name
        )),
        project_path: Some(ctx.project_path.clone()),
        task_plan_id: Some(req.task_plan_id.clone()),
        task_run_name: Some(req.run_ref.name.clone()),
        phase_name: Some(req.phase_name.clone()),
        phase_index: Some(req.phase_index),
        parent_run_id: req.parent_run_id.clone(),
        root_run_id: Some(req.root_run_id.clone()),
    };

    start_run_with_metadata(&ctx.run_store, flow, &req.run_id, metadata)
        .map_err(|e| format!("failed to start run {}: {}", req.run_id, e))?;

    let provider = ctx
        .ai_provider
        .as_ref()
        .ok_or_else(|| "AI provider not configured in TaskPlanContext".to_string())?
        .clone();

    crate::relay::driver::drive_run(
        req.run_id.clone(),
        ctx.run_store.clone(),
        ctx.event_tx.clone(),
        provider,
        req.task,
        ctx.project_path.clone(),
    )
    .await;

    let state = get_run(&ctx.run_store, &req.run_id)
        .ok_or_else(|| format!("run {} disappeared after execution", req.run_id))?;

    let error = if state.status == "completed" {
        None
    } else {
        Some(format!("run ended with status '{}'", state.status))
    };

    let handoff = state
        .step_history
        .last()
        .and_then(|rec| rec.handoff.clone());

    Ok(RunExecutionResult {
        run_id: req.run_id,
        status: state.status,
        handoff,
        error,
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::task_plan::{Phase, RunRef, TaskPlan};
    use std::sync::{Arc, Mutex};

    fn test_context(tmp: &tempfile::TempDir) -> TaskPlanContext {
        let mut guard = crate::relay::flows::FLOW_REGISTRY.lock().unwrap();
        if guard.is_none() {
            *guard = Some(crate::relay::flows::FlowRegistry::load_builtins_only());
        }
        drop(guard);
        TaskPlanContext::new(tmp.path().to_str().unwrap())
    }

    fn success_executor() -> impl Fn(RunRequest) -> std::future::Ready<Result<RunExecutionResult, String>> {
        |req| {
            let mut handoff = HandoffDocument::new("assistant", "next", &req.run_id, 0);
            handoff.summary = req.task.clone();
            std::future::ready(Ok(RunExecutionResult {
                run_id: req.run_id,
                status: "completed".to_string(),
                handoff: Some(handoff),
                error: None,
            }))
        }
    }

    #[tokio::test]
    async fn serial_plan_executes_end_to_end() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = test_context(&tmp);

        let plan = TaskPlan::new("serial")
            .add_phase(Phase::new("p1").add_run(RunRef::new("r1", "fast-track")))
            .add_phase(
                Phase::new("p2")
                    .depends_on(vec!["p1".to_string()])
                    .add_run(RunRef::new("r2", "fast-track")),
            );

        let mut engine = TaskPlanEngine::new(plan, tmp.path().to_str().unwrap(), "do work");
        engine.execute(&ctx, success_executor()).await.unwrap();

        assert_eq!(engine.status, TaskPlanStatus::Completed);
        assert_eq!(
            engine.phase_states.get("p1").unwrap().status,
            PhaseStatus::Completed
        );
        assert_eq!(
            engine.phase_states.get("p2").unwrap().status,
            PhaseStatus::Completed
        );
    }

    #[tokio::test]
    async fn parallel_phase_waits_for_all_runs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = test_context(&tmp);

        let plan = TaskPlan::new("parallel").add_phase(
            Phase::new("p1")
                .with_mode(PhaseMode::Parallel)
                .add_run(RunRef::new("a", "fast-track"))
                .add_run(RunRef::new("b", "fast-track")),
        );

        let mut engine = TaskPlanEngine::new(plan, tmp.path().to_str().unwrap(), "do work");
        engine.execute(&ctx, success_executor()).await.unwrap();

        let state = engine.phase_states.get("p1").unwrap();
        assert_eq!(state.status, PhaseStatus::Completed);
        assert!(state.run_results.contains_key("a"));
        assert!(state.run_results.contains_key("b"));
    }

    #[tokio::test]
    async fn failed_run_fails_task_plan() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = test_context(&tmp);

        let plan = TaskPlan::new("fail").add_phase(
            Phase::new("p1")
                .add_run(RunRef::new("r1", "fast-track"))
                .add_run(RunRef::new("r2", "fast-track")),
        );

        let executor = |req: RunRequest| {
            let status = if req.run_ref.name == "r2" {
                "failed".to_string()
            } else {
                "completed".to_string()
            };
            std::future::ready(Ok(RunExecutionResult {
                run_id: req.run_id,
                status,
                handoff: None,
                error: Some("boom".to_string()),
            }))
        };

        let mut engine = TaskPlanEngine::new(plan, tmp.path().to_str().unwrap(), "do work");
        let err = engine.execute(&ctx, executor).await.unwrap_err();
        assert!(err.contains("boom") || err.contains("failed"));
        assert_eq!(engine.status, TaskPlanStatus::Failed);
        assert_eq!(engine.phase_states.get("p1").unwrap().status, PhaseStatus::Failed);
    }

    #[tokio::test]
    async fn input_from_resolves_previous_handoff() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = test_context(&tmp);

        // Seed the handoff store with a previous run's output.
        let mut handoff = HandoffDocument::new("assistant", "next", "prev", 0);
        handoff.summary = "previous result".to_string();
        ctx.handoff_store
            .save("input-plan", "p1", "r1", &handoff)
            .unwrap();

        let plan = TaskPlan::new("input-plan")
            .add_phase(Phase::new("p2").add_run(
                RunRef::new("r2", "fast-track")
                    .with_input_from(vec!["p1.r1.handoff.summary".to_string()]),
            ));

        let captured = Arc::new(Mutex::new(Vec::new()));
        let captured_exec = captured.clone();
        let executor = move |req: RunRequest| {
            captured_exec.lock().unwrap().push(req.task.clone());
            let mut handoff = HandoffDocument::new("assistant", "next", &req.run_id, 0);
            handoff.summary = req.task.clone();
            std::future::ready(Ok(RunExecutionResult {
                run_id: req.run_id,
                status: "completed".to_string(),
                handoff: Some(handoff),
                error: None,
            }))
        };

        let mut engine = TaskPlanEngine::new(plan, tmp.path().to_str().unwrap(), "initial task");
        engine.execute(&ctx, executor).await.unwrap();

        let tasks = captured.lock().unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].contains("previous result"));
    }
}
