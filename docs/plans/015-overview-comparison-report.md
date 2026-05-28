# Report: Overview Comparison — Old vs New vs CodeViewX

**Date:** 2026-05-28
**Scope:** Compare three versions of AutoForge project overview documentation
1. **Old Overview** (`specs/auto-forge/overview.ad` — before improvements)
2. **New Overview** (`specs/auto-forge/overview_v2.ad` — simulated Architect output with enhanced soul/skills)
3. **CodeViewX** (`cvx/01-overview.md` + `cvx/03-architecture.md` — external AI-generated docs)

---

## Summary Matrix

| Dimension | Old Overview | New Overview | CodeViewX | Winner |
|-----------|-------------|--------------|-----------|--------|
| **技术栈详情** | ❌ 无 | ✅ 完整表格（框架+版本+来源+用途） | ✅ 完整表格 + 依赖片段 | 新版 ≈ CVX |
| **核心概念解释** | ❌ 无 | ✅ 8 个概念表（定义+所在模块） | ✅ 在项目简介中分散解释 | 新版 |
| **模块信息深度** | ⚠️ 仅描述+链接 | ✅ 描述+组件+依赖+链接 | ⚠️ 在架构文档中详细描述 | 新版 |
| **流程视角** | ❌ 无 | ✅ 5 个核心流程速览+链接flows.ad | ✅ 04-core-mechanisms.md 深入分析 | CVX（深度）/ 新版（结构） |
| **代码引用** | ❌ 无 | ⚠️ 无精确行号（但模块ad中有组件名） | ✅ 文件路径+精确行号+10-20行代码 | CVX |
| **部署架构** | ❌ 无 | ✅ Dev/Prod 模式表格 | ✅ 02-quickstart.md 详细部署 | CVX |
| **目录结构** | ❌ 无 | ✅ 树形展示 | ✅ 完整目录结构+文件数 | CVX |
| **导航指南** | ⚠️ 8步 | ✅ 10步（加入flows+module.ad） | ✅ 按读者类型分层的阅读指南 | 新版 |
| **与Specs系统集成** | ✅ 深度集成 | ✅ 深度集成+增强 | ❌ 独立文档，无ID链接 | 旧/新版 |
| **Mermaid图表** | ✅ 1个架构图 | ✅ 1个架构图 | ✅ 多个（架构+数据流+时序） | CVX |
| **准确性保障** | ⚠️ 依赖人工维护 | ⚠️ 新约束要求验证（未实际运行） | ✅ Prompt强制"验证后描述" | CVX |

---

## Detailed Comparison

### 1. 技术栈详情

**Old Overview:**
> 完全没有技术栈部分。只在架构图的节点中标注了 "Vue 3"、"Axum" 等名称，无版本、无依赖来源。

**New Overview:**
```markdown
### Backend
| Technology | Version | Source | Purpose |
|------------|---------|--------|---------|
| **Rust** | 1.80+ | `Cargo.toml` | Systems programming |
| **Axum** | 0.8 | `Cargo.toml` | Web framework |
...

### Frontend
| Technology | Version | Source | Purpose |
|------------|---------|--------|---------|
| **Vue 3** | 3.5+ | `package.json` | UI framework |
...

### AI Providers
| Provider | Models | Configuration |
|----------|--------|---------------|
| **Anthropic** | Claude 3.5 Opus / Sonnet / Haiku | `ANTHROPIC_API_KEY` env |
```

**CodeViewX:**
> 同样提供完整的技术栈表格，但额外包含：
> - `Cargo.toml` 和 `package.json` 的完整依赖片段（10-20行）
> - 每个依赖的具体版本号验证（"基于 `backend/Cargo.toml` 确认"）

**Assessment:** 新版弥补了旧版的最大空白。CodeViewX 更详细（有依赖片段），但新版的结构更紧凑。**差距缩小到 90%。**

---

### 2. 核心概念解释

**Old Overview:**
> 无核心概念表。读者看到 "Soul"、"Profession"、"Relay" 等术语时，需要跳转到各个模块的 goals.ad 才能理解含义。

**New Overview:**
```markdown
| Concept | Definition | Lives In |
|---------|------------|----------|
| **Soul** | Markdown personality definition for an agent | `agent-config` |
| **Profession** | Capability contract: owned specs, allowed tools | `agent-config` |
| **Relay** | Serial agent pipeline with baton-passing | `relay` |
| **Gate** | Human-in-the-loop approval point | `relay` / `chat` |
| **Handoff** | Compressed context document (~5× token savings) | `relay` |
| **Spec** | Structured decision record in `.ad` format | `specs` |
| **Checkpoint** | JSON + git snapshot for crash recovery | `runtime` |
| **Errand** | Fire-and-forget research task | `errand` |
```

**CodeViewX:**
> 核心概念分散在 `01-overview.md` 的"核心概念"段落和 `03-architecture.md` 的各层描述中，没有统一的表格。

**Assessment:** 新版独创了核心概念表，这是 CodeViewX 也没有的。**新版在此维度超越双方。**

---

### 3. 模块信息深度

