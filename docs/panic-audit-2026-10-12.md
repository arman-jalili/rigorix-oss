# Panic-surface audit — production Rust (2026-10-12)

## Why this exists

An unsolicited email (disposable-mail domain, no reproduction, no version)
claimed:

> `actions/src/action_entrypoint/application/context_builder_impl.rs:636` unwraps
> fallible boundary state during execution step. crashes the runner process.

**That specific claim is false.** Line 636 is
`let output = builder.build(input).await.unwrap();` inside the
`#[cfg(test)] mod tests` block that begins at line 496. Test-only code is not
compiled into the binary, so it cannot crash a runner. The report matches a
naive `grep '\.unwrap()'` that does not exclude `cfg(test)` — note also that the
file is in `actions/src/action_entrypoint/` but the report names a path shape
(`actions/src/action_entrypoint/application/...`) the repo does not have.

The **class** it gestures at is real and worth owning: a Rust panic aborts the
process, and a runner/host has no caller to catch it, so one reachable panic
takes down the whole execution instead of failing a single step.

## Method

`.pi/scripts/panic_audit.py`, run by `.pi/scripts/validate-panic-sites.sh`.

Naive greps are useless here: 2 961 raw `.unwrap()`/`.expect()` hits collapse to
**41 production sites** once you exclude

* paths under `tests/` and `target/`
* `#[cfg(test)]`-gated items (brace-matched)
* functions annotated `#[test]` / `#[tokio::test]` / `#[rstest]` / …
* out-of-line test-only modules (`#[cfg(test)] pub mod foo_tests;`)

Getting this right required masking comments and every flavour of string/char
literal before brace matching: the first attempt reported 65 sites because a
test module in `file_patch_tool.rs` contains TypeScript fixtures whose braces
closed the block early, and a later bug lost string-continuation newlines so
line numbers drifted in 35 files. Both are fixed and the masker is verified to
preserve line counts exactly.

## Result

| severity | sites |
|---|---|
| **high** | 8 |
| medium | 10 |
| low | 23 |

## The actionable class: poisoned-lock panics (`server`)

`.lock().expect(...)` panics when a mutex is **poisoned** — i.e. when any thread
panicked while holding it. The failure mode is not graceful degradation: the
first panic poisons the lock, and then *every* subsequent request panics on the
same line, so the process turns a one-off fault into a permanent one (an
availability bug, and a cheap DoS if any panic is attacker-influenced).

| site | kind | severity | reason |
|---|---|---|---|
| `server/src/events.rs:124` | `expect` | **high** | Mutex lock().expect — panics if the lock is POISONED, i.e. any panic while held makes every later call panic too (cascading failure, not a degraded path) |
| `server/src/events.rs:147` | `expect` | **high** | Mutex lock().expect — panics if the lock is POISONED, i.e. any panic while held makes every later call panic too (cascading failure, not a degraded path) |
| `server/src/mcp_http.rs:106` | `expect` | **high** | Mutex lock().expect — poisoned-lock panic in the MCP Streamable HTTP session store; one panic while held turns every later /mcp request into a panic |
| `server/src/mcp_http.rs:111` | `expect` | **high** | Mutex lock().expect — poisoned-lock panic in the MCP Streamable HTTP session store; one panic while held turns every later /mcp request into a panic |
| `server/src/mcp_http.rs:124` | `expect` | **high** | Mutex lock().expect — poisoned-lock panic in the MCP Streamable HTTP session store; one panic while held turns every later /mcp request into a panic |
| `server/src/mcp_http.rs:133` | `expect` | **high** | Mutex lock().expect — poisoned-lock panic in the MCP Streamable HTTP session store; one panic while held turns every later /mcp request into a panic |
| `server/src/mcp_http.rs:140` | `expect` | **high** | Mutex lock().expect — poisoned-lock panic in the MCP Streamable HTTP session store; one panic while held turns every later /mcp request into a panic |
| `server/src/mcp_http.rs:94` | `expect` | **high** | Mutex lock().expect — poisoned-lock panic in the MCP Streamable HTTP session store; one panic while held turns every later /mcp request into a panic |

