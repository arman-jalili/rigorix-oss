# Implementation Roadmap — rigorix CLI

> Canonical implementation plan. Maps all 18 bounded contexts to implementation phases, issues, and milestones.
> All ADR decisions from ADR-001 through ADR-012 govern this roadmap.

---

## Overview

**Goal:** Ship a working `rigorix` CLI binary that wraps `rigorix-engine` (frozen contracts) and provides the full user experience: plan, execute, generate templates, view history, audit, and live TUI.

**Scope:** 18 bounded contexts → 6 implementation phases → ~25 issues.

**Strategy:** 
- **Layer by layer** — build from foundation up, validate each layer before depending on it
- **Engine-first** — CLI is a thin wrapper; engine contracts are frozen, so no engine changes needed
- **Vertical slice for Phase 3** — the `rigorix run` command is the first end-to-end integration; everything before is scaffolding, everything after is enrichment

---

## Dependency Graph

```
Phase 1: Scaffold
  cli-crate (Cargo.toml, main.rs, directory structure)
  └─ configuration (CLI config loading + merging)
  └─ observability (tracing init)
  └─ cancellation (signal handler)

Phase 2: Template System
  ├─ templates (list, show commands) ── depends on: Phase 1
  └─ template-generation (generate command) ── depends on: templates, Phase 1

Phase 3: Execution Core (Vertical Slice)
  ├─ planning (plan command) ── depends on: Phase 1, Phase 2
  ├─ execution-engine (run command) ── depends on: planning, Phase 1
  └─ event-system + state-persistence ── depends on: execution

Phase 4: Observability Commands
  ├─ history (list past sessions) ── depends on: state-persistence, Phase 3
  ├─ logs (stream/filter events) ── depends on: event-system, Phase 3
  └─ audit (list, show, diff trails) ── depends on: audit, Phase 3

Phase 5: TUI
  └─ tui-renderer (ratatui live view) ── depends on: event-system, Phase 3

Phase 6: Polish
  ├─ init command (scaffold project)
  ├─ error messages + help text
  ├─ CI/CD integration (JSON output hardening)
  └─ migration docs + runbooks
```

---

## Phase 1: Foundation Scaffold

**Goal:** CLI binary compiles, loads config, initializes tracing, handles Ctrl+C.
**Files:** ~8 new files in `cli/src/`
**Validation:** architecture-validator, operations-validator

### Issue 1.1 — CLI Crate Scaffold
| Field | Detail |
|-------|--------|
| Module | CLI Boundary |
| ADRs | ADR-002 (CLI/engine split), ADR-007 (ephemeral CLI) |
| Files | `cli/Cargo.toml`, `cli/src/main.rs` |
| Description | Create the Rust binary crate with `rigorix-engine` as a dependency. Binary entry point with clap argument parser. Define `CliCommand` enum. |
| ACs | ① Binary compiles with `cargo build` ② `rigorix --help` lists all commands ③ `rigorix --version` shows version |
| Steps | 1. Create `cli/Cargo.toml` with rigorix-engine dep 2. Create `cli/src/main.rs` with clap CLI definition 3. Define `CliCommand` enum 4. Match-and-dispatch skeleton for each command |
| Validators | ci, architecture |

### Issue 1.2 — Configuration Loading
| Field | Detail |
|-------|--------|
| Module | Configuration |
| ADRs | ADR-002 (CLI/engine split) |
| Files | `cli/src/config.rs` |
| Description | CLI-side config loader that merges CLI flags → env vars (`RIGORIX_*`) → `rigorix.toml` → engine defaults. Handles `--config` flag for custom path. |
| ACs | ① Loads `rigorix.toml` from cwd ② CLI flags override env vars which override file ③ Reports clear error for missing API key ④ Merged config passed to engine `ConfigService` |
| Steps | 1. Create `cli/src/config.rs` with `CliConfig` struct 2. Implement CLI flag → env → file merging 3. Implement validation (API key present, template dir exists, etc.) 4. Wire into main.rs startup sequence |
| Validators | ci, architecture |

### Issue 1.3 — Observability Initialization
| Field | Detail |
|-------|--------|
| Module | Observability |
| ADRs | — |
| Files | `cli/src/tracing.rs` |
| Description | Initialize engine tracing with CLI config. Respect `RIGORIX_LOG` env var. Support `--log-format json|pretty`. |
| ACs | ① `RIGORIX_LOG=debug rigorix` shows debug-level output ② `--log-format json` produces JSON lines ③ Default is pretty human-readable |
| Steps | 1. Create `cli/src/tracing.rs` 2. Wire `engine::observability::init_tracing()` 3. Add `--log-format` and `--log-level` CLI flags |
| Validators | ci, operations |

