# TUI

## Status

**Status:** ✅ Architecture defined — source code implemented
**Last reviewed:** 2026-06-16

## Description

Interactive terminal UI — the **primary user interface** for Rigorix. Users launch `rigorix` (no args) to enter the TUI, then type intents, execute plans, browse history, generate templates, and inspect audit trails — all without leaving the terminal.

The flag-based CLI (`rigorix run <intent>`, `rigorix plan`, etc.) remains for scripting and CI/CD integration. The TUI is the interactive path.

## Design Philosophy

| Principle | Rationale |
|-----------|-----------|
| **TUI-first** | Launch `rigorix` with no args → TUI. CLI flags are for CI/scripting |
| **Plan first** | Every intent shows a plan preview before asking: Run, Plan Only, Generate, or Cancel |
| **Command bar** | Bottom-of-screen input field where users type intents, filter, search |
| **Non-blocking** | Orchestrator runs in a background task — UI stays responsive |
| **Rich feedback** | Real-time DAG progress, node details, event log, budget bars |
| **All commands available** | Run, plan, history, audit, templates, generate — all from the TUI |

## Architecture Layers

```
┌─────────────────────────────────────────────────────┐
│                  COMMAND BAR LAYER                   │
│  Text input: type intents, /filter, :commands       │
├─────────────────────────────────────────────────────┤
│                  PLAN REVIEW LAYER                   │
│  Preview plan: template, confidence, nodes, params  │
│  Choose: [r] Run  [p] Plan Only  [g] Generate  [Esc]│
├─────────────────────────────────────────────────────┤
│                  DISPATCH LAYER                      │
│  Spawn orchestration in background task              │
│  Calls same engine services as cli_boundary::dispatch│
├─────────────────────────────────────────────────────┤
│                  STATE LAYER                         │
│  TuiViewModel, DagViewModel, event timeline, metrics │
├─────────────────────────────────────────────────────┤
│                EVENT BRIDGE LAYER                    │
│  EventBus subscriber → ViewModel mutations (async)  │
├─────────────────────────────────────────────────────┤
│                  RENDERING LAYER                     │
│  ratatui widgets, layouts, styles, animations       │
└─────────────────────────────────────────────────────┘
```

## User Flow

```
$ rigorix
  → TUI opens with Dashboard view, empty state
  → User types "add authentication middleware" in command bar (↲)
  → TUI calls engine::orchestrator::plan_only(intent)
  → Plan Preview view opens:
      ┌─────────────────────────────────────┐
      │  Template: add-endpoint             │
      │  Confidence: 94%                    │
      │  Nodes: [1] file-read → [2] add-    │
      │         route → [3] add-tests       │
      │  Parameters: method=POST, path=/... │
      │  LLM cost: ~2 calls, ~800 tokens   │
      │                                     │
      │  [r] Run    [p] Plan Only           │
      │  [g] Generate Template  [Esc] Cancel│
      └─────────────────────────────────────┘
  → User presses r (Run)
  → TUI spawns orchestrator in background task
  → Execution Dashboard shows real-time node progress
  → User presses Ctrl+C → confirmation modal → graceful cancel
  → User browses event log, inspects node output
  → User presses q → back to command bar
```

## Plan Before Act Flow

Every intent follows this sequence:

```
Type intent → Show Plan Preview → Choose action:
    │
    ├── [r] Run ──────────→ Execute plan with real-time dashboard
    │
    ├── [p] Plan Only ────→ Show plan details, return to command bar
    │                        (no execution)
    │
    ├── [g] Generate ─────→ Save as reusable template to .rigorix/templates/
    │                        Return to command bar
    │
    └── [Esc] Cancel ─────→ Discard plan, return to command bar
```

The Plan Preview is never skipped — every intent shows the plan first and asks
for confirmation. This is the "plan first" principle.

## Startup Entry Point

```
main() {
    let config = load_config();
    install_signal_handler();
    init_tracing();

    if no_subcommand_given {
        // TUI-first: launch interactive mode
        tui::run(config).await
    } else {
        // Scripting/CI path: use cli_boundary dispatch
        cli_boundary::dispatch(command, config).await
    }
}
```

### TUI Startup Modes

