# Rigorix Actions Disaster Recovery

This is an aggregate DR plan index. Each module has its own detailed DR plan:

| Module | DR Plan |
|--------|---------|
| Action Input | [dr-plan-action-input.md](dr-plan-action-input.md) |
| Action Output | [dr-plan-action-output.md](dr-plan-action-output.md) |
| CI Integration | [dr-plan-ci-integration.md](dr-plan-ci-integration.md) |
| Diff Analyzer | [dr-plan-diff-analyzer.md](dr-plan-diff-analyzer.md) |
| Policy Evaluator | [dr-plan-policy-evaluator.md](dr-plan-policy-evaluator.md) |
| Security Config | [dr-plan-security-config.md](dr-plan-security-config.md) |

## RTO / RPO Summary

| Module | RTO | RPO | Stateful? |
|--------|-----|-----|-----------|
| Action Input | < 1 min | N/A | No |
| Action Output | < 1 min | N/A | No (ephemeral) |
| CI Integration | < 1 min | N/A | No |
| Diff Analyzer | < 1 min | N/A | No |
| Policy Evaluator | < 1 min | N/A | No |
| Security Config | < 1 min | N/A | No |
