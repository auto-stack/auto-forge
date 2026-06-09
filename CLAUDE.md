# AutoForge — Agent Guide

## Start Services

Always start **both** backend and frontend dev server. The backend auto-detects Vite and proxies `/forge` to it.

```bash
# 1. Start Vite dev server (independent window)
cd frontend && pnpm run dev

# 2. Start backend (independent window)
cd backend && cargo run
```

**Access**: `http://127.0.0.1:3031/forge`

- If Vite (port 5174) is running → backend **proxies** `/forge` to Vite (hot-reload)
- If Vite is not running → backend serves `frontend/dist/` static files

**Windows independent windows** (prevents background-task timeout kills):
```powershell
cd frontend; Start-Process pnpm -ArgumentList "run","dev" -WindowStyle Normal
cd backend; Start-Process .\target\debug\auto-forge.exe -WindowStyle Normal
```

## Rebuild Frontend (when Vite is down)

If Vite dev server is not running, static files in `frontend/dist/` must be rebuilt:

```bash
cd frontend && pnpm run build
```

## Architecture Decisions

### Agent Roles & Permissions

| Profession | File Tools | Scope |
|-----------|-----------|-------|
| **Assistant** | `bring_in`, `dispatch`, `shell` | Route tasks, no direct file edit |
| **Gofer** | `shell`, `read_file`, `edit_file`, `search` | Gather facts + simple text replacement |
| **Coder** | `read_file`, `edit_file`, `write_file`, `search`, `shell` | Code logic changes |

**Gofer can `edit_file`** — for simple text replacements (i18n, docs, config values) that do not affect code logic. Gofer must NOT create/delete files or modify control flow.

### Search Tool (`context_lines`)

`search` supports `context_lines` (default: 2):

```json
{"pattern": "规格", "path": "frontend/src/i18n", "context_lines": 2}
```

Returns structured JSON with `context_before` / `context_after` for each match.

### edit_file Returns Structured Diff

`edit_file` returns JSON:

```json
{
  "status": "success",
  "applied": 3,
  "file": "...",
  "diffs": [
    {"line": 9, "old_string": "...", "new_string": "..."}
  ],
  "errors": []
}
```

Agents should verify `diffs` against intended changes instead of running secondary `search` validation.

### Simple Text Replacement Flow

For "replace all X with Y" tasks:

```
Assistant → dispatch Gofer(search) → get matches with context
Assistant reviews matches → decides which to replace
Assistant → dispatch Gofer(edit_file with edits array) → apply replacements
Gofer checks returned diffs → reports result
```

No Coder handoff needed for pure text replacements.

## Commit Conventions

- Do NOT add "Co-Authored-By" or similar AI attribution lines to commit messages.
