---
guardian_issue:
  id: "ISSUE-CONTRACT-FREEZE"
  epic: "portable-session"
  component: "Contract Freeze"
  module: "portable-session"
  status: planned
  priority: critical
  dependencies: []

  in_scope:
    - Define public interfaces for all components in this epic
    - Define DTOs, schemas, and API contracts
    - Document event payloads and topics
    - Create interface stubs with no implementation
    - Freeze: no implementation changes without contract change

  out_of_scope:
    - Any implementation logic
    - Database schema changes
    - Infrastructure setup

  affected_layers:
    domain:
      - Interface definitions for domain services
    application:
      - Input/output DTO definitions
    api:
      - REST/event contracts

  canonical_references:
    - module: ".pi/architecture/modules/portable-session.md"

  acceptance_criteria:
    - "All component interfaces defined as stubs (TODO bodies)"
    - "DTO schemas documented with field names and types"
    - "API contracts frozen and reviewed"
    - "Implementation PRs reference these contracts"

  validators:
    - architecture
    - canonical

  implementation_notes: |
    Define the contract before any implementation. Every implementation issue
    depends on this contract being frozen first. The contract should include:
    interfaces, types, DTOs, event schemas, API paths, error formats.

  file_changes:
    - "create: src/portable-session/domain/"
    - "create: src/portable-session/application/"
    - "create: src/portable-session/infrastructure/"
    - "create: src/portable-session/interfaces/"
---

# Contract Freeze: portable-session

## Intent

Define and freeze all public interfaces, contracts, and schemas for the portable-session
epic before any implementation begins. This prevents architecture drift — implementation
must satisfy contracts, not the other way around.

## Included Components

- ReplaySession
- Domain Layer (`domain/`)
- Application Layer (`application/`)
- Infrastructure Layer (`infrastructure/`)
- `ReplaySession::open`
- `ReplaySession::attempt`
- `ReplaySession::seal`
- `RefusalVocabulary`
- `EvidenceProjection`
- `AttemptRun`
- `ExportArtifactSet`
- Export stamp
- Host verification
- Bundle independence
- `WasmContract`
- `TelemetryObserver`
- `domain_isolation`
- `RunConformanceVector`
- `seeded_divergence`
- Bundle
- Release train
- Determinism
- Failure tiers

## What Must Be Frozen

### Interfaces
- Service interfaces for every component
- Repository/DAO interfaces
- Factory interfaces

### Contracts
- Input/output DTO schemas
- API endpoint contracts (method, path, request/response)
- Event payload schemas
- Error response formats

### Out of Bounds (no contracts needed)
- Internal implementation details
- Database column names (hidden behind repository)
- Framework-specific annotations

## Acceptance Criteria

| # | Criterion | How to Verify |
|---|-----------|---------------|
| 1 | All component interfaces defined as stubs (TODO bodies) | Check src/<module>/domain/ and application/ |
| 2 | Contracts reviewed and frozen | PR approval |
| 3 | DTO schemas documented with field names and types | OpenAPI / record types |
| 4 | Implementation depends on contracts | No implementation without interface |

## Full Module Acceptance Criteria (for reference)

> These are the complete ACs for the module. The contract freeze must define the interfaces
> so every row below can be implemented in subsequent issues.

