---
guardian_issue:
  id: "ISSUE-PRODUCTION-PANIC-SURFACE"
  title: "Production panic surface: 41 baselined sites; poisoned-lock panics in server are the reachable class"
  component: "Security / Robustness"
  status: planned
  priority: high
  dependencies: []
  related_docs: ["docs/panic-audit-2026-10-12.md", ".pi/panic-baseline.tsv"]
---

# ISSUE-PRODUCTION-PANIC-SURFACE — panics that abort instead of failing a step

## Origin, and the claim we were sent

An unsolicited email (disposable-mail domain, no version, no reproduction)
claimed:

> `actions/src/action_entrypoint/application/context_builder_impl.rs:636` unwraps
> fallible boundary state during execution step. crashes the runner process.

**Verified false.** Line 636 is `let output = builder.build(input).await.unwrap();`
inside `#[cfg(test)] mod tests` (line 496 onward). Test code is not compiled into
the binary, so it cannot crash a runner. The report is consistent with a naive
`grep '\.unwrap()'` that does not exclude `cfg(test)`.

The **class** is real, so it is owned here rather than dismissed with the
report. Full inventory: `docs/panic-audit-2026-10-12.md`.

## Why it matters here specifically

A Rust panic aborts the thread/process. A runner or host has no caller to catch
it, so one reachable panic takes down an **entire execution** (or the whole
server) instead of failing one step — the opposite of the fail-closed,
step-scoped refusal the engine is built around (ADR-009, ADR-017).

## Inventory

41 production panic-class sites, all recorded in `.pi/panic-baseline.tsv` and
enforced by `.pi/scripts/validate-panic-sites.sh` (gating, see #2 below).

| severity | count | class |
|---|---|---|
| high | 8 | poisoned `Mutex` lock in `server` |
| medium | 10 | `api` validating constructors (2), `startup` fail-fast (3), `invariant` (5) |
| low | 23 | `invariant` (5, tokio watch), `const` (18, compile-time literals) |

## The reachable class — fix this first

| site | problem |
|---|---|
| `server/src/events.rs:124,147` | `.lock().expect("event replay lock")` — poisoned-lock panic on the SSE replay path |
| `server/src/mcp_http.rs:94,106,111,124,133,140` | `.lock().expect("session lock")` — poisoned-lock panic in the MCP session store |

`.lock().expect(..)` panics when a mutex is **poisoned**, which happens after any
panic while the lock was held. That converts a one-off fault into a **permanent**
one: every subsequent request panics on the same line. `mcp_http.rs` is the
newest surface (#915) and the most exposed — every `/mcp` request takes that lock.

Fix is mechanical and low-risk: the guarded data is a plain map, so recover with
`lock().unwrap_or_else(|e| e.into_inner())`, or return a typed error and let the
caller decide. Either way the failure becomes request-scoped.

## Then: panics that should be `Result`

`engine/src/planning/domain/result.rs:51` (`assert_eq!(hash.len(), 64)`) and
`engine/src/plan_validation/domain/loop_config.rs:111`
(`assert!(max_iterations >= 1)`) are *documented* panics, so they are
intentional — but they are validating constructors, and a validator that aborts
the process is the wrong shape for a value that can arrive from config or a plan.
Return `Result<Self, _>` and let the caller refuse.

## Then: fragile invariants

`engine/src/audit/application/chain.rs:78,79,83,106,120` encode an invariant with
`.expect("filtered on Some(sequence)")`. It holds today (the same slice is
filtered before use), but `verify_chain` is the **verification** path — the one
place that deliberately processes *foreign* envelopes — so a future drift turns
into a panic on hostile input rather than the intended `ChainBroken` error. Prefer
carrying the filtered `(producer, sequence)` pairs instead of re-`expect`ing.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | No `.lock().expect(..)` / `.lock().unwrap()` remains in `server`; a poisoned lock is recovered or mapped to a typed error, with a test that poisons it and asserts the request still returns an error response rather than aborting |
| 2 | The two validating constructors return `Result` (or the panic is justified in the baseline with a reachability argument) |
| 3 | `chain.rs` carries its invariant in the type instead of re-`expect`ing |
| 4 | `.pi/panic-baseline.tsv` shrinks accordingly; `validate-panic-sites.sh` passes with the updated baseline |
| 5 | The remaining `const`/`startup` entries each keep a written justification in the baseline |

## Non-goals

- Not a blanket "no `unwrap`" rule: `const` sites (compile-time schema literals,
  HMAC key length) are infallible by construction and are recorded, not rewritten.
- Not a re-audit of test code; `cfg(test)` is out of scope by definition.

## References

- `docs/panic-audit-2026-10-12.md` — method, per-site table, classification
- `.pi/panic-baseline.tsv` · `.pi/scripts/panic_audit.py` · `.pi/scripts/validate-panic-sites.sh`
- ADR-009 (error handling) · ADR-017 (fail-closed refusal) · #915 (MCP Streamable HTTP adapter)
