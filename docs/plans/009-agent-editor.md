# Plan 009: Agent Editor — 角色配置系统

## Context

AutoForge 的 Relay 管线目前有 8 个内置 Profession（Assistant、Advisor、Architect、Planner、Tester、Coder、Reviewer、Documenter），每个 profession 绑定一个 Soul 和硬编码的 ModelConfig。用户无法自定义 agent 的"性格"（Soul）、职业（Profession）或使用的 LLM 来源。

需要一个**游戏角色卡**式的 Agent Editor，让用户像创建游戏角色一样定义 agent：选择职业（class）、编写性格（soul）、选择 LLM 来源（power source）。同时支持为每个 profession 创建多个 agent 变体。

Agent Editor 作为**顶层视图**，与 Chat、Specs、Relay、APIs 并列。

**前置依赖：** Plan 008 (API Sources) 必须先完成，Agent 需要引用已配置的 API Source。

## 数据模型

### AgentConfig — 一个可运行的 Agent 配置

```rust
pub struct AgentConfig {
    pub id: String,              // "default-planner", "fast-coder-v2"
    pub name: String,            // "Planner Agent"
    pub profession_id: String,   // 关联 ProfessionRegistry
    pub soul_id: String,         // 关联 SoulConfig
    pub api_source_id: String,   // 关联 ApiSource.id (来自 Plan 008)
    pub model_tier: ModelTier,   // Light / Mid / Heavy (来自 Plan 008)
    pub is_default: bool,        // 是否为内置默认 agent
    pub temperature: f32,
    pub max_tokens: u32,
    pub reasoning_budget: Option<u32>,
}
```

### 持久化位置

- `dirs::data_local_dir()/autoforge/agent_configs.json`

### 默认 Agent 配置（8 个，每个 profession 一个）

| Agent | Profession | Model Tier | 理由 |
|-------|-----------|------------|------|
| Assistant | assistant | Light | 简单路由，低 token |
| Advisor | advisor | Mid | 需要良好推理进行需求发现 |
| Architect | architect | Heavy | 重度设计推理，需要 reasoning_budget |
| Planner | planner | Mid | 平衡成本 |
| Tester | tester | Light | 测试用例生成较公式化 |
| Coder | coder | Mid | 最多轮次，平衡成本 |
| Reviewer | reviewer | Heavy | 需要仔细分析 |
| Documenter | documenter | Light | 文档生成较公式化 |

每个默认 agent 使用第一个可用的 ApiSource（优先 Anthropic）。

## Phase 1: Backend — 数据类型与持久化

### 1.1 扩展 `backend/src/relay/config.rs`（在 Plan 008 基础上）

新增类型和函数：

```rust
// AgentConfig 类型定义
pub struct AgentConfig { ... }

// CRUD 函数
pub fn load_agent_configs() -> Vec<AgentConfig>
pub fn save_agent_configs(configs: &[AgentConfig])
pub fn generate_default_agents(api_source_id: &str) -> Vec<AgentConfig>
```

`generate_default_agents()` 创建 8 个默认配置，绑定到指定的 `api_source_id`。

### 1.2 修改 `backend/src/relay/mod.rs`

- `RelayRegistry` 新增 `agent_configs: Vec<AgentConfig>` 字段
- `new()` 中调用 `config::load_agent_configs()`，若无则用第一个可用 ApiSource 生成默认
- 新增方法：
  - `get_agent_config(id: &str) -> Option<&AgentConfig>`
  - `resolve_model(config: &AgentConfig) -> ModelConfig` — 将 tier + ApiSource 转为具体 ModelConfig
  - `spawn_agent_from_config(config: &AgentConfig) -> Option<AgentInstance>` — 组装可运行 agent

`resolve_model` 逻辑：
1. 通过 `config.api_source_id` 找到 `ApiSource`
2. 从 `api_source.models` 中找到匹配 `config.model_tier` 的模型
3. 构造 `ModelConfig { provider, model, temperature, max_tokens, ... }`

### 1.3 新增 API 端点（在 `relay/api.rs` 的 `relay_routes()` 中）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/forge/config/agents` | 列出所有 agent 配置 |
| POST | `/api/forge/config/agents` | 创建 agent 配置 |
| PUT | `/api/forge/config/agents/{id}` | 更新 agent 配置 |
| DELETE | `/api/forge/config/agents/{id}` | 删除 agent（默认 agent 不可删除） |
| POST | `/api/forge/config/agents/reset-defaults` | 重置为 8 个默认 agent |

### 1.4 关键文件

- `backend/src/relay/config.rs` — 扩展：AgentConfig 类型 + CRUD + 默认生成
- `backend/src/relay/mod.rs` — 扩展 RelayRegistry + resolve_model + spawn_agent_from_config
- `backend/src/relay/api.rs` — 新增 5 个端点 + handler

## Phase 2: Frontend — Composable

### 2.1 新建 `frontend/src/composables/useAgentConfigs.ts`

单例模式：

```typescript
interface AgentConfigDto {
  id: string; name: string; profession_id: string
  soul_id: string; api_source_id: string
  model_tier: 'light' | 'mid' | 'heavy'
  is_default: boolean; temperature: number
  max_tokens: number; reasoning_budget: number | null
}
```

方法：`loadConfigs`, `createConfig`, `updateConfig`, `deleteConfig`, `resetDefaults`

### 2.2 关键文件

