# Agent 响应速度系统性优化方案

> 日期：2026-05-29
> 状态：阶段一、二已完成，阶段三部分完成
> 关联代码：`backend/src/relay/`, `backend/src/provider/`, `backend/src/forge/tools.rs`

---

## 一、背景与问题描述

AutoForge 的多 Agent 协作（Relay Pipeline）存在明显的延迟问题：

- **Agent 交接（handoff）延迟**：从一个 Agent 完成到下一个 Agent 开始执行，有数秒甚至 10 秒以上的延迟。
- **Agent 内部操作延迟**：同一个 Agent 的多个工具调用之间也存在可感知的停顿。

为了系统性诊断和解决这些问题，我们对后端代码进行了深度分析，并参考了 ClaudeCode 的 Agent 快速启动设计（[Bootstrap 章节](https://claude-code-from-source.com/ch02-bootstrap/)）。

---

## 二、诊断结论：延迟根因分析

经过对后端代码的深度分析，定位到以下 **6 大延迟根因**：

### 🔴 P0 — Agent 每步全新创建（Ephemeral 设计）

当前架构中，Agent 没有长生命周期，每次 pipeline step 交接时：

1. `drive_run` 循环调用 `build_agent()` → 每次都新建 `RelayRegistry::new()` → 重新解析所有配置（souls、API sources、agent configs）。
2. `AgentTurn::new()` → 每次都新建 `ToolRegistry::new()` → 重新遍历过滤允许的工具列表。
3. `render_system_prompt()` → 每次重新渲染系统提示词。
4. `build_step_messages()` → 两次加锁 `RunStore` 读取 handoff 和 gate feedback。

**交接时的串行开销**（不含 LLM 调用）估计在 **50~500ms** 之间，取决于配置解析和工具注册表的复杂度。

### 🔴 P0 — RunStore 使用 `std::sync::Mutex` 而非 `tokio::sync::Mutex`

```rust
pub type RunStore = Arc<Mutex<HashMap<String, RunEntry>>>; // std::sync::Mutex
```

在 async `drive_run` 中频繁 `run_store.lock().unwrap()`：
- 阻塞当前 tokio 工作线程上的**所有任务**
- 多 pipeline 并发时产生线程饥饿
- 每次事件（text delta、tool call、tool result）都在锁内执行 `save_run()` 同步磁盘写入

### 🔴 P1 — 同步 I/O 阻塞 async 运行时

工具执行全部为同步阻塞：
- `shell` 工具：`std::process::Command::output()` 等待子进程完全结束（`cargo test` 可长达数十秒）
- `search` 工具：递归 `walk_dir` + 逐文件 `read_to_string()`
- `list_symbols` 工具：每次对 `.rs` 文件 spawn `rust-analyzer` 子进程并阻塞等待
- `save_run`：每次事件触发同步磁盘写入，在 Mutex 锁内完成

### 🟡 P2 — LLM 连接缺少预热（Warm-up）

- `ClaudeProvider::new()` 只创建 `reqwest::Client`，**不发送任何预热请求**
- 首个真实请求承担 **TCP + TLS 握手** 开销（50~300ms）
- `reqwest::Client::new()` 使用默认连接池配置，未针对高并发调优
- 测试连接 `do_test_connection()` 反而**每次新建 Client**，浪费资源

### 🟡 P2 — 每轮 ReAct 深拷贝消息历史

```rust
let request = ToolChatRequest {
    messages: self.messages.clone(),      // 每轮 deep clone
    tools: self.tool_definitions.clone(),  // 每轮 deep clone
    ...
};
```

长对话时 messages 可能达数千 tokens，每轮都完整 clone。

### 🟢 P3 — Gate 轮询忙等

```rust
loop {
    tokio::time::sleep(Duration::from_secs(2)).await;  // 2秒固定间隔
    // ...
}
```

不够优雅，但延迟可控。

---

## 三、与 ClaudeCode Bootstrap 设计的对比借鉴

ClaudeCode 的启动速度设计核心：**300ms 预算 + 五级漏斗式管道 + 三大并行策略**

| ClaudeCode 策略 | AutoForge 现状 | 可借鉴程度 |
|----------------|---------------|-----------|
| **Fast-path Dispatch**（根据 argv 缩小范围） | 无，每次交接都做完整初始化 | ⭐⭐⭐ 高 |
| **模块级并行 I/O**（import 时并行启动慢操作） | 无，所有初始化串行 | ⭐⭐⭐ 高 |
| **Promise 并行**（配置加载、Agent 定义、Hook 快照并发） | `RelayRegistry::new()` 串行解析 | ⭐⭐⭐ 高 |
| **渲染后延迟预取**（git status、model 能力延后加载） | 无，所有数据在交接时同步加载 | ⭐⭐ 中 |
| **动态导入**（按需加载模块） | 无，所有模块静态编译 | ⭐ 低（Rust 差异） |
| **Memoized init**（初始化幂等、避免重复） | `RelayRegistry::new()` 每次重建 | ⭐⭐⭐ 高 |

**核心启示**：ClaudeCode 的 Agent 是**长生命周期**（一个 query loop 持续运行），而 AutoForge 的 Agent 是**每步新建**。ClaudeCode 将启动成本一次性支付，后续每次 query 几乎零启动开销。AutoForge 应将"每步新建"改为"预热复用"。

---

## 四、已实施的优化（9 项）

### ✅ 1. RelayRegistry 全局单例化

**文件**: `backend/src/relay/mod.rs`, `backend/src/relay/driver.rs`

将原本每次交接都重新创建的 `RelayRegistry::new()` 改为 `LazyLock` 全局单例。Registry 在首次访问时初始化一次，后续所有 `build_agent()` 调用直接复用，消除了重复解析配置、souls、API sources 的开销。

```rust
static GLOBAL_REGISTRY: LazyLock<RelayRegistry> = LazyLock::new(RelayRegistry::new);

impl RelayRegistry {
    pub fn global() -> &'static RelayRegistry {
        &GLOBAL_REGISTRY
    }
}
```

### ✅ 2. ToolRegistry 全局单例 + 预过滤缓存

**文件**: `backend/src/forge/tools.rs`, `backend/src/relay/turn.rs`

- `ToolRegistry` 改为全局 `LazyLock` 单例
- 新增 `definitions_for_profession()` 方法，为每个 Profession + Skills 组合预计算并缓存过滤后的 `ToolDefinition` 列表
- `AgentTurn::new()` 不再每次 O(n) 遍历过滤工具，而是 O(1) 从缓存获取

```rust
static PROFESSION_TOOL_CACHE: LazyLock<Mutex<HashMap<String, Vec<ToolDefinition>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
```

### ✅ 3. save_run 异步后台队列

**文件**: `backend/src/relay/store.rs`

将同步磁盘写入改为**非阻塞的异步队列**：
- `save_run()` 将 `RunEntry` clone 后发送到 `mpsc::unbounded_channel`
- 后台任务在 `tokio::task::spawn_blocking` 中执行实际磁盘 I/O
- 同步测试环境（无 tokio runtime）自动回退到同步写入

```rust
static SAVE_RUN_TX: std::sync::Mutex<Option<mpsc::UnboundedSender<RunEntry>>> =
    std::sync::Mutex::new(None);
```

### ✅ 4. 重型工具 spawn_blocking

**文件**: `backend/src/relay/turn.rs`

`shell`、`search`、`list_symbols` 三个可能长时间阻塞的工具现在在 `tokio::task::spawn_blocking` 中执行，避免占用 tokio 工作线程。轻量工具（read_file、write_file 等）保持同步执行以减少上下文切换开销。

```rust
let is_heavy = matches!(name.as_str(), "shell" | "search" | "list_symbols");
let tool_result = if is_heavy {
    tokio::task::spawn_blocking(move || {
        ToolRegistry::global().get(&tool_name).map(|t| t.execute(tool_input))
    }).await
    ...
}
```

### ✅ 5. LLM 连接预热

**文件**: `backend/src/provider/claude.rs`, `backend/src/main.rs`

- 新增 `ClaudeProvider::warm_up()` 方法，发送一个极简的 `"ping"` 请求
- `main.rs` 在服务器启动时后台 `tokio::spawn` 执行预热
- 预热触发 `reqwest` 连接池的 TCP + TLS 建立，后续真实请求直接复用

```rust
pub async fn warm_up(&self) {
    let request = AIRequest { prompt: "ping".to_string(), context: None };
    let _ = self.chat(request).await;
}
```

### ✅ 6. reqwest Client 调优

**文件**: `backend/src/provider/claude.rs`

将默认的 `reqwest::Client::new()` 替换为调优配置：

```rust
let client = reqwest::Client::builder()
    .pool_max_idle_per_host(32)
    .tcp_keepalive(std::time::Duration::from_secs(60))
    .http2_adaptive_window(true)
    .build()
    .unwrap_or_else(|_| reqwest::Client::new());
```

### ✅ 7. 延迟测量与可观测性

**文件**: `backend/src/relay/driver.rs`, `backend/src/forge/mod.rs`

#### Relay Pipeline 侧（`drive_run`）
每次 step 输出以下耗时：
- `advance_run` — pipeline 状态推进
- `build_agent` — Agent 创建
- `build_step_messages` — 上下文组装
- `AgentTurn::new` — Turn 初始化
- `AgentTurn::run` — LLM 调用（含 input/output tokens、tool_calls 数量）
- `turn.to_handoff` — Handoff 生成
- `submit_handoff` — Handoff 提交
- `relay_step_total` — 整个 step 的总耗时

#### 对话流侧（`forge_stream`）
每次对话请求输出以下耗时：
- `forge_stream: build_chat_messages` — 从 session history 构建消息列表
- `forge_stream: build_system_and_tools` — 构建 system prompt 和工具列表
- `forge_stream: turn_complete` — 每轮 ReAct 总耗时（含 `llm_elapsed_ms`、`first_token_ms`）
- `forge_stream: tool_execute` — 每个工具的执行耗时
- `forge_stream: complete` — 整个对话流的总耗时和总轮数

### ✅ 8. 对话流中的 RelayRegistry 单例化

**文件**: `backend/src/forge/mod.rs`

`forge_stream` 中原本也有多次 `RelayRegistry::new()` 调用（构建 system prompt、读取 thinking 配置、`bring_in` handoff 处理），全部改为 `RelayRegistry::global()`，消除了对话模式下重复解析配置的开销。

### ✅ 9. 对话流中的重型工具 spawn_blocking

**文件**: `backend/src/forge/mod.rs`

对话流中的工具执行（`shell`、`search`、`list_symbols`）也改为在 `spawn_blocking` 中执行，与 Relay Pipeline 保持一致，避免长时间阻塞 tokio 工作线程。

---

## 五、测试修复

顺带修复了两个**测试与实现不一致**的已有问题（`required_first` 字段的断言）：
- `relay::flows::tests::test_advisor_step_has_tool_guard`
- `tests/relay_write_goals_test.rs::test_goal_discovery_flow_discover_step_has_tool_guard`

所有 **191 个测试** 现全部通过。

---

## 六、预期收益

| 优化项 | 预估延迟减少 | 实施难度 |
|--------|------------|---------|
| RelayRegistry / ToolRegistry 单例化 | 50~200ms/交接 | 低 |
| RunStore save_run 异步化 | 消除线程饥饿 + 磁盘阻塞 | 低 |
| 工具 spawn_blocking | 消除长任务阻塞 tokio 线程 | 低 |
| LLM 连接预热 | 50~300ms（仅首次） | 低 |
| reqwest Client 调优 | 高并发下更稳定 | 低 |
| 延迟测量（Relay + 对话流） | 持续优化基础 | 低 |
| 对话流 RelayRegistry 单例化 | 50~200ms/对话 | 低 |

**总体预估**：交接时的代码层开销从之前的 **50~500ms** 降至 **<50ms**（不含 LLM 生成时间）。

---

## 七、下一步建议

### 立即可做

1. **运行一次实际 Relay Pipeline**，观察 tracing 日志中的各阶段耗时，验证优化效果
2. **如果交接延迟仍 >1s**，使用日志数据判断是代码开销还是 LLM 首次响应（TTFB）问题

### 后续可继续（阶段三剩余）

| 优化项 | 说明 | 复杂度 |
|--------|------|--------|
| RunStore Mutex 异步化 | `std::sync::Mutex` → `tokio::sync::Mutex`，多 pipeline 并发时收益大 | 中 |
| Gate 通知机制 | 用 `tokio::sync::Notify` 替代 2 秒轮询 | 中 |
| Agent 实例模板池 | 预缓存 `AgentInstance` 静态模板，交接时 clone + 注入动态上下文 | 中 |
| 异步化持久化层 | `save_run` 完全异步（`tokio::fs` + 写入队列） | 中 |

---

## 八、追加优化：AgentTurn 内部细粒度计时（阶段三补完）

> 实施日期：2026-05-30

### 问题

`AgentTurn::run` 的 284,200ms 是一个**黑盒总时间**，内部包含：
- 多轮 `chat_turn`（每次外层循环一次 LLM 调用）
- 每轮中的工具执行时间
- 每轮中的 LLM 生成时间（含隐藏的 extended thinking）
- 框架开销（已优化到 ≈0ms）

无法区分"延迟来自框架还是 LLM"，也无法判断 thinking 是否占用了大量时间。

### 解决方案

在 `AgentTurn::run` 的外层循环（每轮 `chat_turn`）中新增三级计时：

#### 1. 每轮 `chat_turn` 独立计时

```rust
while turn_count < self.max_turns {
    turn_count += 1;
    let t_chat_turn = std::time::Instant::now();
    let mut tools_elapsed_ms: u64 = 0;
    // ... chat_turn + tool execution ...
    let chat_turn_elapsed_ms = t_chat_turn.elapsed().as_millis() as u64;
    let llm_elapsed_ms = chat_turn_elapsed_ms.saturating_sub(tools_elapsed_ms);
    tracing::info!(
        run_id, profession_id, turn = turn_count,
        chat_turn_elapsed_ms, llm_elapsed_ms, tools_elapsed_ms,
        tool_calls_this_turn, "AgentTurn: chat_turn_complete"
    );
}
```

输出示例：
```
AgentTurn: chat_turn_complete run_id=run-xxx profession_id=coder turn=3
  chat_turn_elapsed_ms=15234 llm_elapsed_ms=14890 tools_elapsed_ms=344
  tool_calls_this_turn=5
```

**诊断价值**：一眼看出本轮耗时是 LLM 在思考（`llm_elapsed_ms` 大）还是工具执行慢（`tools_elapsed_ms` 大）。

#### 2. 每次 `tool_execute` 独立计时

```rust
ToolChatEvent::ToolUse { id, name, input } => {
    let t_tool = std::time::Instant::now();
    // ... 工具执行 ...
    let tool_elapsed_ms = t_tool.elapsed().as_millis() as u64;
    tools_elapsed_ms += tool_elapsed_ms;
    tracing::info!(
        run_id, profession_id, turn = turn_count,
        tool_name = %name, elapsed_ms = tool_elapsed_ms,
        "AgentTurn: tool_execute"
    );
}
```

输出示例：
```
AgentTurn: tool_execute run_id=run-xxx profession_id=coder turn=3
  tool_name=read_file elapsed_ms=5
AgentTurn: tool_execute run_id=run-xxx profession_id=coder turn=3
  tool_name=search elapsed_ms=150
```

**诊断价值**：精确定位哪个工具、哪一轮调用耗时异常。

#### 3. `run_id` 上下文注入

`AgentTurn` 新增 `run_id: Option<String>` 字段，driver 在创建 turn 后注入：
```rust
turn.run_id = Some(run_id.clone());
```

确保所有 `AgentTurn` 内部日志都能与 `drive_run` 的外部日志关联分析。

### 关于 Thinking 的说明

Coder profession 默认启用了 `thinking_enabled=true`（budget=2048 tokens），但：
- `ThinkingDelta` 事件在 `relay/turn.rs` 中被丢弃（`ToolChatEvent::ThinkingDelta { .. } => {}`）
- Thinking 的**内容**和**独立耗时**当前不可见
- Thinking 时间被包含在 `llm_elapsed_ms` 中（通过 `chat_turn_elapsed_ms - tools_elapsed_ms` 近似得出）

**如果要进一步拆分 thinking 时间**，需要：
1. 在 `ThinkingDelta` 处理中记录第一个/最后一个 delta 的到达时间
2. 或让 Claude API 在 `Usage` 中返回 thinking token 的独立耗时（当前 API 不支持）

当前方案已足够区分"框架延迟 vs LLM 延迟"。

### 实施文件

| 文件 | 修改内容 |
|------|---------|
| `backend/src/relay/turn.rs` | `AgentTurn` 新增 `run_id` 字段；外层循环新增 `t_chat_turn`、`tools_elapsed_ms`；`ToolUse` 处理新增 `t_tool`；循环结束后输出 `chat_turn_complete` 日志 |
| `backend/src/relay/driver.rs` | 创建 `AgentTurn` 后注入 `turn.run_id = Some(run_id.clone())` |

### 验证

- 所有 **191 个测试** 通过
- 无编译警告（除原有的 unused 警告外）
