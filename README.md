# Rigorix

[![Crates.io](https://img.shields.io/badge/crate-rigorix-blue)](https://crates.io/crates/rigorix)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green)]()
[![CI](https://img.shields.io/badge/CI-passing-brightgreen)]()
[![Rust](https://img.shields.io/static/v1?label=rust&message=2024%20edition&color=orange)](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)

**A deterministic coding-agent runtime for repeatable, auditable AI software engineering.**

Rigorix compiles natural-language development tasks into executable Directed Acyclic Graphs (DAGs). Instead of relying on an open-ended agent loop, it separates planning from execution: the execution plan is generated, validated, and then executed within configurable policy, permission, and budget constraints. The result is AI-assisted software engineering that is repeatable, inspectable, and suitable for automated environments such as CI/CD.

Rigorix operates through three modes:

- **CLI** (`rigorix`) — Interactive TUI + flag-based scripting for local development
- **GitHub Action** (`rigorix-action`) — PR governance and automated code generation in CI/CD
- **Engine** — The core library powering both

---

## Why Rigorix Exists

Modern coding agents are remarkably capable. They can write code, edit projects, execute commands, and iterate on failures. But they share a fundamental problem: **they are unpredictable, unauditable, and difficult to govern in automated contexts.**

Every agent loop today works the same way: an LLM decides what to do, does it, checks the result, and loops. That loop is powerful — but it has no structure. There's no distinction between planning and execution. There's no audit trail beyond conversation history. There's no way to say "execute this plan but only if it stays within these boundaries."

This works fine when a human is watching every step. It breaks down when you want to:

- **Run in CI/CD** — without a human to approve every tool call
- **Audit what happened** — when conversation history isn't enough for compliance
- **Enforce policies** — "deny any change that touches the auth module" or "flag diffs that modify payment processing"
- **Budget costs** — cap LLM spending per run so a runaway agent doesn't burn your API key


Rigorix is opinionated: it intentionally gives up some **flexibility** in exchange for **repeatability, governance, and deterministic execution.**

The core idea is simple: instead of an LLM deciding what to do at each step, you compile the intent into a DAG first — a deterministic, reviewable plan. The DAG says: *read these files, generate this patch, run these tests, verify these conditions.* The LLM fills in the content; the DAG controls the flow. This is the same pattern that made build systems (Make, Bazel) and data pipelines (Airflow, Dagster) reliable: separate *what* from *how*, validate the plan before running it, and record every execution.

This approach makes tradeoffs. Rigorix is not as flexible as a free-form agent loop. It can't have a "conversation" with you or improvise mid-execution. If you want a coding assistant that chats, Rigorix is the wrong tool. But if you want a CI/CD pipeline that generates code, enforces policies, produces auditable records, and can run without supervision — Rigorix exists for that.

**Rigorix achieves this through bounded autonomy:** every execution is constrained by configurable risk policies, permission rules, execution budgets, and quality gates. The model is intentionally restrictive: the LLM decides what to generate within the execution graph, while Rigorix determines what is allowed to happen.

---

## How It Compares

| Dimension | Rigorix | Claude Code | Copilot / Cursor | Aider | SWE-Agent |
|-----------|---------|-------------|------------------|-------|-----------|
| **Execution model** | Template-driven DAG (bounded, deterministic) | Stateful agent loop| Agent loop (Cursor) / inline completions (Copilot) | Agent loop (file-by-file, git context) | Agent loop (stateless per instance) |
| **Code generation** | Structured: classify → extract → generate → validate → hash | LLM edits files + runs shell commands | Inline completions + agentic edits (Cursor) | Diff-based patches via LLM | Shell commands from agent |
| **Safety** | Risk gating, enforcement caps, budget tracking, permission policies | Permission prompts, `--mode` (auto/plan/ask), project-level settings.json | Cursor: permission prompts for agent mode. Copilot: GitHub code scanning | Git auto-commits for rollback, `--lint` integration, read-only file designation | Docker sandbox for execution isolation |
| **PR governance** | Built-in (policy.toml: deny/review/flag) | External CI required | ✗ (Copilot Review has code-review suggestions, not governance) | ✗ | ✗ |
| **Audit** | HMAC-signed audit envelopes with circuit breaker | ✗ (conversation history only) | ✗ (conversation history only) | ✗ (git log only) | ✗ (ephemeral containers) |
| **Quality gates** | Post-execution GreenContract evaluation | ✗ (implicit — retries on error) | ✗ | ✗ (lint-then-fix is a gating step, but ad-hoc) | ✗ |
| **Self-correcting** | Validate loop (plan → execute → verify → repeat) | Agent loop retries on compilation/runtime errors | ✗ (Cursor) / Copilot code scanning alerts | Lint-then-fix loop (error → fix → re-lint) | Agent loop retries on errors |

Rigorix is designed for **deterministic, auditable, safely-bounded automation** — not open-ended agent loops. If you need a code assistant that chats with you, use Claude Code or Aider. If you need a CI/CD pipeline that enforces policies and generates auditable code changes, use Rigorix.

---

<video src="rigorix-demo.mov" controls width="720"></video>

*🎥 Demo: Rigorix planning and executing a TypeScript refactor — reading code, generating a patch, type-checking, and running tests.*

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
