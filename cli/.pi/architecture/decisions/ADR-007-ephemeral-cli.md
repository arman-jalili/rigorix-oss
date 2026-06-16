# ADR-007: Ephemeral CLI (No Daemon for v1)

**Status:** Accepted
**Date:** 2026-06-16

## Context

Should `rigorix` run as an ephemeral CLI process (run, exit) or as a background daemon with a client CLI?

## Decision

**Ephemeral CLI for v1.** Each `rigorix run` or `rigorix plan` is a single process that exits when done.

### v1 Approach

- Process starts, loads config, runs execution, renders output, exits
- Crash recovery: on startup, detect stale `.rigorix/state/` files from a killed run and offer to resume
- Fast startup: mtime-cached config parsing
- State is persisted via atomic write-rename — no daemon needed for crash safety

### v2 Consideration: `rigorix daemon`

- Watch mode (`rigorix watch`) re-runs plan on file changes
- IDE integration via LSP-like protocol
- Faster subsequent runs (state kept in memory)

## Rationale

1. **Engine already handles crash safety** — atomic write-rename means state survives process death
2. **Daemon adds complexity** — Unix sockets, IPC protocol, process lifecycle management, health checks
3. **Debugging is harder** — two processes, log correlation
4. **Premature optimization** — until there's demand for watch mode or IDE integration, the overhead isn't justified

## Alternatives

| Alternative | Reason Rejected |
|-------------|----------------|
| Daemon mode with socket IPC | Significant complexity. Deferred to v2. |
| Long-lived REPL | Not needed for CI/CD pipeline use case |
| In-process caching only | Already achieved via mtime-based config caching |

*Affects: CLI Boundary*