| Mode | Trigger | Description |
|------|---------|-------------|
| Fresh start | `rigorix` (no args) | TUI opens on Dashboard with command bar, no execution loaded |
| Live execution | Type intent in command bar | Plan → execute in background, UI updates live |
| View past | `rigorix tui --exec <id>` | Load past execution from disk into read-only mode |
| View latest | `rigorix tui` (positional) | Load most recent execution |
| History browser | `/history` command in bar | Browse all past executions |
| Run from CLI | `rigorix tui --run "<intent>"` | Launch TUI with orchestrator already running |

## Command Bar

The command bar is a persistent text input at the bottom of the screen. Users type:

| Input | Action |
|-------|--------|
| `add authentication middleware` | Plan and run an intent |
| `plan refactor database layer` | Plan only (no execution) |
| `/search error` | Filter event log |
| `/history` | Switch to History view |
| `/templates` | List available templates |
| `/generate "add CLI command"` | Generate a new template |
| `/audit` | Browse audit trails |
| `/help` | Show help overlay |
| `:q` | Quit |
| `:cancel` | Cancel running execution |

## EventBridge

Subscribes to the engine's `EventBus` broadcast channel. Each incoming `ExecutionEvent` is mapped to a `ViewModelMutation`.

**Lag handling:**
- < 50 dropped: continue processing remaining events
- ≥ 50 dropped: trigger reconciliation from `ExecutionState` on disk

**Background task management:**

```
User types intent in command bar
  → TUI creates OrchestratorConfig + builds orchestrator
  → tokio::spawn(orchestrator.run(RunInput { intent, config }))
  → EventBridge subscribes to orchestrator.event_bus()
  → UI immediately switches to Execution dashboard
  → User can cancel, inspect nodes, browse events — all non-blocking
  → When orchestrator completes, final state shown
  → User returns to command bar for next action
```

**Reverse channel:** `tokio::sync::mpsc::Sender<TuiCommand>` for TUI → orchestrator:

| Command | Key/Input | Action |
|---------|-----------|--------|
| `Cancel { Graceful }` | `:cancel` or Ctrl+C once | Finish current node, then stop |
| `Cancel { Immediate }` | `:cancel!` or Ctrl+C twice | Abort all in-flight nodes |
| `RetryNode { node_id }` | Select node → `r` key | Retry a failed node (Phase 3+) |

### Event → ViewModel Mapping

| Engine Event | ViewModel Mutation |
|-------------|-------------------|
| `PlanningStarted` | `phase = Planning`, show spinner in command bar |
| `PlanningCompleted` | Store template_id, confidence, show plan summary |
| `NodeStarted` | `phase = Executing`, node → InProgress |
| `NodeCompleted` | node → Completed, increment counter, update progress bar |
| `NodeFailed` | node → Failed, store error, highlight in DAG tree |
| `NodeRetrying` | Increment retry count, node → InProgress, log to event view |
| `ToolExecuted` | Store tool name, risk level, tool count |
| `ExecutionCompleted` | `phase = Completed`, show final summary, re-enable command bar |
| `ExecutionFailed` | `phase = Failed`, show error, re-enable command bar |
| `ExecutionCancelled` | `phase = Cancelled`, show cancelled summary |
| `BudgetWarning` | Update LLM budget bars in metrics panel |

## ViewModel

| Component | Purpose | Fields |
|-----------|---------|--------|
| TuiViewModel | Root state | execution_id, phase, intent, template_id, nodes, event_log, metrics, llm_budget, active_view, error, command_bar_history |
| DagViewModel | Node tree | HashMap<NodeViewModels>, root_ids, exec_order |
| NodeViewModel | Single node | id, name, tool_name, status, dependencies, dependents, timing, output_preview, error, retry_count, risk_level |
| MetricsViewModel | Live counters | llm_calls, tokens, node counts, throughput, tool_counts |
| LlmBudgetViewModel | Budget bars | max_calls, used_calls, max_tokens, used_tokens |

### Double-Buffering

Two copies of the ViewModel to eliminate RwLock contention:

- `write_buffer`: `tokio::sync::RwLock<TuiViewModel>` — EventBridge writes here
- `read_buffer`: `parking_lot::RwLock<TuiViewModel>` — Render loop reads here
- Swap happens once per event batch, not per frame

