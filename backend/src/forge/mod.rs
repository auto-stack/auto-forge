//! AutoSmith — Spec-driven serial agent orchestration
//!
//! Module structure (5 source files):
//!   - mod.rs      : this file — sessions, specs, status, persistence
//!   - errand.rs   : errand log persistence and audit
//!   - project.rs  : project management
//!   - tools.rs    : tool utilities and caching
//!   - wiki.rs     : wiki page storage and retrieval
//!
//! This module adds Forge (chat loop), Specs (knowledge management),
//! and Relay (agent pipeline) endpoints to the auto-playground server.
//! It reuses the existing NotebookActor for VM session sharing with AutoLab.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response, sse::{Event, KeepAlive, Sse}},
    routing::{get, post},
    Json, Router,
};
use futures::{FutureExt, stream::{self, Stream}};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};


pub mod errand;
pub mod project;
pub mod tools;
pub mod wiki;

use axum::extract::FromRef;



// ─── Persistent Session Store ────────────────────────────────────────────────

/// High-level work mode chosen by the Assistant for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkMode {
    /// Answer directly or use simple file tools; no relay pipeline.
    Direct,
    /// Hand off to a single YAML FlowSpec relay pipeline.
    SingleRelay,
    /// Hand off to a multi-relay Atom TaskPlan.
    MultiRelay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeSession {
    pub id: String,
    pub notebook_sid: Option<String>,
    pub project_path: String,
    pub status: ForgeStatus,
    pub messages: Vec<ForgeMessage>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub pending_spec_changes: Vec<SpecChange>,
    #[serde(default)]
    pub focus_section: Option<String>,
    #[serde(default)]
    pub active_profession: Option<String>,
    #[serde(default)]
    pub errand_sessions: Vec<crate::forge::errand::ErrandSession>,
    /// Classified work mode for this session, persisted after first classification.
    #[serde(default)]
    pub work_mode: Option<WorkMode>,
    /// Active TaskPlan instance ID, if any.
    #[serde(default)]
    pub active_task_plan: Option<String>,
    /// Relay run IDs spawned from this session.
    #[serde(default)]
    pub active_relay_runs: Vec<String>,
}

/// Section type determines the lifecycle states and allowed transitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SectionType {
    Goals,
    Architecture,
    Designs,
    Plans,
    Tests,
    Reviews,
    Reports,
}

impl SectionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SectionType::Goals => "goals",
            SectionType::Architecture => "architecture",
            SectionType::Designs => "designs",
            SectionType::Plans => "plans",
            SectionType::Tests => "tests",
            SectionType::Reviews => "reviews",
            SectionType::Reports => "reports",
        }
    }

    pub fn from_id(id: &str) -> Self {
        match id {
            "architecture" => SectionType::Architecture,
            "designs" => SectionType::Designs,
            "plans" => SectionType::Plans,
            "tests" => SectionType::Tests,
            "reviews" => SectionType::Reviews,
            "reports" => SectionType::Reports,
            _ => SectionType::Goals,
        }
    }
}

/// Lifecycle status shared across all categories.
/// Not every category uses every variant — each SectionType configures its own subset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Empty,
    Proposed,
    Draft,
    UnderReview,
    Approved,
    InProgress,
    InImplementation,
    Implemented,
    Verified,
    Done,
    Archived,
    Rejected,
    Backlog,
    Ready,
    InReview,
    Blocked,
    Superseded,
    Outdated,
    Stable,
    Deprecated,
    Published,
    Analysed,
    Obsolete,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Empty => "empty",
            Status::Proposed => "proposed",
            Status::Draft => "draft",
            Status::UnderReview => "under_review",
            Status::Approved => "approved",
            Status::InProgress => "in_progress",
            Status::InImplementation => "in_implementation",
            Status::Implemented => "implemented",
            Status::Verified => "verified",
            Status::Done => "done",
            Status::Archived => "archived",
            Status::Rejected => "rejected",
            Status::Backlog => "backlog",
            Status::Ready => "ready",
            Status::InReview => "in_review",
            Status::Blocked => "blocked",
            Status::Superseded => "superseded",
            Status::Outdated => "outdated",
            Status::Stable => "stable",
            Status::Deprecated => "deprecated",
            Status::Published => "published",
            Status::Analysed => "analysed",
            Status::Obsolete => "obsolete",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "empty" => Status::Empty,
            "proposed" => Status::Proposed,
            "draft" => Status::Draft,
            "under_review" => Status::UnderReview,
            "approved" => Status::Approved,
            "in_progress" => Status::InProgress,
            "in_implementation" => Status::InImplementation,
            "implemented" => Status::Implemented,
            "verified" => Status::Verified,
            "done" => Status::Done,
            "archived" => Status::Archived,
            "rejected" => Status::Rejected,
            "backlog" => Status::Backlog,
            "ready" => Status::Ready,
            "in_review" => Status::InReview,
            "blocked" => Status::Blocked,
            "superseded" => Status::Superseded,
            "outdated" => Status::Outdated,
            "stable" => Status::Stable,
            "deprecated" => Status::Deprecated,
            "published" => Status::Published,
            "analysed" => Status::Analysed,
            "obsolete" => Status::Obsolete,
            _ => Status::Draft,
        }
    }
}

/// A single item inside a SpecsSection.
/// Goals, Architecture, Designs, Plans, Tests, etc. are all represented as items with their own lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecItem {
    pub id: String,
    pub title: String,
    pub content: String,
    pub status: Status,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Auto-populated backlinks: IDs of items that reference this item.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub created_at: u64,
    pub modified_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
}

/// Per-category state-machine configuration.
pub struct SectionConfig {
    pub section_type: SectionType,
    pub allowed_statuses: Vec<Status>,
    pub allowed_transitions: Vec<(Status, Status)>,
}

impl SectionConfig {
    pub fn for_type(section_type: &SectionType) -> Self {
        match section_type {
            SectionType::Goals => Self {
                section_type: SectionType::Goals,
                allowed_statuses: vec![
                    Status::Empty, Status::Proposed, Status::Analysed, Status::Approved,
                    Status::InProgress, Status::Implemented, Status::Done, Status::Archived,
                ],
                allowed_transitions: vec![
                    (Status::Empty, Status::Proposed),
                    (Status::Proposed, Status::Analysed),
                    (Status::Analysed, Status::Approved),
                    (Status::Approved, Status::InProgress),
                    (Status::InProgress, Status::Implemented),
                    (Status::Implemented, Status::Done),
                    (Status::Done, Status::Archived),
                    (Status::InProgress, Status::Archived),
                ],
            },
            SectionType::Architecture | SectionType::Designs => Self {
                section_type: section_type.clone(),
                allowed_statuses: vec![
                    Status::Empty, Status::Draft, Status::UnderReview, Status::Approved,
                    Status::Superseded, Status::Outdated,
                ],
                allowed_transitions: vec![
                    (Status::Empty, Status::Draft),
                    (Status::Draft, Status::UnderReview),
                    (Status::UnderReview, Status::Approved),
                    (Status::UnderReview, Status::Rejected),
                    (Status::Approved, Status::Superseded),
                    (Status::Approved, Status::Outdated),
                ],
            },
            SectionType::Plans => Self {
                section_type: SectionType::Plans,
                allowed_statuses: vec![
                    Status::Empty, Status::Draft, Status::Approved, Status::InProgress,
                    Status::Done, Status::Obsolete,
                ],
                allowed_transitions: vec![
                    (Status::Empty, Status::Draft),
                    (Status::Draft, Status::Approved),
                    (Status::Approved, Status::InProgress),
                    (Status::InProgress, Status::Done),
                    (Status::Done, Status::Obsolete),
                ],
            },
            SectionType::Reports => Self {
                section_type: SectionType::Reports,
                allowed_statuses: vec![
                    Status::Empty, Status::Draft, Status::Published,
                ],
                allowed_transitions: vec![
                    (Status::Empty, Status::Draft),
                    (Status::Draft, Status::Published),
                    (Status::UnderReview, Status::Stable),
                    (Status::Stable, Status::Deprecated),
                ],
            },
            SectionType::Tests => Self {
                section_type: SectionType::Tests,
                allowed_statuses: vec![
                    Status::Empty, Status::Draft, Status::Implemented,
                    Status::Done, Status::Verified, Status::Blocked,
                ],
                allowed_transitions: vec![
                    (Status::Empty, Status::Draft),
                    (Status::Draft, Status::Implemented),
                    (Status::Implemented, Status::Done),
                    (Status::Done, Status::Verified),
                    (Status::Implemented, Status::Blocked),
                    (Status::Blocked, Status::Implemented),
                ],
            },
            SectionType::Reviews | SectionType::Reports => Self {
                section_type: section_type.clone(),
                allowed_statuses: vec![
                    Status::Empty, Status::Draft, Status::Published,
                ],
                allowed_transitions: vec![
                    (Status::Empty, Status::Draft),
                    (Status::Draft, Status::Published),
                ],
            },
        }
    }

    pub fn can_transition(&self, from: &Status, to: &Status) -> bool {
        self.allowed_transitions.contains(&(from.clone(), to.clone()))
            || from == to
            || self.allowed_statuses.contains(to)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecChange {
    pub section_id: String,
    #[serde(default)]
    pub item_id: Option<String>,
    pub old_content: String,
    pub new_content: String,
    pub old_status: String,
    pub new_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgeStatus {
    Idle,
    Thinking,
    ToolCall,
    WaitingApproval,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profession_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub result: Option<String>,
    pub status: String,
}

pub struct SessionStore {
    sessions: std::collections::HashMap<String, ForgeSession>,
    data_dir: PathBuf,
    /// Maps project_path → active_session_id.
    /// Only one session per project may hold the lock at a time.
    project_locks: std::collections::HashMap<String, String>,
}

impl SessionStore {
    fn new() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("autoforge")
            .join("sessions");
        let _ = std::fs::create_dir_all(&data_dir);

        let mut store = Self {
            sessions: std::collections::HashMap::new(),
            data_dir,
            project_locks: std::collections::HashMap::new(),
        };
        store.load_all();
        // Rebuild project locks from loaded sessions (any non-idle session claims its project)
        for (sid, session) in &store.sessions {
            if !matches!(session.status, ForgeStatus::Idle) {
                store.project_locks.insert(session.project_path.clone(), sid.clone());
            }
        }
        store
    }

    fn load_all(&mut self) {
        let Ok(entries) = std::fs::read_dir(&self.data_dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() != Some("json".as_ref()) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            let Ok(session) = serde_json::from_str::<ForgeSession>(&content) else { continue };
            self.sessions.insert(session.id.clone(), session);
        }
        tracing::info!("Loaded {} persistent Forge sessions", self.sessions.len());
    }

    pub fn get(&self, sid: &str) -> Option<&ForgeSession> {
        self.sessions.get(sid)
    }

    pub fn get_mut(&mut self, sid: &str) -> Option<&mut ForgeSession> {
        self.sessions.get_mut(sid)
    }

    pub fn insert(&mut self, session: ForgeSession) {
        self.save(&session);
        self.sessions.insert(session.id.clone(), session);
    }

    fn push_message(&mut self, sid: &str, msg: ForgeMessage) {
        let Some(session) = self.sessions.get_mut(sid) else { return };
        tracing::info!(
            "ForgeMessage pushed: sid={}, msg_id={}, role={}, content_len={}",
            sid, msg.id, msg.role, msg.content.len()
        );
        session.messages.push(msg);
        let session_clone = session.clone();
        self.save(&session_clone);
    }

    pub fn update_status(&mut self, sid: &str, status: ForgeStatus) {
        let Some(session) = self.sessions.get_mut(sid) else { return };
        session.status = status;
        let session_clone = session.clone();
        self.save(&session_clone);
    }

    fn set_focus_section(&mut self, sid: &str, section_id: Option<String>) {
        let Some(session) = self.sessions.get_mut(sid) else { return };
        session.focus_section = section_id;
        let session_clone = session.clone();
        self.save(&session_clone);
    }

    pub fn save(&self, session: &ForgeSession) {
        let path = self.data_dir.join(format!("{}.json", session.id));
        if let Ok(json) = serde_json::to_string_pretty(session) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn list_all(&self) -> Vec<&ForgeSession> {
        self.sessions.values().collect()
    }

    /// Ensure only `sid` is active for its project.
    /// Any other session for the same project is demoted to Idle.
    fn acquire_project_lock(&mut self, sid: &str) {
        let Some(session) = self.sessions.get(sid) else { return };
        let project = session.project_path.clone();
        // Demote previous holder (if any and if different)
        if let Some(prev_sid) = self.project_locks.get(&project) {
            if prev_sid != sid {
                if let Some(prev) = self.sessions.get_mut(prev_sid) {
                    prev.status = ForgeStatus::Idle;
                    let clone = prev.clone();
                    self.save(&clone);
                }
            }
        }
        self.project_locks.insert(project, sid.to_string());
    }

    /// Get the currently active session for a project, if any.
    fn active_session_for(&self, project_path: &str) -> Option<&ForgeSession> {
        let sid = self.project_locks.get(project_path)?;
        self.sessions.get(sid)
    }

    fn rename(&mut self, sid: &str, name: String) -> bool {
        let Some(session) = self.sessions.get_mut(sid) else { return false };
        session.name = Some(name);
        let clone = session.clone();
        self.save(&clone);
        true
    }

    pub fn remove(&mut self, sid: &str) -> bool {
        let existed = self.sessions.remove(sid).is_some();
        if existed {
            let path = self.data_dir.join(format!("{}.json", sid));
            let _ = std::fs::remove_file(path);
            // Also remove any project lock held by this session
            self.project_locks.retain(|_, v| v != sid);
        }
        existed
    }

    pub fn clear(&mut self) {
        for (sid, _) in self.sessions.drain() {
            let path = self.data_dir.join(format!("{}.json", sid));
            let _ = std::fs::remove_file(path);
        }
        self.project_locks.clear();
    }
}

pub fn forge_sessions() -> &'static Mutex<SessionStore> {
    static STORE: OnceLock<Mutex<SessionStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(SessionStore::new()))
}

// ─── Request / Response Types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateForgeSessionRequest {
    pub notebook_sid: Option<String>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    #[serde(default)]
    pub profession_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeMessageResponse {
    pub message: ForgeMessage,
}

/// SSE event types sent to the frontend
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ForgeStreamEvent {
    #[serde(rename = "turn_start")]
    TurnStart { profession_id: String },
    #[serde(rename = "delta")]
    Delta { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        name: String,
        arguments: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        id: String,
        result: String,
    },
    #[serde(rename = "phase_change")]
    PhaseChange { phase: String },
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "agent_handoff")]
    AgentHandoff {
        from_agent: String,
        from_profession: String,
        to_profession: String,
        to_agent: String,
        classification: String,
        reason: String,
    },
    #[serde(rename = "errand_start")]
    ErrandStart {
        errand_id: String,
        profession_id: String,
        task: String,
        tool_call_id: String,
    },
    #[serde(rename = "errand_turn_start")]
    ErrandTurnStart {
        errand_id: String,
        turn: u32,
        profession_id: String,
        tool_call_id: String,
    },
    #[serde(rename = "errand_delta")]
    ErrandDelta {
        errand_id: String,
        text: String,
        tool_call_id: String,
    },
    #[serde(rename = "errand_tool_call")]
    ErrandToolCall {
        errand_id: String,
        id: String,
        name: String,
        arguments: Value,
        tool_call_id: String,
    },
    #[serde(rename = "errand_tool_result")]
    ErrandToolResult {
        errand_id: String,
        id: String,
        result: String,
        tool_call_id: String,
    },
    #[serde(rename = "errand_complete")]
    ErrandComplete {
        errand_id: String,
        status: String,
        result: String,
        token_usage: u64,
        tool_call_id: String,
    },
    #[serde(rename = "relay_spawned")]
    RelaySpawned {
        run_id: String,
        flow_id: String,
        status: String,
    },
    #[serde(rename = "task_plan_spawned")]
    TaskPlanSpawned {
        instance_id: String,
        task_plan_id: String,
        status: String,
    },
    #[serde(rename = "relay_update")]
    RelayUpdate {
        run_id: String,
        step_id: String,
        profession_id: String,
        status: String,
    },
    #[serde(rename = "relay_gate_waiting")]
    RelayGateWaiting {
        run_id: String,
        gate: String,
        step_id: String,
    },
    #[serde(rename = "relay_complete")]
    RelayComplete {
        run_id: String,
        status: String,
        summary: String,
        tokens_used: u64,
    },
}

