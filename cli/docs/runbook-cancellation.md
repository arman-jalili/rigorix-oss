# Runbook: Cancellation Module

> **Module:** `cli/src/cancellation/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16

## Overview

The Cancellation module captures Ctrl+C (SIGINT) with double-press detection and
converts them into structured shutdown requests. It provides the CLI-side signal
handling that forwards to the engine's `CancellationService`.

## Architecture

```
User Ctrl+C → OS Signal → SignalHandlerImpl
                              ↓
                    watch::Receiver<ShutdownLevel>
                              ↓
                    dispatch_command() / orchestrator
                              ↓
                    Engine::CancellationService
```

Two-level shutdown:
- **Single Ctrl+C** → `ShutdownLevel::Graceful` — finish current work, stop accepting new tasks
- **Double Ctrl+C within 2s** → `ShutdownLevel::Immediate` — abort all in-flight work

## Startup Sequence

1. `main()` creates `SignalHandlerImpl::new()` with default 2s double-press window
2. `signal_handler.install()` spawns a tokio task listening for SIGINT
3. Returns a `watch::Receiver<ShutdownLevel>` for the orchestrator to poll

## Graceful Shutdown

Single Ctrl+C triggers:
1. `ShutdownLevel::Graceful` sent to watch channel
2. Orchestrator reads signal, stops accepting new work
3. In-flight tasks get a 30s timeout to complete
4. If second Ctrl+C within 2s → immediate abort

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| Signal handler not installed | No Ctrl+C response early in startup | Move `install()` call before task spawning |
| Double-press misses window | Shutdown takes full graceful timeout | Press Ctrl+C twice quickly (< 2s apart) |
| Signal handler task panics | `eprintln!` on stderr | Check tokio runtime, restart CLI |

## Configuration Reference

| Setting | Source | Default | Description |
|---------|--------|---------|-------------|
| `double_press_window` | `SignalHandlerImpl::with_window()` | 2s | Window for double-press detection |
| graceful timeout | `GracefulShutdownInput::timeout_secs` | 30s | Max wait for in-flight tasks |

## Troubleshooting

### Shutdown not responding
```bash
# Check if process is hung
kill -INT <PID>  # Send SIGINT manually
kill -TERM <PID> # Force termination if unresponsive
```

### Double-press not working
The 2s default window is conservative. Use `SignalHandlerImpl::with_window(5)`
for a more forgiving window, or reduce to 1s for faster abort response.
