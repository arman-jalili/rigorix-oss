# Issue #1277 — IdP unreachable at startup degrades the process

**Branch:** `issue/1277`
**Epic:** authority-boundary · **Component:** Mcp · **Module:** auth
**Priority:** high · **Dependencies:** none

## Defect (measured)

`build_auth_handler()` (`mcp/src/host/mod.rs:1300-1332`) resolves the token
verifier **once**, at startup. `Err` from `idp.discover()` installs
`NullVerifier` for the life of the process, and `NullVerifier` returns
`Unverified` forever. Two consequences:

1. `current_engine_identity()` (`mcp/src/auth/interfaces/mcp/handler_impl.rs:80`)
   collapses `IdentitySource::Unverified` to `None`, so the engine's
   `check_identity_gate` (`engine/.../orchestrator_impl.rs:1249`) reports
   `unauthenticated` for a caller that logged in successfully.
2. The `IdentityRequired` message (`engine/src/orchestrator/domain/error.rs:133`)
   always says "run rigorix_auth_login", which is the one action that cannot help.

## Required end state

- `TokenStatus` gains a fourth literal, `unverified` (serde + Display + the
  frozen note in `mcp/src/auth/domain/status.rs`). One word for one fact: the
  word the engine's refusal already prints.
- `current_engine_identity()` hands the engine the claim it has, tagged
  `Unverified`, instead of dropping it. `None` only when no claim exists.
- `rigorix_auth_status` reports `unverified` for a session whose claim could not
  be checked (token present, `source = unverified`).
- The refusal names the real recovery when `status == "unverified"`
  (restart the MCP server / fix `RIGORIX_IDP_ISSUER`), and keeps the login
  guidance for `unauthenticated`.
- Discovery is re-attempted when attestation is needed and the verifier is the
  offline default — bounded by a per-process cooldown — so a process that
  outlived an IdP outage recovers without a restart.
- The L1 gate still refuses `require_identity` steps while unverified
  (fail closed, unchanged).

## Implementation order (dependencies first)

1. **Domain** `mcp/src/auth/domain/status.rs` — add `Unverified`; rewrite the
   frozen note (`Unauthenticated` = no credential material at all;
   `Unverified` = material present, cannot be checked); extend the Display test.
2. **Application** `mcp/src/auth/application/service_impl.rs::status` — token
   present + `source == Unverified` → `TokenStatus::Unverified`.
3. **Interfaces** `mcp/src/auth/interfaces/mcp/handler_impl.rs` — stop
   collapsing `Unverified`; **trait doc** `interfaces/mcp/mod.rs`.
4. **Infrastructure** new `mcp/src/auth/infrastructure/verifier_impl.rs` —
   `DiscoveryBackedVerifier` implementing
   `rigorix_engine::identity::infrastructure::TokenVerifier`: lazily resolves a
   `JwksVerifier` from `IdpClient::discover()`, caches it on success, retries on
   a cooldown after a failure, and otherwise degrades to `Unverified { reason }`
   naming the cause.
5. **Composition root** `mcp/src/host/mod.rs` — install the lazy verifier
   instead of the one-shot resolve (keep the startup log line).
6. **Engine** `engine/src/orchestrator/domain/error.rs` — `IdentityRequired`
   Display branches on `status`: `unverified` → restart / issuer recovery;
   `unauthenticated` → login recovery.
7. **Server** `server/src/backend.rs::is_authenticated` — keep the old
   semantics (`is_some()` would now be true for an unverified claim); exclude
   `IdentitySource::Unverified`.
8. **Docs** `mcp/.pi/architecture/modules/auth.md` (incl. the
   "IdP unreachable → status unauthenticated" row), `ADR-008`, `mcp/docs/runbook-auth.md`.
9. **Tests** (TDD — failing first): status literal; status DTO for the
   unverified path; `current_engine_identity` returns the unverified claim;
   engine refusal text per status; verifier retry-after-failure and
   no-retry-inside-cooldown.

## Blast radius (gitnexus impact + caller grep)

| Symbol | Direct callers | Risk |
|---|---|---|
| `TokenStatus` | `service_impl::status/refresh`, `dto::StatusOutput/RefreshOutput`, `AuthEvent::AuthStatusChecked`, 6 test files — no exhaustive match outside `status.rs` | LOW |
| `check_identity_gate` | 2 unit tests + `policy_pipeline::enforce_identity_requirement` | LOW |
| `AuthToolHandlerImpl::current_engine_identity` | `host::session_identity` (4 injection sites), `server/src/backend.rs:61`, 1 test stub | LOW-MEDIUM — `server` `is_some()` changes meaning, handled in step 7 |

## Verification

```
cargo build --workspace
cargo test -p rigorix-mcp
cargo test -p rigorix-engine --lib identity
cargo test -p rigorix-server
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```
