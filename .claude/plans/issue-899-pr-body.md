# feat: #899 OSS-AUDIT-C — ADR-016 Phase C anchored mode

Turns on the ADR-016 guarantee in the OSS host: history reads are
anchor-signed and **verified before the matcher sees them**; in `anchored`
mode a consequential run fails closed when the anchor is unreachable or the
slice is forged.

This is an **adapter swap behind the existing `ExecutionHistory` port** — the
matcher (`matcher.rs`, matching logic in `service_impl.rs`) is unchanged, per
ADR-016 Ownership ("no policy in the anchor").

## What changed

**Contract + verification (engine, shared kernel)** — `engine/src/audit/`
- `domain/anchor.rs`: `HistorySlice` / `HistoryActionRef` / `AnchorMode` — a
  frozen struct copy of `rigorix-verifier`'s wire contract, with
  `signing_bytes()` = compact JSON with `sig` nulled (field order matches the
  enterprise signer).
- `infrastructure/anchor.rs`: `AnchorRuntime`, `AnchorSliceSource` port,
  `HttpAnchorClient` (`GET {base}/v1/history?scope=&since=`), `verify_slice`
  (Ed25519), lazy process-global runtime from `RIGORIX_ANCHOR_URL` +
  `RIGORIX_ANCHOR_PUBLIC_KEY` (+ `RIGORIX_ANCHOR_SCOPE`).

**Adapter (engine)** — `sequence_policy/infrastructure/history_anchored.rs`
- `AnchoredHistoryAdapter` fetches the signed projection, **verifies the
  signature before returning any action**, and reports `is_anchored() == true`.
  Unreachable / unsigned / forged / out-of-scope ⇒ `Err`, which the existing
  `SequencePolicyServiceImpl` fail-closes on for deny-class runs.

**Mode selection + tagging**
- `SequencePolicySetup::from_env` uses the anchored adapter when an anchor is
  configured; otherwise the Phase A `EnvelopeHistoryAdapter` is unchanged.
- `AuditEnvelope.anchor_head` — new additive, `serde(skip_serializing_if =
  "None")` field (unsigned local envelopes serialize byte-identically, so the
  local chain/HMAC is unaffected).
- `AuditEnvelopeFactoryImpl::with_anchor` tags `history_integrity = anchored`
  and binds the verified head; default stays `local_unanchored`.
- Wired in the MCP host, CLI, and actions composition roots.

**Reporting**
- `rigorix.system.version` now reports `history_integrity`
  (`local_unanchored` | `anchored`) and `anchor.{id,head}` (`server/src/anchor/`).

## Acceptance criteria

| # | Criterion | Evidence |
|---|-----------|----------|
| 1 | Forged slice rejected before matching | `anchor::tests::rejects_a_forged_slice`; `anchored_forged_slice_fails_closed` |
| 2 | Unreachable anchor refuses a consequential run | `anchored_unreachable_anchor_fails_closed` |
| 3 | Valid slice allows; envelope records head + mode | `anchored_valid_slice_enforces_consequential_rule`; `anchored_factory_tags_mode_and_head` |
| 4 | `local_unanchored` unchanged | `audit_integrity_integration` (5), `deny_class_cross_run_refuses_on_unanchored_history_by_default` |
| 5 | `system.version` reports mode + head | `system_version_reports_engine_and_api_versions` |
| 6 | Matcher unchanged (adapter swap only) | no diff in `matcher.rs` / matching logic |
| 7 | CI green; fmt/clippy `-D warnings` | full workspace gate below |

## Validation

```
cargo build --workspace            ✅
cargo test  --workspace            ✅ 52 suites, 0 failed
cargo clippy --workspace --all-targets -- -D warnings   ✅
cargo fmt --all --check            ✅
```

## Blast radius / notes

- `detect_changes` reports **critical** scope (40 symbols / 20 files). The
  high-risk touch is `AuditEnvelope`: the new field is additive and
  serde-defaulted, and all 7 in-tree construction sites were updated. The
  chain/HMAC round-trip regression suite is green, proving local_unanchored
  canonical bytes are unchanged.
- `validate-architecture` / `validate-canonical` report the same structural
  failures on `main` (they assume a root `src/domain/` layout, not a cargo
  workspace) — pre-existing, unrelated to this change.
- The enterprise `HistorySlice` is exposed today as the application service
  `audit_ingestion::ledger::history_slice(producer_id, since)`; the HTTP
  `/v1/history` read surface is the forward client contract. Tests use signed
  fixtures, so no live anchor is required.

## Out of scope
- The ledger (ENT-AUDIT-B), local tamper-evidence (Phase A), Phase D
  (trusted execution boundary / host-side signing).