`server/src/mcp_http.rs` is the optional MCP Streamable HTTP adapter (#915) — the
newest code in this set, and the most exposed (every `/mcp` request takes that
lock). The fix is mechanical: recover from poisoning
(`lock().unwrap_or_else(|e| e.into_inner())`, since the data is a plain map) or
propagate a typed error.

## Panics that should be `Result` (`api`)

Both are *documented* panics (`# Panics`), so they are intentional — but they are
validating constructors, and a validator that aborts the process instead of
returning a typed refusal is the wrong shape for values that can come from
config or a plan.

| site | kind | severity | reason |
|---|---|---|---|
| `engine/src/plan_validation/domain/loop_config.rs:111` | `assert!` | **medium** | documented panic in a constructor (assert! max_iterations >= 1) |
| `engine/src/planning/domain/result.rs:51` | `assert!` | **medium** | documented panic in a constructor (assert_eq! len==64); a Result would make a bad hash a typed refusal instead of a crash |

## Fail-fast at composition (`startup`)

Intentional: logged, then abort, before any work is served. Recorded so the
decision is explicit rather than accidental.

| site | kind | severity | reason |
|---|---|---|---|
| `engine/src/execution_engine/application/factory.rs:222` | `expect` | **medium** | fail-fast at composition: approval store must open |
| `mcp/src/engine_wiring.rs:31` | `panic!` | **medium** | fail-fast at composition: without an engine the host cannot serve; logs then panics |
| `mcp/src/host/mod.rs:1922` | `expect` | **medium** | app_state() before init_app_state() — programmer error; panics instead of a typed error |

## Fragile internal invariants (`invariant`)

Safe today, but they encode an invariant with a `.expect(...)` instead of a type,
so a refactor turns them into panics. `chain.rs` deserves the most care: it is
the **verification** path, i.e. the one place that deliberately processes foreign
envelopes, so a drift there becomes a panic on hostile input rather than a
`ChainBroken` error.

| site | kind | severity | reason |
|---|---|---|---|
| `engine/src/audit/application/chain.rs:106` | `expect` | **medium** | internal invariant (filtered on Some(sequence)); holds today, but this is the path that verifies FOREIGN envelopes, so a drift becomes a panic on hostile input |
| `engine/src/audit/application/chain.rs:120` | `expect` | **medium** | internal invariant (filtered on Some(sequence)); holds today, but this is the path that verifies FOREIGN envelopes, so a drift becomes a panic on hostile input |
| `engine/src/audit/application/chain.rs:78` | `expect` | **medium** | internal invariant (filtered on Some(sequence)); holds today, but this is the path that verifies FOREIGN envelopes, so a drift becomes a panic on hostile input |
| `engine/src/audit/application/chain.rs:79` | `expect` | **medium** | internal invariant (filtered on Some(sequence)); holds today, but this is the path that verifies FOREIGN envelopes, so a drift becomes a panic on hostile input |
| `engine/src/audit/application/chain.rs:83` | `expect` | **medium** | internal invariant (filtered on Some(sequence)); holds today, but this is the path that verifies FOREIGN envelopes, so a drift becomes a panic on hostile input |
| `engine/src/cancellation/application/cancellation_service_impl.rs:128` | `borrow()` | **low** | tokio watch::Receiver::borrow() — panics only on a same-task conflicting borrow |
| `engine/src/cancellation/application/cancellation_service_impl.rs:161` | `borrow()` | **low** | tokio watch::Receiver::borrow() — panics only on a same-task conflicting borrow |
| `engine/src/cancellation/application/cancellation_service_impl.rs:227` | `borrow()` | **low** | tokio watch::Receiver::borrow() — panics only on a same-task conflicting borrow |
| `engine/src/cancellation/application/cancellation_service_impl.rs:269` | `borrow()` | **low** | tokio watch::Receiver::borrow() — panics only on a same-task conflicting borrow |
| `engine/src/cancellation/application/cancellation_service_impl.rs:275` | `borrow()` | **low** | tokio watch::Receiver::borrow() — panics only on a same-task conflicting borrow |

## Infallible in practice (`const`)

`serde_json::from_str` on compile-time schema constants, a compile-time regex,
and `Hmac::new_from_slice` (which accepts any key length). Low risk; they would
only fire if an edit broke a literal, and the messages claim test enforcement.

| site | kind | severity | reason |
|---|---|---|---|
| `actions/src/action_output/application/output_variable_impl.rs:50` | `expect` | **low** | compile-time regex, covered by validate_name tests |
| `engine/src/approval/domain/hash.rs:44` | `expect` | **low** | infallible in practice (serde_json on an owned struct / HMAC accepts any key length) |
| `engine/src/approval/domain/intent.rs:108` | `expect` | **low** | infallible in practice (serde_json on an owned struct / HMAC accepts any key length) |
| `mcp/src/audit_tools/infrastructure/in_memory_audit_service.rs:393` | `expect` | **low** | HMAC accepts any key length, so new_from_slice cannot fail here |
| `mcp/src/audit_tools/interfaces/mcp/mod.rs:116` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/audit_tools/interfaces/mcp/mod.rs:125` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/audit_tools/interfaces/mcp/mod.rs:134` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/execution_tools/interfaces/mcp/mod.rs:188` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/execution_tools/interfaces/mcp/mod.rs:197` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/execution_tools/interfaces/mcp/mod.rs:206` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/execution_tools/interfaces/mcp/mod.rs:249` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/execution_tools/interfaces/mcp/mod.rs:258` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/execution_tools/interfaces/mcp/mod.rs:267` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/template_tools/interfaces/mcp/mod.rs:185` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/template_tools/interfaces/mcp/mod.rs:194` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/template_tools/interfaces/mcp/mod.rs:203` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/template_tools/interfaces/mcp/mod.rs:212` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |
| `mcp/src/usage_guide/interfaces/mcp/mod.rs:29` | `expect` | **low** | serde_json::from_str on a compile-time schema const (test-enforced) |

## How this is kept honest

`.pi/scripts/validate-panic-sites.sh` compares the live scan against
`.pi/panic-baseline.tsv` and fails when a panic site is **not** in the baseline
(new unfenced surface) or when a baseline entry no longer exists (stale). It runs
in the `audit` job of `ci.yml`, which the Final Gate requires — so it gates, rather
than printing into a `continue-on-error` loop.

```bash
.pi/scripts/validate-panic-sites.sh            # check (CI)
.pi/scripts/validate-panic-sites.sh --update   # re-record after a justified change
```

A panic site is therefore never "fixed" silently: it is either removed (return a
`Result`) or recorded here with the reason it is tolerated.