// ─── Specs Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecsDocument {
    pub project: String,
    pub version: u64,
    pub sections: Vec<SpecsSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecsSection {
    pub id: String,
    #[serde(default = "default_section_type")]
    pub section_type: SectionType,
    pub title: String,
    #[serde(default)]
    pub items: Vec<SpecItem>,
    /// Representative status of the whole section (aggregated from items or set manually).
    #[serde(default = "default_status")]
    pub status: Status,
    /// Legacy content field kept for backward-compat during migration.
    /// If `items` is empty on load, content is auto-migrated into a single item.
    #[serde(default)]
    pub content: String,
    pub depends_on: Vec<String>,
    pub last_modified: u64,
    pub last_verified: Option<u64>,
}

fn default_section_type() -> SectionType {
    SectionType::Goals
}

fn default_status() -> Status {
    Status::Empty
}

// ─── Manifest Types (for .ad + manifest.at format) ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestAt {
    project: String,
    version: u32,
    #[serde(rename = "section", default)]
    sections: Vec<ManifestSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestSection {
    id: String,
    #[serde(rename = "section_type")]
    section_type: String,
    title: String,
    status: String,
    last_modified: u64,
    last_verified: Option<u64>,
}

// ─── Persistent Specs Store ─────────────────────────────────────────────────

pub struct SpecsStore {
    projects: std::collections::HashMap<String, SpecsDocument>,
    data_dir: PathBuf,
    templates_dir: PathBuf,
    /// If `data_dir/manifest.at` exists, this holds the project name from that manifest.
    /// In flat mode, specs live directly in `data_dir` instead of a subdirectory.
    flat_mode_project: Option<String>,
}

// ─── Embedded Default Templates ──────────────────────────────────────────────

const TMPL_GOALS: &str = include_str!("templates/goals.ad");
const TMPL_ARCHITECTURE: &str = include_str!("templates/architecture.ad");
const TMPL_DESIGNS: &str = include_str!("templates/designs.ad");
const TMPL_PLANS: &str = include_str!("templates/plans.ad");
const TMPL_TESTS: &str = include_str!("templates/tests.ad");
const TMPL_REVIEWS: &str = include_str!("templates/reviews.ad");
const TMPL_REPORTS: &str = include_str!("templates/reports.ad");


impl SpecsStore {
    /// Return the parent directory of the specs data_dir (i.e. the project root).
    pub fn project_base_path(&self) -> Option<std::path::PathBuf> {
        self.data_dir.parent().map(|p| p.to_path_buf())
    }

    fn new_default() -> Self {
        let templates_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("autoforge")
            .join("templates");
        let _ = std::fs::create_dir_all(&templates_dir);
        let mut store = Self {
            projects: std::collections::HashMap::new(),
            data_dir: PathBuf::new(),
            templates_dir,
            flat_mode_project: None,
        };
        store.extract_embedded_templates();
        store
    }

    pub fn is_project_open(&self) -> bool {
        !self.data_dir.as_os_str().is_empty()
    }

    pub fn open_project(&mut self, project_path: &std::path::Path) -> Result<project::ProjectInfo, String> {
        if !project_path.exists() {
            return Err(format!("Directory does not exist: {}", project_path.display()));
        }
        let specs_dir = project::find_specs_dir(project_path);
        self.projects.clear();
        self.data_dir = specs_dir.clone();
        self.flat_mode_project = None;
        let _ = std::fs::create_dir_all(&self.data_dir);

        // Detect flat mode
        let flat_manifest = self.data_dir.join("manifest.at");
        if flat_manifest.exists() {
            if let Ok(content) = std::fs::read_to_string(&flat_manifest) {
                if let Ok(manifest) = toml::from_str::<ManifestAt>(&content) {
                    self.flat_mode_project = Some(manifest.project);
                }
            }
        }
        self.load_all();

        let name = project_path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        tracing::info!("Opened project '{}' — specs at {}", name, specs_dir.display());
        notify_watcher_path(Some(self.data_dir.clone()));
        Ok(project::ProjectInfo {
            path: project_path.to_string_lossy().to_string(),
            name,
            specs_dir: specs_dir.to_string_lossy().to_string(),
            has_specs: self.flat_mode_project.is_some() || !self.projects.is_empty(),
            is_open: true,
            is_empty: project::is_project_empty(project_path),
        })
    }

    pub fn close_project(&mut self) {
        self.projects.clear();
        self.data_dir = PathBuf::new();
        self.flat_mode_project = None;
        notify_watcher_path(None);
    }

    fn extract_embedded_templates(&self) {
        let templates: [(&str, &str); 7] = [
            ("goals", TMPL_GOALS),
            ("architecture", TMPL_ARCHITECTURE),
            ("designs", TMPL_DESIGNS),
            ("plans", TMPL_PLANS),
            ("tests", TMPL_TESTS),
            ("reviews", TMPL_REVIEWS),
            ("reports", TMPL_REPORTS),

        ];
        for (name, content) in templates {
            let path = self.templates_dir.join(format!("{}.ad", name));
            if !path.exists() {
                let _ = std::fs::write(&path, content);
            }
        }
        tracing::info!("Templates directory: {:?}", self.templates_dir);
    }

    fn load_template(&self, name: &str) -> String {
        let path = self.templates_dir.join(format!("{}.ad", name));
        std::fs::read_to_string(&path).unwrap_or_else(|_| {
            tracing::warn!("Template file not found: {:?}, using embedded fallback", path);
            match name {
                "goals" => TMPL_GOALS.to_string(),
                "architecture" => TMPL_ARCHITECTURE.to_string(),
                "designs" => TMPL_DESIGNS.to_string(),
                "plans" => TMPL_PLANS.to_string(),
                "tests" => TMPL_TESTS.to_string(),
                "reviews" => TMPL_REVIEWS.to_string(),
                "reports" => TMPL_REPORTS.to_string(),

                _ => String::new(),
            }
        })
    }