### Issue 1.4 — Signal Handler (Cancellation)
| Field | Detail |
|-------|--------|
| Module | Cancellation |
| ADRs | ADR-007 (ephemeral CLI) |
| Files | `cli/src/signal.rs` |
| Description | Captures Ctrl+C (SIGINT). Single press = graceful shutdown. Double press within 2s = immediate shutdown. Forwards to engine's `CancellationService`. |
| ACs | ① Single Ctrl+C triggers graceful cancellation ② Double Ctrl+C within 2s triggers immediate cancellation ③ No crash on SIGINT if no execution is running |
| Steps | 1. Create `cli/src/signal.rs` with `SignalHandler` 2. Implement double-press detection (tokio::time::Duration) 3. Wire CancellationToken to engine's CancellationService |
| Validators | ci, operations |

---

## Phase 2: Template System

**Goal:** Templates are loaded, listable, and generatable. `rigorix template list` works. `rigorix generate` works.
**Files:** ~5 new files
**Validation:** architecture-validator

### Issue 2.1 — Template List & Show Commands
| Field | Detail |
|-------|--------|
| Module | Templates |
| ADRs | ADR-004 (TOML format) |
| Files | `cli/src/template_cmd.rs` |
| Description | `rigorix template list` — shows all registered templates with id, name, description. `rigorix template show <id>` — shows full TOML definition. Uses engine's `TemplateEngine`. |
| ACs | ① `rigorix template list` outputs all built-in + user templates ② `rigorix template show <id>` outputs the TOML ③ Unknown template ID shows clear error |
| Steps | 1. Create `cli/src/template_cmd.rs` 2. Initialize `TemplateEngine` with built-in + user templates 3. Implement list (table output) 4. Implement show (TOML output) |
| Validators | ci, architecture |

### Issue 2.2 — Generate Command
| Field | Detail |
|-------|--------|
| Module | Template Generation |
| ADRs | ADR-009 (Claude provider), ADR-010 (persist generated templates) |
| Files | `cli/src/generate.rs` |
| Description | `rigorix generate <intent>` — generates TOML template via Claude, validates, persists to `.rigorix/templates/`. Supports `--dry-run` and `--stdout`. Builds `RepoContext` for LLM context. |
| ACs | ① `rigorix generate "add endpoint"` produces valid TOML ② `--dry-run` shows output without saving ③ `--stdout` prints TOML to stdout ④ Generated template is immediately usable by `rigorix plan` ⑤ Obsolete fallback behavior in planning pipeline also persists |
| Steps | 1. Create `cli/src/generate.rs` with `CliGenerateHandler` 2. Build `RepoContext` via file tree scan + optional SymbolGraph 3. Wire `ClaudeTemplateGenerator` with API key from config 4. Parse validated TOML → save to `.rigorix/templates/` 5. Add `--dry-run` and `--stdout` flags 6. Modify planning fallback path to persist generated templates |
| Validators | ci, architecture, security (API key handling) |

---

## Phase 3: Execution Core (Vertical Slice)

**Goal:** `rigorix plan` and `rigorix run` work end-to-end. This is the first integration that touches all layers.
**Files:** ~8 new files
**Validation:** architecture-validator, security-validator, operations-validator

### Issue 3.1 — Orchestrator Wrapper
| Field | Detail |
|-------|--------|
| Module | CLI Boundary |
| ADRs | ADR-002 (CLI/engine split) |
| Files | `cli/src/orchestrator.rs` |
| Description | Top-level orchestrator that wires: Config → PlanningPipeline → ExecutionEngine → EventBus → output. Manages the full lifecycle of a single execution session. |
| ACs | ① Creates PlanningPipelineService with config ② Creates EventBus and wires subscribers ③ Creates ParallelExecutor with sealed TaskGraph ④ Drains EventBus at end for audit + state persistence |
| Steps | 1. Create `cli/src/orchestrator.rs` with `CliOrchestrator` 2. Implement `run()` method: init → plan → execute → drain → output 3. Handle cancellation signals during execution 4. Handle errors at each stage with clear messages |
| Validators | ci, architecture, operations |

