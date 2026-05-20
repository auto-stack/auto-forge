//! Relay HTTP API
//!
//! Axum handlers for the Agents Relay module.
//! Uses a global in-memory store for simplicity.

use crate::relay::flow::FlowSpec;
use crate::relay::handoff::HandoffDocument;
use crate::relay::pipeline::GateDecision;
use crate::relay::profession::{self, Profession, ProfessionRegistry};
use crate::relay::store::{
    advance_run, delete_run, get_run, list_runs, new_run_store, resolve_gate, start_run, submit_handoff,
    RunState, RunStore, RunSummary,
};
use crate::relay::config::{self, AgentConfig, ApiSource, ConnectionTestResult};
use crate::relay::skills::{self, SkillDefinition};
use axum::extract::{Multipart, Path, State};
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

/// Access the global run store.
pub fn run_store() -> &'static RunStore {
    &RUN_STORE
}

/// Clone the global event broadcast sender.
pub fn event_sender() -> broadcast::Sender<RunEventBroadcast> {
    EVENT_TX.clone()
}

/// On startup, respawn background drivers for any runs that were in Running
/// state when the backend was last shut down.
pub fn resume_running_runs(ai_provider: crate::provider::AIProviderState) {
    let project_path = crate::forge::current_project_path().unwrap_or_default();
    let mut runs_to_resume: Vec<(String, String)> = Vec::new();
    {
        let map = RUN_STORE.lock().unwrap();
        for (run_id, entry) in map.iter() {
            if let crate::relay::pipeline::PipelineStatus::Running { ref step_id, ref profession_id, .. } = entry.engine.status {
                runs_to_resume.push((run_id.clone(), format!("Resuming step {} ({})", step_id, profession_id)));
            }
        }
    }
    for (run_id, task) in runs_to_resume {
        tracing::info!("Resuming relay driver for run {}: {}", run_id, task);
        tokio::spawn(crate::relay::driver::drive_run(
            run_id.clone(),
            RUN_STORE.clone(),
            EVENT_TX.clone(),
            ai_provider.clone(),
            task,
            project_path.clone(),
        ));
    }
}

#[derive(Clone, Debug)]
pub struct RunEventBroadcast {
    pub run_id: String,
    pub event_type: String,
    pub payload: Option<serde_json::Value>,
}

// -------------------------------------------------------------------------
// DTOs
// -------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct StartRunRequest {
    pub run_id: Option<String>,
    pub flow_id: String,
    pub steps: Vec<FlowStepDto>,
    #[serde(default)]
    pub task: Option<String>,
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
    pub base_skills: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct SoulsResponse {
    pub souls: Vec<SoulDto>,
}

#[derive(serde::Serialize)]
pub struct SoulDto {
    pub id: String,
    pub name: String,
    pub markdown: String,
}

// -------------------------------------------------------------------------
// Handlers
// -------------------------------------------------------------------------

static PROFESSIONS: LazyLock<Mutex<Vec<Profession>>> = LazyLock::new(|| {
    Mutex::new(profession::load_or_generate_professions())
});

pub async fn list_professions() -> Json<ProfessionsResponse> {
    let professions = PROFESSIONS.lock().unwrap();
    let dtos = professions.iter().map(|p| ProfessionDto {
        id: p.id.clone(),
        name: p.name.clone(),
        phase: p.phase.as_str().to_string(),
        owned_sections: p.owned_sections.iter().map(|s| s.as_str().to_string()).collect(),
        allowed_tools: p.allowed_tools.clone(),
        base_skills: p.base_skills.clone(),
    }).collect();
    Json(ProfessionsResponse { professions: dtos })
}

pub async fn list_config_professions() -> Json<Vec<Profession>> {
    let professions = PROFESSIONS.lock().unwrap();
    Json(professions.clone())
}

pub async fn create_profession(
    Json(req): Json<Profession>,
) -> Result<Json<Profession>, StatusCode> {
    let mut professions = PROFESSIONS.lock().unwrap();
    if professions.iter().any(|p| p.id == req.id) {
        return Err(StatusCode::CONFLICT);
    }
    professions.push(req.clone());
    let _ = profession::save_professions(&professions);
    Ok(Json(req))
}

pub async fn get_profession(Path(id): Path<String>) -> Result<Json<Profession>, StatusCode> {
    let professions = PROFESSIONS.lock().unwrap();
    professions.iter().find(|p| p.id == id).cloned().map(Json).ok_or(StatusCode::NOT_FOUND)
}

