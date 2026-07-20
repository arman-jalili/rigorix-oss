# Disaster Recovery Plan: Scored Evaluation Module

## Scope

This DR plan covers the Scored Evaluation module only. For system-wide DR, see `docs/dr-plan-overview.md`.

## RTO / RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | 15 minutes | Time to restore scoring capability after failure |
| RPO (Recovery Point Objective) | 1 hour | Maximum acceptable evaluation result loss |
| MTPD (Maximum Tolerable Period of Disruption) | 4 hours | Without scoring, merges cannot be gated on quality |

## Backup Strategy

### What to Back Up
- Evaluation result files: `.rigorix/evaluations/` directory
- Scoring backend configuration: `.rigorix/scored_evaluation.toml`
- Scoring scripts (for LocalBackend)

### Backup Schedule
| Data | Frequency | Retention | Method |
|------|-----------|-----------|--------|
| Evaluation results | After each execution | 30 days | Filesystem snapshot |
| Configuration | On change | Git history | Git |
| Scoring scripts | On change | Git history | Git |

### Backup Verification
- Monthly: Restore a random evaluation result and verify JSON validity
- Quarterly: Run a full restore of `.rigorix/evaluations/` to a test environment

## Failure Scenarios

### Scenario 1: All Scoring Backends Unavailable

**Impact**: All `scored_evaluation` DAG nodes fail. Policy gating cannot evaluate score thresholds.

**Detection**: Health checks return `false` for all backends. Evaluation attempts return `BackendUnavailable`.

**Recovery Steps:**
1. Identify root cause (network outage, backend service down, config error)
2. Restore backend connectivity
3. Run `health_check()` on each backend to verify recovery
4. Re-run failed evaluations (DAG retry policy will handle this automatically)

**Fallback**: Configure a local script backend as a degraded scoring mechanism:
```toml
[scored_evaluation.backends.fallback]
type = "local"
script_path = "./scripts/degraded_scorer.sh"
timeout_ms = 5_000
```

### Scenario 2: Evaluation Repository Corruption

**Impact**: Cannot persist or retrieve evaluation results.

**Detection**: Repository `save()` or `get()` operations return `Internal` errors.

**Recovery Steps:**
1. Stop evaluation processing
2. Move corrupt data: `mv .rigorix/evaluations/ .rigorix/evaluations.corrupt/`
3. Repository auto-creates new directory on next write
4. Resume evaluation processing
5. Attempt to restore from backup: `cp -r .rigorix/evaluations.backup/ .rigorix/evaluations/`
6. Verify restoration with `list_evaluations()`

### Scenario 3: Policy Engine Scoring Conditions Not Triggering

**Impact**: ScoreAbove/ScoreBelow policy conditions don't gate merges.

**Detection**: Policy rules with scoring conditions are evaluated but never match.

**Recovery Steps:**
1. Verify `LaneContext::scoring_scores` is populated after scored evaluation completes
2. Check that `scoring_scores` uses u8 percentage values (0–100)
3. Verify threshold values in `ScoreAbove { threshold: 80 }` match expectations
4. Check that dimension names match between scoring result and policy config

## Failover Plan

The Scored Evaluation module has a single-region, single-instance architecture.
Failover consists of:

1. **Primary → Secondary Backend**: Configure multiple backends in config.
   The service uses the first healthy backend; falling back to alternatives
   is handled in `ScoredEvaluationServiceImpl`.

2. **Repository Failover**: If the primary filesystem path fails, reconfigure
   the `LocalEvaluationRepository` base directory to an alternative path.

## DR Testing

| Test | Frequency | Success Criteria |
|------|-----------|-----------------|
| Backend failover | Monthly | Evaluations succeed with alternative backend |
| Repository recovery | Quarterly | Restored data is readable and valid |
| Script backup | Quarterly | Scoring scripts can be restored from Git |

## Post-Incident Review

After any DR event:
1. Root cause analysis documented in `docs/postmortem/`
2. Runbook updated with any new failure modes discovered
3. DR plan updated with any gaps identified
4. Backup/restore procedures tested to validate fixes
