//! Integration tests for the TaskPlan execution engine.

use auto_forge::relay::handoff::HandoffDocument;
use auto_forge::relay::task_plan::{Phase, PhaseMode, RunRef, TaskPlan};
use auto_forge::relay::task_plan_engine::{
    RunExecutionResult, RunRequest, TaskPlanContext, TaskPlanEngine, TaskPlanStatus, PhaseStatus,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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

fn context(tmp: &tempfile::TempDir) -> TaskPlanContext {
    let mut ctx = TaskPlanContext::new(tmp.path().to_str().unwrap());
    ctx.run_store = Arc::new(Mutex::new(HashMap::new()));
    ctx
}

#[tokio::test]
async fn test_serial_execution_end_to_end() {
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = context(&tmp);

    let plan = TaskPlan::new("serial-e2e")
        .add_phase(Phase::new("p1").add_run(RunRef::new("r1", "fast-track")))
        .add_phase(
            Phase::new("p2")
                .depends_on(vec!["p1".to_string()])
                .add_run(RunRef::new("r2", "fast-track")),
        );

    let mut engine = TaskPlanEngine::new(plan, tmp.path().to_str().unwrap(), "do work");
    engine.execute(&ctx, success_executor()).await.unwrap();

    assert_eq!(engine.status, TaskPlanStatus::Completed);
    assert_eq!(engine.phase_states.get("p1").unwrap().status, PhaseStatus::Completed);
    assert_eq!(engine.phase_states.get("p2").unwrap().status, PhaseStatus::Completed);
}

#[tokio::test]
async fn test_parallel_execution_with_join_gate() {
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = context(&tmp);

    let plan = TaskPlan::new("parallel-e2e").add_phase(
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
async fn test_failure_propagation() {
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = context(&tmp);

    let plan = TaskPlan::new("failure-e2e").add_phase(
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
async fn test_cross_run_handoff_resolution() {
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = context(&tmp);

    // Seed the handoff store with output from a previous run.
    let mut handoff = HandoffDocument::new("assistant", "next", "prev", 0);
    handoff.summary = "previous result".to_string();
    ctx.handoff_store
        .save("handoff-e2e", "p1", "r1", &handoff)
        .unwrap();

    let plan = TaskPlan::new("handoff-e2e").add_phase(
        Phase::new("p2").add_run(
            RunRef::new("r2", "fast-track")
                .with_input_from(vec!["p1.r1.handoff.summary".to_string()]),
        ),
    );

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

#[tokio::test]
async fn test_dynamic_registration_and_execution() {
    use auto_forge::relay::task_plan_registry::register_task_plan;

    let atom = r#"
    task_plan(id: "dynamic-e2e", version: 1) {
        phase(name: "p1") {
            run(name: "r1", flow_id: "fast-track") {
                input: "dynamic work"
            }
        }
        phase(name: "p2", depends_on: ["p1"]) {
            run(name: "r2", flow_id: "fast-track") {}
        }
    }
    "#;

    let plan = register_task_plan(atom, None).expect("dynamic registration should succeed");

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = context(&tmp);
    let mut engine = TaskPlanEngine::new(plan, tmp.path().to_str().unwrap(), "initial");
    engine.execute(&ctx, success_executor()).await.unwrap();

    assert_eq!(engine.status, TaskPlanStatus::Completed);
    assert_eq!(engine.phase_states.get("p1").unwrap().status, PhaseStatus::Completed);
    assert_eq!(engine.phase_states.get("p2").unwrap().status, PhaseStatus::Completed);
}
