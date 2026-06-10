//! Integration tests for the write_goals tool and goal-discovery flow.

use auto_forge::forge::tools::{ToolRegistry, Tool};
use auto_forge::relay::flows::FlowRegistry;

fn goal_discovery_flow() -> auto_forge::relay::flow::FlowSpec {
    FlowRegistry::load_builtins_only()
        .get("goal-discovery")
        .expect("goal-discovery built-in flow must exist")
}
use auto_forge::relay::flow::{FlowStep, GateType, StepValidator, ToolGuard};
use auto_forge::relay::pipeline::{AdvanceResult, PipelineEngine};
use auto_forge::relay::handoff::HandoffDocument;
use auto_forge::relay::store::{new_run_store, start_run, get_run, advance_run, submit_handoff};

// ─────────────────────────────────────────────────────────────────────────────
// Tool Registry Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_write_goals_tool_is_registered() {
    let registry = ToolRegistry::new();
    let tool = registry.get("write_goals");
    assert!(tool.is_some(), "write_goals tool should be registered");
    assert_eq!(tool.unwrap().name(), "write_goals");
}

#[test]
fn test_write_goals_tool_schema_has_single_content_param() {
    let registry = ToolRegistry::new();
    let tool = registry.get("write_goals").unwrap();
    let schema = tool.input_schema();
    let props = schema.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("content"), "schema should have 'content' property");
    let required = schema.get("required").unwrap().as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("content")));
}

#[test]
fn test_write_goals_tool_executes_with_valid_content() {
    // Set up project context so write_goals can find the specs store
    auto_forge::forge::tools::set_tool_context("d:/autostack/auto-forge", "test-session");

    let registry = ToolRegistry::new();
    let tool = registry.get("write_goals").unwrap();

    let args = serde_json::json!({
        "content": "## G99 Test Goal\n**Status:** proposed\n**Tags:** stack:test, module:test\n**Depends on:** none\n\n- [ ] This is a test goal for integration testing"
    });

    let result = tool.execute(args);
    assert!(result.is_ok(), "write_goals should succeed: {:?}", result);
    let msg = result.unwrap();
    assert!(msg.contains("goals"), "Result should mention goals section: {}", msg);
}

#[test]
fn test_write_goals_tool_fails_without_content() {
    auto_forge::forge::tools::set_tool_context("d:/autostack/auto-forge", "test-session");

    let registry = ToolRegistry::new();
    let tool = registry.get("write_goals").unwrap();

    let args = serde_json::json!({});
    let result = tool.execute(args);
    assert!(result.is_err(), "write_goals should fail without content");
}

// ─────────────────────────────────────────────────────────────────────────────
// Flow Configuration Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_goal_discovery_flow_has_discover_step() {
    let flow = goal_discovery_flow();
    let step = flow.get_step("discover");
    assert!(step.is_some());
    assert_eq!(step.unwrap().profession_id, "advisor");
}

#[test]
fn test_goal_discovery_flow_discover_step_has_tool_guard() {
    let flow = goal_discovery_flow();
    let step = flow.get_step("discover").unwrap();
    let guard = step.tool_guard.as_ref().expect("discover should have a tool guard");
    assert_eq!(guard.required_first, vec!["write_specs", "write_goals"]);
    assert!(guard.always_allowed.contains(&"list_specs".to_string()));
    assert!(guard.always_allowed.contains(&"read_specs".to_string()));
}

#[test]
fn test_goal_discovery_flow_discover_step_has_validator() {
    let flow = goal_discovery_flow();
    let step = flow.get_step("discover").unwrap();
    assert!(!step.validators.is_empty(), "discover should have validators");
}

#[test]
fn test_flow_registry_contains_goal_discovery() {
    // Directly create a registry with a valid data dir
    let registry = auto_forge::relay::flows::FlowRegistry::new(
        std::path::Path::new("d:/autostack/auto-forge")
    );
    let flow = registry.get("goal-discovery");
    assert!(flow.is_some(), "goal-discovery should be in the registry");
    assert_eq!(flow.unwrap().id, "goal-discovery");
}