- `frontend/src/composables/useAgentConfigs.ts` — **新建**

## Phase 3: Frontend — Agent Editor 视图

### 3.1 前置重命名：AgentsView → RelayView

- `frontend/src/views/AgentsView.vue` → 重命名为 `RelayView.vue`
- `App.vue` 中更新导入和引用（功能不变，仅改名释放 "Agents" 名称）

### 3.2 新建 `frontend/src/views/AgentsConfigView.vue`

**设计理念：** 游戏角色卡（Character Sheet）体验

**布局：** 顶部标题栏 + 响应式 agent 卡片网格

**Header：**
- 标题 "Agent Forge"
- "Create Agent" 按钮
- "Reset Defaults" 按钮

**Agent 卡片网格（每张约 300×400px）：**

1. **Avatar 区域** — Profession 图标（大号 64px emoji），覆盖 agent 名称
2. **角色属性条** — 三个紧凑 badge：
   - Profession badge（按 phase 着色）
   - Model Tier badge（Light=绿, Mid=蓝, Heavy=紫）
   - API Source 可用性指示灯
3. **Soul 预览** — Soul markdown 前 2-3 行，淡色渲染，作为"性格标语"
4. **操作按钮** — Edit / Duplicate / Delete（默认 agent 不可删除，显示 "Reset"）

**点击卡片 → 打开 Agent 编辑面板（侧滑或内联展开）：**

- **Name** 输入框
- **Profession** 下拉 — 8 个内置 profession + 自定义（来自 `useRelay().professions`），每个带 emoji 和 phase 标签
- **Soul** — Markdown 编辑区。创建时按所选 profession 预填充默认 soul。使用 textarea + markdown 预览
- **API Source** — 下拉列出已配置的 API sources（来自 `useApiSources()`），显示名称 + provider 类型
- **Model Tier** — "战力等级" 选择器，三个视觉化选项：
  - Light: "Light" 单条杠图标，绿色调
  - Mid: "Mid" 双条杠图标，蓝色调
  - Heavy: "Heavy" 三条杠图标，紫色调
- **Advanced** 可折叠区域：
  - Temperature 滑块 (0.0 - 1.0)
  - Max Tokens 输入
  - Reasoning Budget 输入（仅 tier=Heavy 时显示）

**默认 agent 视觉区分：** 微妙边框 + "Default" badge，只能修改不能删除。

### 3.3 关键文件

- `frontend/src/views/AgentsView.vue` → 重命名为 `RelayView.vue`
- `frontend/src/views/AgentsConfigView.vue` — **新建**
- `frontend/src/App.vue` — 更新导入、tabs、currentView 类型

## Phase 4: 导航集成

### 4.1 修改 App.vue

```typescript
// tabs 数组更新
const tabs = [
  { id: 'chats', label: 'Chat', icon: MessageSquare },
  { id: 'specs', label: 'Specs', icon: Scroll },
  { id: 'agents', label: 'Relay', icon: Orbit },          // RelayView
  { id: 'agents-config', label: 'Agents', icon: Users },   // 新 Agent Editor
  { id: 'apis', label: 'APIs', icon: Server },             // API Sources (Plan 008)
]
```

- 移除 Demo tab
- 导入 `Users`, `Server` from lucide-vue-next
- currentView 类型扩展为 `'chats' | 'specs' | 'agents' | 'agents-config' | 'apis'`
- 添加对应的 `v-else-if` 分支

## Phase 5: Pipeline 集成

### 5.1 修改 `backend/src/relay/pipeline.rs`

Pipeline 启动时，每一步可以通过 `agent_config_id` 指定使用哪个 agent 配置：

```rust
pub struct FlowStep {
    pub id: String,
    pub profession_id: String,
    pub agent_config_id: Option<String>,  // 新增：指定 agent 配置
    pub gate: GateType,
    ...
}
```

如果不指定，默认使用该 profession 的默认 agent（`is_default=true`）。

### 5.2 `StartRunRequest` 扩展

```rust
pub struct FlowStepDto {
    pub id: String,
    pub profession_id: String,
    pub agent_config_id: Option<String>,  // 新增
    pub gate: String,
}
```

### 5.3 关键文件

- `backend/src/relay/flow.rs` — FlowStep 新增 `agent_config_id`
- `backend/src/relay/pipeline.rs` — spawn 时使用 `RelayRegistry::spawn_agent_from_config()`
- `backend/src/relay/api.rs` — FlowStepDto 新增字段

## Verification

1. 首次启动 → 8 个默认 agent 自动生成
2. 点击 agent 卡片 → 编辑面板打开，显示完整配置
3. 修改 soul markdown → 保存 → 卡片预览更新
4. 切换 profession → soul 预填充更新
5. 切换 API Source → 可用性状态更新
6. 创建新 agent（同一 profession 第二个变体）→ 成功
7. 删除自定义 agent → 成功；删除默认 agent → 被拒绝
8. Reset Defaults → 恢复 8 个默认配置
9. 启动 Relay run → 使用指定的 agent config → 正确的 provider/model/tier

## 实现顺序

Phase 1 (后端类型) → Phase 2 (composable) → Phase 3 (视图 + 重命名) → Phase 4 (导航) → Phase 5 (pipeline 集成)

**依赖关系：** Phase 1-4 可在 Plan 008 完成后立即开始。Phase 5 需要 Plan 008 的 API Source 和多提供商支持就绪。
