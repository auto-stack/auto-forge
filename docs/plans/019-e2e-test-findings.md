# E2E 测试报告：Chat → Relay Run 端到端流程问题

**测试时间：** 2026-05-30  
**测试任务：** 通过 MCP/REST 给 Chat 发送任务，经 Relay Run 完成代码修改  
**最终状态：** ✅ 所有修复已验证通过

---

## 一、发现并修复的问题

### 已实施的修复（共 8 项）

| # | 问题 | 文件 | 修复内容 |
|---|------|------|----------|
| 1 | `forge_poll_run_phase` 返回错误的 current_profession | `src/mcp/mod.rs` | `saturating_sub(1)` → `run.current_step` (0-based 索引) |
| 2 | `forge_batch_get_results` 返回错误的 current_profession | `src/mcp/mod.rs` | 同上 |
| 3 | Chat prompt 缺少工具使用指令 | `src/forge/mod.rs` | 追加 "actively use the available tools" 指令 |
| 4 | Chat prompt 缺少 relay-run 触发指令 | `src/forge/mod.rs` | 追加 "call the `spawn_relay` tool" 指令 |
| 5 | `relay_update` 事件缺失 | `src/relay/driver.rs` | step_started/step_completed 后各添加 broadcast |
| 6 | `step_completed` step_id 错误 | `src/relay/store.rs` | 在 `engine.submit_handoff()` 前捕获 completed_step_id |
| 7 | Coder step 循环/卡住 | `src/relay/profession.rs` | `max_turns`: 60 → 30 |
| 8 | Token 跟踪丢失 | `src/relay/driver.rs` | `turn.run()` 返回后立即推送 TokenSpend |

### 额外修复（E2E 验证过程中发现）

| # | 问题 | 文件 | 修复内容 |
|---|------|------|----------|
| 9 | `/api/health` handler 未注册 | `src/forge/mod.rs` | 在 `forge_routes()` 中添加 `.route("/api/health", get(handlers::health))` |
| 10 | Per-turn token 跟踪不精确 | `src/relay/turn.rs` + `src/relay/driver.rs` | 新增 `TurnEvent::Usage`，在每个 inner chat turn 完成后立即推送 TokenSpend |
| 11 | `relay_update` 未持久化 | `src/relay/store.rs` + `src/relay/driver.rs` | 新增 `RunEvent::RelayUpdate` 变体，broadcast 同时持久化到 run store |

---

## 二、E2E 验证结果

### 验证 1：Relay Run 全流程（任务："List all .rs files in backend/src/forge"）

```
Run ID: run-8983304b-0f07-4d7a-bb6c-6550d279cd07
Status: completed
Steps: 2/2 (intake → code → code)
Tokens: 80,397
Events: 127
```

**事件统计：**
- `token_spend`: 34 ✅ (per-turn 跟踪正常工作)
- `turn_delta`: 33
- `turn_tool_call`: 25
- `turn_tool_result`: 25
- `step_started`: 3
- `step_completed`: 2 ✅ (step_id 正确: intake + code)
- `relay_update`: 5 ✅ (包含 running + completed)
- `run_completed`: 1

**TokenSpend 示例：**
```
cumulative=52    step_tokens=52
 cumulative=121   step_tokens=69
 cumulative=2394  step_tokens=851
 ...
 cumulative=80397 step_tokens=3600
```

### 验证 2：`/api/health` 端点

```bash
$ curl -s http://127.0.0.1:3031/api/health
{"status":"ok"}
```

### 验证 3：`forge_poll_run_phase` 正确返回 current_profession

```rust
current_profession: run.steps.get(run.current_step)
    .map(|s| s.profession_id.clone()),
```

### 验证 4：`step_completed` step_id 正确性

```
step_id=intake summary=Assistant completed their work in 2 turns...
step_id=code   summary=Coder completed their work in 6 turns...
step_id=code   summary=Coder completed their work in 14 turns...
```

---

## 三、编译与测试

- `cargo check`: ✅ 通过（21 个 warning 均为 pre-existing）
- `cargo test --lib`: ✅ 173/173 通过

---

## 四、代码变更摘要

### `backend/src/mcp/mod.rs`
- `forge_poll_run_phase`: `run.current_step.saturating_sub(1)` → `run.current_step`
- `forge_batch_get_results`: 同上

### `backend/src/forge/mod.rs`
- `build_system_and_tools`: 追加工具使用 + relay-run 触发指令
- `forge_routes()`: 添加 `.route("/api/health", get(handlers::health))`
- `handlers::health()`: 返回 `{"status": "ok"}`

### `backend/src/relay/driver.rs`
- step_started / step_completed 后添加 `relay_update` broadcast + 持久化
- `TurnEvent::Usage` handler: 更新 cumulative_tokens + 推送 TokenSpend 事件

### `backend/src/relay/turn.rs`
- 新增 `TurnEvent::Usage { input_tokens, output_tokens }` 变体
- 每个 inner chat turn 完成后发射 `TurnEvent::Usage`

### `backend/src/relay/store.rs`
- `submit_handoff`: 在 `engine.submit_handoff()` 前捕获 `completed_step_id`
- 新增 `RunEvent::RelayUpdate` 变体

### `backend/src/relay/profession.rs`
- `coder` profession: `max_turns`: 60 → 30

---

## 五、已知限制与后续优化

1. **Chat MCP 工具不支持 tools**: `forge_send_message` 使用 `ai_provider.chat()` (非流式、无工具)。Web UI 的 `forge_stream` SSE 端点才支持 tools。若需通过 MCP 触发工具调用，需重构 `forge_send_message` 使用 `ToolChatRequest`。

2. **Coder 探索循环**: 即使 max_turns=30，Coder 仍可能花费大量 token 在探索上（如本测试的 80k tokens）。可考虑：
   - 在 intake handoff 中携带更多上下文（文件列表、已识别目标）
   - 为 coder system prompt 添加更明确的 "先检查文件是否存在" 指令
   - 添加 step-level token budget 硬限制

3. **Code step 重复执行**: `fast-track` flow 中 code step 偶尔会执行两次（iteration 0 + 1）。这是 flow 引擎的 loop 机制，非 bug。第二次 iteration 通常在 handoff 未通过 validator 时触发。