pub async fn update_profession(
    Path(id): Path<String>,
    Json(req): Json<Profession>,
) -> Result<Json<Profession>, StatusCode> {
    if req.id != id {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut professions = PROFESSIONS.lock().unwrap();
    let idx = professions.iter().position(|p| p.id == id).ok_or(StatusCode::NOT_FOUND)?;
    professions[idx] = req.clone();
    let _ = profession::save_professions(&professions);
    Ok(Json(req))
}

pub async fn delete_profession(Path(id): Path<String>) -> StatusCode {
    let mut professions = PROFESSIONS.lock().unwrap();
    let len_before = professions.len();
    professions.retain(|p| p.id != id);
    if professions.len() == len_before {
        return StatusCode::NOT_FOUND;
    }
    let _ = profession::save_professions(&professions);
    StatusCode::NO_CONTENT
}

pub async fn reset_profession_defaults() -> Json<Vec<Profession>> {
    let defaults = profession::generate_default_professions();
    let mut professions = PROFESSIONS.lock().unwrap();
    *professions = defaults.clone();
    let _ = profession::save_professions(&professions);
    Json(defaults)
}

pub async fn list_souls() -> Json<SoulsResponse> {
    let souls = vec![
        SoulDto { id: "assistant".into(), name: "Nicole".into(), markdown: include_str!("souls/assistant.md").into() },
        SoulDto { id: "advisor".into(), name: "Isaac".into(), markdown: include_str!("souls/advisor.md").into() },
        SoulDto { id: "planner".into(), name: "Felix".into(), markdown: include_str!("souls/planner.md").into() },
        SoulDto { id: "architect".into(), name: "Vera".into(), markdown: include_str!("souls/architect.md").into() },
        SoulDto { id: "coder".into(), name: "Ash".into(), markdown: include_str!("souls/coder.md").into() },
        SoulDto { id: "tester".into(), name: "Quinn".into(), markdown: include_str!("souls/tester.md").into() },
        SoulDto { id: "reviewer".into(), name: "Marcus".into(), markdown: include_str!("souls/reviewer.md").into() },
        SoulDto { id: "documenter".into(), name: "Luna".into(), markdown: include_str!("souls/documenter.md").into() },
        SoulDto { id: "gofer".into(), name: "Gus".into(), markdown: include_str!("souls/gofer.md").into() },
    ];
    Json(SoulsResponse { souls })
}

pub async fn get_soul(Path(id): Path<String>) -> Result<Json<SoulDto>, StatusCode> {
    let map: [(&str, &str, &str); 9] = [
        ("assistant", "Nicole", include_str!("souls/assistant.md")),
        ("advisor", "Isaac", include_str!("souls/advisor.md")),
        ("planner", "Felix", include_str!("souls/planner.md")),
        ("architect", "Vera", include_str!("souls/architect.md")),
        ("coder", "Ash", include_str!("souls/coder.md")),
        ("tester", "Quinn", include_str!("souls/tester.md")),
        ("reviewer", "Marcus", include_str!("souls/reviewer.md")),
        ("documenter", "Luna", include_str!("souls/documenter.md")),
        ("gofer", "Gus", include_str!("souls/gofer.md")),
    ];
    let (name, markdown) = map.iter()
        .find(|(sid, _, _)| *sid == id)
        .map(|(_, n, m)| (*n, *m))
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(SoulDto { id, name: name.to_string(), markdown: markdown.to_string() }))
}

pub async fn list_runs_handler() -> Json<Vec<RunSummary>> {
    Json(list_runs(&RUN_STORE))
}

