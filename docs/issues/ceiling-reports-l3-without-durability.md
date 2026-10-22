---
guardian_issue:
  id: "ISSUE-CEILING-SCALAR-HIDES-DURABILITY"
  title: "the ceiling is one scalar for two orthogonal axes: doctor prints 'ceiling L3' for a deployment with no durability, and derive's doc comment states a rule the code does not implement"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1182
  component: "EffectCommitment"
  module: "effect-commitment"
  status: planned
  priority: medium
  created_at: "2026-10-23T17:00:00Z"
  updated_at: "2026-10-23T17:00:00Z"

  intent: |
    Report durability and settlement evidence separately, and fix the comment that contradicts both the
    code and the runbook.

    The runbook defines the levels by **claim**, not by a ladder (`runbook-effect-commitment.md:13-26`):

    | level | claim | requires |
    |---|---|---|
    | L1 | fences **in this process** | `in_memory` |
    | L2 | survives process death, contested store | `http` **+ a service** — "OSS ships the client, not the service; enterprise ADR-011 specifies it" |
    | L3 | **the effect committed** | `[receipt_reader]` + provider evidence + a provider that received the key |

    L3 requires no L2. So `EffectCommitmentCeiling::derive(false, true) == L3` is **correct per the
    runbook**: with a receipt reader, an in-process deployment can still prove that an effect
    committed, by re-deriving the intent key and asking the provider. That path is real — the key is
    `H(dag_id, node_id, intent_digest)` built from the persisted `TaskNode` (`dispatch.rs:802-812`,
    `key_impl.rs:37-45`), so it survives in the plan even when the ledger does not.

    Two problems remain, and neither is the derivation:

    1. **`derive`'s doc comment contradicts it** (`setup.rs:88-92`): "the absence of a shared ledger caps
       the claim at L1 even when a receipt reader is configured, because the record itself does not
       survive the process." The code and the runbook both disagree with that sentence.
    2. **One scalar cannot express two axes.** `doctor` prints `ceiling L3` with the description "the
       only ceiling that can prove committed" — true, and silent about the fact that this deployment
       has **no durability at all**: after a restart there is no record, no fence, and no sweep. An
       operator reading "L3" can reasonably infer a cumulative top tier.

    This matters commercially as well as operationally: durability is the enterprise property
    (ADR-011), and a label that reads as "top level" while the deployment is L1 on durability hides
    exactly what the enterprise service adds.

  dependencies:
    - name: "EffectCommitmentCeiling"
      type: internal
      note: "application/setup.rs:76-120 — enum, derive, as_str, description."
    - name: "doctor effect-commitment row"
      type: internal
      note: "cli/src/cli_boundary/doctor.rs:409-419 — prints ceiling + description."
    - name: "runbook ceiling table"
      type: internal
      note: "engine/docs/runbook-effect-commitment.md:13-26 — the spec this issue follows."

  in_scope:
    - "Correct `derive`'s doc comment to match the code and the runbook (L3 = settlement evidence)"
    - "Make the reported ceiling state both axes: durability (in-process | shared) and evidence (none | settled)"
    - "doctor prints both, and says plainly what the deployment cannot do after a restart"
    - "Keep L3 reachable at L1 — do not cap it"

  out_of_scope:
    - "Capping L3 at L2 (would remove a legitimate OSS capability and contradict the runbook)"
    - "Rejecting `[receipt_reader]` with `in_memory`; it is useful for manual reconciliation"
    - "The sweep capability (#1173) and the W17 row (#1183)"

  affected_layers:
    domain:
      - "Modified: EffectCommitmentCeiling — two axes, or one label plus a qualifier"
    application:
      - "Modified: derive doc comment; description text"
    infrastructure:
      - "None"
    api:
      - "Modified: the doctor row's text"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "runbook-effect-commitment.md:13-26 — L3 requires a receipt reader, not L2; L2 names the enterprise service"
    - evidence: "application/setup.rs:93-100 — (_, true) => L3 (correct per the runbook)"
    - evidence: "application/setup.rs:88-92 — the contradicting doc comment"
    - evidence: "application/setup.rs:110-120 — the three descriptions doctor prints"
    - evidence: "cli/src/cli_boundary/doctor.rs:411-419 — ceiling + description in the row"
    - evidence: "dispatch.rs:802-812 / key_impl.rs:37-45 — the intent key is re-derivable from the plan"

  acceptance_criteria:
    - "The reported ceiling names durability and evidence separately, or qualifies a single label"
    - "doctor's row says what the deployment cannot do after a restart when durability is in-process"
    - "L3 remains reachable with an in-process ledger and a receipt reader"
    - "`derive`'s doc comment agrees with the code and the runbook"
    - "Tests cover: in_memory+reader (settlement yes, durability no), http (durability yes, settlement no), http+reader (both)"
    - "validate-ci.sh, validate-tests.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - canonical

  implementation_notes: |
    Prefer two fields over a qualified string: `durability: InProcess | Shared` and
    `settlement: None | ProviderEvidence`. A string like "L3 (L1 durability)" invites the reader to
    parse it, and the whole problem is that one token carries two meanings.

    Keep the labels the runbook uses. Do not invent a new scale — the operator docs, the DR plan and
    the console all speak L1/L2/L3, and a fourth vocabulary is a new source of drift.

    Do not "fix" this by disabling the receipt reader at L1. Asking a provider whether an effect
    committed is useful precisely when the local record is gone; that is the one recovery path an
    in-process deployment has.

    The doctor line should be actionable: "settlement evidence: provider read-back armed;
    durability: in-process — a restart loses the record and the fence. Durability requires the ledger
    service (ADR-011)." That sentence also makes the enterprise gap legible without naming a product.

  file_changes:
    - "modify: engine/src/effect_commitment/application/setup.rs"
    - "modify: cli/src/cli_boundary/doctor.rs"
    - "modify: engine/docs/runbook-effect-commitment.md (if the table needs the two axes spelled out)"
    - "create: engine/tests/effect_commitment_ceiling_matrix.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1182: two axes reported as one label

