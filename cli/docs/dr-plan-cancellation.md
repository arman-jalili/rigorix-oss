# Disaster Recovery Plan: Cancellation Module

> **Module:** `cli/src/cancellation/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16
> **RTO:** < 1 minute (stateless signal handler)
> **RPO:** N/A (no mutable state)

## Overview

The Cancellation module is stateless — it registers an OS signal handler that
forwards signals to a watch channel. There is no persistent state, no database,
and no long-lived connections. Recovery is trivial: restart the CLI.

## Failure Scenarios

### Scenario 1: Signal handler fails to install

**Symptom:** `CliError::SignalHandlerError` on startup, CLI exits

**Impact:** No Ctrl+C handling. User cannot gracefully stop the CLI.

**Recovery:**
1. Check if another process is already listening on SIGINT
2. Restart the CLI — signal handler install should succeed on retry
3. If persistent: check tokio runtime availability

**Verification:** Ctrl+C gracefully stops the CLI

### Scenario 2: Signal handler task panics

**Symptom:** Stderr shows panic message, SIGINT ignored

**Impact:** Ctrl+C does nothing, process must be killed with SIGKILL

**Recovery:**
1. Force kill: `kill -9 <PID>`
2. Restart the CLI
3. If reproducible: investigate signal_impl.rs for race conditions

**Verification:** Ctrl+C works after restart

### Scenario 3: Double-press detection fails

**Symptom:** Single press triggers graceful shutdown correctly, but second press
within window doesn't escalate to immediate abort

**Impact:** Tasks with long timeouts (30s+) cannot be force-aborted

**Recovery:**
1. Kill the process externally: `kill -9 <PID>`
2. File a bug with reproduction steps (rapid double Ctrl+C)

**Verification:** Double Ctrl+C within 2s triggers immediate abort

### Scenario 4: Watch channel receiver dropped

**Symptom:** Signals are received but no component reacts

**Impact:** All signals are silently ignored

**Recovery:**
1. Force kill: `kill -9 <PID>`
2. Check that the orchestrator holds the receiver
3. Fix code that drops the watch::Receiver

**Verification:** Signal is propagated after fix

## Backup Strategy

| Asset | Backup Method | Frequency | Retention |
|-------|--------------|-----------|-----------|
| Signal handler config (window) | Git repository | Every commit | Full git history |

No other assets require backup — the cancellation module has no persistent state.

## Restore Procedure

```bash
# Restore signal handler configuration
git checkout <commit> -- cli/src/cancellation/infrastructure/signal_impl.rs

# Rebuild
cd cli && cargo build
```

## Failover Plan

The cancellation module has no failover mechanism needed — it is stateless and
ephemeral. Each CLI process has its own independent signal handler.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO | < 1 minute | Stateless — just restart CLI |
| RPO | N/A | No mutable state to lose |

## Testing the DR Plan

```bash
# Run cancellation unit tests
cargo test -p rigorix-cli -- cancellation

# Verify signal handler creates
cargo run -- --help  # SignalHandlerImpl::new() called in main()
```
