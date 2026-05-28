# AutoForge 项目概述

## 文档说明

本文档提供 AutoForge 项目的全面技术概述，包括技术栈、目录结构、模块组织和核心概念。

**文档信息**
- **生成时间**: 2025-01-19
- **分析文件数**: 42 个源文件
- **代码行数**: 约 15,000+ 行
- **文档语言**: 中文

---

## 项目简介

AutoForge 是一个**规格驱动的串行智能体 AI 编程助手**。它通过顺序编排专业化的 AI 智能体来完成软件开发任务，每个智能体只接收所需的上下文，通过结构化的规格文档协作。

### 核心价值主张

1. **规格驱动**: 规格是唯一的真实来源，智能体通过规格文档协作，而非聊天历史
2. **串行执行**: 相比并行多智能体节省约 5 倍 token 成本
3. **持久化**: 每次交接后自动保存检查点，可随时恢复或回滚
4. **人在回路**: 通过关卡（Gates）实现人工审批控制

### 应用场景

- 软件开发项目的规格设计和实现
- 代码重构和架构演进
- Bug 修复和测试驱动开发
- 文档生成和知识库维护

---

## 技术栈

### 后端技术栈

| 技术 | 版本 | 用途 |
|------|------|------|
| **Rust** | 1.80+ | 主要编程语言 |
| **Axum** | 0.8 | Web 框架，提供 HTTP 和 WebSocket 支持 |
| **Tokio** | 1.x | 异步运行时 |
| **Reqwest** | 0.12 | HTTP 客户端，用于调用 LLM API |
| **Serde** | 1.x | 序列化/反序列化 |
| **Tower-HTTP** | 0.6 | HTTP 中间件（CORS、静态文件服务） |
| **Tracing** | 0.1 | 日志和追踪 |
| **UUID** | 1.x | 唯一标识符生成 |

**后端依赖分析** (基于 `backend/Cargo.toml`):

```toml
[dependencies]
axum = { version = "0.8", features = ["json", "ws", "multipart"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "stream"] }
uuid = { version = "1", features = ["v4"] }
tower-http = { version = "0.6", features = ["cors", "fs"] }
```

### 前端技术栈

| 技术 | 版本 | 用途 |
|------|------|------|
| **Vue 3** | 3.5+ | 前端框架 |
| **Vite** | 6.x | 构建工具和开发服务器 |
| **TypeScript** | 5.8+ | 类型安全的 JavaScript |
| **TipTap** | 3.23+ | 富文本编辑器 |
| **Markstream** | 0.0.14-beta.8 | Markdown 流式渲染 |
| **Mermaid** | 11.15+ | 图表渲染 |
| **Yjs** | 13.6+ | 实时协作 |

**前端核心依赖** (基于 `frontend/package.json`):

```json
{
  "dependencies": {
    "vue": "^3.5.32",
    "@tiptap/vue-3": "^3.23.4",
    "markstream-vue": "^0.0.14-beta.8",
    "mermaid": "^11.15.0"
  },
  "devDependencies": {
    "vite": "^6.3.5",
    "typescript": "~5.8.3",
    "@vitejs/plugin-vue": "^6.0.6"
  }
}
```

### AI 模型支持

| 提供商 | 模型 | 用途 |
|--------|------|------|
| **Anthropic** | Claude 3.5 Opus/Sonnet/Haiku | 主要推理模型 |
| **OpenAI** | GPT-4o | 备用模型 |
| **本地模型** | 自定义 | 通过 Local URL 配置 |

### 存储和持久化

- **规格文件**: `.ad` 格式，存储在 `docs/specs/`
- **会话数据**: `~/.local/share/autoforge/`
- **配置文件**: `.autoforge/` 目录
- **检查点**: 文件系统快照

---

## 目录结构

### 项目根目录

```
auto-forge/
├── backend/              # Rust 后端
├── frontend/             # Vue 3 前端
├── docs/                 # 项目文档
│   ├── design/          # 设计文档
│   ├── plans/           # 实施计划
│   └── specs/           # 规格模板
├── scripts/             # 辅助脚本
├── specs/               # 项目规格文件
├── wiki/                # Wiki 知识库
├── .claude/             # Claude 配置
├── .git/                # Git 仓库
├── .gitignore           # Git 忽略规则
├── CLAUDE.md            # Claude AI 配置
├── README.md            # 项目说明（英文）
└── README.cn.md         # 项目说明（中文）
```

### 后端目录结构

