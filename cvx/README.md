# AutoForge 技术文档

## 文档概述

本文档是 AutoForge 项目的完整技术文档，由 CodeViewX 自动生成。文档提供了项目的全面技术分析，包括架构设计、核心机制、技术栈和开发指南。

## 文档结构

| 文档 | 说明 | 状态 |
|------|------|------|
| **README.md** | 本文件，文档导航和概述 | ✅ 已完成 |
| **01-overview.md** | 项目概述、技术栈、目录结构 | ⏳ 待生成 |
| **02-quickstart.md** | 快速开始指南 | ⏳ 待生成 |
| **03-architecture.md** | 系统架构设计 | ⏳ 待生成 |
| **04-core-mechanisms.md** | 核心工作机制详解 | ⏳ 待生成 |

## 文档元数据

- **生成时间**: 2025-01-19
- **分析范围**: 42 个源文件，约 15,000+ 行代码
- **主要技术栈**: Rust (Axum), Vue 3, Claude API
- **项目类型**: AI 辅助编程系统
- **文档语言**: 中文

## 项目简介

AutoForge 是一个**规格驱动的串行智能体 AI 编程助手**。它通过顺序编排专业化的 AI 智能体来完成软件开发任务，每个智能体只接收所需的上下文。规格（Specs）是唯一的真实来源，智能体通过结构化的规格文档协作，而非聊天历史。

### 核心特性

1. **规格驱动开发** - 7 类规格（目标、架构、设计、计划、测试、评审、报告），带类型化 ID（G1、A1、D1、P1、S1.1、V1、X42）和双向可追溯性
2. **串行智能体接力** - 智能体通过压缩的交接文档传递，而非完整聊天历史（相比并行多智能体节省约 5 倍 token）
3. **持久化执行** - 每次交接后自动保存检查点，可随时恢复或回滚
4. **Token 预算控制** - 逐步预算分配，自动压缩上下文，成本分析
5. **人工审批关卡** - GSD 模式（仅目标关卡）或 Check 模式（所有关卡）实现人在回路控制
6. **Web UI + CLI** - 聊天（Forge）、规格（Ledger）、智能体（Relay）三个视图，实时流式响应

### 架构概览

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

## 使用指南

### 阅读顺序建议

1. **新手入门**: README.md → 01-overview.md → 02-quickstart.md
2. **架构理解**: README.md → 01-overview.md → 03-architecture.md
3. **深度开发**: README.md → 01-overview.md → 03-architecture.md → 04-core-mechanisms.md

### 核心文档说明

- **01-overview.md**: 提供项目的技术栈、目录结构和模块组织
- **02-quickstart.md**: 详细的安装、配置和运行指南
- **03-architecture.md**: 系统的架构设计，包括模块划分、数据流和设计决策
- **04-core-mechanisms.md**: 深入分析核心工作流程，包括流程引擎、智能体执行和规格管理

## 开发者资源

### 项目结构

```
auto-forge/
├── backend/            # Rust 后端（Axum 服务器、Forge、Relay）
│   ├── src/
│   │   ├── forge/      # 聊天循环、工具定义、规格管理
│   │   ├── relay/      # 智能体编排、流水线、检查点
│   │   ├── provider/   # LLM 提供商（Claude/GPT）
│   │   └── runtime/    # 运行时上下文、权限、会话
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

### 快速链接

- **GitHub 仓库**: [auto-stack/auto-forge](https://github.com/auto-stack/auto-forge)
- **相关项目**: [Auto Language](https://github.com/auto-stack/auto-lang)

## 版本信息

- **当前版本**: 0.1.0
- **Rust 版本**: 1.80+
- **Node.js 版本**: 18+
- **许可证**: MIT

## 贡献指南

本文档由 CodeViewX 自动生成，基于对项目源代码的深度分析。如需更新文档，请：

1. 修改源代码中的注释和文档
2. 重新运行 CodeViewX 生成文档
3. 或手动编辑 `cvx/` 目录下的 Markdown 文件

## 反馈与支持

如有任何问题或建议，请通过以下方式联系：

- GitHub Issues
- 项目讨论区

---

**文档生成器**: CodeViewX  
**最后更新**: 2025-01-19  
**文档版本**: 1.0.0
