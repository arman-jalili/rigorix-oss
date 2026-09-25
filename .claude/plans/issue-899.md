# Plan — `issue/899` · OSS-AUDIT-C: anchored mode

**Issue:** #899 · **Epic:** audit integrity (ADR-016) · **Tier:** High / moderate
**Branch:** `issue/899` (single-issue — distinct component from #907)
**Status:** ready to implement (deps: #888 merged, OSS-AUDIT-A present, ENT-AUDIT-B landed `#210`)

---

## Goal

Turn on the ADR-016 guarantee: the OSS host gains an **anchor client** and an
**`AnchoredHistoryAdapter`** that verifies an anchor-signed `HistorySlice`
(Ed25519) *before* the matcher sees any evidence. In `anchored` mode a
**consequential** (history-dependent / deny-class) run **fails closed** when the
anchor is unreachable or the slice is unverifiable. `rigorix.system.version`
reports the active mode + anchor identity/head. `local_unanchored` behavior is
unchanged.

**Hard rule (ADR-016 Ownership):** this is an **adapter swap behind the
`ExecutionHistory` port**. The matcher (`matcher.rs`) and matching logic in
`service_impl.rs` must NOT change.

---

## The seam (already exists)

`engine/src/sequence_policy/infrastructure/history.rs`

```rust
pub trait ExecutionHistory {
    async fn prior_actions(&self, since: DateTime<Utc>) -> Result<Vec<HistoryAction>, SequencePolicyError>;
    fn is_anchored(&self) -> bool { false }   // Phase C adapter overrides -> true
}
```

`SequencePolicyServiceImpl` (`application/service_impl.rs:222`) already refuses a
deny-class cross-run match when `!allow_unanchored_history && !history.is_anchored()`
via `SequencePolicyError::HistoryUnanchored`. So once the anchored adapter
returns `is_anchored() == true` **and** only after signature verification, the
existing fail-closed path composes without touching the matcher.

---

## File changes

| Layer | File | Change |
|-------|------|--------|
| engine/infrastructure | `engine/src/sequence_policy/infrastructure/history_anchored.rs` | **new** `AnchoredHistoryAdapter` — fetch signed slice, verify Ed25519 over `sig`-nulled canonical JSON, map `HistoryActionRef -> HistoryAction`, `is_anchored() -> true` |
| engine/infrastructure | `.../infrastructure/mod.rs` | export the new adapter |
| server | `server/src/anchor/mod.rs` | **new** — client (push envelope receipt; fetch signed slice) + `verify_slice` |
| server | `server/src/anchor/client.rs` | **new** — `AnchorClient` over `reqwest` (already a workspace dep); timeout + typed errors |
| server | `server/src/config.rs` | **new** — anchor endpoint + hex public key; mode = `anchored` iff both set, else `local_unanchored` |
| server | `server/src/version.rs` | `system_version()` gains mode + anchor identity/head (accept a small `VersionInfo`/state arg) |
| server | `server/src/rpc.rs` | pass the mode/head into `system_version` |
| server | `server/src/backend.rs` + `server/src/main.rs` | build the anchor client; select adapter by config; wire into the composed host |
| mcp | `mcp/src/host/mod.rs` | mode-aware history adapter selection in the composition root (mirror `EnvelopeHistoryAdapter` wiring at `:1409`) |
| tests | `engine/tests/` + `server/tests/` | forged slice rejected; unreachable anchor refuses consequential run; valid slice allows + envelope records head/mode; `local_unanchored` unchanged |

**Crypto:** add `ed25519-dalek = "2"` + `hex` to the relevant crate(s). The SDK
`rigorix-verifier` is a *possible* verifier but is **not on crates.io** (see
#907); do not take a path/git dep — implement against the frozen ADR-016 slice
contract (canonical bytes = compact JSON with `sig: null`).

## Contract to verify (frozen by ADR-016 + enterprise `domain/anchor.rs`)

```jsonc
// HistorySlice (anchor-signed projection)
{ "scope": "...", "since": "<rfc3339>",
  "actions": [ { "node": "...", "principal": "..."|null, "at": "<rfc3339>",
                 "effect_key": "..."|null } ],
  "head_hash": "<hex>", "sig": "<hex ed25519>" }
```

Canonical signing bytes: the struct serialized with `sig` set to `null`.
`verify_slice(bytes, sig_hex, public_key_hex)` → reject on any mismatch.

---

## Implementation order (dependency-first, one commit per step)

1. **engine** — `AnchoredHistoryAdapter` + unit tests (forged slice rejected;
   valid slice returns actions; `is_anchored() == true`). Compile/test engine alone.
2. **server** — `anchor::verify_slice` + `AnchorClient` (fetch/push, timeout),
   with a signed-slice fixture test (ephemeral keypair).
3. **server config + mode selection** — `AnchorConfig`; adapter chosen by mode.
4. **version reporting** — `system_version` mode + head; unit test both modes.
5. **composition wiring** — server `backend.rs`/`main.rs` + `mcp/src/host` select
   the anchored adapter when configured; else keep `EnvelopeHistoryAdapter`.
6. **integration** — unreachable anchor refuses a consequential run in anchored
   mode; non-consequential run unaffected; `local_unanchored` regression green.

## Validation (Phase 6 gate)

```bash
cargo build
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --check
bash .pi/scripts/validate-tests.sh
bash .pi/scripts/validate-architecture.sh
bash .pi/scripts/validate-canonical.sh
bash .pi/scripts/validate-security.sh   # Ed25519 verify, fail-closed
```

Regression: `engine/tests/audit_integrity_integration.rs` must stay green
(Phase A AC #4 — `local_unanchored` unchanged).

---

## Acceptance criteria mapping

| # | Criterion | Where |
|---|-----------|-------|
| 1 | Forged slice rejected before matching | engine unit + server fixture |
| 2 | Unreachable anchor ⇒ consequential run refused (anchored) | server integration |
| 3 | Valid slice allows; envelope records head + `history_integrity = anchored` | integration |
| 4 | `local_unanchored` unchanged | regression suite |
| 5 | `system.version` reports mode + head | version unit |
| 6 | Matcher unchanged (adapter swap only) | `git diff` review on `matcher.rs`/`service_impl.rs` |
| 7 | CI green; fmt/clippy `-D warnings` | CI |

---

## Pre-edit gates (AGENTS.md)

Before editing, run `impact` (upstream) on: `ExecutionHistory`, `EnvelopeHistoryAdapter`,
`SequencePolicyServiceImpl`, `system_version`, and the mcp host composition, and
report blast radius. Run `detect_changes()` before committing. Warn if any
result is HIGH/CRITICAL.

## Open risks

- `system_version()` is currently a no-arg free fn called directly in `rpc.rs` —
  threading mode/head requires a signature change at its call site (small, but
  run impact first).
- Host composition lives in `mcp/src/host/mod.rs`; the server delegates to it
  (`HostBackend`), so mode selection must be done where `EnvelopeHistoryAdapter`
  is constructed (`engine/.../factory.rs:335`, `mcp/src/host/mod.rs:1409`).
- No live enterprise anchor is required for tests — use an ephemeral Ed25519
  keypair + in-test signer; a live `reqwest` call is only for the unreachable-path
  test (point at a closed port / mock).
