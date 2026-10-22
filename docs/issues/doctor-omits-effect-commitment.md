---
guardian_issue:
  id: "ISSUE-DOCTOR-OMITS-EFFECT-COMMITMENT"
  title: "doctor reports every seam except effect-commitment, so the newest enforcement boundary is invisible — including which claim ceiling a deployment is at"
  epic: "operational usability"
  epic_id: "EPIC-OPERATIONAL-USABILITY"
  tracking_issue: 1176
  component: "Cli"
  module: "cli-boundary"
  status: planned
  priority: medium
  created_at: "2026-10-23T15:00:00Z"
  updated_at: "2026-10-23T15:00:00Z"

  intent: |
    Add an effect-commitment probe to `rigorix doctor`, reporting the armed state *and* the claim
    ceiling (L1/L2/L3), because that is the number the runbook forbids misstating.

    `doctor` currently probes: preconditions, approval binding, sequence policy, budget/enforcement,
    the audit anchor, and the capacity ledger. It does not probe effect-commitment — `grep` for
    `effect_commitment` in `cli/src/cli_boundary/doctor.rs` returns nothing — even though the seam is
    constructed in the same three roots and refuses dispatch when misconfigured.

    The consequence is specific rather than cosmetic: without a probe, the answer to "is this
    deployment at L1, L2, or L3?" is only available by reading `.rigorix/effect-commitment.toml` and
    knowing that a missing `[receipt_reader]` caps the ceiling at L2 (#1166). The runbook is explicit
    that a deployment must not be described as L3 without a receipt reader; doctor is the tool that
    should enforce that sentence, and today it is silent about the context entirely.

    Related, and deliberately *not* part of this issue: `rigorix init` writes a config whose seams are
    all commented out and labelled "ALL UNARMED AS WRITTEN (#1101)". That is a product decision, and it
    is documented in the generated file. It only becomes a problem when the operator has no way to
    discover the unarmed state afterwards — which is this issue.

  dependencies:
    - name: "EffectCommitmentSetup::from_env"
      type: internal
      note: "application/setup.rs — returns Ok(None) when no config; the doctor probe must distinguish absent from misconfigured."
    - name: "CapacitySetup probe (the shape to copy)"
      type: internal
      note: "cli/src/cli_boundary/doctor.rs:377 — Armed / Unarmed / Misconfigured with the source named."
    - name: "Claim ceiling (L1/L2/L3)"
      type: internal
      note: "runbook-effect-commitment.md:15-20 — L3 requires a receipt reader; the probe should report the derived level, not the configured one."

  in_scope:
    - "A doctor row for effect-commitment with armed/unarmed/misconfigured, naming the config file"
    - "The derived ceiling in the row's detail: L1 (absent), L2 (ledger configured), L3 (receipt reader configured)"
    - "A redaction test: the row must not print a token or a URL with credentials"
    - "Exit-code behaviour consistent with the other seams (misconfigured ⇒ non-zero)"

  out_of_scope:
    - "Changing `rigorix init` defaults (documented as intentionally unarmed)"
    - "Probing enterprise-side services directly"
    - "The capacity probe, which exists"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: cli_boundary/doctor.rs — one more probe"
    infrastructure:
      - "None"
    api:
      - "The doctor output surface gains a row"

  canonical_references:
    - module: ".pi/architecture/modules/cli-boundary.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "cli/src/cli_boundary/doctor.rs:216 — probe(); :221,:244,:275,:328,:376,:403,:451,:480 — the existing rows"
    - evidence: "no `effect_commitment` reference anywhere in doctor.rs"
    - evidence: "runbook-effect-commitment.md:15-20 — the L1/L2/L3 ceiling and the prohibition on claiming L3"
    - evidence: "cli/src/cli_boundary/dispatch.rs:141-146 — init writes all seams commented out"

  acceptance_criteria:
    - "`rigorix doctor` prints an effect-commitment row in all three states"
    - "The row reports the derived ceiling (L1/L2/L3), not the configured intent"
    - "A missing config prints Unarmed with the expected path, not an error"
    - "No token or credential appears in the output"
    - "Misconfigured ⇒ non-zero exit, consistent with other seams"
    - "validate-ci.sh, validate-tests.sh, validate-security.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - security

  implementation_notes: |
    Copy the capacity probe's shape rather than inventing one: it already distinguishes absent from
    malformed and names the source, which is exactly what an operator needs.

    Report the **derived** ceiling. A deployment with `[backend] kind = "http"` but no `[receipt_reader]`
    is L2, and printing "L3" because a ledger is configured is the one mistake the runbook calls out
    explicitly. The derivation already exists in the setup path; do not duplicate the rule in the CLI.

    Keep credentials out. The config names tokens by environment variable, so the row should print the
    variable name, never its value, and never a base_url containing userinfo.

  file_changes:
    - "modify: cli/src/cli_boundary/doctor.rs"
    - "create: cli/tests/doctor_effect_commitment.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1176: the newest seam is the one doctor cannot see

## What you are looking at

| seam | probed |
|---|---|
| preconditions | ✅ |
| approval binding | ✅ |
| sequence policy | ✅ |
| budget / enforcement | ✅ |
| audit anchor | ✅ |
| capacity ledger | ✅ |
| **effect commitment** | **❌** |

The context that decides whether a run can double-spend is the only one with no row — and the runbook
gives operators a rule ("do not describe a deployment as L3 without a receipt reader") that nothing
checks.

## In Scope

- One probe, three states, derived ceiling, no secrets

## Out of Scope

- `init` defaults (#1101 decision); enterprise service probing; the capacity probe

## Affected Layers

### Application
- `doctor.rs` gains a row

## Canonical References

- **Module:** `.pi/architecture/modules/cli-boundary.md`
- **ADR-019**
- **Evidence:** `doctor.rs:216` and the eight existing rows; `runbook-effect-commitment.md:15-20`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Row present in all three states | `validate-tests.sh` |
| 2 | Ceiling derived, not declared | `validate-tests.sh` |
| 3 | Absent config ⇒ Unarmed, not error | `validate-tests.sh` |
| 4 | No credentials in output | `validate-security.sh` |
| 5 | Misconfigured ⇒ non-zero | `validate-tests.sh` |

## Implementation

1. Read `doctor.rs:216-260` (capacity probe) and `effect_commitment/application/setup.rs`.
2. Add the probe; derive the ceiling from what is configured.
3. Test the three states plus redaction.

## Pitfalls

- **Do not re-implement the ceiling rule in the CLI** — derive it where it is already decided.
- **Do not print token values**, only the env-var names.
- **Do not treat absent config as misconfigured**: absent is L1/unarmed and must exit zero.

## Why this is medium

Nothing is broken; the seam works or is absent. But the one tool an operator runs to learn what is
armed omits the boundary that determines whether a retry can double-spend, and the ceiling it implies.