**Old Overview — Module Index:**
```markdown
| Module | Description | Specs |
|--------|-------------|-------|
| **relay** | Serial agent flows with baton passing | [module](relay/module.ad) · [goals] ... |
```
> 只有 1 句话描述 + spec 链接。

**New Overview — Module Index:**
```markdown
| Module | Description | Key Components | Dependencies | Specs |
|--------|-------------|----------------|--------------|-------|
| **relay** | Serial agent flows... | `PipelineEngine`, `HandoffManager`, `CheckpointManager`, `BudgetTracker` | agent-config, chat, errand, runtime, specs, ui-system | [module] · [goals] ... |
```
> 4 列信息：描述、关键组件、依赖模块、spec 链接。

**CodeViewX:**
> 模块信息分散在 `01-overview.md` 的"模块组织"和 `03-architecture.md` 的各层详细描述中。每个模块有 2-3 段落描述 + 代码示例。

**Assessment:** 新版的模块索引表在单屏内提供了最大信息密度。CodeViewX 更详细但需要滚动多屏。**新版在导航效率上更优。**

---

### 4. 流程视角

**Old Overview:**
> 完全无流程视角。读者无法理解 "Chat Loop" 或 "Relay Pipeline" 如何跨模块工作。

**New Overview:**
```markdown
## Core Flows

Five execution flows span multiple modules. See [flows.ad](flows.ad) for full sequence diagrams.

| Flow | Entry | Exit | Key Modules |
|------|-------|------|-------------|
| **Forge Chat Loop** | User sends message | SSE delivers response | chat, provider, runtime, relay |
| **Relay Pipeline** | FlowSpec loaded | Completed / Failed / Paused | relay, provider, runtime, specs |
...
```
> 提供速览表格 + 链接到 `flows.ad`（含完整时序图）。

**CodeViewX:**
> `04-core-mechanisms.md`（1239 行）深入分析 5 个核心流程，每个流程包含：
> - 概述
> - Mermaid 时序图
> - 详细步骤（触发条件 / 核心代码 / 数据流 / 关键点）
> - 异常处理
> - 设计亮点

**Assessment:** CodeViewX 在深度上无可匹敌（有代码引用和详细步骤）。但新版的 `flows.ad` + overview 速览提供了**可导航的流程索引**，而 CodeViewX 的流程是独立文档、与模块 specs 无链接。**新版在系统集成度上更优，CodeViewX 在分析深度上更优。**

---

### 5. 代码引用精度

**Old Overview:** ❌ 无任何代码引用。

**New Overview:** ⚠️ 无精确代码行号，但模块索引中有组件名称（如 `PipelineEngine`、`HandoffManager`）。这些名称来自 `module.ad` 的自动生成。

**CodeViewX:** ✅ 每个结论都有 `文件:行号 | 描述` 格式，并附带 10-20 行代码片段。例如：
```rust
// 文件: backend/src/forge/mod.rs | 行: 2024-2076
pub async fn create_forge_session(...) -> Json<ForgeSession> {
    let session = ForgeSession::new(req.project, req.title);
    ...
}
```

**Assessment:** 这是 CodeViewX 的核心优势，也是新版 specs 系统的**最大差距**。新版的 Architect soul 已加入"代码验证强制令"，但实际运行中 Architect 是否遵守取决于 gofer 调用的准确性。**差距仍然显著。**

---

### 6. 部署架构

**Old Overview:** ❌ 无部署信息。

**New Overview:**
```markdown
| Mode | Backend | Frontend | Command |
|------|---------|----------|---------|
| Development | `:3031` | `:5173` (Vite dev) | `cargo run` + `npm run dev` |
| Production | `:3031` | served by backend from `frontend/dist/` | `cargo run --release` |
```

**CodeViewX:**
> `02-quickstart.md`（689 行）提供完整的部署指南：安装步骤、配置、验证、Docker 示例、FAQ。

**Assessment:** 新版添加了极简的部署速览。CodeViewX 有完整的快速开始文档。AutoForge 目前**没有 02-quickstart 等价物**，这是一个值得补充的空白。

---

### 7. 与 Specs 系统的集成度

**Old/New Overview:** ✅ 深度集成。Overview 中的每个模块链接都指向 `module.ad`，`flows.ad` 中的每个流程都链接回 `Relay-A1`、`Chat-D3` 等 spec IDs。Spec IDs 在文档间形成双向图。

**CodeViewX:** ❌ 独立文档集。CodeViewX 生成的文档是"一次性快照"，与 AutoForge 的 live spec 系统无连接。如果 specs 更新，CodeViewX 的文档不会自动同步。

**Assessment:** 这是 AutoForge specs 系统的**根本优势**。CodeViewX 适合外部审计或 onboarding，但不适合作为 living documentation。**旧/新版在此维度碾压 CodeViewX。**

---

### 8. 准确性保障机制

**Old Overview:** ⚠️ 依赖人工维护。Architect 的 soul 要求更新 overview，但无强制验证规则。

