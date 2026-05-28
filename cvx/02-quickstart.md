# AutoForge 快速开始指南

## 文档说明

本文档提供 AutoForge 项目的详细安装、配置和运行指南。

**文档信息**
- **生成时间**: 2025-01-19
- **目标读者**: 开发者、系统管理员
- **难度级别**: 初级到中级
- **预计完成时间**: 30-45 分钟

---

## 目录

1. [前置条件](#前置条件)
2. [安装步骤](#安装步骤)
3. [配置](#配置)
4. [运行](#运行)
5. [验证安装](#验证安装)
6. [常见问题](#常见问题)
7. [下一步](#下一步)

---

## 前置条件

### 系统要求

- **操作系统**: Windows 10+, Linux, macOS
- **内存**: 至少 4 GB RAM（推荐 8 GB）
- **磁盘空间**: 至少 2 GB 可用空间
- **网络**: 需要访问 Anthropic 或 OpenAI API

### 必需软件

#### 后端开发环境

| 软件 | 版本 | 用途 | 检查命令 |
|------|------|------|----------|
| **Rust** | 1.80+ | 后端编程语言 | `rustc --version` |
| **Cargo** | 最新版 | Rust 构建工具 | `cargo --version` |

**安装 Rust**:

```bash
# Linux/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# 下载并运行 rustup-init.exe
# 访问: https://rustup.rs/
```

#### 前端开发环境

| 软件 | 版本 | 用途 | 检查命令 |
|------|------|------|----------|
| **Node.js** | 18+ | JavaScript 运行时 | `node --version` |
| **pnpm** | 8+ | 包管理器 | `pnpm --version` |

**安装 Node.js**:

```bash
# 使用 nvm (推荐)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18

# 或从官网下载
# https://nodejs.org/
```

**安装 pnpm**:

```bash
npm install -g pnpm
```

### API 密钥

#### Anthropic Claude API

1. 访问 [Anthropic Console](https://console.anthropic.com/)
2. 创建账户或登录
3. 生成 API 密钥
4. 保存密钥（格式: `sk-ant-...`）

#### OpenAI GPT API (可选)

1. 访问 [OpenAI Platform](https://platform.openai.com/)
2. 创建账户或登录
3. 生成 API 密钥
4. 保存密钥（格式: `sk-...`）

---

## 安装步骤

### 1. 克隆仓库

```bash
# 使用 Git
git clone https://github.com/auto-stack/auto-forge.git
cd auto-forge

# 或下载 ZIP 文件并解压
```

### 2. 安装后端依赖

```bash
cd backend
cargo build
```

**预期输出**:
```
   Compiling auto-forge v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in 45.2s
```

**如果编译失败**:
- 检查 Rust 版本: `rustc --version` (需要 1.80+)
- 更新 Rust: `rustup update`
- 清理并重新构建: `cargo clean && cargo build`

### 3. 安装前端依赖

```bash
cd ../frontend
pnpm install
```

**预期输出**:
```
Packages: +XXX
Progress: resolved XXX, reused XXX, downloaded XXX, added XXX
Done in 23.4s
```

**如果安装失败**:
- 检查 Node.js 版本: `node --version` (需要 18+)
- 清理缓存: `pnpm store prune`
- 删除 node_modules: `rm -rf node_modules && pnpm install`

### 4. 构建前端（可选）

```bash
pnpm run build
```

**预期输出**:
```
frontend/dist/index.html                  0.45 kB
frontend/dist/assets/index-xxx.css        12.34 kB
frontend/dist/assets/index-xxx.js         345.67 kB
```

---

## 配置

### 1. 设置 API 密钥

#### 方法 A: 环境变量（推荐）

**Linux/macOS**:
```bash
export ANTHROPIC_API_KEY="sk-ant-xxxxx"
```

**Windows (PowerShell)**:
```powershell
$env:ANTHROPIC_API_KEY="sk-ant-xxxxx"
```

**永久设置**:

**Linux/macOS** (`~/.bashrc` 或 `~/.zshrc`):
```bash
echo 'export ANTHROPIC_API_KEY="sk-ant-xxxxx"' >> ~/.bashrc
source ~/.bashrc
```

**Windows** (系统环境变量):
1. 搜索"环境变量"
2. 编辑用户环境变量
3. 新建 `ANTHROPIC_API_KEY`

#### 方法 B: 配置文件

创建或编辑 `~/.autoforge/config.toml`:

```toml
[api]
anthropic_key = "sk-ant-xxxxx"
openai_key = "sk-xxxxx"  # 可选
```

### 2. 配置模型（可选）

编辑 `backend/src/relay/agent.rs` 中的默认模型：

```rust
impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            provider: Provider::Anthropic,
            model: String::from("claude-3-5-sonnet-20241022"),
            temperature: 0.3,
            max_tokens: 8192,
            // ...
        }
    }
}
```

### 3. 配置端口（可选）

编辑 `backend/src/main.rs`:

```rust
let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3031));
// 改为其他端口，如 8080
// let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
```

---

## 运行

### 开发模式

#### 启动后端

**终端 1**:
```bash
cd backend
cargo run
```

**预期输出**:
```
AutoForge server listening on http://127.0.0.1:3031
```

**后端服务**:
- HTTP API: `http://127.0.0.1:3031/api/*`
- WebSocket: `ws://127.0.0.1:3031/api/*`

#### 启动前端（开发服务器）

**终端 2**:
```bash
cd frontend
pnpm run dev
```

**预期输出**:
```
  VITE v6.x.x  ready in 234 ms

  ➜  Local:   http://localhost:5173/
  ➜  Network: use --host to expose
```

**前端服务**:
- Web UI: `http://localhost:5173/forge`
- 热重载: 修改代码自动刷新

### 生产模式

#### 启动后端

```bash
cd backend
cargo build --release
./target/release/auto-forge
```

#### 启动前端（使用后端内置服务）

前端已构建到 `frontend/dist/`，后端会自动提供静态文件服务。

访问: `http://127.0.0.1:3031/forge`

---

## 验证安装

### 1. 检查后端 API

```bash
curl http://127.0.0.1:3031/api/forge/sessions
```

**预期输出**:
```json
[]
```

### 2. 访问 Web UI

打开浏览器访问: `http://127.0.0.1:3031/forge`

**预期界面**:
- 聊天界面（Forge）
- 规格管理（Ledger）
- Relay 监控（Relay）

### 3. 运行 Mock 测试

```bash
cd backend
python tests/forge_relay_mock.py
```

**预期输出**:
```
✓ Post-discovery flow completed
✓ Standard flow with gate completed
✓ Gate rejection and retry completed
All tests passed!
```

### 4. 测试聊天功能

在 Web UI 中发送消息:

```
你好，我是 AutoForge 的新用户。请介绍一下你的功能。
```

**预期响应**:
- Isaac（助手）回复问候
- 显示可用的工具和功能

---

## 常见问题

### 问题 1: Rust 编译失败

**错误信息**:
```
error: linker `link.exe` not found
```

**解决方案** (Windows):
1. 安装 [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
2. 或安装 Visual Studio Community（选择"C++ 构建工具"）

**解决方案** (Linux):
```bash
sudo apt install build-essential
```

### 问题 2: 前端依赖安装失败

**错误信息**:
```
ENOENT: no such file or directory, open 'package.json'
```

**解决方案**:
```bash
cd frontend  # 确保在 frontend 目录
pnpm install
```

### 问题 3: API 密钥无效

**错误信息**:
```
ApiError: InvalidApiKey
```

**解决方案**:
1. 检查环境变量: `echo $ANTHROPIC_API_KEY`
2. 确保密钥格式正确: `sk-ant-xxxxx`
3. 验证密钥在 Anthropic Console 中是否有效

### 问题 4: 端口被占用

**错误信息**:
```
Error: Os { code: 48, kind: AddrInUse, message: "Address already in use" }
```

**解决方案**:

**查找占用进程**:
```bash
# Linux/macOS
lsof -i :3031

# Windows
netstat -ano | findstr :3031
```

**终止进程或更改端口**:
```bash
# 终止进程
kill -9 <PID>

# 或更改端口（编辑 backend/src/main.rs）
let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
```

### 问题 5: CORS 错误

**错误信息** (浏览器控制台):
```
Access to fetch at 'http://127.0.0.1:3031/api/...' from origin 'http://localhost:5173' has been blocked by CORS policy
```

**解决方案**:

后端已配置 CORS，但如需自定义：

编辑 `backend/src/main.rs`:
```rust
let cors = CorsLayer::new()
    .allow_origin(tower_http::cors::Any)
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
    .allow_headers(tower_http::cors::Any);
```

### 问题 6: 前端构建失败

**错误信息**:
```
Error: Cannot find module 'vite'
```

**解决方案**:
```bash
cd frontend
rm -rf node_modules pnpm-lock.yaml
pnpm install
```

---

## 下一步

### 学习资源

1. **阅读项目概述** → [01-overview.md](./01-overview.md)
2. **理解架构设计** → [03-architecture.md](./03-architecture.md)
3. **深入核心机制** → [04-core-mechanisms.md](./04-core-mechanisms.md)

### 实践任务

#### 任务 1: 创建第一个聊天会话

1. 打开 `http://127.0.0.1:3031/forge`
2. 创建新会话
3. 发送消息: "帮我创建一个简单的计算器程序"
4. 观察 Isaac 的响应和工具调用

#### 任务 2: 运行 Relay 流程

1. 在聊天中发送: "我想构建一个缓存模块。写完规格后启动 post-discovery relay。"
2. 观察 Relay 启动
3. 在 Relay 视图中监控进度
4. 处理人工关卡（如果出现）

#### 任务 3: 编辑规格文档

1. 打开 Specs 视图
2. 查看现有规格（Goals, Architecture, Designs 等）
3. 编辑某个规格项
4. 保存并查看更新

### 开发任务

#### 添加自定义工具

1. 编辑 `backend/src/forge/tools.rs`
2. 实现新的工具 trait
3. 注册到 ToolRegistry
4. 重启后端

#### 自定义智能体灵魂

1. 编辑 `backend/src/relay/souls/`
2. 创建新的 Markdown 文件
3. 定义灵魂的价值观和行为
4. 在配置中引用

#### 创建自定义流程

1. 在项目根目录创建 `.autoforge/flows/`
2. 添加 YAML 文件定义流程
3. 重启后端
4. 通过 API 使用新流程

---

## 高级配置

### 使用本地 LLM

编辑 `backend/src/relay/agent.rs`:

```rust
pub enum Provider {
    Anthropic,
    OpenAI,
    Local { url: String },  // 添加本地模型支持
}
```

配置:
```rust
ModelConfig {
    provider: Provider::Local {
        url: "http://localhost:11434".to_string()
    },
    model: "llama2".to_string(),
    // ...
}
```

### 调整 Token 预算

编辑 `backend/src/relay/budget.rs`:

```rust
let budget = TokenBudget::new(1_000_000); // 100 万 tokens
```

### 自定义检查点位置

编辑 `backend/src/relay/checkpoint.rs`:

```rust
let checkpoint_dir = PathBuf::from("/custom/path/checkpoints");
```

---

## 性能优化

### 后端优化

1. **启用 Rust 优化**:
```bash
cargo build --release
```

2. **调整 Tokio 运行时**:
```rust
let rt = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(4)
    .enable_all()
    .build()
    .unwrap();
```

### 前端优化

1. **启用生产构建**:
```bash
pnpm run build
```

2. **配置 Vite 缓存**:
```javascript
// vite.config.ts
export default {
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor': ['vue', '@tiptap/vue-3'],
        }
      }
    }
  }
}
```

---

## 部署

### Docker 部署（未来功能）

```dockerfile
# Dockerfile (示例)
FROM rust:1.80 as builder
WORKDIR /app
COPY . .
RUN cd backend && cargo build --release

FROM node:18 as frontend
WORKDIR /app
COPY frontend/ .
RUN pnpm install && pnpm run build

FROM debian:bullseye-slim
COPY --from=builder /app/backend/target/release/auto-forge /usr/local/bin/
COPY --from=frontend /app/dist /var/www/forge
EXPOSE 3031
CMD ["auto-forge"]
```

---

## 监控和日志

### 查看后端日志

```bash
cd backend
RUST_LOG=debug cargo run
```

### 查看前端日志

浏览器控制台 (F12)

### 日志级别

编辑 `backend/src/main.rs`:
```rust
tracing_subscriber::fmt()
    .with_env_filter(
        "auto_forge=debug,tower_http=info"  // 调整级别
    )
    .init();
```

---

## 更新和维护

### 更新依赖

**后端**:
```bash
cd backend
cargo update
```

**前端**:
```bash
cd frontend
pnpm update
```

### 清理构建

```bash
# 后端
cd backend
cargo clean

# 前端
cd frontend
rm -rf node_modules dist
```

---

## 支持和社区

### 获取帮助

- **GitHub Issues**: 报告 Bug 和功能请求
- **讨论区**: 技术讨论和问题解答
- **Wiki**: 详细文档和教程

### 贡献

欢迎贡献代码、文档和测试！

1. Fork 项目
2. 创建特性分支
3. 提交 Pull Request

---

**文档生成器**: CodeViewX  
**最后更新**: 2025-01-19  
**文档版本**: 1.0.0
