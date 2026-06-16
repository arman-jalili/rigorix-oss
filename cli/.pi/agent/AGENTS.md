---
# Guardian Workflow Configuration
# YAML front matter = runtime settings. Body = agent prompt.
# Changes to this file are detected and re-applied without restart.

workspace:
  root: ".pi/workspaces"
  hooks:
    timeout_ms: 60000

agent:
  max_turns: 20
  max_retry_backoff_ms: 300000
  stall_timeout_ms: 300000

system_prompt_tier: full

generate:
  on_conflict: warn
  atomic_writes: true

validate:
  fail_fast: false
  timeout_ms: 300000

goal:
  enabled: true
  max_turns: 20
  judge_validator: true

kanban:
  enabled: true
  auto_create_tasks: true

hooks:
  pre_tool_call: []
  post_tool_call: []
  pre_llm_call: []
  post_llm_call: []
  on_session_start: []
  on_session_end: []
  subagent_stop: []

curator:
  enabled: true
  stale_after_days: 30
  archive_after_days: 90
  auto_review: true

delegation:
  max_spawn_depth: 1
  max_concurrent_children: 3
  max_iterations: 50
  child_timeout_ms: 600000
---

# Project Context

> **Purpose:** Single source of truth for project-specific knowledge. All agents load this ONCE.
> **Customize:** Fill in the sections below for your project. The YAML front matter above already has working defaults.

## Project Overview

- **Name:** rigorix-oss
- **Version:** 0.1.0
- **Language:** [TypeScript / Python / Rust / Go]
- **Type:** [CLI / Web App / Library]
- **Repository:** [owner/repo]

## Core Principles

> These are loaded into EVERY agent's context. Keep to 5-8 items.

1. **Read before edit** — Always read a file before modifying it. Never mutate blindly.
2. **Validate early** — Run `bash .pi/scripts/ci/run_preflight.sh` before committing.
3. **Architecture traceability** — Every implementation file must reference its architecture source in `.pi/architecture/modules/`.
4. **DRY context** — Shared knowledge lives in `.pi/context/`, not scattered across agent files.
5. **Shift-left validation** — Plans are validated before code is written.

## Commands

> Essential commands agents need to run. Update these for your project.

| Command | Purpose |
|---------|---------|
| `cargo build` | Build project |
| `cargo test --all` | Run tests |
| `cargo clippy -- -D warnings` | Lint check |
| `bash .pi/scripts/ci/run_preflight.sh` | Run local preflight checks |
| `bash .pi/scripts/validate-*.sh` | Run specific validator |

## Architecture

### Structure

```
[project]/
├── src/              # Source code
├── tests/            # Test files
├── docs/             # Documentation
└── [other dirs]
```

### Key Files

> Files every agent should know about. Keep under 10.

| File | Purpose |
|------|---------|
| `.pi/architecture/modules/` | Canonical architecture modules |
| `.pi/architecture/roadmap.md` | Implementation roadmap (phases, issues, milestones) |
| `.pi/architecture/decisions/` | ADR-001 through ADR-012 — architecture decisions |
| `.pi/agent/AGENTS.md` | This file — project context + runtime config |
| `.pi/skills/agents/rust-codegen.md` | On-demand Rust codegen patterns — minimal agent, reads sections from the reference doc only when needed |
| `.pi/scripts/` | Validation scripts |
| `.pi/extensions/` | Pi extensions (tools, commands, hooks) |
| `.pi/domain/exploration.md` | DDD domain analysis (actors, FR, NFR, entities, events) |
| `.pi/domain/ubiquitous-language.md` | Canonical glossary (48 terms, prohibited aliases) |
| `.pi/skills/agents/rust-codegen.md` | Minimal agent skill — on-demand Rust codegen patterns |

## Quality Gates

### Before Commit

```bash
bash .pi/scripts/ci/run_preflight.sh
cargo build
```

### Before Push

```bash
cargo test --all
cargo clippy -- -D warnings
```

## Subagent Delegation

| Task | Subagent | Tools |
|------|----------|-------|
| Explore codebase | `explore` | read-only (read, grep, glob) |
| Code review | `code-review` | read-only (read, grep, glob) |
| Security audit | `security-review` | read-only (read, grep, glob) |

Subagents have **restricted tool access** and **fresh context**. Include all relevant context in the spawn prompt.

## Snippets

Available `#handle` tokens for quick instruction injection:

| Handle | Purpose |
|--------|---------|
| `#security-review` | Security audit instructions |
| `#no-comments` | Suppress comments unless WHY is non-obvious |
| `#test-first` | TDD workflow instructions |

## Environment

> Variables agents may reference. Use `$VAR_NAME` in scripts.

| Variable | Purpose |
|----------|---------|
| `$GITHUB_TOKEN` | GitHub API authentication |
| `$CI` | CI environment indicator |

## Security Guards

Path safety guards are enforced by extensions (`.pi/extensions/bash-guard.ts`):

- **Read blocklist:** `.env*`, `*.pem`, `*.key`, `.ssh/*`, `.aws/*`, `.git/*`
- **Write blocklist:** Inherits read restrictions + `/etc/`, `/System/`, `/private/`
- **Command deny-list:** `rm -rf /`, `mkfs`, `dd of=/dev/*`, `terraform destroy`, `kubectl delete`
