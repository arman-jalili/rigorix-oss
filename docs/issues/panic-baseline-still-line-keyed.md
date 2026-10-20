---
guardian_issue:
  id: "ISSUE-PANIC-BASELINE-STILL-LINE-KEYED"
  title: "the panic baseline is still keyed by file:line, so any edit above a site reads as a new panic — the unsafe baseline was re-keyed, this one was not"
  epic: "self-owned vulnerability checks"
  component: "Guards"
  module: "governance"
  status: planned
  priority: medium
  dependencies:
    - "#1088 — re-keyed the UNSAFE baseline to file::code#occurrence; the panic baseline was left untouched"
    - "#1082 — the guard-suite spike"

  intent: |
    Apply to the panic baseline the keying that #1088 already applied to the unsafe baseline.

    Both baselines are ratchets: they fail when the set of risky sites changes. Both were keyed by
    `file:line`. A line number is not an identity — adding a comment, an import, or a lint header
    above a site moves every site below it, so an unrelated edit reads as a removal plus an
    addition.

    #1088 fixed this for `unsafe` and proved it. The identical mechanism one file over still has the
    defect, and it will fire the next time anything above a panic site shifts.

  in_scope:
    - "Key the panic baseline by file::code#occurrence, as the unsafe baseline is"
    - "Regenerate .pi/panic-baseline.tsv in the new format"
    - "Add the same shift-resilience check the unsafe baseline now has"
    - "Keep --self-test and the existing human-readable reason column"

  out_of_scope:
    - "Re-deciding any baselined panic (they are recorded decisions)"
    - "The unsafe baseline (#1088, already correct)"
    - "Removing the baseline entirely (the ratchet is the point)"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: .pi/scripts/validate-panic-sites.sh"
      - "Modified: .pi/panic-baseline.tsv (regenerated)"
    api:
      - "None"

  canonical_references:
    - fixed_sibling: ".pi/scripts/validate-unsafe-sites.sh:87 — base = f\"{f['file']}::{f['code'].strip()}\" then #occurrence"
    - evidence: ".pi/scripts/validate-panic-sites.sh:101 — rows.append((f\"{f['file']}:{f['line']}\", ...)) — still line-keyed"
    - evidence: ".pi/panic-baseline.tsv — locations are path:line (e.g. mcp/src/engine_wiring.rs:31)"
    - evidence: "reproduced: inserting 3 comment lines above engine_wiring.rs:31 yields '1 NEW production panic site (…:34)' plus '1 baseline entry no longer present (…:31)'"
    - evidence: "same experiment against the unsafe baseline stays green (2 production sites, all baselined)"

  acceptance_criteria:
    - "Inserting N lines above any baselined panic site leaves the validator green"
    - "Removing a panic site still reports it as stale"
    - "Adding a genuinely new panic site still fails"
    - "--self-test still catches a planted unwrap"
    - "The reproduced shift experiment is recorded in the PR (red before, green after)"

  validators:
    - ci
    - security

  implementation_notes: |
    Copy the unsafe implementation rather than inventing a second scheme — two validators with two
    keying strategies is how they drift, and drift between ratchets is silent in both directions.

    The occurrence counter matters: `{file}::{code}#{n}` distinguishes two identical code lines in
    one file, which `file:line` did implicitly. Reuse the unsafe implementation's `seen` map so the
    two behave identically.

    Note the failure direction: this is a false *positive*, so it fails loud and gets reverted or
    `--update`d. That is why it survived — the workaround (regenerate until green) is one command,
    and it silently accepts whatever else changed in that run. The unsafe baseline bit for the same
    reason on a lint header, which is how it got fixed.

  file_changes:
    - "modify: .pi/scripts/validate-panic-sites.sh"
    - "modify: .pi/panic-baseline.tsv"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-PANIC-BASELINE-STILL-LINE-KEYED

## The asymmetry

Two ratchets, one fixed:

| baseline | key | resilient to line shifts |
|---|---|---|
| `.pi/unsafe-baseline.tsv` (#1088) | `file::code#occurrence` | **yes** |
| `.pi/panic-baseline.tsv` | `file:line` | **no** |

```rust
// .pi/scripts/validate-unsafe-sites.sh:87-89  (fixed)
let base = f"{f['file']}::{f['code'].strip()}";
rows.append((f"{base}#{seen[base]}", ...))

// .pi/scripts/validate-panic-sites.sh:101  (unchanged)
rows.append((f"{f['file']}:{f['line']}", f["kind"], cls, sev, why))
```

## Reproduced

Insert three comment lines above `mcp/src/engine_wiring.rs:31`:

```
❌ 1 NEW production panic site(s) — not in the baseline:
   [medium/startup] mcp/src/engine_wiring.rs:34

⚠️  1 baseline entr(ies) no longer present (stale):
   mcp/src/engine_wiring.rs:31
```

The same experiment against the unsafe baseline: `✅ unsafe surface unchanged (2 production sites,
all baselined)`.

Nothing was added, nothing was removed, nothing got riskier — a comment moved.

## Why it matters anyway

It fails **loud**, so nobody ships a regression on it. The cost is different and real: the fix is
`--update`, which regenerates the whole baseline. That silently re-accepts every other change in
that run — so the ratchet is briefly a rubber stamp, and the habit it teaches is "regenerate until
green".

That is the same lesson as the unsafe baseline: it bit on a generated lint header, and the fix was
at the key rather than at the invocation. #1088 did that correctly for one of the two.

## Acceptance

Load-bearing: **the shift experiment.** Insert lines above a site → green. Remove a site → stale.
Add a real site → fail. All three, recorded in the PR.
