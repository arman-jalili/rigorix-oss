# Architecture Change Log

<!--
Canonical Reference: .pi/architecture/CHANGELOG.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT GENERATED FILES - Modify this source only
-->

This document tracks all architecture changes requiring implementation updates.

---

## 2026-11-03 — Review pass: identity, failure semantics, verification ownership

Decisions added after reviewing the first generation of this documentation set
against the engine and SDK as they actually are.

### Added
- **ADR-011 — Crate identity and release-train membership.** Names the crate
  (`rigorix-gate-wasm`), the npm package (`rigorix-gate`), lockstep versioning, inherited
  licence (`MIT OR Apache-2.0`) and the six registration points, including the
  `release.yml` tag loop that `rigorix-actions` fell out of for seven weeks.
- **ADR-012 — Failure semantics on wasm32.** NFR-004 promised that a trap yields a
  refusal; that is impossible without unwinding. Failures are now tiered: typed refusals
  for logical faults, panic-free gate path enforced by baseline and deny-lints, traps
  surfaced to the host as fatal and never as permission.
- **ADR-013 — Who verifies.** Verification moves out of the crate entirely: it produces
  and exports, the host checks with the independent SDK. `VerifyExportedArtifacts` and
  `VerifierBridge` removed; the wasm surface drops from five functions to four.

### Changed
- **ADR-001** — crate named correctly; npm package named; identity delegated to ADR-011.
- **ADR-002** — the session-store port is withdrawn; the artifact set is the only
  reconstitution input.
- **ADR-009** — prerequisite ordering added: a ten-day feasibility spike precedes Phase 0,
  and the engine seam is a parallel track from day 11 rather than a phase after the
  crate's own last phase. Criteria 19–22 read *blocked* until FR-011 lands.
- **Module `portable-session`** — prerequisites section; implementation sequence
  re-ordered (spike → registration → ports → …); criteria 12/13/14 rewritten for
  host-side verification; criteria 25–27 added (cross-instance determinism, failure
  tiers, release-train registration).
- **Roadmap** — Phase -1 (feasibility spike) added; day ranges renumbered; Phase 5 marked
  parallel; risk table extended.
- **Exploration** — NFR-004 tiered, FR-006 restated as export-not-verify, FR-011 carries
  the spike ordering, `VerifyExportedArtifacts` removed from the service list, the
  verification invariant restated host-side, and the four decisions recorded.

---

## Change Log Format

Each entry follows this structure:

```markdown
## [YYYY-MM-DD] - [Change Title]

### Changed
- Module: [module-name]
  - [Component]: [what changed]
  - [Component]: [what changed]

### Impact Analysis
- Files affected:
  - src/[path1]
  - src/[path2]
- Canonical refs to update:
  - .pi/architecture/modules/[module].md#[section]
- Validators required:
  - [validator-name]

### Migration Steps
1. [Step 1]
2. [Step 2]
3. [Step 3]

### Status
- [ ] Architecture doc updated
- [ ] CHANGELOG entry added
- [ ] Implementation updated
- [ ] Canonical refs updated
- [ ] Validators run
```

---

## Entries

## [2026-11-03] - Architecture Package: ADRs, Diagrams, Module Enrichment, Roadmap

### Changed
- Module: `portable-session`
  - `ModelingDepth`: kept `tactical (core domain)`; added `Modeling Depth Note (ADR-008)` explaining why delivery concerns stay thin
  - `ReplaySession`: aggregate root stub with behaviour methods (`open` / `attempt` / `seal` / `take_events`), no setters, eight exploration invariants carried verbatim with `illegalStatePrevented`
  - `GateAttempt`: entity stub (one attempt per session, recorded once)
  - `ValueObjects`: 12 VOs documented with entity-vs-VO justification (`identityPersists` / `immutable` / `equalByState`)
  - `Ports`: six engine-declared ports (Clock, IdSource, Randomness, KeySource, ProviderTransport, BusinessSink) + domain-owned `ReplaySessionRepository`
  - `DomainEvents`: 7 events with payload + ADR-008 triage; 6 evidence, 1 telemetry (`RandomDrawConsumed` never signed)
  - `ApplicationServices`: the 5 catalog services reproduced verbatim (`OpenReplaySession`, `AttemptRun`, `ExportArtifactSet`, `VerifyExportedArtifacts`, `RunConformanceVector`) plus non-catalog helpers below them
  - `APIEndpoints`: 5 wasm-bindgen exports documented (no HTTP surface exists)
  - `Diagrams`: data flow, entity relationship, aggregate state, key use-case sequence
  - `AcceptanceCriteria`: 24 testable criteria mapped to FR/NFR and verify-in layer
  - `Components`: per-layer inventory with `status:` / `depends:` markers
  - `Dependencies`, `ImplementationSequence` (13 steps), `Patterns & OOP Mapping`, `Security Considerations`, `Testing Requirements`, `Error Handling`
