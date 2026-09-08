# Contract Conformance — F-20260907-01 / F-20260907-02 (2026-09-08)

**Status:** Conformance round for rigorix-sdk schemas v1 (`b23492e`), contract-sync
milestone 1. Companion issues #876 (envelope v2) + #877 (policy/claims). Cross-repo:
rigorix-sdk PR (fixtures + verifier parity) referenced from #876.

## Findings (evidence first)

### F-1: Envelope canonicalization was NOT byte-reproducible (drift, both repos)

- **Evidence:** probe against rigorix-verifier — `canonical_bytes(json)` twice in
  one process returned *different* bytes once a map-typed envelope field
  (`scoring_results` / `dimensions`) had >1 entry; `verify_signature()` FAILED
  (`SignatureMismatch`) even *within* the verifier crate. Root cause: serde
  serializes `std::collections::HashMap` keys in hash-random per-process
  iteration order — the "HashMap fields serialize deterministically" assumption
  in `envelope_factory_impl.rs::compute_signature` was false for typed HashMap
  fields (it holds only for `serde_json::Value`, whose Map is BTreeMap-backed).
- **Impact:** the documented canonical form (SHA-256-HMAC over
  `to_string(envelope with signature=None)`) could not be reproduced byte-exact
  by any independent verifier for rich envelopes — invalidating the core
  Strategy-A claim ("HMAC trail checkable anywhere").
- **Fix (engine-side + SDK mirror):** map-typed envelope fields serialize with
  **sorted keys** via a `serialize_with` helper in both
  `engine/src/audit/domain/envelope.rs` (this repo) and the rigorix-verifier
  frozen copies (rigorix-sdk PR). JSON shape unchanged (object key order is not
  semantic); canonical bytes are now deterministic. Schema description in
  rigorix-sdk `schemas/envelope.json` documents the sorted-key rule (contract
  decision made in the SDK repo with evidence, not silently here).
- **Post-fix proof:** all 5 real engine-signed fixtures verify byte-exact via
  rigorix-verifier + validate vs `envelope.json` v1.

### No other drift

- EventStatus casing (`Success|Failure|Skipped|Cancelled`) confirmed engine-truth.
- Real conference-demo `sequence-policy.toml` (incl. R7 `history` rule) parses,
  serializes and conforms to `policy.json` v1; IdentityClaim variants +
  IdentityRef conform to `claims.json` v1. No schema edits needed.

## What landed

- `engine/src/audit/domain/envelope.rs` + `envelope_factory_impl.rs` — sorted-key
  map serialization (canonical-bytes determinism).
- `engine/tests/conformance/**` — feature-gated suite (`--features conformance`,
  `RIGORIX_SDK_SCHEMAS`): envelope (minimal unsigned / signed / rich), policy
  (real conference-demo TOML → schema), claims (IdentityClaim variants +
  IdentityRef), fixture generator (`RIGORIX_FIXTURE_OUT`, env-gated).
- `.github/workflows/ci.yml` — mandatory conformance job: checkout rigorix-sdk
  (RIGORIX_SDK_TOKEN) → run conformance suite → run rigorix-verifier
  (independent-HMAC proof over committed fixtures). Wired into the final gate.
- rigorix-sdk (PR): `schemas/fixtures/envelope/*.json` (5 real engine-signed
  fixtures + README w/ test-only key), verifier sorted-key parity fix, fixture
  test (`engine_signed_fixtures_verify_byte_exact`), `envelope.json` contract
  description note, contract-sync progress-log entry.

## Validation

```bash
RIGORIX_SDK_SCHEMAS=<sdk>/schemas cargo test -p rigorix-engine --features conformance
# 10 passed (envelope 3, policy 2, claims 5); fixture generator env-gated
cd <sdk>/rust && cargo test -p rigorix-verifier   # 9 passed incl. fixture byte-exact proof
```
