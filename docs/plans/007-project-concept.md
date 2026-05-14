# Plan 253: AutoForge Project Concept Implementation Plan

## Context

AutoForge 当前没有"工程"概念——后端自动从 `docs/specs/` 加载 specs，前端硬编码项目名为 `'auto-lang'`。需要引入类似 VS Code 的"打开文件夹"模型：启动时无数据，用户选择工程目录后加载 specs，并记住上次打开的工程。同时将 `docs/specs/` 提升为顶层 `specs/` 目录。

## 目录变更

```
auto-forge/           (auto-forge 工程目录，用户 Open Folder 选择此目录)
├── specs/            ← 从 docs/specs/ 移出
│   ├── manifest.at
│   ├── goals.ad
│   └── ...
├── docs/             ← 只保留 design/ 和 plans/
├── backend/
└── frontend/
```

## Phase 1: Backend — Project 模块与 API

### 1.1 新建 `backend/src/forge/project.rs`

- **ProjectConfig** 持久化到 `dirs::data_local_dir()/autoforge/config.json`：
  - `last_project_path: Option<String>`
  - `recent_projects: Vec<RecentProject>`（最多 10 个，去重）
- **find_specs_dir(project_path)**: 按顺序检查 `specs/` → `docs/specs/` → 自动创建 `specs/`
- **browse_directory(path)**: 列出子目录供前端浏览

### 1.2 修改 `backend/src/forge/mod.rs`

- SpecsStore::new_default() — 创建空 store（等待 open_project）
- SpecsStore::open_project(&mut self, project_path) — 清空 projects，设置 data_dir，load_all()
- SpecsStore::close_project() — 清空数据
- SpecsStore::is_project_open() — 检查 data_dir 是否非空
- 修改 specs() 单例：初始调用 new_default()
- start_periodic_reload() 增加 is_project_open() 检查

### 1.3 新增 API 端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/forge/project/status` | 当前工程状态 |
| POST | `/api/forge/project/open` | 打开工程 `{ "path": "..." }` |
| POST | `/api/forge/project/close` | 关闭工程 |
| GET | `/api/forge/project/recent` | 最近打开的工程列表 |
| GET | `/api/forge/project/browse?path=...` | 浏览目录 |

### 1.4 关键修改文件

- `backend/src/forge/mod.rs` — SpecsStore 重构 + 新路由 + 新 handler
- `backend/src/forge/project.rs` — 新建
- `backend/src/main.rs` — 移除 start_periodic_reload 中的自动加载依赖（保持不变，reload 会自动检查 is_project_open）

## Phase 2: Frontend — Project 管理 UI

### 2.1 新建 `frontend/src/composables/useProject.ts`

- 单例状态：projectInfo, recentProjects, isLoading
- 方法：fetchStatus, openProject, closeProject, fetchRecentProjects, browseDirectory
- 计算属性：isOpen, projectName, projectPath

### 2.2 新建 `frontend/src/views/WelcomeView.vue`

启动时无工程打开的欢迎页：
- 居中卡片：AutoForge logo + 标语
- "Open Folder" 按钮（路径输入框 + 浏览按钮）
- 最近工程列表（点击可重新打开）

### 2.3 修改 `frontend/src/App.vue`

- 引入 useProject，onMounted 调用 fetchStatus() 恢复上次工程
- 无工程时显示 WelcomeView
- 有工程时显示正常的 Chat/Specs/Agents 视图
- 导航栏增加工程指示器（工程名 + 关闭按钮）

### 2.4 修改 `frontend/src/views/SpecsView.vue`

- 行 152: `const project = ref('auto-lang')` → 从 useProject() 获取动态 project name
- 增加 watch：project 变化时重新加载 specs

### 2.5 修改 `frontend/src/views/ChatsView.vue`

- 新建 session 时传入当前 projectPath

### 2.6 关键修改文件

- `frontend/src/composables/useProject.ts` — 新建
- `frontend/src/views/WelcomeView.vue` — 新建
- `frontend/src/App.vue` — 条件渲染 + 工程指示器
- `frontend/src/views/SpecsView.vue` — 动态 project
- `frontend/src/views/ChatsView.vue` — 传入 projectPath

## Phase 3: 目录重组

- `git mv docs/specs specs`（auto-forge 仓库自身）
- 后端 find_specs_dir 同时支持 `specs/` 和 `docs/specs/`（兼容旧项目）

## Verification

1. 启动后端和前端
2. 验证初始状态显示 WelcomeView
3. 输入 `d:/autostack/auto-forge` 路径打开工程
4. 验证 Specs 页面加载 specs/ 数据
5. 刷新页面，验证自动恢复上次工程
6. 关闭工程，验证回到 WelcomeView
7. 浏览目录功能正常
