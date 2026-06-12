# Multi-Relay Task Plan Implementation Plan

> **Date:** 2026-06-12  
> **Scope:** `backend/src/relay/*`, `backend/src/forge/*`, frontend Relay view, `.autoforge/task_plans/*.atom`  
> **Depends on:** `docs/design/multi-relay-task-plan.md`, `../auto-lang/crates/auto-atom`  
> **Estimated effort:** 2–3 weeks (1 engineer)

---

## 1. Goal

Implement the multi-relay TaskPlan system designed in `docs/design/multi-relay-task-plan.md`:

- AutoForge backend can parse Atom TaskPlan files using the external `auto-atom` crate.
- Assistant classifies user requests into `DIRECT | SINGLE_RELAY | MULTI_RELAY`.
- A deterministic `TaskPlanEngine` executes phases serially or in parallel, with join gates and cross-run handoffs.
- Runs carry TaskPlan metadata so the UI can show a tree of related runs.
- Dynamic TaskPlan generation by a Planner agent is supported via `register_task_plan`.

---

## 2. Prerequisites

- [x] Extract `auto-atom` crate from `auto-lang`.
- [x] Add static Atom parser to `auto-atom`.
- [x] Update `auto-lang` to depend on `auto-atom`.
- [ ] Add `auto-atom` dependency to AutoForge backend.

---

## 3. Implementation Phases

### Phase 1: Backend dependency + TaskPlan data model (2–3 days)

**Files:**
- `backend/Cargo.toml`
- `backend/src/relay/task_plan.rs` (new)
- `backend/src/relay/task_plan_parser.rs` (new)

**Tasks:**

1. Add `auto-atom` to `backend/Cargo.toml`:
   ```toml
   [dependencies]
   auto-atom = { path = "../../auto-lang/crates/auto-atom" }
   ```
2. Define `TaskPlan`, `Phase`, `RunRef`, `PhaseMode`, `TaskMode` structs in `relay/task_plan.rs`.
3. Implement `TryFrom<auto_val::Node> for TaskPlan`:
   - Validate root node name is `task_plan`.
   - Extract `id`, `version`, `title`, `description`, `default_mode` from props.
   - Map each child node named `phase` to `Phase`.
   - Map each child node named `run` inside a phase to `RunRef`.
   - Validate dependency graph is acyclic.
   - Validate all `flow_id`s exist in `FlowRegistry`.
   - Validate `input_from` path syntax.
4. Add helper `parse_task_plan(input: &str) -> AtomResult<TaskPlan>`.
5. Add unit tests for valid and invalid TaskPlan parsing.

**Definition of done:**
- `cargo test -p auto-forge` includes passing TaskPlan parser tests.
- Invalid TaskPlans produce line/column errors.

---

### Phase 2: TaskPlan registry (1 day)

**Files:**
- `backend/src/relay/task_plan_registry.rs` (new)
- `backend/src/relay/mod.rs`

**Tasks:**

1. Create `TaskPlanRegistry`:
   ```rust
   pub struct TaskPlanRegistry {
       builtins: HashMap<String, TaskPlan>,
       user: HashMap<String, TaskPlan>,
   }
   ```
2. Load built-ins via `include_str!` from `backend/src/relay/task_plans/builtin/*.atom`.
3. Load user plans from `.autoforge/task_plans/*.atom` at startup.
4. Implement `get`, `list`, `register`, `validate` methods.
5. Expose a global `Mutex<Option<TaskPlanRegistry>>` similar to `FlowRegistry`.
6. Wire registry initialization into backend startup.

**Definition of done:**
- Built-in and user TaskPlan files load at startup.
- Duplicate IDs are rejected with clear errors.

---

### Phase 3: Run metadata extensions (1 day)

**Files:**
- `backend/src/relay/store.rs`
- `backend/src/relay/api.rs` (request DTOs)
- `backend/src/relay/driver.rs`

**Tasks:**

1. Extend `RunMetadata`:
   ```rust
   pub struct RunMetadata {
       pub title: String,
       pub project_path: String,
       pub originating_chat_session: Option<String>, // now populated
       pub task_plan_id: Option<String>,             // NEW
       pub task_run_name: Option<String>,            // NEW
       pub phase_name: Option<String>,               // NEW
       pub phase_index: Option<usize>,               // NEW
       pub parent_run_id: Option<String>,            // NEW
       pub root_run_id: Option<String>,              // NEW
   }
   ```
2. Update `StartRunRequest` to accept optional TaskPlan context.
3. Update `driver.rs` to populate new metadata fields when starting a run from a TaskPlan.
4. Backward compatibility: old runs have `None` for all new fields.

