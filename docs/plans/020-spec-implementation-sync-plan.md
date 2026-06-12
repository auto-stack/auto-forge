# Spec-Implementation Synchronization Plan

> **Date:** 2026-06-12
> **Scope:** `specs/auto-forge/` ↔ `backend/src/` + `frontend/src/`
> **Goal:** Bring the spec ledger back into truth with the actual codebase, and surface features that are spec'd but not built.

---

## 1. Executive Summary

The AutoForge codebase is substantially ahead of its own specs ledger. Many top-level specs still carry **draft/proposed** headings while their bodies (and the code) say **implemented ✅**. Conversely, several specs claim features are complete that are only partial, stubbed, or missing entirely. A few modules (`auth`, `i18n`) are essentially empty in specs but have real code.

This plan proposes a phased reconciliation:

1. **Status scrub** — align heading statuses with code reality.
2. **Spec-fill for implemented modules** — add missing architecture/design/test entries for code that exists.
3. **Honest downgrade** — mark partial/stubbed features as `in_progress` or `draft`.
4. **Implementation backlog** — keep genuinely missing features as `proposed` with clear P0/P1 labels.
5. **Structural cleanup** — remove duplicates, complete stub modules, archive stale files.

---

## 2. Methodology

We scanned:

- `specs/auto-forge/**/*.ad` — top-level and per-module specs (~120 files)
- `backend/src/**/*.rs` — Rust backend
- `frontend/src/**/*.{ts,vue}` — Vue 3 frontend
- Design docs in `docs/design/`

For each spec item we checked whether the described behavior has a concrete implementation: a struct, an endpoint, a component, or a composable. We did **not** evaluate quality or test coverage, only existence and wiring.

---

## 3. Findings

### 3.1 Specs that understate implementation (status should be upgraded)

| Spec ID | Current Heading Status | Code Evidence | Recommended Status |
|---------|------------------------|---------------|--------------------|
| `A1` Relay Orchestrator Architecture | `draft` | `relay/pipeline.rs`, `relay/driver.rs`, 9 built-in flows | `implemented` |
| `A2` Agent Instance Lifecycle | `draft` | `relay/agent.rs`, `relay/soul.rs`, `relay/profession.rs` | `implemented` |
| `A3` Checkpoint & State Persistence | `draft` | `relay/checkpoint.rs`, `PipelineEngine::from_checkpoint` | `implemented` |
| `A5` Frontend Composable State Architecture | `implemented` (already correct) | `useForge.ts`, `useRelay.ts`, `useGateInbox.ts` | keep |
| `A6` Chat Ephemeral Chrome Architecture | `implemented` | `SecretaryMessage.vue`, `GateCard.vue` | keep |
| `D1` AgentInstance & Soul Rendering | `implemented` | `relay/agent.rs` | keep |
| `D2` PipelineEngine & Flow Execution | `implemented` | `relay/pipeline.rs` | keep |
| `D3` HandoffDocument Generation | `implemented` | `relay/handoff.rs` | keep |
| `G1`–`G7` core relay/runtime goals | mostly `implemented` | matches backend | keep |
| `G8`–`G9` chat/specs goals | mostly `implemented` | matches frontend | keep |

**Pattern:** `architecture.ad` has a systematic drift where the **heading** says `draft` but the **body** says `Status: implemented ✅` with a verification date. This should be fixed in bulk.

### 3.2 Specs that overstate implementation (should be downgraded or qualified)

