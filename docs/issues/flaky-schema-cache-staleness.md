---
guardian_issue:
  id: "ISSUE-FLAKY-SCHEMA-CACHE-STALENESS"
  title: "Flaky test: mcp test_schema_cache_is_stale asserts on a zero-duration window, so it races the clock"
  epic: "tooling / test reliability"
  component: "MCP / Audit Tools"
  module: "audit-tools"
  status: planned
  priority: low
  dependencies: []

  intent: |
    Remove a genuine flake. `test_schema_cache_is_stale` calls
    `is_stale(Duration::seconds(0))` immediately after `update()`, which requires
    the clock to have advanced since the update. If no time elapses (fast machine,
    coarse clock resolution, or an implementation that compares with `>=`), the
    assertion fails for reasons unrelated to the behaviour under test.

  in_scope:
    - "Make the staleness test deterministic (inject the clock, or test the boundary without racing wall time)"

  out_of_scope:
    - "Changing the staleness semantics themselves (the max-age contract stays)"
    - "Auditing other tests for flakiness (separate, if it recurs)"

  affected_layers:
    domain:
      - "Modified: SchemaCache staleness boundary (only if it takes an injected clock)"
    application:
      - "No change"
    infrastructure:
      - "No change"
    api:
      - "No change"

  canonical_references:
    - module: "mcp/src/audit_tools (schema cache)"
    - last_sync: "2026-10-12"

  acceptance_criteria:
    - "The test no longer depends on wall-clock advance between two statements"
    - "The max-age boundary is still tested (a zero window is stale, a large window is not)"
    - "The test passes repeatedly in the full suite (not only in isolation)"

  validators:
    - ci
    - tests

  implementation_notes: |
    Observed during verification of PR #1003: the test failed once in a full run
    and then passed 5/5 in isolation — the classic signature of a timing
    dependency rather than a product bug. It is pre-existing, unrelated to that PR.

    Current shape (mcp/tests/enterprise_proxy_contract_freeze_test.rs):

        cache.update(metadata);
        assert!(!cache.is_stale(chrono::Duration::seconds(3600)));
        assert!(cache.is_stale(chrono::Duration::seconds(0)));   // <- races the clock

    The last line requires `elapsed > 0`. It is true only if the clock advanced
    between `update()` and the assertion.

    Prefer injecting the clock (a `now` parameter or a test clock) so the boundary
    is exercised without racing real time. If `is_stale` compares with `>` rather
    than `>=`, note that in the fix — the zero-window case is exactly where the two
    differ, and the test is unintentionally pinning that choice.

  file_changes:
    - "modify: mcp/src/audit_tools (schema cache staleness, if the clock is injected)"
    - "modify: mcp/tests/enterprise_proxy_contract_freeze_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-FLAKY-SCHEMA-CACHE-STALENESS — a test that races the clock

## Problem

`mcp/tests/enterprise_proxy_contract_freeze_test.rs`:

```rust
#[test]
fn test_schema_cache_is_stale() {
    let mut cache = SchemaCache::new();

    // Empty cache is always stale
    assert!(cache.is_stale(chrono::Duration::seconds(1)));

    let metadata = EnterpriseMetadata { /* ... */ };
    cache.update(metadata);

    // Just updated — should not be stale
    assert!(!cache.is_stale(chrono::Duration::hours(1)));
    assert!(!cache.is_stale(chrono::Duration::seconds(3600)));

    // Already elapsed — stale
    assert!(cache.is_stale(chrono::Duration::seconds(0)));   // <- needs elapsed > 0
}
```

The final assertion asserts that a cache updated microseconds ago is stale under a
**zero** max-age. That is only true if the clock has advanced since `update()`. On
a fast machine, or with coarser clock resolution, no time elapses and the test
fails — reporting a product bug that does not exist.

## Evidence

Observed in the PR #1003 verification run: failed once in the full suite, then
passed 5/5 in isolation. Pre-existing; unrelated to that PR. Recorded here so it is
not mistaken later for a regression in whatever PR happens to trigger it.

## Fix

Inject the clock (a `now` parameter or a test clock) so the boundary is exercised
without racing wall time. While fixing it, note whether `is_stale` compares with
`>` or `>=` — the zero-window case is precisely where they differ, so this test is
currently pinning that choice implicitly rather than stating it.

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | No dependency on elapsed wall time between two statements | `validate-tests.sh` |
| 2 | The max-age boundary is still covered (zero → stale, large → fresh) | `validate-tests.sh` |
| 3 | Passes repeatedly in a full run, not only in isolation | `validate-ci.sh` |

## Implementation

> **Agent:** This is a test-determinism fix, not a behaviour change. Do not alter
> the staleness contract — only how the test establishes "time has passed".

### Steps

1. Read the `SchemaCache` staleness implementation and note `>` vs `>=`
2. Inject the clock (or take `now` as a parameter) where the boundary is decided
3. Rewrite the test to advance the injected clock instead of racing real time
4. Keep both boundary assertions (zero window stale, large window fresh)
5. Run the full mcp suite repeatedly to confirm the flake is gone
