# Batch 3 — Issues #1071 + #1073

**Branch:** `feature/audit-integrity-1071-1073`

## #1071 — the label must come from evidence, not configuration
- `HistoryIntegrity::AnchoredPending` (wire: `anchored_pending`).
- Label decided from `(AnchorMode, head)`: anchored + head ⇒ `anchored`;
  anchored without a head ⇒ `anchored_pending`; otherwise `local_unanchored`.
- New negative test: anchored runtime with no verified head is **not** `anchored`.

## #1073 — narrow the outcome, do not overclaim provenance
- `BundleVerification::ChainConsistent` (wire: `chain_consistent`) is the
  verifier's return type; there is no `Verified`/`Provenance` variant.
- Docs state what is and is not established: consistency between projection and
  supplied history; **not** authenticity, anchoring or producer attribution
  (`verify_chain` checks linkage, not anchor signatures or HMACs; `anchor_ref`
  compares the bundle with itself).
- CLI: `check: chain_consistency`, `provenance_established: false`, message says
  provenance needs a separate trusted-anchor check.
- Decision: narrow, not strengthen — shipping an anchor-signed checkpoint +
  trusted-key plumbing is a feature, not a bug fix.

## Notes
- `precondition::infrastructure::runner` tests (3) flaked once under full-suite
  parallelism: each spawns `/bin/sh` with a 5 s budget while 2200+ tests run.
  They pass in isolation and on re-run; pre-existing, not caused by this change.