| # | Component | Criterion | Verify In |
|---|-----------|-----------|-----------|
| 1 | `ReplaySession::open` | A scenario or fixture transcript that does not parse under engine types returns a typed `SessionOpenError`; no default is substituted, no panic crosses FFI | unit test + `parse_failure_is_refusal` |
| 2 | `ReplaySession::open` | Two sessions opened from identical inputs derive the same `ReplaySessionId`; different seeds derive different ids | unit test |
| 3 | `ReplaySession::attempt` | A second attempt on the same session returns `AttemptError::AlreadyAttempted`; the first outcome is unchanged | unit test |
| 4 | `ReplaySession::attempt` | With any port answer missing/exhausted/malformed, the result status is `refused` in 100% of fault-injection cases and `allowed` in 0% | unit + integration (fault injection) |
| 5 | `ReplaySession::attempt` | An identity-port failure yields engine status `unverified`; no message contains `unauthenticated` | unit test + message allow-list scan |
| 6 | `ReplaySession::seal` | A second `seal` returns `SealError::AlreadySealed`; the first `ArtifactSet` bytes are unchanged | unit test |
| 7 | `ReplaySession::seal` | Artifact bytes contain no key material, no absolute path, no PII: pre-export scan refuses rather than sanitises | unit test + CI scanner |
| 8 | `RefusalVocabulary` | Every engine refusal kind maps to a business-worded reason; an unmapped kind is a compile error (exhaustive match), never a generic string | unit test |
| 9 | `EvidenceProjection` | `StepOutcome` order equals engine evaluation order; `not_reached` is preserved as a distinct decision, not collapsed to `refused` | unit test |
| 10 | `AttemptRun` | `RunOutcome` never reports `refused` without naming the step that caused it (FR-002) | unit test |
| 11 | `ExportArtifactSet` | Stamps carry `artifact_format_version`, `engine_version` (from `ENGINE_VERSION`), scenario and fixture digests, `outcome_digest` | unit test |
| 12 | Export stamp | An artifact set whose `artifact_format_version` the host's verifier does not support yields `unverifiable(UnsupportedFormatVersion)` host-side; the crate never emits the words `not_verified` or `unverifiable` itself, and no message from the crate collapses one into the other (ADR-013) | unit test + message allow-list |
| 13 | Host verification | An artifact set produced in wasm verifies in a Node process that has never loaded the wasm module, using `rigorix-verifier` alone (ADR-013) | integration (wasm-pack + node harness) |
| 14 | Bundle independence | The wasm bundle's dependency tree contains no verifier crate, and `infrastructure/` contains no verification logic: the crate produces, the host checks (ADR-013) | architecture test + `cargo tree --target wasm32-unknown-unknown` |
| 15 | `WasmContract` | All four functions reject unknown DTO fields; none accepts a verdict/allow/override field | integration test |
| 16 | `WasmContract` | No exported function panics or throws on a refusal; only a malformed handle produces an error | integration test (fault matrix) |
| 17 | `TelemetryObserver` | `RandomDrawConsumed` never appears in any exported artifact set | unit test |
| 18 | `domain_isolation` | `domain/` names no `std::time`, `std::env`, `Uuid::new_v4`, `reqwest`, `rand::thread_rng`, no `infrastructure`/`interfaces` path | architecture test (CI) |
| 20 | `RunConformanceVector` | Committed goldens match; a stale golden fails the live diff rather than being regenerated silently | CI |
| 21 | `seeded_divergence` | The deliberately altered vector **must** diverge; if the comparator reports equality, the build fails | CI (must-fail test) |
| 23 | Bundle | Release wasm stays within the measured budget (+25%, NFR-002) and contains no tokio/reqwest/tree-sitter/prometheus/tracing-appender/libc | CI (size gate + `cargo tree`) |
| 24 | Release train | npm package version equals `rigorix-engine` version; default-feature suites of engine/mcp/cli/actions pass unchanged from the same commit | CI |
| 25 | Determinism | The same vector replayed twice in one wasm instance and once in a freshly instantiated module produces byte-identical artifact sets (NFR-001, across instances) | integration test |
| 26 | Failure tiers | A panic on the gate path fails the build (panic-site baseline); a trap is surfaced to the host as fatal and is never rendered as a refusal or an allow (ADR-012) | architecture test + host-contract test |
| 27 | Release train | `wasm/Cargo.toml` is in `[workspace] members`, the tag/version loop in `release.yml` names it, and `validate-workspace-pins.sh` passes with the engine pin (ADR-011) | CI |

## Full Implementation Sequence (for reference)

- 1. **Feasibility spike (P1)** — a branch, not a crate: engine gate slice on
- 2. **Crate skeleton + registration (P3/ADR-011)** — `wasm/Cargo.toml` inheriting the
- 3. **`domain/ports`** — six traits + `PortFailure` + `PortId`, with ADR-012's tiering
- 4. **`domain` value objects** — `ScenarioRef`, `FixtureWorld`, `ClockReading`, `RandomSeed`,
- 5. **`domain/services::RefusalVocabulary`** — exhaustive map; fixes the vocabulary before
- 6. **`domain` outcome VOs** — `StepOutcome`, `RunOutcome`, `EvidenceRef`, and the export
- 7. **`domain/session` aggregate** — `open` (FR-001), then `attempt` (FR-002/4/9/13), then
- 8. **`domain/events`** — event enum with evidence/telemetry classification.
- 9. **`infrastructure` adapters** — six ports, each adapter's failure path a test.
- 10. **`application`** — `OpenReplaySession` → `AttemptRun` → `ExportArtifactSet` with
- 11. **`interfaces::WasmContract`** — four exported functions, DTOs with
- 12. **Engine seam (P2/FR-011), in parallel from step 1** — call-site inventory, the two
- 13. **Conformance harness** — goldens, live CLI diff, then the seeded divergence
- 14. **CI gates** — size budget, dependency ban list, `domain_isolation`,
- 15. **npm packaging** — `rigorix-gate` at the engine version (ADR-011).



## Implementation

> **Agent:** Create interface-only files. No implementation. Use Clean Architecture layers:
> 1. Read the architecture module to understand each component's role
> 2. Place domain interfaces in domain/, service interfaces in application/, DTOs in application/
> 3. Controllers/routers are CONCRETE — they do NOT get interfaces. The concrete router/handler class goes directly in interfaces/http/ without a separate contract file.
> 4. Event schemas go in domain/event/
> 5. Repository interfaces go in infrastructure/repository/
>
> The goal is a reviewed, frozen contract that implementation issues can depend on.
