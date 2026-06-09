#!/usr/bin/env python3
"""
Generate project-index.ad — a human-readable code map for AutoForge.

Usage:
    cd D:/autostack/auto-forge
    python scripts/generate_project_index.py

Output: wiki/project-index.ad
"""

import os
import re
from pathlib import Path

PROJECT_ROOT = Path("D:/autostack/auto-forge")
OUT_PATH = PROJECT_ROOT / "wiki" / "project-index.ad"

# ---------------------------------------------------------------------------
#  Filename → friendly description heuristics
# ---------------------------------------------------------------------------
FRONTEND_VIEW_MAP = {
    "ChatsView.vue": "聊天页面（会话列表、消息展示、侧边栏）",
    "ExplorerView.vue": "文件浏览器页面",
    "SpecsView.vue": "规格/规范文档页面",
    "WikiView.vue": "知识库页面",
    "AgentsConfigView.vue": "智能体配置页面",
    "RelayView.vue": "流水线运行页面",
    "LoginView.vue": "登录页面",
}

FRONTEND_COMPOSABLE_MAP = {
    "useForge.ts": "Forge 会话管理（createSession, sendMessage, loadHistory）",
    "useAuth.ts": "认证与 authFetch（JWT token, login/logout）",
    "useRelay.ts": "流水线运行管理（loadRuns, startRun, gateDecision）",
    "useSpecs.ts": "规格文档加载与保存",
    "useWiki.ts": "知识库页面加载与保存",
    "useProject.ts": "项目打开/关闭/浏览",
    "useSessions.ts": "会话列表管理",
    "useProfessions.ts": "职业/角色配置管理",
    "useSkills.ts": "技能配置管理",
    "useAgentConfigs.ts": "智能体配置管理",
    "useApiSources.ts": "API 来源配置管理",
    "useItemRelations.ts": "规格项目关系管理",
}

BACKEND_MODULE_MAP = {
    "forge": "Forge Chat 核心（session, message, stream, tools）",
    "relay": "Relay 流水线（profession, agent, run, gate, handoff）",
    "rbac": "RBAC 权限系统（JWT, user, middleware）",
    "provider": "AI Provider 抽象（Claude, OpenAI, local）",
}

# ---------------------------------------------------------------------------

def describe_frontend_file(rel: Path) -> str:
    name = rel.name
    parts = rel.parts

    if "i18n/locales" in str(rel).replace("\\", "/"):
        if name == "zh.json":
            return "中文翻译配置文件"
        if name == "en.json":
            return "英文翻译配置文件"
        return f"{name} 翻译文件"

    if "views" in parts:
        return FRONTEND_VIEW_MAP.get(name, f"{name} 页面")

    if "composables" in parts:
        return FRONTEND_COMPOSABLE_MAP.get(name, f"{name} 逻辑封装")

    if "components" in parts:
        stem = name.replace(".vue", "").replace(".ts", "")
        return f"{stem} 组件"

    if "types" in parts:
        return f"{name} 类型定义"

    if name == "main.ts":
        return "应用入口（Vue app 创建、路由、i18n 初始化）"

    if name == "router.ts" or name == "router":
        return "前端路由配置"

    if name == "App.vue":
        return "根组件"

    return ""


def describe_backend_file(rel: Path) -> str:
    name = rel.name
    parts = rel.parts
    stem = name.replace(".rs", "")

    # Top-level module files
    for mod, desc in BACKEND_MODULE_MAP.items():
        if mod in parts:
            # Specific file inside module
            if stem == "mod":
                return f"{mod}/mod.rs — {desc}"
            if "profession" in stem.lower():
                return f"{mod}/{name} — 职业/角色定义与管理"
            if "agent" in stem.lower():
                return f"{mod}/{name} — Agent 实例与运行逻辑"
            if "soul" in stem.lower():
                return f"{mod}/{name} — Soul 配置解析"
            if "tool" in stem.lower():
                return f"{mod}/{name} — 文件工具实现（search, edit_file, read_file）"
            if "handler" in stem.lower():
                return f"{mod}/{name} — HTTP API 处理器"
            return f"{mod}/{name} — {desc}"

    if name == "main.rs":
        return "程序入口（路由注册、服务启动、Vite 代理）"

    if "lib.rs" in name:
        return "库入口"

    return ""


def scan_dir(root: Path, rel_prefix: str, max_depth: int = 4):
    """Yield (relative_path, description) tuples."""
    src = root / rel_prefix
    if not src.exists():
        return

    for path in sorted(src.rglob("*")):
        rel = path.relative_to(PROJECT_ROOT)
        depth = len(rel.parts) - len(Path(rel_prefix).parts)
        if depth > max_depth:
            continue
        if path.is_dir():
            continue
        if path.name.startswith("."):
            continue
        # Skip test files, __tests__, node_modules, target, etc.
        skip_patterns = ["__tests__", "node_modules", "target", "dist", ".git", "specs/"]
        if any(p in str(rel).replace("\\", "/") for p in skip_patterns):
            continue
        # Only include source files
        if path.suffix not in {".vue", ".ts", ".tsx", ".rs", ".json", ".ad", ".md"}:
            continue
        yield rel


def build_index() -> str:
    lines = []
    lines.append("= AutoForge 工程文件索引")
    lines.append("")
    lines.append(":toc: left")
    lines.append(":toclevels: 2")
    lines.append("")
    lines.append("== 说明")
    lines.append("")
    lines.append("本文档由 `scripts/generate_project_index.py` 自动生成，")
    lines.append("列出项目核心源文件及其功能，便于快速反向定位。")
    lines.append("如需更新，运行：`python scripts/generate_project_index.py`")
    lines.append("")

    # ----------------- Frontend -----------------
    lines.append("== 前端 (frontend/src)")
    lines.append("")
    lines.append("|===")
    lines.append("| 功能 | 文件路径")

    for rel in scan_dir(PROJECT_ROOT, "frontend/src", max_depth=3):
        desc = describe_frontend_file(rel)
        if desc:
            lines.append(f"| {desc} | `{rel}`")

    lines.append("|===")
    lines.append("")

    # ----------------- Backend -----------------
    lines.append("== 后端 (backend/src)")
    lines.append("")
    lines.append("|===")
    lines.append("| 功能 | 文件路径")

    for rel in scan_dir(PROJECT_ROOT, "backend/src", max_depth=3):
        desc = describe_backend_file(rel)
        if desc:
            lines.append(f"| {desc} | `{rel}`")

    lines.append("|===")
    lines.append("")

    # ----------------- i18n -----------------
    lines.append("== 国际化配置")
    lines.append("")
    lines.append("|===")
    lines.append("| 语言 | 文件路径")
    lines.append("| 中文 | `frontend/src/i18n/locales/zh.json`")
    lines.append("| 英文 | `frontend/src/i18n/locales/en.json`")
    lines.append("|===")
    lines.append("")

    # ----------------- Wiki / Specs -----------------
    lines.append("== Wiki 与 Specs")
    lines.append("")
    lines.append("|===")
    lines.append("| 内容 | 文件路径")
    lines.append("| 工程文件索引（本文档） | `wiki/project-index.ad`")
    lines.append("| Forge 模块规格 | `specs/auto-forge/forge-module.ad`")
    lines.append("| Relay 模块规格 | `specs/auto-forge/relay-module.ad`")
    lines.append("|===")
    lines.append("")

    return "\n".join(lines)


def main():
    content = build_index()
    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT_PATH, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Generated: {OUT_PATH}")
    # Count lines
    line_count = content.count("\n") + 1
    print(f"Lines: {line_count}")


if __name__ == "__main__":
    main()
