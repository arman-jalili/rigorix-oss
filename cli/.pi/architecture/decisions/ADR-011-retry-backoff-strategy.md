# ADR-011: Retry and Backoff Strategy

**Status:** Accepted
**Date:** 2026-06-16

## Context

Node execution may fail transiently (network timeout, rate limit, LSP conflict). The system needs retry logic that balances reliability against resource usage.

## Decision

**Three-tier retry with exponential backoff and jitter.**

### Per-Node Retry Configuration (default)

| Parameter | Default | Description |
|-----------|---------|-------------|
| max_attempts | 4 | Original + 3 retries |
| retry_strategies | [SameOperation, SameOperation, ExpandContext] | Escalating approach |
| backoff | Exponential, base=100ms, multiplier=2.0, max=30s | Delay between retries |
| retry_on | [Transient, LspConflict] | Which failures trigger retry |

### Attempt Sequence (default)

| Attempt | Strategy | Delay | Reasoning |
|---------|----------|-------|-----------|
| 1 (original) | — | — | First attempt |
| 2 (retry 1) | SameOperation | ~100ms | Quick retry — transient may have resolved |
| 3 (retry 2) | SameOperation | ~200ms | Slightly longer wait |
| 4 (retry 3) | ExpandContext | ~400ms | Broader context may resolve LSP/compile issues |

### When Retries Are Exhausted

1. If `fallback_node` is configured → execute the fallback node
2. If `skip_on_exhaustion` is true → mark node as Skipped (not Failed)
3. Otherwise → node is Failed, execution may abort if `max_failures_before_abort` is exceeded

### Session-Level Limits

| Limit | Default | Purpose |
|-------|---------|---------|
| max_total_retries_per_session | 100 | Prevents infinite retry loops across the entire DAG |
| max_failures_before_abort | 0 (unlimited) | Max permanent failures before aborting |

## Rationale

- **Exponential backoff** prevents overwhelming the system after transient failures
- **ExpandContext strategy** provides a meaningful escalation path (more context = better LLM planning)
- **Max retries = 3** is the bounded autonomy principle in practice — 3 is enough for transients, not enough for infinite loops
- **Fallback nodes** allow graceful degradation instead of hard failure

*Affects: Execution Engine, DAG Engine, Failure Classification*
