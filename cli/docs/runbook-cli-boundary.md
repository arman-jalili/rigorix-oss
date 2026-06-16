# Runbook: CLI Boundary Module

> **Module:** `cli/src/cli_boundary/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16

## Overview

The CLI boundary is the entry point for all user-facing commands. It handles command parsing, config loading, signal handling, tracing init, template dispatch, and output formatting.

## Architecture

```
User → CliArgs (clap) → dispatch_command()
          ↓
    CliOrchestrator (run/plan/generate)
    TemplateCommandHandler (template list/show)
    LogFormatter (pretty/json/quiet output)
```

## Startup Sequence

1. CLI args parsed via clap
2. Config loaded (flags > env > file > defaults)
3. API key validated for LLM commands
4. Engine ConfigService initialized
5. Tracing initialized
6. Signal handler installed
7. Command dispatched → output formatted → exit

## Graceful Shutdown

Ctrl+C → SignalHandler → `ShutdownLevel::Graceful` → orchestrator stops accepting work.
Double Ctrl+C within 2s → `ShutdownLevel::Immediate` → abort all in-flight.

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| Missing config | ConfigNotFound error | Run `rigorix init` |
| Missing API key | MissingConfig error | Set RIGORIX_API_KEY |
| Unknown command | UnknownCommand error | Check `rigorix --help` |
| Engine error | Engine error passthrough | Check engine status |