pub async fn get_run_handler(Path(run_id): Path<String>) -> Result<Json<RunState>, StatusCode> {
    get_run(&RUN_STORE, &run_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn delete_run_handler(Path(run_id): Path<String>) -> StatusCode {
    if delete_run(&RUN_STORE, &run_id) {
        let _ = EVENT_TX.send(RunEventBroadcast {
            run_id: run_id.clone(),
            event_type: "run_deleted".into(),
            payload: None,
        });
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

pub async fn start_run_handler(
    State(ai_provider): State<crate::provider::AIProviderState>,
    Json(req): Json<StartRunRequest>,
) -> Result<Json<StartRunResponse>, StatusCode> {
    // If steps are provided inline, build a custom flow.
    // Otherwise, look up the flow_id in the registry (built-in or YAML).
    let flow = if req.steps.is_empty() {
        match crate::relay::flows::get_flow(&req.flow_id) {
            Some(f) => f,
            None => {
                tracing::warn!("Flow '{}' not found in registry", req.flow_id);
                return Err(StatusCode::NOT_FOUND);
            }
        }
    } else {
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
        flow
    };

    let run_id = req.run_id.unwrap_or_else(|| format!("run-{}", uuid::Uuid::new_v4()));
    match start_run(&RUN_STORE, flow, &run_id) {
        Ok(run_state) => {
            let _ = EVENT_TX.send(RunEventBroadcast {
                run_id: run_id.clone(),
                event_type: "run_started".into(),
                payload: None,
            });

            // Spawn background driver to execute the pipeline
            let project_path = crate::forge::current_project_path().unwrap_or_default();
            let task = req.task.unwrap_or_default();
            tokio::spawn(crate::relay::driver::drive_run(
                run_id.clone(),
                RUN_STORE.clone(),
                EVENT_TX.clone(),
                ai_provider,
                task,
                project_path,
            ));

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
        payload: None,
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
        payload: None,
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
        payload: None,
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
                    "payload": msg.payload,
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

// -------------------------------------------------------------------------
// Config — Skill Handlers
// -------------------------------------------------------------------------

static SKILLS: LazyLock<Mutex<Vec<SkillDefinition>>> = LazyLock::new(|| {
    Mutex::new(skills::load_or_generate_skills())
});

pub async fn list_skills() -> Json<Vec<SkillDefinition>> {
    let skills = SKILLS.lock().unwrap();
    Json(skills.clone())
}

pub async fn create_skill(
    Json(req): Json<SkillDefinition>,
) -> Result<Json<SkillDefinition>, StatusCode> {
    let mut skills = SKILLS.lock().unwrap();
    if skills.iter().any(|s| s.id == req.id) {
        return Err(StatusCode::CONFLICT);
    }
    skills.push(req.clone());
    let _ = skills::save_skills(&skills);
    Ok(Json(req))
}

pub async fn get_skill(Path(id): Path<String>) -> Result<Json<SkillDefinition>, StatusCode> {
    let skills = SKILLS.lock().unwrap();
    skills.iter().find(|s| s.id == id).cloned().map(Json).ok_or(StatusCode::NOT_FOUND)
}

pub async fn update_skill(
    Path(id): Path<String>,
    Json(req): Json<SkillDefinition>,
) -> Result<Json<SkillDefinition>, StatusCode> {
    if req.id != id {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut skills = SKILLS.lock().unwrap();
    let idx = skills.iter().position(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    skills[idx] = req.clone();
    let _ = skills::save_skills(&skills);
    Ok(Json(req))
}

pub async fn delete_skill(Path(id): Path<String>) -> StatusCode {
    let mut skills = SKILLS.lock().unwrap();
    let len_before = skills.len();
    skills.retain(|s| s.id != id);
    if skills.len() == len_before {
        return StatusCode::NOT_FOUND;
    }
    let _ = skills::save_skills(&skills);
    StatusCode::NO_CONTENT
}

pub async fn reset_skill_defaults() -> Json<Vec<SkillDefinition>> {
    let defaults = skills::generate_default_skills();
    let mut skills = SKILLS.lock().unwrap();
    *skills = defaults.clone();
    let _ = skills::save_skills(&skills);
    Json(defaults)
}

pub async fn reset_agent_defaults() -> Json<Vec<AgentConfig>> {
    let source_id = {
        let sources = API_SOURCES.lock().unwrap();
        sources.first().map(|s| s.id.clone()).unwrap_or_default()
    };
    let defaults = config::generate_default_agents_with_source(&source_id);
    let mut configs = AGENT_CONFIGS.lock().unwrap();
    *configs = defaults.clone();
    let _ = config::save_agent_configs(&configs);
    Json(defaults)
}

#[derive(serde::Serialize)]
pub struct AvatarUrlResponse {
    pub avatar_url: String,
}

/// Upload an avatar image for an agent.
pub async fn upload_agent_avatar(
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<AvatarUrlResponse>, StatusCode> {
    let field = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let ext = field
        .file_name()
        .and_then(|n| {
            let n = n.to_lowercase();
            if n.ends_with(".png") {
                Some("png")
            } else if n.ends_with(".jpg") || n.ends_with(".jpeg") {
                Some("jpg")
            } else if n.ends_with(".gif") {
                Some("gif")
            } else if n.ends_with(".webp") {
                Some("webp")
            } else {
                Some("png")
            }
        })
        .unwrap_or("png");

    let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
    if data.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let dir = config::avatars_dir();
    std::fs::create_dir_all(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let filename = format!("{}.{}", id, ext);
    let path = dir.join(&filename);
    std::fs::write(&path, &data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let avatar_url = format!("/avatars/{}?v={}", filename, ts);

    // Update config
    {
        let mut configs = AGENT_CONFIGS.lock().unwrap();
        if let Some(idx) = configs.iter().position(|c| c.id == id) {
            configs[idx].avatar_url = Some(avatar_url.clone());
            let _ = config::save_agent_configs(&configs);
        }
    }

    Ok(Json(AvatarUrlResponse { avatar_url }))
}

/// Generate an avatar image using Pollinations.ai.
pub async fn generate_agent_avatar(
    Path(id): Path<String>,
) -> Result<Json<AvatarUrlResponse>, StatusCode> {
    let (name, profession_id) = {
        let configs = AGENT_CONFIGS.lock().unwrap();
        let cfg = configs.iter().find(|c| c.id == id).ok_or(StatusCode::NOT_FOUND)?;
        (cfg.name.clone(), cfg.profession_id.clone())
    };

    let prompt = format!(
        "A professional friendly avatar portrait of a {} named {}, minimalist flat illustration style, solid pastel background, clean vector look, centered face, warm expression, no text, no watermark",
        profession_id, name
    );
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    let url = format!(
        "https://image.pollinations.ai/prompt/{}?width=512&height=512&seed={}&nologo=true&negative=blurry,ugly,deformed,text,watermark,logo",
        urlencoding::encode(&prompt),
        seed
    );

    let client = reqwest::Client::new();
    let mut last_err = String::new();
    let resp = {
        let mut response = None;
        for attempt in 1..=3 {
            match client
                .get(&url)
                .header("User-Agent", "auto-forge/0.1")
                .timeout(std::time::Duration::from_secs(60))
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => {
                    response = Some(r);
                    break;
                }
                Ok(r) => {
                    last_err = format!("HTTP {}", r.status());
                    tracing::warn!("Avatar gen attempt {} failed: HTTP {}", attempt, r.status());
                }
                Err(e) => {
                    last_err = e.to_string();
                    tracing::warn!("Avatar gen attempt {} failed: {}", attempt, e);
                }
            }
            if attempt < 3 {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
        response.ok_or_else(|| {
            tracing::error!("Failed to generate avatar after 3 attempts: {}", last_err);
            StatusCode::BAD_GATEWAY
        })?
    };

    let data = resp.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    let dir = config::avatars_dir();
    std::fs::create_dir_all(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let filename = format!("{}.png", id);
    let path = dir.join(&filename);
    std::fs::write(&path, &data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let avatar_url = format!("/avatars/{}?v={}", filename, seed);

    {
        let mut configs = AGENT_CONFIGS.lock().unwrap();
        if let Some(idx) = configs.iter().position(|c| c.id == id) {
            configs[idx].avatar_url = Some(avatar_url.clone());
            let _ = config::save_agent_configs(&configs);
        }
    }

    Ok(Json(AvatarUrlResponse { avatar_url }))
}

// -------------------------------------------------------------------------
// Router
// -------------------------------------------------------------------------

pub fn relay_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    crate::provider::AIProviderState: axum::extract::FromRef<S>,
{
    Router::new()
        // Relay runs
        .route("/api/forge/relay/professions", get(list_professions))
        .route("/api/forge/relay/souls", get(list_souls))
        .route("/api/forge/relay/souls/{id}", get(get_soul))
        .route("/api/forge/relay/runs", get(list_runs_handler).post(start_run_handler))
        .route("/api/forge/relay/runs/{run_id}", get(get_run_handler).delete(delete_run_handler))
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
        // Config — Professions
        .route("/api/forge/config/professions", get(list_config_professions).post(create_profession))
        .route("/api/forge/config/professions/{id}", get(get_profession).put(update_profession).delete(delete_profession))
        .route("/api/forge/config/professions/reset-defaults", post(reset_profession_defaults))
        // Config — Skills
        .route("/api/forge/config/skills", get(list_skills).post(create_skill))
        .route("/api/forge/config/skills/{id}", get(get_skill).put(update_skill).delete(delete_skill))
        .route("/api/forge/config/skills/reset-defaults", post(reset_skill_defaults))
        // Config — Agent Configs
        .route("/api/forge/config/agents", get(list_agent_configs).post(create_agent_config))
        .route("/api/forge/config/agents/{id}", put(update_agent_config).delete(delete_agent_config))
        .route("/api/forge/config/agents/reset-defaults", post(reset_agent_defaults))
        .route("/api/forge/config/agents/{id}/avatar", post(upload_agent_avatar))
        .route("/api/forge/config/agents/{id}/avatar/generate", post(generate_agent_avatar))
}
