---
guardian_issue:
  id: "ISSUE-CEILING-L3-WITHOUT-DURABILITY"
  title: "the ceiling derivation returns L3 for an in-memory ledger, contradicting its own doc comment — doctor over-reports the one number the runbook forbids misstating"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1182
  component: "EffectCommitment"
  module: "effect-commitment"
  status: planned
  priority: high
  created_at: "2026-10-23T16:00:00Z"
  updated_at: "2026-10-23T16:00:00Z"

  intent: |
    Make the ceiling derivation match its documentation: a deployment whose records die with the
    process cannot claim L3.

    `EffectCommitmentCeiling::derive` (`application/setup.rs:93-100`) is:

    ```rust
    match (shared_ledger, provider_evidence) {
        (_, true) => Self::L3,          // ← provider evidence alone upgrades
        (true, false) => Self::L2,
        (false, false) => Self::L1,
    }
    ```

    Its own doc comment, three lines above, says the opposite:

    > "Provider evidence is what upgrades L2 to L3; **the absence of a shared ledger caps the claim at
    > L1 even when a receipt reader is configured**, because the record itself does not survive the
    > process."

    The enum agrees with the comment: `L1` is defined as *"In-process ledger: exact within this
    process, lost on restart"*, `L3` as *"Provider receipt read-back is armed: can prove committed"*.
    The code picks L3 for `(in_memory, receipt_reader)`.

    **The case is reachable.** `EffectCommitmentConfig::parse` accepts `kind = "in_memory"` together
    with a `[receipt_reader]` section (verified by parsing that exact config: `is_ok = true`; the
    in-memory rejection rule covers only backend HTTP fields, `config.rs:237-244`). `EffectCommitmentSetup`
    computes the two booleans independently (`setup.rs:161-166`), and `doctor` prints
    `setup.ceiling()` directly (`doctor.rs:411`).

    So an operator with an in-process ledger and a receipt reader runs `rigorix doctor`, reads
    **ceiling L3**, and is told that a deployment which loses every record on restart can prove the
    effect committed — which is the claim the runbook explicitly forbids (`runbook:19-20`).

    A unit test asserts the defect (`setup.rs:359-360`: `derive(false, true) == L3`), so the suite is
    green on the wrong behaviour.

  dependencies:
    - name: "EffectCommitmentCeiling::derive"
      type: internal
      note: "application/setup.rs:93-100 — the match arm; :88-92 is the contradictory doc comment."
    - name: "EffectCommitmentConfig::parse"
      type: internal
      note: "config.rs:237-244 — in_memory rejects backend HTTP fields only; a [receipt_reader] is allowed."
    - name: "doctor effect-commitment row"
      type: internal
      note: "cli/src/cli_boundary/doctor.rs:409-419 — prints the derived ceiling (#1176)."
    - name: "runbook ceiling table"
      type: internal
      note: "engine/docs/runbook-effect-commitment.md:15-20 — L3 conditions and the prohibition."

  in_scope:
    - "Fix the derivation so provider evidence cannot lift an in-process ledger above L1"
    - "Flip the unit test that asserts L3 for (false, true); add the (true,true) case explicitly"
    - "State in the runbook and doctor description what L1-with-a-receipt-reader can and cannot prove"
    - "Test the full matrix via setup: in_memory+reader ⇒ L1, http ⇒ L2, http+reader ⇒ L3"

  out_of_scope:
    - "Whether an L1 deployment should be allowed to configure a receipt reader at all (it may be useful for manual reconciliation)"
    - "Splitting the ceiling into two axes (durability vs settlement evidence) — see the note below"
    - "The doctor row itself (#1176)"

  affected_layers:
    domain:
      - "Modified: EffectCommitmentCeiling::derive"
    application:
      - "Modified: setup.rs doc comment and the unit test"
    infrastructure:
      - "None"
    api:
      - "Modified: doctor output for the in_memory + receipt_reader configuration"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "application/setup.rs:93-100 — (_, true) => L3"
    - evidence: "application/setup.rs:88-92 — 'caps the claim at L1 even when a receipt reader is configured'"
    - evidence: "application/setup.rs:76-84 — the L1/L2/L3 enum doc"
    - evidence: "application/setup.rs:359-360 — the test asserting (false,true) == L3"
    - evidence: "orchestrator: config.rs:237-244 — in_memory rejects backend HTTP fields only"
    - evidence: "parsed 'kind=\"in_memory\"' + [receipt_reader] successfully (is_ok = true)"
    - evidence: "cli/src/cli_boundary/doctor.rs:411 — ceiling printed from setup.ceiling()"
    - evidence: "engine/docs/runbook-effect-commitment.md:15-20"

  acceptance_criteria:
    - "derive(false, true) == L1; derive(true, false) == L2; derive(true, true) == L3"
    - "The unit test asserts the corrected matrix and would fail on the old behaviour"
    - "An end-to-end setup test covers in_memory+reader ⇒ L1 and http+reader ⇒ L3"
    - "doctor prints L1 for an in-memory ledger with a receipt reader"
    - "The runbook explains what L1-with-a-reader can prove (settlement, while the process lives) and what it cannot (durability)"
    - "validate-ci.sh, validate-tests.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - canonical

  implementation_notes: |
    The doc comment is the specification; the code drifted from it. Change the code, not the comment.

    Two axes are being collapsed into one label, and that is worth a sentence rather than a redesign:
    durability (does the record survive?) and settlement evidence (can we learn what happened?) are
    independent. An L1 deployment with a receipt reader genuinely can ask the provider "did this
    commit?", but it cannot fence anything after a restart, because the record is gone. If you want to
    express both, do it in the doctor detail line — "ceiling L1 (record not durable); receipt reader
    configured for manual reconciliation" — rather than by widening the enum in this issue.

    Do not silently drop the receipt reader at L1. Configuring one is legitimate; reporting a ceiling
    it does not earn is not.

  file_changes:
    - "modify: engine/src/effect_commitment/application/setup.rs"
    - "modify: engine/docs/runbook-effect-commitment.md"
    - "modify: cli/src/cli_boundary/doctor.rs (description only, if the detail line changes)"
    - "create: engine/tests/effect_commitment_ceiling_matrix.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1182: L3 without durability

