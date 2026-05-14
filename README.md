# AutoForge

**Spec-driven, serial-agent AI coding assistant.**

AutoForge orchestrates specialized AI agents sequentially — Planner → Architect → Coder → Tester → Reviewer — so each agent receives only the context it needs. Specs are the single source of truth; agents coordinate through structured specifications, not chat history.

Originally part of the [Auto Language](https://github.com/auto-stack/auto-lang) project, now a standalone tool.

## Features

- **Spec-Driven Development** — 7 spec categories (Goals, Architecture, Designs, Plans, Tests, Reviews, Reports) with typed IDs (`G1`, `A1`, `D1`, `P1`, `S1.1`, `V1`, `X42`) and bidirectional traceability
- **Serial Agent Relay** — Agents hand off compressed documents instead of full chat history (~5x token savings vs parallel multi-agent)
- **Durable Execution** — Checkpoint after every handoff; resume or roll back at any time
- **Token Budgeting** — Per-step budgets with automatic compression and cost analytics
- **Human Gates** — GSD mode (Goal gate only) or Check mode (all gates) for human-in-the-loop control
- **Web UI + CLI** — Chat (Forge), Specs (Ledger), and Relay (Agents) views with real-time streaming

## Architecture

```
User Request → Forge (chat loop)
                    ↓
             Relay (pipeline engine)
                    ↓
   [Advisor] → [Architect] → [Planner] → [Coder] → [Tester] → [Reviewer]
         handoff →   handoff →    handoff →   handoff →  handoff
                    ↓
              Specs (Ledger) ← single source of truth
```

### Key Concepts

| Concept | Description |
|---------|-------------|
| **Forge** | Chat loop that classifies user intent and routes to the right agent |
| **Relay** | Pipeline engine that executes agent flows sequentially with handoffs |
| **Specs** | File-based knowledge base (`.ad` files) — the contract between agents |
| **Agent** | Has a Soul (personality), Profession (scope), and Model (LLM config) |
| **Gate** | Human approval checkpoint at key decision points |

### 8 Built-in Professions

Assistant, Advisor, Architect, Planner, Coder, Tester, Reviewer, Documenter — each with scoped tool access and owned spec sections.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, Axum 0.8, Tokio, Reqwest |
| Frontend | Vue 3, Vite, Markstream, Mermaid |
| AI Models | Claude, GPT (configurable per profession) |
| Storage | File-based (specs in `docs/specs/`, sessions in `~/.local/share/autoforge/`) |

## Getting Started

### Prerequisites

- Rust 1.80+ and Cargo
- Node.js 18+ (for frontend)
- An LLM API key (Anthropic or OpenAI)

### Build & Run

**Backend:**
```bash
cd backend
cargo build
cargo run    # Starts at http://127.0.0.1:3031
```

**Frontend:**
```bash
cd frontend
npm install
npm run dev  # Dev server with hot reload
npm run build  # Production build
```

### Access

- Web UI: `http://127.0.0.1:3031/forge`
- API: `http://127.0.0.1:3031/api/forge/*`

## Project Structure

```
auto-forge/
├── backend/            # Rust backend (Axum server, forge, relay)
│   ├── src/
│   │   ├── forge/      # Chat loop, tool definitions, spec management
│   │   └── relay/      # Agent orchestration, pipeline, checkpoints
│   └── tests/
├── frontend/           # Vue 3 frontend
│   ├── src/
│   │   ├── views/      # Chat, Specs, Agents views
│   │   ├── composables/# useForge, useLedger, useGateInbox
│   │   └── components/ # SpecItem, GatePanel, MarkdownContent...
│   └── dist/
└── docs/
    ├── design/         # Architecture and design documents
    ├── plans/          # Implementation plans
    └── specs/          # Spec templates and project data
```

## Documentation

- [Spec-Driven Forge Design](docs/design/spec-driven-forge.md) — core design philosophy
- [Agent Relay Orchestration](docs/design/agents-relay-orchestration.md) — how agents cooperate
- [Spec Categories](docs/design/spec-categories.md) — spec type system and status lifecycle
- [Spec UI & Traceability](docs/design/spec-ui-and-relations.md) — frontend spec management

## Origin

AutoForge was originally developed within the [Auto Language](https://github.com/auto-stack/auto-lang) project as its AI-assisted development toolchain. It has been extracted as a standalone project to serve any codebase, not just Auto language projects.

## License

MIT
