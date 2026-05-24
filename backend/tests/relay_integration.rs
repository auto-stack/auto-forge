//! Integration test: End-to-end Agents Relay flow
//!
//! Tests the pipeline engine with synthetic handoffs (no LLM calls).

use auto_forge::relay::handoff::{HandoffDocument, TokenUsage, SpecUpdate, WorkProduct, Decision};
use auto_forge::relay::pipeline::{AdvanceResult, GateDecision};
use auto_forge::relay::store::{advance_run, get_run, new_run_store, resolve_gate, start_run, submit_handoff};
use auto_forge::relay::flows::standard_spec_flow;

/// Build a handoff with validation-aware data for each profession.
fn make_handoff(
    run_id: &str,
    from: &str,
    to: &str,
    checkpoint: u64,
    summary: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> HandoffDocument {
    let mut h = HandoffDocument::new(from, to, run_id, checkpoint);
    h.summary = summary.into();
    h.token_usage = TokenUsage {
        step_input: input_tokens,
        step_output: output_tokens,
        cumulative: input_tokens + output_tokens,
        budget_remaining: 100000 - (input_tokens + output_tokens),
    };

    match from {
        "advisor" => {
            h.spec_updates.push(SpecUpdate {
                section_id: "goals".to_string(),
                item_id: None,
                change_type: "modified".to_string(),
                description: "Added goals".to_string(),
            });
        }
        "architect" => {
            h.spec_updates.push(SpecUpdate {
                section_id: "architecture".to_string(),
                item_id: None,
                change_type: "modified".to_string(),
                description: "Updated architecture".to_string(),
            });
        }
        "planner" => {
            h.spec_updates.push(SpecUpdate {
                section_id: "plans".to_string(),
                item_id: None,
                change_type: "modified".to_string(),
                description: "Updated plans".to_string(),
            });
        }
        "tester" => {
            h.work_product.push(WorkProduct {
                path: "src/lib.rs".to_string(),
                description: "Test files".to_string(),
                lines: None,
            });
        }
        "coder" => {
            h.work_product.push(WorkProduct {
                path: "src/main.rs".to_string(),
                description: "Code files".to_string(),
                lines: None,
            });
        }
        "reviewer" => {
            h.decisions.push(Decision {
                id: "D1".to_string(),
                title: "Approved".to_string(),
                status: "made".to_string(),
                rationale: String::new(),
            });
        }
        "documenter" => {
            h.work_product.push(WorkProduct {
                path: "docs/report.md".to_string(),
                description: "Report".to_string(),
                lines: None,
            });
        }
        _ => {}
    }
    h
}

/// Drive a step and any necessary gates/loops until we reach a new profession.
fn drive_until_next(
    store: &auto_forge::relay::store::RunStore,
    run_id: &str,
    expected_profession: &str,
) -> AdvanceResult {
    for _ in 0..20 {
        let state = get_run(store, run_id).unwrap();
        if state.status == "Completed" {
            return AdvanceResult::Completed;
        }
        if state.waiting_for_gate.is_some() {
            resolve_gate(store, run_id, GateDecision::Approve);
            continue;
        }
        let step_idx = state.current_step;
        let prof = state.steps[step_idx.min(state.steps.len() - 1)].profession_id.clone();
        let step_id = state.steps[step_idx.min(state.steps.len() - 1)].id.clone();

        if prof == expected_profession {
            let r = advance_run(store, run_id).unwrap();
            return r;
        }

        // Auto-submit handoff for the current step
        let h = make_handoff(run_id, &prof, "next", step_idx as u64, "Auto step.", 500, 300);
        let r = submit_handoff(store, run_id, h).unwrap();
        if matches!(r, AdvanceResult::Completed) {
            return r;
        }
    }
    panic!("drive_until_next hit safety limit");
}

#[test]
fn test_end_to_end_standard_flow_with_mock_handoffs() {
    let store = new_run_store();
    let flow = standard_spec_flow();

    let run_id = "e2e-standard-1";
    start_run(&store, flow, run_id).expect("start run");

    // Verify initial state
    let state = get_run(&store, run_id).unwrap();
    assert_eq!(state.total_steps, 9);
    assert_eq!(state.status, "Idle");

    // Drive each step
    let professions = vec![
        "assistant", "advisor", "architect", "planner",
        "tester", "coder", "tester", "reviewer", "documenter"
    ];

    for (i, expected) in professions.iter().enumerate() {
        let r = drive_until_next(&store, run_id, expected);
        if i == professions.len() - 1 {
            // Last step (documenter) — submit handoff to complete
            let h = make_handoff(run_id, "documenter", "done", 8, "Done.", 500, 300);
            let r = submit_handoff(&store, run_id, h).unwrap();
            assert_eq!(r, AdvanceResult::Completed);
        }
    }

    // Final state
    let final_state = get_run(&store, run_id).unwrap();
    assert_eq!(final_state.status, "Completed");
    assert!(
        final_state.current_step >= 9,
        "current_step should be at least 9, got {}",
        final_state.current_step
    );

    // All steps completed
    for step in &final_state.steps {
        assert_eq!(step.status, "completed", "step {} should be completed", step.id);
    }

    // Savings vs parallel should be positive
    assert!(final_state.savings > 0);
    assert!(final_state.savings_ratio > 0.0);
}

#[test]
fn test_end_to_end_reject_gate_routes_back() {
    let store = new_run_store();
    let flow = standard_spec_flow();

    let run_id = "e2e-reject-1";
    start_run(&store, flow, run_id).unwrap();

    // Intake → Discover gate
    advance_run(&store, run_id);
    let h = HandoffDocument::new("assistant", "advisor", run_id, 0);
    submit_handoff(&store, run_id, h);

    // Reject discover gate
    let r = resolve_gate(&store, run_id, GateDecision::Reject {
        feedback: "Need more detail on error handling".into(),
    }).unwrap();

    // Should re-enter discover step
    assert!(matches!(r, AdvanceResult::ExecuteStep { ref profession_id, .. } if profession_id == "advisor"));

    let state = get_run(&store, run_id).unwrap();
    assert_eq!(state.current_step, 1); // still on discover
    assert_eq!(state.steps[1].status, "running");
}

#[test]
fn test_checkpoint_during_flow() {
    use auto_forge::relay::checkpoint::Checkpoint;

    let store = new_run_store();
    let flow = standard_spec_flow();

    let run_id = "e2e-checkpoint-1";
    start_run(&store, flow.clone(), run_id).unwrap();

    // Drive through first 4 steps
    for expected in &["assistant", "advisor", "architect", "planner"] {
        drive_until_next(&store, run_id, expected);
    }

    // At this point 4 steps should be completed (intake, discover, design, plan)
    let state = get_run(&store, run_id).unwrap();
    assert!(
        state.current_step >= 3,
        "Should be at step 3+ after planner, got {}",
        state.current_step
    );

    // Create checkpoint
    let map = store.lock().unwrap();
    let entry = map.get(run_id).unwrap();
    let checkpoint = Checkpoint::create(&entry.engine, std::path::Path::new("."), None).unwrap();
    drop(map);

    assert_eq!(checkpoint.run_id, run_id);
    let checkpoint_step = checkpoint.current_step;
    let checkpoint_history_len = checkpoint.step_history.len();
    assert!(
        checkpoint_step >= 3,
        "Checkpoint should be at step 3+, got {}",
        checkpoint_step
    );
    assert_eq!(checkpoint_history_len, checkpoint_step);

    // Resume from checkpoint
    let mut resumed = auto_forge::relay::pipeline::PipelineEngine::from_checkpoint(checkpoint, flow).unwrap();
    assert_eq!(resumed.current_step, checkpoint_step);
    assert_eq!(resumed.step_history.len(), checkpoint_history_len);

    // Can continue — should get a valid next step
    let r = resumed.advance();
    assert!(
        matches!(r, AdvanceResult::ExecuteStep { .. }),
        "Should advance to a valid step, got {:?}", r
    );
}
