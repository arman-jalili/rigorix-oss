---
guardian_issue:
  id: "ISSUE-PORTABLE-SESSION-18"
  epic: "TBD"
  component: "`RunConformanceVector`"
  module: "portable-session"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - "Committed goldens match; a stale golden fails the live diff rather than being regenerated silently"

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/portable-session.md"
    - acceptance_criteria: ".pi/architecture/modules/portable-session.md#acceptance-criteria"

  acceptance_criteria:
    - "Committed goldens match; a stale golden fails the live diff rather than being regenerated silently"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Read .pi/architecture/modules/portable-session.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: `RunConformanceVector`.
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.ts file.

  file_changes:
    - "create: src/portable-session/domain/"
    - "create: src/portable-session/application/"
    - "create: src/portable-session/infrastructure/"
    - "modify: src/portable-session/interfaces/"
    -     - "update: tests/unit/ (failing tests already generated — make them pass)"
---

# ISSUE-PORTABLE-SESSION-18: Implement `RunConformanceVector` — portable-session

## Intent

Implement **`RunConformanceVector`** for the `portable-session` module.

> ⚠️ **Read before implementing:** `.pi/architecture/modules/portable-session.md`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** portable-session
- **Component:** `RunConformanceVector`
- **Status:** planned
- **Dependencies:** none

## In Scope (this component)

- Committed goldens match; a stale golden fails the live diff rather than being regenerated silently

## Acceptance Criteria (this issue)

These acceptance criteria must be satisfied before this issue can be closed:

| # | Criterion | Verify In |
|---|-----------|-----------|
| 20 | Committed goldens match; a stale golden fails the live diff rather than being regenerated silently | CI |

## Full Module Acceptance Criteria

> All items below must pass before the **epic** is closed.
> Items may be split across multiple issues — verify your component's items before creating the MR.

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

## Implementation Sequence (from module doc)

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

> **Agent instructions:**
> 1. Open `.pi/architecture/modules/portable-session.md` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **`RunConformanceVector`**
> 3. Create concrete implementation files (`.impl.ts`) in `src/portable-session/` — the interface stubs from the contract freeze are NOT enough
> 4. Each domain aggregate/service must have a working implementation with business logic
> 5. Verify each AC row is satisfied in `src/` before marking done
> 6. Run validators and create MR

### Steps

1. Read canonical architecture references
2. Run the pre-generated failing tests: `cd tests/unit && bun test`
3. Verify tests FAIL (Red phase)
4. Implement domain entities and interfaces
5. Implement application service/handler
6. Add infrastructure connections
7. Run tests again — they should PASS (Green phase)
8. Refactor if needed (Refactor phase)
9. Write integration tests
10. Run all validators
11. Create MR
