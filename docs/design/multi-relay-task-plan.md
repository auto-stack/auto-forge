# Multi-Relay Task Plans: Atom-Based Orchestration

> **Status:** Design Draft  
> **Scope:** `backend/src/relay/*`, `backend/src/forge/*`, `.autoforge/task_plans/*.atom`  
> **Date:** 2026-06-12

---

## 1. Problem Statement

AutoForge currently supports three cooperation primitives in chat:

| Primitive | When Used | Runs |
|-----------|-----------|------|
| `bring_in` | Switch profession mid-chat | 1 chat |
| `dispatch` | Run an isolated errand | 1 errand run |
| `spawn_relay` | Execute a single YAML `FlowSpec` | 1 pipeline run |

The user has described three work modes that do not all map cleanly to these primitives:

1. **Small change** → Assistant classifies as `DIRECT` and either answers or calls a Coder/Errand tool inline.
2. **Single goal** → Assistant classifies as `SINGLE_RELAY` and spawns one `FlowSpec` (e.g., `post-discovery`).
3. **Large goal** → Assistant classifies as `MULTI_RELAY`. The task is decomposed into a **tree of relay runs** that may execute in parallel phases, serial phases, or nested sub-flows, with explicit join gates and cross-run handoffs.

The current YAML `FlowSpec` is ideal for the sequential/branch/loop pipeline of a **single relay run**, but it is not expressive enough for multi-run orchestration: parallel fan-out, fork/join, cross-run context propagation, and dynamic step generation at runtime.

This document proposes a new layer called **TaskPlan** that is authored in the **Atom** format (the static data subset of AutoLang) and executed by a deterministic Rust orchestrator.

---

## 2. Design Decisions

### 2.1 Keep YAML `FlowSpec` for single-relay pipelines

- YAML `FlowSpec` already works and is being extended with conditional routing ([pipeline-configurability-and-dynamic-workflows.md](./pipeline-configurability-and-dynamic-workflows.md)).
- Single-relay flows remain the common case. Do not force a migration.
- `FlowSpec` is the **micro** unit of execution: one ordered, possibly branching/looping pipeline inside one run.

### 2.2 Add Atom `TaskPlan` for multi-relay orchestration

- `TaskPlan` is the **macro** unit of execution: a tree of phases and runs.
- It is written in **static Atom** — a JSON/XML-like block DSL that is the data-exchange format of AutoLang.
- Static Atom has no variables, functions, or runtime interpolation. It is pure data, suitable for a deterministic orchestrator.
- Choosing Atom keeps us aligned with the AutoLang ecosystem and makes future dynamic generation natural.

### 2.3 Use the new `auto-atom` crate

The full `auto-lang` crate in `../auto-lang/` pulls in many optional features (UI, Python FFI, networking, transpilers). Adding it as a direct dependency would significantly expand AutoForge's compile graph.

**Decision:** depend on the new `auto-atom` crate instead.

- `auto-atom` was extracted from `auto-lang` and lives at `../auto-lang/crates/auto-atom`.
- It depends only on `auto-val` and `thiserror`.
- It provides the core `Atom` data structures (`Atom`, `AtomBuilder`, `AtomError`, `AtomResult`).
- It includes a lightweight **static Atom parser** (`auto_atom::AtomParser`) that reads the static subset of Atom and produces `Atom` values directly.
- `auto-lang` re-exports `auto-atom` types for backward compatibility.

AutoForge will:

- Add `auto-atom = { path = "../auto-lang/crates/auto-atom" }` to `backend/Cargo.toml`.
- Use `auto_atom::AtomParser` to parse TaskPlan files.
- Map `Atom` values into `TaskPlan` structs (or deserialize via a small schema-driven mapper).
- Reject dynamic-Auto constructs (`var`, `for`, interpolation) at parse time.

### 2.4 Deterministic orchestration, no LLM in the loop driver

The TaskPlan executor is a Rust state machine. It decides when to start runs, when to wait for joins, and how to route handoffs. It does not call an LLM to decide routing. LLMs appear only inside individual relay runs as agent steps.

---

## 3. Work-Mode Classification

### 3.1 Upgrade the Assistant soul prompt

The Assistant profession must classify every user request into one of three modes before acting:

```text
work_mode: DIRECT | SINGLE_RELAY | MULTI_RELAY
```