### Issue 3.2 — Plan Command (Preview)
| Field | Detail |
|-------|--------|
| Module | Planning Pipeline |
| ADRs | ADR-005 (EventBus), ADR-009 (Claude provider) |
| Files | `cli/src/plan.rs` |
| Description | `rigorix plan <intent>` — runs the 6-phase planning pipeline without execution. Outputs DAG preview (nodes, dependencies, tool bindings). |
| ACs | ① `rigorix plan "add endpoint"` shows a valid DAG preview ② `--json` flag outputs structured plan JSON ③ Low-confidence matches suggest alternative templates ④ Budget pre-check is performed before LLM call |
| Steps | 1. Create `cli/src/plan.rs` with `PlanCommand` 2. Wire PlanningPipelineService 3. Render PlanOutput as human-readable DAG 4. Add `--json` flag for structured output |
| Validators | ci, architecture |

### Issue 3.3 — Run Command
| Field | Detail |
|-------|--------|
| Module | Execution Engine |
| ADRs | ADR-011 (retry/backoff), ADR-012 (risk gating) |
| Files | `cli/src/run.rs`, `cli/src/session.rs` |
| Description | `rigorix run <intent>` — plan + execute with real-time output. Creates `ExecutionSession` that manages the full lifecycle. |
| ACs | ① `rigorix run "add endpoint"` executes the full pipeline ② Node status transitions are printed to console ③ Failures show retry attempts with backoff ④ Ctrl+C triggers graceful cancellation ⑤ Final summary shows completed/failed/skipped counts |
| Steps | 1. Create `cli/src/session.rs` with `ExecutionSession` 2. Create `cli/src/run.rs` with `RunCommand` 3. Wire CLI orchestrator into session lifecycle 4. Add console output for real-time status 5. Wire risk gating confirmation prompts 6. Add `--json` flag for structured output |
| Validators | ci, architecture, security, operations |

---

## Phase 4: Observability Commands

**Goal:** Users can view past executions, stream logs, and inspect audit trails.
**Files:** ~4 new files
**Validation:** architecture-validator, security-validator

### Issue 4.1 — History Command
| Field | Detail |
|-------|--------|
| Module | State Persistence |
| ADRs | ADR-008 (atomic write-rename) |
| Files | `cli/src/history_cmd.rs` |
| Description | `rigorix history` — lists past execution sessions with date, template, status, duration. `rigorix history show <id>` — shows full session details. Reads from persisted `ExecutionState` files. |
| ACs | ① Lists past sessions chronologically ② Shows status (completed/failed/cancelled) ③ `history show <id>` shows node-level detail |
| Steps | 1. Create `cli/src/history_cmd.rs` 2. Scan `.rigorix/state/` for execution files 3. Deserialize and display summary 4. Add `show <id>` subcommand |
| Validators | ci, architecture |

### Issue 4.2 — Logs Command
| Field | Detail |
|-------|--------|
| Module | Event System |
| ADRs | ADR-005 (EventBus) |
| Files | `cli/src/logs_cmd.rs` |
| Description | `rigorix logs` — streams/replays execution events. Can filter by type, node, severity. Can follow live execution or replay past runs. |
| ACs | ① Replays events from a past execution ② Filters by event type (e.g. `--type node_failed`) ③ Follows live execution with `--follow` |
| Steps | 1. Create `cli/src/logs_cmd.rs` 2. Query PersistedEvents from EventBus 3. Implement filters by type, node_id, severity 4. Add `--follow` for live streaming |
| Validators | ci, architecture |

### Issue 4.3 — Audit Commands
| Field | Detail |
|-------|--------|
| Module | Audit |
| ADRs | ADR-008 (atomic write-rename for state) |
| Files | `cli/src/audit_cmd.rs` |
| Description | `rigorix audit list` — lists audit envelopes. `rigorix audit show <id>` — shows full envelope. `rigorix audit diff <id1> <id2>` — compares two execution plans via planning hash. |
| ACs | ① Lists audit envelopes with summary ② Shows full envelope with events ③ Diffs two plans via planning_hash comparison |
| Steps | 1. Create `cli/src/audit_cmd.rs` 2. Read persisted audit envelopes 3. Implement list, show, diff subcommands 4. Format output as JSON or human-readable |
| Validators | ci, architecture, security |

---

## Phase 5: Terminal UI

**Goal:** Real-time TUI showing execution progress.
**Files:** ~5 new files in `cli/src/tui/`
**Validation:** architecture-validator, operations-validator