```
backend/
├── src/
│   ├── main.rs              # 应用入口，HTTP 服务器启动
│   ├── lib.rs               # 库入口
│   ├── cache.rs             # 内存缓存实现
│   ├── forge/               # Forge 模块（聊天循环）
│   │   ├── mod.rs           # Forge 主模块（约 2800 行）
│   │   ├── errand.rs        # Errand 会话管理
│   │   ├── project.rs       # 项目管理
│   │   ├── tools.rs         # 工具注册和定义
│   │   ├── wiki.rs          # Wiki 知识库
│   │   └── templates/       # 提示词模板
│   ├── relay/               # Relay 模块（智能体编排）
│   │   ├── mod.rs           # Relay 主模块
│   │   ├── agent.rs         # 智能体实例（约 390 行）
│   │   ├── api.rs           # REST API 端点
│   │   ├── budget.rs        # Token 预算管理
│   │   ├── checkpoint.rs    # 检查点系统
│   │   ├── config.rs        # 配置管理
│   │   ├── driver.rs        # Relay 执行驱动
│   │   ├── flow.rs          # 流程规范（约 370 行）
│   │   ├── flows.rs         # 内置流程定义
│   │   ├── handoff.rs       # 交接文档
│   │   ├── pipeline.rs      # 流水线引擎（约 1010 行）
│   │   ├── profession.rs    # 职业定义
│   │   ├── skills.rs        # 技能系统
│   │   ├── soul.rs          # 灵魂配置
│   │   ├── store.rs         # 运行存储
│   │   ├── title.rs         # 标题生成
│   │   ├── turn.rs          # 智能体轮次
│   │   └── souls/           # 灵魂模板（Markdown）
│   ├── provider/            # LLM 提供商
│   │   ├── mod.rs           # Provider 主模块
│   │   ├── claude.rs        # Claude API 实现
│   │   ├── sse.rs           # SSE 解析器
│   │   └── types.rs         # 共享类型定义
│   └── runtime/             # 运行时
│       ├── mod.rs           # Runtime 主模块
│       ├── context.rs       # 上下文管理
│       ├── permission.rs    # 权限策略
│       └── session.rs       # 会话管理
├── tests/                   # 测试文件
│   └── forge_relay_mock.py  # Mock 测试
├── Cargo.toml               # Rust 项目配置
└── Cargo.lock               # 依赖锁定
```

### 前端目录结构

```
frontend/
├── src/
│   ├── main.ts              # 应用入口
│   ├── App.vue              # 根组件
│   ├── views/               # 页面视图
│   │   ├── ChatView.vue     # 聊天界面
│   │   ├── SpecsView.vue    # 规格管理
│   │   └── RelayView.vue    # Relay 监控
│   ├── components/          # 可复用组件
│   │   ├── SpecItem.vue     # 规格项组件
│   │   ├── GatePanel.vue    # 关卡面板
│   │   └── MarkdownContent.vue  # Markdown 渲染
│   ├── composables/         # 组合式 API
│   │   ├── useForge.ts      # Forge 状态管理
│   │   ├── useLedger.ts     # Ledger 状态管理
│   │   └── useGateInbox.ts  # 关卡收件箱
│   ├── types/               # TypeScript 类型
│   ├── utils/               # 工具函数
│   └── styles/              # 样式文件
├── public/                  # 静态资源
├── dist/                    # 构建输出
├── index.html               # HTML 入口
├── package.json             # 项目配置
├── vite.config.ts           # Vite 配置
└── tsconfig.json            # TypeScript 配置
```

---

## 核心模块组织

### 后端模块划分

#### 1. Forge 模块 (`backend/src/forge/`)

**职责**: 聊天循环、工具定义、规格管理

**核心文件**:
- `mod.rs` (2800+ 行): Forge 主逻辑，包括会话管理、API 端点
- `tools.rs`: 工具注册表和工具定义
- `project.rs`: 项目管理和文件浏览
- `wiki.rs`: Wiki 知识库管理
- `errand.rs`: Errand 会话（异步任务）

**主要功能**:
- 聊天会话管理 (`ForgeSession`)
- 规格文档存储 (`SpecsStore`)
- 工具注册和调用 (`ToolRegistry`)
- 项目文件浏览和读取
- Wiki 页面管理

#### 2. Relay 模块 (`backend/src/relay/`)

**职责**: 智能体编排、流水线执行、检查点管理

