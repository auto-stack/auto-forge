//! AutoForge MCP Server — 30 Tools
//!
//! Exposes tools via rmcp so external MCP clients (Claude Desktop, Cursor,
//! etc.) can interact with AutoForge without going through the REST API.
//!
//! Core:
//!   • forge_get_project_status
//!   • forge_create_session
//!   • forge_send_message
//!   • forge_list_professions
//!   • forge_start_relay_run
//!   • forge_list_runs
//!   • forge_get_run
//!   • forge_read_specs
//!   • forge_list_specs_sections
//!   • forge_update_spec
//!
//! Session Management:
//!   • forge_get_session
//!   • forge_list_sessions
//!   • forge_delete_session
//!
//! File/Project:
//!   • forge_read_file
//!   • forge_browse_directory
//!   • forge_open_project
//!   • forge_close_project
//!
//! Spec Workflow:
//!   • forge_approve_spec
//!   • forge_reject_spec
//!
//! API Sources:
//!   • forge_list_api_sources
//!   • forge_test_api_connection
//!
//! Batch:
//!   • forge_batch_start_runs
//!   • forge_batch_get_results
//!
//! Monitoring & Control:
//!   • forge_get_performance_logs
//!   • forge_poll_chat_status
//!   • forge_poll_run_phase
//!   • forge_advance_run
//!   • forge_submit_handoff
//!   • forge_resolve_gate

use rmcp::{
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars,
    tool, tool_handler, tool_router,
};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Server struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AutoForgeMcpServer {
    pub ai_provider: crate::provider::AIProviderState,
}

impl AutoForgeMcpServer {
    pub fn new(ai_provider: crate::provider::AIProviderState) -> Self {
        Self { ai_provider }
    }
}