    fn load_all(&mut self) {
        // Flat mode: manifest.at lives directly in data_dir
        if let Some(ref flat_project) = self.flat_mode_project {
            if let Some(doc) = self.load_ad_format(&self.data_dir, flat_project) {
                let mut debug = format!("load_all flat_mode: project={}, sections={}\n", flat_project, doc.sections.len());
                for sec in &doc.sections {
                    debug.push_str(&format!("  {}: {} items\n", sec.id, sec.items.len()));
                }
                let _ = std::fs::write("D:/autostack/auto-forge/specs/debug_load_all.txt", &debug);
                self.projects.insert(flat_project.clone(), doc);
                let mut debug2 = format!("load_all after insert: keys={:?}\n", self.projects.keys().collect::<Vec<_>>());
                for (k, v) in &self.projects {
                    debug2.push_str(&format!("  {}: {} sections\n", k, v.sections.len()));
                    for sec in &v.sections {
                        debug2.push_str(&format!("    {}: {} items\n", sec.id, sec.items.len()));
                    }
                }
                let _ = std::fs::write("D:/autostack/auto-forge/specs/debug_load_all.txt", &debug2);
            } else {
                let _ = std::fs::write("D:/autostack/auto-forge/specs/debug_load_all.txt", "load_all: load_ad_format returned None\n");
            }
        } else {
            let _ = std::fs::write("D:/autostack/auto-forge/specs/debug_load_all.txt", "load_all: flat_mode_project is None\n");
        }

        // Nested mode: scan subdirectories for additional projects
        let Ok(entries) = std::fs::read_dir(&self.data_dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let project_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                // Skip backup directories and already loaded projects
                if project_name.ends_with(".bak") || self.projects.contains_key(&project_name) {
                    continue;
                }
                if let Some(doc) = self.load_ad_format(&path, &project_name) {
                    self.projects.insert(project_name, doc);
                }
            } else if path.extension() == Some("json".as_ref()) {
                let project_name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                if self.projects.contains_key(&project_name) {
                    continue;
                }
                if let Some(doc) = self.load_json_and_migrate(&path, &project_name) {
                    self.projects.insert(project_name, doc);
                }
            }
        }
        // Derive statuses for all loaded documents and persist if changed
        let names: Vec<String> = self.projects.keys().cloned().collect();
        for name in &names {
            let original = self.projects.get(name).unwrap().clone();
            let changed = {
                let doc = self.projects.get_mut(name).unwrap();
                Self::rebuild_relations(doc);
                Self::derive_statuses(doc);
                Self::doc_changed(&original, doc)
            };
            if changed {
                tracing::info!("Derived statuses changed for '{}' on startup, persisting", name);
                let doc = self.projects.get(name).unwrap();
                self.save_ad_format(doc, name);
            }
        }
        tracing::info!("Loaded {} persistent specs documents", self.projects.len());
    }

    /// Compare two documents for meaningful changes (statuses, content, items).
    fn doc_changed(a: &SpecsDocument, b: &SpecsDocument) -> bool {
        if a.sections.len() != b.sections.len() {
            return true;
        }
        for (sa, sb) in a.sections.iter().zip(b.sections.iter()) {
            if sa.status != sb.status || sa.content != sb.content || sa.title != sb.title {
                return true;
            }
            if sa.items.len() != sb.items.len() {
                return true;
            }
            for (ia, ib) in sa.items.iter().zip(sb.items.iter()) {
                if ia.status != ib.status
                    || ia.content != ib.content
                    || ia.title != ib.title
                    || ia.depends_on != ib.depends_on
                {
                    return true;
                }
            }
        }
        false
    }

    /// Reload specs from disk for all projects, derive statuses, and persist if changed.
    fn reload_changed(&mut self) {
        let names: Vec<String> = self.projects.keys().cloned().collect();
        for name in &names {
            let project_dir = if self.flat_mode_project.as_deref() == Some(name) {
                self.data_dir.clone()
            } else {
                self.data_dir.join(sanitize_filename(name))
            };

            // Detect module format to skip auto-save (derive_statuses would rewrite module files)
            let manifest_path = project_dir.join("manifest.at");
            let is_module_format = if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                content.lines().any(|l| {
                    let t = l.trim();
                    t.starts_with("module ") && !t.starts_with("module = ")
                })
            } else {
                false
            };

            // Try .ad + manifest.at format
            if let Some(mut new_doc) = self.load_ad_format(&project_dir, name) {
                Self::rebuild_relations(&mut new_doc);
                Self::derive_statuses(&mut new_doc);

                if let Some(old_doc) = self.projects.get(name) {
                    if Self::doc_changed(old_doc, &new_doc) {
                        if is_module_format {
                            tracing::info!(
                                "Specs for '{}' changed on disk (module format), reloading without auto-save",
                                name
                            );
                        } else {
                            tracing::info!(
                                "Specs for '{}' changed on disk (or derived statuses drifted), reloading and persisting",
                                name
                            );
                            self.save_ad_format(&new_doc, name);
                        }
                        self.projects.insert(name.clone(), new_doc);
                    }
                }
            }
        }
    }

    fn load_ad_format(&self, project_dir: &std::path::Path, project_name: &str) -> Option<SpecsDocument> {
        let manifest_path = project_dir.join("manifest.at");
        tracing::debug!("load_ad_format: manifest_path={}", manifest_path.display());
        let manifest_content = std::fs::read_to_string(&manifest_path).ok()?;

        // Detect new module-based format: contains "module <name>" lines
        let is_module_format = manifest_content.lines().any(|l| {
            let t = l.trim();
            t.starts_with("module ") && !t.starts_with("module = ")
        });

        if is_module_format {
            self.load_module_format(project_dir, project_name, &manifest_content)
        } else {
            // Legacy flat format: toml with [[section]] entries
            let manifest: ManifestAt = toml::from_str(&manifest_content).ok()?;
            tracing::debug!("load_ad_format: parsed legacy manifest with {} sections", manifest.sections.len());

            let mut sections = Vec::new();
            for msec in &manifest.sections {
                let ad_path = project_dir.join(format!("{}.ad", msec.id));
                tracing::debug!("load_ad_format: loading {} from {}", msec.id, ad_path.display());
                if let Ok(ad_content) = std::fs::read_to_string(&ad_path) {
                    if let Some(section) = Self::parse_ad_file(&msec.id, &msec.section_type, &msec.title, &ad_content) {
                        tracing::debug!("load_ad_format: {} parsed {} items", msec.id, section.items.len());
                        sections.push(SpecsSection {
                            id: msec.id.clone(),
                            section_type: Self::parse_section_type(&msec.section_type),
                            title: msec.title.clone(),
                            items: section.items,
                            status: Self::parse_status(&msec.status),
                            content: section.content,
                            depends_on: section.depends_on,
                            last_modified: msec.last_modified,
                            last_verified: msec.last_verified,
                        });
                    } else {
                        tracing::warn!("load_ad_format: {} parse_ad_file returned None", msec.id);
                    }
                } else {
                    tracing::warn!("load_ad_format: {} failed to read ad file", msec.id);
                }
            }
            tracing::info!("load_ad_format: loaded {} sections total", sections.len());
            Some(SpecsDocument {
                project: manifest.project,
                version: manifest.version as u64,
                sections,
            })
        }
    }

    fn load_module_format(&self, project_dir: &std::path::Path, project_name: &str, manifest_content: &str) -> Option<SpecsDocument> {
        let mut project = project_name.to_string();
        let mut modules: Vec<String> = Vec::new();

        for line in manifest_content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("project ") {
                project = rest.to_string();
            } else if let Some(rest) = trimmed.strip_prefix("module ") {
                modules.push(rest.to_string());
            }
        }

        tracing::info!("load_module_format: project='{}' modules={:?}", project, modules);

        let type_map: [(&str, &str, SectionType); 7] = [
            ("goals", "Goals", SectionType::Goals),
            ("architecture", "Architecture", SectionType::Architecture),
            ("designs", "Designs", SectionType::Designs),
            ("plans", "Plans", SectionType::Plans),
            ("tests", "Tests", SectionType::Tests),
            ("reviews", "Reviews", SectionType::Reviews),
            ("reports", "Reports", SectionType::Reports),
        ];

        let mut section_items: std::collections::HashMap<String, Vec<SpecItem>> = std::collections::HashMap::new();
        let mut section_titles: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        for module in &modules {
            let module_dir = project_dir.join(module);
            if !module_dir.is_dir() {
                tracing::warn!("load_module_format: module dir not found: {}", module_dir.display());
                continue;
            }
            for (filename, type_str, _) in &type_map {
                let ad_path = module_dir.join(format!("{}.ad", filename));
                if let Ok(content) = std::fs::read_to_string(&ad_path) {
                    if let Some(section) = Self::parse_ad_file(filename, type_str, type_str, &content) {
                        tracing::debug!(
                            "load_module_format: {}/{}.ad parsed {} items",
                            module, filename, section.items.len()
                        );
                        section_titles.insert(filename.to_string(), section.title);
                        let entry = section_items.entry(filename.to_string()).or_default();
                        entry.extend(section.items);
                        Self::dedupe_items(entry);
                    }
                }
            }
        }

        let mut sections = Vec::new();
        let now = now_secs();
        for (filename, type_str, section_type) in &type_map {
            let items = section_items.remove(*filename).unwrap_or_default();
            let title = section_titles.remove(*filename).unwrap_or_else(|| type_str.to_string());
            sections.push(SpecsSection {
                id: filename.to_string(),
                section_type: section_type.clone(),
                title,
                status: Status::InProgress,
                items,
                content: String::new(),
                depends_on: vec![],
                last_modified: now,
                last_verified: None,
            });
        }

        tracing::info!("load_module_format: loaded {} sections total", sections.len());
        Some(SpecsDocument {
            project,
            version: 2,
            sections,
        })
    }

    fn load_json_and_migrate(&self, path: &std::path::Path, project_name: &str) -> Option<SpecsDocument> {
        let content = std::fs::read_to_string(path).ok()?;
        let mut doc: SpecsDocument = serde_json::from_str(&content).ok()?;
        // Migrate to .ad + manifest.at
        tracing::info!("Migrating legacy JSON specs for '{}' to .ad + manifest.at", project_name);
        self.save_ad_format(&doc, project_name);
        // Rename old JSON to .json.bak instead of deleting
        let bak_path = path.with_extension("json.bak");
        let _ = std::fs::rename(path, bak_path);
        Some(doc)
    }

    fn parse_section_type(s: &str) -> SectionType {
        match s {
            "goals" => SectionType::Goals,
            "architecture" => SectionType::Architecture,
            "designs" => SectionType::Designs,
            "plans" => SectionType::Plans,
            "tests" => SectionType::Tests,
            "reviews" => SectionType::Reviews,
            "reports" => SectionType::Reports,
            _ => SectionType::Goals,
        }
    }

    fn parse_status(s: &str) -> Status {
        match s.to_lowercase().as_str() {
            "empty" => Status::Empty,
            "proposed" => Status::Proposed,
            "draft" => Status::Draft,
            "under_review" => Status::UnderReview,
            "approved" => Status::Approved,
            "in_progress" => Status::InProgress,
            "in_implementation" => Status::InImplementation,
            "implemented" => Status::Implemented,
            "verified" => Status::Verified,
            "done" => Status::Done,
            "archived" => Status::Archived,
            "rejected" => Status::Rejected,
            "backlog" => Status::Backlog,
            "ready" => Status::Ready,
            "in_review" => Status::InReview,
            "blocked" => Status::Blocked,
            "superseded" => Status::Superseded,
            "outdated" => Status::Outdated,
            "stable" => Status::Stable,
            "deprecated" => Status::Deprecated,
            "published" => Status::Published,
            "analysed" => Status::Analysed,
            "obsolete" => Status::Obsolete,
            _ => Status::Draft,
        }
    }

    pub(crate) fn serialize_status(status: &Status) -> String {
        match status {
            Status::Empty => "empty",
            Status::Proposed => "proposed",
            Status::Draft => "draft",
            Status::UnderReview => "under_review",
            Status::Approved => "approved",
            Status::InProgress => "in_progress",
            Status::InImplementation => "in_implementation",
            Status::Implemented => "implemented",
            Status::Verified => "verified",
            Status::Done => "done",
            Status::Archived => "archived",
            Status::Rejected => "rejected",
            Status::Backlog => "backlog",
            Status::Ready => "ready",
            Status::InReview => "in_review",
            Status::Blocked => "blocked",
            Status::Superseded => "superseded",
            Status::Outdated => "outdated",
            Status::Stable => "stable",
            Status::Deprecated => "deprecated",
            Status::Published => "published",
            Status::Analysed => "analysed",
            Status::Obsolete => "obsolete",
        }.to_string()
    }

    pub(crate) fn parse_ad_file(section_id: &str, _section_type: &str, title: &str, content: &str) -> Option<SpecsSection> {
        use regex::Regex;
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() { return None; }

        // First line should be # Title
        let title_re = Regex::new(r"^#\s+(.+)$").unwrap();
        let first_line = lines[0];
        let parsed_title = title_re.captures(first_line).map(|c| c[1].to_string()).unwrap_or_else(|| title.to_string());

        let mut section_content_lines: Vec<&str> = Vec::new();
        let mut items: Vec<SpecItem> = Vec::new();
        let mut current_item: Option<SpecItem> = None;
        let mut item_content_lines: Vec<&str> = Vec::new();
        let mut in_section_content = true;
        let mut passed_separator = false;

        let item_heading_re = Regex::new(r"^(?:##|###)\s+((?:[A-Za-z]+-)?[GADPSVXTIR]\d+(?:\.\d+)?)\s+(.+)$").unwrap();
        let meta_re = Regex::new(r"^\*\*(.+?):\*\*\s*(.*)$").unwrap();

        for line in lines.iter().skip(1) {
            let trimmed = line.trim();

            // Detect separator (--- or === or <!-- items -->)
            if in_section_content && (trimmed == "---" || trimmed == "===" || trimmed == "<!-- items -->") {
                passed_separator = true;
                continue;
            }

            // Detect item heading
            if let Some(caps) = item_heading_re.captures(line) {
                // Flush previous item
                if let Some(mut item) = current_item.take() {
                    item.content = item_content_lines.join("\n").trim().to_string();
                    items.push(item);
                    item_content_lines.clear();
                }
                in_section_content = false;
                let id = caps[1].to_string();
                let item_title = caps[2].to_string();
                current_item = Some(SpecItem {
                    id,
                    title: item_title,
                    content: String::new(),
                    status: Status::Draft,
                    depends_on: Vec::new(),
                    related: Vec::new(),
                    priority: None,
                    assignee: None,
                    test_file: None,
                    file: None,
                    milestone: None,
                    module: None,
                    tags: Vec::new(),
                    created_at: now_secs(),
                    modified_at: now_secs(),
                    completed_at: None,
                });
                continue;
            }

            // If we're inside an item, try parsing metadata
            if let Some(ref mut item) = current_item {
                if let Some(meta_caps) = meta_re.captures(line) {
                    let key = meta_caps[1].trim().to_lowercase();
                    let value = meta_caps[2].trim();
                    match key.as_str() {
                        "status" => item.status = Self::parse_status(value),
                        "priority" => item.priority = Some(value.to_string()),
                        "assignee" => item.assignee = Some(value.to_string()),
                        "test file" => item.test_file = Some(value.to_string()),
                        "file" => item.file = Some(value.to_string()),
                        "milestone" => item.milestone = Some(value.to_string()),
                        "module" => item.module = Some(value.to_string()),
                        "tags" => {
                            item.tags = value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                            // Infer module from `module:` tag if no explicit Module field was set.
                            if item.module.is_none() {
                                if let Some(mod_tag) = item.tags.iter().find(|t| t.to_lowercase().starts_with("module:")) {
                                    item.module = Some(mod_tag.split(':').nth(1).unwrap_or("").trim().to_string());
                                }
                            }
                        }
                        "depends on" => {
                            item.depends_on = value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                        }
                        _ => {}
                    }
                    continue;
                }
            }

            if in_section_content {
                section_content_lines.push(line);
            } else if current_item.is_some() {
                item_content_lines.push(line);
            }
        }

        // Flush last item
        if let Some(mut item) = current_item.take() {
            item.content = item_content_lines.join("\n").trim().to_string();
            items.push(item);
        }

        // If no items were found and no separator, treat everything as section content
        let section_content = if items.is_empty() && !passed_separator {
            content.lines().skip(1).collect::<Vec<_>>().join("\n").trim().to_string()
        } else {
            section_content_lines.join("\n").trim().to_string()
        };

        Some(SpecsSection {
            id: section_id.to_string(),
            section_type: Self::parse_section_type(section_id),
            title: parsed_title,
            items,
            status: Status::Empty,
            content: section_content,
            depends_on: Vec::new(),
            last_modified: now_secs(),
            last_verified: None,
        })
    }

    fn serialize_section_to_ad(section: &SpecsSection) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("# {}", section.title));
        lines.push(String::new());
        if !section.content.trim().is_empty() {
            lines.push(section.content.trim().to_string());
            lines.push(String::new());
        }
        if !section.items.is_empty() {
            lines.push("---".to_string());
            lines.push(String::new());
            for item in &section.items {
                lines.push(format!("## {} {}", item.id, item.title));
                lines.push(format!("**Status:** {}", Self::serialize_status(&item.status)));
                if let Some(ref p) = item.priority { lines.push(format!("**Priority:** {}", p)); }
                if let Some(ref a) = item.assignee { lines.push(format!("**Assignee:** {}", a)); }
                if let Some(ref t) = item.test_file { lines.push(format!("**Test File:** {}", t)); }
                if let Some(ref f) = item.file { lines.push(format!("**File:** {}", f)); }
                if let Some(ref m) = item.milestone { lines.push(format!("**Milestone:** {}", m)); }
                if let Some(ref m) = item.module { lines.push(format!("**Module:** {}", m)); }
                if !item.tags.is_empty() { lines.push(format!("**Tags:** {}", item.tags.join(", "))); }
                if !item.depends_on.is_empty() { lines.push(format!("**Depends on:** {}", item.depends_on.join(", "))); }
                if !item.content.trim().is_empty() {
                    lines.push(String::new());
                    lines.push(item.content.trim().to_string());
                }
                lines.push(String::new());
            }
        }
        lines.join("\n")
    }

    pub fn save_ad_format(&self, doc: &SpecsDocument, project_name: &str) {
        // In flat mode, save directly to data_dir if the project matches
        let project_dir = if self.flat_mode_project.as_deref() == Some(project_name) {
            self.data_dir.clone()
        } else {
            self.data_dir.join(sanitize_filename(project_name))
        };
        let _ = std::fs::create_dir_all(&project_dir);

        let manifest_path = project_dir.join("manifest.at");
        let is_module_format = if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            content.lines().any(|l| {
                let t = l.trim();
                t.starts_with("module ") && !t.starts_with("module = ")
            })
        } else {
            false
        };

        if is_module_format {
            self.save_module_format(doc, &project_dir);
        } else {
            // Legacy flat format
            let manifest = ManifestAt {
                project: doc.project.clone(),
                version: doc.version as u32,
                sections: doc.sections.iter().map(|s| ManifestSection {
                    id: s.id.clone(),
                    section_type: match s.section_type {
                        SectionType::Goals => "goals",
                        SectionType::Architecture => "architecture",
                        SectionType::Designs => "designs",
                        SectionType::Plans => "plans",
                        SectionType::Tests => "tests",
                        SectionType::Reviews => "reviews",
                        SectionType::Reports => "reports",
                    }.to_string(),
                    title: s.title.clone(),
                    status: Self::serialize_status(&s.status),
                    last_modified: s.last_modified,
                    last_verified: s.last_verified,
                }).collect(),
            };
            if let Ok(toml_str) = toml::to_string_pretty(&manifest) {
                let _ = std::fs::write(&manifest_path, toml_str);
            }

            for section in &doc.sections {
                let ad_path = project_dir.join(format!("{}.ad", section.id));
                let ad_content = Self::serialize_section_to_ad(section);
                let _ = std::fs::write(&ad_path, ad_content);
            }
        }
    }

    fn save_module_format(&self, doc: &SpecsDocument, project_dir: &std::path::Path) {
        let manifest_path = project_dir.join("manifest.at");
        let manifest_content = std::fs::read_to_string(&manifest_path).unwrap_or_default();

        let mut modules: Vec<String> = Vec::new();
        for line in manifest_content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("module ") {
                modules.push(rest.to_string());
            }
        }

        let type_map: [(&str, SectionType); 7] = [
            ("goals", SectionType::Goals),
            ("architecture", SectionType::Architecture),
            ("designs", SectionType::Designs),
            ("plans", SectionType::Plans),
            ("tests", SectionType::Tests),
            ("reviews", SectionType::Reviews),
            ("reports", SectionType::Reports),
        ];

        // Group items by (module, type)
        let mut grouped: std::collections::HashMap<(String, String), Vec<SpecItem>> = std::collections::HashMap::new();

        for section in &doc.sections {
            let type_name = match section.section_type {
                SectionType::Goals => "goals",
                SectionType::Architecture => "architecture",
                SectionType::Designs => "designs",
                SectionType::Plans => "plans",
                SectionType::Tests => "tests",
                SectionType::Reviews => "reviews",
                SectionType::Reports => "reports",
            };

            for item in &section.items {
                let module = Self::id_to_module(&item.id).unwrap_or_else(|| "general".to_string());
                grouped.entry((module, type_name.to_string())).or_default().push(item.clone());
            }
        }

        for module in &modules {
            let module_dir = project_dir.join(module);
            let _ = std::fs::create_dir_all(&module_dir);

            for (filename, section_type) in &type_map {
                let mut items = grouped.get(&(module.clone(), filename.to_string())).cloned().unwrap_or_default();
                Self::dedupe_items(&mut items);
                let title = match *section_type {
                    SectionType::Goals => "Goals",
                    SectionType::Architecture => "Architecture",
                    SectionType::Designs => "Designs",
                    SectionType::Plans => "Plans",
                    SectionType::Tests => "Tests",
                    SectionType::Reviews => "Reviews",
                    SectionType::Reports => "Reports",
                };

                let section = SpecsSection {
                    id: filename.to_string(),
                    section_type: section_type.clone(),
                    title: title.to_string(),
                    status: Status::InProgress,
                    items,
                    content: String::new(),
                    depends_on: vec![],
                    last_modified: now_secs(),
                    last_verified: None,
                };

                let ad_path = module_dir.join(format!("{}.ad", filename));
                let ad_content = Self::serialize_section_to_ad(&section);
                let _ = std::fs::write(&ad_path, ad_content);
            }
        }
    }

    /// Extract module name from item id, e.g. "Relay-G1" -> "relay", "UiSystem-A1" -> "ui-system"
    fn id_to_module(id: &str) -> Option<String> {
        let prefix = id.split('-').next()?;
        let mut result = String::new();
        let chars: Vec<char> = prefix.chars().collect();
        for (i, c) in chars.iter().enumerate() {
            if c.is_uppercase() && i > 0 {
                let prev_lower = chars.get(i - 1).map(|p| p.is_lowercase()).unwrap_or(false);
                let next_lower = chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false);
                if prev_lower || next_lower {
                    result.push('-');
                }
            }
            result.push(c.to_lowercase().next().unwrap());
        }
        if result.is_empty() { None } else { Some(result) }
    }

    /// Remove duplicate items by id, keeping the richest occurrence.
    ///
    /// "Richest" means the longest non-empty content. If two items have the same
    /// id, the one with more content wins; this prevents an empty or stub entry
    /// from shadowing a detailed one. The winner is placed at the first
    /// occurrence position to preserve stable ordering.
    fn dedupe_items(items: &mut Vec<SpecItem>) {
        let mut best_by_id: std::collections::HashMap<String, SpecItem> = std::collections::HashMap::new();
        let mut first_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (idx, item) in items.drain(..).enumerate() {
            let id = item.id.clone();
            first_index.entry(id.clone()).or_insert(idx);
            let keep = best_by_id.get(&id).map_or(true, |existing| {
                item.content.trim().len() > existing.content.trim().len()
            });
            if keep {
                best_by_id.insert(id, item);
            }
        }
        let mut ordered: Vec<(usize, SpecItem)> = best_by_id
            .into_iter()
            .map(|(id, item)| (*first_index.get(&id).unwrap_or(&usize::MAX), item))
            .collect();
        ordered.sort_by_key(|(idx, _)| *idx);
        items.extend(ordered.into_iter().map(|(_, item)| item));
    }

    pub(crate) fn get(&self, project: &str) -> Option<&SpecsDocument> {
        self.projects.get(project).or_else(|| {
            self.flat_mode_project.as_ref().and_then(|fp| self.projects.get(fp))
        })
    }

    pub fn get_or_default(&mut self, project: &str) -> &mut SpecsDocument {
        if !self.projects.contains_key(project) {
            // Flat mode fallback: if flat_mode_project exists and has a loaded doc, use it
            if let Some(ref fp) = self.flat_mode_project {
                if self.projects.contains_key(fp) {
                    return self.projects.get_mut(fp).unwrap();
                }
            }
            let doc = self.default_specs(project);
            self.save_ad_format(&doc, project);
            self.projects.insert(project.to_string(), doc);
        }
        // Ensure all default sections exist (backward compat: add missing sections)
        let default_doc = self.default_specs(project);
        let missing: Vec<SpecsSection> = {
            let doc = self.projects.get(project).unwrap();
            let existing_ids: std::collections::HashSet<String> =
                doc.sections.iter().map(|s| s.id.clone()).collect();
            default_doc
                .sections
                .into_iter()
                .filter(|s| !existing_ids.contains(&s.id))
                .collect()
        };
        let doc = self.projects.get_mut(project).unwrap();
        for section in missing {
            doc.sections.push(section);
        }
        doc
    }

    fn default_specs(&self, project: &str) -> SpecsDocument {
        let now = now_secs();
        SpecsDocument {
            project: project.to_string(),
            version: 1,
            sections: vec![
                SpecsSection { id: String::from("goals"), section_type: SectionType::Goals, title: String::from("🎯 Goals"), status: Status::Empty, items: vec![], content: self.load_template("goals"), depends_on: vec![], last_modified: now, last_verified: None },
                SpecsSection { id: String::from("architecture"), section_type: SectionType::Architecture, title: String::from("🏗️ Architecture"), status: Status::Empty, items: vec![], content: self.load_template("architecture"), depends_on: vec![], last_modified: now, last_verified: None },
                SpecsSection { id: String::from("designs"), section_type: SectionType::Designs, title: String::from("🎨 Designs"), status: Status::Empty, items: vec![], content: self.load_template("designs"), depends_on: vec![], last_modified: now, last_verified: None },
                SpecsSection { id: String::from("plans"), section_type: SectionType::Plans, title: String::from("📅 Plans"), status: Status::Empty, items: vec![], content: self.load_template("plans"), depends_on: vec![], last_modified: now, last_verified: None },
                SpecsSection { id: String::from("tests"), section_type: SectionType::Tests, title: String::from("🧪 Tests"), status: Status::Empty, items: vec![], content: self.load_template("tests"), depends_on: vec![], last_modified: now, last_verified: None },
                SpecsSection { id: String::from("reviews"), section_type: SectionType::Reviews, title: String::from("📝 Reviews"), status: Status::Empty, items: vec![], content: self.load_template("reviews"), depends_on: vec![], last_modified: now, last_verified: None },
                SpecsSection { id: String::from("reports"), section_type: SectionType::Reports, title: String::from("📊 Reports"), status: Status::Empty, items: vec![], content: self.load_template("reports"), depends_on: vec![], last_modified: now, last_verified: None },

            ],
        }
    }

    pub fn update_section(&mut self, project: &str, section_id: &str, content: String, status: String) -> Result<(), String> {
        let doc = self.get_or_default(project);
        if let Some(section) = doc.sections.iter_mut().find(|s| s.id == section_id) {
            section.content = content;
            section.status = match status.as_str() {
                "empty" => Status::Empty,
                "proposed" => Status::Proposed,
                "draft" => Status::Draft,
                "under_review" => Status::UnderReview,
                "approved" => Status::Approved,
                "in_progress" => Status::InProgress,
                "in_implementation" => Status::InImplementation,
                "implemented" => Status::Implemented,
                "verified" => Status::Verified,
                "done" => Status::Done,
                "archived" => Status::Archived,
                "rejected" => Status::Rejected,
                "backlog" => Status::Backlog,
                "ready" => Status::Ready,
                "in_review" => Status::InReview,
                "blocked" => Status::Blocked,
                "superseded" => Status::Superseded,
                "outdated" => Status::Outdated,
                "stable" => Status::Stable,
                "deprecated" => Status::Deprecated,
                "published" => Status::Published,
                "analysed" => Status::Analysed,
                "obsolete" => Status::Obsolete,
                _ => Status::Draft,
            };
            section.last_modified = now_secs();
            doc.version += 1;
            Self::rebuild_relations(doc);
            Self::derive_statuses(doc);
            let doc_clone = doc.clone();
            self.save(&doc_clone);
            Ok(())
        } else {
            Err(format!("Section '{}' not found", section_id))
        }
    }

    // ─── Fine-grained item operations ─────────────────────────────────────────

    pub fn upsert_spec_item(
        &mut self,
        project: &str,
        section_id: &str,
        item_id: &str,
        title: Option<&str>,
        content: Option<&str>,
        status: Option<&str>,
        priority: Option<&str>,
        assignee: Option<&str>,
        test_file: Option<&str>,
        file: Option<&str>,
        milestone: Option<&str>,
        module: Option<&str>,
        depends_on: Option<Vec<String>>,
        tags: Option<Vec<String>>,
    ) -> Result<String, String> {
        let now = now_secs();
        let doc = self.get_or_default(project);

        // Find or create section
        let section_idx = doc.sections.iter().position(|s| s.id == section_id);
        let section = if let Some(idx) = section_idx {
            &mut doc.sections[idx]
        } else {
            let new_section = SpecsSection {
                id: section_id.to_string(),
                section_type: SectionType::from_id(section_id),
                title: section_id.to_string(),
                items: vec![],
                content: String::new(),
                status: Status::Empty,
                depends_on: vec![],
                last_modified: now,
                last_verified: None,
            };
            doc.sections.push(new_section);
            doc.sections.last_mut().unwrap()
        };

        let is_new = if let Some(item) = section.items.iter_mut().find(|i| i.id == item_id) {
            if let Some(t) = title { item.title = t.to_string(); }
            if let Some(c) = content {
                if !c.trim().is_empty() || item.content.trim().is_empty() {
                    item.content = c.to_string();
                } else {
                    tracing::warn!(
                        "upsert_spec_item: refusing to overwrite non-empty content of {} with empty content",
                        item_id
                    );
                }
            }
            if let Some(s) = status { item.status = Status::from_str_lossy(s); }
            if let Some(p) = priority { item.priority = Some(p.to_string()); }
            if let Some(a) = assignee { item.assignee = Some(a.to_string()); }
            if let Some(t) = test_file { item.test_file = Some(t.to_string()); }
            if let Some(f) = file { item.file = Some(f.to_string()); }
            if let Some(m) = milestone { item.milestone = Some(m.to_string()); }
            if let Some(m) = module { item.module = Some(m.to_string()); }
            if let Some(d) = depends_on { item.depends_on = d; }
            if let Some(t) = tags { item.tags = t; }
            // Infer module from `module:` tag if no explicit Module field was set.
            if item.module.is_none() {
                if let Some(mod_tag) = item.tags.iter().find(|tag| tag.to_lowercase().starts_with("module:")) {
                    item.module = Some(mod_tag.split(':').nth(1).unwrap_or("").trim().to_string());
                }
            }
            item.modified_at = now;
            false
        } else {
            let new_item = SpecItem {
                id: item_id.to_string(),
                title: title.unwrap_or(item_id).to_string(),
                content: content.unwrap_or("").to_string(),
                status: Status::from_str_lossy(status.unwrap_or("draft")),
                depends_on: depends_on.unwrap_or_default(),
                related: vec![],
                priority: priority.map(String::from),
                assignee: assignee.map(String::from),
                test_file: test_file.map(String::from),
                file: file.map(String::from),
                milestone: milestone.map(String::from),
                module: module.map(String::from).or_else(|| {
                    tags.as_ref()?.iter().find(|t| t.to_lowercase().starts_with("module:"))
                        .map(|t| t.split(':').nth(1).unwrap_or("").trim().to_string())
                }),
                tags: tags.unwrap_or_default(),
                created_at: now,
                modified_at: now,
                completed_at: None,
            };
            section.items.push(new_item);
            true
        };

        section.last_modified = now;
        doc.version += 1;
        Self::rebuild_relations(doc);
        Self::derive_statuses(doc);
        let doc_clone = doc.clone();
        self.save(&doc_clone);

        Ok(if is_new { "created".to_string() } else { "updated".to_string() })
    }

    pub fn delete_spec_item(&mut self, project: &str, section_id: &str, item_id: &str) -> Result<String, String> {
        let doc = self.get_or_default(project);
        let section = doc.sections.iter_mut().find(|s| s.id == section_id)
            .ok_or_else(|| format!("Section '{}' not found", section_id))?;
        let old_len = section.items.len();
        section.items.retain(|i| i.id != item_id);
        if section.items.len() == old_len {
            return Ok(format!("Item '{}' not found in section '{}'", item_id, section_id));
        }
        section.last_modified = now_secs();
        doc.version += 1;
        Self::rebuild_relations(doc);
        Self::derive_statuses(doc);
        let doc_clone = doc.clone();
        self.save(&doc_clone);
        Ok("deleted".to_string())
    }

    pub fn patch_spec_item(&mut self, project: &str, section_id: &str, item_id: &str, content: &str) -> Result<String, String> {
        let doc = self.get_or_default(project);
        let section = doc.sections.iter_mut().find(|s| s.id == section_id)
            .ok_or_else(|| format!("Section '{}' not found", section_id))?;
        let item = section.items.iter_mut().find(|i| i.id == item_id)
            .ok_or_else(|| format!("Item '{}' not found in section '{}'", item_id, section_id))?;
        if !content.trim().is_empty() || item.content.trim().is_empty() {
            item.content = content.to_string();
        } else {
            tracing::warn!(
                "patch_spec_item: refusing to overwrite non-empty content of {} with empty content",
                item_id
            );
        }
        item.modified_at = now_secs();
        section.last_modified = now_secs();
        doc.version += 1;
        Self::rebuild_relations(doc);
        Self::derive_statuses(doc);
        let doc_clone = doc.clone();
        self.save(&doc_clone);
        Ok("patched".to_string())
    }

    fn update_full(&mut self, incoming: SpecsDocument) -> Result<SpecsDocument, String> {
        let project = incoming.project.clone();
        let doc = self.get_or_default(&project);
        // Simple optimistic concurrency: just overwrite for now
        // (version check can be added later)
        *doc = incoming;
        doc.version += 1;
        Self::rebuild_relations(doc);
        Self::derive_statuses(doc);
        let doc_clone = doc.clone();
        self.save(&doc_clone);
        Ok(doc_clone)
    }

    fn save(&self, doc: &SpecsDocument) {
        self.save_ad_format(doc, &doc.project);
    }

    /// Rebuild bidirectional `related` links across all items.
    /// Scans `depends_on` and content text for ID references.
    fn rebuild_relations(doc: &mut SpecsDocument) {
        use regex::Regex;
        let id_re = Regex::new(r"\b((?:[A-Za-z]+-)?[GADPSVXTIR]\d+(?:\.\d+)?)\b").unwrap();

        // Collect all item IDs for validation
        let mut all_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for section in &doc.sections {
            for item in &section.items {
                all_ids.insert(item.id.clone());
            }
        }

        // Build forward links: ref_id -> [referrer_id]
        let mut links: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for section in &doc.sections {
            for item in &section.items {
                // From depends_on
                for dep in &item.depends_on {
                    if all_ids.contains(dep) {
                        links.entry(dep.clone()).or_default().push(item.id.clone());
                    }
                }
                // From content text
                for cap in id_re.captures_iter(&item.content) {
                    let ref_id = cap[1].to_string();
                    if ref_id != item.id && all_ids.contains(&ref_id) {
                        links.entry(ref_id).or_default().push(item.id.clone());
                    }
                }
            }
        }

        // Write back
        for section in &mut doc.sections {
            for item in &mut section.items {
                item.related = links.get(&item.id).cloned().unwrap_or_default();
                item.related.sort();
                item.related.dedup();
            }
        }
    }

    /// Derive Goal and section statuses from downstream items.
    ///
    /// Rules:
    /// - Goal → Implemented: when all related Plans are Done (and Goal ≤ InProgress)
    /// - Goal → Verified: when Goal is Implemented, all related Tests are Done/Verified,
    ///                    and at least one related Review is Published
    /// - Plans section → Done: when all Plan items are Done
    /// - Goals section → Done: when all Goal items are Done
    fn derive_statuses(doc: &mut SpecsDocument) {
        // Build lookup: item_id -> (section_type_index, item_index)
        let mut item_locations: std::collections::HashMap<String, (usize, usize)> = std::collections::HashMap::new();
        for (si, section) in doc.sections.iter().enumerate() {
            for (ii, item) in section.items.iter().enumerate() {
                item_locations.insert(item.id.clone(), (si, ii));
            }
        }

        // ─── 1. Derive Goal item statuses ──────────────────────────────────────
        // Collect needed data first to avoid borrow checker issues
        let mut goal_updates: Vec<(usize, usize, Status)> = vec![];
        let goal_section_idx = doc.sections.iter().position(|s| s.section_type == SectionType::Goals);
        if let Some(gsi) = goal_section_idx {
            for gi in 0..doc.sections[gsi].items.len() {
                let goal_status = doc.sections[gsi].items[gi].status.clone();
                let related = doc.sections[gsi].items[gi].related.clone();

                // Collect statuses of related Plans, Tests, Reviews
                let mut plan_statuses: Vec<Status> = vec![];
                let mut test_statuses: Vec<Status> = vec![];
                let mut review_statuses: Vec<Status> = vec![];

                for ref_id in &related {
                    if let Some(&(si, ii)) = item_locations.get(ref_id) {
                        let item = &doc.sections[si].items[ii];
                        match doc.sections[si].section_type {
                            SectionType::Plans => plan_statuses.push(item.status.clone()),
                            SectionType::Tests => test_statuses.push(item.status.clone()),
                            SectionType::Reviews => review_statuses.push(item.status.clone()),
                            _ => {}
                        }
                    }
                }

                // Rule 1: Goal → Implemented (if all Plans are Done)
                let mut new_status = goal_status.clone();
                if !plan_statuses.is_empty()
                    && matches!(goal_status, Status::Empty | Status::Proposed | Status::Draft | Status::UnderReview | Status::Approved | Status::InProgress)
                    && plan_statuses.iter().all(|s| *s == Status::Done)
                {
                    new_status = Status::Implemented;
                }

                // Rule 2: Goal → Verified (if Implemented, all Tests done, ≥1 Review published)
                if new_status == Status::Implemented {
                    let tests_passing = test_statuses.is_empty()
                        || test_statuses.iter().all(|s| matches!(s, Status::Done | Status::Verified));
                    let has_published_review = review_statuses.iter().any(|s| *s == Status::Published);
                    if tests_passing && has_published_review {
                        new_status = Status::Verified;
                    }
                }

                if new_status != goal_status {
                    goal_updates.push((gsi, gi, new_status));
                }
            }
        }

        // Apply Goal updates
        for (gsi, gi, status) in goal_updates {
            doc.sections[gsi].items[gi].status = status;
            doc.sections[gsi].items[gi].modified_at = now_secs();
        }

        // ─── 2. Derive section statuses from items ─────────────────────────────
        for section in &mut doc.sections {
            if section.items.is_empty() {
                continue;
            }
            let derived = match section.section_type {
                SectionType::Plans => {
                    if section.items.iter().all(|i| matches!(i.status, Status::Done | Status::Implemented)) {
                        Some(Status::Done)
                    } else if section.items.iter().any(|i| matches!(i.status, Status::InProgress | Status::Implemented)) {
                        Some(Status::InProgress)
                    } else if section.items.iter().all(|i| matches!(i.status, Status::Approved | Status::Done | Status::Implemented)) {
                        Some(Status::Approved)
                    } else {
                        None
                    }
                }
                SectionType::Goals => {
                    if section.items.iter().all(|i| i.status == Status::Done) {
                        Some(Status::Done)
                    } else if section.items.iter().all(|i| i.status == Status::Verified) {
                        Some(Status::Verified)
                    } else if section.items.iter().all(|i| matches!(i.status, Status::Implemented | Status::Verified | Status::Done)) {
                        Some(Status::Implemented)
                    } else if section.items.iter().any(|i| matches!(i.status, Status::InProgress | Status::Implemented | Status::Verified | Status::Done)) {
                        Some(Status::InProgress)
                    } else {
                        None
                    }
                }
                SectionType::Tests => {
                    if section.items.iter().all(|i| matches!(i.status, Status::Done | Status::Verified)) {
                        Some(Status::Done)
                    } else if section.items.iter().any(|i| i.status == Status::Blocked) {
                        Some(Status::Blocked)
                    } else {
                        None
                    }
                }
                SectionType::Reviews => {
                    if section.items.iter().all(|i| i.status == Status::Published) {
                        Some(Status::Published)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(new_status) = derived {
                if section.status != new_status {
                    section.status = new_status;
                    section.last_modified = now_secs();
                }
            }
        }
    }
}



fn sanitize_filename(name: &str) -> String {
    name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
}

pub fn specs() -> &'static Mutex<SpecsStore> {
    static STORE: OnceLock<Mutex<SpecsStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(SpecsStore::new_default()))
}

