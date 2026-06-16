# Runbook: Observability Module

> **Module:** `cli/src/observability/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16

## Overview

The Observability module initializes and manages tracing, health checks, and metrics for the CLI boundary. It wraps engine observability contracts with CLI-specific configuration.

## Architecture

```
main() → init_tracing(log_level, log_format)
              ↓
    tracing-subscriber (pretty or JSON)
              ↓
    Engine's observability::init_tracing()
```

## Startup Sequence

1. Config loads → determines log level and format
2. `init_tracing()` called with CLI config values
3. Respects `RIGORIX_LOG` env var override
4. Tracing initialized for all downstream components

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| Tracing already initialized | Panic (tracing-subscriber) | Ensure idempotent init |
| Invalid log level | Falls back to info | Check RIGORIX_LOG value |
| No output | Tracing not initialized | Check startup sequence |

## Configuration Reference

| Setting | Source | Default | Description |
|---------|--------|---------|-------------|
| `log_level` | CLI flag / env / config | `info` | trace, debug, info, warn, error |
| `log_format` | CLI flag | `pretty` | pretty (dev), json (production) |
| `RIGORIX_LOG` | Env var | — | Overrides log_level |
