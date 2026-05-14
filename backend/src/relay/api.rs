//! Relay HTTP API
//!
//! Axum handlers for the Agents Relay module.
//! Uses a global in-memory store for simplicity.

use crate::relay::flow::FlowSpec;
use crate::relay::handoff::HandoffDocument;
use crate::relay::pipeline::GateDecision;
use crate::relay::profession::ProfessionRegistry;
use crate::relay::store::{
    advance_run, get_run, list_runs, new_run_store, resolve_gate, start_run, submit_handoff,
    RunState, RunStore, RunSummary,
};
use crate::relay::config::{self, AgentConfig, ApiSource, ConnectionTestResult};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use std::convert::Infallible;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

// Global in-memory run store
static RUN_STORE: LazyLock<RunStore> = LazyLock::new(new_run_store);

// Global event broadcast for SSE
static EVENT_TX: LazyLock<broadcast::Sender<RunEventBroadcast>> = LazyLock::new(|| {
    let (tx, _rx) = broadcast::channel(256);
    tx
});

#[derive(Clone, Debug)]
pub struct RunEventBroadcast {
    pub run_id: String,
    pub event_type: String,
}

// -------------------------------------------------------------------------
// DTOs
// -------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct StartRunRequest {
    pub run_id: Option<String>,
    pub flow_id: String,
    pub steps: Vec<FlowStepDto>,
}

#[derive(serde::Deserialize)]
pub struct FlowStepDto {
    pub id: String,
    pub profession_id: String,
    #[serde(default)]
    pub agent_config_id: Option<String>,
    #[serde(default)]
    pub gate: String,
}

#[derive(serde::Serialize)]
pub struct StartRunResponse {
    pub run_id: String,
    pub state: RunState,
}

