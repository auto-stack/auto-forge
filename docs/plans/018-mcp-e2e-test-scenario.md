# MCP End-to-End Test Scenario for AutoForge

## Overview

This document describes a complete end-to-end validation workflow that exercises
AutoForge entirely through the MCP interface. It covers project setup, chat
sessions, Relay Run execution, gate resolution, handoff submission, and state
verification.

**Prerequisites:**
- AutoForge backend running on `http://127.0.0.1:3031/mcp`
- MCP client configured (e.g. Claude Desktop, Cursor, or custom client)
- At least one API source configured and tested
- One flow available (e.g. `standard-v1`)

---

## Test Flow

### Phase 1: Project Setup

#### Step 1.1 — Get initial project status
```json
// tool: forge_get_project_status
// expected: { "project_open": false, "project_path": null }
```

#### Step 1.2 — Open a project
```json
// tool: forge_open_project
{ "path": "D:/autostack/auto-forge" }
// expected: project metadata with root path
```

#### Step 1.3 — Verify project is open
```json
// tool: forge_get_project_status
// expected: { "project_open": true, "project_path": "D:/autostack/auto-forge" }
```

#### Step 1.4 — Read project specs
```json
// tool: forge_read_specs
{ "project": "D:/autostack/auto-forge" }
// expected: specs document with sections
```

---

### Phase 2: Chat Session Lifecycle

#### Step 2.1 — Create a session
```json
// tool: forge_create_session
{ "notebook_sid": null, "project_path": "D:/autostack/auto-forge" }
// expected: { "id": "fs-<uuid>", "status": "Idle", "message_count": 0 }
```

**Record `sid` for subsequent steps.**

#### Step 2.2 — Poll empty session
```json
// tool: forge_poll_chat_status
{ "sid": "<sid>" }
// expected: { "status": "Idle", "assistant_reply": null, "message_count": 0 }
```

#### Step 2.3 — Send a message
```json
// tool: forge_send_message
{ "sid": "<sid>", "content": "Hello, what can you do?", "profession_id": null }
// expected: { "assistant_message": "...", "error": null }
```

#### Step 2.4 — Poll chat status after message
```json
// tool: forge_poll_chat_status
{ "sid": "<sid>" }
// expected: status "Idle", assistant_reply present, message_count >= 2
```

#### Step 2.5 — Get session with history
```json
// tool: forge_get_session
{ "sid": "<sid>", "include_history": true }
// expected: messages array with roles ["user", "assistant"], timestamps, profession_ids
```

#### Step 2.6 — Get session without history (default)
```json
// tool: forge_get_session
{ "sid": "<sid>" }
// expected: message_count present but messages field omitted
```

---

### Phase 3: Relay Run — Full Lifecycle

#### Step 3.1 — List available flows
```json
// tool: forge_list_professions
// note: this shows professions; flow_id is typically "standard-v1"
```

#### Step 3.2 — Start a Relay Run
```json
// tool: forge_start_relay_run
{
  "flow_id": "standard-v1",
  "task": "Add a new endpoint to list API sources with pagination"
}
// expected: { "run_id": "run-<uuid>", "status": "pending", "current_step": 0, ... }
```

**Record `run_id` for subsequent steps.**

#### Step 3.3 — Poll run phase immediately
```json
// tool: forge_poll_run_phase
{ "run_id": "<run_id>" }
// expected: status "pending" or "running", current_step 0 or 1
```

#### Step 3.4 — Wait and poll again
```json
// tool: forge_poll_run_phase
{ "run_id": "<run_id>" }
// Repeat every 2-3 seconds until status changes from "pending" / "running"
```

#### Step 3.5 — Get full run state
```json
// tool: forge_get_run
{ "run_id": "<run_id>" }
// expected: complete RunState with steps, events, title
```

#### Step 3.6 — List all runs
```json
// tool: forge_list_runs
// expected: array containing the new run with summary fields
```

---

### Phase 4: Manual Run Control (Testing/Debugging)

> These steps are typically used when a run is paused or when testing the
> pipeline without waiting for LLM responses.

#### Step 4.1 — Advance a run manually
```json
// tool: forge_advance_run
{ "run_id": "<run_id>" }
// expected: { "result": "ExecuteStep { ... }", "run_id": "<run_id>" }
```

#### Step 4.2 — Poll to see advancement
```json
// tool: forge_poll_run_phase
{ "run_id": "<run_id>" }
// expected: current_step incremented, new profession
```

#### Step 4.3 — Submit a handoff (simulated)
```json
// tool: forge_submit_handoff
{
  "run_id": "<run_id>",
  "handoff": {
    "from": "planner",
    "to": "architect",
    "run_id": "<run_id>",
    "checkpoint_id": 1,
    "summary": "Decomposed task into 3 sub-tasks",
    "decisions": [],
    "open_questions": [],
    "spec_updates": [],
    "work_product": [],
    "context_for_next": { "files_to_read": [], "specs_to_follow": [], "warnings": [] },
    "token_usage": { "step_input": 1000, "step_output": 500, "cumulative": 1500, "budget_remaining": 8500 }
  }
}
// expected: { "result": "...", "run_id": "<run_id>" }
```

#### Step 4.4 — Resolve a gate
```json
// tool: forge_resolve_gate
{
  "run_id": "<run_id>",
  "decision": "approve",
  "feedback": null
}
// expected: { "result": "...", "run_id": "<run_id>" }
```

