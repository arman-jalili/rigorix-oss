# Ubiquitous Language — CLI Boundary

> Canonical glossary for the **rigorix-cli** crate.
> Engine terms (DAG, PlanningHash, AuditEnvelope, etc.) are defined in `engine/.pi/domain/ubiquitous-language.md`.
> This file covers only CLI-specific terms.

## CLI Terms

| Term | Definition | Aliases/Synonyms | Examples |
|------|-----------|-----------------|---------|
| CliBoundary | The CLI crate's single module: command parsing, TUI, config, signals, output formatting | shell, interface-layer | `rigorix run "add auth middleware"` |
| CliCommand | Parsed CLI command: Run, Plan, Init, Generate, History, Logs, Audit, Template | command, subcommand | `CliCommand::Run { intent: "..." }` |
| CliError | CLI error type mapping engine errors to exit codes | error | Maps `CoreOrchestratorError` to exit codes 0-137 |
| LogFormatter | Formats engine output as Pretty (human), JSON (CI/CD), or Quiet | formatter, output | `LogFormatter::format_run(result)` |
| TuiRenderer | Ratatui-based terminal UI subscribed to engine EventBus | terminal-ui, tui | Subscribes to `ExecutionEvent`, renders node graph |
| ExecutionSession | A single CLI-managed execution run linking engine execution_id to CLI session metadata | session, run | Created on `rigorix run` |
| CliConfig | CLI-specific config settings: output format, log level, TUI enabled, color mode | config, settings | Merged from flags + env + toml |
| SignalHandler | Ctrl+C double-press detection forwarding to engine CancellationService | signal, shutdown | Single Ctrl+C = Graceful, Double = Immediate |

## Engine Terms Used by CLI (referenced, not owned)

| Term | Engine Source | How CLI Uses It |
|------|--------------|-----------------|
| `Config` | `engine::configuration::domain::Config` | Passed to after loading/merging |
| `ExecutionEvent` | `engine::event_system::domain::ExecutionEvent` | Subscribed for TUI rendering |
| `CancellationToken` | `engine::cancellation::domain::CancellationToken` | Forwarded signal triggers cancellation |
| `CoreOrchestratorError` | `engine::error::CoreOrchestratorError` | Mapped to exit codes via CliError |
| `PlanningResult` | `engine::planning::domain::result::PlanningResult` | Displayed by LogFormatter |
| `ExecutionResult` | `engine::execution_engine::domain::ExecutionResult` | Displayed by LogFormatter |
| `AuditEnvelope` | `engine::audit::domain::AuditEnvelope` | Displayed by `rigorix audit` command |
| `ExecutionState` | `engine::state_persistence::domain::ExecutionState` | Displayed by `rigorix history` command |

## Adding New Terms

1. Identify the term used in conversation and code
2. If it describes **engine** behavior, add it to `engine/.pi/domain/ubiquitous-language.md`
3. If it describes **CLI-only** behavior, add it to this file
4. Run `.pi/scripts/validate-ubiquitous-language.sh` to detect drift