**核心文件**:
- `pipeline.rs` (1010 行): 流水线状态机
- `agent.rs` (390 行): 智能体实例和上下文
- `flow.rs` (370 行): 流程规范定义
- `flows.rs` (585 行): 内置流程定义
- `handoff.rs`: 交接文档结构
- `checkpoint.rs`: 检查点系统
- `profession.rs`: 职业定义
- `soul.rs`: 灵魂配置

**主要功能**:
- 流水线引擎 (`PipelineEngine`)
- 智能体实例化 (`AgentInstance`)
- 流程规范 (`FlowSpec`, `FlowStep`)
- 检查点保存和恢复
- Token 预算管理

#### 3. Provider 模块 (`backend/src/provider/`)

**职责**: LLM API 集成

**核心文件**:
- `claude.rs`: Claude API 实现
- `types.rs`: 共享类型定义
- `sse.rs`: Server-Sent Events 解析

**主要功能**:
- Claude API 调用
- 流式响应处理
- 工具调用协议

#### 4. Runtime 模块 (`backend/src/runtime/`)

**职责**: 运行时上下文和权限管理

**核心文件**:
- `context.rs`: 上下文管理
- `permission.rs`: 权限策略
- `session.rs`: 会话存储

### 前端模块划分

#### 1. Views 层

**主要视图**:
- `ChatView.vue`: 聊天界面，显示对话历史和工具调用
- `SpecsView.vue`: 规格管理界面，显示和编辑规格文档
- `RelayView.vue`: Relay 监控界面，显示流水线状态

#### 2. Components 层

**核心组件**:
- `SpecItem.vue`: 规格项显示和编辑
- `GatePanel.vue`: 人工审批关卡面板
- `MarkdownContent.vue`: Markdown 渲染组件

#### 3. Composables 层

**状态管理**:
- `useForge.ts`: Forge 状态（聊天会话、消息）
- `useLedger.ts`: Ledger 状态（规格文档）
- `useGateInbox.ts`: 关卡收件箱状态

---

## 核心概念

### 1. Forge（聊天循环）

Forge 是聊天循环的核心，负责：
- 接收用户消息
- 意图分类和路由
- 工具调用管理
- 规格文档更新

**关键数据结构**:
```rust
pub struct ForgeSession {
    pub id: String,
    pub project: String,
    pub messages: Vec<ForgeMessage>,
    pub status: ForgeStatus,
}
```

### 2. Relay（流水线引擎）

Relay 是智能体编排引擎，负责：
- 流程定义和执行
- 智能体实例化
- 交接文档传递
- 检查点管理

**关键数据结构**:
```rust
pub struct PipelineEngine {
    pub flow: FlowSpec,
    pub current_step: usize,
    pub status: PipelineStatus,
    pub step_history: Vec<StepRecord>,
}
```

### 3. Specs（规格文档）

规格是项目的唯一真实来源，包括 7 类：
- **Goals** (G1, G2, ...): 项目目标
- **Architecture** (A1, A2, ...): 架构设计
- **Designs** (D1, D2, ...): 详细设计
- **Plans** (P1, P2, ...): 实施计划
- **Tests** (S1.1, S1.2, ...): 测试规格
- **Reviews** (V1, V2, ...): 评审记录
- **Reports** (X42, X43, ...): 最终报告

### 4. Agent（智能体）

智能体由三部分组成：
- **Soul** (灵魂): 个性、价值观、行为风格
- **Profession** (职业): 职责范围、工具权限、负责的规格区块
- **Model** (模型): LLM 配置（提供商、模型、温度等）

**8 种内置职业**:
1. Assistant (助手) - 意图分类和路由
2. Advisor (顾问) - 需求分析和目标定义
3. Architect (架构师) - 系统架构设计
4. Planner (规划师) - 实施计划制定
5. Coder (编码员) - 代码实现
6. Tester (测试员) - 测试编写和执行
7. Reviewer (评审员) - 代码评审
8. Documenter (文档员) - 文档生成

### 5. Gate（关卡）

关卡是人工审批检查站，用于关键决策点：
- **GoalGate**: Advisor → Architect 边界（必须审批）
- **其他关卡**: 根据模式决定是否需要审批

**两种模式**:
- **GSD 模式**: 只有 GoalGate 需要审批
- **Check 模式**: 所有关卡都需要审批

---

## 数据流

### 聊天流程

```
用户输入 
  → Forge (意图分类)
  → 工具调用 (read_file, write_specs, etc.)
  → LLM 响应
  → 更新会话历史
  → 流式输出到前端
```

### Relay 流程

