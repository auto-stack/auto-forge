# AutoForge 全功能 MCP Server 实现计划

> 日期：2026-05-30
> 状态：计划阶段
> 关联代码：`backend/src/mcp/`, `backend/src/main.rs`

---

## 一、背景与目标

AutoForge 已具备一套完整的 REST API（~40+ 端点），覆盖 Forge Chat、Relay Pipeline、Specs 管理、Config 配置、Wiki/Raw 文件等全部功能。但这些 API 是**人类前端导向**的，AI Agent 无法自动发现、理解和调用。

**目标**：在现有 REST API 之上新建一层 **MCP（Model Context Protocol）Server**，使任何支持 MCP 的 AI Agent（Claude Desktop、Kimi CLI、Cursor、Codex CLI 等）能够：

1. **自动发现** AutoForge 的全部能力（Tools、Resources、Prompts）
2. **程序化操控** — 创建对话、启动 Relay Run、读取 Specs、检查日志
3. **自动化测试与监控** — 批量创建 runs、分析 timing 日志、对比性能
4. **双向协作** — AI Agent 作为"AutoForge 的操作员"，AutoForge 作为"AI Agent 的工具平台"

---

## 二、技术选型

### 2.1 SDK 选择：官方 `rmcp`

| 选项 | 说明 | 选择 |
|------|------|------|
| `rmcp` (官方) | Anthropic 官方 Rust SDK，`#[tool]`/`#[tool_router]` 宏，tokio 异步，活跃维护 | ✅ 首选 |
| `mcpkit` (第三方) | `#[mcp_server]` 宏，支持 2025-11-25 协议，较新但生态小 | ❌ 暂不采用 |
| `rust-mcp-core` | 配置驱动，YAML/JSON 定义 tools，适合快速原型 | ❌ 不适合深度定制 |

**选择 `rmcp` 的理由**：
- 官方维护，协议更新最快
- 与 AutoForge 的 tokio 运行时天然兼容
- `#[tool]` 宏可以零样板地暴露 Rust 函数为 MCP Tools
- 支持 stdio 和 HTTP (SSE) 两种 transport

### 2.2 集成方式：直接模块集成（非 HTTP 代理）

调研结论：AutoForge **没有内部 HTTP client**，所有组件通过直接函数调用和全局 statics 通信。

```
┌─────────────────────────────────────────────────────────────┐
│                     MCP Client                              │
│         (Claude Desktop / Kimi CLI / Cursor)                │
└──────────────────────┬──────────────────────────────────────┘
                       │ MCP Protocol (stdio or HTTP/SSE)
┌──────────────────────▼──────────────────────────────────────┐
│              AutoForge MCP Server Module                    │
│  ┌─────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ MCP Tools   │  │ MCP Resources   │  │ MCP Prompts    │  │
│  │ (#[tool])   │  │ (URI scheme)    │  │ (templates)    │  │
│  └──────┬──────┘  └────────┬────────┘  └───────┬────────┘  │
│         │                  │                   │           │
│         └──────────────────┼───────────────────┘           │
│                            │ 直接函数调用                   │
├────────────────────────────┼───────────────────────────────┤
│     现有 AutoForge 业务模块 │ (relay/store, relay/driver,   │
│     (无需 HTTP 中间层)      │  relay/turn, forge/mod, ...)  │
└────────────────────────────┴───────────────────────────────┘
```

**优势**：
- 零序列化开销（不走 HTTP）
- 直接使用全局状态（RUN_STORE、EVENT_TX、PROFESSIONS 等）
- 与现有业务逻辑保持强类型安全
- 无需维护两套 API 的兼容性

---

## 三、架构设计

### 3.1 模块结构

新建 `backend/src/mcp/` 目录：

```
backend/src/mcp/
├── mod.rs              # 模块入口，McpServer 结构体定义
├── tools/              # MCP Tools 实现
│   ├── mod.rs          # Tools 路由注册
│   ├── chat.rs         # Forge Chat 相关 tools
│   ├── relay.rs        # Relay Run 相关 tools
│   ├── specs.rs        # Specs 管理 tools
│   ├── config.rs       # Config 管理 tools
│   └── system.rs       # 系统级 tools (logs, status)
├── resources/          # MCP Resources 实现
│   ├── mod.rs
│   ├── specs.rs        # forge://specs/{project}
│   ├── runs.rs         # forge://runs/{run_id}
│   ├── chats.rs        # forge://chats/{sid}
│   └── logs.rs         # forge://logs/{kind}
├── prompts/            # MCP Prompts 实现
│   ├── mod.rs
│   ├── analysis.rs     # 性能分析 prompt
│   ├── review.rs       # Specs 审查 prompt
│   └── batch_test.rs   # 批量测试 prompt
└── transport.rs        # Transport 配置 (stdio / HTTP)
```