Classification rules (prompt guidance):

| Mode | Criteria | Example |
|------|----------|---------|
| `DIRECT` | Question, clarification, trivial edit (< few files, no design), or isolated errand | "Explain this function", "Fix this typo" |
| `SINGLE_RELAY` | One well-scoped goal that fits a single profession pipeline | "Add JWT login using the existing auth module" |
| `MULTI_RELAY` | Large goal requiring decomposition, parallel workstreams, or multiple subsystems | "Build the v2 API with auth, billing, and admin modules" |

The Assistant persists the chosen mode in `ForgeSession`:

```rust
pub struct ForgeSession {
    pub id: String,
    pub messages: Vec<ForgeMessage>,
    pub active_profession: Option<String>,
    pub work_mode: Option<WorkMode>,          // NEW
    pub active_task_plan: Option<String>,     // NEW: task_plan_id if multi-relay
    pub active_relay_runs: Vec<String>,       // NEW: run_ids spawned from this chat
}

pub enum WorkMode { Direct, SingleRelay, MultiRelay }
```

### 3.2 Mode-specific behavior

| Mode | Assistant action |
|------|------------------|
| `DIRECT` | Answer, use Coder/Errand tools, or call `bring_in` another profession. |
| `SINGLE_RELAY` | Confirm goal, then call `spawn_relay` with a single `flow_id` (e.g., `post-discovery`). |
| `MULTI_RELAY` | Confirm high-level goal, then call `spawn_task_plan` with an Atom `TaskPlan` file or inline Atom. |

---

## 4. TaskPlan Data Model

### 4.1 Atom schema

A TaskPlan is an Atom document. The top-level node is `task_plan`.

```atom
// .autoforge/task_plans/api_v2.atom
task_plan(id: "api_v2", version: 1) {
    title: "Build v2 API"
    description: "Implement auth, billing, and admin modules in parallel then integrate."
    default_mode: "gsd"

    phase(name: "discovery") {
        mode: "serial"
        run(name: "discover") {
            flow_id: "goal-discovery"
            input: "Build the v2 API with auth, billing, and admin modules."
        }
    }

    phase(name: "design") {
        mode: "serial"
        depends_on: ["discovery"]
        run(name: "architecture") {
            flow_id: "post-discovery"
            input_from: "discover.handoff.goals"
        }
    }

    phase(name: "implementation") {
        mode: "parallel"
        depends_on: ["design"]

        run(name: "auth_module") {
            flow_id: "post-discovery"
            input_from: "design.handoff.specs"
            context: "Focus on the auth subsystem."
        }
        run(name: "billing_module") {
            flow_id: "post-discovery"
            input_from: "design.handoff.specs"
            context: "Focus on the billing subsystem."
        }
        run(name: "admin_module") {
            flow_id: "post-discovery"
            input_from: "design.handoff.specs"
            context: "Focus on the admin subsystem."
        }
    }

    phase(name: "integration") {
        mode: "serial"
        depends_on: ["implementation"]
        run(name: "merge_and_test") {
            flow_id: "post-discovery"
            input_from: [
                "implementation.auth_module.handoff.work_product",
                "implementation.billing_module.handoff.work_product",
                "implementation.admin_module.handoff.work_product"
            ]
        }
    }
}
```

### 4.2 Rust structs

```rust
// backend/src/relay/task_plan.rs

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub id: String,
    pub version: u32,
    pub title: Option<String>,
    pub description: Option<String>,
    pub default_mode: TaskMode,
    pub phases: Vec<Phase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskMode {
    Gsd,   // Get Shit Done
    Check, // Human reviews gates
}

impl Default for TaskMode {
    fn default() -> Self { TaskMode::Gsd }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    pub name: String,
    pub mode: PhaseMode,
    pub depends_on: Vec<String>,
    pub runs: Vec<RunRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseMode {
    Serial,
    Parallel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRef {
    pub name: String,
    pub flow_id: String,
    pub input: Option<String>,
    pub input_from: Option<InputSource>,
    pub context: Option<String>,
    pub mode_override: Option<TaskMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputSource {
    Single(String),
    Many(Vec<String>),
}
```

### 4.3 Addressing rules

Paths in `input_from` use a simple dot notation:

```text
<phase_name>.<run_name>.handoff.<field>
<phase_name>.<run_name>.handoff.work_product
<phase_name>.<run_name>.output.<custom_key>
```

Examples:

- `discovery.discover.handoff.goals` — handoff goals produced by the `discover` run.
- `implementation.auth_module.handoff.work_product` — list of file paths.
- `design.architecture.handoff.specs` — spec pointers.

The orchestrator resolves these paths when a phase becomes ready.

---

## 5. Execution Semantics

### 5.1 TaskPlan executor

A new `TaskPlanEngine` is the top-level state machine:

```rust
// backend/src/relay/task_plan_engine.rs

pub struct TaskPlanEngine {
    pub plan: TaskPlan,
    pub status: TaskPlanStatus,
    pub phase_states: HashMap<String, PhaseState>,
    pub run_states: HashMap<String, RunState>,
    pub run_id_to_task_run: HashMap<String, String>, // relay run_id -> task run name
}

pub enum TaskPlanStatus {
    Idle,
    Running,
    WaitingForHuman,
    Completed,
    Failed,
}
```

Lifecycle:

1. Validate the plan (acyclic dependencies, all `flow_id`s known, all path refs syntactically valid).
2. Mark phases whose `depends_on` is empty as **ready**.
3. For each ready phase:
   - `serial`: start runs one after another.
   - `parallel`: start all runs concurrently.
4. Wait for all runs in a phase to reach a terminal state (`Completed` or `Failed`).
5. Mark dependent phases as ready and repeat.
6. When all phases complete, finalize the TaskPlan.

### 5.2 Mapping a `RunRef` to a relay run

When the executor starts a `RunRef`, it creates a `StartRunRequest` for the existing relay pipeline:

```rust
StartRunRequest {
    flow_id: run_ref.flow_id,
    task: build_task_text(run_ref, &resolved_inputs),
    mode: run_ref.mode_override.unwrap_or(plan.default_mode),
    originating_chat_session: Some(chat_session_id),
    parent_run_id: None, // set per run metadata below
}
```

### 5.3 Run metadata extensions

`RunMetadata` gains fields for parent/child relationships and TaskPlan grouping:

```rust
// backend/src/relay/store.rs

pub struct RunMetadata {
    pub title: String,
    pub project_path: String,
    pub originating_chat_session: Option<String>, // now populated
    pub task_plan_id: Option<String>,             // NEW
    pub task_run_name: Option<String>,            // NEW: name inside TaskPlan
    pub phase_name: Option<String>,               // NEW
    pub phase_index: Option<usize>,               // NEW
    pub parent_run_id: Option<String>,            // NEW: for nested TaskPlans
    pub root_run_id: Option<String>,              // NEW: topmost ancestor
}
```

These fields let the UI show a tree of runs belonging to one TaskPlan.

### 5.4 Join gate for parallel phases

Parallel phases use a `JoinGate`:

```rust
pub struct JoinGate {
    pub phase_name: String,
    pub expected: Vec<String>,
    pub completed: HashMap<String, HandoffDocument>,
    pub failed: Vec<String>,
}
```

Policy when a parallel run fails:

- **Default**: fail the entire phase (and TaskPlan).
- **Optional future**: add `on_failure: "continue"` to a phase to allow partial results.

### 5.5 Cross-run handoff persistence

A completed relay run already produces a `HandoffDocument`. For TaskPlan execution we persist it in a new `HandoffStore` keyed by `(task_plan_id, phase_name, run_name)`:

```rust
pub struct HandoffStore;

impl HandoffStore {
    pub fn save(&self, task_plan_id: &str, phase: &str, run: &str, handoff: &HandoffDocument);
    pub fn load(&self, task_plan_id: &str, phase: &str, run: &str) -> Option<HandoffDocument>;
    pub fn resolve_path(&self, path: &str) -> Option<ResolvedValue>;
}
```

This lets later phases query previous handoffs by path.

---

## 6. Dynamic Task Plans

### 6.1 The Planner agent writes TaskPlan at runtime

For very large or ambiguous goals, the Assistant can hand off to a **Planner** profession whose job is to decompose the goal and write an Atom TaskPlan.

Flow:

1. User asks a large question.
2. Assistant classifies as `MULTI_RELAY`.
3. Assistant calls `spawn_task_plan` with a minimal TaskPlan containing one `planner` run:
   ```atom
   task_plan(id: "deferred_api_v2") {
       phase(name: "planning") {
           mode: "serial"
           run(name: "write_plan") {
               flow_id: "planner-decompose"
               input: "Build the v2 API with auth, billing, and admin modules."
           }
       }
   }
   ```
4. The `planner-decompose` flow runs a Planner agent that writes `.autoforge/task_plans/api_v2.atom`.
5. The Planner's handoff contains `task_plan_id: "api_v2"`.
6. The executor detects the new TaskPlan and starts it automatically.

### 6.2 Tool: `register_task_plan`

Agents that generate TaskPlans need a tool to register them:

```rust
pub struct RegisterTaskPlanInput {
    pub atom: String, // inline Atom text
    pub file_path: Option<String>, // optional: persist to this path
}

pub struct RegisterTaskPlanOutput {
    pub task_plan_id: String,
    pub validation_result: ValidationResult,
}
```

Validation runs the same rules as static file loading.

### 6.3 Tool: `spawn_task_plan`

Chat agents call this to start a TaskPlan:

```rust
pub struct SpawnTaskPlanInput {
    pub task_plan_id: String, // registry id or file path
    pub initial_input: Option<String>,
    pub mode: Option<String>, // "gsd" | "check"
}
```

---

## 7. Registry and Storage

### 7.1 TaskPlanRegistry

```rust
// backend/src/relay/task_plan_registry.rs

pub struct TaskPlanRegistry {
    builtins: HashMap<String, TaskPlan>,
    user: HashMap<String, TaskPlan>, // loaded from .autoforge/task_plans/*.atom
}

impl TaskPlanRegistry {
    pub fn load(data_dir: &Path) -> Result<Self>;
    pub fn get(&self, id: &str) -> Option<&TaskPlan>;
    pub fn list(&self) -> Vec<TaskPlanSummary>;
    pub fn register(&mut self, plan: TaskPlan) -> Result<(), Vec<String>>;
    pub fn validate(&self, plan: &TaskPlan) -> Vec<String>;
}
```

### 7.2 File layout

```text
.autoforge/
  flows/
    *.yml                 # single-relay FlowSpec (unchanged)
  task_plans/
    *.atom                # multi-relay TaskPlan (new)
```

Built-in TaskPlans can be embedded with `include_str!`:

```rust
const BUILTIN_TASK_PLANS: &[(&str, &str)] = &[
    ("deferred-decompose", include_str!("builtin/deferred-decompose.atom")),
];
```

---

## 8. Atom Parsing

### 8.1 Use `auto_atom::AtomParser`

The `auto-atom` crate now provides a static Atom parser. AutoForge uses it to parse TaskPlan files:

```rust
use auto_atom::AtomParser;
use auto_val::Value;

pub fn parse_task_plan(input: &str) -> AtomResult<TaskPlan> {
    let atom = AtomParser::parse(input)?;
    let node = atom.into_node()?; // Atom::Node -> Node
    TaskPlan::try_from(node)
}
```

### 8.2 Supported static subset

The parser accepts:

```ebnf
document    ::= value
value       ::= node | object | array | literal
node        ::= ident [ "(" args ")" ] [ "{" children "}" ]
args        ::= value ("," value)*
children    ::= (pair | node)*
object      ::= "{" pair* "}"
array       ::= "[" value* "]"
pair        ::= ident ":" value
literal     ::= string | number | bool | null
ident       ::= [A-Za-z_][A-Za-z0-9_-]*
```

Newlines may replace commas in arrays, objects, and node bodies.

### 8.3 Mapping to TaskPlan

```rust
// backend/src/relay/task_plan.rs

impl TryFrom<auto_val::Node> for TaskPlan {
    type Error = AtomError;

    fn try_from(node: auto_val::Node) -> Result<Self, Self::Error> {
        if node.name != "task_plan" {
            return Err(AtomError::ValidationError(
                "expected root node 'task_plan'".to_string()
            ));
        }
        // Extract id, version, title, description, default_mode from props.
        // Iterate over child nodes named 'phase' and map each to Phase.
        // ...
    }
}
```

### 8.4 Error handling

The parser reports line/column for:

- Missing required property (`id`, `version`, `phases`).
- Unknown property or child node.
- Invalid `phase.mode` value.
- Duplicate phase/run names.
- Dynamic-Auto syntax (`var`, `for`, `#{}`).

---

## 9. API Surface

Add to `backend/src/relay/api.rs`:

```text
GET    /api/forge/relay/task_plans            → ListTaskPlanResponse
GET    /api/forge/relay/task_plans/{id}       → TaskPlan
POST   /api/forge/relay/task_plans            → CreateTaskPlan (Atom body)
PUT    /api/forge/relay/task_plans/{id}       → UpdateTaskPlan
DELETE /api/forge/relay/task_plans/{id}       → Delete user TaskPlan
POST   /api/forge/relay/task_plans/validate   → ValidateTaskPlanResponse

POST   /api/forge/relay/task_plans/{id}/runs  → StartTaskPlanRun
GET    /api/forge/relay/task_plans/runs       → ListTaskPlanRuns
GET    /api/forge/relay/task_plans/runs/{rid} → TaskPlanRunStatus
POST   /api/forge/relay/task_plans/runs/{rid}/pause
POST   /api/forge/relay/task_plans/runs/{rid}/resume
POST   /api/forge/relay/task_plans/runs/{rid}/cancel
```

SSE events:

- `TaskPlanStarted`
- `TaskPlanPhaseStarted`
- `TaskPlanRunStarted`
- `TaskPlanRunCompleted`
- `TaskPlanPhaseCompleted`
- `TaskPlanWaitingHuman`
- `TaskPlanCompleted`
- `TaskPlanFailed`

---

## 10. UI Considerations

- **Chat**: Assistant shows the selected `work_mode` and a card for the active TaskPlan.
- **Relay view**: Add a "Task Plans" tab. Each TaskPlan run renders as a **Gantt-like tree**:
  - Phases as rows.
  - Runs as bars inside phases.
  - Dependency arrows between phases.
  - Status colors (idle/running/completed/failed/waiting).
- **Run detail**: Show the parent TaskPlan and sibling runs via `RunMetadata` fields.

---

## 11. Migration Path

| Phase | Change | Compatibility |
|-------|--------|---------------|
| 1 | Add `TaskPlan` structs, Atom parser, registry, and `spawn_task_plan` tool | Existing flows unchanged |
| 2 | Add run metadata fields (`task_plan_id`, `phase_name`, etc.) | Old runs have `None`; UI shows flat list |
| 3 | Build `TaskPlanEngine` with serial/parallel execution | Single-relay runs still use `PipelineEngine` |
| 4 | Add dynamic TaskPlan generation via Planner agent | Optional; existing workflows untouched |
| 5 | (Future) Migrate YAML FlowSpec to Atom if desired | Requires spec/design doc of its own |

---

## 12. Relationship to Existing Docs

- [agents-relay-orchestration.md](./agents-relay-orchestration.md) — defines the Relay philosophy, Soul/Profession, and single-relay driver.
- [pipeline-configurability-and-dynamic-workflows.md](./pipeline-configurability-and-dynamic-workflows.md) — defines YAML `FlowSpec`, conditional routing, and CRUD API.
- This doc adds the **multi-relay macro layer** on top.

---

## 13. Open Questions

1. Should failed parallel runs offer a per-run retry, or always fail the whole phase?
2. Should `input_from` support transformation expressions (e.g., joining multiple handoff summaries)? If so, is that a future mini-DSL or a tool call?
3. Should TaskPlans be allowed to nest other TaskPlans as runs, or only `FlowSpec`s?
4. Should the planner-generated TaskPlan be auto-approved in GSD mode, or does it stop for a human gate?
5. Do we want a file-system watcher for `.autoforge/task_plans/*.atom` hot-reload?

---

## 14. Summary

- Introduce **TaskPlan** as the macro orchestration layer for multi-relay work.
- Author TaskPlans in **static Atom**, keeping YAML `FlowSpec` for single-relay pipelines.
- Depend on the new **`auto-atom`** crate for Atom data structures and the static Atom parser.
- Extend `RunMetadata` to track TaskPlan parentage and phase/run identity.
- Build a deterministic `TaskPlanEngine` that executes serial and parallel phases with join gates.
- Support **dynamic TaskPlan generation** by a Planner agent at runtime.
- Add registry, validation, API endpoints, and UI tree visualization.