// ─────────────────────────────────────────────────────────────────────────────
// Pipeline + Store Integration: Mock Handoffs
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_goal_discovery_flow_completes_with_mock_handoff() {
    let store = new_run_store();
    let flow = goal_discovery_flow();

    let run_id = "test-goal-discovery-1";
    start_run(&store, flow, run_id).expect("start run");

    // Advance to discover step
    let r1 = advance_run(&store, run_id).unwrap();
    assert!(
        matches!(r1, AdvanceResult::ExecuteStep { profession_id, .. } if profession_id == "advisor"),
        "First step should be advisor"
    );

    // Submit a synthetic handoff with spec_updates (simulating write_goals success)
    let mut h1 = HandoffDocument::new("advisor", "done", run_id, 0);
    h1.spec_updates.push(auto_forge::relay::handoff::SpecUpdate {
        section_id: "goals".to_string(),
        item_id: Some("G99".to_string()),
        change_type: "modified".to_string(),
        description: "Added G99 via write_goals".to_string(),
    });
    h1.token_usage = auto_forge::relay::handoff::TokenUsage {
        step_input: 1000,
        step_output: 500,
        cumulative: 1500,
        budget_remaining: 98500,
    };

    let r1b = submit_handoff(&store, run_id, h1).unwrap();
    assert_eq!(r1b, AdvanceResult::Completed, "Goal discovery is a single-step flow");

    // Verify final state
    let final_state = get_run(&store, run_id).unwrap();
    assert_eq!(final_state.status, "completed");
    assert_eq!(final_state.current_step, 1);
    assert_eq!(final_state.step_history.len(), 1);
    assert_eq!(final_state.cumulative_tokens, 1500);
}

#[test]
fn test_goal_discovery_flow_fails_validation_without_spec_updates() {
    let store = new_run_store();
    let flow = goal_discovery_flow();

    let run_id = "test-goal-discovery-fail";
    start_run(&store, flow, run_id).expect("start run");

    // Advance to discover step
    advance_run(&store, run_id).unwrap();

    // Submit handoff WITHOUT spec_updates — should fail validation
    let h1 = HandoffDocument::new("advisor", "done", run_id, 0);
    let r1b = submit_handoff(&store, run_id, h1).unwrap();

    // With retry logic, it will retry up to 3 times then fail
    // The exact behavior depends on the pipeline implementation
    // We just check that it's not immediately Completed
    let state = get_run(&store, run_id).unwrap();
    assert_ne!(state.status, "completed", "Should not complete without spec updates");
}

// ─────────────────────────────────────────────────────────────────────────────
// StepValidator Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_step_validator_spec_updates_non_empty_passes() {
    let validator = StepValidator::SpecUpdatesNonEmpty {
        sections: vec!["goals".to_string()],
    };

    let mut handoff = HandoffDocument::new("advisor", "architect", "test", 0);
    handoff.spec_updates.push(auto_forge::relay::handoff::SpecUpdate {
        section_id: "goals".to_string(),
        item_id: None,
        change_type: "modified".to_string(),
        description: "test".to_string(),
    });

    assert!(validator.check(&handoff).is_none(), "Should pass when spec_updates has goals");
}

#[test]
fn test_step_validator_spec_updates_non_empty_fails() {
    let validator = StepValidator::SpecUpdatesNonEmpty {
        sections: vec!["goals".to_string()],
    };

    let handoff = HandoffDocument::new("advisor", "architect", "test", 0);
    assert!(validator.check(&handoff).is_some(), "Should fail when spec_updates is empty");
}

#[test]
fn test_step_validator_work_product_has_extensions_passes() {
    let validator = StepValidator::WorkProductHasExtensions {
        exts: vec![".ad".to_string()],
    };

    let mut handoff = HandoffDocument::new("advisor", "architect", "test", 0);
    handoff.work_product.push(auto_forge::relay::handoff::WorkProduct {
        path: "specs/goals.ad".to_string(),
        description: "updated".to_string(),
        lines: None,
    });

    assert!(validator.check(&handoff).is_none(), "Should pass when work_product has .ad file");
}

#[test]
fn test_step_validator_any_composite_passes_if_one_passes() {
    let validator = StepValidator::Any(vec![
        StepValidator::SpecUpdatesNonEmpty { sections: vec!["goals".to_string()] },
        StepValidator::WorkProductHasExtensions { exts: vec![".ad".to_string()] },
    ]);

    // Only spec_updates — should pass
    let mut handoff = HandoffDocument::new("advisor", "architect", "test", 0);
    handoff.spec_updates.push(auto_forge::relay::handoff::SpecUpdate {
        section_id: "goals".to_string(),
        item_id: None,
        change_type: "modified".to_string(),
        description: "test".to_string(),
    });

    assert!(validator.check(&handoff).is_none(), "Any composite should pass if one child passes");
}

#[test]
fn test_tool_guard_check_required_first() {
    let guard = ToolGuard {
        required_first: vec!["write_goals".to_string()],
        always_allowed: vec!["list_specs".to_string()],
        forbidden: vec!["dispatch".to_string()],
        unlocks: std::collections::HashMap::new(),
    };

    // list_specs is always_allowed — should pass even before required_first
    assert!(guard.check("list_specs", &[]).is_ok());

    // write_file is not allowed before write_goals
    assert!(guard.check("write_file", &[]).is_err());

    // After write_goals is called, write_file should be allowed
    assert!(guard.check("write_file", &["write_goals".to_string()]).is_ok());

    // dispatch is forbidden
    assert!(guard.check("dispatch", &["write_goals".to_string()]).is_err());
}
