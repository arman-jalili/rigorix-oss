# Issue Implementation Groups

**Generated:** 2026-09-25
**Total Issues:** 2
**Source:** `.claude/plans/issues-fetched.json` (open issues #907, #899)
**Epics:** `audit integrity (ADR-016)` · `server mode — C (D-3)`

---

## Grouping Strategy

Issues are grouped by:
1. **Component** — same module/files affected (2–5 issues per batch)
2. **Priority** — Critical > High > Medium > Low
3. **Dependency** — blocking relationships

The two open issues touch **disjoint components** — `engine/sequence_policy` +
`server/` (anchor mode, #899) vs. `server/tests/catalog_parity.rs`
(catalog-subset guard, #907). No component batch is possible. Each is a
single-issue branch (`issue/{number}`), ordered by readiness.

---

## Batch Order (implement top → bottom)

| # | Branch | Issue | Tier | Component | Readiness |
|---|--------|-------|------|-----------|-----------|
| 1 | `issue/899` | #899 OSS-AUDIT-C | High / moderate | `engine/sequence_policy` + `server/` | **READY** — deps met |
| 2 | `issue/907` | #907 OSS-CATALOG-SUBSET-VERIFIER | Low / simple | `server/tests/catalog_parity.rs` | **BLOCKED** — crate not on crates.io |

### Why not one batch?
#899 is an adapter/feature change across the engine seam and the server host;
#907 is a single test-file refactor. Bundling would span two unrelated review
surfaces and violate the "same module/files" rule.

---

## Dependency Graph (must-land-before)

```
ADR-016 (accepted) ─┐
OSS-AUDIT-A (done) ─┼─► #899 OSS-AUDIT-C (anchor client + AnchoredHistoryAdapter)
ENT-AUDIT-B (done) ─┤      deps: ENT-AUDIT-B ledger (#210), #888 server (#888 CLOSED)
#888 OSS-C2 (done) ─┘
                          ⚠ NOT gated on the SDK verifier crate (Ed25519 verify is
                            implemented against the ADR-016 slice contract)

SDK-SCHEMAS-PUBLISH #18 ─► SDK-VERIFIER-PUBLISH #19 (issue CLOSED 2026-09-25T15:39Z)
                              ⚠ crates.io sparse index: `rigorix-verifier` NoSuchKey
                              ⚠ crates.io API: "crate `rigorix-verifier` does not exist"
                              │
                              └─► #907 OSS-CATALOG-SUBSET-VERIFIER  ⛔ BLOCKED
```

- **#899** depends on #888 (merged), OSS-AUDIT-A (Phase A, present) and
  ENT-AUDIT-B (landed in enterprise `#210`, commit `4a8c29f`). Its
  `AnchoredHistoryAdapter` verifies the ADR-016 `HistorySlice` contract
  (Ed25519, `sig` nulled for canonical bytes) which is already frozen; it can
  be built/tested with a locally generated keypair + mock anchor.
- **#907** acceptance criterion #1 requires a **published crates.io**
  dev-dependency. SDK issue #19 is closed but the crate is not yet resolvable.
  Until the publish propagates, adding the dep would break a fresh public
  `cargo test` (violates AC #4/#5).

## Critical Path

`#899` (independent, ready) → then re-evaluate `#907` once crates.io lists
`rigorix-verifier`. If the SDK instead exposes the relation only via the JSON
corpus (no crate), close #907 as **won't-do** per its implementation notes.

---

## Per-Batch Validation (standard)

Each batch before MR:

```bash
cargo build
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --check
bash .pi/scripts/validate-tests.sh
bash .pi/scripts/validate-architecture.sh
bash .pi/scripts/validate-canonical.sh
```

#899 additionally: `validate-security.sh` (Ed25519 verify, fail-closed) and the
`engine/tests/audit_integrity_integration.rs` suite must stay green (Phase A
regression, AC #4).

## Status Tracking

| Issue | Branch | Status |
|-------|--------|--------|
| #899 | `issue/899` | ready — planned |
| #907 | `issue/907` | blocked — crates.io publish not propagated |
