# SDK + Server Mode — Session Freeze (2026-09-06)

**Status:** Freeze — records the SDK/server-mode strategy decided this session so future
work in rigorix-oss starts from it. No code in this commit; docs only.

## TL;DR

- New repo **rigorix-sdk** (github.com/arman-jalili/rigorix-sdk, private) is the
  contract-first home: `schemas/` (canonical contracts + native `rigorix.*` API catalog),
  per-language SDK dirs later, docs (strategy freeze + contract-sync design).
- **Strategy A (now):** schemas + envelope verifiers — make the HMAC-signed trail
  independently checkable anywhere. Not redundant with `rigorix-action` (GitHub-only).
- **Strategy B (next): server mode** — a new **`rigorix-server` workspace crate** here
  (single-tenant reference execution server: engine + approvals + identity + audit store
  behind one native `rigorix.*` JSON-RPC catalog over HTTP + SSE push). Enterprise side:
  execution-API **module in core** (module-in-core, not a sidecar), same catalog.
  stdio MCP unchanged.
- **SSE verdict:** legacy MCP SSE stays removed (GAP-A-10). What returns is HTTP
  JSON-RPC + a native push channel. MCP Streamable HTTP optional later (GUI agents).
- **Sequencing:** contract sync (milestone 1) → catalog design → server in both repos →
  SDK clients. Sync = three contracts (envelope v2, policy/config model, identity
  claims) + conformance tests + enterprise trust gate F-20260904-01 + log reconciliation
  — **sync-by-doing, not an open-ended catch-up phase.**

## Decisions frozen (cross-repo)

| # | Decision |
|---|---|
| D-1 | SDK = standalone rigorix-sdk repo, contract-first; rust client is protocol-only (no rigorix-engine dep) |
| D-2 | Strategy A first: schemas + envelope verifiers (trust multiplier) |
| D-3 | Server mode in both repos on ONE rigorix.* JSON-RPC catalog (OSS reference server + enterprise module-in-core) |
| D-4 | SSE: legacy not resurrected; native HTTP JSON-RPC + push channel; Streamable HTTP optional later |
| D-5 | Sequencing: contract sync → catalog → servers → SDKs |
| D-6 | Governance invariant: SDK surfaces expose governed flows only; embedded stays Rust-only via rigorix-engine |

Full rationale: rigorix-sdk `docs/strategy-freeze-2026-09-06.md`. Sync design:
rigorix-sdk `docs/contract-sync-2026-09-06.md`. Enterprise-side situation +
forward path: rigorix-enterprise `findings/contract-sync-server-plan-2026-09-06.md`
(decisions D-011..D-013 there).

## What this means for rigorix-oss (future work items, in order)

1. **Contract sync substrate:** drift CI — schema snapshot of MCP tool descriptors +
   envelope/plan payloads must match rigorix-sdk `schemas/`. Fixtures (real envelopes
   from conference-demo `.rigorix/audit`) become the conformance corpus.
2. **Catalog design:** ✅ DONE (2026-09-19) — rigorix-sdk froze the `rigorix.*` catalog in
   `schemas/api/catalog.json` (+ `catalog.schema.json`, `errors.json`) under
   `docs/adr/ADR-0001-native-api-catalog.md`. It fixes JSON-RPC 2.0 over `POST /rpc`,
   dot-separated lowerCamelCase names mapped 1:1 to the MCP tools, three auth levels, and
   the identity invariant (caller-supplied identity is display-only; authorization is the
   attested session). rigorix-server below is now the next step.
3. **`rigorix-server` crate:** reference execution server. Reuses engine orchestrator,
   event bus, approval binding, identity/keychain, audit writer, enterprise-proxy
   JsonRpc types. New dependency: an HTTP stack (axum) — the first since GAP-A-10.
4. **SDK clients** (rigorix-sdk rust/ python/ typescript/ ...): thin typed clients +
   verifiers against the frozen catalog.

Note for future sessions: this freeze supersedes nothing in the code — it is a pointer
for where the SDK/server work is planned. Start by reading rigorix-sdk docs before
opening any SDK/server issue.

---

## Status update — 2026-09-21

- **Catalog freeze: done** (item 2). rigorix-sdk `schemas/api/catalog.json` +
  ADR-0001 (19 `rigorix.*` methods). OSS CI now enforces the MCP↔catalog
  adapter table (`catalog_drift_tests`).
- **ADR-014 effect-identity: released 1.5.0** — value-identity predicate
  (`equals_step`) + effect-keyed history + envelope `effect_key`; conformance
  fixture `envelope-effect-key-signed.json` proves byte-exact HMAC parity.
- **Remaining workstreams (C: servers/clients, D: cleanups)** are captured in
  [`docs/next-workstreams-2026-09-21.md`](./next-workstreams-2026-09-21.md).
  Next: C1 (SSE spec) → C2 `rigorix-server` + C3 enterprise Execution API.

## Status update — 2026-09-25 (delivered)

The freeze is essentially fully delivered:

- **Strategy A (D-2):** schemas + **Rust/Python/TypeScript verifiers**, published as
  `rigorix-schemas` + `rigorix-verifier` on crates.io; consumed by OSS + enterprise.
- **D-3 (one catalog):** `rigorix.*` catalog frozen and now **closed/subset-modeled**
  (23 methods); `verify_catalog_subset` guards OSS, SDK and enterprise; OSS
  `rigorix-server` (#888/#900) + enterprise Execution API (#206/#208) both serve it.
- **D-4 (SSE):** ADR-0002 spec + live `GET /events`.
- **D-5 (sequencing):** contract sync → catalog → **both servers** → **clients**
  (Rust #26 + TypeScript #27) — complete.
- **Beyond the freeze:** ADR-015 (1.6.0 + enterprise), **ADR-016** audit integrity
  Phases **A** (#898, 1.7.0) / **B** (enterprise #207) / **C** (#899) shipped, with
  Phase D deferred. GAP-A-29 + GAP-A-30 resolved.

Open items are the demo/operator cleanups (**D1** key rotation, **D3** public
same-effect scene, **D4** payouts-demo visibility) — see the workstreams doc.
