# Disaster Recovery Plan: failure-parser Module

<!--
Canonical Reference: .pi/architecture/modules/failure-parser.md
Last Updated: 2026-06-19
-->

## Overview

The failure-parser module is stateless — all its logic lives in pure Rust code with no
external dependencies, databases, or persistent state. As such, the disaster recovery
profile is minimal. This plan covers edge cases where parser registrations or custom
configurations need preservation.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | 0 seconds | Module is recreated in-memory at startup |
| RPO (Recovery Point Objective) | N/A | No persistent state to recover |

## Backup Strategy

### What to Back Up

Since the module is stateless, no runtime data needs backup. However, the following
should be version-controlled:

| Asset | Location | Backup Method |
|-------|----------|---------------|
| Source code | `src/failure_parser/` | Git (primary) |
| Architecture docs | `.pi/architecture/modules/failure-parser.md` | Git (primary) |
| Runbook | `docs/runbook-failure-parser.md` | Git (primary) |
| DR plan | `docs/dr-plan-failure-parser.md` | Git (primary) |
| CI scripts | `.pi/scripts/ci/check_failure-parser_*.sh` | Git (primary) |
| Issue tracking | GitHub issue #494 | GitHub |

### Backup Schedule

| Frequency | Action |
|-----------|--------|
| Continuous | Source committed to git on every change |
| Per-sprint | Architecture docs reviewed and updated |

## Restore Procedure

### Full Restore

The module is stateless, so "restore" means re-building from source:

1. Check out the code from git:
   ```bash
   git checkout <tag-or-commit>
   ```
2. Build:
   ```bash
   cd engine && cargo build
   ```
3. Run tests:
   ```bash
   cargo test --lib failure_parser
   cargo test --test failure_parser_template_integration
   cargo test --test failure_parser_service_integration
   cargo test --test failure_parser_typescript_integration
   cargo test --test failure_parser_suggestion_integration
   ```
4. Verify all tests pass (136+ tests)

### Partial Restore (After Code Corruption)

If individual files are corrupted:

1. Restore specific file from git:
   ```bash
   git checkout HEAD -- src/failure_parser/domain/failure.rs
   ```
2. Rebuild and re-test

## Failover Plan

The failure-parser module has no failover requirements — it is not a distributed service.
If the module fails to compile or tests fail:

1. Identify the failing component via test output
2. Check git history for recent changes
3. Revert the problematic commit:
   ```bash
   git revert <problematic-commit-hash>
   ```
4. Rebuild and re-test
5. If the issue persists, escalate to the engineering team

## Degraded Operation

| Scenario | Impact | Mitigation |
|----------|--------|------------|
| TypeScriptParser fails | Can't parse tsc output | Fall back to generic error handling |
| FixSuggestionService unavailable | No suggestions | Failures still classified, just without fixes |
| ParserRegistry empty | All parse attempts fail | Register at least one parser at startup |
| Single parser fails | Only that tool affected | Other parsers continue to work |

## Testing the DR Plan

### Schedule

| Frequency | Test |
|-----------|------|
| Per-release | Clear target directory and full rebuild |
| Per-release | Run all tests (cargo test) |
| Per-epic | Verify integration tests pass |

### Test Procedure

```bash
# 1. Simulate clean checkout
git clone <repo> /tmp/test-restore
cd /tmp/test-restore/engine

# 2. Build
cargo build

# 3. Run all failure-parser tests
cargo test --lib failure_parser
cargo test --test failure_parser_template_integration
cargo test --test failure_parser_service_integration
cargo test --test failure_parser_typescript_integration
cargo test --test failure_parser_suggestion_integration

# 4. Verify proofing scripts
bash .pi/scripts/ci/stage_failure-parser_proofing.sh
```
