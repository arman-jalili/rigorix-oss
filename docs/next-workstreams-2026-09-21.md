# Next workstreams (2026-09-21)

Backlog captured after the SDK/server **catalog freeze** (rigorix-sdk ADR-0001)
and the **ADR-014 effect-identity** release (**1.5.0**). This file exists so the
larger workstreams are not lost between sessions. Source of truth:
`docs/sdk-server-strategy-freeze-2026-09-06.md` (decisions D-1…D-6).

Status at capture:
- Contract-sync milestone 1 — **done** (envelope/claims/policy schemas + real
  signed fixtures + byte-exact HMAC parity).
- Catalog freeze — **done** (rigorix-sdk `schemas/api/catalog.json` + ADR-0001,
  19 `rigorix.*` methods).
- ADR-014 effect-identity — **done**, released **1.5.0**
  (`equals_step` value-identity predicate + `effect_key` history + envelope).

---

## C. The committed roadmap — catalog → servers → clients (D-5 / D-012)

### C1. SSE push semantics
Specify `GET /events` reconnect + `Last-Event-ID` semantics and the exact event
payloads (`approval_required`, `run_progress`, `policy_changed`) before the
server is built. ADR-0001 fixed the event names; the resume protocol is open.
*Owner: rigorix-sdk (spec) → both servers.*

### C2. `rigorix-server` (OSS reference execution host)
New crate in rigorix-oss: **axum** HTTP server exposing the frozen catalog —
`POST /rpc` (JSON-RPC 2.0, batch) + `GET /events` (SSE).
- Reuses: engine orchestrator, event bus, approval binding, identity/keychain,
  audit writer, enterprise-proxy JSON-RPC types.
- First HTTP dependency since GAP-A-10 (which removed the legacy MCP SSE path).
- Acceptance: the 19 catalog methods behave identically to their MCP-tool
  counterparts (same DTOs, same error taxonomy — ADR-0001 D6).

### C3. Enterprise Execution API (Phase 5)
Implement `core/.pi/architecture/modules/execution-api.md` (already teller-grade)
via `/architect`. Module-in-core, team-scoped `rigorix.*` over the same frozen
catalog + fixtures.
- Services: `JsonRpcGateway` (edge auth, team scope), `CatalogRouter`,
  `PlanValidationService` (pre-ingestion gate F-02), `ApprovalService`
  (claims-bound), `ClaimVerificationService` (JWKS), `PolicyBundleService`,
  `RunRegistryService` (envelope v2), `AuditReadService` (v2 blocks).
- Migration: `018_execution_api_initial.sql`.
- Enterprise deltas already recorded: `rigorix.policy.bundle` is **admin** level
  (enterprise is source of truth); `rigorix.auth.*` scopes claims to
  teams/roles via JWKS (landed in PR #201).
- First live target of the OSS `rigorix_enterprise_*` proxy seam.

### C4. SDK clients
`rigorix-sdk/{rust,python,typescript,java,go}` — thin typed clients + the
protocol-only verifier, generated/hand-written against the frozen catalog.
Governance invariant (D-6): clients expose governed flows only; embedded
execution stays Rust-only via `rigorix-engine`.

### C5. Enterprise conformance AC
Exported policy bundle → OSS engine enforces **identically** to local config
(conformance fixture round-trip). Listed in the Phase-5 AC table.

---

## D. Cleanups / advisories

### D1. Rotate the `rgx_live_sk_…` backend key
The migration-demo key was exposed earlier (before the public-repo
remediation). It is **not** in current git history, but rotating it on
app.rigorix.eu is still the right call. `migration-demo/rigorix.toml` holds it
as an **uncommitted** local edit — keep it that way.

### D2. `payouts-demo` reset caveat
`./reset-demo.sh` runs `git checkout -- .`, which discards **all** uncommitted
edits (that is its base-reset job). Consider narrowing the revert to the demo's
own files, or document the caveat more loudly. (Already noted in the run output
and README.)

### D3. Optional product narrative — same-effect scene in the public demo
Add a "same effect, different name" scene to the **public** conference-demo
(mirrors the 1.5.0 ADR-014 capability) so the public repo tells the full story
alongside the private `payouts-demo`.

### D4. `payouts-demo` visibility
Currently private (`arman-jalili/payouts-demo`). Decide whether to publish it
with the article (as was done for conference-demo).

---

## Sequencing

1. **C1** (spec) — small, unblocks C2/C3.
2. **C2 `rigorix-server`** and **C3 enterprise Execution API** — in parallel;
   they share one catalog + fixtures.
3. **C4 clients** once C2/C3 have a stable reference implementation.
4. **D1–D4** opportunistically.

Related: `docs/sdk-server-strategy-freeze-2026-09-06.md` (decisions),
`docs/contract-sync-2026-09-06.md` (schema deltas),
rigorix-sdk `docs/adr/ADR-0001-native-api-catalog.md`.
