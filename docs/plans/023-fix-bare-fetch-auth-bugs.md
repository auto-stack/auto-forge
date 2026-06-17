# Plan 023: 修复前端 6 处裸 fetch 漏 auth 的 bug

> **类型**：Bugfix（一致性 + 潜在功能性故障）
> **风险**：极低（纯机械替换，`fetch(` → `authFetch(`，逻辑不变）
> **预估工作量**：~1 小时
> **来源**：auto-musk 前端架构分析（auto-forge 前端技术债评估）发现

---

## 背景

前端统一用 `authFetch()`（`frontend/src/composables/useAuth.ts:158`）封装 fetch——它自动注入 `Authorization: Bearer <jwt>` 头并处理 401。但有 **6 处**调用了裸 `fetch(...)`，绕过了 auth 封装。

**危害**：
- 对需要鉴权的端点，裸 fetch **不带 token**，会被后端 `auth_middleware` 拒绝（401），导致功能静默失败。
- 其中最严重的是 `useSpecs.ts:56` 的 `saveSection`——**保存 Spec 规格会 401 失败**（Specs 是 auto-forge 的核心数据）。
- 即便某些端点当前恰好不需要 auth（或后端宽松），这也是一致性隐患，未来收紧 auth 时会批量暴雷。

## 修复原则

逐处把 `fetch(...)` 替换为 `authFetch(...)`，**保持其余参数和逻辑完全不变**。若文件尚未 import `authFetch`，补 `import { authFetch } from './useAuth'`（views 从 `@/composables/useAuth`）。

`authFetch` 签名与 `fetch` 兼容（都是 `authFetch(input, init?)`），所以是纯替换，无需改参数。但注意：`authFetch` 在 401 时会自动登出/跳登录——确认被替换的端点确实期望"401 即未授权"语义（都是 forge API，符合）。

---

## 6 处修复清单

### 1. `frontend/src/composables/useSpecs.ts:56` 【最严重】
- **函数**：`saveSection(project, section)`（PUT 保存 Spec section）
- **现状**：`const resp = await fetch(${API_BASE}/.../{project}/{section.id}, { method:'PUT', ... })`
- **影响**：**保存规格 401 失败**
- **修复**：`fetch` → `authFetch`（文件已 import authFetch，:3；同文件其它 6 处都用 authFetch，纯遗漏）

### 2. `frontend/src/composables/useItemRelations.ts:22`
- **函数**：`loadRelations(itemId)`（GET 加载 spec item 的关联）
- **现状**：`const resp = await fetch(${API_BASE}/.../related/${itemId})`
- **影响**：关联关系加载 401 失败
- **修复**：`fetch` → `authFetch`（文件已 import authFetch，:3；但仅此一处没用上）

### 3. `frontend/src/composables/useSouls.ts:15`
- **函数**：`loadSouls()`（GET 加载 Soul 列表）
- **现状**：`const resp = await fetch('/api/forge/relay/souls')`
- **影响**：Soul 列表加载 401 失败；且本函数无 loading/error 态（与其它 CRUD composable 不一致，但本次只修 auth，不改状态管理）
- **修复**：`fetch` → `authFetch`；**并补 import**：文件当前未 import authFetch，需加 `import { authFetch } from './useAuth'`

### 4. `frontend/src/views/ExplorerView.vue:165`
- **函数**：加载项目文件树
- **现状**：`const resp = await fetch('/api/forge/project/tree')`
- **影响**：文件树加载 401 失败
- **修复**：`fetch` → `authFetch`；补 import（若 view 未引入则加 `import { authFetch } from '@/composables/useAuth'`）

### 5. `frontend/src/views/WelcomeView.vue:140`
- **函数**：选择文件夹（pick-folder）
- **现状**：`const resp = await fetch('/api/forge/project/pick-folder')`
- **影响**：选文件夹 401 失败
- **修复**：`fetch` → `authFetch`；按需补 import

### 6. `frontend/src/views/WikiView.vue:369`
- **函数**：下载 raw 文件
- **现状**：`const resp = await fetch(rawFileUrl(project.value, payload.path))`
- **影响**：raw 文件下载 401 失败
- **修复**：`fetch` → `authFetch`；按需补 import
- **注意**：若此处下载的是静态文件（非 forge API，走的是不需要 auth 的静态服务），需确认 raw 文件端点是否真的需要 auth。若是纯静态资源服务，可保留 fetch 并加注释说明；若是 forge API 则必须改。请实现时核实该端点的鉴权要求。

---

## 验收

1. 全文搜索 `frontend/src/` 下 `fetch(`（排除 `authFetch`、`useAuth.ts` 自身、`.then`/`response.json` 等非调用）——应只剩 `useAuth.ts` 内 login/register/wrapper 本身的 3 处合理裸 fetch。
2. 手动验证 SpecsView 保存一个 section → 成功（不再 401）。
3. 验证 Souls 加载、Explorer 文件树、Welcome 选文件夹、Wiki 下载 raw 各功能正常。
4. `npm run build`（或 `vue-tsc --noEmit`）通过，无类型错误。

## 不在本次范围

- 不重构 composable 的状态管理（useSouls 缺 loading/error 态等一致性问题是技术债，但属于 R1 范畴，另议）。
- 不改 authFetch 本身的行为。
- 只做"裸 fetch → authFetch"的机械替换，不顺手改其它逻辑。

## 参考证据

分析报告（auto-musk 侧）：`auto-musk/plans/002-frontend-framework-plan.md` §2.2 的 R6 条目，及 auto-musk 前端架构分析会话记录。
