# Configuration Module DR Plan

> **Last updated:** 2026-06-13
> **Module:** Configuration (`engine/src/configuration/`)
> **RTO:** < 1 minute (restart)
> **RPO:** 0 (no mutable state — reads config at startup)

## System State

The configuration module is **stateless** — it reads configuration at startup and
serves it read-only for the duration of the process. There is no mutable state
to recover.

| State Type | Storage | Persistence |
|-----------|---------|------------|
| Config file | `rigorix.toml` (filesystem) | Persistent |
| Env overrides | `RIGORIX__*` env vars | Set at process start |
| CLI overrides | CLI arguments | Set at process start |
| Cached config (optional) | `~/.rigorix/config.cache` | Atomic write-rename |

## Backup Strategy

| Asset | Frequency | Method | Retention |
|-------|-----------|--------|-----------|
| `rigorix.toml` | Per-deployment | Git-tracked (part of repo) | Git history |
| `~/.rigorix/config.toml` | Per-user | Manual backup | N/A |

## Restore Procedure

1. **Checkout the correct version** of `rigorix.toml` from git
2. **Set environment variables** for any runtime-specific overrides
3. **Restart the process**

```bash
# 1. Restore config from git
git checkout <tag> -- rigorix.toml

# 2. Set required env vars
export ANTHROPIC_API_KEY="<key>"
export RIGORIX__LOGGING__LEVEL="info"

# 3. Restart
cargo run
```

## Failover Plan

Configuration is loaded per-process. In multi-instance deployments:

1. Each instance loads its own config at startup
2. Config files should be identical across instances (deployed via CI/CD)
3. Blue/green deployments: new instances validate config before accepting traffic
4. No shared config state between instances

## RTO/RPO

| Metric | Target | Notes |
|--------|--------|-------|
| RTO | < 1 minute | Time to restart process with correct config |
| RPO | 0 | No mutable state in config module |