#### Step 4.5 — Reject with feedback
```json
// tool: forge_resolve_gate
{
  "run_id": "<run_id>",
  "decision": "reject",
  "feedback": "The spec is missing error handling details"
}
// expected: run routes back to same step for redraft
```

---

### Phase 5: Batch Operations

#### Step 5.1 — Start batch runs
```json
// tool: forge_batch_start_runs
{
  "flow_id": "standard-v1",
  "task": "Refactor auth module",
  "count": 3
}
// expected: { "run_ids": ["run-batch-0-...", "run-batch-1-...", "run-batch-2-..."], "count": 3 }
```

#### Step 5.2 — Batch poll phases
```json
// tool: forge_batch_get_results
{
  "run_ids": ["run-batch-0-...", "run-batch-1-...", "run-batch-2-..."]
}
// expected: array of 3 RunBrief objects with statuses
```

#### Step 5.3 — Poll individual batch run
```json
// tool: forge_poll_run_phase
{ "run_id": "run-batch-0-..." }
```

---

### Phase 6: Spec Workflow Integration

#### Step 6.1 — Send a message that triggers spec changes
```json
// tool: forge_send_message
{
  "sid": "<sid>",
  "content": "Update the specs to add a new 'Monitoring' section",
  "profession_id": null
}
```

#### Step 6.2 — Poll for pending changes
```json
// tool: forge_poll_chat_status
{ "sid": "<sid>" }
// expected: pending_changes = true, status may be "WaitingApproval"
```

#### Step 6.3 — Approve spec changes
```json
// tool: forge_approve_spec
{ "sid": "<sid>" }
// expected: { "status": "ok", "phase": "execution", "applied_changes": N }
```

#### Step 6.4 — Reject spec changes (alternative path)
```json
// tool: forge_reject_spec
{ "sid": "<sid>" }
// expected: { "status": "rejected" }
```

#### Step 6.5 — Verify specs updated
```json
// tool: forge_read_specs
{ "project": "D:/autostack/auto-forge" }
// expected: updated specs document reflecting approved changes
```

---

### Phase 7: File System Operations

#### Step 7.1 — Browse project root
```json
// tool: forge_browse_directory
{ "path": "." }
// expected: directory entries with names, sizes, is_dir flags
```

#### Step 7.2 — Read a specific file
```json
// tool: forge_read_file
{ "path": "Cargo.toml" }
// expected: { "path": "Cargo.toml", "content": "...", "size": N }
```

---

### Phase 8: Performance & Diagnostics

#### Step 8.1 — Get performance logs
```json
// tool: forge_get_performance_logs
// expected: recent backend.log lines with chat_turn_complete and tool_execute entries
```

#### Step 8.2 — List API sources
```json
// tool: forge_list_api_sources
// expected: array of configured API sources
```

#### Step 8.3 — Test API connection
```json
// tool: forge_test_api_connection
{ "id": "default" }
// expected: ConnectionTestResult with latency and status
```

---

### Phase 9: Cleanup

#### Step 9.1 — Delete chat session
```json
// tool: forge_delete_session
{ "sid": "<sid>" }
// expected: { "deleted": true, "sid": "<sid>" }
```

#### Step 9.2 — Verify session deleted
```json
// tool: forge_get_session
{ "sid": "<sid>" }
// expected: error "Session not found"
```

#### Step 9.3 — Close project
```json
// tool: forge_close_project
// expected: { "status": "closed" }
```

---

## Validation Checklist

| # | Check | Tools Used |
|---|-------|-----------|
| 1 | Project can be opened and closed | `forge_open_project`, `forge_close_project`, `forge_get_project_status` |
| 2 | Chat session can be created, messaged, polled, and deleted | `forge_create_session`, `forge_send_message`, `forge_poll_chat_status`, `forge_get_session`, `forge_delete_session` |
| 3 | Session history can be retrieved with `include_history=true` | `forge_get_session` |
| 4 | Relay Run can be started and polled | `forge_start_relay_run`, `forge_poll_run_phase`, `forge_get_run`, `forge_list_runs` |
| 5 | Run can be manually advanced | `forge_advance_run` |
| 6 | Handoff can be submitted | `forge_submit_handoff` |
| 7 | Gate can be approved, rejected, or edited | `forge_resolve_gate` |
| 8 | Batch runs can be started and queried | `forge_batch_start_runs`, `forge_batch_get_results` |
| 9 | Spec changes can be approved/rejected | `forge_send_message`, `forge_poll_chat_status`, `forge_approve_spec`, `forge_reject_spec`, `forge_read_specs` |
| 10 | Files can be read and directories browsed | `forge_read_file`, `forge_browse_directory` |
| 11 | Performance logs are accessible | `forge_get_performance_logs` |
| 12 | API sources can be listed and tested | `forge_list_api_sources`, `forge_test_api_connection` |
| 13 | Professions can be listed | `forge_list_professions` |

---

## Notes for Automated Testing

- **Polling loops:** Use `forge_poll_chat_status` and `forge_poll_run_phase` in a
  retry loop with exponential backoff (e.g. 1s, 2s, 4s, max 60s) rather than
  blocking on `forge_send_message` or `forge_start_relay_run`.

- **Gate handling:** If `forge_poll_run_phase` returns `waiting_on` field, the
  run is paused at a gate. Use `forge_resolve_gate` to continue.

- **Async nature:** `forge_send_message` and `forge_start_relay_run` return
  immediately; the actual AI work happens in background tasks. Always poll
  to verify completion.

- **Session→Run correlation:** Currently there is no automatic linkage from a
  chat session to a run it spawned. If you need this correlation, track both
  IDs in your test harness.
