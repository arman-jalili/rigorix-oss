# Rigorix

[![Crates.io](https://img.shields.io/badge/crate-rigorix-blue)](https://crates.io/crates/rigorix)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green)]()
[![CI](https://img.shields.io/badge/CI-passing-brightgreen)]()
[![Rust](https://img.shields.io/static/v1?label=rust&message=2024%20edition&color=orange)](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)

**Template-driven DAG execution engine with bounded autonomy.**

Rigorix is a deterministic coding agent framework that compiles natural-language intents into executable Directed Acyclic Graphs (DAGs). It operates through three modes:

- **CLI** (`rigorix`) — Interactive TUI + flag-based scripting for local development
- **GitHub Action** (`rigorix-action`) — PR governance and automated code generation in CI/CD
- **Engine** — The core library powering both

---

<video src="rigorix-demo.mov" controls width="720"></video>

*🎥 Demo: Rigorix planning and executing a TypeScript refactor — reading code, generating a patch, type-checking, and running tests.*

---

## Why Rigorix?

| Dimension | Rigorix | Claude Code | Copilot / Cursor | Aider | SWE-Agent |
|-----------|---------|-------------|------------------|-------|-----------|
| **Execution model** | Template-driven DAG (bounded, deterministic) | Agent loop (stateless) | LLM-in-the-loop (stateless) | LLM-in-the-loop (file-by-file) | Agent loop (stateless) |
| **Code generation** | Structured: classify → extract → generate → validate → hash | LLM to edit files + shell | Inline completions | Diff-based patches | Shell commands |
| **Safety** | Risk gating, enforcement caps, budget tracking, permission policies | None | None | None | None |
| **PR governance** | Built-in (policy.toml: deny/review/flag) | ✗ | ✗ | ✗ | ✗ |
| **Audit** | HMAC-signed audit envelopes with circuit breaker | ✗ | ✗ | ✗ | ✗ |
| **Quality gates** | Post-execution GreenContract evaluation | ✗ | ✗ | ✗ | ✗ |
| **Self-correcting** | Validate loop (plan → execute → verify → repeat) | ✗ | ✗ | ✗ | ✗ |

Rigorix is designed for **deterministic, auditable, safely-bounded automation** — not open-ended agent loops. If you need a code assistant that chats with you, use Claude Code or Copilot. If you need a CI/CD pipeline that enforces policies and generates auditable code changes, use Rigorix.

---

## Quickstart

### Install

```bash
# From source
cargo install --git https://github.com/arman-jalili/rigorix-oss rigorix-cli

# Or build locally
git clone https://github.com/arman-jalili/rigorix-oss
cd rigorix-oss && cargo build --release -p rigorix-cli
./target/release/rigorix --help
```

### Set your API key

```bash
export RIGORIX__LLM__API_KEY="sk-ant-..."   # Anthropic
# or: export ANTHROPIC_API_KEY="sk-ant-..."
```

### Initialize a project

```bash
cd my-project
rigorix init
```

### Run your first task

```bash
rigorix run "Explain how the main module works"
```

Rigorix will: classify the intent → extract parameters → generate a DAG → execute nodes (file reads, edits, bash commands) → validate results.

### Plan before running (recommended)

```bash
rigorix plan "Add a new endpoint to the API"   # Review the DAG, then:
rigorix run "Add a new endpoint to the API"
```

Or just plan and confirm in one flow:

```bash
rigorix plan "Add error handling to the parser"
# Shows plan, then prompts:
# Run this plan now? [y/N]: y
```

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                   User (Developer)                           │
│            (CLI / TUI / GitHub Action)                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                     Planning Phase                           │
│                                                              │
│  Intent → Classify → Extract → Generate TaskGraph → Validate │
│                  ↕ (low-confidence fallback)                 │
│        Template System + LLM Template Generator              │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                     Execution Phase                          │
│                                                              │
│  DAG Engine (topo sort) → ParallelExecutor (tokio JoinSet)   │
│       → Tool System (file/git/command/LSP)                   │
│       → Retry/Recovery/Fallback                               │
│       → Cancellation (graceful/immediate)                    │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                  Observability & Persistence                  │
│                                                              │
│  Event Bus → State Persistence → Audit (HMAC-signed)         │
│         + Prometheus Metrics + Tracing                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Repository Structure

```
rigorix-oss/
├── engine/              # Core library — all business logic
│   ├── src/             # 30 modules (27 Clean Architecture bounded contexts)
│   └── .pi/             # Architecture docs, ADRs, diagrams
├── cli/                 # CLI binary — thin wrapper over engine
│   ├── src/cli_boundary/# Flag-based CLI (Clap, dispatch, config)
│   ├── src/tui/         # Interactive TUI (ratatui)
│   └── .pi/             # Architecture docs
├── actions/             # GitHub Action — thin adapter over engine
│   ├── src/             # 9 bounded-context modules
│   └── .pi/             # Architecture docs
├── Cargo.toml           # Workspace root
└── .pi/                 # Root-level architecture docs, prompts, scripts
```

---

## Development

### Prerequisites

- Rust 2024 edition (stable toolchain)
- LLM API key (set `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`)

### Quality Checks

```bash
# Build everything
cargo build --workspace

# Lint
cargo clippy --workspace

# Format
cargo fmt --check

# Test
cargo test --workspace

# Security audit
cargo audit
```

---

## Architecture Documentation

Each crate has its own `.pi/architecture/` directory with:
- **Module specs** — Detailed interface contracts for each bounded context
- **ADRs** — Architecture Decision Records explaining key design choices
- **Diagrams** — System context, data flow, deployment

---

## Contributing

We welcome contributions! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for our development process, coding standards, and pull request workflow.

Key guidelines:
- Every edit must pass `cargo clippy --workspace` and `cargo fmt --check`
- All modules follow Clean Architecture with frozen contracts (see `.pi/architecture/`)
- Run `cargo test --workspace` before submitting
- New features require architecture documentation (see `.pi/prompts/feature-development.md`)

---

## License

Licensed under either of:

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
