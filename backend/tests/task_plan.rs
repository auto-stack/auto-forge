//! Integration tests for TaskPlan parsing, registry, and dynamic registration.

use auto_forge::relay::handoff::HandoffDocument;
use auto_forge::relay::task_plan_engine::{
    RunExecutionResult, RunRequest, TaskPlanContext, TaskPlanEngine, TaskPlanStatus,
};
use auto_forge::relay::task_plan_registry::{register_task_plan, TaskPlanRegistry};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn empty_context(tmp: &tempfile::TempDir) -> TaskPlanContext {
    let mut ctx = TaskPlanContext::new(tmp.path().to_str().unwrap());
    ctx.run_store = Arc::new(Mutex::new(HashMap::new()));
    ctx
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

#[test]
fn test_parse_all_builtin_task_plans() {
    let registry = TaskPlanRegistry::load_builtins_only();
    let plans = registry.list();
    assert!(!plans.is_empty(), "at least one built-in TaskPlan should load");

    for summary in &plans {
        let plan = registry.get(&summary.id);
        assert!(plan.is_some(), "built-in plan '{}' should be retrievable", summary.id);
        let plan = plan.unwrap();
        assert_eq!(plan.phases.len(), summary.phase_count);
    }
}

#[test]
fn test_builtin_deferred_decompose_structure() {
    let registry = TaskPlanRegistry::load_builtins_only();
    let plan = registry.get("deferred-decompose").expect("deferred-decompose must exist");
    assert_eq!(plan.id, "deferred-decompose");
    assert!(!plan.phases.is_empty());
}

#[test]
fn test_register_task_plan_and_execute() {
    let atom = r#"
    task_plan(id: "registered-test", version: 1) {
        phase(name: "p1") {
            run(name: "r1", flow_id: "fast-track") {
                input: "hello"
            }
        }
    }
    "#;

    let plan = register_task_plan(atom, None).expect("registration should succeed");
    assert_eq!(plan.id, "registered-test");

    let fetched = auto_forge::relay::task_plan_registry::get_task_plan("registered-test");
    assert!(fetched.is_some());

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = empty_context(&tmp);
    let mut engine = TaskPlanEngine::new(plan, tmp.path().to_str().unwrap(), "initial");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        engine.execute(&ctx, success_executor()).await.unwrap();
        assert_eq!(engine.status, TaskPlanStatus::Completed);
    });
}

#[test]
fn test_register_task_plan_rejects_unknown_flow() {
    let atom = r#"
    task_plan(id: "bad-flow", version: 1) {
        phase(name: "p1") {
            run(name: "r1", flow_id: "no-such-flow") {}
        }
    }
    "#;

    let err = register_task_plan(atom, None).unwrap_err();
    assert!(err.contains("no-such-flow"));
}

#[test]
fn test_register_task_plan_persists_to_disk() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join(".autoforge").join("task_plans").join("disk-test.atom");

    let atom = r#"
    task_plan(id: "disk-test", version: 1) {
        phase(name: "p1") {
            run(name: "r1", flow_id: "fast-track") {}
        }
    }
    "#;

    register_task_plan(atom, Some(&path)).expect("registration with file path should succeed");
    assert!(path.is_file());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("disk-test"));
}
