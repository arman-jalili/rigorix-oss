# Issue groups — 2026-10-13

Fetched: 4 open issues (`.claude/plans/issues-fetched.json`).

| # | Title | Epic | Component | Prio | Deps |
|---|-------|------|-----------|------|------|
| #1001 | Authority-fact projection (third-party-verifiable export) | consequence gating | Audit / Evidence | medium | ADR-016, ADR-017, SDK verifiers |
| #1006 | Projection surfaces: CLI command + MCP tool | consequence gating | Audit / Evidence | medium | #1001 (landed), SDK catalog |
| #1004 | Mirrored `.pi/scripts` validators have no single source of truth | tooling / ci integrity | CI / Validators | medium | — |
| #1005 | Flaky test: `test_schema_cache_is_stale` races the clock | tooling / test reliability | MCP / Audit Tools | low | — |

## Repo state at fetch time

Local `main` = `c6ac481f`. `#1001`'s producer core landed in `608ec3d5` (PR #1003);
its **re-derivation verifier** landed in `d9e4b7c3` (`AuthorityFactBundle`,
`verify_authority_fact_bundle`). So `#1001` is *mostly* implemented in-repo — the
unfinished parts are the surfaces and the published-verifier offline check, and
`#1006` now owns the surfaces explicitly.

---

## Batch A — consequence-gating surfaces (`feat/consequence-gating-surfaces-1001-1006`)

**Group type:** related batch (dependency-linked). `#1006` depends on `#1001`; `#1006`
is the surface half of `#1001`.

| Issue | Remaining work |
|-------|----------------|
| #1001 | AC1/AC7: verify the projection **offline with the published verifiers** (rust/python/typescript), and show a tampered projection is rejected. Everything else (domain, bundle, re-derivation verifier, redaction, claim ceiling) already landed. |
| #1006 | CLI: emit a bundle + a `verify` subcommand (non-zero on failure); assert no path/username leak in CLI output. **MCP tool: a DECISION + possibly deferral** — it is a frozen-catalog change across three repos (SDK catalog → both servers → conformance), and the SDK method does not exist yet. |

**Order:** #1001 offline verification → #1006 CLI → #1006 MCP decision (likely: decide
*yes, but gated*, and file/keep the SDK dependency; implement only after the SDK
catalog method exists).

**Sequencing guard:** the MCP half **must not** be started before the SDK catalog
method lands — parity tests will fail otherwise, by design. The CLI half is our own
surface and blocks nothing.

**Key files:** `cli/src/cli_boundary/cli.rs`, `…/orchestrator.rs` (and `dispatch.rs`),
new `cli/.../tests`; `engine/src/audit/{domain/authority_fact.rs,application/projection.rs}`;
`docs/spec/consequence-gating-v1.md §4.3`; (only if the tool is approved)
`mcp/src/audit_tools/interfaces/mcp/mod.rs`, `mcp/src/host/mod.rs`,
`server/tests/catalog_parity.rs`.

**Validators:** ci, tests, security, architecture, canonical.

---

## Batch B — tooling & test integrity (`fix/validator-drift-and-flaky-test-1004-1005`)

**Group type:** component batch (both are `tooling / test-reliability`, no cross-deps).

| Issue | Work |
|-------|------|
| #1005 | Make `test_schema_cache_is_stale` deterministic. Root cause found: `SchemaCache::is_stale` uses `Utc::now() - t > ttl` (**strict `>`**), and `update()` stamps `last_fetched = Utc::now()`; with `ttl = 0` the test needs `elapsed > 0`, so it races the clock (and implicitly pins `>` vs `>=`). Fix: inject the clock (a `now`/`last_fetched` parameter or a test clock) and exercise the boundary without wall-time. |
| #1004 | One source of truth for the mirrored rust validators + a **gating drift guard**, and fix the `language: C.UTF-8` message (35 dispatcher files). Measured drift (2026-10-13, `shasum`): `validate-tests.sh` **4** variants, `validate-ci.sh` **3**, `validate-integration.sh` **3**, `validate-architecture.sh` **2**, `validate-canonical.sh` **2**; `operations`/`security` **1**. Dispatcher sets: ROOT 13, engine 10, mcp/cli/actions 9, server 0. |

**Warning (from the issue, do not repeat):** do **not** blanket-normalise. Cosmetic
differences in inert `languages/{go,java,python,typescript}/` files and the four
security guards are *not* defects; "hash differs" is a question, not a verdict.
Record intentional per-tree differences before adding the guard.

**Key files:** `mcp/src/enterprise_proxy/domain/entity.rs`,
`mcp/tests/enterprise_proxy_contract_freeze_test.rs`;
`.pi/scripts/languages/rust/validate-*.sh` (+ per-tree copies),
`.pi/scripts/validate-*.sh` (+ per-tree copies), new
`.pi/scripts/validate-script-drift.sh`, `.pi/guards.tsv`.

**Validators:** ci, tests, architecture, (guards).

---

## Recommended order

**Batch B first** — self-contained, low blast radius, restores CI/test reliability,
and one half (#1005) is a defect I already reproduced. Then **Batch A**, whose MCP
half is gated on a cross-repo SDK catalog change and should be sequenced SDK-first.

`#1001` closure: close it once the published-verifier offline check lands in Batch A
(the surfaces it also asked for are now tracked by `#1006`).
