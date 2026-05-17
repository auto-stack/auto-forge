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

                // Drain turn events so the channel doesn't back up
                let (turn_tx, mut turn_rx) =
                    tokio::sync::mpsc::unbounded_channel::<crate::relay::turn::TurnEvent>();
                tokio::spawn(async move {
                    while let Some(_event) = turn_rx.recv().await {}
                });

                let turn_result = turn.run(&*provider, turn_tx).await;

                // Build handoff document from turn result
                let to_profession = guess_next_profession(&run_store, &run_id)
                    .unwrap_or_else(|| "next".to_string());
                let handoff = turn.to_handoff(&turn_result, &to_profession, &run_id, 0);

                // Submit handoff — pipeline engine advances internally
                let next_result = submit_handoff(&run_store, &run_id, handoff);

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
                        });
                        tracing::info!("Relay driver completed run {}", run_id);
                        break;
                    }
                    Some(AdvanceResult::Failed { error }) => {
                        let _ = event_tx.send(RunEventBroadcast {
                            run_id: run_id.clone(),
                            event_type: format!("run_failed: {}", error),
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
                });
                tracing::info!("Relay driver completed run {}", run_id);
                break;
            }
            Some(AdvanceResult::Failed { error }) => {
                let _ = event_tx.send(RunEventBroadcast {
                    run_id: run_id.clone(),
                    event_type: format!("run_failed: {}", error),
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