### 3.2 核心结构

```rust
// backend/src/mcp/mod.rs
use rmcp::{ServerHandler, model::*};
use crate::provider::ClaudeProviderState;

pub struct AutoForgeMcpServer {
    /// AI provider (shared with main app)
    pub ai_provider: ClaudeProviderState,
    /// Project path for context
    pub project_path: Option<String>,
}

#[tool_router]
impl AutoForgeMcpServer {
    pub fn new(ai_provider: ClaudeProviderState) -> Self {
        Self { ai_provider, project_path: None }
    }
}

#[tool_handler]
impl ServerHandler for AutoForgeMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "autoforge".into(),
            instructions: Some(
                "AutoForge MCP Server: 管理 AI Agent 协作 pipeline、对话、Specs 和配置".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            ..Default::default()
        }
    }
}
```

### 3.3 Transport 策略

| Transport | 适用场景 | 启动方式 |
|-----------|---------|---------|
| **stdio** | 本地 AI CLI（Claude Code、Kimi CLI） | `autoforge --mcp` 子命令 |
| **HTTP (SSE)** | 远程/多客户端、Web UI | `/api/mcp` 端点挂载到现有 Axum server |

**默认行为**：
- 启动后端服务时，**自动挂载** `/api/mcp` HTTP transport
- 同时支持 `--mcp-stdio` 命令行参数启动独立 stdio 模式

---

## 四、全功能映射清单

### 4.1 Tools（AI 可调用的操作）

#### Tier 1 — 核心工作流（Phase 1 实现）

| # | Tool Name | 功能 | 映射的 REST/内部 API |
|---|-----------|------|---------------------|
| 1 | `forge_create_session` | 创建 Forge 对话会话 | `create_forge_session()` |
| 2 | `forge_send_message` | 向会话发送消息 | `send_forge_message()` |
| 3 | `forge_get_session` | 获取会话详情 | `get_forge_session()` |
| 4 | `forge_list_sessions` | 列出所有会话 | `list_forge_sessions()` |
| 5 | `forge_start_relay_run` | 启动 Relay Run | `start_run(&RUN_STORE, ...)` |
| 6 | `forge_get_run` | 获取 Run 状态 | `get_run(&RUN_STORE, ...)` |
| 7 | `forge_list_runs` | 列出所有 Runs | `list_runs(&RUN_STORE)` |
| 8 | `forge_advance_run` | 推进 Run 到下一步 | `advance_run(&RUN_STORE, ...)` |
| 9 | `forge_resolve_gate` | 解析 Gate（通过/拒绝） | `resolve_gate(&RUN_STORE, ...)` |
| 10 | `forge_submit_handoff` | 提交 Handoff | `submit_handoff(&RUN_STORE, ...)` |
| 11 | `forge_read_specs` | 读取 Specs 内容 | `get_specs()` / `get_specs_section()` |
| 12 | `forge_update_specs` | 更新 Specs 段落 | `update_specs_section()` |
| 13 | `forge_list_professions` | 列出可用 Professions | `list_professions()` |
| 14 | `forge_get_project_status` | 获取当前项目状态 | `get_project_status()` |

#### Tier 2 — 深度操作（Phase 2 实现）

