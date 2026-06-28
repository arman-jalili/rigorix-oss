# Runbook — Rigorix Operations

## Incident Response

| Severity | Response Time | Escalation Path |
|----------|--------------|-----------------|
| P1 (Critical) | < 15 min | On-call → Engineering Lead |
| P2 (High) | < 1 hour | On-call → Engineering Lead |
| P3 (Medium) | < 4 hours | Engineering Team |
| P4 (Low) | Next business day | Issue triage |

## Common Incidents

### CI Pipeline Failure
1. Check GitHub Actions logs for the failing job
2. Run `bash .pi/scripts/local-ci.sh --stage=<stage>` locally
3. Check `.pi/output/ci-report-*.txt` for details

### Build Failure
1. Verify `cargo check -p rigorix-{engine,cli,actions}` passes
2. Check for dependency issues: `cargo audit`
3. Run `cargo clean && cargo build`

### Test Flakiness
1. Run the failing test 3 times: `cargo test <test_name> -- --count 3`
2. If sporadic, check for filesystem race conditions
3. Add `.flush().await` and `.sync_all().await` to file operations

## Rollback Procedure

1. `git revert HEAD` to undo last commit
2. `git push origin main` to deploy revert
3. Verify CI passes after revert
4. Create a fix branch from the revert point

## On-Call

- Primary: @arman-jalili
- Escalation: Create GitHub issue with `incident` label
