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

## 测试

### 接力流水线测试

快速模拟测试（零 LLM 成本）—— 验证流水线状态机、关卡逻辑和监控 UI：

```bash
# 启动后端
cd backend && cargo run

# 运行模拟接力测试
python backend/tests/forge_relay_mock.py
```

该测试通过 API 创建接力运行，并使用合成交接文档手动驱动各步骤。测试覆盖：
- **Post-discovery 流程** — 7 个步骤，无关卡，自动完成
- **Standard 流程** — 9 个步骤，顾问关卡暂停 + 审批
- **拒绝与重试** — 关卡拒绝存储反馈，重新执行步骤，然后审批

要在 UI 中观察运行状态，请在测试运行时打开 `http://localhost:5174/forge/relay`。

### 完整 E2E 聊天测试

要通过聊天层测试自动接力功能（需要 LLM API 密钥）：

1. 打开 `http://localhost:5174/forge/chats`
2. 发送：*"我想构建一个简单的缓存模块。目标：G1-增删改查，G2-TTL 过期，G3-单元测试。写完规格后启动 post-discovery 接力。"*
3. Isaac 应该会调用 `spawn_relay` —— 聊天中出现 🚀 接力卡片
4. 点击 **"Monitor →"** 打开 Relay 视图
5. 观察步骤进度；如果出现关卡则进行审批
6. 完成后聊天显示 `relay_complete`

## 文档

- [规格驱动 Forge 设计](docs/design/spec-driven-forge.md) — 核心设计理念
- [智能体接力编排](docs/design/agents-relay-orchestration.md) — 智能体如何协作
- [规格类别体系](docs/design/spec-categories.md) — 规格类型系统与状态生命周期
- [规格 UI 与可追溯性](docs/design/spec-ui-and-relations.md) — 前端规格管理

## 起源

AutoForge 最初在 [Auto 语言](https://github.com/auto-stack/auto-lang)项目中开发，作为其 AI 辅助开发工具链。现已提取为独立项目，可服务于任何代码库，不仅限于 Auto 语言项目。

## 许可证

MIT
