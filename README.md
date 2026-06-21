# Rigorix

[![Crates.io](https://img.shields.io/badge/crate-rigorix-blue)](https://crates.io/crates/rigorix)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green)]()
[![CI](https://img.shields.io/badge/CI-passing-brightgreen)]()
[![Rust](https://img.shields.io/badge/rust-2024-edition-orange)](https://blog.rust-lang.org/2025/02/13/Rust-2024.html)

**Template-driven DAG execution engine with bounded autonomy.**

Rigorix is a deterministic coding agent framework that compiles natural-language intents into executable Directed Acyclic Graphs (DAGs). It operates through three modes:

- **CLI** (`rigorix`) — Interactive TUI + flag-based scripting for local development
- **GitHub Action** (`rigorix-action`) — PR governance and automated code generation in CI/CD
- **Engine** — The core library powering both

---

## Why Rigorix?

| Dimension | Rigorix | Copilot / Cursor | Aider | SWE-Agent |
|-----------|---------|------------------|-------|-----------|
| **Execution model** | Template-driven DAG (bounded, deterministic) | LLM-in-the-loop (stateless) | LLM-in-the-loop (file-by-file) | Agent loop (stateless) |
| **Code generation** | Structured: classify → extract → generate → validate → hash | Inline completions | Diff-based patches | Shell commands |
| **Safety** | Risk gating, enforcement caps, budget tracking, permission policies | None | None | None |
| **PR governance** | Built-in (policy.toml: deny/review/flag) | ✗ | ✗ | ✗ |
| **Fork safety** | Fork detection, secret masking, policy tamper detection | ✗ | ✗ | ✗ |
| **Audit** | HMAC-signed audit envelopes with circuit breaker | ✗ | ✗ | ✗ |
| **Offline-first** | ✓ (local TUI, no mandatory cloud) | ✗ | ✓ | ✗ |
| **Retry strategies** | SameOperation → ExpandContext → SimplifyOperation → SkipAndContinue | Fixed retry | None | None |
| **Quality gates** | Post-execution GreenContract evaluation | ✗ | ✗ | ✗ |
| **Self-correcting** | Validate loop (plan → execute → verify → repeat) | ✗ | ✗ | ✗ |

Rigorix is designed for **deterministic, auditable, safely-bounded automation** — not open-ended agent loops. If you need a code assistant that chats with you, use Copilot. If you need a CI/CD pipeline that enforces policies and generates auditable code changes, use Rigorix.

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
│   ├── src/             # 28 bounded-context modules (Clean Architecture)
│   └── .pi/             # Architecture docs, ADRs, diagrams
│
├── cli/                 # CLI binary — thin wrapper over engine
│   ├── src/cli_boundary/# Flag-based CLI (Clap, dispatch, config)
│   ├── src/tui/         # Interactive TUI (ratatui)
│   └── .pi/             # Architecture docs
│
├── actions/             # GitHub Action — thin adapter over engine
│   ├── src/             # 9 bounded-context modules (input, output, CI, security, etc.)
│   └── .pi/             # Architecture docs
│
├── Cargo.toml           # Workspace root
├── .pi/                 # Root-level architecture docs, prompts, scripts
├── .agents/             # Agent skill definitions
└── .gitnexus/           # GitNexus code intelligence index
```

---

## Crate Overview

| Crate | Purpose | Key Dependencies |
|-------|---------|-----------------|
| `rigorix-engine` | All business logic: planning, DAG execution, tools, enforcement, observability | serde, tokio, tree-sitter, tracing |
| `rigorix-cli` | `rigorix` binary: CLI parsing, config loading, TUI, output formatting | clap, ratatui, crossterm, engine |
| `rigorix-actions` | `rigorix-action` binary: GitHub Action input/output, diff analysis, policy eval | reqwest, serde_yaml, engine |

---

## Quick Start

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Run the CLI (opens TUI by default)
cargo run -p rigorix-cli

# Run a plan
cargo run -p rigorix-cli -- plan "refactor the auth module to use async"

# Run an execution
cargo run -p rigorix-cli -- run "add error handling to the database layer"

# Initialize a project
cargo run -p rigorix-cli -- init
```

---

## Key Concepts

### Intent → Plan → Execute

1. **Intent**: A natural-language string describing what the user wants (e.g., "add input validation to the login form")
2. **Plan**: The engine classifies the intent against templates, extracts parameters, generates a `TaskGraph` (a DAG of operations)
3. **Execute**: The `ParallelExecutor` runs the DAG with configurable concurrency, retry policies, and tool gating

### Bounded Autonomy

Rigorix enforces safety through layered controls:

| Layer | Mechanism |
|-------|-----------|
| **Risk Gating** | Tool-level risk classification (Low/Med/High) with confirm/dry-run/block |
| **Enforcement** | Hard caps on concurrency, total operations, LLM calls, tokens |
| **Budget Tracking** | RAII-style reservation system for LLM budgets |
| **Permission** | Path-based allow/deny rules for file access |
| **Quality Gates** | Post-execution quality evaluation against contracts |

---

## Development

### Prerequisites

- Rust 2024 edition (stable toolchain)
- LLM API key (set `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`)

### Quality Checks

```bash
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
- **CHANGELOG** — Architecture change history

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