```
用户请求
  → spawn_relay 工具
  → PipelineEngine 启动
  → 执行第一步 (e.g., Architect)
  → Agent 实例化并运行
  → 生成 HandoffDocument
  → 提交到 pipeline
  → 自动验证
  → 检查关卡
  → 执行下一步或等待审批
  → 重复直到完成
```

---

## API 端点

### Forge API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/forge/sessions` | POST | 创建会话 |
| `/api/forge/sessions/:id/messages` | POST | 发送消息 |
| `/api/forge/sessions/:id/stream` | GET | SSE 流式响应 |
| `/api/forge/sessions` | GET | 列出会话 |
| `/api/forge/specs/:project` | GET | 获取规格文档 |
| `/api/forge/specs/:project` | PUT | 更新规格文档 |

### Relay API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/relay/runs` | POST | 启动 Relay |
| `/api/relay/runs` | GET | 列出运行 |
| `/api/relay/runs/:id` | GET | 获取运行状态 |
| `/api/relay/runs/:id/advance` | POST | 推进运行 |
| `/api/relay/runs/:id/gate` | POST | 解决关卡 |
| `/api/relay/professions` | GET | 列出职业 |
| `/api/relay/souls` | GET | 列出灵魂 |

---

## 配置文件

### 后端配置

**位置**: `backend/Cargo.toml`

**关键配置**:
- Axum 0.8 (Web 框架)
- Tokio (异步运行时)
- Reqwest (HTTP 客户端)

### 前端配置

**位置**: `frontend/package.json`, `frontend/vite.config.ts`

**关键配置**:
- Vue 3.5+ (前端框架)
- Vite 6.x (构建工具)
- TypeScript 5.8+ (类型系统)

---

## 开发环境

### 后端环境

- **Rust**: 1.80+
- **Cargo**: 构建工具
- **操作系统**: Windows, Linux, macOS

### 前端环境

- **Node.js**: 18+
- **pnpm**: 包管理器
- **浏览器**: Chrome, Firefox, Safari (现代浏览器)

---

## 测试

### 后端测试

**Mock 测试** (`backend/tests/forge_relay_mock.py`):
- Relay 流水线测试
- 关卡逻辑测试
- 状态机验证

**运行方式**:
```bash
cd backend
cargo run
python backend/tests/forge_relay_mock.py
```

### 前端测试

**E2E 测试** (Playwright):
- 聊天界面测试
- 规格编辑测试
- Relay 监控测试

---

## 部署

### 开发部署

**后端**:
```bash
cd backend
cargo run
# 启动于 http://127.0.0.1:3031
```

**前端**:
```bash
cd frontend
pnpm install
pnpm run dev
# 开发服务器（热重载）
```

### 生产部署

**后端**:
```bash
cargo build --release
./target/release/auto-forge
```

**前端**:
```bash
pnpm run build
# 输出到 frontend/dist/
```

---

## 性能特性

### Token 优化

- **串行执行**: 相比并行多智能体节省约 5 倍 token
- **上下文压缩**: 每次交接只传递必要信息
- **预算控制**: 逐步预算分配，防止成本失控

### 持久化

- **检查点**: 每次交接后自动保存
- **恢复机制**: 可随时恢复或回滚
- **文件快照**: 保存项目状态

---

## 安全性

### API 密钥管理

- Claude API 密钥从环境变量读取
- 支持多个 API 源配置
- 密钥编码存储

### 权限控制

- 工具访问权限（按职业）
- 文件读写权限
- 操作审计日志

---

## 扩展性

### 自定义流程

支持通过 YAML 定义自定义流程：
```yaml
id: custom-flow
steps:
  - id: step1
    profession_id: architect
    gate: Auto
    exit: Next
```

### 自定义职业

支持添加自定义职业配置：
- 职责范围
- 工具权限
- 模型配置

### 自定义灵魂

支持通过 Markdown 定义智能体个性：
```markdown
# Soul of the Custom Agent

## Core Values
- Value 1
- Value 2
```

---

## 相关资源

### 内部文档

- [规格驱动 Forge 设计](../docs/design/spec-driven-forge.md)
- [智能体接力编排](../docs/design/agents-relay-orchestration.md)
- [规格类别体系](../docs/design/spec-categories.md)
- [规格 UI 与可追溯性](../docs/design/spec-ui-and-relations.md)

### 外部资源

- [Axum 文档](https://docs.rs/axum/)
- [Vue 3 文档](https://vuejs.org/)
- [Claude API 文档](https://docs.anthropic.com/)

---

**文档生成器**: CodeViewX  
**最后更新**: 2025-01-19  
**文档版本**: 1.0.0