**Definition of done:**
- Existing tests still pass.
- New runs from TaskPlan carry correct metadata.

---

### Phase 4: Cross-run handoff store (1–2 days)

**Files:**
- `backend/src/relay/handoff_store.rs` (new)
- `backend/src/relay/handoff.rs` (minor)
- `backend/src/relay/store.rs`

**Tasks:**

1. Create `HandoffStore` keyed by `(task_plan_id, phase_name, run_name)`.
2. Persist `HandoffDocument` as JSON next to the run store, e.g.:
   ```
   .autoforge/runs/{run_id}/handoff.json
   .autoforge/task_plans/{task_plan_id}/handoffs/{phase}.{run}.json
   ```
   Use whichever location is simpler; the design doc proposes task-plan-scoped storage.
3. Implement `save_handoff` and `load_handoff`.
4. Implement `resolve_path(path: &str) -> Option<ResolvedValue>` where path is like `phase.run.handoff.field`.
5. Call `save_handoff` when a relay run completes successfully.

**Definition of done:**
- A completed run's handoff can be loaded by path.
- Missing paths return `None` cleanly.

---

### Phase 5: TaskPlan engine (3–4 days)

**Files:**
- `backend/src/relay/task_plan_engine.rs` (new)
- `backend/src/relay/driver.rs` (reuse / adapt)

**Tasks:**

1. Define `TaskPlanEngine` and `TaskPlanStatus`:
   ```rust
   pub struct TaskPlanEngine {
       pub plan: TaskPlan,
       pub status: TaskPlanStatus,
       pub phase_states: HashMap<String, PhaseState>,
       pub run_states: HashMap<String, RunState>,
   }
   ```
2. Implement validation:
   - Acyclic dependency graph.
   - All referenced flow IDs exist.
   - All `input_from` paths are syntactically valid.
3. Implement execution loop:
   - Mark phases with no `depends_on` as ready.
   - For each ready phase:
     - `serial`: start runs one at a time, waiting for each to finish.
     - `parallel`: start all runs concurrently, wait for all via `JoinGate`.
   - On phase completion, mark dependent phases ready.
   - On any run failure, fail the phase and TaskPlan (default policy).
4. Implement `JoinGate`:
   ```rust
   pub struct JoinGate {
       pub expected: Vec<String>,
       pub completed: HashMap<String, HandoffDocument>,
       pub failed: Vec<String>,
   }
   ```
5. Implement `build_task_for_run(run_ref, resolved_inputs) -> String`.
6. Integrate with existing `relay::driver::drive_run` for actual run execution.
7. Emit SSE events for TaskPlan lifecycle.

**Definition of done:**
- Serial TaskPlan executes end-to-end.
- Parallel phase waits for all runs.
- Failed run fails the TaskPlan.

---

### Phase 6: Assistant work-mode classification (1–2 days)

**Files:**
- `backend/src/forge/mod.rs` (`ForgeSession`)
- `backend/src/forge/tools.rs` (`SpawnTaskPlanTool`)
- `.autoforge/souls/assistant.md` (or wherever Assistant soul lives)

**Tasks:**

1. Add to `ForgeSession`:
   ```rust
   pub work_mode: Option<WorkMode>,
   pub active_task_plan: Option<String>,
   pub active_relay_runs: Vec<String>,
   ```
2. Define `WorkMode` enum.
3. Update Assistant soul prompt to classify into `DIRECT | SINGLE_RELAY | MULTI_RELAY`.
4. Add `spawn_task_plan` tool to chat tools.
5. Persist chosen mode after first classification.

**Definition of done:**
- Assistant can call `spawn_task_plan`.
- Mode is persisted across turns.

---

### Phase 7: Dynamic TaskPlan generation tools (1–2 days)

**Files:**
- `backend/src/forge/tools.rs`
- `backend/src/relay/task_plan_registry.rs`

**Tasks:**

1. Add `RegisterTaskPlanTool`:
   ```rust
   pub struct RegisterTaskPlanInput {
       pub atom: String,
       pub file_path: Option<String>,
   }
   ```
2. Validate Atom and TaskPlan on registration.
3. Persist to `.autoforge/task_plans/{id}.atom` if `file_path` is provided.
4. Add to Planner profession's allowed tools.
5. Add `SpawnTaskPlanTool`:
   ```rust
   pub struct SpawnTaskPlanInput {
       pub task_plan_id: String,
       pub initial_input: Option<String>,
       pub mode: Option<String>, // "gsd" | "check"
   }
   ```

**Definition of done:**
- Planner can write and register a TaskPlan at runtime.
- Assistant can spawn a registered TaskPlan.

---

