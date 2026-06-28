# Runbook — Rigorix CLI

## Incident Response

| Severity | Response Time | Escalation Path |
|----------|--------------|-----------------|
| P1 (Critical) | < 15 min | On-call → Engineering Lead |
| P2 (High) | < 1 hour | On-call → Engineering Lead |
| P3 (Medium) | < 4 hours | Engineering Team |
| P4 (Low) | Next business day | Issue triage |

## Common Incidents

### CLI Panic or Crash
1. Run `RUST_BACKTRACE=1 rigorix <command>` to get full traceback
2. Check `~/.rigorix/logs/` for recent logs
3. Verify config file at `~/.rigorix/config.toml`

### TUI Display Issues
1. Check terminal size: `tput lines && tput cols`
2. Verify terminal emulator supports true color: check `TERM` and `COLORTERM` env vars
3. Try `rigorix --no-tui` to run in CLI-only mode

### Command Not Found
1. Verify installation: `which rigorix`
2. Check PATH: `echo $PATH`
3. Rebuild from source: `cd cli && cargo build`

## Rollback Procedure

1. `git revert HEAD` to undo last commit
2. `cargo build -p rigorix-cli --release` to rebuild
3. Verify: `rigorix --version`

## On-Call

- Primary: @arman-jalili
- Escalation: Create GitHub issue with `incident` label
