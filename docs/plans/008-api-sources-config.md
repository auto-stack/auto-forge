# Plan 008: API Sources 配置系统

## Context

AutoForge 目前只支持 Anthropic Claude 一个 AI 提供商，API key 从 `~/.claude/settings.json` 硬编码读取。随着 Agent 系统扩展，需要支持多个 AI 提供商（Anthropic、OpenAI、Ollama 本地等），用户需要在前端管理 API Source（提供商 + 模型列表 + 密钥），为后续 Agent 配置提供 LLM 来源基础。

API Sources 作为**顶层视图**，与 Chat、Specs、Relay 并列，因为未来还要加入 Tools、Skills 等顶层视图。

## 数据模型

### ApiSource — 一个 AI 提供商配置

```rust
pub struct ApiSource {
    pub id: String,              // "anthropic-primary", "openai-work", "local-ollama"
    pub name: String,            // "Anthropic (Claude)"
    pub provider: Provider,      // 复用现有 Provider enum (Anthropic, OpenAI, Local{url})
    pub api_key_env: String,     // 环境变量名 或 "settings:KEY" 表示 ~/.claude 检测
    pub api_key_stored: Option<String>,  // 直接存储的 key (base64 混淆)
    pub base_url: Option<String>,        // 自定义 base URL（代理或本地服务）
    pub is_available: bool,      // 启动时检测
    pub models: Vec<ModelDefinition>,
}
```

### ModelDefinition — 模型条目

```rust
pub struct ModelDefinition {
    pub id: String,              // "claude-3-5-sonnet-20241022"
    pub name: String,            // "Claude 3.5 Sonnet"
    pub tier: ModelTier,         // Light / Mid / Heavy
}

pub enum ModelTier { Light, Mid, Heavy }
```

### 持久化位置

- `dirs::data_local_dir()/autoforge/api_sources.json`

### 默认模型 Tier 映射

| Tier | Anthropic | OpenAI | 用途 |
|------|-----------|--------|------|
| Light | claude-3-5-haiku | gpt-4o-mini | 轻量任务（路由、测试生成） |
| Mid | claude-3-5-sonnet | gpt-4o | 平衡任务（规划、编码） |
| Heavy | claude-3-opus | o1 | 重推理（架构设计、评审） |

## Phase 1: Backend — 数据类型与持久化

### 1.1 新建 `backend/src/relay/config.rs`

核心类型：`ApiSource`、`ModelDefinition`、`ModelTier`

CRUD 函数：
- `load_api_sources() -> Vec<ApiSource>` — 读 JSON，文件不存在时执行自动检测
- `save_api_sources(sources: &[ApiSource])` — 写 JSON
- `detect_available_providers() -> Vec<ApiSource>` — 首次启动自动检测

自动检测逻辑（复用 `ai.rs:22-57` 的现有检测）：
1. 检查 `~/.claude/settings.json` 的 `ANTHROPIC_AUTH_TOKEN` → 创建 Anthropic source
2. 检查环境变量 `OPENAI_API_KEY` → 创建 OpenAI source
3. 检查 `http://localhost:11434/api/tags` → 创建 Ollama Local source
4. 都没有 → 返回空列表，前端引导用户配置

### 1.2 修改 `backend/src/relay/mod.rs`

- 添加 `pub mod config;`
- `RelayRegistry` 新增 `api_sources: Vec<ApiSource>` 字段
- `new()` 中调用 `config::load_api_sources()` 加载

### 1.3 新增 API 端点（在 `relay/api.rs` 的 `relay_routes()` 中）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/forge/config/api-sources` | 列出所有 API source |
| POST | `/api/forge/config/api-sources` | 创建新 source |
| PUT | `/api/forge/config/api-sources/{id}` | 更新 source |
| DELETE | `/api/forge/config/api-sources/{id}` | 删除 source |
| POST | `/api/forge/config/api-sources/{id}/test` | 测试连通性 |

### 1.4 关键文件