### Phase 8: API endpoints (1–2 days)

**Files:**
- `backend/src/relay/api.rs`

**Tasks:**

1. Add endpoints:
   ```text
   GET    /api/forge/relay/task_plans
   GET    /api/forge/relay/task_plans/{id}
   POST   /api/forge/relay/task_plans
   PUT    /api/forge/relay/task_plans/{id}
   DELETE /api/forge/relay/task_plans/{id}
   POST   /api/forge/relay/task_plans/validate
   POST   /api/forge/relay/task_plans/{id}/runs
   GET    /api/forge/relay/task_plans/runs
   GET    /api/forge/relay/task_plans/runs/{rid}
   POST   /api/forge/relay/task_plans/runs/{rid}/pause
   POST   /api/forge/relay/task_plans/runs/{rid}/resume
   POST   /api/forge/relay/task_plans/runs/{rid}/cancel
   ```
2. Add request/response DTOs.
3. Wire endpoints to `TaskPlanRegistry` and `TaskPlanEngine`.
4. Emit SSE events.

**Definition of done:**
- All endpoints respond correctly.
- SSE events are emitted for TaskPlan lifecycle.

---

### Phase 9: Frontend (3–4 days)

**Files:**
- `frontend/src/views/RelayView.vue` or new `TaskPlanView.vue`
- `frontend/src/components/relay/` (new)
- `frontend/src/composables/useTaskPlan.ts` (new)
- `frontend/src/components/chat/` (relay card updates)

**Tasks:**

1. Create `useTaskPlan` composable with API client.
2. Add TaskPlan list view.
3. Add TaskPlan detail / tree view:
   - Phases as rows or swimlanes.
   - Runs as status-colored cards.
   - Dependency arrows between phases.
4. Update chat to show TaskPlan cards when Assistant spawns one.
5. Update RelayView to show related TaskPlan runs via metadata.

**Definition of done:**
- Users can view and monitor TaskPlans in the UI.
- Chat shows spawned TaskPlan status.

---

### Phase 10: Integration tests (2–3 days)

**Files:**
- `backend/tests/task_plan.rs` (new)
- `backend/tests/task_plan_engine.rs` (new)

**Tasks:**

1. Test parsing all built-in TaskPlan files.
2. Test serial execution end-to-end.
3. Test parallel execution with join gate.
4. Test failure propagation.
5. Test cross-run handoff resolution.
6. Test dynamic TaskPlan registration and execution.

**Definition of done:**
- New integration tests pass.
- Existing relay tests still pass.

---

## 4. File Structure After Implementation

```
backend/src/relay/
  mod.rs                  # re-export TaskPlan, TaskPlanEngine, registry, parser
  flow.rs                 # unchanged (YAML FlowSpec)
  pipeline.rs             # unchanged
  driver.rs               # minor: accept TaskPlan metadata
  store.rs                # extended RunMetadata
  handoff.rs              # minor: save/load integration
  handoff_store.rs        # NEW
  task_plan.rs            # NEW: data model
  task_plan_parser.rs     # NEW: Atom -> TaskPlan
  task_plan_registry.rs   # NEW: registry
  task_plan_engine.rs     # NEW: execution engine
  api.rs                  # extended with TaskPlan endpoints
  task_plans/
    builtin/
      deferred-decompose.atom
      example-v2-api.atom

.autoforge/task_plans/
  example-v2-api.atom     # exists
  *.atom                  # user-defined plans
```

---

## 5. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `auto-atom` parser doesn't cover all Atom syntax users expect | Medium | Medium | Extend parser as needed; fall back to `auto_lang::atom::AtomReader` for dynamic Atom |
| Parallel run failure policy too strict | Medium | Medium | Start with "fail whole phase"; add `on_failure: continue` later |
| Run metadata changes break existing run store | Low | High | Keep new fields optional; test migration |
| Cross-repo `auto-atom` changes out of sync | Medium | Medium | Pin path dependency; add CI check for both repos |
| Frontend tree visualization complex | Medium | Medium | Start with simple list/Gantt; enhance later |

---

## 6. Open Questions

1. Should `register_task_plan` auto-approve in GSD mode, or stop for human validation?
2. Should TaskPlan failures allow per-run retry, or always fail the whole phase?
3. Should we support nested TaskPlans (a run's `flow_id` can be another TaskPlan)?
4. Where exactly should handoff files be persisted — run-scoped or task-plan-scoped?
5. Do we want hot-reload of `.autoforge/task_plans/*.atom` in dev mode?

---

## 7. Immediate Next Step

Start **Phase 1**: add `auto-atom` dependency to AutoForge backend and implement `TaskPlan` structs + parser.
