---
guardian_issue:
  id: "ISSUE-LEGACY-LEDGER-ADOPTION-IS-A-DOWNGRADE"
  title: "stripping the consumption ledger's wrapper turns a truncation into a 'verified' ledger, because bare-map adoption runs before authentication"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1247
  dependencies:
    - "#1244 — the authenticated ledger"

  intent: |
    #1244 made the ledger `{ version, entries, mac }` and refused a marker set whose MAC does
    not match. It kept a migration path: a pre-#1244 bare map is adopted and re-sealed on the
    next write.

    The migration path runs *before* authentication and cannot tell a legacy file from a
    sealed file whose wrapper was removed. So the attack #1244 closed is still available, one
    wrapper strip away:

    ```
    sealed,  marker removed → Approval consumption ledger integrity failure at '<path>'   refused
    stripped to {},  same deletion → verified, 0 marker(s)                                accepted
    ```

    Both measured against the same store and the same key. The deletion is identical; the only
    difference is that the second presents itself as pre-#1244, and the loader adopts it and
    re-seals the emptiness.

    `doctor` then reports the truncated ledger as `verified, 0 marker(s)` — a word that means
    a MAC was checked, printed for a file that carried no MAC at all.

  in_scope:
    - "A ledger without a wrapper is not usable when a run key is configured: it is refused, and the refusal names the file and the migration command"
    - "Migration is an explicit action that reports the marker count it adopted"
    - "'verified' is printed only when a MAC was checked; an unauthenticated ledger says so"
    - "Seeds: sealed + marker removed → refused; stripped + marker removed → refused by name; explicit migration → adopts and reports the count; sealed → verified"

  out_of_scope:
    - "The sealed format itself, and the run-key fingerprint in the filename, which work"
    - "Rolling back the store and the ledger to an older authentic pair — no local anchor detects that"
    - "Unbound mode, where no key exists and nothing about the store is authenticated either"

  affected_layers:
    domain:
      - "Modified: ledger authentication is a precondition of use, not a property of the sealed shape"
    application:
      - "Modified: explicit migration entry point"
    infrastructure:
      - "Modified: legacy adoption moves behind the migration command"
    api:
      - "Modified: doctor's ledger line distinguishes authenticated, unauthenticated and absent"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs:155 — 'Pre-#1244 bare map. Adopt its current contents and seal them', reached whenever the wrapper is absent"
    - evidence: "measured: sealed ledger, one marker removed → 'Approval consumption ledger integrity failure at …'; wrapper replaced with {} → 'verified, 0 marker(s)', store unchanged, key unchanged"
    - evidence: "engine/tests/approval_store_rollback_integration.rs — a_fresh_store_without_a_marker_still_works proves an absent marker does not stop a valid pending record"

  acceptance_criteria:
    - "With a run key configured, a ledger lacking `version`/`entries`/`mac` is refused, naming the file and the migration command"
    - "The refusal is distinguishable from a MAC mismatch in its message"
    - "The migration command adopts a bare map, prints how many markers it adopted, and writes the sealed form"
    - "doctor prints 'verified' only after a MAC check; an unauthenticated ledger prints its state instead"
    - "Seeds fail if the adoption path is restored to the loader"

  validators:
    - ci
    - tests
    - security
    - docs

  implementation_notes: |
    - This is the same shape as #1244 one level up: authority that can be presented in an older,
      unprotected form is authority an attacker chooses the form of. An empty file is already
      refused with "an absent first-run ledger must be deleted, not truncated" — a bare map is
      the same truncation wearing the one format the loader still trusts.
    - The upgrade note and CHANGELOG must carry the migration command, because a fail-closed
      loader turns an un-migrated ledger into a refused dispatch.
    - Do not make the migration automatic on first write. That is how the wrapper strip works.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: engine/src/approval/application/service_impl.rs"
    - "modify: cli/src/cli_boundary/doctor.rs"
    - "modify: engine/tests/approval_store_rollback_integration.rs"
    - "modify: docs/upgrade-notes.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-LEGACY-LEDGER-ADOPTION-IS-A-DOWNGRADE

## The two probes, one deletion, two verdicts

Same store, same key, same removed marker:

```
$ ls ~/.rigorix/approval-consumption-*.json
approval-consumption-f0e80a14fdcb8b7c.json

# sealed file, marker removed from `entries`, MAC left as it was
→ approval store unavailable: …: Approval consumption ledger integrity failure at
  '/Users/arman/.rigorix/approval-consumption-f0e80a14fdcb8b7c.json'

# same file with the wrapper replaced by {}
→ mode: intent-bound; … 3 record(s) verify …; consumption ledger … (outside agent-writable
  workspace; verified, 0 marker(s))
```

The first is #1244 working. The second is #1244 being avoided by choosing the format the loader
migrates from.

## Why the loader cannot tell

```rust
if value.get("entries").is_some() {
    /* authenticate: version, then MAC over entries */
} else {
    // Pre-#1244 bare map. Adopt its current contents and seal them…
    Ok((entries, true))
}
```

There is no evidence in the file, or anywhere else, that it was ever sealed — the check for
"was this authenticated before" is the presence of the wrapper the attacker removes. An empty
file is already refused (*"an absent first-run ledger must be deleted, not truncated"*); a bare
map is the same truncation in the one format still trusted.

## Why "verified, 0 marker(s)" is the sharper half

`verified` is the word a reader relies on, and here it is printed for a file with no MAC. It is
the failure this repo keeps finding: a check that did not run, printed as a check that passed.
The ledger line must say what was actually established — authenticated and how many markers, or
unauthenticated and how many, or absent.

## The required end state

In a keyed deployment an unsealed ledger is refused by name with the migration command; migration
is explicit, reports what it adopted, and writes the sealed form; `doctor` says `verified` only
after a MAC check; and the upgrade notes carry the migration step, because a fail-closed loader
turns an un-migrated ledger into a refused dispatch.

## How it is proven

| seed | expected |
|---|---|
| sealed ledger, one marker removed | refused: MAC mismatch, both paths named |
| wrapper stripped, marker removed | refused: unauthenticated ledger, migration command named |
| `rigorix approve ledger migrate` on a real legacy file | adopts N markers, prints N, writes the sealed form |
| the sealed ledger, untouched | `verified, 1 marker(s)` |
| adoption restored to the loader | the second seed passes silently — the test fails |
