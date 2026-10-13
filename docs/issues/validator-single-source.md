---
guardian_issue:
  id: "ISSUE-VALIDATOR-SINGLE-SOURCE"
  title: "The mirrored .pi/scripts validators have no single source of truth — five trees have drifted into 2–4 variants each"
  epic: "tooling / ci integrity"
  component: "CI / Validators"
  module: "validator-tooling"
  status: planned
  priority: medium
  dependencies:
    - "461ea2b9 (cwd resolution preamble — already landed, fixes a different defect)"

  intent: |
    Establish one source of truth for the validator scripts that are mirrored into
    every crate tree, so that `bash engine/.pi/scripts/validate-tests.sh` and
    `bash mcp/.pi/scripts/validate-tests.sh` are the same program — or, where a
    per-tree difference is intentional, record it explicitly and enforce it.

    This issue is deliberately NOT "make all copies byte-identical". That was
    attempted and reverted; see the warning section. The drift is real, but some
    per-tree differences are legitimate and a blanket normalisation makes things
    worse.

  in_scope:
    - "Inventory which mirrored files are (a) substantively drifted, (b) cosmetically drifted, (c) intentionally per-tree"
    - "Reconcile the substantively-drifted rust validators to one implementation"
    - "A guard that FAILS on unintentional drift, so it cannot recur silently"
    - "Fix the hardcoded 'language: C.UTF-8' message (35 dispatcher files)"

  out_of_scope:
    - "Rewriting the non-rust language validators: every tree is Rust, so languages/{go,java,python,typescript}/* never execute here. Their drift is cosmetic and inert."
    - "Changing which validators exist per tree (engine's validate-code-graph-contracts.sh, server having none, guards being root-only are all intentional)"
    - "Cross-repo deduplication with rigorix-sdk — tracked separately"

  affected_layers:
    domain:
      - "No code change — tooling"
    application:
      - "No code change"
    infrastructure:
      - "Modified: the rust validator implementations, reconciled from the newest variant"
      - "Modified: .pi/scripts/validate-*.sh dispatchers (the C.UTF-8 message)"
      - "New: a drift guard wired into a gating path"

  canonical_references:
    - module: ".pi/guards.tsv"
    - adr: []
    - last_sync: "2026-10-12"

  acceptance_criteria:
    - "The substantively-drifted rust validators have ONE implementation each, shared or byte-identical by construction"
    - "A gating guard fails when a mirrored file drifts, and prints which trees disagree"
    - "Intentional per-tree differences are recorded (which file, which trees, why) so the guard can distinguish drift from design"
    - "The 'language: C.UTF-8' message prints the detected language"
    - "Every per-tree validator still passes when invoked from the repo root in the form the hardening loop uses"
    - "validate-ci.sh / validate-guards.sh pass"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    ## WARNING — do not blanket-normalise (this was tried and reverted)

    An attempt was made to make every mirrored file identical across all trees.
    It was reverted because it did two harmful things:

    1. It rewrote 20 files under `languages/{go,java,python,typescript}/` whose
       differences were COSMETIC (`|| true` additions, a `LANG` -> `LANG_DETECTED`
       rename) and which are INERT — every tree is Rust, so `detect_language`
       returns `rust` and those files never execute. Checksum-matching dead files
       is churn, not a fix.
    2. It copied the four security guards (`validate-guards.sh`,
       `validate-panic-sites.sh`, `validate-secret-logs.sh`,
       `validate-unsafe-sites.sh`) into every tree. Those scripts resolve
       `.pi/panic-baseline.tsv` and friends relative to a tree, so the copies would
       have run against baselines that do not exist there — breaking the very
       guards, and making the hardening discovery loop run each of them 5x.

    Treat "hashes differ" as a QUESTION, not a verdict. Drift is only a defect
    where the difference is substantive AND the file executes.

    ## What is actually drifted (measured 2026-10-12)

    Mirrored in 5 trees (ROOT, engine, mcp, cli, actions); server has no
    dispatchers at all.

    Rust validators — variants across the 5 trees:
      tests         4 variants   <- substantive
      ci            3 variants   <- substantive
      integration   3 variants   <- substantive
      architecture  2 variants   <- substantive
      canonical     2 variants   <- substantive
      operations    1 variant    <- already consistent
      security      1 variant    <- already consistent (fixed earlier)

    Dispatcher sets per tree: ROOT 13, engine 10, mcp 9, cli 9, actions 9,
    server 0. Different SETS, not just different content — and that may be
    intentional, so map it before changing it.

    ## The substantive differences (these are the fix)

    - `validate-tests.sh`: ROOT uses `cargo tarpaulin`; engine uses
      `cargo llvm-cov --fail-under-lines 80`. Different tool, different result.
    - `validate-integration.sh`: some trees call `cargo test --test '*'`, which
      ERRORS on a target declaring `required-features` (a false failure); others
      call `cargo test --tests`, which skips such targets. The fix exists in some
      trees only.
    - `validate-ci.sh`: engine scopes with `-p rigorix-{crate}`; mcp has an ad-hoc
      `cd mcp` hack; ROOT has neither.
    - `validate-architecture.sh`: engine detects per-module layers
      (`src/{module}/domain/`) and accepts a flat/thin crate; ROOT only checks
      root-level `src/domain` and FAILS a thin crate.

    ## Real bug found while gathering this

    35 dispatcher files across the 5 trees print:

        echo "No <x> validator for language: C.UTF-8"

    `C.UTF-8` is a locale, not a language. This is a `$LANG` that was expanded to
    its value at edit time; the message should name the detected language. (Note
    also that one variant assigns `LANG=$(detect_language)`, shadowing the locale
    environment variable — prefer `LANG_DETECTED`.)

    ## Already fixed, do not redo

    The cwd-sensitivity defect (validators reading `src/` relative to the caller's
    cwd, so they failed or silently PASSED while skipping work when run from the
    repo root) was fixed in 461ea2b9 by resolving the tree from the script's own
    path, and the CI integration job now runs the per-crate copies.

    ## Cross-repo note

    `rigorix-sdk` carries the same seven `languages/rust/validate-*.sh` files and
    all seven differ from OSS. Filed separately in that repo; do not try to fix it
    from here.

  file_changes:
    - "modify: .pi/scripts/languages/rust/validate-*.sh (reconcile the substantive drift)"
    - "modify: */​.pi/scripts/languages/rust/validate-*.sh (same implementation)"
    - "modify: .pi/scripts/validate-*.sh and per-tree copies (the C.UTF-8 message)"
    - "create: .pi/scripts/validate-script-drift.sh (+ a recorded intentional-difference list)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-VALIDATOR-SINGLE-SOURCE — mirrored validators have drifted

