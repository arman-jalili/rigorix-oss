# Disaster Recovery Plan: repo-engine Module

<!--
Canonical Reference: .pi/architecture/modules/repo-engine.md
Last Updated: 2026-06-14
-->

## Scope

This DR plan covers the `repo-engine` module — the multi-language code indexing
and symbol graph management system. The module indexes source files into an in-memory
symbol graph with O(1) lookups. Since the graph is ephemeral (rebuilt on each startup),
the primary risk is indexed symbol data loss and indexing corruption.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 5 minutes | Symbol graph can be rebuilt from source files on restart |
| RPO (Recovery Point Objective) | < 1 hour | Symbol graph is ephemeral; source files are the source of truth |

## Backup Strategy

### What to Back Up

The repo-engine module has no persistent runtime state. The symbol graph is rebuilt
from source files on each startup. However, optional cache files and configuration
should be backed up:

| Item | Location | Backup Priority |
|------|----------|-----------------|
| Source code files | Project repository | High — symbols are extracted from these files |
| Repo-engine source code | `src/repo_engine/` | High — source of truth for all interfaces |
| Architecture module docs | `.pi/architecture/modules/repo-engine.md` | Medium — restore module design context |
| Contract freeze files | `src/repo_engine/` | High — frozen contracts for all interfaces |
| Runbook and DR plan | `docs/runbook-repo-engine.md`, `docs/dr-plan-repo-engine.md` | Medium — operational procedures |
| Proofing scripts | `.pi/scripts/ci/check_repo-engine_*.sh` | Medium — CI validation |
| Optional symbol cache (if configured) | `.rigorix/symbol_cache/` | Low — rebuild from source |

### Backup Schedule

| Type | Frequency | Retention | Method |
|------|-----------|-----------|--------|
| Version control | Every commit | Indefinite | Git — all source code and configuration |
| Periodic archive | Weekly | 90 days | Git bundle of repo-engine source and config |
| Symbol cache (optional) | Daily | 7 days | JSON export of symbol graph |

### Backup Automation

```bash
#!/usr/bin/env bash
# Backup repo-engine configuration and symbol cache
BACKUP_DIR="/var/backups/rigorix/repo-engine"
mkdir -p "$BACKUP_DIR"

# Backup source code
git archive HEAD:engine/src/repo_engine/ --format=tar | tar -x -C "$BACKUP_DIR/src"

# Backup CI scripts
git archive HEAD:.pi/scripts/ci/check_repo-engine_* --format=tar | tar -x -C "$BACKUP_DIR/ci"

# Backup docs
cp engine/docs/runbook-repo-engine.md "$BACKUP_DIR/"
cp engine/docs/dr-plan-repo-engine.md "$BACKUP_DIR/"

echo "Repo-engine backed up to $BACKUP_DIR at $(date)"
```

## Restore Procedure

### Scenario 1: Symbol Graph Corrupted

**Severity:** Low — graph is rebuilt on restart

1. Restart the orchestrator process — the symbol graph is rebuilt from source files
2. If a symbol cache exists, it may be stale; re-indexing is always preferred
3. Verify the graph by running a sample lookup:
   ```rust
   let lookup = graph_service.lookup_symbol(LookupSymbolInput {
       name: "known_symbol".to_string(),
       include_adjacency: false,
       reference_depth: 0,
   }).await;
   assert!(lookup.found);
   ```

### Scenario 2: Source Files Lost or Corrupted

**Severity:** High — symbols cannot be extracted without source files

1. Restore source files from Git:
   ```bash
   git checkout HEAD -- src/
   ```
2. Re-run indexing via `IndexerService::index_directory()`
3. Verify the restored graph has the expected symbol count

### Scenario 3: Tree-Sitter Grammar Failure

**Severity:** Medium — one language may be unavailable

1. Check the grammar repository for the failing language
2. Reinstall or rebuild the tree-sitter grammar:
   ```bash
   cargo build  # Rebuilds embedded grammars
   ```
3. Restart the indexing process for the affected language
4. If the grammar cannot be restored, index other languages and skip the
   failing one until the grammar is fixed

### Scenario 4: Full Module Failure

**Severity:** Critical — repo-engine completely unavailable

1. Restore the complete module from version control:
   ```bash
   git checkout HEAD -- engine/src/repo_engine/
   git checkout HEAD -- engine/.pi/architecture/modules/repo-engine.md
   git checkout HEAD -- engine/.pi/scripts/ci/check_repo-engine_*.sh
   ```
2. Rebuild the project:
   ```bash
   cd engine
   cargo build
   ```
3. Run the CI proofing scripts to verify:
   ```bash
   bash .pi/scripts/ci/check_repo-engine_contracts.sh
   bash .pi/scripts/ci/check_repo-engine_coverage.sh
   ```
4. Run full test suite:
   ```bash
   cargo test -p rigorix -- repo_engine
   ```

## Failover Plan

### Multi-Instance Failover

The repo-engine module is designed as a single in-memory graph within a single
process. For high-availability scenarios:

1. **Primary instance** — Handles all indexing and symbol queries
2. **Standby instance** — Maintains a warm copy of the symbol graph via periodic
   symbol cache synchronization
3. **Failover trigger** — Primary instance health check fails (3 consecutive failures)
4. **Failover procedure:**
   - Route traffic to standby instance
   - Standby loads its symbol graph cache
   - If cache is stale, standby re-indexes from source files
   - Verify with health check before accepting traffic

```bash
# Failover script
PRIMARY_HEALTH=$(curl -s -o /dev/null -w "%{http_code}" http://primary:8080/health/repo-engine)
if [ "$PRIMARY_HEALTH" != "200" ]; then
    echo "Primary unhealthy. Failing over to standby..."
    # Route traffic to standby
    kubectl patch service repo-engine -p '{"spec":{"selector":{"instance":"standby"}}}'
    # Wait for standby to be ready
    while [ "$(curl -s -o /dev/null -w '%{http_code}' http://standby:8080/health/repo-engine/ready)" != "200" ]; do
        sleep 1
    done
    echo "Failover complete. Standby is now active."
fi
```

## Disaster Scenarios

| Scenario | Impact | RTO | RPO | Recovery Action |
|----------|--------|-----|-----|-----------------|
| Symbol graph corruption | Lookups return stale/missing data | < 5 min | < 1 hour | Restart process to rebuild graph |
| Source file deletion | Symbols for deleted files lost | < 30 min | < 5 min | Restore from Git |
| Tree-sitter grammar failure | One language cannot be indexed | < 1 hour | N/A | Rebuild or reinstall grammar |
| Full module corruption | All symbol operations fail | < 2 hours | < 5 min | Restore from Git, rebuild, re-index |
| Data center outage | Complete service unavailability | < 15 min | < 1 hour | Failover to standby instance |

## Testing the DR Plan

| Test | Frequency | Success Criteria |
|------|-----------|-----------------|
| Restart rebuild | Every deployment | Graph is populated with expected symbol count within 30 seconds |
| Source restore | Monthly | Full restore from Git completes in < 5 minutes |
| Grammar failure simulation | Quarterly | Indexing continues for other languages when one grammar fails |
| Failover drill | Quarterly | Standby accepts traffic within 15 minutes |
| Full DR test | Bi-annually | Complete recovery within 2 hours |

## Continuous Improvement

1. **Post-mortem after every DR test**: Document what went wrong and update this plan
2. **Automated recovery drills**: Schedule regular DR test runs via CI/CD
3. **Metric-based alerting**: Alert when symbol count drops significantly compared to
   the previous indexing session