| # | Tool Name | 功能 | 映射的 REST/内部 API |
|---|-----------|------|---------------------|
| 15 | `forge_delete_session` | 删除会话 | `delete_forge_session()` |
| 16 | `forge_approve_spec` | 审批 Spec | `approve_spec()` |
| 17 | `forge_reject_spec` | 拒绝 Spec | `reject_spec()` |
| 18 | `forge_list_errands` | 列出会话中的 errands | `list_errands()` |
| 19 | `forge_get_errand` | 获取 errand 详情 | `get_errand()` |
| 20 | `forge_trigger_drift_check` | 触发 Specs drift 检查 | `trigger_drift_check()` |
| 21 | `forge_rebuild_relations` | 重建 Specs 关系 | `rebuild_relations_endpoint()` |
| 22 | `forge_list_api_sources` | 列出 API Sources | `list_api_sources()` |
| 23 | `forge_test_api_connection` | 测试 API 连接 | `test_api_connection()` |
| 24 | `forge_list_agent_configs` | 列出 Agent 配置 | `list_agent_configs()` |
| 25 | `forge_update_agent_config` | 更新 Agent 配置 | `update_agent_config()` |
| 26 | `forge_list_skills` | 列出 Skills | `list_skills()` |
| 27 | `forge_list_wiki_pages` | 列出 Wiki 页面 | `list_wiki_pages()` |
| 28 | `forge_read_wiki_page` | 读取 Wiki 页面 | `get_wiki_page()` |
| 29 | `forge_create_wiki_page` | 创建 Wiki 页面 | `create_wiki_page_api()` |
| 30 | `forge_read_file` | 读取项目文件 | `read_file()` |
| 31 | `forge_browse_directory` | 浏览目录 | `browse_directory()` |

#### Tier 3 — 系统与监控（Phase 3 实现）

| # | Tool Name | 功能 | 实现方式 |
|---|-----------|------|---------|
| 32 | `forge_get_performance_logs` | 获取最近 timing 日志 | 读取 tracing 日志文件或内存缓存 |
| 33 | `forge_get_run_events` | 获取 Run 的 SSE 事件历史 | 从 RunEntry.events 读取 |
| 34 | `forge_cancel_run` | 取消正在运行的 Run | 通过 channel 发送取消信号 |
| 35 | `forge_open_project` | 打开项目 | `open_project()` |
| 36 | `forge_close_project` | 关闭项目 | `close_project()` |

### 4.2 Resources（AI 可读取的数据）

| URI 模式 | 内容类型 | 说明 |
|---------|---------|------|
| `forge://specs/{project}` | `application/json` | 项目完整 Specs |
| `forge://specs/{project}/{section}` | `application/json` | 指定 section |
| `forge://runs` | `application/json` | Run 列表摘要 |
| `forge://runs/{run_id}` | `application/json` | Run 完整状态 |
| `forge://runs/{run_id}/events` | `application/json` | Run 事件历史 |
| `forge://chats` | `application/json` | 会话列表 |
| `forge://chats/{sid}` | `application/json` | 会话详情 |
| `forge://chats/{sid}/history` | `application/json` | 对话历史 |
| `forge://config/professions` | `application/json` | Profession 配置 |
| `forge://config/agents` | `application/json` | Agent 配置 |
| `forge://config/skills` | `application/json` | Skill 配置 |
| `forge://config/api-sources` | `application/json` | API Source 配置 |
| `forge://logs/performance` | `text/plain` | 最近性能日志 |
| `forge://project/status` | `application/json` | 当前项目状态 |

### 4.3 Prompts（预定义模板）

| Prompt Name | 参数 | 用途 |
|------------|------|------|
| `performance-analysis` | `run_id?: string`, `n: number = 5` | "分析最近 N 个 Relay Run 的 timing 日志，找出 Agent 启动、切换、LLM 推理各阶段瓶颈" |
| `spec-review` | `project: string` | "读取项目 specs，检查 goals、designs、architecture 之间的一致性，输出审查报告" |
| `batch-test` | `flow_id: string`, `n: number = 3` | "创建 N 个相同 flow 的 Relay Run，对比完成时间和输出质量" |
| `agent-config-audit` | — | "检查所有 profession 的 thinking_enabled、token_budget、model 配置，建议优化" |
| `conversation-seed` | `task: string`, `profession_id?: string` | "创建一个新 Forge 会话，以指定任务启动对话" |
| `run-debug` | `run_id: string` | "获取指定 Run 的完整状态和事件历史，诊断卡住或失败原因" |

---

## 五、分阶段实施计划

### Phase 1：MCP Server 脚手架 + 核心 Tools（3-4 天）

**目标**：可运行的 MCP Server，支持最核心的 Chat + Relay + Specs 操作。

