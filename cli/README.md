# rigorix-cli

**The `rigorix` binary — a thin wrapper over `rigorix-engine`.** It holds no
business logic of its own: every command resolves a config, dispatches to an
engine service, and formats the result. That is deliberate. A CLI that reimplements
a rule is a second source of truth, and the engine's rules are the product.

```bash
cargo install rigorix-cli
```

Running `rigorix` with no arguments opens the **interactive TUI**. Every command
below is also available non-interactively for CI and scripting.

---

## Commands

### Lifecycle

| Command | Description |
|---|---|
| `rigorix run <intent>` | Full lifecycle: plan → execute → persist → emit → record |
| `rigorix plan <intent>` | Plan only — preview the generated plan without executing |
| `rigorix cancel <id>` | Cancel a running execution (graceful, then immediate) |
| `rigorix status` | Current or most recent execution status |

`plan` before `run` is the intended workflow: the plan is the reviewable artifact,
and it is where operator requirements and sequence rules are already applied.

### Inspection

| Command | Description |
|---|---|
| `rigorix history [--limit N] [--status S]` | List past executions |
| `rigorix explain <id> [--diff <id2>]` | Detailed execution info, optionally compared |
| `rigorix diff-plan <id1> <id2>` | Compare two plans side by side |
| `rigorix logs [--session-id <id>]` | Raw execution session logs |
| `rigorix template {list\|show <id>}` | Browse or inspect templates |
| `rigorix generate <intent>` | Generate a reusable template from an intent |

### Audit — including offline verification

| Command | Description |
|---|---|
| `rigorix audit list` / `show` / `diff` | Browse the signed audit trail |
| `rigorix audit projection [--subject <s>]... [-o <file>]` | Emit an authority-fact projection bundle |
| `rigorix audit verify-projection <bundle>` | **Verify a bundle offline by re-derivation** |

`audit verify-projection` is the one to look at first. It needs **no engine, no
session and no key**: it reads the bundle, re-derives the projection from the
bundled signed history, and exits non-zero if the two disagree or the history is
not an intact chain. That is the whole point of the export — a third party checks
it themselves (spec §4.3.1).

```bash
rigorix audit projection --subject authority:beneficiary-eligible -o bundle.json
rigorix audit verify-projection bundle.json     # exit 0 = verified
```

Note the disclosure: a bundle carries the envelopes. The consumer receives the
signed history, not a summary.

### Setup

| Command | Description |
|---|---|
| `rigorix init` | Scaffold `.rigorix/` with a default `rigorix.toml` |
| `rigorix config {init\|show\|validate}` | Manage and validate configuration |
| `rigorix key [--label NAME]` | Generate API keys |

---

## Output formats

Every command accepts `--format`:

| Format | Use |
|---|---|
| `pretty` *(default)* | Human-readable, Unicode, for a terminal |
| `json` | Machine-readable, for CI and scripting |
| `markdown` | Documentation output |
| `quiet` | Minimal output; the exit code is the interface |

Exit codes are the contract in `quiet` mode: non-zero means the operation did not
succeed — including a projection that failed verification.

---

## Configuration

`rigorix init` scaffolds `.rigorix/`:

| File | Owner | Purpose |
|---|---|---|
| `rigorix.toml` | operator | Engine configuration and the audit HMAC key |
| `models.json` | operator | Model definitions and selection |
| `policy.toml` | operator | Declarative policy |
| `permissions.toml` | operator | Path and capability rules |
| `sequence-policy.toml` | operator | Sequence rules and operator step requirements (ADR-013/015) |
| `preconditions.toml` | operator | Dispatch-time preconditions (ADR-017) |

**`.rigorix/` is a trust surface.** The default permission config denies executing
agents write access to `.rigorix/**` — otherwise an agent could rewrite the rules
it is judged by. Treat these files as you would a CI configuration or a
branch-protection rule.

---

## Architecture

2 modules, Clean Architecture with frozen contracts:

| Module | Responsibility |
|---|---|
| `cli_boundary` | Clap command surface, config resolution, dispatch, output formatting, signal handling |
| `tui` | The ratatui interactive shell: plan review, command bar, event bridge, view models |

```
cli/src
├── cli_boundary/   cli.rs (surface) · dispatch.rs (routing) · config.rs
│                   output.rs (pretty/json/markdown/quiet) · error.rs · signal.rs
└── tui/            views/ · widgets/ · input/ · plan_review.rs · event_bridge.rs
```

`cli_boundary` is the only place that knows about argv. Everything below it takes
typed arguments and returns typed results, which is why the whole command surface
is unit-testable without spawning a process — and the projection round-trip *is*
tested by spawning the real binary in an empty temp directory
(`cli/tests/projection_surface.rs`).

---

## Development

```bash
cargo test -p rigorix-cli
cargo clippy -p rigorix-cli -- -D warnings
cargo run -p rigorix-cli -- --help
cargo run -p rigorix-cli -- audit --help
```

Architecture: [`.pi/architecture/modules/`](.pi/architecture/modules/).

---

## License

Licensed under either of MIT ([LICENSE-MIT](../LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](../LICENSE-APACHE)), at your option.