// ---------------------------------------------------------------------------
// Input DTOs  (need JsonSchema for rmcp parameter schema generation)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateSessionInput {
    pub notebook_sid: Option<String>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SendMessageInput {
    pub sid: String,
    pub content: String,
    pub profession_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct StartRunInput {
    pub flow_id: String,
    pub task: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetRunInput {
    pub run_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadSpecsInput {
    pub project: String,
    /// Optional: filter to a single section (e.g. "goals", "architecture")
    pub section_id: Option<String>,
    /// Optional: filter to a single item within the section (requires section_id)
    pub item_id: Option<String>,
    /// Optional: when false, omit items array from sections (summaries only)
    pub include_items: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateSpecInput {
    pub project: String,
    pub section_id: String,
    pub item_id: String,
    pub action: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub depends_on: Option<Vec<String>>,
    pub assignee: Option<String>,
    pub test_file: Option<String>,
    pub file: Option<String>,
    pub milestone: Option<String>,
    pub tags: Option<Vec<String>>,
    pub module: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ListSpecsSectionsInput {
    pub project: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetSessionInput {
    pub sid: String,
    pub include_history: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteSessionInput {
    pub sid: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadFileInput {
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BrowseDirectoryInput {
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ApproveSpecInput {
    pub sid: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct OpenProjectInput {
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TestApiConnectionInput {
    pub id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BatchStartRunsInput {
    pub flow_id: String,
    pub task: Option<String>,
    pub count: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BatchGetResultsInput {
    pub run_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PollChatStatusInput {
    pub sid: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PollRunPhaseInput {
    pub run_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AdvanceRunInput {
    pub run_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SubmitHandoffInput {
    pub run_id: String,
    pub handoff: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ResolveGateInput {
    pub run_id: String,
    pub decision: String,
    pub feedback: Option<String>,
}

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

fn text_response<T: Serialize>(data: &T) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| McpError::internal_error(format!("JSON serialization failed: {e}"), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn error_text(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![Content::text(msg.into())])
}

// ---------------------------------------------------------------------------
// Tool router
// ---------------------------------------------------------------------------

#[tool_router(server_handler)]
impl AutoForgeMcpServer {
    // -----------------------------------------------------------------------
    // 1. Project status
    // -----------------------------------------------------------------------
    #[tool(description = "Get the current project open status and path", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_get_project_status(&self) -> Result<CallToolResult, McpError> {
        let specs_store = crate::forge::specs().lock().unwrap();
        let project_open = specs_store.is_project_open();
        let project_path = crate::forge::current_project_path();

        #[derive(Serialize)]
        struct Resp {
            project_open: bool,
            project_path: Option<String>,
        }
        text_response(&Resp {
            project_open,
            project_path,
        })
    }

    // -----------------------------------------------------------------------
    // 2. Create session
    // -----------------------------------------------------------------------
    #[tool(description = "Create a new Forge chat session", annotations(destructive_hint = false))]
    async fn forge_create_session(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<CreateSessionInput>,
    ) -> Result<CallToolResult, McpError> {
        let sid = format!("fs-{}", uuid::Uuid::new_v4());
        let session = crate::forge::ForgeSession {
            id: sid.clone(),
            notebook_sid: input.0.notebook_sid.clone(),
            project_path: input.0.project_path.clone().unwrap_or_default(),
            status: crate::forge::ForgeStatus::Idle,
            messages: Vec::new(),
            name: None,
            pending_spec_changes: Vec::new(),
            focus_section: None,
            active_profession: None,
            errand_sessions: Vec::new(),
        };
        crate::forge::forge_sessions().lock().unwrap().insert(session.clone());

        #[derive(Serialize)]
        struct SessionInfo {
            id: String,
            name: Option<String>,
            notebook_sid: Option<String>,
            project_path: String,
            status: String,
            message_count: usize,
        }
        text_response(&SessionInfo {
            id: session.id,
            name: session.name,
            notebook_sid: session.notebook_sid,
            project_path: session.project_path,
            status: format!("{:?}", session.status),
            message_count: 0,
        })
    }

    // -----------------------------------------------------------------------
    // 3. Send message
    // -----------------------------------------------------------------------
    #[tool(description = "Send a message to a Forge session and get an AI reply", annotations(destructive_hint = false))]
    async fn forge_send_message(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<SendMessageInput>,
    ) -> Result<CallToolResult, McpError> {
        let sid = input.0.sid.clone();
        let content = input.0.content.clone();
        let profession_id = input.0.profession_id.clone();

        // 1. Look up session and append user message
        {
            let mut store = crate::forge::forge_sessions().lock().unwrap();
            let session = store
                .get_mut(&sid)
                .ok_or_else(|| McpError::invalid_params("Session not found", None))?;
            let user_msg = crate::forge::ForgeMessage {
                id: format!("m-{}", uuid::Uuid::new_v4()),
                role: "user".into(),
                content: content.clone(),
                timestamp: now_secs(),
                tool_calls: None,
                profession_id: profession_id.clone(),
            };
            session.messages.push(user_msg);
            let session_clone = session.clone();
            let _ = session;
            store.save(&session_clone);
        } // store (MutexGuard) dropped here before await

        // 2. Call AI provider (non-streaming)
        let ai_request = crate::provider::AIRequest {
            prompt: content.clone(),
            context: None,
        };
        let response = self.ai_provider.chat(ai_request).await;

        // 3. Append assistant message and persist
        {
            let mut store = crate::forge::forge_sessions().lock().unwrap();
            let session = store
                .get_mut(&sid)
                .ok_or_else(|| McpError::invalid_params("Session not found", None))?;
            let assistant_msg = crate::forge::ForgeMessage {
                id: format!("m-{}", uuid::Uuid::new_v4()),
                role: "assistant".into(),
                content: response.content.clone(),
                timestamp: now_secs(),
                tool_calls: None,
                profession_id: profession_id.clone(),
            };
            session.messages.push(assistant_msg);
            let session_clone = session.clone();
            let _ = session;
            store.save(&session_clone);
        }

        #[derive(Serialize)]
        struct Reply {
            assistant_message: String,
            error: Option<String>,
        }
        text_response(&Reply {
            assistant_message: response.content,
            error: response.error,
        })
    }

    // -----------------------------------------------------------------------
    // 4. List professions
    // -----------------------------------------------------------------------
    #[tool(description = "List all available professions with id, name and phase", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_list_professions(&self) -> Result<CallToolResult, McpError> {
        let professions = crate::relay::api::professions().lock().unwrap();

        #[derive(Serialize)]
        struct ProfessionBrief {
            id: String,
            name: String,
            phase: String,
        }

        let list: Vec<_> = professions
            .iter()
            .map(|p| ProfessionBrief {
                id: p.id.clone(),
                name: p.name.clone(),
                phase: format!("{:?}", p.phase),
            })
            .collect();

        text_response(&list)
    }

    // -----------------------------------------------------------------------
    // 5. Start relay run
    // -----------------------------------------------------------------------
    #[tool(description = "Start a new relay pipeline run", annotations(destructive_hint = false))]
    async fn forge_start_relay_run(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<StartRunInput>,
    ) -> Result<CallToolResult, McpError> {
        let flow = crate::relay::flows::get_flow(&input.0.flow_id).ok_or_else(|| {
            McpError::invalid_params(
                "Flow not found",
                Some(serde_json::json!({ "flow_id": &input.0.flow_id })),
            )
        })?;

        let run_id = format!("run-{}", uuid::Uuid::new_v4());
        let run_store = crate::relay::api::run_store();
        let event_tx = crate::relay::api::event_sender();

        let run_state = crate::relay::store::start_run(run_store, flow.clone(), &run_id)
            .map_err(|e| McpError::internal_error(e, None))?;

        // Set title from task description
        let task = input.0.task.clone().unwrap_or_default();
        {
            let mut store = run_store.lock().unwrap();
            if let Some(entry) = store.get_mut(&run_id) {
                entry.metadata.title = Some(crate::relay::title::generate_title(&task));
                crate::relay::store::save_run(entry);
            }
        }

        // Broadcast event
        let _ = event_tx.send(crate::relay::api::RunEventBroadcast {
            run_id: run_id.clone(),
            event_type: "run_started".into(),
            payload: None,
        });

        // Spawn background driver
        let project_path = crate::forge::current_project_path().unwrap_or_default();
        tokio::spawn(crate::relay::driver::drive_run(
            run_id.clone(),
            run_store.clone(),
            event_tx.clone(),
            self.ai_provider.clone(),
            task,
            project_path,
        ));

        tracing::info!(run_id = %run_id, "forge_start_relay_run: returning run_state");
        let result = text_response(&run_state);
        tracing::info!(run_id = %run_id, success = result.is_ok(), "forge_start_relay_run: text_response done");
        result
    }

    // -----------------------------------------------------------------------
    // 6. List runs
    // -----------------------------------------------------------------------
    #[tool(description = "List all relay runs with summary information", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_list_runs(&self) -> Result<CallToolResult, McpError> {
        let runs = crate::relay::store::list_runs(crate::relay::api::run_store());
        text_response(&runs)
    }

    // -----------------------------------------------------------------------
    // 7. Get run
    // -----------------------------------------------------------------------
    #[tool(description = "Get the detailed state of a relay run by run_id", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_get_run(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<GetRunInput>,
    ) -> Result<CallToolResult, McpError> {
        let run = crate::relay::store::get_run(
            crate::relay::api::run_store(),
            &input.0.run_id,
        )
        .ok_or_else(|| {
            McpError::invalid_params(
                "Run not found",
                Some(serde_json::json!({ "run_id": &input.0.run_id })),
            )
        })?;
        text_response(&run)
    }

    // -----------------------------------------------------------------------
    // 8. Read specs
    // -----------------------------------------------------------------------
    #[tool(description = "Read the specs document for a project. Use section_id/item_id to filter, and include_items=false for lightweight summaries.", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_read_specs(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<ReadSpecsInput>,
    ) -> Result<CallToolResult, McpError> {
        let project = if input.0.project.is_empty() {
            crate::forge::tools::current_project()
        } else {
            input.0.project.clone()
        };
        let mut store = crate::forge::specs().lock().unwrap();
        let has_filter = input.0.section_id.is_some()
            || input.0.item_id.is_some()
            || input.0.include_items == Some(false);

        if has_filter {
            let doc = store.get(&project).ok_or_else(|| {
                McpError::invalid_params(
                    format!("Project '{}' not found", project),
                    None,
                )
            })?;

            if let Some(ref section_id) = input.0.section_id {
                let section = doc.sections.iter().find(|s| s.id == *section_id).ok_or_else(|| {
                    McpError::invalid_params(
                        format!("Section '{}' not found", section_id),
                        None,
                    )
                })?;

                if let Some(ref item_id) = input.0.item_id {
                    let item = section.items.iter().find(|i| i.id == *item_id).ok_or_else(|| {
                        McpError::invalid_params(
                            format!("Item '{}' not found in section '{}'", item_id, section_id),
                            None,
                        )
                    })?;
                    return text_response(item);
                }

                if input.0.include_items == Some(false) {
                    let mut sec = section.clone();
                    sec.items.clear();
                    return text_response(&sec);
                }
                return text_response(section);
            }

            // include_items=false without section_id: return doc with empty items
            let mut doc_copy = doc.clone();
            for s in &mut doc_copy.sections {
                s.items.clear();
            }
            return text_response(&doc_copy);
        }

        let doc = store.get_or_default(&project);
        text_response(doc)
    }

    // -----------------------------------------------------------------------
    // 8b. List specs sections
    // -----------------------------------------------------------------------
    #[tool(description = "List all sections for a project with lightweight summaries (id, title, type, status, item_count)", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_list_specs_sections(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<ListSpecsSectionsInput>,
    ) -> Result<CallToolResult, McpError> {
        let project = if input.0.project.is_empty() {
            crate::forge::tools::current_project()
        } else {
            input.0.project.clone()
        };
        let store = crate::forge::specs().lock().unwrap();
        let doc = store.get(&project).ok_or_else(|| {
            McpError::invalid_params(
                format!("Project '{}' not found", project),
                None,
            )
        })?;

        #[derive(Serialize)]
        struct SectionSummary {
            id: String,
            title: String,
            section_type: String,
            status: String,
            item_count: usize,
            last_modified: u64,
        }

        let sections: Vec<_> = doc.sections.iter().map(|s| SectionSummary {
            id: s.id.clone(),
            title: s.title.clone(),
            section_type: format!("{:?}", s.section_type),
            status: s.status.as_str().to_string(),
            item_count: s.items.len(),
            last_modified: s.last_modified,
        }).collect();

        text_response(&serde_json::json!({
            "project": doc.project.clone(),
            "section_count": sections.len(),
            "sections": sections,
        }))
    }

    // -----------------------------------------------------------------------
    // 8c. Update spec item
    // -----------------------------------------------------------------------
    #[tool(description = "Update, create, or delete a single spec item. Action: upsert (default), delete, or patch.", annotations(destructive_hint = false))]
    async fn forge_update_spec(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<UpdateSpecInput>,
    ) -> Result<CallToolResult, McpError> {
        let project = if input.0.project.is_empty() {
            crate::forge::tools::current_project()
        } else {
            input.0.project.clone()
        };
        let action = input.0.action.as_deref().unwrap_or("upsert");
        let mut store = crate::forge::specs().lock().unwrap();

        let result = match action {
            "delete" => {
                store.delete_spec_item(&project, &input.0.section_id, &input.0.item_id)
            }
            "patch" => {
                let content = input.0.content.as_deref().ok_or_else(|| {
                    McpError::invalid_params("'patch' action requires 'content'", None)
                })?;
                store.patch_spec_item(&project, &input.0.section_id, &input.0.item_id, content)
            }
            "upsert" => {
                store.upsert_spec_item(
                    &project,
                    &input.0.section_id,
                    &input.0.item_id,
                    input.0.title.as_deref(),
                    input.0.content.as_deref(),
                    input.0.status.as_deref(),
                    input.0.priority.as_deref(),
                    input.0.assignee.as_deref(),
                    input.0.test_file.as_deref(),
                    input.0.file.as_deref(),
                    input.0.milestone.as_deref(),
                    input.0.module.as_deref(),
                    input.0.depends_on.clone(),
                    input.0.tags.clone(),
                )
            }
            other => {
                return Ok(error_text(format!(
                    "Invalid action '{}'. Use 'upsert', 'delete', or 'patch'.",
                    other
                )));
            }
        };

        match result {
            Ok(msg) => text_response(&serde_json::json!({
                "result": msg,
                "project": project,
                "section_id": input.0.section_id,
                "item_id": input.0.item_id,
                "action": action,
            })),
            Err(e) => Ok(error_text(e)),
        }
    }

    // -----------------------------------------------------------------------
    // 9. Get session
    // -----------------------------------------------------------------------
    #[tool(description = "Get details of a Forge chat session. Set include_history=true to retrieve the full message log.", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_get_session(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<GetSessionInput>,
    ) -> Result<CallToolResult, McpError> {
        let store = crate::forge::forge_sessions().lock().unwrap();
        let session = store
            .get(&input.0.sid)
            .ok_or_else(|| McpError::invalid_params("Session not found", None))?;

        let include_history = input.0.include_history.unwrap_or(false);

        #[derive(Serialize)]
        struct SessionDetail {
            id: String,
            name: Option<String>,
            notebook_sid: Option<String>,
            project_path: String,
            status: String,
            active_profession: Option<String>,
            focus_section: Option<String>,
            message_count: usize,
            pending_changes_count: usize,
            #[serde(skip_serializing_if = "Option::is_none")]
            messages: Option<Vec<serde_json::Value>>,
        }

        let messages = if include_history {
            Some(session.messages.iter().map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "role": m.role,
                    "content": m.content,
                    "timestamp": m.timestamp,
                    "profession_id": m.profession_id,
                })
            }).collect())
        } else {
            None
        };

        text_response(&SessionDetail {
            id: session.id.clone(),
            name: session.name.clone(),
            notebook_sid: session.notebook_sid.clone(),
            project_path: session.project_path.clone(),
            status: format!("{:?}", session.status),
            active_profession: session.active_profession.clone(),
            focus_section: session.focus_section.clone(),
            message_count: session.messages.len(),
            pending_changes_count: session.pending_spec_changes.len(),
            messages,
        })
    }

    // -----------------------------------------------------------------------
    // 10. List sessions
    // -----------------------------------------------------------------------
    #[tool(description = "List all Forge chat sessions", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_list_sessions(&self) -> Result<CallToolResult, McpError> {
        let store = crate::forge::forge_sessions().lock().unwrap();

        #[derive(Serialize)]
        struct SessionSummary {
            id: String,
            name: Option<String>,
            status: String,
            message_count: usize,
            project_path: String,
        }

        let list: Vec<_> = store
            .list_all()
            .iter()
            .map(|s| SessionSummary {
                id: s.id.clone(),
                name: s.name.clone(),
                status: format!("{:?}", s.status),
                message_count: s.messages.len(),
                project_path: s.project_path.clone(),
            })
            .collect();

        text_response(&list)
    }

    // -----------------------------------------------------------------------
    // 11. Delete session
    // -----------------------------------------------------------------------
    #[tool(description = "Delete a Forge chat session", annotations(destructive_hint = true))]
    async fn forge_delete_session(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<DeleteSessionInput>,
    ) -> Result<CallToolResult, McpError> {
        let mut store = crate::forge::forge_sessions().lock().unwrap();
        let existed = store.remove(&input.0.sid);
        text_response(&serde_json::json!({
            "deleted": existed,
            "sid": input.0.sid,
        }))
    }

    // -----------------------------------------------------------------------
    // 12. Read file
    // -----------------------------------------------------------------------
    #[tool(description = "Read a file from the current project", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_read_file(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<ReadFileInput>,
    ) -> Result<CallToolResult, McpError> {
        let specs_store = crate::forge::specs().lock().unwrap();
        if !specs_store.is_project_open() {
            return Ok(error_text("No project open"));
        }
        let project_path = specs_store.project_base_path().unwrap_or_default();
        drop(specs_store);

        let path = std::path::Path::new(&input.0.path);
        let canonical_project = std::fs::canonicalize(&project_path)
            .unwrap_or_else(|_| project_path.clone());
        let canonical_path = std::fs::canonicalize(path)
            .map_err(|e| McpError::invalid_params(format!("Invalid path: {e}"), None))?;

        if !canonical_path.starts_with(&canonical_project) {
            return Ok(error_text("Path outside project"));
        }

        let max_size = 1024 * 1024; // 1MB limit
        let metadata = std::fs::metadata(&canonical_path)
            .map_err(|e| McpError::internal_error(format!("Failed to read file metadata: {e}"), None))?;
        if metadata.len() > max_size {
            return Ok(error_text(format!("File too large: {} bytes (max 1MB)", metadata.len())));
        }

        let content = std::fs::read_to_string(&canonical_path)
            .map_err(|e| McpError::internal_error(format!("Failed to read file: {e}"), None))?;

        #[derive(Serialize)]
        struct FileContent {
            path: String,
            content: String,
            size: u64,
        }
        text_response(&FileContent {
            path: input.0.path.clone(),
            content,
            size: metadata.len(),
        })
    }

    // -----------------------------------------------------------------------
    // 13. Browse directory
    // -----------------------------------------------------------------------
    #[tool(description = "Browse a directory in the current project", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_browse_directory(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<BrowseDirectoryInput>,
    ) -> Result<CallToolResult, McpError> {
        let specs_store = crate::forge::specs().lock().unwrap();
        if !specs_store.is_project_open() {
            return Ok(error_text("No project open"));
        }
        let project_path = specs_store.project_base_path().unwrap_or_default();
        drop(specs_store);

        let path = std::path::Path::new(&input.0.path);
        let canonical_project = std::fs::canonicalize(&project_path)
            .unwrap_or_else(|_| project_path.clone());
        let canonical_path = std::fs::canonicalize(path)
            .map_err(|e| McpError::invalid_params(format!("Invalid path: {e}"), None))?;

        if !canonical_path.starts_with(&canonical_project) {
            return Ok(error_text("Path outside project"));
        }

        if !canonical_path.is_dir() {
            return Ok(error_text("Not a directory"));
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&canonical_path)
            .map_err(|e| McpError::internal_error(format!("Cannot read directory: {e}"), None))?
        {
            let entry = entry.map_err(|e| McpError::internal_error(format!("Directory entry error: {e}"), None))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path().to_string_lossy().to_string();
            let is_dir = entry.path().is_dir();
            let size = if is_dir { None } else {
                entry.metadata().ok().map(|m| m.len())
            };
            entries.push(serde_json::json!({
                "name": name,
                "path": path,
                "is_dir": is_dir,
                "size": size,
            }));
        }

        entries.sort_by(|a, b| {
            let a_dir = a["is_dir"].as_bool().unwrap_or(false);
            let b_dir = b["is_dir"].as_bool().unwrap_or(false);
            match (b_dir, a_dir) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => a["name"].as_str().unwrap_or("").to_lowercase()
                    .cmp(&b["name"].as_str().unwrap_or("").to_lowercase()),
            }
        });

        text_response(&serde_json::json!({
            "path": input.0.path,
            "entries": entries,
        }))
    }

    // -----------------------------------------------------------------------
    // 14. Approve spec
    // -----------------------------------------------------------------------
    #[tool(description = "Approve pending spec changes for a session", annotations(destructive_hint = false))]
    async fn forge_approve_spec(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<ApproveSpecInput>,
    ) -> Result<CallToolResult, McpError> {
        let sid = input.0.sid.clone();
        let (project, changes) = {
            let store = crate::forge::forge_sessions().lock().unwrap();
            let session = store.get(&sid).cloned().unwrap_or_else(|| crate::forge::ForgeSession {
                id: sid.clone(),
                notebook_sid: None,
                project_path: String::new(),
                status: crate::forge::ForgeStatus::Idle,
                name: None,
                messages: vec![],
                pending_spec_changes: vec![],
                focus_section: None,
                active_profession: None,
                errand_sessions: vec![],
            });
            (session.project_path.clone(), session.pending_spec_changes.clone())
        };

        if !project.is_empty() && !changes.is_empty() {
            let mut specs = crate::forge::specs().lock().unwrap();
            for change in &changes {
                let _ = specs.update_section(
                    &project,
                    &change.section_id,
                    change.new_content.clone(),
                    change.new_status.clone(),
                );
            }
        }

        {
            let mut store = crate::forge::forge_sessions().lock().unwrap();
            if let Some(session) = store.get_mut(&sid) {
                session.pending_spec_changes.clear();
                let clone = session.clone();
                let _ = session;
                store.save(&clone);
            }
            store.update_status(&sid, crate::forge::ForgeStatus::Idle);
        }

        text_response(&serde_json::json!({
            "status": "ok",
            "phase": "execution",
            "applied_changes": changes.len(),
        }))
    }

    // -----------------------------------------------------------------------
    // 15. Reject spec
    // -----------------------------------------------------------------------
    #[tool(description = "Reject pending spec changes for a session", annotations(destructive_hint = true))]
    async fn forge_reject_spec(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<ApproveSpecInput>,
    ) -> Result<CallToolResult, McpError> {
        let sid = input.0.sid.clone();
        {
            let mut store = crate::forge::forge_sessions().lock().unwrap();
            if let Some(session) = store.get_mut(&sid) {
                session.pending_spec_changes.clear();
                let clone = session.clone();
                let _ = session;
                store.save(&clone);
            }
            store.update_status(&sid, crate::forge::ForgeStatus::Idle);
        }
        text_response(&serde_json::json!({ "status": "rejected" }))
    }

    // -----------------------------------------------------------------------
    // 16. List API sources
    // -----------------------------------------------------------------------
    #[tool(description = "List all configured API sources", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_list_api_sources(&self) -> Result<CallToolResult, McpError> {
        let sources = crate::relay::api::api_sources().lock().unwrap();
        let list: Vec<_> = sources.clone();
        text_response(&list)
    }

    // -----------------------------------------------------------------------
    // 17. Test API connection
    // -----------------------------------------------------------------------
    #[tool(description = "Test connection to an API source by ID", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_test_api_connection(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<TestApiConnectionInput>,
    ) -> Result<CallToolResult, McpError> {
        let result = crate::relay::api::do_test_connection(&input.0.id).await;
        text_response(&result)
    }

    // -----------------------------------------------------------------------
    // 18. Open project
    // -----------------------------------------------------------------------
    #[tool(description = "Open a project by path", annotations(destructive_hint = false))]
    async fn forge_open_project(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<OpenProjectInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = std::path::PathBuf::from(&input.0.path);
        let mut store = crate::forge::specs().lock().unwrap();
        let info = store.open_project(&path)
            .map_err(|e| McpError::internal_error(e, None))?;
        text_response(&info)
    }

    // -----------------------------------------------------------------------
    // 19. Close project
    // -----------------------------------------------------------------------
    #[tool(description = "Close the currently open project", annotations(destructive_hint = true))]
    async fn forge_close_project(&self) -> Result<CallToolResult, McpError> {
        let mut store = crate::forge::specs().lock().unwrap();
        store.close_project();
        text_response(&serde_json::json!({ "status": "closed" }))
    }

    // -----------------------------------------------------------------------
    // 20. Get performance logs
    // -----------------------------------------------------------------------
    #[tool(description = "Get recent performance/timing logs from backend.log", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_get_performance_logs(&self) -> Result<CallToolResult, McpError> {
        let log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("backend.log");
        let content = std::fs::read_to_string(&log_path)
            .unwrap_or_default();
        let lines: Vec<&str> = content.lines().collect();
        let recent: Vec<&str> = lines.iter().rev().take(200).cloned().collect();
        text_response(&serde_json::json!({
            "log_file": log_path.to_string_lossy().to_string(),
            "total_lines": lines.len(),
            "recent_lines": recent.len(),
            "lines": recent.into_iter().rev().collect::<Vec<_>>(),
        }))
    }

    // -----------------------------------------------------------------------
    // 21. Batch start runs
    // -----------------------------------------------------------------------
    #[tool(description = "Start multiple relay pipeline runs in parallel", annotations(destructive_hint = false))]
    async fn forge_batch_start_runs(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<BatchStartRunsInput>,
    ) -> Result<CallToolResult, McpError> {
        let flow = crate::relay::flows::get_flow(&input.0.flow_id).ok_or_else(|| {
            McpError::invalid_params("Flow not found", Some(serde_json::json!({ "flow_id": &input.0.flow_id })))
        })?;

        let count = input.0.count.max(1).min(10); // limit to 1-10 runs
        let task = input.0.task.clone().unwrap_or_default();
        let run_store = crate::relay::api::run_store();
        let event_tx = crate::relay::api::event_sender();
        let project_path = crate::forge::current_project_path().unwrap_or_default();

        let mut run_ids = Vec::new();
        let mut run_states = Vec::new();

        for i in 0..count {
            let run_id = format!("run-batch-{}-{}", i, uuid::Uuid::new_v4());
            let run_state = crate::relay::store::start_run(run_store, flow.clone(), &run_id)
                .map_err(|e| McpError::internal_error(e, None))?;

            {
                let mut store = run_store.lock().unwrap();
                if let Some(entry) = store.get_mut(&run_id) {
                    let title = if count == 1 { task.clone() } else { format!("{} (batch {}/{})", task, i + 1, count) };
                    entry.metadata.title = Some(crate::relay::title::generate_title(&title));
                    crate::relay::store::save_run(entry);
                }
            }

            let _ = event_tx.send(crate::relay::api::RunEventBroadcast {
                run_id: run_id.clone(),
                event_type: "run_started".into(),
                payload: None,
            });

            tokio::spawn(crate::relay::driver::drive_run(
                run_id.clone(),
                run_store.clone(),
                event_tx.clone(),
                self.ai_provider.clone(),
                task.clone(),
                project_path.clone(),
            ));

            run_ids.push(run_id);
            run_states.push(run_state);
        }

        #[derive(Serialize)]
        struct BatchResult {
            run_ids: Vec<String>,
            count: u32,
        }
        text_response(&BatchResult { run_ids, count })
    }

    // -----------------------------------------------------------------------
    // 22. Batch get results
    // -----------------------------------------------------------------------
    #[tool(description = "Get the state of multiple relay runs at once", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_batch_get_results(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<BatchGetResultsInput>,
    ) -> Result<CallToolResult, McpError> {
        let run_store = crate::relay::api::run_store();

        #[derive(Serialize)]
        struct RunBrief {
            run_id: String,
            status: String,
            current_step: usize,
            total_steps: usize,
            current_profession: Option<String>,
            title: Option<String>,
        }

        let mut results = Vec::new();
        for run_id in &input.0.run_ids {
            if let Some(run) = crate::relay::store::get_run(run_store, run_id) {
                results.push(RunBrief {
                    run_id: run.run_id.clone(),
                    status: run.status.clone(),
                    current_step: run.current_step,
                    total_steps: run.total_steps,
                    current_profession: run.steps.get(run.current_step)
                        .map(|s| s.profession_id.clone()),
                    title: run.title.clone(),
                });
            }
        }

        text_response(&serde_json::json!({
            "requested": input.0.run_ids.len(),
            "found": results.len(),
            "runs": results,
        }))
    }

    // -----------------------------------------------------------------------
    // 23. Poll chat status
    // -----------------------------------------------------------------------
    #[tool(description = "Poll the current status of a Forge chat session — returns latest assistant reply, profession, and pending changes", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_poll_chat_status(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<PollChatStatusInput>,
    ) -> Result<CallToolResult, McpError> {
        let store = crate::forge::forge_sessions().lock().unwrap();
        let session = store
            .get(&input.0.sid)
            .ok_or_else(|| McpError::invalid_params("Session not found", None))?;

        let assistant_reply = session.messages.iter().rev().find(|m| m.role == "assistant").map(|m| m.content.clone());

        #[derive(Serialize)]
        struct ChatStatus {
            sid: String,
            status: String,
            assistant_reply: Option<String>,
            active_profession: Option<String>,
            pending_changes: bool,
            message_count: usize,
        }
        text_response(&ChatStatus {
            sid: session.id.clone(),
            status: format!("{:?}", session.status),
            assistant_reply,
            active_profession: session.active_profession.clone(),
            pending_changes: !session.pending_spec_changes.is_empty(),
            message_count: session.messages.len(),
        })
    }

    // -----------------------------------------------------------------------
    // 24. Poll run phase
    // -----------------------------------------------------------------------
    #[tool(description = "Get the current phase of a Relay Run — lightweight alternative to forge_get_run", annotations(read_only_hint = true, idempotent_hint = true))]
    async fn forge_poll_run_phase(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<PollRunPhaseInput>,
    ) -> Result<CallToolResult, McpError> {
        let run = crate::relay::store::get_run(
            crate::relay::api::run_store(),
            &input.0.run_id,
        )
        .ok_or_else(|| {
            McpError::invalid_params(
                "Run not found",
                Some(serde_json::json!({ "run_id": &input.0.run_id })),
            )
        })?;

        let waiting_on = run.waiting_for_gate.as_ref().map(|g| {
            format!("gate:{} step:{} prof:{}", "human", g.step_id, g.profession_id)
        });

        #[derive(Serialize)]
        struct RunPhase {
            run_id: String,
            status: String,
            current_step: usize,
            total_steps: usize,
            current_profession: Option<String>,
            waiting_on: Option<String>,
            title: Option<String>,
        }
        text_response(&RunPhase {
            run_id: run.run_id.clone(),
            status: run.status.clone(),
            current_step: run.current_step,
            total_steps: run.total_steps,
            current_profession: run.steps.get(run.current_step)
                .map(|s| s.profession_id.clone()),
            waiting_on,
            title: run.title.clone(),
        })
    }

    // -----------------------------------------------------------------------
    // 25. Advance run
    // -----------------------------------------------------------------------
    #[tool(description = "Manually advance a Relay Run to the next step", annotations(destructive_hint = false))]
    async fn forge_advance_run(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<AdvanceRunInput>,
    ) -> Result<CallToolResult, McpError> {
        let result = crate::relay::store::advance_run(
            crate::relay::api::run_store(),
            &input.0.run_id,
        )
        .ok_or_else(|| McpError::invalid_params("Run not found", None))?;

        let _ = crate::relay::api::event_sender().send(crate::relay::api::RunEventBroadcast {
            run_id: input.0.run_id.clone(),
            event_type: "step_advanced".into(),
            payload: None,
        });

        text_response(&serde_json::json!({
            "result": format!("{:?}", result),
            "run_id": input.0.run_id,
        }))
    }

    // -----------------------------------------------------------------------
    // 26. Submit handoff
    // -----------------------------------------------------------------------
    #[tool(description = "Submit a handoff document to advance a run from one agent to the next", annotations(destructive_hint = false))]
    async fn forge_submit_handoff(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<SubmitHandoffInput>,
    ) -> Result<CallToolResult, McpError> {
        let handoff: crate::relay::handoff::HandoffDocument = serde_json::from_value(input.0.handoff.clone())
            .map_err(|e| McpError::invalid_params(format!("Invalid handoff document: {e}"), None))?;

        let result = crate::relay::store::submit_handoff(
            crate::relay::api::run_store(),
            &input.0.run_id,
            handoff,
        )
        .ok_or_else(|| McpError::invalid_params("Run not found", None))?;

        let _ = crate::relay::api::event_sender().send(crate::relay::api::RunEventBroadcast {
            run_id: input.0.run_id.clone(),
            event_type: "handoff_submitted".into(),
            payload: None,
        });

        text_response(&serde_json::json!({
            "result": format!("{:?}", result),
            "run_id": input.0.run_id,
        }))
    }

    // -----------------------------------------------------------------------
    // 27. Resolve gate
    // -----------------------------------------------------------------------
    #[tool(description = "Resolve a pending gate (approve, reject, or edit) to continue a Relay Run", annotations(destructive_hint = false))]
    async fn forge_resolve_gate(
        &self,
        input: rmcp::handler::server::wrapper::Parameters<ResolveGateInput>,
    ) -> Result<CallToolResult, McpError> {
        let decision = match input.0.decision.as_str() {
            "approve" => crate::relay::pipeline::GateDecision::Approve,
            "reject" => crate::relay::pipeline::GateDecision::Reject {
                feedback: input.0.feedback.unwrap_or_default(),
            },
            "edit" => crate::relay::pipeline::GateDecision::Edit {
                changes: input.0.feedback.unwrap_or_default(),
            },
            other => {
                return Ok(error_text(format!(
                    "Invalid decision '{}'. Use 'approve', 'reject', or 'edit'.",
                    other
                )));
            }
        };

        let result = crate::relay::store::resolve_gate(
            crate::relay::api::run_store(),
            &input.0.run_id,
            decision,
        )
        .ok_or_else(|| McpError::invalid_params("Run not found", None))?;

        let _ = crate::relay::api::event_sender().send(crate::relay::api::RunEventBroadcast {
            run_id: input.0.run_id.clone(),
            event_type: "gate_resolved".into(),
            payload: None,
        });

        text_response(&serde_json::json!({
            "result": format!("{:?}", result),
            "run_id": input.0.run_id,
        }))
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
