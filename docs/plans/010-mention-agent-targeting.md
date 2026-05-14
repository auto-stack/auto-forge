# Plan: @mention Agent Targeting in Chat

## Context

The chat currently sends all messages to a hardcoded generic "AutoForge" system prompt. The project has a rich relay/agent system (8 professions, soul configs, agent configs with model tiers) that is **not wired to the chat**. The goal: type `@coder` to talk to the coder agent, `@advisor` to talk to the advisor, etc. Without `@`, messages go to the last-used profession (defaulting to "assistant").

## Design Decisions

- **Frontend parses `@mention`**, sends `profession_id` as a separate API field (not inline in content)
- **Session remembers** `active_profession` across messages — no need to re-type `@coder` every time
- **Backend resolves** the profession to a full agent (soul + profession system prompt) via `RelayRegistry`
- **Autocomplete dropdown** anchored to bottom of textarea (not cursor-relative — simpler and sufficient)

---

## Step 1: Backend — schema changes

Files: `backend/src/forge/mod.rs`

### 1a. Add `active_profession` to `ForgeSession` (line 32-44)
```rust
#[serde(default)]
pub active_profession: Option<String>,
```

### 1b. Add `profession_id` to `ForgeMessage` (line ~289)
```rust
#[serde(default)]
#[serde(skip_serializing_if = "Option::is_none")]
pub profession_id: Option<String>,
```

### 1c. Add `profession_id` to `SendMessageRequest` (line 455)
```rust
#[serde(default)]
pub profession_id: Option<String>,
```

## Step 2: Backend — `send_forge_message` handler (line 1542-1574)

Resolve effective profession:
1. `req.profession_id` if provided
2. `session.active_profession` if set
3. Fall back to `"assistant"`

Update `session.active_profession` to the resolved value. Store it on the user message.

## Step 3: Backend — `forge_stream` handler (line 1576-1841)

Replace the hardcoded `build_system_prompt(&focus_section)` (line 1657) with:

1. Read `session.active_profession` (already loaded at line 1591)
2. Create `RelayRegistry::new()` (same pattern as `list_professions()` in relay/api.rs:113)
3. Call `registry.default_agent_for(&profession_id)` → `AgentConfig`
4. Call `registry.spawn_agent_from_config(&config)` → `AgentInstance`
5. Call `agent.render_system_prompt()` → rich system prompt with soul + profession
6. On any failure, fall back to existing `build_system_prompt()`

## Step 4: Frontend — type updates

File: `frontend/src/types/forge.ts`
- Add `profession_id?: string` to `ForgeMessage`
- Add `active_profession?: string` to `ForgeSession`

## Step 5: Frontend — parse @mention in sendMessage

File: `frontend/src/composables/useForge.ts` (line 135-161)

Change signature to `sendMessage(content: string, professionId?: string)` and include `profession_id` in the POST body.

File: `frontend/src/views/ChatsView.vue` (line 368-373)

Parse `^@(\w+)\s*` from input:
- Extract professionId, strip from content
- Pass both to `forgeSendMessage(content, professionId)`

## Step 6: Frontend — autocomplete dropdown

### 6a. Create `MentionDropdown.vue` component
- Props: `professions`, `visible`, `filter`
- Emits: `select(professionId)`
- Floating list with keyboard nav (up/down/enter/escape)

### 6b. Integrate into ChatsView.vue
- Load professions via `useAgentConfigs` (already imported)
- Detect `@` at start of input → show dropdown
- Filter as user types after `@`
- On select: set a local `targetProfession` ref, show a chip badge next to input, close dropdown

### 6c. Active profession indicator
- Show `@Coder` badge in `.input-extras` bar when a profession is targeted
- Reads from session response after each message

## Step 7: Frontend — agent identity in messages

In the message list, show profession name (e.g. "Coder") instead of "assistant" when `msg.profession_id` is set. Resolve name from loaded configs.

---

## Verification

1. Start backend + frontend (`cargo run` + `pnpm run dev`)
2. Open chat, type `@coder write a hello world function` — verify:
   - Autocomplete dropdown appears after typing `@`
   - Message sends, response uses coder soul/profession system prompt
3. Type another message without `@` — verify it still targets coder (session remembers)
4. Type `@assistant hello` — verify it switches back to generic assistant
5. Check that existing sessions without `active_profession` still work (backward compat)