| Spec ID | Claim | Reality | Recommended Action |
|---------|-------|---------|--------------------|
| `G10.1` Node-graph with animated handoff lines | `implemented` | Horizontal step list + chevron connectors exist; no traveling-dot animation, no backward retry edge visualization | Change to `in_progress`; split into "basic step list" (done) and "animated node graph" (pending) |
| `G10.2` Expanded node cards — Output tab specs touched | `implemented` | Live log + token bar exist; "Specs Touched" panel and per-step cost breakdown are missing | Change to `in_progress` |
| `G10.3` Tester→Coder retry loop backward edge | `implemented` | Loop exists in backend; UI does not render a backward arrow or "Retry N/3" badge | Change to `in_progress` |
| `G10.4` Cost Breakdown panel | `implemented` | Segmented progress bar exists; per-profession cost table and "savings vs parallel" are partial | Change to `in_progress` |
| `G11.1` 3-zone responsive layout | `implemented` | Sidebar + main exist; Zone-C context drawer is not persistent | Change to `in_progress` or split |
| `G11.2` Ctrl+K jump-to-ID | `implemented` | Shortcut is bound to `preventDefault()` only; no dialog implemented | Change to `draft` |
| `G11.4` Screen reader annotations for node graph | `implemented` | Step list has basic labels; full aria live-region coverage for relations unverified | Verify or downgrade |
| `G5` Token Budget strategies | `implemented` | `BudgetStrategy` enum has 4 variants; only `HardStop` and `Warning` are wired | Split: tracking done, strategies partial |
| `D1` `ModelConfig.fallback_chain` | declared | Field exists; fallback logic not implemented in `provider/claude.rs` | Add note in design; downgrade related goals |
| `G17` / `Chat-D3` `bring_in` chat-to-agent handoff | mixed | `bring_in` switches profession in chat; true baton-style handoff with HandoffDocument into Relay is not wired | Clarify scope; keep `proposed` for full handoff |
| `G13` API Source Configuration (top-level) | `proposed` | Module `ApiSources-G1` says `implemented`; UI + backend CRUD exist | Upgrade top-level G13 to `implemented` |

### 3.3 Genuinely missing features (in specs, not in code)