- `backend/src/relay/config.rs` — **新建**：类型 + CRUD + 自动检测
- `backend/src/relay/mod.rs` — 添加 config 模块 + 加载逻辑
- `backend/src/relay/api.rs` — 新增 5 个端点 + handler

## Phase 2: Backend — 多提供商 AI 分发

### 2.1 重构 `backend/src/ai.rs`

当前 `ClaudeProvider` 硬编码。改为通用分发器：

```rust
pub async fn dispatch_chat(
    source: &ApiSource,
    model: &str,
    system: &str,
    messages: Vec<ChatMessage>,
    tx: UnboundedSender<AIStreamDelta>,
) -> Option<String>
```

- `Provider::Anthropic` → 复用现有 Claude SSE 逻辑
- `Provider::OpenAI` → 新增 OpenAI chat completions 格式（`/v1/chat/completions`, Bearer token）
- `Provider::Local` → Ollama 兼容格式

### 2.2 修改 `backend/src/forge/ai.rs`

`ToolClaudeProvider` 泛化为 `ToolProviderDispatcher`，接受 `ApiSource` + model 而非硬编码 Claude。

### 2.3 API Key 运行时解析

优先级：`api_key_env` (环境变量) > `settings:KEY` (~/.claude) > `api_key_stored` (base64)

### 2.4 关键文件

- `backend/src/ai.rs` — 添加 `dispatch_chat()`，新增 OpenAI/Local 支持
- `backend/src/forge/ai.rs` — 泛化 provider

## Phase 3: Frontend — Composable

### 3.1 新建 `frontend/src/composables/useApiSources.ts`

单例模式（与 useProject.ts 一致）：

```typescript
interface ApiSource {
  id: string; name: string; provider: 'anthropic' | 'openai' | 'local'
  api_key_env: string; api_key_stored: string | null; base_url: string | null
  is_available: boolean; models: ModelDefinition[]
}
```

方法：`loadSources`, `createSource`, `updateSource`, `deleteSource`, `testConnection`

## Phase 4: Frontend — API Sources 视图

### 4.1 新建 `frontend/src/views/ApiSourcesView.vue`

**布局：** 左侧 Source 列表（280px） + 右侧详情编辑器

**左侧列表：**
- 每个 source 一张卡片：提供商图标 + 名称 + 可用状态圆点 + 模型数
- 底部 "Add API Source" 按钮

**右侧编辑器：**
- Provider 类型选择器（Anthropic / OpenAI / Local）
- 名称输入
- API Key 输入（密码框 + 显示/隐藏切换）
- Base URL（仅 Local 类型显示，其他可选）
- 模型管理表格（ID / Name / Tier 下拉）
- "Test Connection" 按钮
- Save / Delete 按钮

**首次启动：** 如果 sources 为空或全部不可用，显示引导提示

### 4.2 关键文件

- `frontend/src/composables/useApiSources.ts` — **新建**
- `frontend/src/views/ApiSourcesView.vue` — **新建**
- `frontend/src/App.vue` — 添加 APIs tab + 导入新视图

## Phase 5: 集成 App.vue 导航

### 5.1 修改导航栏

```typescript
// 新增 tab
{ id: 'apis', label: 'APIs', icon: Server }
```

- 移除 Demo tab
- 导入 `ApiSourcesView`
- 添加 `v-else-if="currentView === 'apis'"` 分支

## Verification

1. 首次启动（无 api_sources.json）→ 自动检测已有 Claude key
2. 手动添加 OpenAI source → 输入 key → Test Connection 成功
3. 编辑模型列表 → 添加/删除模型
4. 删除 source → 确认删除
5. 重启后端 → 验证 api_sources.json 持久化加载
6. 切换到其他视图 → 回到 APIs → 状态保持

## 实现顺序

Phase 1 (后端类型) → Phase 2 (多提供商) → Phase 3 (composable) → Phase 4 (视图) → Phase 5 (导航集成)
