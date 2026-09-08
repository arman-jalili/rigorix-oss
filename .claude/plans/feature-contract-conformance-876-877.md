# Batch Plan — feat/contract-conformance-876-877

**Issues:** #876 F-20260907-01 (envelope-v2 conformance), #877 F-20260907-02 (policy/claims conformance)
**Branch:** `feat/contract-conformance-876-877`
**Cross-repo:** rigorix-sdk PR (fixtures + frozen-verifier parity fix). CI secret `RIGORIX_SDK_TOKEN` set.

## Key finding (evidence-based)

Probe in rigorix-verifier (`probe_map_order`):
- `canonical_bytes()` twice in-process differs when a HashMap-typed field has >1 entry; `verify_signature` FAILS even within the crate.
- Root cause: envelope maps are `std::collections::HashMap`; serde emits keys in hash-random iteration order. Engine comment "serde_json::Map is BTreeMap-backed → HashMap fields serialize deterministically" is **false for typed HashMap fields** (only true for `serde_json::Value`).
- Impact: byte-exact HMAC reproduction (core #876 DoD) is impossible for rich envelopes until maps serialize deterministically on **both** sides.
- Fix: use `BTreeMap` for `AuditEnvelope.scoring_results` + `ScoringResultRef.dimensions` (engine + SDK frozen copies). JSON shape unchanged; canonical bytes become sorted. SDK schema description note updated (contract decision documented in SDK PR).

## Implementation order (commit per step)

1. **#876 engine**: deterministic envelope maps — `BTreeMap` swap in envelope.rs + dto/mod.rs; update construction sites (orchestrator_impl, audit_*_impl, history.rs, mocks, tests).
2. **#876 engine**: conformance harness — engine `[[test]] conformance` under feature `conformance`; jsonschema dev-dep; `RIGORIX_SDK_SCHEMAS` env schema loading; envelope cases (a) minimal unsigned evidence_degraded=false, (b) HMAC-signed, (c) rich envelope built via real factory + real event payloads (identity/approval/scope_violations/sequence_policy_findings/scoring_results/decision_context_ref). No hand-rolled JSON.
3. **#877 engine**: policy + claims conformance — real conference-demo `.rigorix/sequence-policy.toml` (vendored pinned copy + provenance note) → `TomlSequencePolicyRepository::load_config` → serialize `SequencePolicyConfig` → validate vs `policy.json` (incl. R7 history rule); serialize `IdentityClaim` variants + `IdentityRef` → validate vs `claims.json`.
4. **#876 fixtures**: env-gated conformance test writes real signed envelopes (fixed key `fixture-hmac-key-2026`) → commit 3–5 JSON files to rigorix-sdk `schemas/fixtures/envelope/` via SDK PR.
5. **CI**: rigorix-oss ci.yml conformance job (checkout rigorix-sdk with token, set `RIGORIX_SDK_SCHEMAS`, `cargo test -p rigorix-engine --features conformance`); add to gate.

## Cross-repo SDK PR
- `schemas/fixtures/envelope/*.json` + README (test-only KEY note)
- verifier frozen copies HashMap → BTreeMap (parity fix)
- `schemas/envelope.json` canonicalization description note (sorted map keys)
- verifier fixture test: load committed fixtures → validate_schema + verify_signature byte-exact

## Validation
```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
RIGORIX_SDK_SCHEMAS=<sdk>/schemas cargo test -p rigorix-engine --features conformance
# per-crate check_*.sh suite (engine) — integration stage convention
```
