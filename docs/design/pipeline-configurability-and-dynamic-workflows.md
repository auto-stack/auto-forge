# Pipeline Configurability & Dynamic Workflows

> **Status:** Design Draft  
> **Scope:** `backend/src/relay/*`, `.autoforge/flows/*`, frontend Relay API clients  
> **Author:** AutoForge Team  
> **Date:** 2026-06-09

---

## 1. Problem Statement

The current Relay pipeline is **partially configurable but not dynamic**.

- Flow definitions already exist as `FlowSpec` structs and can be loaded from `.autoforge/flows/*.yml` at startup.
- However, the 9 built-in flows (`standard-spec`, `fast-track`, `auto-discovery`, `bug-fix`, etc.) are **hard-coded in Rust** (`backend/src/relay/flows.rs`).
- The `FlowRegistry` is static: loaded once at startup, with no runtime CRUD, no hot-reload, and no validation API.
- Routing is limited to `Next`, `Branch` (based on the agent-written `to` field in a handoff), and `Loop`. There is no **runtime condition evaluation** based on step outputs, token usage, validator results, or external state.
- Steps are strictly sequential. There is no parallelism, no sub-flow composition, and no runtime flow mutation.

This design doc proposes a phased path from the current state to **configurable flows** (Level 1) and **semidynamic routing** (Level 2), while keeping the deterministic Rust state-machine core intact.

---

## 2. Current State Audit

### 2.1 Data Model (`backend/src/relay/flow.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSpec {
    pub id: String,
    pub steps: Vec<FlowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowStep {
    pub id: String,
    pub profession_id: String,
    pub agent_config_id: Option<String>,
    pub gate: GateType,
    pub max_turns: Option<u32>,
    pub exit: ExitRouting,
    pub validators: Vec<StepValidator>,
    pub tool_guard: Option<ToolGuard>,
}

