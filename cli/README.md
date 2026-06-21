# rigorix-cli

**Thin binary wrapper around `rigorix-engine` — the user-facing CLI for planning and executing DAG tasks.**

Per [ADR-002](../cli/.pi/architecture/decisions/ADR-002-cli-engine-split.md), the CLI contains **zero business logic**. All execution, planning, and domain logic lives in the engine crate. The CLI is responsible only for:

- Command parsing (Clap)
- Config loading and merging (TOML + env + flags + `models.json`)
- Output formatting (Pretty, JSON, Markdown, Quiet)
- Signal handling (Ctrl+C, SIGTERM)
- Tracing initialization
- Dispatching to engine services

---

## Quick Start

```bash
# Launch interactive TUI (default)
cargo run -p rigorix-cli

# Plan only — preview generated plan
cargo run -p rigorix-cli -- plan "add input validation to the login form"

# Full run — plan + execute + persist
cargo run -p rigorix-cli -- run "refactor database queries to use async"

# Cancel a running execution
cargo run -p rigorix-cli -- cancel <execution-id>

# Show execution status
cargo run -p rigorix-cli -- status
```

---

## Commands

### Tier 1 — Full Lifecycle (via `OrchestratorService`)

| Command | Description |
|---------|-------------|
| `rigorix run <intent>` | Full lifecycle: plan → execute → persist → emit → audit |
| `rigorix plan <intent>` | Plan only — preview the generated plan without executing |
| `rigorix cancel <id>` | Cancel a running execution (graceful then immediate) |
| `rigorix status` | Show current or most recent execution status |

### Tier 2 — Direct Engine Services

| Command | Description |
|---------|-------------|
| `rigorix history [--limit N] [--status S]` | List past executions |
| `rigorix explain <id> [--diff <id2>]` | Detailed execution info with optional comparison |
| `rigorix diff-plan <id1> <id2>` | Compare two plans side-by-side |
| `rigorix generate <intent>` | Generate a reusable template from natural language |
| `rigorix template {list\|show <id>}` | Browse or inspect templates |
| `rigorix audit {list\|show\|diff}` | Browse audit trails |
| `rigorix logs [--session-id <id>]` | View execution event logs |
| `rigorix config {init\|show\|validate}` | Manage configuration |

### Tier 3 — CLI-Only

| Command | Description |
|---------|-------------|
| `rigorix init` | Scaffold `.rigorix/` directory with default `rigorix.toml` |
| `rigorix key [--label NAME]` | Generate API keys |

### Shortcut Flags

| Flag | Expands To |
|------|-----------|
| `--run <intent>` | `rigorix run <intent>` |
| `--exec <id>` | `rigorix tui --exec <id>` |
| `--history` | `rigorix history` |

> When no subcommand or flag is given, the **interactive TUI** launches by default.

---

## Configuration

Config loading follows a layered priority (highest wins):

1. CLI flag overrides
2. Environment variables (`RIGORIX__LLM__API_KEY`, `RIGORIX__ORCHESTRATOR__MAX_PARALLEL_TASKS`, etc.)
3. `rigorix.toml` in CWD (or parent, walking up to repo root)
4. `~/.rigorix/config.toml` (user-level fallback)
5. `~/.rigorix/models.json` or `.rigorix/models.json` (provider URL, max_tokens)
6. Compiled-in defaults

```bash
# Initialize default config
rigorix init

# View merged config (secrets redacted)
rigorix config show

# Validate config against safety caps
rigorix config validate
```

### Models.json

The `models.json` file provides LLM provider configuration (base URLs, model IDs, max tokens). It follows the same format as `~/.pi/agent/models.json`. Example:

```json
{
  "providers": {
    "anthropic": {
      "baseUrl": "https://api.anthropic.com/v1",
      "apiKey": "${ANTHROPIC_API_KEY}",
      "models": [
        {"id": "claude-sonnet-4-6", "maxTokens": 8192}
      ]
    }
  }
}
```

---

## Module Structure

```
cli/src/
├── main.rs                   # Binary entry point
├── lib.rs                    # Library root
│
├── cli_boundary/             # Flag-based CLI
│   ├── cli.rs                # Clap parser: 14 commands + 3 shortcuts
│   ├── dispatch.rs           # Command → engine service routing
│   ├── orchestrator.rs       # OrchestratorBuilder wrapper
│   ├── config.rs             # Multi-source config loader
│   ├── output.rs             # Output formatter trait
│   ├── signal.rs             # Ctrl+C / SIGTERM handler
│   ├── tracing.rs            # tracing-subscriber init
│   ├── error.rs              # CliError → exit codes
│   └── tests.rs              # Integration tests
│
└── tui/                      # Interactive Terminal UI
    ├── views/                # 7 views: Dashboard, Plan, Nodes, Diff, History, Events, Settings
    ├── widgets/              # 6 widgets: StatusBar, CmdBar, DAG Tree, Event Log, Node Detail, Metrics
    ├── input/                # Keymap, command palette
    ├── event_bridge.rs       # Engine events → VM commands
    ├── orchestrator_spawner.rs # Background orchestrator task
    └── view_model.rs         # UI state model
```

---

## Output Formats

All commands support `--format` (default: Pretty):

| Format | Use Case |
|--------|----------|
| `pretty` | Human-readable with Unicode symbols (default) |
| `json` | CI/CD integration and scripting |
| `markdown` | Documentation output |
| `quiet` | Minimal output, exit codes only |

---

## Output Examples

### Plan Output (Pretty)

```
Plan: refactor database queries to use async (confidence 92%)
  Template: async-refactor | LLM: 2 calls, 850 tokens
  Parameters:
    ├── target_module: src/db/
    ├── connection_pool: PgPool
  Graph: 4 node(s), sealed=true
    · analyze (root)
    · rewrite ← [analyze]
    · test ← [rewrite]
    · verify ← [test]
```

### Run Output (Pretty)

```
Run: Completed — 0 failed, 4 passed, 0 skipped (4 total)
  Template: async-refactor | LLM: 2 calls, 850 tokens

  ✓ analyze — Completed
  ✓ rewrite — Completed
  ✓ test — Completed
  ✓ verify — Completed
```

---

## License

MIT OR Apache-2.0