/// Return the path of the currently open project, if any.
pub fn current_project_path() -> Option<String> {
    let store = specs().lock().ok()?;
    if !store.is_project_open() {
        return None;
    }
    store.data_dir.parent()
        .map(|p| p.to_string_lossy().to_string())
}

/// Restore the last opened project from persisted config.
pub fn restore_last_project() {
    let config = project::load_config();
    if let Some(ref path) = config.last_project_path {
        let project_path = std::path::Path::new(path);
        if project_path.exists() {
            if let Ok(mut store) = specs().lock() {
                match store.open_project(project_path) {
                    Ok(info) => tracing::info!("Restored last project: {}", info.name),
                    Err(e) => tracing::warn!("Failed to restore project '{}': {}", path, e),
                }
            }
        }
    }
}

// ─── Specs File Watcher ──────────────────────────────────────────────────────

use notify::Watcher;
use tokio::sync::mpsc::UnboundedSender;

static WATCHER_TX: std::sync::Mutex<Option<UnboundedSender<Option<PathBuf>>>> = std::sync::Mutex::new(None);

/// Start a background file watcher that reloads specs when .ad or manifest.at files change.
/// Replaces the old 5-second polling with native OS file system events.
pub fn start_specs_watcher() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Option<PathBuf>>();
    *WATCHER_TX.lock().unwrap() = Some(tx);

    tokio::spawn(async move {
        let mut _watcher: Option<notify::RecommendedWatcher> = None;
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        loop {
            tokio::select! {
                Some(path_opt) = rx.recv() => {
                    _watcher = None; // drop old watcher
                    if let Some(ref path) = path_opt {
                        let tx = event_tx.clone();
                        let new_watcher = notify::recommended_watcher(
                            move |res: Result<notify::Event, notify::Error>| {
                                if let Ok(event) = res {
                                    let is_relevant = matches!(
                                        event.kind,
                                        notify::EventKind::Modify(_) |
                                        notify::EventKind::Create(_) |
                                        notify::EventKind::Remove(_)
                                    ) && event.paths.iter().any(|p| {
                                        p.extension().map_or(false, |e| e == "ad") ||
                                        p.file_name().map_or(false, |n| n == "manifest.at")
                                    });
                                    if is_relevant {
                                        let _ = tx.send(());
                                    }
                                }
                            },
                        );
                        if let Ok(mut w) = new_watcher {
                            let _ = w.watch(path.as_ref(), notify::RecursiveMode::Recursive);
                            _watcher = Some(w);
                            tracing::info!("Watching specs directory: {}", path.display());
                        }
                    } else {
                        tracing::info!("Stopped watching specs directory");
                    }
                }
                Some(()) = event_rx.recv() => {
                    // Debounce: wait 300ms, then drain any queued events
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    while event_rx.try_recv().is_ok() {}
                    if let Ok(mut store) = specs().lock() {
                        if store.is_project_open() {
                            store.reload_changed();
                        }
                    }
                }
            }
        }
    });
}