#[derive(...)]
pub enum ExitRouting {
    Next,
    Branch { on: String, arms: HashMap<String, String>, default: String },
    Loop { target_step_id: String, max_iterations: u32 },
}
```

### 2.2 Registry (`backend/src/relay/flows.rs`)

- `FlowRegistry::new(data_dir)` loads built-ins + YAML overrides.
- Built-ins are Rust functions (`standard_spec_flow()`, `fast_track_flow()`, ...).
- YAML files live in `<data_dir>/.autoforge/flows/*.y{,a}ml`.
- Global static: `static FLOW_REGISTRY: Mutex<Option<FlowRegistry>>`.
- `get_flow()` supports hyphen/underscore aliases.

### 2.3 API (`backend/src/relay/api.rs`)

- `POST /api/forge/relay/runs` accepts `flow_id` **or** inline `steps` array.
- No endpoints exist to list, create, update, delete, or validate flows.

### 2.4 Engine (`backend/src/relay/pipeline.rs`)

- `PipelineEngine` is a deterministic state machine.
- `advance()` checks gates, emits `ExecuteStep` / `WaitForHuman` / `Completed` / `Failed`.
- `submit_handoff()` records history, tracks budget, validates output, and routes via `ExitRouting`.
- Auto-retry and escalation logic is hard-coded for the `code` step.

---

## 3. Design Principles

1. **Deterministic orchestration stays in Rust.** The engine remains zero-LLM-token. No LLM decides routing directly.
2. **Flow definitions are data, not code.** All flows (built-in + user-defined) should live as serialized specs, not Rust functions.
3. **Incremental adoption.** Phase 1 should make the existing 9 flows movable to YAML without changing their behavior.
4. **Backward compatibility.** Existing `StartRunRequest` with inline `steps` and hard-coded `flow_id` references must keep working.
5. **Fail fast.** Flows are validated at registration time, not at step 7 of a 3-hour run.

---

## 4. Phase 1: Static Configurability

### 4.1 Built-in Flow YAML Migration

Move the 9 built-in flows from `flows.rs` into YAML files embedded at compile time.

**New file layout:**

```
backend/
  src/
    relay/
      flows/
        mod.rs                 # FlowRegistry + loader
        builtin/
          standard-spec.yml
          fast-track.yml
          auto-discovery.yml
          post-discovery.yml
          bug-fix.yml
          goal-discovery.yml
          doc-patch.yml
          spec-tweak.yml
          superpower.yml
```

**Rust loader:**

```rust
// backend/src/relay/flows/mod.rs
const BUILTIN_FLOWS: &[(&str, &str)] = &[
    ("standard-spec-driven-development", include_str!("builtin/standard-spec.yml")),
    ("fast-track", include_str!("builtin/fast-track.yml")),
    // ...
];
```

The 9 `*_flow()` functions in `flows.rs` are deleted. Tests in `flows.rs` are kept but now deserialize from YAML.

### 4.2 User Flow Directory

User-defined flows continue to live in:

```
<project-root>/.autoforge/flows/*.yml
```

User flows **override** built-ins of the same `id` (current behavior preserved).

### 4.3 Flow CRUD API

New endpoints:

```
GET    /api/forge/relay/flows              → ListFlowResponse
GET    /api/forge/relay/flows/{flow_id}    → FlowSpec
POST   /api/forge/relay/flows              → create flow (YAML/JSON body)
PUT    /api/forge/relay/flows/{flow_id}    → update flow
DELETE /api/forge/relay/flows/{flow_id}    → delete flow
POST   /api/forge/relay/flows/validate     → ValidateFlowResponse
```

For safety:
- Built-in flows are **read-only** via these endpoints. You can clone them to a user flow and modify.
- `DELETE` only removes `.autoforge/flows/{flow_id}.yml`.

#### Request/Response Schemas

```rust
#[derive(Deserialize)]
pub struct CreateFlowRequest {
    pub flow_id: String,
    pub spec: FlowSpec,           // Or raw YAML string if preferring text body
}

#[derive(Serialize)]
pub struct ListFlowResponse {
    pub flows: Vec<FlowSummary>,
}

#[derive(Serialize)]
pub struct FlowSummary {
    pub id: String,
    pub source: FlowSource,       // Builtin | User
    pub step_count: usize,
}

#[derive(Deserialize)]
pub struct ValidateFlowRequest {
    pub spec: FlowSpec,
}

#[derive(Serialize)]
pub struct ValidateFlowResponse {
    pub valid: bool,
    pub errors: Vec<String>,
}
```

### 4.4 Flow Validation Rules

Validation runs on:
- Server startup (for all registered flows)
- `POST /flows/validate`
- `POST /flows` and `PUT /flows/{id}`

Rules:

| # | Rule | Severity |
|---|------|----------|
| 1 | `flow.id` matches filename stem (for user flows) | Warning |
| 2 | All `step.id` values are unique | Error |
| 3 | All `step.profession_id` exist in `ProfessionRegistry` | Error |
| 4 | `ExitRouting::Branch` arms reference known `step_id` | Error |
| 5 | `ExitRouting::Loop` target references known `step_id` | Error |
| 6 | No unreachable steps (all steps reachable from step 0) | Warning |
| 7 | No infinite loops without `max_iterations` cap | Warning |
| 8 | `tool_guard` references only tools in `ToolRegistry` | Error |
| 9 | `validators` reference only known validator types | Error |

Implementation: add `FlowSpec::validate(&self, professions: &ProfessionRegistry, tools: &ToolRegistry) -> Vec<ValidationIssue>`.

### 4.5 Flow Registry Reload

Replace the static `Mutex<Option<FlowRegistry>>` with a reloadable registry:

```rust
pub struct FlowRegistry {
    builtins: HashMap<String, FlowSpec>,
    user: HashMap<String, FlowSpec>,
}

impl FlowRegistry {
    pub fn reload(&mut self, data_dir: &Path) { ... }
}
```

Expose:

```
POST /api/forge/relay/flows/reload   # reload from disk (admin only)
```

Also consider a file-system watcher (e.g. `notify` crate) for development mode, but this is optional and gated behind a config flag.

---

## 5. Phase 2: Conditional / Responsive Routing

### 5.1 Motivation

Current `Branch` routing depends on the previous agent writing a correct `to` field in the handoff. This is brittle:
- Agents sometimes mis-name the next profession.
- There is no way to route based on objective data (token usage, test exit code, validator failure, spec diff size).

### 5.2 New Exit Routing: `Condition`

Extend `ExitRouting`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitRouting {
    Next,
    Branch { on: String, arms: HashMap<String, String>, default: String },
    Loop { target_step_id: String, max_iterations: u32 },
    Condition {
        condition: RoutingCondition,
        true_branch: Box<ExitRouting>,
        false_branch: Box<ExitRouting>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RoutingCondition {
    /// True if any validator failed for the current step.
    ValidatorFailed,
    /// True if the handoff's `to` field matches a value.
    HandoffFieldEquals { field: String, value: String },
    /// True if cumulative token usage exceeds threshold.
    TokenUsageCumulativeExceeds { limit: u64 },
    /// True if step token usage exceeds threshold.
    TokenUsageStepExceeds { limit: u64 },
    /// True if a work_product file matching pattern was produced.
    WorkProductExists { glob: String },
    /// Logical AND of multiple conditions.
    All(Vec<RoutingCondition>),
    /// Logical OR of multiple conditions.
    Any(Vec<RoutingCondition>),
    /// Negation.
    Not(Box<RoutingCondition>),
}
```

### 5.3 YAML Example

```yaml
id: bug-fix
steps:
  - id: intake
    profession_id: assistant
    exit: next

  - id: code
    profession_id: coder
    validators:
      - type: work_product_has_extensions
        content:
          exts: [".rs", ".ts", ".vue"]
    exit:
      type: condition
      condition:
        type: validator_failed
      true_branch:
        type: loop
        target_step_id: code
        max_iterations: 3
      false_branch: next

  - id: test
    profession_id: tester
    exit:
      type: condition
      condition:
        type: any
        content:
          - type: validator_failed
          - type: handoff_field_equals
            content:
              field: to
              value: code
      true_branch:
        type: loop
        target_step_id: code
        max_iterations: 3
      false_branch: next

  - id: review
    profession_id: reviewer
    exit: next
```

### 5.4 Engine Changes

In `PipelineEngine::submit_handoff()`, replace the linear match on `ExitRouting` with a recursive evaluator:

```rust
fn resolve_exit(&self, exit: &ExitRouting, handoff: &HandoffDocument) -> ResolvedRoute {
    match exit {
        ExitRouting::Next => ResolvedRoute::StepIndex(self.current_step + 1),
        ExitRouting::Branch { on, arms, default } => { ... }
        ExitRouting::Loop { target_step_id, max_iterations } => { ... }
        ExitRouting::Condition { condition, true_branch, false_branch } => {
            let result = evaluate_condition(condition, handoff, &self.step_history, &self.budget_tracker);
            if result {
                self.resolve_exit(true_branch, handoff)
            } else {
                self.resolve_exit(false_branch, handoff)
            }
        }
    }
}
```

This evaluation is:
- **Deterministic**: no LLM involved.
- **Cheap**: operates on in-memory structs only.
- **Observable**: log every condition result for debugging.

### 5.5 Deprecating Hard-Coded Escalation

The current hard-coded rule in `submit_handoff()`:

> "Coder gets 2 self-retries (3 total attempts), then escalation to design"

becomes expressible as flow data:

```yaml
exit:
  type: condition
  condition:
    type: validator_failed
  true_branch:
    type: condition
    condition:
      type: handoff_field_equals
      content: { field: retry_count, value: "3" }   # or a new counter expression
    true_branch:
      type: branch
      on: escalation
      arms:
        design: design
      default: failed
    false_branch:
      type: loop
      target_step_id: code
      max_iterations: 3
  false_branch: next
```

This is **aspirational** for Phase 2; the hard-coded rule can be kept as a fallback until Phase 3.

---

## 6. Data Model Changes Summary

### 6.1 Additions to `backend/src/relay/flow.rs`

```rust
// New variant on ExitRouting
ExitRouting::Condition { ... }

// New enum
pub enum RoutingCondition { ... }

// New struct
pub struct ValidationIssue {
    pub severity: ValidationSeverity,  // Error | Warning
    pub message: String,
    pub step_id: Option<String>,
}

pub enum ValidationSeverity { Error, Warning }
```

### 6.2 Additions to `FlowSpec`

```rust
impl FlowSpec {
    pub fn validate(
        &self,
        professions: &ProfessionRegistry,
        tools: &ToolRegistry,
    ) -> Vec<ValidationIssue>;
}
```

### 6.3 Additions to `FlowRegistry`

```rust
impl FlowRegistry {
    pub fn reload(&mut self, data_dir: &Path);
    pub fn insert(&mut self, flow: FlowSpec) -> Result<(), Vec<ValidationIssue>>;
    pub fn remove(&mut self, flow_id: &str) -> Option<FlowSpec>;
    pub fn list(&self) -> Vec<FlowSummary>;
    pub fn source(&self, flow_id: &str) -> Option<FlowSource>;
}
```

---

## 7. API Schema

### 7.1 List Flows

**Request**
```http
GET /api/forge/relay/flows
```

**Response `200 OK`**
```json
{
  "flows": [
    { "id": "standard-spec-driven-development", "source": "builtin", "step_count": 9 },
    { "id": "fast-track", "source": "builtin", "step_count": 2 },
    { "id": "my-team-onboarding", "source": "user", "step_count": 5 }
  ]
}
```

### 7.2 Get Flow

**Request**
```http
GET /api/forge/relay/flows/standard-spec-driven-development
```

**Response `200 OK`**
```json
{
  "id": "standard-spec-driven-development",
  "steps": [ ... ]
}
```

### 7.3 Create Flow

**Request**
```http
POST /api/forge/relay/flows
Content-Type: application/json

{
  "flow_id": "my-team-onboarding",
  "spec": {
    "id": "my-team-onboarding",
    "steps": [ ... ]
  }
}
```

**Response `201 Created`** on success.  
**Response `422 Unprocessable Entity`** on validation failure:
```json
{
  "valid": false,
  "errors": [
    { "severity": "error", "message": "Unknown profession_id 'super-coder-2'", "step_id": "implement" }
  ]
}
```

### 7.4 Validate Flow

**Request**
```http
POST /api/forge/relay/flows/validate
Content-Type: application/json

{ "spec": { "id": "x", "steps": [ ... ] } }
```

**Response**
```json
{
  "valid": true,
  "errors": []
}
```

### 7.5 Reload Flows

**Request**
```http
POST /api/forge/relay/flows/reload
```

**Response `204 No Content`**.

---

## 8. File Structure Changes

```
backend/src/relay/
  flow.rs                 # add ExitRouting::Condition + RoutingCondition + ValidationIssue
  flows/
    mod.rs                # FlowRegistry, reload, CRUD, validation orchestration
    builtin/
      standard-spec.yml
      fast-track.yml
      auto-discovery.yml
      post-discovery.yml
      bug-fix.yml
      goal-discovery.yml
      doc-patch.yml
      spec-tweak.yml
      superpower.yml
    tests.rs              # migrated tests (deserializes YAML)
  pipeline.rs             # add resolve_exit() + condition evaluator
  api.rs                  # add 5 new endpoints + request/response DTOs
```

Delete: `backend/src/relay/flows.rs` (split into `flows/mod.rs` + YAML files).

---

## 9. Implementation Milestones

### Milestone 1: YAML Migration (1–2 days)
- [ ] Convert 9 built-in flows to YAML.
- [ ] Rewrite `FlowRegistry::load_builtin` to use `include_str!`.
- [ ] Move `flows.rs` → `flows/mod.rs` + `flows/tests.rs`.
- [ ] Ensure existing `cargo test` still passes.

### Milestone 2: Validation Layer (1 day)
- [ ] Add `FlowSpec::validate()`.
- [ ] Integrate validation into registry load.
- [ ] Log warnings/errors at startup; fail hard only on errors.

### Milestone 3: CRUD API (1–2 days)
- [ ] Add `ListFlowResponse`, `FlowSummary`, `CreateFlowRequest`, etc.
- [ ] Implement 5 new endpoints in `api.rs`.
- [ ] Add RBAC: only project admins can create/update/delete/reload.

### Milestone 4: Conditional Routing (2–3 days)
- [ ] Add `ExitRouting::Condition` and `RoutingCondition`.
- [ ] Implement `evaluate_condition()`.
- [ ] Rewrite `submit_handoff()` routing block to use recursive resolver.
- [ ] Add tests for complex nested conditions.
- [ ] Update `bug-fix.yml` to use `Condition` as the reference example.

### Milestone 5: Frontend Support (optional, 2–3 days)
- [ ] Flow list view.
- [ ] Flow editor (YAML/JSON) with validation button.
- [ ] Clone built-in flow → user flow.

---

## 10. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| YAML serialization drift from Rust structs | Medium | High | Add round-trip tests for every built-in flow |
| Validation too strict breaks existing user flows | Medium | Medium | Warnings do not block; errors can be gated by a compatibility flag |
| Recursive `Condition` allows deeply nested routing | Low | Medium | Enforce max depth (e.g. 16) in validator |
| `HandoffFieldEquals` on missing field | Medium | Low | Missing field evaluates to `false`; logged as debug |
| RBAC misconfiguration on flow mutations | Low | High | Reuse existing admin middleware; write tests |

---

## 11. Future Work (Out of Scope for This Doc)

- **Parallel steps / Fork-Join**: `ExitRouting::Fork { steps: Vec<String>, join: String }`.
- **Sub-flows**: A step whose `profession_id` is a nested `FlowSpec` invocation.
- **Runtime flow mutation**: Allow an agent or user to insert/remove/replace steps while a run is in progress.
- **LLM-generated flows**: An `orchestrator` profession that composes a flow on the fly based on task classification.

---

## 12. Open Questions

1. Should user flows be stored project-local (`.autoforge/flows/`) or in the backend database/project store for multi-user safety?
2. Do we want a file-system watcher for hot-reload in dev, or is explicit `POST /reload` sufficient?
3. Should `RoutingCondition` support expressions in a mini-DSL (e.g. `handoff.token_usage.total > 50000`) instead of the nested enum style proposed here?
4. Should conditional routing support "step retries with exponential back-off" natively, or leave that to `Loop`?