| Feature | Spec IDs | Implementation State | Recommendation |
|---------|----------|----------------------|----------------|
| Multi-provider auto-dispatch / fallback | `A7`, `D19`, `D20`, `Provider-G4-G7` | 5-tier `ModelTier` exists; source selection and health-based routing not wired | Keep as `proposed` or `in_progress`; add design task |
| Wiki semantic / RAG search | `Wiki-G7-G8`, `Wiki-D4` | Storage/upload exist; no embedding/search | Keep `proposed` |
| AutoDown WYSIWYG / spec-aware templates | `AutoDown-G3-G8`, `Editor-*` | TipTap editor exists; spec-aware templates and block pipeline are draft | Keep `draft` |
| CLI interactive REPL | `Cli-*` | No CLI binary found; only backend server | Verify or archive |
| Full checkpoint file restore on resume | `G4`, `A3` | Checkpoints serialize file manifest; `drive_run`/`resume_running_runs` does not call `restore_files` | Add as `in_progress` task |
| RBAC permission enforcement on forge/relay routes | `Auth-A1`, `Auth-D1` | Auth middleware exists; `require_permission` not applied to forge/relay/config routes | Add as `in_progress` task |
| Context manager / permission policy integration | `Runtime-G5-G7` | Code exists in `runtime/context.rs`, `runtime/permission.rs`; not wired into chat/tool path | Add as `in_progress` task |
| `Cache` module usage | none (not spec'd) | `cache.rs` exists but unused | Either spec it or remove |

### 3.4 Structural spec issues

| Issue | Evidence | Recommended Fix |
|-------|----------|-----------------|
| Duplicate goals | `Chat-G10` appears twice with different statuses | Merge into one item |
| Overlapping cleanup goals | `Specs-G8` and `Specs-G9` both describe spec cleanup | Merge or clarify scope |
| Empty stub modules | `auth/goals.ad` empty; `i18n/*.ad` empty; no `auth/module.ad` or `i18n/module.ad` | Fill or remove; do not leave empty placeholders |
| Stale backend spec tree | `backend/auto-forge/*.ad` are minimal stubs | Delete or move to `tests/`; they are not authoritative |
| Consolidated vs module ID drift | Top-level `G13` is `proposed`, module `ApiSources-G1` is `implemented` | Reconcile during status scrub |
| `flows.ad` empty | Top-level `flows.ad` has no typed flow entries | Add overview of built-in flows from `relay/flows/builtin/*.yml` |
| Missing `Relates to` cross-links | Module IDs (`Relay-D3`) not linked to consolidated IDs (`D24`) | Add optional `Relates to:` field during cleanup |

---

## 4. Synchronization Plan

### Phase 0 — Pre-flight cleanup (1–2 hours)

1. Delete or archive `backend/auto-forge/*.ad` stubs.
2. Decide fate of empty `auth/` and `i18n/` modules:
   - **Option A:** Fill them to match existing code.
   - **Option B:** Remove empty files and add a note in `overview.ad`.
3. Run `cargo test` and `pnpm test` to establish a baseline.

### Phase 1 — Status scrub (4–6 hours)

1. Bulk-edit `specs/auto-forge/architecture.ad` headings to match body `Status:` lines.
2. Walk `goals.ad`, `designs.ad`, `plans.ad`, `tests.ad`, `reviews.ad`, `reports.ad` and align heading statuses with code.
3. Apply downgrade recommendations from §3.2.
4. Apply upgrade recommendations from §3.1 and §3.3.

### Phase 2 — Fill gaps for implemented code (6–10 hours)

1. Add missing architecture/design entries for:
   - Relay API surface (`relay/api.rs`)
   - MCP server (`mcp/mod.rs`)
   - Wiki/raw file endpoints (`forge/wiki.rs`)
   - Errand runner (`forge/errand.rs`)
   - RBAC (`rbac/`)
2. Add tests.ad entries that reference actual test files:
   - `backend/tests/relay_integration.rs`
   - `backend/tests/relay_write_goals_test.rs`
   - frontend component/composable tests
3. Update `overview.ad` module index to reflect real views (RelayView, AgentsConfigView, ProfessionsView, ApiSourcesView, SkillsView, WikiView).

### Phase 3 — Honest backlog (2–4 hours)

1. For each genuinely missing feature in §3.3, ensure the spec:
   - Has a clear, single-sentence goal.
   - Links to the relevant module.
   - Is marked `proposed` or `in_progress`.
2. Add a `P0` tag to multi-provider dispatch and `bring_in` full handoff if they are strategic priorities.

### Phase 4 — Structural consistency (2–3 hours)

1. Merge duplicate/overlapping goals.
2. Add `Relates to:` cross-links between module IDs and consolidated IDs.
3. Populate `flows.ad` with the 9 built-in flows.
4. Add a "Spec Health" review entry documenting this sync.

### Phase 5 — Validation (2–3 hours)

1. Run spec parsers/scripts (`scripts/regenerate_specs.py`, `scripts/generate_module_overviews.py`) to ensure no broken IDs.
2. Verify all `depends_on` references resolve.
3. Do a final read-through of `overview.ad` and top-level files.

---

## 5. Deliverables

- Updated `specs/auto-forge/goals.ad` with honest statuses.
- Updated `specs/auto-forge/architecture.ad` with heading/body consistency.
- Updated `specs/auto-forge/designs.ad` with partial features qualified.
- Filled or removed empty `auth/` and `i18n/` stubs.
- Deleted `backend/auto-forge/*.ad` or moved them out of the spec path.
- Updated `overview.ad` module index and `flows.ad`.
- A new report entry documenting the sync and remaining gaps.

---

## 6. Open Questions

1. Should empty `auth/` and `i18n/` modules be filled now, or removed and re-spec'd later?
2. Is `Cli-*` still a planned feature, or should it be archived?
3. Should `cache.rs` be spec'd as a runtime component or removed as dead code?
4. What is the desired priority order for genuinely missing features (multi-provider dispatch vs `bring_in` handoff vs semantic wiki search)?

---

## 7. Risk

- Over-downgrading may hide real progress from users reading the specs.
- Over-upgrading may create false expectations.
- The safest rule is: **if the code has a working endpoint/component that a user can trigger from the UI, mark it implemented; if it has a type/field but no wiring, mark it in_progress.**
