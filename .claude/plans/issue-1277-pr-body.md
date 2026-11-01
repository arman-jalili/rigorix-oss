# fix: #1277 — an unreachable IdP at startup no longer disables attestation for the process

Implements **#1277** (`ISSUE-IDP-UNREACHABLE-AT-STARTUP-DEGRADES-THE-PROCESS`).

## What was wrong

`build_auth_handler()` resolved the token verifier **once**, before the auth
service existed. A single failed discovery request in the first second of the
process installed `NullVerifier` for the life of the process — and
`current_engine_identity()` collapsed its `Unverified` marker to `None`, so the
L1 gate printed at a caller who had just logged in:

```
startup : WARN auth: IdP discovery failed (…) — claims degrade to unverified
login   : AuthLoginSucceeded { subject: 5404ca13-… }
status  : Authenticated, source: unverified
run     : Step 'pay_a' requires an attested identity but the caller is unauthenticated
          — run rigorix_auth_login (or execute as a local principal) first
```

Two wrong reports: the caller *did* authenticate, and the cause — and the only
recovery — appeared nowhere.

## What changes

| Surface | Before | After |
|---|---|---|
| `TokenStatus` | `authenticated \| expired \| unauthenticated` | + `unverified` — credential present, cannot be checked |
| `rigorix_auth_status` | `authenticated` + `source: unverified` | `unverified` + the claim still attached |
| `current_engine_identity()` | `Unverified` → `None` | hands the engine the claim, tagged `unverified` |
| `IdentityRequired` (unverified) | "run rigorix_auth_login" | "restart the MCP server once the IdP answers, or fix `RIGORIX_IDP_ISSUER`" |
| Verifier resolution | once, at composition | at attestation time, with a bounded retry |

**The verdict is the weakest property the row carries.** `unverified` is the
word the engine's refusal already printed, so status output and refusal say one
word, not two.

**The decision is no longer made once.** `DiscoveryBackedVerifier` resolves the
JWKS when a claim needs verifying, caches on success, and re-attempts on a 30 s
per-process cooldown after a failure — a process that outlived an IdP outage
recovers without a restart, while a flapping IdP is not hammered once per tool
call. The lock is deliberately held across `discover()` so a cold process makes
one attempt, not one per concurrent call; discovery itself is bounded by the
client's timeout.

## Not changed (deliberately)

- **The gate still fails closed.** An `unverified` claim is still refused on
  `require_identity` steps — the exit posture is identical; only the report is
  truthful. No fail-open, no unsigned acceptance.
- **`server`'s `is_authenticated()` keeps "attested" semantics.** It would have
  silently changed meaning (`is_some()` is now true for an `unverified` claim);
  it explicitly excludes `IdentitySource::Unverified`, preserving behaviour.

## Files

- `mcp/src/auth/domain/status.rs` — fourth literal, frozen note, Display.
- `mcp/src/auth/application/service_impl.rs` — `status()` verdict follows the source.
- `mcp/src/auth/interfaces/mcp/handler_impl.rs` — stop collapsing `Unverified`.
- `mcp/src/auth/infrastructure/discovery_verifier.rs` *(new)* — lazy, bounded discovery.
- `mcp/src/host/mod.rs` — composition root wires the lazy verifier.
- `engine/src/orchestrator/domain/error.rs`, `mcp/src/execution_tools/domain/error.rs`
  — the refusal names the recovery that can help.
- `server/src/backend.rs` — `is_authenticated()` semantics preserved.
- `mcp/.pi/architecture/modules/auth.md`, `ADR-008`, `mcp/docs/runbook-auth.md`
  — the four literals, the gate's fail-closed behaviour, the new runbook entry.

## Acceptance criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | `TokenStatus` gains `unverified`, distinct from `unauthenticated` | ✅ `status.rs` + freeze test |
| 2 | `current_engine_identity` hands over the unverified claim | ✅ `handler_impl::tests` |
| 3 | status + refusal report `unverified`; refusal names restart/issuer | ✅ engine + mcp tests |
| 4 | discovery re-attempted, bounded per process | ✅ `discovery_verifier::tests` |
| 5 | the three existing literals keep meaning and tests | ✅ 3916 tests green |
| 6 | contract notes updated (`status.rs`, `auth.md`, `ADR-008`, runbook) | ✅ |

## Validation

```
cargo build --workspace                      # ok
cargo test --workspace                       # 92 suites, 3916 passed, 0 failed
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo fmt --all --check                      # clean
engine/.pi/scripts/validate-{ci,tests,operations,integration,architecture,canonical}.sh  # all PASS
.pi/scripts/validate-security.sh             # PASS (2/2)
.pi/scripts/ci/run_preflight.sh              # 5 pass, 8 skip, 1 pre-existing fail (see below)
```

**Pre-existing, not from this PR:** `.pi/scripts/ci/check_architecture_conformance.sh`
exits 1 *after* printing every check as PASS/SKIP — it dies before its own
summary, so `validate-architecture-readiness.sh` reports "conformance failed".
Reproduced identically on `origin/main` in a clean worktree; conformance itself
is green. Left untouched — it is a separate validator bug, not a bypass.
