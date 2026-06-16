# TUI — Runbook

## Startup Sequence

1. **Tracing Init**: `cli_boundary::tracing::init_tracing()` — reads `RIGORIX_LOG` env var
2. **Config Load**: `cli_boundary::config::load_config()` — merges TOML + env + defaults
3. **Signal Handler**: `cli_boundary::signal::install_signal_handler()` — Ctrl+C/SIGTERM
4. **Parse Args**: `cli_boundary::cli::parse_args()` — defaults to `Tui` when no subcommand
5. **Terminal Init**: Enable raw mode, enter alternate screen, capture mouse
6. **Render Loop**: Draw frame → poll input (100ms timeout) → handle action → redraw

## Startup Modes

| Mode | Trigger | Behaviour |
|------|---------|-----------|
| Fresh start | `rigorix` (no args) | Dashboard with empty state, command bar focused |
| View past | `rigorix tui --exec <id>` | Load execution from disk into read-only mode |
| Run from CLI | `rigorix tui --run "<intent>"` | Launch TUI with orchestrator already running |

## Shutdown Sequence

| Trigger | Behaviour |
|---------|-----------|
| `:q` + Enter | Exit render loop, restore terminal, return |
| Ctrl+C (single) | Graceful: finish in-flight node, cancel orchestrator |
| Ctrl+C (double) | Immediate: abort all in-flight nodes |
| SIGTERM | Immediate abort, exit 137 |

## View Navigation

| Key | Action |
|-----|--------|
| Tab | Cycle views: Dashboard → Nodes → Events → History |
| Esc | Toggle command bar focus |
| h / l | Previous / next view (in Dashboard) |

## Common Failure Modes

| Symptom | Likely Cause | Resolution |
|---------|-------------|-----------|
| Terminal garbled after exit | Raw mode not disabled | Run `reset` in shell |
| TUI won't start | Terminal too small (< 80×24) | Enlarge terminal window |
| Input not responding | Command bar not focused | Press Esc to focus |
| Slow rendering | Terminal emulator throttling | Reduce terminal font size |
| Panic on startup | Missing crossterm features | Ensure TERM env var is set |

## Performance Budgets

| Metric | Target | Enforcement |
|--------|--------|-------------|
| Render frame time | < 33ms (30fps) | Soft (warn if exceeded) |
| Event processing | < 5ms | Soft |
| Memory usage | < 50MB (10K events) | Hard (abort if exceeded) |
| Startup time | < 500ms | Soft |