| 任务 | 文件 | 说明 |
|------|------|------|
| 1.1 添加 rmcp 依赖 | `backend/Cargo.toml` | `rmcp = { version = "0.16", features = ["server", "macros"] }` |
| 1.2 创建 MCP 模块结构 | `backend/src/mcp/mod.rs`, `mcp/tools/mod.rs` | 模块入口、McpServer 结构体、`#[tool_router]` |
| 1.3 实现 Chat Tools | `mcp/tools/chat.rs` | `forge_create_session`, `forge_send_message`, `forge_get_session`, `forge_list_sessions` |
| 1.4 实现 Relay Tools | `mcp/tools/relay.rs` | `forge_start_relay_run`, `forge_get_run`, `forge_list_runs`, `forge_advance_run`, `forge_resolve_gate`, `forge_submit_handoff` |
| 1.5 实现 Specs Tools | `mcp/tools/specs.rs` | `forge_read_specs`, `forge_update_specs` |
| 1.6 实现 System Tools | `mcp/tools/system.rs` | `forge_get_project_status`, `forge_list_professions` |
| 1.7 挂载 HTTP Transport | `backend/src/main.rs` | 在现有 Axum router 上添加 `/api/mcp` endpoint |
| 1.8 实现 stdio Transport | `backend/src/main.rs` | 添加 `--mcp-stdio` CLI 参数 |
| 1.9 端到端验证 | Claude Desktop / Kimi CLI | 配置 MCP，验证工具发现和调用 |

**Phase 1 验收标准**：
- `npx @anthropics/mcp-inspector` 能列出所有 Tools
- Claude Desktop 配置后能调用 `forge_start_relay_run` 并收到正确响应
- 所有现有测试（191 个）继续通过

### Phase 2：Resources + Prompts + 深度 Tools（3-4 天）

**目标**：完整的 Resources 读取能力和 Prompts 模板。

| 任务 | 文件 | 说明 |
|------|------|------|
| 2.1 实现 Specs Resources | `mcp/resources/specs.rs` | `forge://specs/{project}`, `forge://specs/{project}/{section}` |
| 2.2 实现 Run Resources | `mcp/resources/runs.rs` | `forge://runs`, `forge://runs/{run_id}`, `forge://runs/{run_id}/events` |
| 2.3 实现 Chat Resources | `mcp/resources/chats.rs` | `forge://chats`, `forge://chats/{sid}`, `forge://chats/{sid}/history` |
| 2.4 实现 Config Resources | `mcp/resources/config.rs` | `forge://config/professions`, `forge://config/agents`, etc. |
| 2.5 实现 Performance Logs Resource | `mcp/resources/logs.rs` | `forge://logs/performance`（读取 backend.log） |
| 2.6 实现 Prompts | `mcp/prompts/*.rs` | `performance-analysis`, `spec-review`, `batch-test`, `agent-config-audit`, `conversation-seed`, `run-debug` |
| 2.7 实现 Tier 2 Tools | `mcp/tools/config.rs`, `mcp/tools/wiki.rs` | 配置管理、Wiki、Drift Check 等 |
| 2.8 实现 Tier 3 Tools | `mcp/tools/system.rs` | 性能日志读取、Run 取消 |

**Phase 2 验收标准**：
- MCP Inspector 能读取所有 Resources
- Claude 能执行 prompt "分析最近 3 个 Run 的性能" 并自动调用相关 tools
- Resources 支持订阅（当 Run 状态变化时推送更新）

### Phase 3：高级功能与优化（2-3 天）

| 任务 | 说明 |
|------|------|
| 3.1 SSE 事件流适配 | 将 Relay Run 的 SSE events 通过 MCP `notification` 推送给客户端 |
| 3.2 批量操作工具 | `forge_batch_start_runs`, `forge_batch_get_results` |
| 3.3 工具注解 | 为 tools 添加 `readOnly`、`destructive`、`idempotent` 注解 |
| 3.4 错误处理优化 | 统一的 McpError 转换，保留 AutoForge 内部错误信息 |
| 3.5 文档与配置示例 | `mcp-config.json` 示例（Claude Desktop、Kimi CLI、Cursor） |
| 3.6 E2E 测试 | 用 MCP Test Client 验证全功能 |

**Phase 3 验收标准**：
- 能同时运行多个 Run 并通过 MCP 监控
- 所有 Tools 有正确的 readOnly/destructive 标注
- 提供开箱即用的配置模板

---

## 六、关键设计决策

### 6.1 状态访问策略

MCP Tools 直接访问 AutoForge 的全局状态：