## Intent

`.pi/scripts` is mirrored into every crate tree. The validator implementations
inside it have drifted apart, so the same command name means a different program
depending on which tree you invoke it from.

The goal is **one source of truth**, plus a guard so drift cannot recur silently —
not "make every file identical", which is both wrong and already reverted once.

## Evidence (measured 2026-10-12)

| rust validator | variants across 5 trees | nature |
|---|---|---|
| `validate-tests.sh` | **4** | substantive (`tarpaulin` vs `llvm-cov --fail-under-lines 80`) |
| `validate-ci.sh` | **3** | substantive (per-crate `-p rigorix-{crate}` scoping vs a `cd mcp` hack vs neither) |
| `validate-integration.sh` | **3** | substantive (`--test '*'` errors on `required-features` targets; `--tests` is the fix) |
| `validate-architecture.sh` | **2** | substantive (module layers + flat-crate acceptance vs root-only, which FAILS thin crates) |
| `validate-canonical.sh` | **2** | substantive (`SRC_DIR`/`PI_DIR` fallback + module-name matching) |
| `validate-operations.sh` | 1 | consistent |
| `validate-security.sh` | 1 | consistent (fixed earlier) |

Dispatcher sets also differ by tree — ROOT 13, engine 10, mcp 9, cli 9, actions 9,
server 0 — and **that may be intentional**, so map before changing.

## Warning — the trap

A blanket normalisation was attempted and reverted. It:

1. rewrote 20 **inert, cosmetically-different** files under
   `languages/{go,java,python,typescript}/` (every tree is Rust; those never run);
2. copied the four security guards into every tree, where they would resolve
   `.pi/panic-baseline.tsv` against baselines that don't exist — breaking the
   guards and multiplying each of them 5x in the discovery loop.

**"Hashes differ" is a question, not a verdict.** Drift is a defect only where the
difference is substantive *and* the file executes.

## Also here

35 dispatcher files print `language: C.UTF-8` — a `$LANG` expanded at edit time
instead of the detected language. Small, real, and trivially fixed. (One variant
also assigns `LANG=`, shadowing the locale env var; prefer `LANG_DETECTED`.)

## Out of scope

- The non-rust language validators (inert here).
- Changing which validators exist per tree.
- Cross-repo dedup with `rigorix-sdk` (filed there).

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Substantively-drifted rust validators have one implementation | `validate-ci.sh` |
| 2 | A gating guard fails on unintentional drift and names the disagreeing trees | `validate-guards.sh` |
| 3 | Intentional per-tree differences are recorded, so the guard distinguishes drift from design | review |
| 4 | The `C.UTF-8` message prints the detected language | `validate-ci.sh` |
| 5 | Every per-tree validator still passes when invoked as the hardening loop invokes it | `validate-integration.sh` |

## Implementation

> **Agent:** Read the warning section first. Measure which differences are
> substantive before touching anything, and record intentional differences before
> adding the guard — otherwise the guard will fail on design.

### Steps

1. Re-measure the variant matrix per tree (do not trust the table above; it ages)
2. Classify every difference: substantive / cosmetic / intentional
3. Reconcile only the substantive ones, from the newest variant
4. Fix the `C.UTF-8` message (and prefer `LANG_DETECTED` over `LANG`)
5. Record intentional per-tree differences in a list the guard reads
6. Add `validate-script-drift.sh` and register it in `.pi/guards.tsv` as gating
7. Verify each per-tree copy from the repo root; run `validate-guards.sh`