- Diagrams (system level)
  - `diagrams/system-context.md`: rewritten for one bounded context — system context, trust boundary table, context-map flow, deployment view
  - `diagrams/event-flow.md`: created — evidence vs telemetry classification flow, end-to-end sequence, event catalogue by destination
- Decisions
  - `ADR-001`: rewritten from the scaffolded modular-monolith stub to the real topology (one core context, four layers, additive engine feature, npm pinned to engine version)
  - `ADR-002` … `ADR-010`: created (storage = none; events = in-process evidence/telemetry; host contract = wasm-bindgen + JSON DTOs; no principal, key ownership; no metering; execute-the-gate-not-judge-compliance; proportional depth; conformance harness; artifact format)
- Roadmap
  - `implementation-roadmap.md`: generated — 7 phases (Days 1–52), dependency graph, effort estimates with risks, milestones M1–M7, explicit no-migrations statement

### Impact Analysis
- Files affected (planned crate, none created yet):
  - `wasm/src/domain/{ports,session,artifact,events,services,errors}/`
  - `wasm/src/application/{open_replay_session,attempt_run,export_artifact_set,verify_exported_artifacts,run_conformance_vector}.rs`
  - `wasm/src/infrastructure/{adapters,verifier_bridge,content_scanner}/`
  - `wasm/src/interfaces/{wasm_api,dto,errors,events}.rs`
  - `wasm/tests/conformance/` (+ `seeded_divergence.rs`, `fixtures/vectors/`)
- Canonical refs to update on implementation: every module-doc anchor listed in `## Related ADRs`
- Validators required:
  - architecture-validator (`domain_isolation`, layer direction, no verdict-parameter API)
  - security-validator (no key/path/PII in artifacts or messages; approval-claim mismatch refused)
  - test-validator (invariant + fault-matrix + must-fail divergence tests exist)
  - operations-validator (fail-closed at the FFI boundary, no ambient capability)
  - ci-mr-validator (live CLI diff, goldens, size budget, dependency ban list, npm version parity)

### Migration Steps
1. Freeze the six ports and the refusal vocabulary (Phase 0) before writing any aggregate code.
2. Enforce the eight invariants as unit tests as each aggregate method lands (Phase 1).
3. Prove fail-closed with the 6 × 4 fault matrix before wiring application services (Phase 2).
4. Land the engine `Clock`/`IdSource` seam behind a non-default feature and verify the four published crates' default suites unchanged (Phase 5).
5. Only then enable the byte-exact live CLI diff; delete any labelled normalised comparison (Phase 6).

### Status
- [x] Architecture doc updated
- [x] ADRs written (001–010)
- [x] Diagrams written (system-context, event-flow, 4 module diagrams)
- [x] Roadmap written
- [ ] Implementation started (crate source does not exist yet — `Source drift` validation will fail until Phase 0 lands)
- [ ] Validators run

---

<!-- Add new entries above this line -->

---

## Template Usage

When making architecture changes:

1. **Before change**: Review existing architecture docs
2. **During change**: Update `.pi/architecture/modules/[module].md`
3. **After change**: Add entry to this CHANGELOG
4. **Implementation**: Follow migration steps, update canonical refs
5. **Validation**: Run `validate-canonical.sh` to verify sync

---

## Architecture Sync Status

Track which changes have been synced to implementation:

| Date | Change | Module | Sync Status | Validator Status |
|------|--------|--------|-------------|------------------|
| 2026-11-03 | ADR-001 rewritten to real topology (one core context, four layers) | portable-session | doc complete, impl pending | pending (no source yet) |
| 2026-11-03 | ADR-002..ADR-010 created | portable-session | doc complete, impl pending | pending |
| 2026-11-03 | system-context.md rewritten; event-flow.md created | portable-session | doc complete | n/a |
| 2026-11-03 | Module doc enriched (aggregate, VOs, ports, events, endpoints, 24 ACs, patterns) | portable-session | doc complete, impl pending | pending |
| 2026-11-03 | implementation-roadmap.md generated (7 phases, Days 1-52) | portable-session | doc complete | n/a |
| 2026-11-03 | Source drift validation FAILS by construction (crate source does not exist) | ALL | expected | fail until Phase 0 |

---

*Last updated: 2026-11-03*
*Framework version: 1.2.0*