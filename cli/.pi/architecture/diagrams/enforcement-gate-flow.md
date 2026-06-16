# Enforcement — Tool Execution Gating Flow

```mermaid
graph TB
    CALL[Tool invocation requested] --> RISK_CLASSIFY[Classify tool risk level]
    
    RISK_CLASSIFY --> BUDGET_CHECK[Check resource budgets<br/>Tokens, calls, time used vs limits]

    BUDGET_CHECK --> |Budget OK| POLICY_CHECK[Check tool policy for risk level]
    BUDGET_CHECK --> |Budget exceeded| BLOCKED[Block: BudgetWarning emitted]

    POLICY_CHECK --> |Allow| ALLOW[Allow execution]
    POLICY_CHECK --> |Confirm| CONFIRM[Request user confirmation]
    POLICY_CHECK --> |Block| BLOCKED2[Block: tool denied by policy]
    POLICY_CHECK --> |DryRun| DRYRUN[Execute in dry-run mode]

    CONFIRM --> |Approved| ALLOW
    CONFIRM --> |Rejected| BLOCKED2

    ALLOW --> TRACK[Track resource usage]
    TRACK --> EMIT[Emit ToolExecuted event]

    BLOCKED --> EMIT
    BLOCKED2 --> EMIT
    DRYRUN --> EMIT
```

## Policy Matrix

| Risk Level | Default Policy | TUI Behavior |
|------------|---------------|--------------|
| Low | Allow | Auto-execute, show in log |
| Medium | Confirm | Show prompt: "Allow [tool]? (y/N)" |
| High | DryRun | Show what would happen, ask to confirm |
| Critical | Block | Deny, log to audit trail |

*Part of: Enforcement module*
