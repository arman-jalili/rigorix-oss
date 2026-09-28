# Issue Implementation Groups

**Generated:** 2026-09-26
**Total Issues:** 6 (fresh fetch; #899/#907 merged + closed)
**Source:** `.claude/plans/issues-fetched.json`

---

## Grouping Strategy

Grouped by component / priority / dependency. The batch is dominated by a
**tracking umbrella** (#914) with two children, one independent security fix,
and two blocked/deferred items.

| # | Issue | Priority | Component | Readiness |
|---|-------|----------|-----------|-----------|
| 1 | **#913** OSS-SERVER-SSE-AUTH | Medium | `server/src/events.rs` | ✅ **READY** — dep #900 merged |
| 2 | **#917** OSS-REFACTOR-TESTS | Low | engine test modules | ✅ **READY** — Tier 0, pure move |
| 3 | **#916** OSS-REFACTOR-ORCHESTRATOR | Medium | `engine/src/orchestrator` | ⏳ after #917 |
| 4 | **#914** OSS-REFACTOR-PROGRAM | Medium | umbrella (tracking) | 📋 tracking — no code PR of its own |
| 5 | **#912** OSS-ANCHOR-WIRE-E2E | High | engine + server + CI | ⛔ **BLOCKED** — enterprise `#215` (ENT-ANCHOR-ENDPOINT) still OPEN |
| 6 | **#915** OSS-MCP-STREAMABLE-HTTP | Low | `server/` transport | ⛔ **DEFERRED** — trigger-gated ("do not build speculatively") |

## Batch Order (implement top → bottom)

| # | Branch | Issues | Notes |
|---|--------|--------|-------|
| 1 | `issue/913` | #913 | Single. Apply the `POST /rpc` session gate to `GET /events` (fail closed when an IdP is configured; preserve no-IdP local mode). |
| 2 | `issue/917` | #917 | Single. Externalize the 3 in-module test files (>1000 LOC) as `tests/` submodules — pure move, test count invariant. |
| 3 | `issue/916` | #916 | Single. Split `orchestrator_impl.rs` (4166 LOC) — depends on #917 landing first. |

### Why single-issue branches (not batches)
#913 is a security fix in `server/`; #917/#916 are mechanical engine refactors
with strict "no behaviour change" invariants. Different components + different
review surfaces → `issue/{number}` each.

---

## Dependency Graph (must-land-before)

```
#900 / #888 (GET /events + bridge, MERGED) ──► #913 SSE auth  ✅ ready
                                                    
#914 umbrella ──► #917 (Tier 0, pure move) ──► #916 (Tier 1 orchestrator)
                                                    
SDK #34 (0.2.0, CLOSED) ─┐
                         ├─► #912 anchor wire E2E  ⛔ needs enterprise #215 OPEN
ENT-ANCHOR-ENDPOINT #215 ┘

ADR-0001 D4 + #888 (MERGED) ──► #915 Streamable HTTP  ⛔ trigger-gated
```

## Deferred / Blocked rationale

- **#912** — its own prerequisite table says `ENT-ANCHOR-ENDPOINT (#215)` **must
  land first** to reconcile the OSS `HttpAnchorClient` (`GET /v1/history`, bare
  `HistorySlice`) with the enterprise server (`POST /rpc`, JSON-RPC envelope).
  `#215` is **OPEN**. SDK #34 (0.2.0, knows `anchor_head`) is CLOSED. Do not
  start until #215 merges.
- **#915** — explicitly trigger-gated: build only on a demonstrated GUI-agent
  need. No trigger present → leave open, do not implement.
- **#914** — umbrella: its AC is satisfied by the child PRs (#916/#917) +
  inventory accuracy. No standalone code PR; update the inventory if LOC drifts.

---

## Per-Batch Validation (standard)

```bash
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
bash .pi/scripts/validate-ci.sh
bash .pi/scripts/validate-tests.sh
bash .pi/scripts/validate-security.sh
bash .pi/scripts/validate-operations.sh
```

#917 additionally proves the **`#[test]` count is invariant** before/after the
move (`cargo test -p rigorix-engine -- --list | wc -l`).

## Status Tracking

| Issue | Branch | Status |
|-------|--------|--------|
| #913 | `issue/913` | ready — planned |
| #917 | `issue/917` | ready — planned (Tier 0) |
| #916 | `issue/916` | planned (after #917) |
| #914 | — | tracking umbrella |
| #912 | — | blocked by enterprise #215 |
| #915 | — | deferred (trigger-gated) |
