---
guardian_issue:
  id: "ISSUE-ARMED-SEAM-BLOCKS-RETRY"
  title: "with effect-commitment armed, a node cannot retry after a definite failure: the retry fence allows NotCommitted, begin_effect refuses every terminal record"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1196
  component: "ExecutionEngine"
  module: "effect-commitment"
  status: planned
  priority: high
  created_at: "2026-10-24T06:00:00Z"
  updated_at: "2026-10-24T06:00:00Z"

  intent: |
    Let a definite failure be retried, which is what `NotCommitted` means.

    Two checks disagree about a terminal record:

    ```rust
    // effect_commitment/domain/state.rs:57
    pub fn blocks_retry(&self) -> bool { matches!(self, Self::Ambiguous) }
    // → may_retry(NotCommitted) == true        (asserted in retryfence_test.rs:67)

    // execution_engine/.../dispatch.rs:899
    if record.is_terminal() {
        return Err(... "already settled … refusing to re-dispatch a terminal record")
    }
    ```

    `begin_effect` runs `may_retry` first (correct: it refuses `Ambiguous`) and then refuses **any**
    terminal state, so `NotCommitted` is blocked by the second check even though the fence allows it.

    Until recently this never fired on a retry: the intent carried a per-attempt key, so a retry began a
    fresh record. Two changes made it reachable:

    - **#1187** made the key intent-stable — `H(dag_id, node_id, intent_digest)`, with `attempt`
      explicitly "diagnostics only; never part of key derivation" (`domain/intent.rs:48-52`).
    - **#1185** moved the write-ahead into `gate_node`, which runs on **every** dispatch attempt.

    So the second attempt for a node whose tool failed reads the same record, finds it terminal
    `NotCommitted`, and fails with an internal error instead of retrying. With the seam armed, the retry
    policy is defeated for every node that fails once — and the error reads as infrastructure trouble,
    not as a policy refusal.

    **No test arms the seam and retries.** `effect_commitment_dispatch_seam_integration.rs` covers a
    successful dispatch, a definite failure recorded as `NotCommitted`, and a failed write-ahead — each
    in one shot.

  dependencies:
    - name: "EffectState::blocks_retry"
      type: internal
      note: "domain/state.rs:57 — Ambiguous only."
    - name: "begin_effect terminal refusal"
      type: internal
      note: "dispatch.rs:899-906 — refuses every terminal state, after the may_retry check."
    - name: "gate_node calls begin_effect"
      type: internal
      note: "gate.rs:214 — once per dispatch attempt, for both routes (#1185)."
    - name: "intent-stable key"
      type: internal
      note: "domain/intent.rs:48-52 (#1187) — why the retry now sees the same record."

  in_scope:
    - "Allow re-dispatch of a `NotCommitted` record; keep `Committed` refused and `Ambiguous` fenced"
    - "Decide and implement how the record represents a re-dispatch (re-open to `Dispatched` with attempt++,
      or an attempt-scoped record) — the chosen model must keep the audit trail readable"
    - "Test: armed seam, tool fails, retry succeeds, record history shows both attempts"
    - "Test: armed seam, `Committed`, re-dispatch still refused"

  out_of_scope:
    - "The retry budget policy"
    - "Reconciliation and the sweep"
    - "Capacity keys (#1163/#1187, done)"

  affected_layers:
    domain:
      - "Modified: EffectState/EffectRecord — a NotCommitted → Dispatched re-arm, if that is the model"
    application:
      - "Modified: begin_effect — the terminal check becomes state-specific"
    infrastructure:
      - "Modified: ledger begin/mark semantics if re-arming is chosen"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-24"
    - evidence: "domain/state.rs:57-59 — blocks_retry is Ambiguous only"
    - evidence: "dispatch.rs:891-906 — may_retry then is_terminal, both refusing"
    - evidence: "gate.rs:214 — begin_effect inside the shared gate"
    - evidence: "domain/intent.rs:48-52 — attempt is not part of the key"
    - evidence: "tests/unit/effect-commitment/retryfence/retryfence_test.rs:67 — may_retry(resolved) is true"
    - evidence: "effect_commitment_dispatch_seam_integration.rs — success / definite failure / failed write-ahead only"

  acceptance_criteria:
    - "With the seam armed, a node whose tool failed is retried and can succeed"
    - "A `Committed` record still refuses re-dispatch"
    - "An `Ambiguous` record still refuses (F2), with the existing message"
    - "The record's history shows both attempts (attempt counter or transition log)"
    - "Tests cover the three cases with the seam armed"
    - "validate-ci.sh, validate-tests.sh, validate-architecture.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - architecture

  implementation_notes: |
    The state machine should own the re-arm, not the executor: `EffectRecord::reopen_for_retry(attempt)`
    with a legal transition `NotCommitted → Dispatched`, recorded in the transition log. That keeps
    "which attempt am I looking at" answerable from the signed record and stops a second design from
    appearing in `begin_effect`.

    Do **not** solve this by putting the attempt back into the key. The intent-stable key is what makes
    same-intent lookup possible — it is the point of #1187 and ADR-019 — and re-introducing
    attempt-scoping would restore the double-hold problem #1163 fixed for capacity.

    Do **not** simply delete the terminal check: `Committed` must stay refused, because re-dispatching a
    committed effect is the double-spend this whole context exists to prevent.

  file_changes:
    - "modify: engine/src/effect_commitment/domain/state.rs"
    - "modify: engine/src/effect_commitment/domain/record.rs"
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs"
    - "create: engine/tests/effect_commitment_retry_with_seam.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1196: enabling the fence disables retries

## What you are looking at

| record state | fence says | begin_effect does | correct |
|---|---|---|---|
| `Dispatched` | retry ok | dispatches | — |
| `Ambiguous` | blocked | refuses (F2) | ✅ |
| `NotCommitted` | **retry ok** | **refuses "already settled"** | ❌ |
| `Committed` | — | refuses | ✅ |

```
attempt 1: begin → Dispatched → tool fails → settle NotCommitted
attempt 2: gate → begin → same key → terminal NotCommitted → Err("already settled")
```

## In Scope

- Permit `NotCommitted` re-dispatch; keep the other two refusals
- A record model that shows both attempts; tests with the seam armed

## Out of Scope

- Retry budget; reconciliation; keys (#1187 done)

## Affected Layers

### Domain
- `NotCommitted → Dispatched` re-arm, or attempt-scoped records

### Application
- `begin_effect` state-specific refusal

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **ADR-019**
- **Evidence:** `state.rs:57`; `dispatch.rs:891-906`; `gate.rs:214`; `intent.rs:48-52`; `retryfence_test.rs:67`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Armed seam + tool failure ⇒ retry succeeds | `validate-tests.sh` |
| 2 | `Committed` still refused | `validate-tests.sh` |
| 3 | `Ambiguous` still refused | `validate-tests.sh` |
| 4 | Record shows both attempts | `validate-integration.sh` |
| 5 | No attempt in the key | `validate-architecture.sh` |

## Implementation

1. Read `state.rs:40-80`, `dispatch.rs:862-910`, `gate.rs:190-225`, `intent.rs:41-60`.
2. Add the re-arm transition; make the refusal state-specific.
3. Add the three armed-seam tests.

## Pitfalls

- **Do not add the attempt to the key** — that restores the per-attempt identity #1163 removed.
- **Do not drop the terminal check** — `Committed` must stay refused.
- **Do not put the re-arm in the executor**; the state machine owns transitions and the log records them.

## Why this is high

Turning the feature on turns retries off. The failure looks like an internal error rather than a policy
decision, so an operator would debug the tool, not the fence — and no test exercises the combination.