**New Overview:** ⚠️ Architect soul v2 加入了"代码验证强制令"，但尚未经过实际运行验证。约束包括：
> - 读取 `Cargo.toml`/`package.json` 验证技术栈
> - dispatch gofer 验证文件存在性
> - 每个结论引用实际代码

**CodeViewX:** ✅ Prompt 中硬编码了 9 条质量约束，包括"绝对禁止捏造""技术栈必须验证""代码证据必须引用"。Agent 在 ReAct 循环中被迫遵守。

**Assessment:** 新版的约束设计已经对标 CodeViewX，但实际效果取决于 LLM 的执行。需要至少 3-5 次实际 Relay run 来验证约束的有效性。

---

## Quantitative Scoring

评分标准：每个维度 0-10 分。

| Dimension | Weight | Old | New | CodeViewX | Notes |
|-----------|--------|-----|-----|-----------|-------|
| Tech Stack Detail | 10% | 1 | 8 | 9 | New closed most gap |
| Core Concepts | 8% | 0 | 9 | 5 | New invented concept table |
| Module Depth | 12% | 3 | 8 | 7 | New's 4-column table is dense |
| Flow Perspective | 15% | 0 | 7 | 9 | CVX deeper, New more navigable |
| Code Citations | 15% | 0 | 2 | 10 | Still the biggest gap |
| Deployment Info | 8% | 0 | 5 | 8 | New added quick table |
| Specs Integration | 15% | 9 | 9 | 2 | AutoForge's core advantage |
| Accuracy Assurance | 10% | 3 | 6 | 8 | New designed constraints, not yet validated |
| Navigation Guide | 7% | 5 | 8 | 6 | New added flows + module.ad steps |
| **Weighted Total** | **100%** | **2.2** | **6.9** | **7.2** | |

### Interpretation

- **Old Overview (2.2/10):** 严重缺乏技术细节、流程视角、代码引用。它假设读者已经熟悉项目。
- **New Overview (6.9/10):** 大幅改进了技术栈、核心概念、模块深度、流程速览。主要差距仍在**代码引用精度**（需要实际运行 Architect 来验证）。
- **CodeViewX (7.2/10):** 在分析深度和代码引用上最强，但**与 live specs 系统完全脱节**，无法持续维护。

---

## Key Findings

### 1. New Overview is a massive improvement over Old
> 从 2.2 → 6.9，提升了 **+4.7 分**。新增的技术栈表格、核心概念表、流程速览、部署信息都是旧版完全缺失的。

### 2. CodeViewX still leads on depth, but not on integration
> CodeViewX 的 04-core-mechanisms.md（1239 行）提供了无法比拟的分析深度。但它是一个**外部快照**，不能替代 living specs。

### 3. The remaining gap is "Code Evidence"
> 新版 specs 系统缺少**精确的代码行号引用**。这是 CodeViewX 的核心优势，也是最难复制的——它需要：
> - 静态代码分析工具（AST 解析、符号定位）
> - 或 LLM agent 在 ReAct 循环中反复验证文件路径
>
> 建议后续引入 `tree-sitter` 或 `ripgrep` 后端工具，让 Architect 可以直接获取精确的代码位置。

### 4. Quickstart Guide is a missing piece
> CodeViewX 的 `02-quickstart.md`（689 行）是 AutoForge 完全没有的文档类型。建议新增：
> - `specs/auto-forge/quickstart.ad` 或
> - 由 Documenter profession 生成的 `docs/QUICKSTART.md`

---

## Recommendations

### P0 (High Impact, Low Effort)
1. **将 `overview_v2.ad` 转正** — 替换现有的 `overview.ad`，删除 `_v2` 后缀
2. **运行一次实际的 Architect Relay** — 验证新的 soul 约束是否真的能产出更准确的 overview

### P1 (Medium Impact)
3. **新增 `quickstart.ad`** — 安装、配置、运行、验证步骤
4. **为 `flows.ad` 添加代码引用** — 在每个流程的关键步骤中引用实际代码位置（需要读取源码验证）

### P2 (Strategic)
5. **后端新增 `analyze_codebase` 工具** — 提供精确的 struct/fn/line 信息，让 Architect 能够像 CodeViewX 一样引用代码
6. **定期用 CodeViewX 做外部审计** — 每季度运行一次，对比 drift，发现 specs 中遗漏的架构变化

---

## Appendix: Document Size Comparison

| Document | Lines | Words | Purpose |
|----------|-------|-------|---------|
| `overview.ad` (Old) | 71 | ~450 | Navigational entry point |
| `overview_v2.ad` (New) | ~280 | ~1,800 | Enhanced navigational entry point |
| `cvx/01-overview.md` | 625 | ~6,000 | Comprehensive project overview |
| `cvx/03-architecture.md` | 770 | ~8,000 | Deep architectural analysis |
| `cvx/04-core-mechanisms.md` | 1,239 | ~12,000 | Implementation-deep workflow analysis |
| `specs/auto-forge/flows.ad` | ~430 | ~3,500 | Cross-module execution flows |

**Note:** New overview is intentionally shorter than CodeViewX. It serves as a **navigational hub** (跳转点) rather than a comprehensive analysis. The detailed analysis lives in module specs + flows.ad.
