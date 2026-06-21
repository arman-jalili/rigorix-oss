# Rigorix Actions Runbook

This is an aggregate runbook index. Each module has its own detailed runbook:

| Module | Runbook |
|--------|---------|
| Action Input | [runbook-action-input.md](runbook-action-input.md) |
| Action Output | [runbook-action-output.md](runbook-action-output.md) |
| CI Integration | [runbook-ci-integration.md](runbook-ci-integration.md) |
| Diff Analyzer | [runbook-diff-analyzer.md](runbook-diff-analyzer.md) |
| Policy Evaluator | [runbook-policy-evaluator.md](runbook-policy-evaluator.md) |
| Security Config | [runbook-security-config.md](runbook-security-config.md) |

## Incident Response

For any incident:

1. **Identify** which module is affected from the error logs
2. **Navigate** to the specific module runbook from the table above
3. **Follow** the incident/rollback/recovery procedures in that runbook

## Escalation

| Severity | Response Time | Escalation Path |
|----------|---------------|-----------------|
| Critical | < 15 min | DevOps → Engineering Lead |
| High | < 1 hour | DevOps |
| Medium | < 24 hours | Issue tracking |
| Low | Next sprint | Issue tracking |