/// Notify the watcher task to watch a new path (or stop watching).
pub fn notify_watcher_path(path: Option<PathBuf>) {
    if let Ok(tx) = WATCHER_TX.lock() {
        if let Some(ref tx) = *tx {
            let _ = tx.send(path);
        }
    }
}

// ─── Handlers ────────────────────────────────────────────────────────────────

mod handlers {
    use super::*;
    use crate::provider::AIProviderState;
    use crate::provider::{ChatMessage, ContentBlock, ToolChatEvent, ToolChatRequest};
    use crate::runtime::{delete_all_sessions, sessions_dir_for_workspace};
    use serde_json::Value;

    // ─── Health ────────────────────────────────────────────────────────────

    pub async fn health() -> Json<Value> {
        Json(serde_json::json!({"status": "ok"}))
    }

    // ─── Project Statistics ────────────────────────────────────────────────

    const SOURCE_EXTENSIONS: &[&str] = &[".rs", ".vue", ".ts", ".js", ".md", ".ad"];

    #[derive(Debug, serde::Serialize)]
    pub struct StatsResponse {
        pub source_files: u64,
        pub total_lines_of_code: u64,
        pub active_sessions: u64,
    }

    #[derive(Debug, serde::Serialize)]
    struct ErrorResponse {
        error: String,
    }