```rust
// mcp/tools/relay.rs
#[tool(description = "启动一个新的 Relay Run")]
async fn forge_start_relay_run(
    &self,
    #[tool(param)] flow_id: String,
    #[tool(param)] task: String,
    #[tool(param)] steps: Vec<FlowStepDto>,
) -> Result<CallToolResult, McpError> {
    let flow = crate::relay::flows::get_flow(&flow_id)
        .ok_or_else(|| McpError::invalid_request("Flow not found", None))?;
    
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    let run_state = crate::relay::store::start_run(
        &crate::relay::api::RUN_STORE,
        flow,
        &run_id
    ).map_err(|e| McpError::internal_error(e.to_string(), None))?;
    
    // Spawn driver in background
    tokio::spawn(crate::relay::driver::drive_run(
        run_id.clone(),
        crate::relay::api::RUN_STORE.clone(),
        crate::relay::api::EVENT_TX.clone(),
        self.ai_provider.clone(),
        Some(task),
        self.project_path.clone(),
    ));
    
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&run_state).unwrap()
    )]))
}
```

### 6.2 SSE 事件桥接

Relay Run 的 progress events 通过 MCP `notification` 推送给订阅的客户端：

```rust
// 在 driver 事件广播中增加 MCP 通道
pub enum RunEventBroadcast {
    // ... existing variants
    McpNotification { run_id: String, event: serde_json::Value },
}
```

### 6.3 认证

Phase 1-2 使用**本地模式**（stdio transport 或 localhost HTTP），依赖操作系统的用户权限。

Phase 3 可选添加：
- API Key header 验证（`X-API-Key`）
- Bearer Token（与现有 auth 系统集成）

---

## 七、预期收益与使用场景

### 场景 1：自动化性能回归测试

```
用户 (对 Claude Desktop):
"帮我用 AutoForge 跑 5 个不同配置的 Relay Run，
 对比 Agent 启动时间，然后出一份报告"

Claude (通过 MCP):
1. forge_list_professions() → 获取可用 professions
2. forge_start_relay_run() × 5 → 启动 runs
3. [等待]
4. forge_get_run() × 5 → 获取结果
5. forge_get_performance_logs() → 读取 timing 日志
6. [自动分析并输出报告]
```

### 场景 2：Specs 一致性审查

```
用户:
"读取 auto-forge 项目的 specs，检查 goals 和 designs 是否对齐"

Claude:
1. resources/read: forge://specs/auto-forge → 获取完整 specs
2. resources/read: forge://specs/auto-forge/goals → 获取 goals
3. resources/read: forge://specs/auto-forge/designs → 获取 designs
4. [分析并输出审查意见]
5. forge_update_specs() → 自动修复不一致（如果授权）
```

### 场景 3：对话种子 + 自动跟进

```
用户:
"创建一个对话让 advisor 检查一下项目的 i18n 模块"

Claude:
1. forge_create_session({project_path: "..."})
2. forge_send_message({content: "请检查 i18n 模块的 specs 完整性"})
3. [通过 SSE 监听对话进展]
4. forge_get_session() → 检查状态
5. [如果需要，自动 approve/reject]
```

---

## 八、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| rmcp SDK 版本更新导致 API 变更 | 中 | 锁定版本，关注官方迁移指南 |
| MCP 协议标准仍在演进 | 中 | 只实现核心功能（Tools/Resources/Prompts），避免前沿特性 |
| 全局 static 状态并发问题 | 低 | 复用现有的 `Mutex<HashMap>` 保护，无需新增并发控制 |
| 与前端 REST API 的语义分歧 | 低 | MCP 层直接调用内部函数，不封装 REST，语义一致 |
| 安全：AI 误操作删除/修改数据 | 中 | 为 destructive tools 标注 `destructive`，让客户端提示确认 |

---

## 九、工作量预估

| 阶段 | 天数 | 产出 |
|------|------|------|
| Phase 1：脚手架 + 核心 Tools | 3-4 天 | 可运行的 MCP Server，14 个核心 Tools |
| Phase 2：Resources + Prompts + 深度 Tools | 3-4 天 | 14 个 Resources，6 个 Prompts，21 个 Tools |
| Phase 3：高级功能 + 文档 | 2-3 天 | SSE 桥接、批量工具、配置模板、E2E 测试 |
| **总计** | **8-11 天** | **全功能 MCP Server** |

---

## 十、下一步

1. **用户确认计划** → 调整优先级和范围
2. **创建 Phase 1 开发分支** → `feat/mcp-server`
3. **实现 rmcp 依赖 + 模块结构** → 第一天
4. **逐个实现核心 Tools 并验证** → 第 2-4 天
