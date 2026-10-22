---
guardian_issue:
  id: "ISSUE-CONTRACT-PROOF-CANNOT-GATE"
  title: "the 150-check contract proof never runs in CI: the script exists only at repo root, every check loop runs with working-directory set to the crate, and the one root loop is continue-on-error + || true"
  epic: "verification discipline"
  epic_id: "EPIC-OPERATIONAL-USABILITY"
  tracking_issue: 1186
  component: "Docs"
  module: "effect-commitment"
  status: planned
  priority: high
  created_at: "2026-10-23T18:00:00Z"
  updated_at: "2026-10-23T18:00:00Z"

  intent: |
    Make the contract proof run in a job that fails. Three independent mechanisms each prevent it
    today, so fixing one is not enough.

    1. **The script exists only at root.** `.pi/scripts/ci/check_effect-commitment_contracts.sh` has no
       mirror under `engine/.pi/scripts/ci/` (or any crate's), unlike the 32 older `check_*_contracts.sh`
       scripts that are mirrored everywhere.
    2. **Every gating loop iterates inside a crate.** `ci.yml:85,115,145` run
       `for script in .pi/scripts/ci/check_*.sh` with `working-directory: ${{ matrix.crate }}`, and
       `:230,:255` iterate `"$crate"/.pi/scripts/ci/check_*.sh`. A root-only script is invisible to
       both.
    3. **The one loop that does see root scripts cannot fail.** `hardening.yml:80-89` iterates root and
       per-crate scripts but the step is `continue-on-error: true` and the body ends with `|| true`.

    `guards.tsv` — the registry the real-exit-code step reads (`hardening.yml:239`, "REAL exit codes —
    no continue-on-error, no || true") — does not list it.

    Net effect: the 150-check proof passes locally and in a non-gating job, and a regression in any of
    those 150 assertions cannot fail a build. **This also applies to `check_capacity-ledger_contracts.sh`**:
    same root-only placement, same absence from the crate mirrors and the registry — so both contexts
    added in this series are unprotected.

    Correction to an earlier validation of mine: I reported that this gated, on the basis of testing
    the loop's shell semantics under `bash -e` (a failing script does abort the step — true). That test
    was about `-e`, not about whether the file is on the loop's path. It is not.

  dependencies:
    - name: "check_effect-commitment_contracts.sh"
      type: internal
      note: ".pi/scripts/ci/ — root only; exits 1 on failure (:384-385), which is not the problem."
    - name: "the mirroring convention"
      type: internal
      note: "32 check_*_contracts.sh are mirrored into each crate's .pi/scripts/ci/; the two newest were not."
    - name: ".pi/guards.tsv"
      type: internal
      note: "35 lines; the registry the failing step reads. Neither newer script is registered."

  in_scope:
    - "Register both scripts in `.pi/guards.tsv` so the real-exit-code step runs them"
    - "Or mirror them into the crate `ci/` directories so the existing per-crate loops pick them up"
    - "Remove the `|| true` and `continue-on-error` from the step that currently swallows failures, or
      document which step is authoritative"
    - "Show the fix fires: a seeded failing assertion must fail the build"

  out_of_scope:
    - "Rewriting the 150 checks"
    - "The other root-only scripts (audit of all of them is a follow-up if the pattern recurs)"
    - "Branch-protection settings"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: .pi/guards.tsv or the crate mirrors; .github/workflows/hardening.yml"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: ".pi/scripts/ci/check_effect-commitment_contracts.sh — root only (no crate mirror)"
    - evidence: "engine/.pi/scripts/ci/ — 32 check_*_contracts.sh, neither effect-commitment nor capacity-ledger"
    - evidence: ".github/workflows/ci.yml:85-88,115-118,145-148 — working-directory: matrix.crate"
    - evidence: ".github/workflows/ci.yml:230,255 — \"$crate\"/.pi/scripts/ci/check_*.sh"
    - evidence: ".github/workflows/hardening.yml:80-89 — continue-on-error: true and || true"
    - evidence: ".pi/guards.tsv — 35 lines, no effect-commitment or capacity-ledger entry"

  acceptance_criteria:
    - "A failing assertion in either contract script fails a CI job"
    - "The mechanism is one of: guards.tsv registration, crate mirroring, or a root loop without || true"
    - "Both newer scripts are covered, not only effect-commitment"
    - "A seeded violation proves the gate fires (the guard must be shown to fail)"
    - "validate-ci.sh passes on a clean tree"

  validators:
    - architecture-validator
    - ci
    - canonical

  implementation_notes: |
    `guards.tsv` is the cheapest correct fix: it is the registry the one step with real exit codes
    reads, and registration is one line per script. Mirroring into five crates is 10 files to keep in
    sync — the drift that produced the original #1004 five-variant problem.

    Whichever mechanism, prove it fires. Seed a failing assertion (or a deliberately broken check) on a
    branch, confirm the job goes red, revert. A gate that has never been seen to fail is
    indistinguishable from a gate that always passes — that is the lesson from #1089's panic baseline
    and from this issue.

    Remove the `|| true`: while it is there, the "Docs policy scripts" step reports green for every
    script it runs, which is worse than not running them, because the log says they passed.

  file_changes:
    - "modify: .pi/guards.tsv"
    - "modify: .github/workflows/hardening.yml (drop || true from the authoritative step)"
    - "modify: engine/.pi/scripts/ci/ (+4 crates, only if mirroring is chosen)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1186: the proof passes, the build cannot fail

## Three independent blocks

| # | block | evidence |
|---|---|---|
| 1 | script not in any crate mirror | root-only; 32 peers are mirrored |
| 2 | every gating loop runs in a crate dir | `ci.yml:85/115/145` + `:230/255` |
| 3 | the root loop cannot fail | `hardening.yml:80-89` `continue-on-error` + `\|\| true` |
| 4 | not in the registry the failing step reads | `.pi/guards.tsv` has 35 lines, neither script |

## In Scope

- Register both scripts (effect-commitment **and** capacity-ledger) where they gate
- Remove the swallow, or make the swallowing step non-authoritative
- Prove it fires with a seeded failure

## Out of Scope

- Rewriting checks; auditing every root-only script; branch protection

## Affected Layers

### Infrastructure
- `guards.tsv` / crate mirrors; `hardening.yml`

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **Evidence:** the four rows above

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | A failing assertion fails a job | seeded violation |
| 2 | Both newer scripts covered | review |
| 3 | No `\|\| true` on the authoritative step | review |
| 4 | Clean tree passes | `validate-ci.sh` |

## Implementation

1. Read `hardening.yml:60-110` and `:235-260`; `.pi/guards.tsv`.
2. Register both scripts in `guards.tsv`.
3. Seed a failure; confirm red; revert.

## Pitfalls

- **Do not add a fifth mirrored copy** — use the registry; the mirrors already cost #1004.
- **Prove the gate fires.** Never-seen-to-fail is why this issue exists.
- **Fix capacity-ledger too**, not just the script the review happened to notice.
- **`continue-on-error` + `|| true` double-swallows**; removing one is not enough.

## Why this is high

The discipline is real and the artifacts are good, but nothing enforces them. A regression in 150
assertions produces a green build, and the log will say the checks passed.
