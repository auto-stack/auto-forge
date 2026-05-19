//! Relay Background Driver
//!
//! Auto-executes relay pipeline steps by running AgentTurn for each ExecuteStep,
//! submitting handoffs, and polling for gate resolution. This bridges the gap
//! between the relay state machine (PipelineEngine) and actual LLM execution.

use crate::forge::tools::{set_tool_context, ToolRegistry};
use crate::provider::{ChatMessage, ClaudeProviderState};
use crate::relay::agent::AgentInstance;
use crate::relay::pipeline::{AdvanceResult, PipelineStatus};
use crate::relay::store::{RunStore, advance_run, submit_handoff};
use crate::relay::api::RunEventBroadcast;
use tokio::sync::broadcast;
use serde_json::json;

/// Drive a relay run to completion in a background task.
///
/// Loop: advance_run → AgentTurn::run() → submit_handoff → repeat.
/// At human gates, sleeps and polls until the gate is resolved via RelayView.
pub async fn drive_run(
    run_id: String,
    run_store: RunStore,
    event_tx: broadcast::Sender<RunEventBroadcast>,
    provider: ClaudeProviderState,
    initial_task: String,
    project_path: String,
) {
    tracing::info!("Relay driver started for run {}", run_id);

    // Set project context so specs tools work during relay execution
    set_tool_context(&project_path, &run_id);

    loop {
        let advance_result = advance_run(&run_store, &run_id);

        match advance_result {
            Some(AdvanceResult::ExecuteStep {
                step_id,
                profession_id,
                agent_config_id,
            }) => {
                // Notify listeners that a step has started
                let _ = event_tx.send(RunEventBroadcast {
                    run_id: run_id.clone(),
                    event_type: "step_started".to_string(),
                    payload: Some(json!({
                        "step_id": &step_id,
                        "profession_id": &profession_id,
                    })),
                });

                // Build agent instance for this profession
                let agent = match build_agent(&profession_id, agent_config_id.as_deref()) {
                    Some(a) => a,
                    None => {
                        tracing::error!(
                            "Run {}: failed to spawn agent for profession {}",
                            run_id, profession_id
                        );
                        break;
                    }
                };

                // Gather context for this step
                let messages = build_step_messages(
                    &run_store,
                    &run_id,
                    &step_id,
                    &profession_id,
                    &initial_task,
                );

                // Run the agent turn
                let mut turn = crate::relay::turn::AgentTurn::new(
                    agent,
                    ToolRegistry::new(),
                    messages,
                );
                turn.max_turns = turn.agent.profession.max_turns;

                // Forward turn events to the broadcast channel for live session log
                // and persist them to the run store for replay after restart
                let (turn_tx, mut turn_rx) =
                    tokio::sync::mpsc::unbounded_channel::<crate::relay::turn::TurnEvent>();
                let event_tx_fwd = event_tx.clone();
                let run_id_fwd = run_id.clone();
                let profession_id_fwd = profession_id.clone();
                let run_store_fwd = run_store.clone();
                tokio::spawn(async move {
                    let mut text_buffer = String::new();
                    let mut flush_interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
                    flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                    let flush_text = |buf: &mut String, tx: &broadcast::Sender<RunEventBroadcast>, store: &RunStore| {
                        if !buf.is_empty() {
                            let text = buf.clone();
                            let _ = tx.send(RunEventBroadcast {
                                run_id: run_id_fwd.clone(),
                                event_type: "turn_delta".to_string(),
                                payload: Some(json!({
                                    "profession_id": profession_id_fwd.clone(),
                                    "text": &text,
                                })),
                            });
                            if let Ok(mut map) = store.lock() {
                                if let Some(entry) = map.get_mut(&run_id_fwd) {
                                    entry.events.push(crate::relay::store::RunEvent::TurnDelta {
                                        profession_id: profession_id_fwd.clone(),
                                        text,
                                    });
                                    crate::relay::store::save_run(entry);
                                }
                            }
                            buf.clear();
                        }
                    };

                    loop {
                        tokio::select! {
                            _ = flush_interval.tick() => {
                                flush_text(&mut text_buffer, &event_tx_fwd, &run_store_fwd);
                            }
                            maybe_event = turn_rx.recv() => {
                                match maybe_event {
                                    Some(crate::relay::turn::TurnEvent::TextDelta { text }) => {
                                        text_buffer.push_str(&text);
                                    }
                                    Some(crate::relay::turn::TurnEvent::ToolCall { id, name, arguments }) => {
                                        flush_text(&mut text_buffer, &event_tx_fwd, &run_store_fwd);
                                        let _ = event_tx_fwd.send(RunEventBroadcast {
                                            run_id: run_id_fwd.clone(),
                                            event_type: "turn_tool_call".to_string(),
                                            payload: Some(json!({
                                                "profession_id": profession_id_fwd.clone(),
                                                "tool_id": &id,
                                                "tool_name": &name,
                                                "arguments": &arguments,
                                            })),
                                        });
                                        if let Ok(mut map) = run_store_fwd.lock() {
                                            if let Some(entry) = map.get_mut(&run_id_fwd) {
                                                entry.events.push(crate::relay::store::RunEvent::TurnToolCall {
                                                    profession_id: profession_id_fwd.clone(),
                                                    tool_id: id,
                                                    tool_name: name,
                                                    arguments,
                                                });
                                                crate::relay::store::save_run(entry);
                                            }
                                        }
                                    }
                                    Some(crate::relay::turn::TurnEvent::ToolResult { id, result }) => {
                                        flush_text(&mut text_buffer, &event_tx_fwd, &run_store_fwd);
                                        let _ = event_tx_fwd.send(RunEventBroadcast {
                                            run_id: run_id_fwd.clone(),
                                            event_type: "turn_tool_result".to_string(),
                                            payload: Some(json!({
                                                "profession_id": profession_id_fwd.clone(),
                                                "tool_id": &id,
                                                "result": &result,
                                            })),
                                        });
                                        if let Ok(mut map) = run_store_fwd.lock() {
                                            if let Some(entry) = map.get_mut(&run_id_fwd) {
                                                entry.events.push(crate::relay::store::RunEvent::TurnToolResult {
                                                    profession_id: profession_id_fwd.clone(),
                                                    tool_id: id,
                                                    result: result.clone(),
                                                });
                                                crate::relay::store::save_run(entry);
                                            }
                                        }
                                    }
                                    Some(crate::relay::turn::TurnEvent::Complete) => {
                                        flush_text(&mut text_buffer, &event_tx_fwd, &run_store_fwd);
                                        let _ = event_tx_fwd.send(RunEventBroadcast {
                                            run_id: run_id_fwd.clone(),
                                            event_type: "turn_complete".to_string(),
                                            payload: Some(json!({
                                                "profession_id": profession_id_fwd.clone(),
                                            })),
                                        });
                                        if let Ok(mut map) = run_store_fwd.lock() {
                                            if let Some(entry) = map.get_mut(&run_id_fwd) {
                                                entry.events.push(crate::relay::store::RunEvent::TurnComplete {
                                                    profession_id: profession_id_fwd.clone(),
                                                });
                                                crate::relay::store::save_run(entry);
                                            }
                                        }
                                    }
                                    Some(crate::relay::turn::TurnEvent::Error { message }) => {
                                        flush_text(&mut text_buffer, &event_tx_fwd, &run_store_fwd);
                                        let _ = event_tx_fwd.send(RunEventBroadcast {
                                            run_id: run_id_fwd.clone(),
                                            event_type: "turn_error".to_string(),
                                            payload: Some(json!({
                                                "profession_id": profession_id_fwd.clone(),
                                                "message": &message,
                                            })),
                                        });
                                        if let Ok(mut map) = run_store_fwd.lock() {
                                            if let Some(entry) = map.get_mut(&run_id_fwd) {
                                                entry.events.push(crate::relay::store::RunEvent::TurnError {
                                                    profession_id: profession_id_fwd.clone(),
                                                    message: message.clone(),
                                                });
                                                crate::relay::store::save_run(entry);
                                            }
                                        }
                                    }
                                    Some(crate::relay::turn::TurnEvent::BudgetWarning { remaining }) => {
                                        flush_text(&mut text_buffer, &event_tx_fwd, &run_store_fwd);
                                        let _ = event_tx_fwd.send(RunEventBroadcast {
                                            run_id: run_id_fwd.clone(),
                                            event_type: "turn_budget_warning".to_string(),
                                            payload: Some(json!({
                                                "profession_id": profession_id_fwd.clone(),
                                                "remaining": remaining,
                                            })),
                                        });
                                        if let Ok(mut map) = run_store_fwd.lock() {
                                            if let Some(entry) = map.get_mut(&run_id_fwd) {
                                                entry.events.push(crate::relay::store::RunEvent::TurnBudgetWarning {
                                                    profession_id: profession_id_fwd.clone(),
                                                    remaining,
                                                });
                                                crate::relay::store::save_run(entry);
                                            }
                                        }
                                    }
                                    Some(crate::relay::turn::TurnEvent::BudgetExceeded) => {
                                        flush_text(&mut text_buffer, &event_tx_fwd, &run_store_fwd);
                                        let _ = event_tx_fwd.send(RunEventBroadcast {
                                            run_id: run_id_fwd.clone(),
                                            event_type: "turn_budget_exceeded".to_string(),
                                            payload: Some(json!({
                                                "profession_id": profession_id_fwd.clone(),
                                            })),
                                        });
                                        if let Ok(mut map) = run_store_fwd.lock() {
                                            if let Some(entry) = map.get_mut(&run_id_fwd) {
                                                entry.events.push(crate::relay::store::RunEvent::TurnBudgetExceeded {
                                                    profession_id: profession_id_fwd.clone(),
                                                });
                                                crate::relay::store::save_run(entry);
                                            }
                                        }
                                    }
                                    None => {
                                        flush_text(&mut text_buffer, &event_tx_fwd, &run_store_fwd);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });

                let turn_result = turn.run(provider.clone(), turn_tx).await;

                // Build handoff document from turn result
                let to_profession = guess_next_profession(&run_store, &run_id)
                    .unwrap_or_else(|| "next".to_string());
                let handoff = turn.to_handoff(&turn_result, &to_profession, &run_id, 0);

                // Submit handoff — pipeline engine advances internally
                let next_result = submit_handoff(&run_store, &run_id, handoff.clone());

                // Broadcast step completion with token usage
                let _ = event_tx.send(RunEventBroadcast {
                    run_id: run_id.clone(),
                    event_type: "step_completed".to_string(),
                    payload: Some(json!({
                        "step_id": &step_id,
                        "profession_id": &profession_id,
                        "summary": &handoff.summary,
                        "tokens_used": handoff.token_usage.step_input + handoff.token_usage.step_output,
                    })),
                });

                match next_result {
                    Some(AdvanceResult::ExecuteStep { .. }) => {
                        // Loop continues — top of loop calls advance_run again
                        continue;
                    }
                    Some(AdvanceResult::WaitForHuman { .. }) => {
                        wait_for_gate_resolution(&run_store, &run_id, &event_tx).await;
                        continue;
                    }
                    Some(AdvanceResult::Completed) => {
                        let _ = event_tx.send(RunEventBroadcast {
                            run_id: run_id.clone(),
                            event_type: "run_completed".to_string(),
                            payload: None,
                        });
                        tracing::info!("Relay driver completed run {}", run_id);
                        break;
                    }
                    Some(AdvanceResult::Failed { error }) => {
                        let _ = event_tx.send(RunEventBroadcast {
                            run_id: run_id.clone(),
                            event_type: format!("run_failed: {}", error),
                            payload: None,
                        });
                        tracing::error!("Relay driver failed run {}: {}", run_id, error);
                        break;
                    }
                    None => break,
                }
            }
            Some(AdvanceResult::WaitForHuman { .. }) => {
                wait_for_gate_resolution(&run_store, &run_id, &event_tx).await;
                continue;
            }
            Some(AdvanceResult::Completed) => {
                let _ = event_tx.send(RunEventBroadcast {
                    run_id: run_id.clone(),
                    event_type: "run_completed".to_string(),
                    payload: None,
                });
                tracing::info!("Relay driver completed run {}", run_id);
                break;
            }
            Some(AdvanceResult::Failed { error }) => {
                let _ = event_tx.send(RunEventBroadcast {
                    run_id: run_id.clone(),
                    event_type: format!("run_failed: {}", error),
                    payload: None,
                });
                tracing::error!("Relay driver failed run {}: {}", run_id, error);
                break;
            }
            None => {
                tracing::warn!("Relay driver got None for run {}", run_id);
                break;
            }
        }
    }
}

/// Build an AgentInstance for the given profession.
fn build_agent(
    profession_id: &str,
    agent_config_id: Option<&str>,
) -> Option<AgentInstance> {
    let registry = crate::relay::RelayRegistry::new();

    if let Some(config_id) = agent_config_id {
        // Look up specific agent config by ID
        let config = registry
            .agent_configs
            .iter()
            .find(|c| c.id == config_id)?;
        registry.spawn_agent_from_config(config)
    } else {
        // Use default agent config for this profession
        registry
            .default_agent_for(profession_id)
            .and_then(|cfg| registry.spawn_agent_from_config(cfg))
    }
}

/// Build the initial ChatMessages for a pipeline step.
fn build_step_messages(
    run_store: &RunStore,
    run_id: &str,
    step_id: &str,
    profession_id: &str,
    initial_task: &str,
) -> Vec<ChatMessage> {
    let mut context = String::new();

    context.push_str(&format!("## Task\n\n{}\n\n", initial_task));

    // Include previous handoff if available
    let previous_handoff = {
        let map = run_store.lock().unwrap();
        map.get(run_id)
            .and_then(|e| e.engine.step_history.last())
            .and_then(|r| r.handoff.clone())
    };

    if let Some(ref handoff) = previous_handoff {
        context.push_str("## Previous Agent's Handoff\n\n");
        context.push_str(&handoff.render());
        context.push_str("\n\n");
    }

    // Include gate feedback if this step was rejected before
    let feedback = {
        let map = run_store.lock().unwrap();
        map.get(run_id)
            .and_then(|e| e.engine.gate_feedback.get(step_id))
            .cloned()
            .unwrap_or_default()
    };

    if !feedback.is_empty() {
        context.push_str("## Feedback from Previous Attempt\n\n");
        for fb in &feedback {
            context.push_str(&format!("- {}\n", fb));
        }
        context.push_str("\n");
    }

    context.push_str(&format!(
        "You are step '{}' ({}) in a relay pipeline. Do your work using available tools. \
         When you are finished, stop making tool calls and the pipeline will advance automatically.",
        step_id, profession_id
    ));

    vec![ChatMessage::user(&context)]
}

/// Guess the next profession from the flow spec for handoff metadata.
fn guess_next_profession(run_store: &RunStore, run_id: &str) -> Option<String> {
    let map = run_store.lock().unwrap();
    let entry = map.get(run_id)?;
    let engine = &entry.engine;

    // current_step has already been advanced by submit_handoff, so it points
    // to the *next* step. Use it directly.
    engine
        .flow
        .steps
        .get(engine.current_step)
        .map(|s| s.profession_id.clone())
}

/// Poll until the run is no longer waiting for a human gate.
async fn wait_for_gate_resolution(
    run_store: &RunStore,
    run_id: &str,
    event_tx: &broadcast::Sender<RunEventBroadcast>,
) {
    let _ = event_tx.send(RunEventBroadcast {
        run_id: run_id.to_string(),
        event_type: "gate_waiting".to_string(),
        payload: None,
    });

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let is_waiting = {
            let map = run_store.lock().unwrap();
            match map.get(run_id) {
                Some(entry) => matches!(
                    entry.engine.status,
                    PipelineStatus::WaitingForHuman { .. }
                ),
                None => false,
            }
        };

        if !is_waiting {
            break;
        }
    }
}
