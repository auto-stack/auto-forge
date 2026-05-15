# 012 Wiki Multi-Level Directory + Raw Resource Import

## Context

当前 Wiki 是扁平 slug 列表，没有 raw 原始资料的概念。按 LLM Wiki 模式，需要一个 `raw/` 目录存放原始资料（PDF、文档等），`wiki/` 目录存放 Agent 加工后的知识页面。两侧都支持多层目录，左侧导航改为树状结构，Raw 区域支持拖拽上传。

## 目录结构

```
{project}/
├── wiki/
│   ├── _manifest.json
│   ├── guide/
│   │   ├── getting-started.md
│   │   └── advanced/
│   │       └── api-reference.md
│   └── readme.md
└── raw/
    ├── datasheets/
    │   ├── stm32f4-datasheet.pdf
    │   └── pinout.png
    └── meeting-notes/
        └── 2024-01-15.md
```

## 前端导航布局

单侧栏双区域：上部 Raw（树+DropZone），下部 Wiki（树+页面）。无外部依赖。

## Phase 1: Backend — Tree API + Path-Based CRUD

### 1.1 启用 axum multipart
- `backend/Cargo.toml` — 添加 `"multipart"` feature
- `backend/src/main.rs` — CORS 添加 PUT/DELETE methods

### 1.2 新增 TreeNode + 目录遍历
- `backend/src/forge/wiki.rs` — TreeNode struct + `build_tree()` 递归函数

### 1.3 路径式 Wiki CRUD
- slug 支持 `/`（如 `guide/getting-started`），写入前创建父目录
- manifest 改名为 `_manifest.json`
- 所有 handler 添加路径遍历校验

### 1.4 Tree + Catch-All 路由
- `GET /api/forge/wiki/{project}/tree`
- `GET /api/forge/raw/{project}/tree`
- Wiki page 路由改为 `page/{*slug}`

## Phase 2: Backend — Raw File API

### 2.1 文件上传
- `POST /api/forge/raw/{project}/upload?prefix=...` (multipart, 50MB limit)

### 2.2 文件服务
- `GET /api/forge/raw/{project}/file/{*path}` + MIME 猜测

### 2.3 删除 + 创建目录
- `DELETE /api/forge/raw/{project}/file/{*path}`
- `POST /api/forge/raw/{project}/mkdir`

## Phase 3: Frontend — Types + Composable

- `TreeNode` 类型 + composable 扩展（tree load, upload, raw ops）

## Phase 4: Frontend — TreeView Component

- 递归 Vue 组件，文件夹展开/折叠，文件图标映射

## Phase 5: Frontend — WikiView Overhaul + DropZone

- 双区域树（Raw + Wiki）+ DropZone 拖拽上传 + Raw 文件预览

## 关键文件

| 文件 | 操作 |
|---|---|
| `backend/Cargo.toml` | 修改（multipart feature） |
| `backend/src/main.rs` | 修改（CORS） |
| `backend/src/forge/wiki.rs` | 大改（TreeNode、tree、raw API、upload） |
| `frontend/src/types/wiki.ts` | 修改（TreeNode） |
| `frontend/src/composables/useWiki.ts` | 修改（tree/raw ops） |
| `frontend/src/components/TreeView.vue` | 新建 |
| `frontend/src/components/DropZone.vue` | 新建 |
| `frontend/src/views/WikiView.vue` | 重写 |

## 风险

1. 路径遍历 — `..` 校验 + canonicalize
2. 上传大小 — axum 默认 2MB → 50MB
3. 路由顺序 — catch-all 在 pages/search 之后
4. 向后兼容 — 现有扁平 slug 无需迁移
