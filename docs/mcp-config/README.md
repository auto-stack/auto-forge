# AutoForge MCP Server 配置指南

AutoForge 后端（端口 3031）内置了 MCP Server，通过 **Streamable HTTP** 协议暴露。

## 快速开始

1. 启动 AutoForge 后端：`cargo run`（端口 3031）
2. MCP endpoint：`http://127.0.0.1:3031/mcp`
3. 配置你的 MCP 客户端（见下方）

## 可用能力

- **22 个 Tools**：项目管理、聊天会话、Relay Pipeline、Specs、文件系统、配置管理、批量操作
- **Annotations**：所有工具标注了 `readOnly` / `destructive` / `idempotent`，客户端会据此提示确认

## 客户端配置

### Claude Desktop

配置文件位置：
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "autoforge": {
      "url": "http://127.0.0.1:3031/mcp"
    }
  }
}
```

重启 Claude Desktop 后，在工具栏中即可看到所有 AutoForge 工具。

### Cursor

Cursor 目前主要支持 `stdio` transport 的 MCP Servers。对于 HTTP transport，可以使用 [mcp-proxy](https://github.com/sparfenyuk/mcp-proxy) 桥接：

```bash
# 安装 mcp-proxy
pipx install mcp-proxy

# Cursor 配置 ~/.cursor/mcp.json
{
  "mcpServers": {
    "autoforge": {
      "command": "mcp-proxy",
      "args": ["http://127.0.0.1:3031/mcp"]
    }
  }
}
```

### Kimi CLI / 其他支持 HTTP MCP 的客户端

```json
{
  "mcpServers": {
    "autoforge": {
      "url": "http://127.0.0.1:3031/mcp",
      "headers": {
        "Accept": "application/json, text/event-stream"
      }
    }
  }
}
```

## 使用示例

启动 AutoForge 后端后，你可以对 AI 说：

> "列出当前项目的所有 specs 模块"

Claude 会自动调用 `forge_read_specs(project="auto-forge")` 并返回结果。

> "创建一个 advisor 会话，让它检查 chat 模块的设计文档"

Claude 会依次调用：
1. `forge_create_session()`
2. `forge_send_message(content="请检查 chat 模块的设计文档", profession_id="advisor")`

> "启动 3 个 post-discovery flow 的 runs，对比它们的完成时间"

Claude 会调用：
1. `forge_batch_start_runs(flow_id="post-discovery", count=3)`
2. （等待后）`forge_batch_get_results(run_ids=[...])`

## 工具清单

| 类别 | 工具 | 说明 |
|------|------|------|
| **项目** | `forge_get_project_status` | 项目状态 |
| | `forge_open_project` | 打开项目 |
| | `forge_close_project` | 关闭项目（destructive） |
| **聊天** | `forge_create_session` | 创建会话 |
| | `forge_send_message` | 发送消息 |
| | `forge_get_session` | 获取会话详情 |
| | `forge_list_sessions` | 列出会话 |
| | `forge_delete_session` | 删除会话（destructive） |
| **Relay** | `forge_start_relay_run` | 启动流水线 |
| | `forge_list_runs` | 列出运行 |
| | `forge_get_run` | 获取运行详情 |
| | `forge_batch_start_runs` | 批量启动 |
| | `forge_batch_get_results` | 批量获取结果 |
| **Specs** | `forge_read_specs` | 读取 Specs |
| | `forge_approve_spec` | 审批 Spec 变更 |
| | `forge_reject_spec` | 拒绝 Spec 变更（destructive） |
| **文件** | `forge_read_file` | 读取文件（只读） |
| | `forge_browse_directory` | 浏览目录（只读） |
| **配置** | `forge_list_professions` | 列出职业 |
| | `forge_list_api_sources` | 列出 API Sources |
| | `forge_test_api_connection` | 测试 API 连接 |
| **系统** | `forge_get_performance_logs` | 性能日志 |