#[derive(serde::Deserialize)]
pub struct GateRequest {
    pub decision: String,
    #[serde(default)]
    pub feedback: Option<String>,
    #[serde(default)]
    pub changes: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct HandoffRequest {
    pub handoff: HandoffDocument,
}

#[derive(serde::Serialize)]
pub struct ProfessionsResponse {
    pub professions: Vec<ProfessionDto>,
}

#[derive(serde::Serialize)]
pub struct ProfessionDto {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub owned_sections: Vec<String>,
    pub allowed_tools: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct SoulsResponse {
    pub souls: Vec<SoulDto>,
}

#[derive(serde::Serialize)]
pub struct SoulDto {
    pub id: String,
    pub name: String,
}

// -------------------------------------------------------------------------
// Handlers
// -------------------------------------------------------------------------

pub async fn list_professions() -> Json<ProfessionsResponse> {
    let professions = ProfessionRegistry::new().list().into_iter().map(|p| ProfessionDto {
        id: p.id.clone(),
        name: p.name.clone(),
        phase: p.phase.as_str().to_string(),
        owned_sections: p.owned_sections.iter().map(|s| s.as_str().to_string()).collect(),
        allowed_tools: p.allowed_tools.clone(),
    }).collect();

    Json(ProfessionsResponse { professions })
}

pub async fn list_souls() -> Json<SoulsResponse> {
    let souls = vec![
        SoulDto { id: "assistant".into(), name: "Assistant".into() },
        SoulDto { id: "advisor".into(), name: "Advisor".into() },
        SoulDto { id: "planner".into(), name: "Planner".into() },
        SoulDto { id: "architect".into(), name: "Architect".into() },
        SoulDto { id: "coder".into(), name: "Coder".into() },
        SoulDto { id: "tester".into(), name: "Tester".into() },
        SoulDto { id: "reviewer".into(), name: "Reviewer".into() },
        SoulDto { id: "documenter".into(), name: "Documenter".into() },
    ];
    Json(SoulsResponse { souls })
}

pub async fn list_runs_handler() -> Json<Vec<RunSummary>> {
    Json(list_runs(&RUN_STORE))
}

pub async fn get_run_handler(Path(run_id): Path<String>) -> Result<Json<RunState>, StatusCode> {
    get_run(&RUN_STORE, &run_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn start_run_handler(
    Json(req): Json<StartRunRequest>,
) -> Result<Json<StartRunResponse>, StatusCode> {
    let mut flow = FlowSpec::new(&req.flow_id);
    for step in req.steps {
        let gate = match step.gate.as_str() {
            "human" => crate::relay::flow::GateType::Human,
            _ => crate::relay::flow::GateType::Auto,
        };
        flow.add_step(
            crate::relay::flow::FlowStep::new(step.id, step.profession_id)
                .with_gate(gate)
                .with_agent_config(step.agent_config_id),
        );
    }

    let run_id = req.run_id.unwrap_or_else(|| format!("run-{}", uuid::Uuid::new_v4()));
    match start_run(&RUN_STORE, flow, &run_id) {
        Ok(run_state) => {
            let _ = EVENT_TX.send(RunEventBroadcast {
                run_id: run_id.clone(),
                event_type: "run_started".into(),
            });
            Ok(Json(StartRunResponse { run_id, state: run_state }))
        }
        Err(_) => Err(StatusCode::CONFLICT),
    }
}

pub async fn advance_run_handler(
    Path(run_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = advance_run(&RUN_STORE, &run_id).ok_or(StatusCode::NOT_FOUND)?;
    let _ = EVENT_TX.send(RunEventBroadcast {
        run_id: run_id.clone(),
        event_type: "step_advanced".into(),
    });
    Ok(Json(serde_json::json!({ "result": format!("{:?}", result) })))
}

pub async fn submit_handoff_handler(
    Path(run_id): Path<String>,
    Json(req): Json<HandoffRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = submit_handoff(&RUN_STORE, &run_id, req.handoff).ok_or(StatusCode::NOT_FOUND)?;
    let _ = EVENT_TX.send(RunEventBroadcast {
        run_id: run_id.clone(),
        event_type: "handoff_submitted".into(),
    });
    Ok(Json(serde_json::json!({ "result": format!("{:?}", result) })))
}

pub async fn resolve_gate_handler(
    Path(run_id): Path<String>,
    Json(req): Json<GateRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let decision = match req.decision.as_str() {
        "approve" => GateDecision::Approve,
        "reject" => GateDecision::Reject {
            feedback: req.feedback.unwrap_or_default(),
        },
        "edit" => GateDecision::Edit {
            changes: req.changes.unwrap_or_default(),
        },
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let result = resolve_gate(&RUN_STORE, &run_id, decision).ok_or(StatusCode::NOT_FOUND)?;
    let _ = EVENT_TX.send(RunEventBroadcast {
        run_id: run_id.clone(),
        event_type: "gate_resolved".into(),
    });
    Ok(Json(serde_json::json!({ "result": format!("{:?}", result) })))
}

/// SSE stream for run events.
pub async fn run_events_handler(
    Path(run_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = EVENT_TX.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(move |msg| {
            let Ok(msg) = msg else { return None };
            if msg.run_id != run_id {
                return None;
            }
            let event = Event::default()
                .event("run_event")
                .data(serde_json::to_string(&serde_json::json!({
                    "run_id": msg.run_id,
                    "event_type": msg.event_type,
                })).unwrap_or_default());
            Some(Ok(event))
        });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

// -------------------------------------------------------------------------
// Config — API Source Handlers
// -------------------------------------------------------------------------

static API_SOURCES: LazyLock<Mutex<Vec<ApiSource>>> = LazyLock::new(|| {
    Mutex::new(config::load_or_detect_api_sources())
});

pub async fn list_api_sources() -> Json<Vec<ApiSource>> {
    let sources = API_SOURCES.lock().unwrap();
    Json(sources.clone())
}

/// Scan the system for importable LLM providers (does not save).
pub async fn scan_api_sources() -> Json<Vec<ApiSource>> {
    Json(config::scan_importable_sources())
}

#[derive(serde::Deserialize)]
pub struct ImportApiSourcesRequest {
    pub source_ids: Vec<String>,
}

/// Import selected scanned sources into the configured sources list.
pub async fn import_api_sources(
    Json(req): Json<ImportApiSourcesRequest>,
) -> Json<Vec<ApiSource>> {
    let scanned = config::scan_importable_sources();
    let mut sources = API_SOURCES.lock().unwrap();
    for source in &scanned {
        if req.source_ids.contains(&source.id) && !sources.iter().any(|s| s.id == source.id) {
            sources.push(source.clone());
        }
    }
    let _ = config::save_api_sources(&sources);
    Json(sources.clone())
}

#[derive(serde::Deserialize)]
pub struct CreateApiSourceRequest {
    pub source: ApiSource,
}

pub async fn create_api_source(
    Json(req): Json<CreateApiSourceRequest>,
) -> Result<Json<ApiSource>, StatusCode> {
    let mut sources = API_SOURCES.lock().unwrap();
    if sources.iter().any(|s| s.id == req.source.id) {
        return Err(StatusCode::CONFLICT);
    }
    let source = req.source;
    sources.push(source.clone());
    let _ = config::save_api_sources(&sources);
    Ok(Json(source))
}

pub async fn update_api_source(
    Path(id): Path<String>,
    Json(req): Json<ApiSource>,
) -> Result<Json<ApiSource>, StatusCode> {
    let mut sources = API_SOURCES.lock().unwrap();
    let idx = sources.iter().position(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    if req.id != id {
        return Err(StatusCode::BAD_REQUEST);
    }
    sources[idx] = req.clone();
    let _ = config::save_api_sources(&sources);
    Ok(Json(req))
}

pub async fn delete_api_source(
    Path(id): Path<String>,
) -> StatusCode {
    let mut sources = API_SOURCES.lock().unwrap();
    let len_before = sources.len();
    sources.retain(|s| s.id != id);
    if sources.len() == len_before {
        return StatusCode::NOT_FOUND;
    }
    let _ = config::save_api_sources(&sources);
    StatusCode::NO_CONTENT
}

pub async fn test_api_connection(
    Path(id): Path<String>,
) -> Json<ConnectionTestResult> {
    let result = do_test_connection(&id).await;
    Json(result)
}

async fn do_test_connection(id: &str) -> ConnectionTestResult {
    let source = {
        let sources = API_SOURCES.lock().unwrap();
        match sources.iter().find(|s| s.id == id) {
            Some(s) => s.clone(),
            None => {
                return ConnectionTestResult {
                    success: false,
                    model: None,
                    error: Some("Source not found".into()),
                    latency_ms: None,
                };
            }
        }
    };

    let key = match config::resolve_api_key(&source) {
        Some(k) => k,
        None => {
            return ConnectionTestResult {
                success: false,
                model: None,
                error: Some("No API key configured".into()),
                latency_ms: None,
            };
        }
    };

    let model = source.models.first().map(|m| m.id.clone()).unwrap_or_default();
    let start = std::time::Instant::now();

    let client = reqwest::Client::new();
    let (url, auth_header, body) = match &source.provider {
        crate::relay::agent::Provider::Anthropic => {
            let base = source.base_url.as_deref().unwrap_or("https://api.anthropic.com");
            (
                format!("{}/v1/messages", base.trim_end_matches('/')),
                ("x-api-key".to_string(), key),
                serde_json::json!({
                    "model": model,
                    "max_tokens": 10,
                    "messages": [{"role": "user", "content": "Hi"}]
                }),
            )
        }
        crate::relay::agent::Provider::OpenAI => {
            let base = source.base_url.as_deref().unwrap_or("https://api.openai.com");
            (
                format!("{}/v1/chat/completions", base.trim_end_matches('/')),
                ("Authorization".to_string(), format!("Bearer {}", key)),
                serde_json::json!({
                    "model": model,
                    "max_tokens": 10,
                    "messages": [{"role": "user", "content": "Hi"}]
                }),
            )
        }
        crate::relay::agent::Provider::Local { url } => (
            format!("{}/v1/chat/completions", url.trim_end_matches('/')),
            ("Authorization".to_string(), format!("Bearer {}", key)),
            serde_json::json!({
                "model": model,
                "max_tokens": 10,
                "messages": [{"role": "user", "content": "Hi"}]
            }),
        ),
    };

    let result = client
        .post(&url)
        .header(&auth_header.0, &auth_header.1)
        .header("content-type", "application/json")
        .json(&body)
        .timeout(Duration::from_secs(15))
        .send()
        .await;

    let latency = start.elapsed().as_millis() as u64;

    match result {
        Ok(resp) if resp.status().is_success() => ConnectionTestResult {
            success: true,
            model: Some(model),
            error: None,
            latency_ms: Some(latency),
        },
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            ConnectionTestResult {
                success: false,
                model: None,
                error: Some(format!("HTTP {}: {}", status, body.chars().take(200).collect::<String>())),
                latency_ms: Some(latency),
            }
        }
        Err(e) => ConnectionTestResult {
            success: false,
            model: None,
            error: Some(format!("Connection failed: {}", e)),
            latency_ms: Some(latency),
        },
    }
}

// -------------------------------------------------------------------------
// Config — Agent Config Handlers
// -------------------------------------------------------------------------

static AGENT_CONFIGS: LazyLock<Mutex<Vec<AgentConfig>>> = LazyLock::new(|| {
    let sources = API_SOURCES.lock().unwrap().clone();
    Mutex::new(config::load_or_generate_agent_configs(&sources))
});

pub async fn list_agent_configs() -> Json<Vec<AgentConfig>> {
    let configs = AGENT_CONFIGS.lock().unwrap();
    Json(configs.clone())
}

#[derive(serde::Deserialize)]
pub struct CreateAgentConfigRequest {
    pub config: AgentConfig,
}

pub async fn create_agent_config(
    Json(req): Json<CreateAgentConfigRequest>,
) -> Result<Json<AgentConfig>, StatusCode> {
    let mut configs = AGENT_CONFIGS.lock().unwrap();
    if configs.iter().any(|c| c.id == req.config.id) {
        return Err(StatusCode::CONFLICT);
    }
    let cfg = req.config;
    configs.push(cfg.clone());
    let _ = config::save_agent_configs(&configs);
    Ok(Json(cfg))
}

pub async fn update_agent_config(
    Path(id): Path<String>,
    Json(req): Json<AgentConfig>,
) -> Result<Json<AgentConfig>, StatusCode> {
    if req.id != id {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut configs = AGENT_CONFIGS.lock().unwrap();
    let idx = configs.iter().position(|c| c.id == id).ok_or(StatusCode::NOT_FOUND)?;
    configs[idx] = req.clone();
    let _ = config::save_agent_configs(&configs);
    Ok(Json(req))
}

pub async fn delete_agent_config(
    Path(id): Path<String>,
) -> StatusCode {
    let mut configs = AGENT_CONFIGS.lock().unwrap();
    let is_default = configs.iter().any(|c| c.id == id && c.is_default);
    if is_default {
        return StatusCode::FORBIDDEN;
    }
    let len_before = configs.len();
    configs.retain(|c| c.id != id);
    if configs.len() == len_before {
        return StatusCode::NOT_FOUND;
    }
    let _ = config::save_agent_configs(&configs);
    StatusCode::NO_CONTENT
}

pub async fn reset_agent_defaults() -> Json<Vec<AgentConfig>> {
    let defaults = config::generate_default_agents();
    let mut configs = AGENT_CONFIGS.lock().unwrap();
    *configs = defaults.clone();
    let _ = config::save_agent_configs(&configs);
    Json(defaults)
}

// -------------------------------------------------------------------------
// Router
// -------------------------------------------------------------------------

pub fn relay_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        // Relay runs
        .route("/api/forge/relay/professions", get(list_professions))
        .route("/api/forge/relay/souls", get(list_souls))
        .route("/api/forge/relay/runs", get(list_runs_handler).post(start_run_handler))
        .route("/api/forge/relay/runs/{run_id}", get(get_run_handler))
        .route("/api/forge/relay/runs/{run_id}/advance", post(advance_run_handler))
        .route("/api/forge/relay/runs/{run_id}/handoff", post(submit_handoff_handler))
        .route("/api/forge/relay/runs/{run_id}/gate", post(resolve_gate_handler))
        .route("/api/forge/relay/runs/{run_id}/events", get(run_events_handler))
        // Config — API Sources
        .route("/api/forge/config/api-sources", get(list_api_sources).post(create_api_source))
        .route("/api/forge/config/api-sources/scan", get(scan_api_sources))
        .route("/api/forge/config/api-sources/import", post(import_api_sources))
        .route("/api/forge/config/api-sources/{id}", put(update_api_source).delete(delete_api_source))
        .route("/api/forge/config/api-sources/{id}/test", post(test_api_connection))
        // Config — Agent Configs
        .route("/api/forge/config/agents", get(list_agent_configs).post(create_agent_config))
        .route("/api/forge/config/agents/{id}", put(update_agent_config).delete(delete_agent_config))
        .route("/api/forge/config/agents/reset-defaults", post(reset_agent_defaults))
}