## Views

| View | File | Description |
|------|------|-------------|
| Dashboard | `views/dashboard.rs` | DAG tree + selected node details + metrics + progress |
| Nodes | `views/nodes.rs` | Full node list table with detail panel |
| Events | `views/events.rs` | Filterable event timeline |
| Plan | `views/plan.rs` | Plan preview with template, confidence, nodes, parameters, and action choices: [r] Run, [p] Plan Only, [g] Generate, [Esc] Cancel |
| History | `views/history.rs` | Past execution browser |
| Settings | `views/settings.rs` | Configuration panel |
| Templates | `views/templates.rs` | Template list/show |
| Clarification | `views/clarification.rs` | LLM clarification requests |
| Diff | `views/diff.rs` | Plan comparison side-by-side |

## Widgets

| Widget | File | Description |
|--------|------|-------------|
| Command Bar | `widgets/command_bar.rs` | Persistent text input at bottom of screen |
| DAG Tree | `widgets/dag_tree.rs` | Tree visualization of execution DAG |
| Progress Bar | `widgets/progress_bar.rs` | Styled progress with percentage |
| Modal | `widgets/modal.rs` | Confirmation dialogs (cancel, retry) |
| Status Bar | `widgets/status_bar.rs` | Global header with execution info |
| Event Log | `widgets/event_log.rs` | Filterable event timeline |
| Keybind Hints | `widgets/keybind_hint.rs` | Bottom-bar keyboard hints |
| Tool Output | `widgets/tool_output.rs` | Full tool output display |

## Adaptive Layout

| Mode | Min Terminal Size | Layout |
|------|-------------------|--------|
| Compact | 80×24 | Single column: node status list + status bar + command bar |
| Standard | 120×30 | Two columns: DAG tree + details. Event log below. Command bar always at bottom |
| Full | 160×40 | Three columns: DAG tree + details + metrics. Event log below. Command bar always at bottom |

The command bar is **always visible** at the bottom of the screen regardless of layout mode.

## Input System

| Module | File | Description |
|--------|------|-------------|
| Handler | `input/handler.rs` | Keyboard event dispatch |
| Keymap | `input/keymap.rs` | Key binding configuration |
| Command Palette | `input/command_palette.rs` | Fuzzy-find `/commands` |
| Text Entry | `input/text_entry.rs` | Command bar text input |

### Key Bindings

#### Global

| Key | Action |
|-----|--------|
| `Esc` | Focus command bar / clear input |
| `Enter` | Execute command bar input |
| `Tab` | Cycle views |
| `q` | Quit (confirm if execution running) |
| `F1` | Show help overlay |
| `Ctrl+C` | Cancel execution (first = graceful, second = immediate) |
| `↑` / `↓` | Command bar history navigation |

#### Command Bar Focused

| Key | Action |
|-----|--------|
| Type text | Enter intent or `/command` |
| `↲` (Enter) | Execute intent / command |
| `↑` / `↓` | Navigate command history |
| `Tab` | Autocomplete `/commands` |
| `Esc` | Blur command bar (focus view) |

#### Plan Preview (intent submitted)

| Key | Action |
|-----|--------|
| `r` | **Run** — execute the plan with real-time dashboard |
| `p` | **Plan Only** — show plan details, return to command bar (no execution) |
| `g` | **Generate** — save as reusable template to `.rigorix/templates/` |
| `d` | View diff against previous execution |
| `e` | Edit parameters |
| `Esc` | **Cancel** — discard plan, return to command bar |
| `↑` / `↓` | Navigate nodes in plan |

#### Dashboard (execution in progress)

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate nodes in tree |
| `Enter` | Expand/collapse node dependencies |
| `d` | Show node detail panel |
| `o` | Show full tool output |
| `Space` | Toggle node collapse/expand |

#### Events (view focused)

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll events |
| `g` / `G` | Go to top/bottom |
| `1`–`5` | Filter by event type |
| `/` | Search events by keyword |

## Components

### CommandBar

status: planned
depends: none
**Purpose:** Persistent text input at screen bottom for intents and `/commands`. Supports autocomplete, history navigation, and text entry with fuzzy-find command palette.