## What you are looking at

```
durability:   in-process  |  shared (enterprise service)
settlement:   none        |  provider read-back
```

`ceiling()` collapses these into one of L1/L2/L3, and `doctor` prints it with "the only ceiling that
can prove committed". For `kind = "in_memory"` + `[receipt_reader]` that yields **L3** while both
durability and the ability to fence across a restart are absent.

The derivation is right per the runbook (L3 = the effect committed, no L2 required). The **label** is
what misleads, and `derive`'s doc comment states the opposite rule in prose.

## Decision (already made)

Keep L3 reachable at L1. Report the two axes separately; fix the comment; make doctor say what a
restart costs.

## In Scope

- Two-axis reporting; comment fix; doctor text; matrix tests

## Out of Scope

- Capping L3 at L2; rejecting readers at L1; #1173/#1183

## Affected Layers

### Domain
- `EffectCommitmentCeiling` — two fields

### Application
- `derive` comment; `description`

### API
- doctor row text

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **Evidence:** `runbook:13-26`; `setup.rs:88-120`; `doctor.rs:411-419`; `key_impl.rs:37-45`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Both axes reported | `validate-tests.sh` |
| 2 | doctor says what a restart costs | `validate-tests.sh` |
| 3 | L3 reachable at L1 | `validate-tests.sh` |
| 4 | Comment matches code + runbook | `validate-canonical.sh` |
| 5 | Matrix tests (3 combinations) | `validate-integration.sh` |

## Implementation

1. Read `setup.rs:76-130`, `doctor.rs:400-425`, `runbook:13-26`.
2. Split the type; keep the L1/L2/L3 vocabulary for each axis.
3. Update the doctor text; add the matrix tests.

## Pitfalls

- **Do not cap L3 at L1.** It contradicts the runbook and removes the only recovery path an
  in-process deployment has.
- **Do not invent new level names** — the runbook, DR plan and console all speak L1/L2/L3.
- **Do not fix it with a string** like "L3 (L1 durability)"; that is the same conflation in text form.

## Why this is medium

Operationally the deployment works as designed. But the label is what an operator quotes when asked
what protection is in place, and "L3" for a deployment that cannot fence after a restart overstates
durability — while hiding the one thing the enterprise service adds.
