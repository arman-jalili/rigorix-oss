# rigorix CLI — Disaster Recovery Plan

> **Canonical:** `.pi/architecture/modules/cli-boundary.md`
> **Last updated:** 2026-06-16

## Overview

The `rigorix` CLI is an ephemeral binary. It does not run as a daemon, so
traditional server DR patterns (failover, hot standby) do not apply. Recovery
focuses on: state persistence, configuration recovery, and process management.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | < 5 minutes | Reinstall + reconfigure |
| RPO (Recovery Point Objective) | < 1 execution | State persisted atomically |
| Max data loss | Last in-flight execution | Atomic writes prevent corruption |

## Backup Strategy

### What to Back Up
| Artifact | Location | Frequency |
|----------|----------|-----------|
| CLI binary | `cargo build --release` | Per release |
| Config file | `.rigorix/config.toml` or `./rigorix.toml` | On change |
| State files | `.rigorix/state/` | On each execution |
| Templates | `.rigorix/templates/` | On generation |
| Engine state | `~/.rigorix/` | On each execution |

### Backup Command
```bash
# Backup CLI configuration and state
tar czf rigorix-backup-$(date +%Y%m%d).tar.gz \
    .rigorix/ \
    rigorix.toml \
    --exclude='.rigorix/state/*.tmp'
```

### CI/CD Artifact Backup
For CI/CD usage, cache the built binary:
```bash
cargo build --release --package rigorix
cp target/release/rigorix /backup/rigorix-$(git rev-parse --short HEAD)
```

## Restore Procedure

### Full Restore
```bash
# 1. Install Rust toolchain (if not present)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Checkout the matching engine version
git checkout <commit-hash>
cargo build --release --package rigorix

# 3. Restore configuration
tar xzf rigorix-backup-*.tar.gz

# 4. Verify installation
./target/release/rigorix --version
./target/release/rigorix template list
```

### Configuration Recovery
```bash
# Quick config setup
rigorix init --non-interactive --api-key "$RIGORIX_API_KEY"

# Or restore from backup
cp /backup/rigorix.toml ./rigorix.toml
```

### State Recovery
State files are self-healing — atomic write-rename ensures consistency:
```bash
# List past sessions
rigorix history

# Resume an interrupted session (future feature)
# rigorix resume <session-id>
```

## Failure Scenarios

### Scenario 1: Binary Corrupted
- **Symptom:** `rigorix: command not found` or segfault
- **Recovery:** `cargo build --release --package rigorix`
- **Prevention:** CI/CD artifact caching; pinned release versions

### Scenario 2: Config File Lost
- **Symptom:** `Error: No configuration found`
- **Recovery:** `rigorix init` or restore from backup
- **Prevention:** Store config in version control (without secrets)

### Scenario 3: State File Corruption
- **Symptom:** `rigorix history` shows garbled sessions
- **Recovery:** Delete `~/.rigorix/state/*.json` (non-critical cache)
- **Prevention:** Atomic write-rename prevents in-flight corruption

### Scenario 4: Engine Version Mismatch
- **Symptom:** `Error: Engine error` with version-related message
- **Recovery:** Rebuild with correct engine version
- **Prevention:** Workspace ensures CLI and engine versions are in sync

### Scenario 5: Process Killed Mid-Execution
- **Symptom:** SIGKILL or power loss during `rigorix run`
- **Recovery:** Check `rigorix history` for partial results; restart execution
- **Prevention:** Engine uses atomic writes; in-flight state is recoverable

## Verification

After any recovery procedure, run:
```bash
rigorix --version
rigorix template list
rigorix plan "read README" --dry-run
```
All commands should complete without errors.

## Testing the DR Plan

```bash
# Simulate failure scenarios:
# 1. Binary loss
mv target/release/rigorix /tmp/
cargo build --release --package rigorix

# 2. Config loss
mv rigorix.toml /tmp/
rigorix init --non-interactive

# 3. State corruption
rm -rf .rigorix/state/
rigorix history  # Should return empty gracefully
```
