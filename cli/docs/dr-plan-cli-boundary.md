# CLI Boundary — Disaster Recovery Plan

## RTO/RPO

| Metric | Target |
|--------|--------|
| Recovery Time Objective (RTO) | < 5 minutes |
| Recovery Point Objective (RPO) | N/A (no persistent state in CLI) |

The CLI is a stateless binary — it has no database, no persistent queues,
and no long-lived processes. Recovery means rebuilding the binary and
re-running the command.

## Backup Strategy

The CLI has no persistent state to back up. The following are ephemeral:

| Artifact | Location | Backup? |
|----------|----------|---------|
| Config file | `rigorix.toml` (user-managed) | User-managed |
| API keys | `.rigorix/keys.toml` | User-managed |
| Templates | `.rigorix/templates/` | User-managed |

## Restore Procedure

### From Source

```bash
# 1. Clone repository
git clone <repo-url>
cd rigorix

# 2. Build CLI
cargo build --release -p rigorix-cli

# 3. Restore config
cp /backup/rigorix.toml ./rigorix.toml

# 4. Verify
./target/release/rigorix config validate
```

### From Binary (fallback)

```bash
# Download pre-built binary from CI artifacts
# Verify checksum
# Move to PATH
cp rigorix /usr/local/bin/
rigorix config validate
```

## Failover Plan

The CLI has internal redundancy for all engine operations:

| Service | Fallback | Graceful Degradation |
|---------|----------|---------------------|
| Orchestrator | Engine handles retries | Partial results on failure |
| Event Bus | In-memory buffer | Events lost on crash |
| State | Atomic file writes | Last valid state preserved |

## Testing

| Test | Frequency | Procedure |
|------|-----------|-----------|
| Build | Every PR | `cargo build -p rigorix-cli` |
| Tests | Every PR | `cargo test -p rigorix-cli` |
| Integration | Weekly | Full pipeline run |
| Recovery | Monthly | Clone + build from scratch |
