# Issue Implementation Groups

**Generated:** 2026-09-22
**Total Issues:** 2
**Source:** `.claude/plans/issues-fetched.json` (open issues #888, #889)
**Epic:** server mode — C: catalog → servers → clients (D-3 / D-4 / D-5 / D-012, §C5)

---

## Grouping Strategy

Issues are grouped by:
1. **Component** — same module/files affected (2–5 issues per batch)
2. **Priority** — Critical > High > Medium > Low
3. **Dependency** — blocking relationships

Every issue in this fetch is a distinct component (engine `sequence_policy`
vs. a brand-new `server/` crate), so no component batch is possible. They are
ordered by dependency: **#889 (bundle ingestion) lands before #888**, because
#888 is the composition root that wires the bundle source into the server.

---

## Batch Order (implement top → bottom)

| # | Branch | Issues | Tier | Component | Notes |
|---|--------|--------|------|-----------|-------|
| 1 | `issue/889` | #889 OSS-C5 | H / moderate | `engine/sequence_policy` | Bundle JSON → `SequencePolicyConfig`, shared validation, precedence vs local TOML, conformance round-trip. Internal dep #886 (ADR-015) done; cross-repo export #204/#205 done. |
| 2 | `issue/888` | #888 OSS-C2 | H / complex | new `server/` crate | axum JSON-RPC 2.0 (`POST /rpc`) + SSE (`GET /events`) host over the existing engine/MCP facade; catalog-parity + error taxonomy + auth levels + identity invariant. Consumes #889's bundle wiring in the server composition. |

### Why not one batch?
`#888` *creates* a workspace crate and touches root `Cargo.toml`, CI, and the
workspace dependency graph. Bundling it with a `sequence_policy` change would
make one PR span two unrelated review surfaces and violate the "same
module/files" rule. Single-issue branches (`issue/{number}`) are the correct
shape.

---

## Dependency Graph (must-land-before)

```
#886 (ADR-015, done) ──► #889 bundle ingestion + conformance ──┐
                                                                 │ consumed by
#204/#205 export (done) ────────────────────────────────────────┤
                                                                 ▼
                        #888 rigorix-server (server composition)
```

- #889 is independently testable and does not require #888.
- #888 wires `SequencePolicySetup` (now bundle-aware) into the server
  composition — the same `from_env` path MCP uses.

## Critical Path

`#889 → #888`. Both are "high"; neither blocks the other's *implementation*
except for the final server composition wiring in #888.

---

## Per-Batch Validation (standard)

Each batch before MR:

```bash
cargo build
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --check
bash .pi/scripts/validate-tests.sh
bash .pi/scripts/validate-architecture.sh
bash .pi/scripts/validate-canonical.sh
```

`#888` additionally requires the new `server/` crate to be included in the
workspace members so `cargo build`/`clippy`/`test --all` cover it, and the
local-ci crate list (`engine mcp cli actions server`) updated.

## Status Tracking

| Issue | Branch | Status |
|-------|--------|--------|
| #889 | `issue/889` | planned |
| #888 | `issue/888` | blocked-by #889 (composition wiring) |
