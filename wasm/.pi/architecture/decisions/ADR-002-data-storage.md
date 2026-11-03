# ADR-002: Data Storage Strategy — No Persistence; the Artifact Set Is the Only Durable State

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

The scaffolded template asks for "Postgres + SQLx per context". That presupposes a
server. This crate is a browser-targeted WebAssembly module: `wasm32-unknown-unknown`
has **no filesystem, no sockets and no threads**, so neither a Postgres client nor
SQLx can exist in the shipped binary. The exploration's non-goals are explicit:
"providing a server, an API, or persistence".

What actually needs to survive a page reload is the **record**: the signed envelope
set plus the evidence references that let an auditor re-verify it without the
producer. That is not a row set — it is a self-describing byte sequence with a
format version, and it is already the engine's artifact format (engine ADR-016).

## Decision

**No datastore. No storage-backed repository. No migrations.**

1. `ReplaySession` lives in memory for one host session; the aggregate is created,
   sealed and dropped.
2. The durable unit is `ArtifactSet` — a version-stamped byte sequence produced by
   `ExportArtifactSet`, immutable once sealed. Persistence is the **host's** concern:
   localStorage, IndexedDB, a download, the partner's own store. The crate never
   learns where it went.
3. **No session-store port.** Verification's input is the `ArtifactSet`, not a session:
   the artifact is self-describing and version-stamped, so anything that needs to
   re-check a run takes the exported bytes. A store port would exist only to persist a
   live aggregate and would invite a host to treat a stored session as evidence — the
   two things this ADR exists to prevent. A native test harness that wants durability
   uses a `cfg(test)` file helper, not a domain trait.
4. Nothing is nullable-by-omission: an absent field is a parse refusal, never a
   default (FR-001: "rejecting any input that does not parse under the engine's own
   types with a typed refusal rather than a default").
5. Zero migrations. If a future context needs storage it gets its own ADR and its own
   crate; this one will not grow a database.

## Consequences

### Positive
- No database in the browser bundle, so the size budget (NFR-002) and the wasm
  dependency ban list (no `tokio`, no `reqwest`, no SQLx) stay satisfiable.
- The audit trail is portable by construction: an artifact set is verifiable by a
  party who never had our process, memory or disk.
- No schema migrations to lockstep with engine versions — only the
  **artifact-format version**, stamped into every export (FR-010, FR-014).

### Negative
- Listing past sessions is the host's job. Accepted: the demo keeps one artifact in
  memory and offers a download.
- In-memory sessions are lost on reload; replay is driven by a fixture vector, not by
  resuming a live session.

### Neutral
- A native integration harness may use `tempfile`-backed adapters for conformance
  vectors; test-only, never reachable from the wasm target.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| IndexedDB via `web-sys` | Real browser persistence | Large wasm-bindgen surface inside `domain`; storage becomes a side effect of a decision | Storage is the host's concern; also breaks "no I/O on the run path" |
| SQLite compiled to wasm | Familiar SQL | Hundreds of KB of bundle; nothing queries it | No query workload exists |
| Postgres + SQLx (scaffold default) | Team familiarity | Cannot link on `wasm32-unknown-unknown`; requires a server we must not build | Environment makes it impossible |
| Serialize the aggregate as JSON and reload it | Simple resume | A resumed session invites re-running an attempt that must never be re-scored | Violates "one run per session, sealed at completion" |

## Implementation

**Affected Modules:** `.pi/architecture/modules/portable-session.md`

**Crate paths:**
- `wasm/src/domain/ports/session_store.rs` — trait owned by the domain
- `wasm/src/infrastructure/adapters/memory_session_store.rs` — shipped adapter
- `wasm/src/domain/artifact/artifact_set.rs` — the durable value object

**Canonical References:** `// @canonical .pi/architecture/decisions/ADR-002-data-storage.md`

## Validation

- **architecture-validator**: no storage crate in `Cargo.toml` for the wasm target.
- **ci-mr-validator**: `cargo tree --target wasm32-unknown-unknown` has no `sqlx`,
  `rusqlite`, or `web-sys` storage feature.
- **security-validator**: exported bytes scanned for host paths (NFR-003).

## References

- Related ADRs: ADR-001, ADR-004, ADR-010
- Engine ADR-016 (signed audit envelope) — the format we consume, not re-invent

---

*Decision date: 2026-11-03*
*Decision makers: architecture-coordinator, domain exploration session 3baf6e5c*