### Issue 5.1 — TUI Renderer
| Field | Detail |
|-------|--------|
| Module | CLI Boundary |
| ADRs | ADR-003 (ratatui), ADR-005 (EventBus) |
| Files | `cli/src/tui/mod.rs`, `cli/src/tui/events.rs`, `cli/src/tui/widgets.rs` |
| Description | ratatui-based terminal UI. Subscribes to EventBus broadcast channel. Three panels: DAG graph (node statuses), Budget bars (tokens/calls/time), Event log (scrollable). Handles terminal resize. Falls back to console output if TUI is unavailable or `--no-tui` flag is set. |
| ACs | ① Shows live node status transitions ② Shows budget bars updating in real-time ③ Event log is scrollable ④ Handles terminal resize without corruption ⑤ `--no-tui` flag falls back to console output ⑥ Ctrl+C still works (TUI doesn't consume signal) |
| Steps | 1. Create `cli/src/tui/mod.rs` with TuiRenderer 2. Create `cli/src/tui/events.rs` — EventBus subscriber → mpsc channel 3. Create `cli/src/tui/widgets.rs` — DAG graph, budget bar, event log widgets 4. Wire terminal resize (SIGWINCH) handling 5. Add `--no-tui` fallback flag 6. Integrate into RunCommand execution session |
| Validators | ci, architecture, operations |

---

## Phase 6: Polish & Production Readiness

**Goal:** Production-ready CLI with good UX, documentation, and CI integration.
**Files:** ~3 new files + documentation
**Validation:** All validators

### Issue 6.1 — Init Command
| Field | Detail |
|-------|--------|
| Module | CLI Boundary |
| ADRs | ADR-004 (TOML format) |
| Files | `cli/src/init.rs` |
| Description | `rigorix init` — scaffolds `.rigorix/` directory with default `rigorix.toml`, `.rigorix/templates/` with built-in templates, and `.gitignore` entry. Interactive mode prompts for API key, enforcement preset, etc. |
| ACs | ① Creates `.rigorix/` directory structure ② Writes default `rigorix.toml` ③ Copies built-in templates ④ Adds `.rigorix/state/` to `.gitignore` ⑤ Interactive mode with prompts |
| Steps | 1. Create `cli/src/init.rs` 2. Scaffold directory structure 3. Write default config 4. Copy built-in templates 5. Add to gitignore 6. Interactive prompts |
| Validators | ci, architecture |

### Issue 6.2 — Error Polish & Help Text
| Field | Detail |
|-------|--------|
| Module | CLI Boundary |
| ADRs | — |
| Files | `cli/src/error.rs`, help strings throughout |
| Description | Consistent error formatting with error codes, suggestions, and color. `--help` text follows handbook style with examples. All `CoreOrchestratorError` variants rendered as user-friendly messages. |
| ACs | ① Errors show error code, message, and suggestion ② `--help` on every command with examples ③ Color output where supported ④ JSON errors for CI integration |
| Steps | 1. Create `cli/src/error.rs` with error formatting 2. Audit all error sites for user-friendly messages 3. Add examples to all `--help` texts 4. Test with `--json` flag for structured errors |
| Validators | ci, architecture |

### Issue 6.3 — CI/CD Integration
| Field | Detail |
|-------|--------|
| Module | CLI Boundary |
| ADRs | — |
| Files | `cli/src/output.rs`, CI config |
| Description | Harden JSON output format for CI/CD consumption. Add `--quiet` flag (minimal output). Ensure non-zero exit codes on failure. Add CI pipeline (build, test, lint, audit). |
| ACs | ① `rigorix run --json` produces valid JSON ② Non-zero exit code on failure ③ `--quiet` produces no stdout ④ `cargo build` succeeds with warnings-as-errors ⑤ CI pipeline passes |
| Steps | 1. Harden JSON serialization 2. Add `--quiet` flag 3. Ensure exit codes propagate correctly 4. Create CI pipeline config |
| Validators | ci, architecture |

### Issue 6.4 — Migration Docs & Runbooks
| Field | Detail |
|-------|--------|
| Module | All |
| ADRs | — |
| Files | `cli/docs/MIGRATION.md`, `cli/docs/runbook.md`, `cli/docs/development.md` |
| Description | Document how the CLI maps to engine frozen contracts. Runbook for common operations. Development guide for contributors. |
| ACs | ① MIGRATION.md maps each engine contract to CLI implementation ② Runbook covers install, config, common workflows ③ Development guide covers build, test, contributions |
| Steps | 1. Create `cli/docs/MIGRATION.md` 2. Create `cli/docs/runbook.md` 3. Create `cli/docs/development.md` |
| Validators | architecture |

---

## Timeline Estimate

| Phase | Issues | Engine Dependencies | Est. Effort | Validation Gates |
|-------|--------|-------------------|-------------|-----------------|
| **P1: Scaffold** | 4 | engine (lib) | 1-2 days | cargo build, --help, config load |
| **P2: Templates** | 2 | engine::templates, engine::template_generation | 2-3 days | template list, generate --dry-run |
| **P3: Execution** | 3 | engine::planning, engine::dag_engine, engine::execution_engine, engine::enforcement, engine::risk_gating | 4-5 days | rigorix run with a built-in template |
| **P4: Observability** | 3 | engine::state_persistence, engine::event_system, engine::audit | 2-3 days | rigorix history lists past run |
| **P5: TUI** | 1 | engine::event_system (broadcast channel) | 3-4 days | Live TUI shows node transitions |
| **P6: Polish** | 4 | engine (all) | 2-3 days | Full CI green, --help complete |
| **Total** | ~17 | — | ~14-20 days | — |

---

## Risk Matrix

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Engine API changes break CLI | Low | High | Pin engine version in Cargo.toml; CI runs with latest engine HEAD |
| Claude API rate limits during dev | Medium | Medium | Mock ClaudeGenerator in tests; configurable retries in generator |
| TUI rendering blocks event processing | Low | High | ratatui runs in separate tokio task with mpsc channel; EventBus subscriber never blocks |
| Double Ctrl+C timing too tight | Low | Low | Configurable double-press window (default 2s); test with SIGINT simulation |
| Generated templates have invalid TOML | Medium | Low | `ClaudeTemplateGenerator` retries up to 3 times with parse error feedback; template validation post-generation |
| No LLM API key configured on first run | High | Low | `rigorix init` prompts for key; `rigorix run` shows clear error if missing; plan-only mode still works for template preview |

---

## Validation Per Phase

| Phase | Required Validators | Scope |
|-------|--------------------|-------|
| P1 | ci, architecture | Simple-Moderate |
| P2 | ci, architecture | Moderate |
| P3 | ci, architecture, security, operations | Complex (vertical slice) |
| P4 | ci, architecture | Moderate |
| P5 | ci, architecture, operations | Complex |
| P6 | All | Moderate-Critical |

---

## Key Milestones

| Milestone | Phase | Artifact | Criteria |
|-----------|-------|----------|----------|
| **M1: CLI boots** | P1 | `rigorix --help` | Binary compiles, help text shown |
| **M2: Templates work** | P2 | `rigorix template list` | 13 built-in templates shown |
| **M3: First execution** | P3 | `rigorix run "add endpoint" --dry-run` | Plan preview shown without LLM |
| **M4: Full execution** | P3 | `rigorix run "read README"` | DAG executes, output shown |
| **M5: History works** | P4 | `rigorix history` | Past execution listed |
| **M6: TUI live** | P5 | TUI shows during run | Node transitions visible |
| **M7: Ship ready** | P6 | CI green, docs done | All validators pass |

---

## ADR Reference Cross-Map

| ADR | Decides | Implemented In |
|-----|---------|---------------|
| ADR-001 | DDD with bounded contexts | All phases (module structure) |
| ADR-002 | CLI/engine split | P1.1 (crate scaffold) |
| ADR-003 | ratatui TUI | P5.1 (TUI renderer) |
| ADR-004 | TOML template format | P2.1 (template commands) |
| ADR-005 | EventBus pub-sub | P3.3 (run command), P5.1 (TUI) |
| ADR-006 | Plugin deferral v2 | Not implemented in v1 |
| ADR-007 | Ephemeral CLI | P1.1 (crate scaffold), P1.4 (signal handler) |
| ADR-008 | Atomic write-rename | P4.1 (history reads state) |
| ADR-009 | Claude LLM provider | P2.2 (generate command) |
| ADR-010 | Persist generated templates | P2.2 (generate command), P3.2 (plan fallback) |
| ADR-011 | Retry + backoff | P3.3 (run command — engine handles) |
| ADR-012 | Risk gating levels | P3.3 (run command — confirmation UI) |

---

*Version: 1.0.0*
*Last updated: 2026-06-16*
*Generated from session: 71e2b81a-a7a1-48ee-ab8f-56284bbec92d*