    fn count_source_stats(dir: &std::path::Path) -> (u64, u64) {
        let mut file_count = 0u64;
        let mut total_lines = 0u64;
        let Ok(entries) = std::fs::read_dir(dir) else { return (0, 0) };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if project::should_skip_entry(&name) {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                let (sub_files, sub_lines) = count_source_stats(&path);
                file_count += sub_files;
                total_lines += sub_lines;
            } else {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if SOURCE_EXTENSIONS.contains(&ext) {
                    file_count += 1;
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        total_lines += content.lines().count() as u64;
                    }
                }
            }
        }
        (file_count, total_lines)
    }

    pub async fn get_stats() -> Result<Json<StatsResponse>, (StatusCode, Json<ErrorResponse>)> {
        let Some(project_path) = current_project_path() else {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "no project open".to_string() }),
            ));
        };
        let (source_files, total_lines_of_code) = count_source_stats(std::path::Path::new(&project_path));
        let active_sessions = {
            let store = forge_sessions().lock().unwrap();
            store.list_all().iter().filter(|s| !matches!(s.status, ForgeStatus::Idle)).count() as u64
        };
        Ok(Json(StatsResponse { source_files, total_lines_of_code, active_sessions }))
    }

    // ─── Project Management ───────────────────────────────────────────────

    pub async fn get_project_status() -> Json<project::ProjectInfo> {
        let store = specs().lock().unwrap();
        if store.is_project_open() {
            let name = store.data_dir.parent()
                .and_then(|p| p.file_name())
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            Json(project::ProjectInfo {
                path: store.data_dir.parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                name,
                specs_dir: store.data_dir.to_string_lossy().to_string(),
                has_specs: !store.projects.is_empty() || store.flat_mode_project.is_some(),
                is_open: true,
                is_empty: store.data_dir.parent()
                    .map(|p| project::is_project_empty(p))
                    .unwrap_or(false),
            })
        } else {
            Json(project::ProjectInfo {
                path: String::new(),
                name: String::new(),
                specs_dir: String::new(),
                has_specs: false,
                is_open: false,
                is_empty: false,
            })
        }
    }

    #[derive(Deserialize)]
    pub struct OpenProjectBody {
        pub path: String,
    }

    pub async fn open_project(
        Json(req): Json<OpenProjectBody>,
    ) -> Result<Json<project::ProjectInfo>, (StatusCode, String)> {
        let path = PathBuf::from(&req.path);
        let mut store = specs().lock().unwrap();
        let info = store.open_project(&path)
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        project::add_recent(&req.path);
        Ok(Json(info))
    }

    pub async fn close_project() -> Json<serde_json::Value> {
        let mut store = specs().lock().unwrap();
        store.close_project();
        Json(serde_json::json!({"status": "closed"}))
    }

    pub async fn list_recent_projects() -> Json<Vec<project::RecentProject>> {
        let config = project::load_config();
        Json(config.recent_projects)
    }

    #[derive(Deserialize)]
    pub struct BrowseQuery {
        pub path: String,
    }

    pub async fn browse_directory(
        axum::extract::Query(query): axum::extract::Query<BrowseQuery>,
    ) -> Result<Json<project::BrowseResponse>, (StatusCode, String)> {
        let result = project::browse_directory(&query.path)
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        Ok(Json(result))
    }

    pub async fn project_tree() -> Result<Json<Vec<project::ProjectTreeNode>>, (StatusCode, String)> {
        let store = specs().lock().unwrap();
        if !store.is_project_open() {
            return Err((StatusCode::BAD_REQUEST, "No project open".into()));
        }
        let project_path = store.data_dir.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        drop(store);
        let tree = project::build_project_tree(&project_path);
        Ok(Json(tree))
    }

    #[derive(Deserialize)]
    pub struct FileQuery {
        pub path: String,
    }

    pub async fn read_file(
        axum::extract::Query(query): axum::extract::Query<FileQuery>,
    ) -> Result<axum::response::Response, (StatusCode, String)> {
        let store = specs().lock().unwrap();
        if !store.is_project_open() {
            return Err((StatusCode::BAD_REQUEST, "No project open".into()));
        }
        let project_path = store.data_dir.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        drop(store);

        let path = std::path::Path::new(&query.path);
        let canonical_project = std::fs::canonicalize(&project_path)
            .unwrap_or_else(|_| project_path.clone());
        let canonical_path = std::fs::canonicalize(path)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid path: {}", e)))?;

        if !canonical_path.starts_with(&canonical_project) {
            return Err((StatusCode::FORBIDDEN, "Path outside project".into()));
        }

        let data = std::fs::read(&canonical_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read file: {}", e)))?;

        let mime = match canonical_path.extension().and_then(|e| e.to_str()) {
            Some("md") => "text/markdown",
            Some("txt") => "text/plain",
            Some("pdf") => "application/pdf",
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("svg") => "image/svg+xml",
            Some("json") => "application/json",
            Some("csv") => "text/csv",
            Some("html") => "text/html",
            Some("js") => "application/javascript",
            Some("css") => "text/css",
            Some("xml") => "application/xml",
            Some("zip") => "application/zip",
            _ => "application/octet-stream",
        };

        Ok(([(header::CONTENT_TYPE, mime)], data).into_response())
    }

    pub async fn pick_folder() -> Json<Option<String>> {
        let path = tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .set_title("Select Project Folder")
                .pick_folder()
                .map(|p| p.to_string_lossy().to_string())
        })
        .await
        .ok()
        .flatten();
        Json(path)
    }

    // ─── System Prompt & Tools ───────────────────────────────────────────

    fn build_system_prompt(_focus_section: &Option<String>) -> String {
        String::from(
            "You are an AI coding assistant. \
             You can read and write files, run shell commands, search code, \
             and manage project specifications (Jades). \
             Use the tools available to help the user build software."
        )
    }

    pub async fn create_forge_session(
        Json(req): Json<CreateForgeSessionRequest>,
    ) -> Json<ForgeSession> {
        let sid = format!("forge-{}", uuid::Uuid::new_v4());
        let resolved_path = match req.project_path {
            Some(ref p) if !p.is_empty() && p != "." => p.clone(),
            _ => {
                // Fall back to the currently opened project
                let specs_guard = specs().lock().unwrap();
                specs_guard.data_dir.parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| String::from("."))
            }
        };
        let session = ForgeSession {
            id: sid.clone(),
            notebook_sid: req.notebook_sid,
            project_path: resolved_path,
            status: ForgeStatus::Idle,
            name: None,
            pending_spec_changes: vec![],
            focus_section: None,
            active_profession: None,
            errand_sessions: vec![],
            work_mode: None,
            active_task_plan: None,
            active_relay_runs: vec![],
            messages: vec![ForgeMessage {
                id: format!("m-{}", uuid::Uuid::new_v4()),
                role: String::from("system"),
                content: {
                    let relay = crate::relay::RelayRegistry::global();
                    let agent_name = relay.default_agent_for("assistant")
                        .map(|c| c.name.as_str())
                        .unwrap_or("Assistant Agent");
                    format!(
                        "You are {}, a spec-driven AI coding assistant. \
                         Help the user build software by understanding goals, \
                         proposing specs, and generating code.",
                        agent_name,
                    )
                },
                timestamp: now_secs(),
                tool_calls: None,
                profession_id: None,
            }],
        };

        {
            let mut store = forge_sessions().lock().unwrap();
            store.insert(session.clone());
            store.acquire_project_lock(&sid);
        }
        Json(session)
    }

    pub async fn get_forge_session(Path(sid): Path<String>) -> Json<Option<ForgeSession>> {
        let store = forge_sessions().lock().unwrap();
        Json(store.get(&sid).cloned())
    }

    pub async fn send_forge_message(
        Path(sid): Path<String>,
        Json(req): Json<SendMessageRequest>,
    ) -> Json<ForgeMessageResponse> {
        // Resolve effective profession: explicit > session sticky > assistant.
        // If a bring_in handoff was previously used, the session's active_profession
        // remains sticky so follow-up questions stay with the same agent.
        let effective_profession = {
            let store = forge_sessions().lock().unwrap();
            let session_prof = store.get(&sid).and_then(|s| s.active_profession.clone());
            req.profession_id.clone().or(session_prof).unwrap_or_else(|| String::from("assistant"))
        };

        let user_msg = ForgeMessage {
            id: format!("m-{}", uuid::Uuid::new_v4()),
            role: String::from("user"),
            content: req.content,
            timestamp: now_secs(),
            tool_calls: None,
            profession_id: Some(effective_profession.clone()),
        };

        forge_sessions().lock().unwrap().push_message(&sid, user_msg.clone());

        {
            let mut store = forge_sessions().lock().unwrap();
            if let Some(session) = store.get_mut(&sid) {
                session.status = ForgeStatus::Thinking;
                session.active_profession = Some(effective_profession);
                let session_clone = session.clone();
                store.save(&session_clone);
            }
        }

        let assistant_msg = ForgeMessage {
            id: format!("m-{}", uuid::Uuid::new_v4()),
            role: String::from("assistant"),
            content: String::new(),
            timestamp: now_secs(),
            tool_calls: None,
            profession_id: None,
        };

        Json(ForgeMessageResponse { message: assistant_msg })
    }

    pub async fn forge_stream(
        Path(sid): Path<String>,
        State(ai): State<AIProviderState>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let (event_tx, event_rx) =
            tokio::sync::mpsc::unbounded_channel::<Result<Event, Infallible>>();

        tokio::spawn(async move {
            use futures::FutureExt;
            let sid_for_panic = sid.clone();
            let event_tx_for_panic = event_tx.clone();
            let result = std::panic::AssertUnwindSafe(async move {
                let stream_start = std::time::Instant::now();
                let registry = crate::forge::tools::ToolRegistry::global();
            let ai_for_turns = ai.clone();
            let _provider = ai.clone();

            // Inject project/session context for Jades tools
            let (focus_section, active_profession, project_path_for_tools) = {
                let store = forge_sessions().lock().unwrap();
                match store.get(&sid) {
                    Some(session) => {
                        crate::forge::tools::set_tool_context(&session.project_path, &sid);
                        let prof = session.active_profession.clone().unwrap_or_else(|| String::from("assistant"));
                        crate::forge::tools::set_current_profession(&prof);
                        (
                            session.focus_section.clone(),
                            prof,
                            session.project_path.clone(),
                        )
                    }
                    None => {
                        let _ = event_tx.send(Ok(Event::default().data(
                            serde_json::to_string(&ForgeStreamEvent::Error {
                                message: "Session not found".to_string(),
                            })
                            .unwrap(),
                        )));
                        return;
                    }
                }
            };

            // Build conversation messages from session history
            let mut chat_messages = Vec::new();
            let t_chat_msgs = std::time::Instant::now();
            {
                let store = forge_sessions().lock().unwrap();
                if let Some(session) = store.get(&sid) {
                    let mut handled = std::collections::HashSet::new();
                    for (i, msg) in session.messages.iter().enumerate() {
                        if handled.contains(&i) {
                            continue;
                        }
                        match msg.role.as_str() {
                            "system" => {
                                // System prompt is handled separately via phase prompt
                            }
                            "user" => {
                                chat_messages.push(ChatMessage::user(&msg.content));
                            }
                            "assistant" => {
                                if let Some(ref calls) = msg.tool_calls {
                                    // In old sessions, tool results were persisted BEFORE the
                                    // assistant message. Gather preceding tool messages that
                                    // match this assistant's tool_calls and output them AFTER.
                                    let mut preceding_tools = Vec::new();
                                    let mut j = i;
                                    while j > 0 && session.messages[j - 1].role == "tool" {
                                        j -= 1;
                                        if let Some(ref tool_calls) = session.messages[j].tool_calls {
                                            if calls.iter().any(|c| tool_calls.iter().any(|tc| tc.id == c.id)) {
                                                preceding_tools.push(j);
                                            }
                                        }
                                    }

                                    let mut blocks = Vec::new();
                                    if !msg.content.is_empty() {
                                        blocks.push(ContentBlock::text(&msg.content));
                                    }
                                    for call in calls {
                                        blocks.push(ContentBlock::ToolUse {
                                            id: call.id.clone(),
                                            name: call.name.clone(),
                                            input: call.arguments.clone(),
                                        });
                                    }
                                    chat_messages.push(ChatMessage {
                                        role: "assistant".to_string(),
                                        content: blocks,
                                    });

                                    // Output matching tool results in forward order
                                    for &idx in preceding_tools.iter().rev() {
                                        handled.insert(idx);
                                        let tool_msg = &session.messages[idx];
                                        if let Some(ref tool_calls) = tool_msg.tool_calls {
                                            for call in tool_calls {
                                                if let Some(ref result) = call.result {
                                                    chat_messages.push(ChatMessage::tool_result(
                                                        &call.id, result,
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    chat_messages.push(ChatMessage::assistant_text(&msg.content));
                                }
                            }
                            "tool" => {
                                // Only output standalone tool messages (those not handled by
                                // a preceding assistant in the loop above).
                                if let Some(ref calls) = msg.tool_calls {
                                    for call in calls {
                                        if let Some(ref result) = call.result {
                                            chat_messages.push(ChatMessage::tool_result(
                                                &call.id, result,
                                            ));
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            tracing::info!(sid = %sid, elapsed_ms = t_chat_msgs.elapsed().as_millis() as u64, "forge_stream: build_chat_messages");

            // Build system prompt and tool set
            fn build_system_and_tools(
                registry: &crate::forge::tools::ToolRegistry,
                profession_id: &str,
            ) -> (String, Vec<crate::forge::tools::ToolDefinition>, Vec<String>, u32) {
                let relay = crate::relay::RelayRegistry::global();
                let agent_config = relay.default_agent_for(profession_id);
                let prompt = match agent_config.and_then(|cfg| relay.spawn_agent_from_config(cfg)) {
                    Some(agent) => agent.render_system_prompt(),
                    None => {
                        let name = agent_config
                            .map(|c| c.name.as_str())
                            .unwrap_or("Assistant");
                        format!(
                            "You are {}, an AI coding assistant.\n\
                             You can read and write files, run shell commands, search code, \
                             and manage project specifications (Jades). \
                             Use the tools available to help the user build software.",
                            name
                        )
                    }
                };
                let allowed = relay.professions.get(profession_id)
                    .map(|p| p.allowed_tools.clone())
                    .unwrap_or_default();
                let tools: Vec<_> = registry.definitions().into_iter()
                    .filter(|t| allowed.is_empty() || allowed.contains(&t.name))
                    .collect();
                let mut prompt = prompt;
                if !tools.is_empty() {
                    prompt.push_str("\n\nYou have access to tools. When you need to explore the project structure, read files, or modify code, actively use the available tools rather than asking the user to provide information.");
                    prompt.push_str("\nIf the task requires multi-step code changes across multiple files, call the `spawn_relay` tool to start a Relay Run pipeline instead of doing everything yourself.");
                }
                let max_tokens = agent_config.map(|c| c.max_tokens).unwrap_or(4096);
                (prompt, tools, allowed, max_tokens)
            }

            let t_sys = std::time::Instant::now();
            let (system_prompt, all_tools, allowed_tool_names, max_tokens) = build_system_and_tools(&registry, &active_profession);
            tracing::info!(sid = %sid, profession_id = %active_profession, elapsed_ms = t_sys.elapsed().as_millis() as u64, "forge_stream: build_system_and_tools");

            // Load thinking configuration: prefer AgentConfig, fall back to Profession defaults
            let (thinking_enabled, thinking_budget) = {
                let relay = crate::relay::RelayRegistry::global();
                if let Some(agent_cfg) = relay.default_agent_for(&active_profession) {
                    (agent_cfg.thinking_enabled, agent_cfg.thinking_budget.unwrap_or(0))
                } else {
                    relay.professions.get(&active_profession)
                        .map(|p| (p.thinking_enabled, p.thinking_budget))
                        .unwrap_or((false, 0))
                }
            };

            // ReAct loop: chat → tool_use → execute → tool_result → chat → ...
            let mut turn_count = 0;
            let max_turns = 64;
            let mut system_prompt = system_prompt;
            let mut current_profession = active_profession.clone();
            let mut all_tools = all_tools;
            let mut allowed_tool_names = allowed_tool_names;
            let mut spawn_relay_done = false;

            while turn_count < max_turns {
                turn_count += 1;
                let turn_start = std::time::Instant::now();
                let mut turn_text = String::new();

                // Re-resolve thinking config when profession changes (e.g. after bring_in)
                let (thinking_enabled, thinking_budget) = {
                    let relay = crate::relay::RelayRegistry::global();
                    if let Some(agent_cfg) = relay.default_agent_for(&current_profession) {
                        (agent_cfg.thinking_enabled, agent_cfg.thinking_budget.unwrap_or(0))
                    } else {
                        relay.professions.get(&current_profession)
                            .map(|p| (p.thinking_enabled, p.thinking_budget))
                            .unwrap_or((false, 0))
                    }
                };

                // Notify frontend that a new turn is starting (creates a new message bubble)
                let turn_start_event = Event::default().data(
                    serde_json::to_string(&ForgeStreamEvent::TurnStart {
                        profession_id: current_profession.clone(),
                    })
                    .unwrap(),
                );
                let _ = event_tx.send(Ok(turn_start_event));

                let request = ToolChatRequest {
                    messages: chat_messages.clone(),
                    tools: all_tools.clone(),
                    system_prompt: Some(system_prompt.clone()),
                    thinking_budget: if thinking_enabled {
                        Some(thinking_budget)
                    } else {
                        None
                    },
                    max_tokens: Some(max_tokens),
                };

                let (turn_tx, mut turn_rx) = tokio::sync::mpsc::unbounded_channel::<ToolChatEvent>();
                let provider_clone = ai_for_turns.clone();

                let llm_start = std::time::Instant::now();
                let mut first_token_ms: Option<u64> = None;
                let turn_task = tokio::spawn(async move {
                    provider_clone.chat_turn(request, turn_tx).await
                });

                let mut got_tool_use = false;
                turn_text.clear();
                let mut turn_tool_calls: Vec<ToolCallInfo> = Vec::new();

                while let Some(event) = turn_rx.recv().await {
                    match event {
                        ToolChatEvent::TextDelta { text } => {
                            if first_token_ms.is_none() {
                                first_token_ms = Some(llm_start.elapsed().as_millis() as u64);
                            }
                            turn_text.push_str(&text);
                            let event = Event::default().data(
                                serde_json::to_string(&ForgeStreamEvent::Delta {
                                    text: text.clone(),
                                })
                                .unwrap(),
                            );
                            let _ = event_tx.send(Ok(event));
                        }
                        ToolChatEvent::ThinkingDelta { thinking } => {
                            let event = Event::default().data(
                                serde_json::to_string(&ForgeStreamEvent::Thinking {
                                    thinking: thinking.clone(),
                                })
                                .unwrap(),
                            );
                            let _ = event_tx.send(Ok(event));
                        }
                        ToolChatEvent::ToolUse { id, name, input } => {
                            got_tool_use = true;
                            let input_clone = input.clone();
                            let call = ToolCallInfo {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: input_clone.clone(),
                                result: None,
                                status: "running".to_string(),
                            };
                            turn_tool_calls.push(call.clone());

                            // Notify frontend about the tool call
                            let event = Event::default().data(
                                serde_json::to_string(&ForgeStreamEvent::ToolCall {
                                    id: id.clone(),
                                    name: name.clone(),
                                    arguments: input_clone.clone(),
                                })
                                .unwrap(),
                            );
                            let _ = event_tx.send(Ok(event));

                            // Execute the tool (re-inject context in case task migrated threads)
                            crate::forge::tools::set_tool_context(&project_path_for_tools, &sid);
                            crate::forge::tools::set_current_profession(&current_profession);
                            if !allowed_tool_names.is_empty() && !allowed_tool_names.contains(&name) {
                                // LLM hallucinated a tool call outside allowed set — skip execution
                                let err = format!("Tool '{}' is not available for this agent", name);
                                if let Some(c) = turn_tool_calls.iter_mut().find(|c| c.id == id) {
                                    c.result = Some(err.clone());
                                    c.status = "error".to_string();
                                }

                                // Notify frontend about the error
                                let event = Event::default().data(
                                    serde_json::to_string(&ForgeStreamEvent::ToolResult {
                                        id: id.clone(),
                                        result: err,
                                    })
                                    .unwrap(),
                                );
                                let _ = event_tx.send(Ok(event));
                            } else if let Some(tool) = registry.get(&name) {
                                let t_tool = std::time::Instant::now();
                                let is_heavy = matches!(name.as_str(), "shell" | "search" | "list_symbols");
                                let tool_input = input.clone();
                                let tool_result = if is_heavy {
                                    let tool_name = name.clone();
                                    let tool_name_err = tool_name.clone();
                                    tokio::task::spawn_blocking(move || {
                                        crate::forge::tools::ToolRegistry::global()
                                            .get(&tool_name)
                                            .map(|t| t.execute(tool_input))
                                    })
                                    .await
                                    .ok()
                                    .flatten()
                                    .unwrap_or_else(|| Err(crate::forge::tools::ToolError::ExecutionFailed(
                                        format!("Tool '{}' failed or panicked", tool_name_err)
                                    )))
                                } else {
                                    tool.execute(input)
                                };
                                let tool_elapsed_ms = t_tool.elapsed().as_millis() as u64;
                                tracing::info!(sid = %sid, profession_id = %current_profession, tool_name = %name, elapsed_ms = tool_elapsed_ms, "forge_stream: tool_execute");
                                let mut result_str = match tool_result {
                                    Ok(r) => r,
                                    Err(e) => format!("Error: {}", e),
                                };

                                // ── Handle dispatch tool result ──
                                if name == "dispatch" {
                                    if let Ok(dispatch_data) = serde_json::from_str::<serde_json::Value>(&result_str) {
                                        if dispatch_data.get("dispatch").and_then(|v| v.as_bool()) == Some(true) {
                                            let agent = dispatch_data["agent"].as_str().unwrap_or("gofer");
                                            let task = dispatch_data["task"].as_str().unwrap_or("");
                                            let context = dispatch_data["context"].as_str();
                                            let max_turns = dispatch_data.get("max_turns").and_then(|v| v.as_u64()).unwrap_or(80) as u32;

                                            let mut errand = crate::forge::errand::ErrandSession::new(
                                                sid.clone(),
                                                agent.to_string(),
                                                task.to_string(),
                                                context.map(|s| s.to_string()),
                                                max_turns,
                                                id.clone(),
                                            );

                                            match errand.run_sync(ai_for_turns.clone(), &registry, &project_path_for_tools, Some(event_tx.clone())) {
                                                Ok(errand_result) => {
                                                    result_str = errand_result;
                                                }
                                                Err(e) => {
                                                    result_str = format!("Error running errand: {}", e);
                                                }
                                            }

                                            // Persist errand to session
                                            {
                                                let mut store = forge_sessions().lock().unwrap();
                                                if let Some(session) = store.get_mut(&sid) {
                                                    session.errand_sessions.push(errand);
                                                    let clone = session.clone();
                                                    store.save(&clone);
                                                }
                                            }
                                        }
                                    }
                                }

                                // ── Handle spawn_relay tool result ──
                                if name == "spawn_relay" {
                                    if let Ok(relay_data) = serde_json::from_str::<serde_json::Value>(&result_str) {
                                        if relay_data.get("relay_spawned").and_then(|v| v.as_bool()) == Some(true) {
                                            let run_id = relay_data["run_id"].as_str().unwrap_or("").to_string();
                                            let flow_id = relay_data["flow_id"].as_str().unwrap_or("standard").to_string();
                                            let mode_str = relay_data["mode"].as_str().unwrap_or("gsd").to_string();
                                            let task = relay_data["task"].as_str().unwrap_or("").to_string();

                                            // Build FlowSpec
                                            let resolved_flow_id = match flow_id.as_str() {
                                                "standard" => "standard-spec-driven-development",
                                                "post_discovery" => "post-discovery",
                                                "fast_track" => "fast-track",
                                                "bug_fix" => "bug-fix",
                                                "goal_discovery" => "goal-discovery",
                                                "doc_patch" => "doc-patch",
                                                "spec_tweak" => "spec-tweak",
                                                other => other,
                                            };
                                            let flow = crate::relay::flows::get_flow(resolved_flow_id)
                                                .unwrap_or_else(|| {
                                                    tracing::warn!("Flow '{}' not found, falling back to standard", resolved_flow_id);
                                                    crate::relay::flows::get_flow("standard-spec-driven-development")
                                                        .expect("standard-spec-driven-development built-in flow must exist")
                                                });

                                            // Set mode on flow engine
                                            let mode = match mode_str.as_str() {
                                                "check" => crate::relay::pipeline::RelayMode::Check,
                                                _ => crate::relay::pipeline::RelayMode::GSD,
                                            };

                                            // Get project path from session
                                            let project_path = {
                                                let store = forge_sessions().lock().unwrap();
                                                store.get(&sid)
                                                    .map(|s| s.project_path.clone())
                                                    .filter(|p| !p.is_empty())
                                            };

                                            // Persist work mode and active relay run
                                            {
                                                let mut store = forge_sessions().lock().unwrap();
                                                if let Some(session) = store.get_mut(&sid) {
                                                    session.work_mode = Some(crate::forge::WorkMode::SingleRelay);
                                                    session.active_relay_runs.push(run_id.clone());
                                                    let clone = session.clone();
                                                    store.save(&clone);
                                                }
                                            }

                                            // Start run in store
                                            let run_store = crate::relay::api::run_store();
                                            let _ = crate::relay::store::start_run(run_store, flow, &run_id, project_path.clone());

                                            // Store title and original task for resumption
                                            {
                                                let mut map = run_store.lock().unwrap();
                                                if let Some(entry) = map.get_mut(&run_id) {
                                                    entry.metadata.title = Some(crate::relay::title::generate_title(&task));
                                                    entry.metadata.initial_task = Some(task.clone());
                                                    entry.engine.mode = mode;
                                                    crate::relay::store::save_run(entry);
                                                }
                                            }

                                            // Spawn background driver
                                            tokio::spawn(crate::relay::driver::drive_run(
                                                run_id.clone(),
                                                run_store.clone(),
                                                crate::relay::api::event_sender(),
                                                ai_for_turns.clone(),
                                                task,
                                                project_path.unwrap_or_default(),
                                            ));

                                            // Emit to chat SSE
                                            let event = Event::default().data(
                                                serde_json::to_string(&ForgeStreamEvent::RelaySpawned {
                                                    run_id: run_id.clone(),
                                                    flow_id: flow_id.clone(),
                                                    status: "started".into(),
                                                }).unwrap(),
                                            );
                                            let _ = event_tx.send(Ok(event));

                                            // Mark spawn_relay as done so we break the ReAct loop
                                            spawn_relay_done = true;

                                            // Replace tool result with a friendly message that tells LLM to stop
                                            result_str = format!(
                                                "SUCCESS: Relay pipeline '{}' has been started (flow: {}, mode: {}). \
                                                 DO NOT call spawn_relay again. The conversation will now pause while the relay pipeline runs.",
                                                run_id, flow_id, mode_str
                                            );
                                        }
                                    }
                                }

                                // ── Handle spawn_task_plan tool result ──
                                if name == "spawn_task_plan" {
                                    if let Ok(tp_data) = serde_json::from_str::<serde_json::Value>(&result_str) {
                                        if tp_data.get("task_plan_spawned").and_then(|v| v.as_bool()) == Some(true) {
                                            let instance_id = tp_data["instance_id"].as_str().unwrap_or("").to_string();
                                            let task_plan_id = tp_data["task_plan_id"].as_str().unwrap_or("").to_string();
                                            let initial_input = tp_data["initial_input"].as_str().unwrap_or("").to_string();

                                            let project_path = {
                                                let store = forge_sessions().lock().unwrap();
                                                store.get(&sid)
                                                    .map(|s| s.project_path.clone())
                                                    .filter(|p| !p.is_empty())
                                                    .unwrap_or_default()
                                            };

                                            // Persist work mode and active TaskPlan
                                            {
                                                let mut store = forge_sessions().lock().unwrap();
                                                if let Some(session) = store.get_mut(&sid) {
                                                    session.work_mode = Some(crate::forge::WorkMode::MultiRelay);
                                                    session.active_task_plan = Some(instance_id.clone());
                                                    let clone = session.clone();
                                                    store.save(&clone);
                                                }
                                            }

                                            // Resolve the TaskPlan and start the engine
                                            if let Some(plan) = crate::relay::task_plan_registry::get_task_plan(&task_plan_id) {
                                                let mut engine = crate::relay::task_plan_engine::TaskPlanEngine::new(
                                                    plan,
                                                    project_path.clone(),
                                                    initial_input,
                                                );
                                                let ctx = crate::relay::task_plan_engine::TaskPlanContext {
                                                    run_store: crate::relay::api::run_store().clone(),
                                                    handoff_store: std::sync::Arc::new(crate::relay::handoff_store::HandoffStore::new(std::path::PathBuf::from(&project_path))),
                                                    event_tx: crate::relay::api::event_sender(),
                                                    ai_provider: Some(ai_for_turns.clone()),
                                                    project_path: project_path.clone(),
                                                };

                                                tokio::spawn(async move {
                                                    let _ = engine.execute(&ctx, |req| {
                                                        crate::relay::task_plan_engine::drive_task_plan_run(&ctx, req)
                                                    }).await;
                                                });

                                                // Emit to chat SSE
                                                let event = Event::default().data(
                                                    serde_json::to_string(&ForgeStreamEvent::TaskPlanSpawned {
                                                        instance_id: instance_id.clone(),
                                                        task_plan_id: task_plan_id.clone(),
                                                        status: "started".into(),
                                                    }).unwrap(),
                                                );
                                                let _ = event_tx.send(Ok(event));
                                            } else {
                                                result_str = format!("Error: TaskPlan '{}' not found", task_plan_id);
                                            }

                                            if !result_str.starts_with("Error:") {
                                                // Replace tool result with a friendly message
                                                result_str = format!(
                                                    "SUCCESS: TaskPlan '{}' has been started (instance: {}). \
                                                     DO NOT call spawn_task_plan again. The conversation will now pause while the TaskPlan runs.",
                                                    task_plan_id, instance_id
                                                );
                                                spawn_relay_done = true;
                                            }
                                        }
                                    }
                                }

                                // Update call with result
                                if let Some(c) = turn_tool_calls.iter_mut().find(|c| c.id == id) {
                                    c.result = Some(result_str.clone());
                                    c.status = "success".to_string();
                                }

                                // Notify frontend about the result
                                let event = Event::default().data(
                                    serde_json::to_string(&ForgeStreamEvent::ToolResult {
                                        id: id.clone(),
                                        result: result_str.clone(),
                                    })
                                    .unwrap(),
                                );
                                let _ = event_tx.send(Ok(event));

                                // ── Handle bring_in tool result ──
                                if name == "bring_in" {
                                    if let Ok(handoff_data) = serde_json::from_str::<serde_json::Value>(&result_str) {
                                        if handoff_data.get("handoff").and_then(|v| v.as_bool()) == Some(true) {
                                            let target = handoff_data["target"].as_str().unwrap_or("").to_string();
                                            let classification = handoff_data["classification"].as_str().unwrap_or("DIRECT").to_string();
                                            let reason = handoff_data["reason"].as_str().unwrap_or("").to_string();
                                            let from_profession = handoff_data["from_profession"].as_str().unwrap_or("").to_string();

                                            // Resolve display names for the event
                                            let relay = crate::relay::RelayRegistry::global();
                                            let from_name = relay.default_agent_for(&from_profession)
                                                .map(|c| c.name.clone())
                                                .unwrap_or_else(|| from_profession.clone());
                                            let to_name = relay.default_agent_for(&target)
                                                .map(|c| c.name.clone())
                                                .unwrap_or_else(|| target.clone());

                                            // Switch active_profession on the session AND extract research summary
                                            let research_summary = {
                                                let mut store = forge_sessions().lock().unwrap();
                                                if let Some(session) = store.get_mut(&sid) {
                                                    session.active_profession = Some(target.clone());

                                                    // Extract recent research from messages to pass to the next agent
                                                    let mut research_parts = Vec::new();
                                                    for msg in session.messages.iter().rev().take(20) {
                                                        if let Some(ref tool_calls) = msg.tool_calls {
                                                            for tc in tool_calls {
                                                                if let Some(ref result) = tc.result {
                                                                    match tc.name.as_str() {
                                                                        "read_file" | "search_code" | "dispatch" => {
                                                                            let snippet = if result.chars().count() > 600 {
                                                                                let end = result.char_indices().nth(600).map(|(i, _)| i).unwrap_or(result.len());
                                                                                format!("{}... (truncated)", &result[..end])
                                                                            } else {
                                                                                result.clone()
                                                                            };
                                                                            let args_preview = tc.arguments.to_string();
                                                                            let args_short = if args_preview.chars().count() > 120 {
                                                                                let end = args_preview.char_indices().nth(120).map(|(i, _)| i).unwrap_or(args_preview.len());
                                                                                format!("{}...", &args_preview[..end])
                                                                            } else {
                                                                                args_preview
                                                                            };
                                                                            research_parts.push(format!(
                                                                                "- [{}] {} => {}",
                                                                                tc.name, args_short, snippet
                                                                            ));
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    let clone = session.clone();
                                                    store.save(&clone);
                                                    if research_parts.is_empty() {
                                                        None
                                                    } else {
                                                        // Deduplicate and limit
                                                        research_parts.dedup();
                                                        let limited: Vec<String> = research_parts.into_iter().take(8).collect();
                                                        Some(limited.join("\n"))
                                                    }
                                                } else {
                                                    None
                                                }
                                            };

                                            // Inject handoff note into chat history for the next agent
                                            let research_section = research_summary
                                                .map(|s| format!(
                                                    "\n\n## Research Already Done (DO NOT repeat)\n\n{}\n\n\
                                                     **Use the information above. Proceed directly to editing or writing files.**",
                                                    s
                                                ))
                                                .unwrap_or_default();

                                            let note = format!(
                                                "## Handoff Note\n\
                                                 {} ({}) handed off to you.\n\
                                                 **Classification:** {}\n\
                                                 **Summary:** {}\n\
                                                 You are now the active agent. Do NOT call bring_in — you are already here.\n\
                                                 Continue the conversation from here. Do not ask the user to repeat what was already discussed.{}",
                                                from_name, from_profession, classification, reason, research_section
                                            );
                                            chat_messages.push(ChatMessage::user(&note));

                                            // Update tool context for subsequent tools
                                            crate::forge::tools::set_current_profession(&target);

                                            let target_for_prompt = target.clone();

                                            // Emit agent_handoff SSE event
                                            let handoff_event = Event::default().data(
                                                serde_json::to_string(&ForgeStreamEvent::AgentHandoff {
                                                    from_agent: from_name,
                                                    from_profession: from_profession,
                                                    to_profession: target.clone(),
                                                    to_agent: to_name,
                                                    classification,
                                                    reason,
                                                })
                                                .unwrap(),
                                            );
                                            let _ = event_tx.send(Ok(handoff_event));

                                            // Persist handoff so subsequent user messages stay with this agent
                                            {
                                                let mut store = forge_sessions().lock().unwrap();
                                                if let Some(session) = store.get_mut(&sid) {
                                                    session.active_profession = Some(target_for_prompt.clone());
                                                    let clone = session.clone();
                                                    store.save(&clone);
                                                }
                                            }

                                            // Rebuild system prompt and tools for the new agent
                                            current_profession = target_for_prompt.clone();
                                            let (new_prompt, new_tools, new_allowed, new_max_tokens) = build_system_and_tools(&registry, &target_for_prompt);
                                            system_prompt = new_prompt;
                                            all_tools = new_tools;
                                            allowed_tool_names = new_allowed;
                                            let _ = new_max_tokens; // max_tokens is set per-turn via the request

                                            // Reset turn count so the incoming agent gets a full budget
                                            turn_count = 0;
                                        }
                                    }
                                }
                            }
                        }
                        ToolChatEvent::Usage { input_tokens, output_tokens } => {
                            // Token usage is tracked per-turn; could be accumulated on session if needed
                            let _ = (input_tokens, output_tokens);
                        }
                        ToolChatEvent::Done => break,
                        ToolChatEvent::Error { message } => {
                            let event = Event::default().data(
                                serde_json::to_string(&ForgeStreamEvent::Error { message })
                                    .unwrap(),
                            );
                            let _ = event_tx.send(Ok(event));
                            break;
                        }
                    }
                }

                // Check for turn errors
                if let Ok(Some(err)) = turn_task.await {
                    let event = Event::default().data(
                        serde_json::to_string(&ForgeStreamEvent::Error { message: err }).unwrap(),
                    );
                    let _ = event_tx.send(Ok(event));
                    break;
                }

                let llm_elapsed_ms = llm_start.elapsed().as_millis() as u64;
                let turn_elapsed_ms = turn_start.elapsed().as_millis() as u64;
                tracing::info!(
                    sid = %sid,
                    profession_id = %current_profession,
                    turn = turn_count,
                    llm_elapsed_ms = llm_elapsed_ms,
                    first_token_ms = first_token_ms.unwrap_or(0),
                    turn_elapsed_ms = turn_elapsed_ms,
                    text_len = turn_text.len(),
                    tool_calls = turn_tool_calls.len(),
                    "forge_stream: turn_complete"
                );

                // Persist assistant message first, then tool results
                // (Anthropic API requires assistant with tool_use BEFORE user with tool_result)
                if !turn_text.is_empty() || !turn_tool_calls.is_empty() {
                    let assistant_msg = ForgeMessage {
                        id: format!("m-{}", uuid::Uuid::new_v4()),
                        role: "assistant".to_string(),
                        content: turn_text.clone(),
                        timestamp: now_secs(),
                        tool_calls: if turn_tool_calls.is_empty() {
                            None
                        } else {
                            Some(turn_tool_calls.clone())
                        },
                        profession_id: Some(current_profession.clone()),
                    };
                    forge_sessions().lock().unwrap().push_message(&sid, assistant_msg.clone());

                    // Add assistant message to chat_messages for next turn continuity
                    if got_tool_use {
                        let mut blocks = Vec::new();
                        if !turn_text.is_empty() {
                            blocks.push(ContentBlock::text(&turn_text));
                        }
                        for call in &turn_tool_calls {
                            blocks.push(ContentBlock::ToolUse {
                                id: call.id.clone(),
                                name: call.name.clone(),
                                input: call.arguments.clone(),
                            });
                        }
                        chat_messages.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: blocks,
                        });
                    }
                }

                // Persist tool result messages after assistant message
                for call in &turn_tool_calls {
                    if let Some(ref result) = call.result {
                        let tool_msg = ForgeMessage {
                            id: format!("m-{}", uuid::Uuid::new_v4()),
                            role: "tool".to_string(),
                            content: result.clone(),
                            timestamp: now_secs(),
                            tool_calls: Some(vec![call.clone()]),
                            profession_id: None,
                        };
                        forge_sessions().lock().unwrap().push_message(&sid, tool_msg);

                        // Add tool result to chat_messages for next turn
                        chat_messages.push(ChatMessage::tool_result(&call.id, result));
                    }
                }

                // If spawn_relay was called, stop the loop to prevent duplicate runs
                if spawn_relay_done {
                    break;
                }

                // If no tool_use was requested, we're done
                if !got_tool_use {
                    break;
                }
            }

            // Persist the work mode after first classification if it wasn't set by a spawn tool.
            {
                let mut store = forge_sessions().lock().unwrap();
                if let Some(session) = store.get_mut(&sid) {
                    if session.work_mode.is_none() {
                        session.work_mode = Some(crate::forge::WorkMode::Direct);
                        let clone = session.clone();
                        store.save(&clone);
                    }
                }
            }

            // After turn completes, set session back to Idle.
            // Note: active_profession is intentionally NOT reset here — if a bring_in
            // handoff occurred, the user likely wants the same agent to continue
            // answering follow-up questions. The profession persists until the user
            // explicitly switches agents via the UI or starts a new topic.
            {
                let mut store = forge_sessions().lock().unwrap();
                store.update_status(&sid, ForgeStatus::Idle);
            }

            // Final done event
            let event = Event::default().data(
                serde_json::to_string(&ForgeStreamEvent::Done).unwrap(),
            );
            let _ = event_tx.send(Ok(event));
            tracing::info!(sid = %sid, elapsed_ms = stream_start.elapsed().as_millis() as u64, turns = turn_count, "forge_stream: complete");
            }).catch_unwind().await;

            if let Err(panic_payload) = result {
                let msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "Unknown internal server error".to_string()
                };
                let event = Event::default().data(
                    serde_json::to_string(&ForgeStreamEvent::Error { message: format!("Server panic: {}", msg) }).unwrap(),
                );
                let _ = event_tx_for_panic.send(Ok(event));
                let event = Event::default().data(
                    serde_json::to_string(&ForgeStreamEvent::Done).unwrap(),
                );
                let _ = event_tx_for_panic.send(Ok(event));
                let mut store = forge_sessions().lock().unwrap();
                store.update_status(&sid_for_panic, ForgeStatus::Idle);
            }
        });

        let sse_stream = stream::unfold(event_rx, |mut rx| async move {
            rx.recv().await.map(|event| (event, rx))
        });

        Sse::new(sse_stream).keep_alive(KeepAlive::default())
    }

    pub async fn forge_history(Path(sid): Path<String>) -> Json<Vec<ForgeMessage>> {
        let store = forge_sessions().lock().unwrap();
        let messages = store
            .get(&sid)
            .map(|s| s.messages.clone())
            .unwrap_or_default();
        Json(messages)
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ForgeSessionSummary {
        pub id: String,
        pub status: ForgeStatus,
        pub focus_section: Option<String>,
        pub name: Option<String>,
        pub preview: String,
        pub message_count: usize,
        pub last_activity: u64,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct RenameForgeSessionRequest {
        pub name: String,
    }

    pub async fn rename_forge_session(
        Path(sid): Path<String>,
        Json(req): Json<RenameForgeSessionRequest>,
    ) -> StatusCode {
        let mut store = forge_sessions().lock().unwrap();
        if store.rename(&sid, req.name) {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        }
    }

    pub async fn delete_forge_session(Path(sid): Path<String>) -> StatusCode {
        let mut store = forge_sessions().lock().unwrap();
        if store.remove(&sid) {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DeleteAllSessionsResponse {
        pub deleted_count: usize,
        pub new_session_id: String,
        pub session: ForgeSessionSummary,
    }

    pub async fn delete_all_forge_sessions() -> Json<DeleteAllSessionsResponse> {
        let mut store = forge_sessions().lock().unwrap();
        let deleted_count = store.list_all().len();
        store.clear();

        // Clean up session files on disk
        if let Some(project_path) = current_project_path() {
            let sessions_dir = sessions_dir_for_workspace(&project_path);
            let _ = delete_all_sessions(&sessions_dir);
        }

        // Create a new blank session
        let new_sid = format!("forge-{}", uuid::Uuid::new_v4());
        let now = now_secs();
        let new_session = ForgeSession {
            id: new_sid.clone(),
            notebook_sid: None,
            project_path: String::new(),
            status: ForgeStatus::Idle,
            name: None,
            pending_spec_changes: vec![],
            focus_section: None,
            active_profession: None,
            errand_sessions: vec![],
            work_mode: None,
            active_task_plan: None,
            active_relay_runs: vec![],
            messages: vec![],
        };
        store.insert(new_session);

        let summary = ForgeSessionSummary {
            id: new_sid.clone(),
            status: ForgeStatus::Idle,
            focus_section: None,
            name: None,
            preview: String::from("New session"),
            message_count: 0,
            last_activity: now,
        };

        Json(DeleteAllSessionsResponse {
            deleted_count,
            new_session_id: new_sid,
            session: summary,
        })
    }

    pub async fn list_forge_sessions() -> Json<Vec<ForgeSessionSummary>> {
        let store = forge_sessions().lock().unwrap();
        let mut summaries: Vec<ForgeSessionSummary> = store
            .list_all()
            .iter()
            .map(|s| {
                let preview = s
                    .messages
                    .iter()
                    .find(|m| m.role == "user")
                    .map(|m| {
                        let content = m.content.trim();
                        let char_count = content.chars().count();
                        if char_count > 60 {
                            let truncated: String = content.chars().take(60).collect();
                            format!("{}…", truncated)
                        } else {
                            content.to_string()
                        }
                    })
                    .unwrap_or_else(|| String::from("New session"));

                let last_activity = s
                    .messages
                    .last()
                    .map(|m| m.timestamp)
                    .unwrap_or(0);

                ForgeSessionSummary {
                    id: s.id.clone(),
                    status: s.status.clone(),
                    focus_section: s.focus_section.clone(),
                    name: s.name.clone(),
                    preview,
                    message_count: s.messages.len(),
                    last_activity,
                }
            })
            .collect();

        // Sort by most recent activity first
        summaries.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        Json(summaries)
    }

    // ─── Specs Handlers ─────────────────────────────────────────────────

    pub async fn get_specs(Path(project): Path<String>) -> Json<SpecsDocument> {
        let mut store = specs().lock().unwrap();
        let keys: Vec<String> = store.projects.keys().cloned().collect();
        let debug_info = format!(
            "get_specs: project='{}', keys={:?}\n",
            project, keys
        );
        let _ = std::fs::write("D:/autostack/auto-forge/specs/debug_get_specs.txt", &debug_info);
        let doc = store.get_or_default(&project).clone();
        let mut debug = debug_info;
        debug.push_str(&format!("doc sections: {}\n", doc.sections.len()));
        for sec in &doc.sections {
            debug.push_str(&format!("  {}: {} items, content_len={}\n", sec.id, sec.items.len(), sec.content.len()));
        }
        let _ = std::fs::write("D:/autostack/auto-forge/specs/debug_get_specs.txt", &debug);
        Json(doc)
    }

    pub async fn update_specs(
        Path(project): Path<String>,
        Json(doc): Json<SpecsDocument>,
    ) -> Result<Json<SpecsDocument>, String> {
        let mut store = specs().lock().unwrap();
        // Ensure the project matches the URL
        if doc.project != project {
            return Err("Project mismatch".to_string());
        }
        let updated = store.update_full(doc)?;
        Ok(Json(updated))
    }

    pub async fn get_specs_overview(Path(project): Path<String>) -> Json<serde_json::Value> {
        let store = specs().lock().unwrap();
        let project_dir = store.data_dir.join(&project);
        let overview_path = project_dir.join("overview.ad");
        if let Ok(content) = std::fs::read_to_string(&overview_path) {
            Json(serde_json::json!({ "content": content, "exists": true }))
        } else {
            Json(serde_json::json!({ "content": "", "exists": false }))
        }
    }

    pub async fn get_module_outline(
        Path((project, module)): Path<(String, String)>,
    ) -> Json<serde_json::Value> {
        let store = specs().lock().unwrap();
        let project_dir = store.data_dir.join(&project);
        let module_dir = project_dir.join(sanitize_filename(&module));
        let outline_path = module_dir.join("module.ad");
        if let Ok(content) = std::fs::read_to_string(&outline_path) {
            Json(serde_json::json!({ "content": content, "exists": true }))
        } else {
            Json(serde_json::json!({ "content": "", "exists": false }))
        }
    }

    pub async fn get_specs_section(
        Path((project, section_id)): Path<(String, String)>,
    ) -> Json<Option<SpecsSection>> {
        let store = specs().lock().unwrap();
        let section = store
            .get(&project)
            .and_then(|d| d.sections.iter().find(|s| s.id == section_id).cloned());
        Json(section)
    }

    pub async fn update_specs_section(
        Path((project, section_id)): Path<(String, String)>,
        Json(body): Json<serde_json::Value>,
    ) -> Result<Json<serde_json::Value>, String> {
        let content = body
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let status = body
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("draft")
            .to_string();

        let mut store = specs().lock().unwrap();
        store.update_section(&project, &section_id, content, status)?;
        Ok(Json(serde_json::json!({"status": "ok"})))
    }

    pub async fn get_related_items(
        Path((project, item_id)): Path<(String, String)>,
    ) -> Json<serde_json::Value> {
        let store = specs().lock().unwrap();
        let doc = store.get(&project);

        let mut parents: Vec<serde_json::Value> = vec![];
        let mut children: Vec<serde_json::Value> = vec![];

        if let Some(doc) = doc {
            // Find the target item
            let mut target_item: Option<&SpecItem> = None;
            for section in &doc.sections {
                if let Some(item) = section.items.iter().find(|i| i.id == item_id) {
                    target_item = Some(item);
                    break;
                }
            }

            if let Some(target) = target_item {
                // Parents = items referenced by target's depends_on
                for dep_id in &target.depends_on {
                    for section in &doc.sections {
                        if let Some(item) = section.items.iter().find(|i| &i.id == dep_id) {
                            parents.push(serde_json::json!({
                                "id": item.id,
                                "title": item.title,
                                "section_type": section.section_type.as_str(),
                                "status": item.status.as_str(),
                            }));
                        }
                    }
                }
                // Children = items that have target in their related
                for section in &doc.sections {
                    for item in &section.items {
                        if item.related.contains(&item_id) {
                            children.push(serde_json::json!({
                                "id": item.id,
                                "title": item.title,
                                "section_type": section.section_type.as_str(),
                                "status": item.status.as_str(),
                            }));
                        }
                    }
                }
            }
        }

        Json(serde_json::json!({
            "id": item_id,
            "parents": parents,
            "children": children,
        }))
    }

    pub async fn rebuild_relations_endpoint(
        Path(project): Path<String>,
    ) -> Result<Json<SpecsDocument>, String> {
        let mut store = specs().lock().unwrap();
        let doc = store.get_or_default(&project);
        SpecsStore::rebuild_relations(doc);
        doc.version += 1;
        let doc_clone = doc.clone();
        store.save(&doc_clone);
        Ok(Json(doc_clone))
    }

    pub async fn trigger_drift_check(
        Path(project): Path<String>,
        State(ai): State<AIProviderState>,
    ) -> Json<serde_json::Value> {
        let specs_doc = {
            let store = specs().lock().unwrap();
            store.get(&project).cloned()
        };

        let Some(doc) = specs_doc else {
            return Json(serde_json::json!({
                "status": "ok",
                "drift_detected": false,
                "sections_checked": 0,
                "message": "No specs found",
            }));
        };

        // Find goals and plans sections
        let goals = doc.sections.iter().find(|s| s.id == "goals").map(|s| s.content.clone()).unwrap_or_default();
        let plans = doc.sections.iter().find(|s| s.id == "plans").map(|s| s.content.clone()).unwrap_or_default();

        // Extract file paths from plans (simple heuristic: lines mentioning file paths)
        let mut file_paths = Vec::new();
        for line in plans.lines() {
            // Look for patterns like `src/...`, `crates/...`, `.rs`, `.ts`, `.vue`
            for word in line.split_whitespace() {
                if word.contains('/') && (word.ends_with(".rs") || word.ends_with(".ts") || word.ends_with(".vue") || word.ends_with(".js")) {
                    let clean = word.trim_matches(|c| c == '(' || c == ')' || c == '`' || c == '"' || c == ',' || c == '.');
                    if !clean.is_empty() && !file_paths.contains(&clean.to_string()) {
                        file_paths.push(clean.to_string());
                    }
                }
            }
        }

        // Read up to 5 files
        let mut code_content = String::new();
        for path in file_paths.iter().take(5) {
            if let Ok(content) = std::fs::read_to_string(path) {
                code_content.push_str(&format!("\n--- {} ---\n{}", path, content));
            }
        }

        if goals.is_empty() || code_content.is_empty() {
            return Json(serde_json::json!({
                "status": "ok",
                "drift_detected": false,
                "sections_checked": 0,
                "message": "No goals or code files to compare",
            }));
        }

        // Call AI to verify goals against code
        let prompt = format!(
            r#"You are a goals verifier. Compare the following goals against the implemented code.

Goals:
{}

Implemented code:
{}

For each goal, state whether it is:
- FULLY implemented
- PARTIALLY implemented
- NOT implemented
- UNKNOWN (cannot determine from code)

Format your response as:
G1: <status> — <brief explanation>
G2: <status> — <brief explanation>
...

If no goal IDs exist, number them sequentially."#,
            goals, code_content
        );

        let request = crate::provider::AIRequest {
            prompt,
            context: None,
        };

        let response = ai.chat(request).await;

        let drift_detected = response.content.to_lowercase().contains("not implemented")
            || response.content.to_lowercase().contains("partially implemented");

        // Update specs: mark goals section as drift if detected
        if drift_detected {
            let mut store = specs().lock().unwrap();
            let _ = store.update_section(&project, "goals", goals.clone(), "drift".to_string());
        }

        Json(serde_json::json!({
            "status": "ok",
            "drift_detected": drift_detected,
            "sections_checked": 1,
            "report": response.content,
            "error": response.error,
        }))
    }

    // ─── Relay Handlers ──────────────────────────────────────────────────

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RunRequest {
        pub task: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RunInfo {
        pub id: String,
        pub task: String,
        pub status: String,
    }

    pub async fn start_run(Json(req): Json<RunRequest>) -> Json<RunInfo> {
        Json(RunInfo {
            id: format!("run-{}", uuid::Uuid::new_v4()),
            task: req.task,
            status: String::from("started"),
        })
    }

    pub async fn list_runs() -> Json<Vec<RunInfo>> {
        Json(vec![])
    }

    // ─── Approval Gate Handlers ──────────────────────────────────────────

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ApproveSpecRequest {
        #[serde(default)]
        pub edited_specs: std::collections::HashMap<String, String>,
    }

    pub async fn approve_spec(
        Path(sid): Path<String>,
        Json(body): Json<ApproveSpecRequest>,
    ) -> Json<serde_json::Value> {
        // 1. Capture pending changes and project path
        let (project, mut changes) = {
            let store = forge_sessions().lock().unwrap();
            let session = store.get(&sid).cloned().unwrap_or_else(|| ForgeSession {
                id: sid.clone(),
                notebook_sid: None,
                project_path: String::new(),
                status: ForgeStatus::Idle,
                name: None,
                messages: vec![],
                pending_spec_changes: vec![],
                focus_section: None,
                active_profession: None,
                errand_sessions: vec![],
                work_mode: None,
                active_task_plan: None,
                active_relay_runs: vec![],
            });
            (session.project_path.clone(), session.pending_spec_changes.clone())
        };

        // 2. Override with user-edited specs if provided
        if !body.edited_specs.is_empty() {
            for change in &mut changes {
                if let Some(edited) = body.edited_specs.get(&change.section_id) {
                    change.new_content = edited.clone();
                }
            }
        }

        // 3. Apply pending (possibly edited) changes to Specs
        if !project.is_empty() && !changes.is_empty() {
            let mut specs = specs().lock().unwrap();
            for change in &changes {
                let _ = specs.update_section(
                    &project,
                    &change.section_id,
                    change.new_content.clone(),
                    change.new_status.clone(),
                );
            }
        }

        // 3. Clear pending changes and transition phase
        {
            let mut store = forge_sessions().lock().unwrap();
            if let Some(session) = store.get_mut(&sid) {
                session.pending_spec_changes.clear();
                let clone = session.clone();
                store.save(&clone);
            }
            store.update_status(&sid, ForgeStatus::Idle);
        }

        Json(serde_json::json!({"status": "ok", "phase": "execution"}))
    }

    pub async fn reject_spec(Path(sid): Path<String>) -> Json<serde_json::Value> {
        {
            let mut store = forge_sessions().lock().unwrap();
            if let Some(session) = store.get_mut(&sid) {
                session.pending_spec_changes.clear();
                let clone = session.clone();
                store.save(&clone);
            }
            store.update_status(&sid, ForgeStatus::Idle);
        }
        Json(serde_json::json!({"status": "ok", "phase": "spec_draft"}))
    }

    // ─── Errand Endpoints ────────────────────────────────────────────────

    pub async fn list_errands(Path(sid): Path<String>) -> Json<Vec<serde_json::Value>> {
        let store = forge_sessions().lock().unwrap();
        let errands = if let Some(session) = store.get(&sid) {
            session
                .errand_sessions
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "id": e.id,
                        "profession_id": e.profession_id,
                        "task": e.task,
                        "status": match &e.status {
                            crate::forge::errand::ErrandStatus::Running => "running",
                            crate::forge::errand::ErrandStatus::Completed { .. } => "completed",
                            crate::forge::errand::ErrandStatus::Failed { .. } => "failed",
                            crate::forge::errand::ErrandStatus::Truncated { .. } => "truncated",
                        },
                        "started_at": e.started_at,
                        "completed_at": e.completed_at,
                    })
                })
                .collect()
        } else {
            vec![]
        };
        Json(errands)
    }

    pub async fn get_errand(Path((sid, eid)): Path<(String, String)>) -> Json<Option<crate::forge::errand::ErrandSession>> {
        let store = forge_sessions().lock().unwrap();
        let errand = if let Some(session) = store.get(&sid) {
            session.errand_sessions.iter().find(|e| e.id == eid).cloned()
        } else {
            None
        };
        Json(errand)
    }

    // ─── Helpers ─────────────────────────────────────────────────────────

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Non-generic route builder — caller must provide state that can produce AIProviderState
pub fn routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    crate::provider::AIProviderState: FromRef<S>,
{
    Router::new()
        // Health
        .route("/api/health", get(handlers::health))
        // Project management
        .route("/api/forge/project/status", get(handlers::get_project_status))
        .route("/api/forge/project/open", post(handlers::open_project))
        .route("/api/forge/project/close", post(handlers::close_project))
        .route("/api/forge/project/recent", get(handlers::list_recent_projects))
        .route("/api/forge/project/browse", get(handlers::browse_directory))
        .route("/api/forge/project/tree", get(handlers::project_tree))
        .route("/api/forge/project/file", get(handlers::read_file))
        .route("/api/forge/project/pick-folder", get(handlers::pick_folder))
        // Forge
        .route("/api/forge/chats/session", post(handlers::create_forge_session))
        .route("/api/forge/chats/sessions", get(handlers::list_forge_sessions).delete(handlers::delete_all_forge_sessions))
        .route("/api/forge/chats/session/{sid}", get(handlers::get_forge_session).patch(handlers::rename_forge_session).delete(handlers::delete_forge_session))
        .route("/api/forge/chats/{sid}/message", post(handlers::send_forge_message))
        .route("/api/forge/chats/{sid}/stream", get(handlers::forge_stream))
        .route("/api/forge/chats/{sid}/history", get(handlers::forge_history))
        .route("/api/forge/chats/{sid}/errands", get(handlers::list_errands))
        .route("/api/forge/chats/{sid}/errands/{eid}", get(handlers::get_errand))
        .route("/api/forge/chats/{sid}/approve", post(handlers::approve_spec))
        .route("/api/forge/chats/{sid}/reject", post(handlers::reject_spec))
        // Specs (more specific routes FIRST)
        .route("/api/forge/specs/{project}/drift-check", post(handlers::trigger_drift_check))
        .route("/api/forge/specs/{project}/rebuild-relations", post(handlers::rebuild_relations_endpoint))
        .route("/api/forge/specs/{project}/overview", get(handlers::get_specs_overview))
        .route("/api/forge/specs/{project}/module/{module}/outline", get(handlers::get_module_outline))
        .route("/api/forge/specs/{project}/related/{item_id}", get(handlers::get_related_items))
        .route("/api/forge/specs/{project}/{section_id}", get(handlers::get_specs_section).put(handlers::update_specs_section))
        .route("/api/forge/specs/{project}", get(handlers::get_specs).put(handlers::update_specs))
        // Relay
        .merge(crate::relay::api::relay_routes())
        // Wiki
        .merge(crate::forge::wiki::wiki_routes())
}
