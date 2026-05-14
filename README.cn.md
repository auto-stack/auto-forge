# AutoForge

**规格驱动的串行智能体 AI 编程助手。**

AutoForge 按顺序编排专业化的 AI 智能体 —— 规划师 → 架构师 → 编码员 → 测试员 → 评审员 —— 每个智能体仅接收所需的上下文。规格（Specs）是唯一的真实来源；智能体通过结构化的规格文档协作，而非聊天历史。

最初是 [Auto 语言](https://github.com/auto-stack/auto-lang)项目的一部分，现为独立工具。

## 特性

- **规格驱动开发** — 7 类规格（目标、架构、设计、计划、测试、评审、报告），带类型化 ID（`G1`、`A1`、`D1`、`P1`、`S1.1`、`V1`、`X42`）和双向可追溯性
- **串行智能体接力** — 智能体之间传递压缩的交接文档而非完整聊天历史（相比并行多智能体节省约 5 倍 token）
- **持久化执行** — 每次交接后自动保存检查点，可随时恢复或回滚
- **Token 预算控制** — 逐步预算分配，自动压缩上下文，成本分析
- **人工审批关卡** — GSD 模式（仅目标关卡）或 Check 模式（所有关卡）实现人在回路控制
- **Web UI + CLI** — 聊天（Forge）、规格（Ledger）、智能体（Relay）三个视图，实时流式响应

## 架构

```
用户请求 → Forge（聊天循环）
                    ↓
             Relay（流水线引擎）
                    ↓
   [顾问] → [架构师] → [规划师] → [编码员] → [测试员] → [评审员]
       交接 →    交接 →     交接 →    交接 →   交接
                    ↓
            Specs（Ledger）← 唯一真实来源
```

### 核心概念

| 概念 | 说明 |
|------|------|
| **Forge** | 聊天循环，分类用户意图并路由到合适的智能体 |
| **Relay** | 流水线引擎，通过交接文档串行执行智能体流程 |
| **Specs** | 基于文件的知识库（`.ad` 文件）—— 智能体之间的契约 |
| **Agent** | 拥有灵魂（个性）、职业（职责范围）和模型（LLM 配置） |
| **Gate** | 关键决策点的人工审批检查站 |

### 8 种内置职业

助手、顾问、架构师、规划师、编码员、测试员、评审员、文档员 —— 每种职业拥有独立的工具访问权限和负责的规格区块。

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust, Axum 0.8, Tokio, Reqwest |
| 前端 | Vue 3, Vite, Markstream, Mermaid |
| AI 模型 | Claude, GPT（可按职业配置） |
| 存储 | 基于文件（规格在 `docs/specs/`，会话在 `~/.local/share/autoforge/`） |

## 快速开始

### 前置条件

- Rust 1.80+ 及 Cargo
- Node.js 18+（前端）
- LLM API 密钥（Anthropic 或 OpenAI）

### 构建与运行

**后端：**
```bash
cd backend
cargo build
cargo run    # 启动于 http://127.0.0.1:3031
```

**前端：**
```bash
cd frontend
pnpm install
pnpm run dev  # 开发服务器（热重载）
pnpm run build  # 生产构建
```

### 访问

- Web UI：`http://127.0.0.1:3031/forge`
- API：`http://127.0.0.1:3031/api/forge/*`

## 项目结构

```
auto-forge/
├── backend/            # Rust 后端（Axum 服务器、Forge、Relay）
│   ├── src/
│   │   ├── forge/      # 聊天循环、工具定义、规格管理
│   │   └── relay/      # 智能体编排、流水线、检查点
│   └── tests/
├── frontend/           # Vue 3 前端
│   ├── src/
│   │   ├── views/      # Chat、Specs、Agents 视图
│   │   ├── composables/# useForge、useLedger、useGateInbox
│   │   └── components/ # SpecItem、GatePanel、MarkdownContent 等
│   └── dist/
└── docs/
    ├── design/         # 架构与设计文档
    ├── plans/          # 实施计划
    └── specs/          # 规格模板与项目数据
```

## 文档

- [规格驱动 Forge 设计](docs/design/spec-driven-forge.md) — 核心设计理念
- [智能体接力编排](docs/design/agents-relay-orchestration.md) — 智能体如何协作
- [规格类别体系](docs/design/spec-categories.md) — 规格类型系统与状态生命周期
- [规格 UI 与可追溯性](docs/design/spec-ui-and-relations.md) — 前端规格管理

## 起源

AutoForge 最初在 [Auto 语言](https://github.com/auto-stack/auto-lang)项目中开发，作为其 AI 辅助开发工具链。现已提取为独立项目，可服务于任何代码库，不仅限于 Auto 语言项目。

## 许可证

MIT
