# TUI — Disaster Recovery Plan

## RTO/RPO

| Metric | Target |
|--------|--------|
| Recovery Time Objective (RTO) | < 2 minutes |
| Recovery Point Objective (RPO) | N/A (no persistent state in TUI) |

The TUI is a stateless terminal application — it has no database, no persistent
queues, and no long-lived processes. Recovery means rebuilding the binary.

## Failure Scenarios

| Scenario | Impact | Recovery |
|----------|--------|----------|
| TUI panic/crash | Terminal left in raw mode | Run `reset` in shell, `stty sane` |
| Ctrl+C during render | Corrupted terminal state | `reset`, or close and reopen terminal |
| OOM (50MB+ events) | Process killed by OOM killer | Reduce max event log size |
| Deadlock on RwLock | UI freezes | SIGTERM, fix contention in next release |

## Backup Strategy

The TUI has no persistent state. Ephemeral data:

| Data | Source | Recovery |
|------|--------|----------|
| Command history | In-memory `command_bar.history` | Lost on exit |
| Execution events | In-memory `event_log` | Lost on exit; engine persists on disk |
| ViewModel state | In-memory `TuiViewModel` | Lost on exit |

## Restore Procedure

### From Source

```bash
git clone <repo-url> && cd rigorix
cargo build --release -p rigorix-cli
./target/release/rigorix
```

### From Binary

```bash
# Download from CI artifacts
curl -L <ci-artifact-url> -o rigorix
chmod +x rigorix && ./rigorix
```

## Testing

| Test | Frequency | Command |
|------|-----------|---------|
| Build | Every PR | `cargo build -p rigorix-cli` |
| Unit tests | Every PR | `cargo test -p rigorix-cli` |
| TUI smoke test | Manual | `cargo run` — verify dashboard renders |
| Key bindings | Manual | Tab cycle, Esc focus, :q quit |