## What you are looking at

```
derive(shared_ledger, provider_evidence)
  (_,             true)  => L3      ← in-memory + receipt reader reaches L3
  (true,          false) => L2
  (false,         false) => L1
```

Doc comment above it: *"the absence of a shared ledger caps the claim at L1 even when a receipt reader
is configured, because the record itself does not survive the process."*

Reachable, verified: `kind = "in_memory"` + `[receipt_reader]` parses, setup derives, doctor prints.
The result is `ceiling L3` for a deployment whose records are lost on restart — the claim the runbook
forbids.

## Decision (already made)

Fix the derivation to match its comment: `(false, _) => L1`, `(true, false) => L2`, `(true, true) => L3`.
Flip the test that asserts the defect.

## In Scope

- The match arms, the test matrix, the runbook sentence, an end-to-end setup test

## Out of Scope

- Whether L1 may configure a reader (it may)
- Splitting the ceiling into two axes (note only)
- The doctor row (#1176)

## Affected Layers

### Domain
- `derive`

### Application
- Doc comment + unit test

### API
- doctor output for that configuration

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **Evidence:** `setup.rs:76-100,359-360`; `config.rs:237-244`; `doctor.rs:411`; `runbook:15-20`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Correct matrix | `validate-tests.sh` |
| 2 | Test fails on the old behaviour | `validate-tests.sh` |
| 3 | Setup matrix test (in_memory+reader ⇒ L1) | `validate-integration.sh` |
| 4 | doctor prints L1 for that config | `validate-tests.sh` |
| 5 | Runbook explains both axes | `validate-canonical.sh` |

## Implementation

1. Read `setup.rs:76-110`, `:155-170`, `:340-370`; `config.rs:230-250`.
2. Fix the match; fix the test; add the matrix test through setup.
3. One runbook sentence on what L1-with-a-reader proves.
4. Run validators.

## Pitfalls

- **Do not change the doc comment to match the code** — the comment is the specification.
- **Do not reject the config** — a reader at L1 is legitimate for manual reconciliation.
- **Beware tests that assert the bug**: `setup.rs:359` does, which is why the suite never caught it.

## Why this is high

It is the number the runbook singles out as not-to-be-misstated, printed by the tool built to report
it honestly, in the configuration most likely to be tried first (opt-in, in-process, plus a reader).