### PlanReview

status: planned
depends: none
**Purpose:** Plan preview view showing template, confidence, DAG nodes, parameters, and action choices ([r] Run, [p] Plan Only, [g] Generate, [Esc] Cancel). Every intent shows plan preview first (plan-first principle).

### EventBridge

status: planned
depends: none
**Purpose:** Async subscriber converting engine `ExecutionEvent`s into `ViewModelMutation`s via the engine's EventBus broadcast channel. Handles lag (trigger reconciliation at ≥ 50 dropped events) and reverse-channel `TuiCommand` messages.

### ViewModel

status: planned
depends: EventBridge
**Purpose:** Root state model with double-buffering — `TuiViewModel` (execution phase, DAG tree, event log, metrics, budget), `DagViewModel` (node tree), `NodeViewModel`, `MetricsViewModel`, `LlmBudgetViewModel`. Write buffer written by EventBridge, read buffer consumed by render loop.

### Renderer

status: planned
depends: ViewModel
**Purpose:** ratatui widget rendering: DAG Tree, Progress Bar, Status Bar, Event Log, Modal, Keybind Hints, Tool Output. Adaptive layout (Compact 80×24, Standard 120×30, Full 160×40). Command bar always visible at bottom.

### Views

status: planned
depends: Renderer
**Purpose:** Full-screen view implementations: Dashboard (DAG tree + details + metrics), Nodes (node list table), Events (filterable timeline), Plan (preview with actions), History (past execution browser), Settings, Templates, Clarification, Diff (plan comparison).

### InputHandler

status: planned
depends: none
**Purpose:** Keyboard event dispatch with global key bindings (Esc, Tab, q, F1, Ctrl+C), command-bar-focused bindings, plan preview bindings, and view-specific bindings. Also handles `:q`, `:cancel`, `:cancel!` commands.

### OrchestratorSpawner

status: planned
depends: none
**Purpose:** Spawns engine orchestrator in a `tokio::spawn`'d background task. Builds same `OrchestratorBuilder` as cli_boundary. Manages lifecycle — start, graceful cancel, immediate abort — keeping UI responsive.

## Performance Budgets

| Metric | Target |
|--------|--------|
| Render frame time | < 33ms (30fps) |
| Event processing latency | < 5ms |
| Memory usage | < 50MB (10K event log) |
| Startup time | < 500ms |
| State load from disk | < 200ms |
| Orchestrator spawn latency | < 100ms |

## Engine Surface

The TUI dispatches to the same engine services as `cli_boundary`. Unlike the original read-only TUI (which only consumed EventBus), the interactive TUI needs write access:

| Engine Service | Used For |
|----------------|----------|
| `orchestrator::OrchestratorService` | `run()`, `plan_only()`, `cancel()`, `status()`, `event_bus()` |
| `state_persistence::StateManagerService` | Load past executions, list history |
| `templates::TemplateEngine` | List/show templates |
| `template_generation::TemplateGenerator` | Generate templates |
| `audit::AuditService` | Browse audit trails |
| `event_system::EventBusService` | Subscribe to live events |

The TUI builds its own `OrchestratorBuilder` (same as `cli_boundary`) and spawns the orchestrator in a `tokio::spawn`'d task. The `OrchestratorService::event_bus()` provides the subscription for the EventBridge.

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| Command Bar | Persistent text input at screen bottom for intents and commands |
| EventBridge | Async subscriber converting engine ExecutionEvents into ViewModel mutations |
| TuiViewModel | Root state model: execution phase, DAG tree, event log, metrics, budget |
| DagViewModel | Tree-ordered node state for DAG visualization |
| TuiCommand | Reverse-channel command from TUI to orchestrator (Cancel, RetryNode) |

## Dependencies

- Depends on: `rigorix-engine` crate (orchestrator, event_system, state_persistence, templates, template_generation, audit)
- Depends on: `ratatui` + `crossterm` (terminal rendering)
- Depends on: `parking_lot` (double-buffered RwLock)
- Shares: `CliConfig` with `cli_boundary/` module

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-003 | Ratatui TUI | Accepted |
| ADR-007 | Ephemeral CLI — No Daemon for v1 | Accepted |
