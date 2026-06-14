# Architecture Change Log

<!--
Canonical Reference: .pi/architecture/CHANGELOG.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT GENERATED FILES - Modify this source only
-->

This document tracks all architecture changes requiring implementation updates.

---

## [2026-06-14] - Planning Pipeline Epic Implementation Complete

### Added
- Module: planning-pipeline
  - Contract freeze: UserIntent, PlanningResult, PlanningHash, PlanOutput,
    ClassificationResult, Classifier trait, ParameterExtractor trait,
    TemplateGenerator trait, PlanningError (12 variants), PlanningEvent (12 payloads),
    PlanningPipelineService trait (10 methods), PlanningPipelineFactory trait (3 methods),
    CompositeValidator trait, PlanningResultRepository trait, HTTP API contracts (5 endpoints),
    10+ DTO types
  - Implemented: PlanningPipelineImpl — 6-phase orchestrator with budget pre-check,
    intent classification, parameter extraction, graph generation, plan validation,
    deterministic SHA-256 hash computation
  - Implemented: PlanningPipelineFactoryImpl — three factory methods
    (create_default, create_with_generator, create_custom)
  - Implemented: MockClassifier — deterministic test double with configurable
    confidence thresholds for auto-select/clarification/generator paths
  - Implemented: MockParameterExtractor — test double with defaults, overrides, errors
  - Implemented: ClaudeClassifier — Anthropic Messages API via reqwest
    (claude-sonnet-4-20250514, configurable endpoint, timeout, temperature)
  - Implemented: OpenaiClassifier — OpenAI Chat Completions API via reqwest
    (gpt-4o, Bearer auth, token usage tracking)
  - Implemented: compute_planning_hash — public SHA-256 based deterministic hash
  - 57 unit tests across all layers
  - Documentation: runbook-planning-pipeline.md, dr-plan-planning-pipeline.md
  - CI: Proofing stage (stage 23) added to hardening pipeline with 30 contract checks
    + coverage threshold (57 tests found)
  - Architecture: Module doc updated with final implementation details
  - Verified: All proofing checks pass, build clean, cargo test passes

---

## [2026-06-14] - Repo Engine Epic Implementation Complete

### Added
- Module: repo-engine (Final Epic Issue)
  - Contract freeze: SymbolGraph, SymbolDefinition (12 SymbolKind variants, SourceLanguage,
    SymbolVisibility), SharedSymbolGraph, SymbolWorkspaceIntent (4 variants),
    RepoEngineError (11 variants), RepoEngineEvent (11 event payloads)
  - Implemented: SymbolGraphServiceImpl — RwLock-backed thread-safe graph with
    add, lookup (with adjacency), search (with kind/language filters), remove, clear,
    stats, reference tracking
  - Implemented: WorkspaceValidationServiceImpl — Phase 3 pre-execution validation
    for ReadOnly, ReadWrite, Modification, Deletion intents with conflict detection
    and orphaned reference checking
  - 46 unit tests across all layers (SymbolGraphService: 19, SymbolDefinition: 13,
    SymbolWorkspaceIntent: 7, WorkspaceValidationService: 7)
  - All tests passing (cargo test -p rigorix -- repo_engine)
  - Documentation: runbook-repo-engine.md, dr-plan-repo-engine.md
  - CI: Proofing stage (stage 22) added to hardening pipeline with 28 contract checks
    + coverage threshold
  - Architecture: Module doc updated with final implementation details
  - Verified: All proofing checks pass, build clean

---

## [2026-06-14] - Template System Epic Implementation Complete

### Added
- Module: template-system (Phase 2, Module #10)
  - Contract freeze: Template, TemplateNode, TemplateAction (9 variants), ParameterDef,
    TemplateError (10 variants), TemplateEvent (7 payload schemas)
  - Implemented: TemplateParserImpl — TOML deserialization, structural validation,
    cycle detection via Kahn's algorithm, directory loading
  - Implemented: TemplateEngineImpl — in-memory registry, `{{ param }}` substitution,
    topological sort, graph generation
  - Implemented: InMemoryTemplateRepository — test double for TemplateRepository
  - 31 unit tests across all layers (TemplateParser: 21, TemplateEngine: 10)
  - All tests passing (cargo test --lib templates)
  - Documentation: runbook-template-system.md, dr-plan-template-system.md
  - CI: Proofing stage (stage 20) added to hardening pipeline with 13 contract checks
    + coverage threshold (21 tests detected)
  - Architecture: Module doc updated to v2.0.0 with actual implementation details
  - Verified: All proofing checks pass, build clean, fmt compliant

### Changed
- Architecture: template-system module doc updated from blueprint to implementation-reflect

---

## [2026-06-14] - Risk-Gating Epics Implementation Complete

### Added
- Module: risk-gating
  - Contract freeze: RiskLevel, RiskClassifier trait, RiskConfig entity, RiskGatingError (5 variants),
    RiskGateEvent (5 event types), RiskGateService trait (7 methods), RiskGateFactory trait (4 methods),
    RiskConfigRepository trait, HTTP API contracts (6 endpoints), 12 DTO types
  - Implemented: DefaultClassifier — 20+ built-in tool→risk mapping rules with override precedence
  - Implemented: RiskGateServiceImpl — evaluate_gate, classify_tool, resolve_gate, override_tool, reload_config
  - Implemented: RiskGateFactoryImpl — create_default, create_from_config, create_with_overrides, create_with_policy
  - Implemented: GateStateRegistry — thread-safe pending gate tracking with register/resolve/cleanup
  - Implemented: InMemoryConfigRepository — per-execution RiskConfig storage
  - 89 unit tests across all layers (RiskLevel: 21, RiskConfig: 26, DefaultClassifier: 15,
    GateStateRegistry: 5, RiskGateServiceImpl: 13, RiskGateFactoryImpl: 4, InMemoryConfigRepository: 7)
  - Documentation: runbook-risk-gating.md, dr-plan-risk-gating.md
  - CI: Proofing stage (stage 19) added to hardening pipeline with 21 contract checks + coverage threshold
  - Verified: All proofing checks pass

## [2026-06-14] - State Persistence Implementation Complete

### Added
- Module: state-persistence
  - Implemented: FileSystemStateRepository — filesystem state storage with atomic write-rename
  - Implemented: FileSystemStateManager — StateManagerService with save/load/node-transitions/list
  - Implemented: FileSystemGraphRepository — graph storage for TUI history with execution_id index
  - Implemented: FileSystemGraphManager — GraphManagerService with CRUD for execution graphs
  - Implemented: FileSystemExecutionRecordRepository — complete execution record storage
  - Implemented: FileSystemStateManagerFactory + FileSystemGraphManagerFactory — constructors
  - Documentation: runbook-state-persistence.md, dr-plan-state-persistence.md
  - CI: Proofing stage (stage 18) added to hardening pipeline
  - Coverage: 41 unit tests for state_persistence module
  - Verified: 409 total tests passing across entire project

---

## [2026-06-13] - Initial Architecture Scaffold from Domain Exploration

### Added
- Decision Records: 8 ADRs covering key architecture decisions
  - ADR-001: Domain-Driven Design with 17 Bounded Contexts (Modular Monolith)
  - ADR-002: TOML Template Format for Workflow Definitions
  - ADR-003: LLM Provider Abstraction via Traits (Classifier/Extractor/Generator)
  - ADR-004: Three Autonomy Presets with Hard Enforcement Caps (Default/Advanced/Aggressive)
  - ADR-005: Event Bus with Synchronous In-Memory Persistence (tokio broadcast + Mutex)
  - ADR-006: Atomic Write-Rename for State Persistence (crash-safe)
  - ADR-007: Risk Gating Model — Low / Medium / High with RiskClassifier
  - ADR-008: RAII Budget Reservation for LLM Cost Control

### Changed
- Module: template-system
  - Created: Full module document with TOML parsing, TemplateEngine, built-in template loading
  - Diagram: TOML file → Parser → Engine → TaskGraph generation flow
- Module: planning-pipeline
  - Created: 6-phase pipeline (budget → classify → extract → generate → validate → hash)
  - Diagram: Phase flow with generator fallback on low confidence
- Module: template-generation
  - Created: TemplateGenerator trait, ClaudeTemplateGenerator, OpenaiTemplateGenerator, RepoContext
  - Diagram: Generation subprocess with 3-attempt LLM retry loop and Phase 3 symbol validation
- Module: dag-engine
  - Created: Two-phase TaskGraph construction, Kahn's algorithm topological sort, cycle detection
  - Diagram: Node addition → seal → sort → validate → execute
- Module: execution-engine
  - Created: ParallelExecutor with tokio JoinSet, per-node retry loop, backoff/jitter
  - Diagram: Node lifecycle decision tree (start → tool → retry/fallback)
- Module: risk-gating
  - Created: RiskClassifier mapping tool names to Low/Medium/High with configurable RiskConfig
  - Diagram: Classification → gating decision tree
- Module: tool-system
  - Created: Tool trait, ToolRegistry, 9 tool implementations, execute_with_risk_gate
  - Diagram: Registry lookup → risk gate → execute flow
- Module: repo-engine
  - Created: Multi-language symbol indexing (Rust/Python/TypeScript), SymbolGraph with O(1) lookup
  - Diagram: Language-specific indexers → SymbolGraph → consumers
- Module: event-system
  - Created: EventBus with tokio broadcast + synchronous Mutex persistence, 11 event variants
  - Diagram: Publishers → broadcast + persistence → subscribers
- Module: enforcement
  - Created: EnforcementConfig with 3 presets, ExecutionEnforcer with atomic counters
  - Diagram: Per-limit checking flow with config validation
- Module: budget-tracking
  - Created: LlmBudget with RAII reservation pattern, auto-rollback on Drop
  - Diagram: Reserve → commit/rollback → exhaustion handling
- Module: state-persistence
  - Created: ExecutionState, NodeState, StateManager with atomic write-rename
  - Diagram: Sequence diagram of Orchestrator → StateManager lifecycle
- Module: cancellation
  - Created: CancellationManager with Graceful/Immediate shutdown signals
  - Diagram: Sequence diagram of signal propagation
- Module: failure-classification
  - Created: FailureType enum (7 variants), classify_failure(), RetryStrategy mapping
  - Diagram: Error message → FailureType → RetryStrategy mapping
- Module: audit
  - Created: AuditEnvelope, AuditSender with circuit breaker, AuditQueue
  - Diagram: Envelope build → send → circuit breaker retry
- Module: configuration
  - Created: Multi-source Config loading (env/file/CLI), Secret wrapper for API keys
  - Diagram: Layered loading → sub-config distribution
- Module: error-handling
  - Created: CoreOrchestratorError root type with #[from] for all 11 domain errors
  - Diagram: Error hierarchy tree with all domain errors

- Diagrams: system-context
  - Created: Mermaid interaction graph showing phased architecture (Planning → Execution → Observability → Cross-cutting)
  - Created: Sequence diagram of full execution lifecycle
- Diagrams: system-overview
  - Created: CLI-specific ASCII art layer architecture (replaced generic web app template)
  - Created: Module dependency graph, data flow overview, event flow, security boundaries

### Impact Analysis
- Files affected:
  - All `.pi/architecture/modules/*.md` (17 module documents)
  - All `.pi/architecture/decisions/ADR-*.md` (8 ADRs)
  - `.pi/architecture/diagrams/system-context.md`
  - `.pi/architecture/diagrams/system-overview.md`
- Canonical refs to update:
  - `.pi/domain/exploration.md` (source — already synced)
  - `.pi/domain/ubiquitous-language.md` (source — already synced)
- Validators required:
  - architecture-validator

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [ ] Implementation updated
- [x] Canonical refs updated
- [ ] Validators run

---

## [2026-06-13] - Configuration Module Implementation (Phase 0)

### Added
- Module: configuration
  - Implemented: `ConfigService` with multi-source loading (CLI > ENV > file > defaults)
  - Implemented: `FilesystemConfigRepository` for reading TOML files and path resolution
  - Implemented: `ConfigFactoryImpl` for building `ConfigDto` from TOML, env overrides, CLI flags
  - Implemented: `SecretService` for loading API keys from environment variables
  - Implemented: `SecretFactoryImpl` for wrapping secret values with redacted output
  - Implemented: Full validation against `SafetyCaps` (parallelism, retries, tokens, temperature)
  - 27 unit tests across all layers
- Module: configuration (contract freeze)
  - Defined: All domain entities (Config, Secret, ConfigurationError)
  - Defined: Service, factory, and repository traits
  - Defined: DTOs with validation, event payloads, HTTP API contracts
  - Defined: Canonical references for all 15 source files (100% coverage)
- CI: configuration_proofing stage (stage 11) in hardening pipeline
  - `check_configuration_contracts.sh` — validates all interfaces have implementations
  - `check_configuration_coverage.sh` — enforces 80% coverage threshold
- Docs: `docs/runbook-configuration.md`, `docs/dr-plan-configuration.md`

### Changed
- `.pi/architecture/modules/configuration.md` — reference implementation matches spec
- All extension validator paths updated to `engine/.pi/scripts/...` prefix
- Security, CI, and canonical validators fixed for workspace layout

### Impact Analysis
- Files affected:
  - `engine/src/configuration/` (22 source files total)
  - `engine/.pi/scripts/ci/check_configuration_*.sh` (3 new scripts)
  - `engine/.pi/scripts/ci/run_hardening_stages.sh` (stage 11 added)
  - `docs/runbook-configuration.md`, `docs/dr-plan-configuration.md`
- Validators required: ci, tests, security, architecture, canonical, operations

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [x] Implementation updated
- [x] Canonical refs updated
- [x] Validators run

---

## [2026-06-13] - Audit Module Implementation + Contract Freeze + Proofing

### Added
- Module: audit
  - Implemented: Full audit module with 8 implementation files
  - Implemented: `AuditEnvelopeFactoryImpl` — envelope building with SHA-256 hash + HMAC signing
  - Implemented: `AuditSenderImpl` — HTTP delivery with reqwest, exponential backoff + jitter
  - Implemented: `AuditQueueImpl` — bounded in-memory FIFO queue (capacity 100)
  - Implemented: `CircuitBreakerImpl` — Closed/Open/HalfOpen state machine with atomic counters
  - Implemented: `AuditServiceImpl` — orchestrates build-and-send flow with retry management
  - Implemented: `LocalAuditEnvelopeRepository` — filesystem persistence with atomic write-rename
  - 34 unit tests across all layers (61 total project-wide)
- Module: audit (contract freeze)
  - Defined: All domain entities (AuditEnvelope, AuditError, AuditEvent, CircuitBreakerState)
  - Defined: Service, factory, and repository traits (AuditService, AuditSender, AuditQueue, CircuitBreaker)
  - Defined: DTOs, HTTP API contracts (5 endpoints), unified error format
  - Defined: Canonical references for all 15 source files
- CI: audit_proofing stage (stage 12) in hardening pipeline
  - `check_audit_contracts.sh` — validates all 11 interfaces have implementations
  - `check_audit_coverage.sh` — enforces 80% coverage threshold
  - `stage_audit_proofing.sh` — CI stage wrapper
- Docs: `docs/runbook-audit.md`, `docs/dr-plan-audit.md`

### Changed
- `.pi/architecture/modules/audit.md` — updated with final implementation details
- `engine/Cargo.toml` — added uuid, chrono, sha2, hmac, reqwest, rand dependencies
- `engine/src/lib.rs` — added `pub mod audit;`

### Impact Analysis
- Files affected:
  - `engine/src/audit/` (26 source files total)
  - `engine/.pi/scripts/ci/check_audit_*.sh` (3 new scripts)
  - `engine/.pi/scripts/ci/run_hardening_stages.sh` (stage 12 added)
  - `docs/runbook-audit.md`, `docs/dr-plan-audit.md`
  - `.pi/architecture/modules/audit.md`
  - `.pi/architecture/CHANGELOG.md`
- Validators required: ci, tests, security, architecture, canonical, operations

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [x] Implementation updated
- [x] Canonical refs updated
- [x] Validators run

---

## [2026-06-13] - Failure Classification Module Implementation (Phase 0)

### Added
- Module: failure-classification
  - Implemented: `FailureClassifierServiceImpl` — pattern-matching classification for 7 failure types
  - Implemented: `FailureMappingServiceImpl` — default FailureType→RetryStrategy mapping with override support
  - Implemented: `StrategyFactoryImpl` — validated RetryStrategy construction (ExpandContext 0–5, PatchWithFeedback non-empty)
  - Implemented: `classify_failure()` free function for quick classification
  - Implemented: Comprehensive integration tests covering all 7 FailureType→Strategy pipelines
  - 125 unit/integration tests across all layers (215 total project-wide)
- Module: failure-classification (contract freeze)
  - Defined: All domain entities (FailureType, RetryStrategy, FailureClassificationError)
  - Defined: Service, factory, and repository traits
  - Defined: DTOs with validation, event payloads, HTTP API contracts (4 endpoints)
  - Defined: Canonical references for all 15 source files
- CI: failure-classification_proofing stage (stage 14) in hardening pipeline
  - `check_failure-classification_contracts.sh` — validates all 19 contract points
  - `check_failure-classification_coverage.sh` — enforces 80% coverage threshold (125 tests found)
  - `stage_failure-classification_proofing.sh` — CI stage wrapper
- Docs: `docs/runbook-failure-classification.md`, `docs/dr-plan-failure-classification.md`

### Changed
- `.pi/architecture/modules/failure-classification.md` — updated with final implementation details
- `engine/src/lib.rs` — added `pub mod failure_classification;`
- `engine/.pi/scripts/ci/run_hardening_stages.sh` — stage 14 added

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [x] Implementation updated
- [x] Canonical refs updated
- [x] Validators run

---

## [2026-06-13] - Event-System Module Implementation (event-system Epic)

### Added
- Module: event-system (contract freeze)
  - Defined: `ExecutionEvent` enum with 11 variants (all execution lifecycle events)
  - Defined: `PersistedEvent` with monotonic sequence numbering
  - Defined: `EventSystemError` with 8 structured error variants
  - Defined: `EventBusService` trait (publish, subscribe, drain_persisted, query_events, status)
  - Defined: `EventBusFactory` trait with config validation (min 16 channel, 64 buffer)
  - Defined: `PersistedEventRepository` trait (save, query, drain, count, prune, clear)
  - Defined: 12 DTOs for all operations
  - Defined: 6 HTTP endpoints with unified error format
  - Canonical references for all 13 source files (100% coverage)
- Module: event-system (implementation)
  - Implemented: `EventBusServiceImpl` — tokio broadcast + Mutex persistence + AtomicU64 sequences
  - Implemented: `EventBusFactoryImpl` — validated construction with min capacity checks
  - Implemented: `InMemoryEventRepository` — thread-safe Vec storage with bounded capacity
  - Implemented: `ExecutionEvent` helper methods — event_type_name(), execution_id(), timestamp(),
    summary(), is_terminal(), is_error()
  - Implemented: 11 convenience constructors — one per variant
  - Implemented: 34 serde round-trip + helper + constructor tests
- Observability:
  - `EventBusService::status()` — persisted_count, current_sequence, subscribers, capacities
  - `EventBusService::event_count()` — published total, persisted, drained
- CI: event-system_proofing stage (stage 15) in hardening pipeline
  - `check_event-system_contracts.sh` — validates all 34 contract points
  - `check_event-system_coverage.sh` — enforces 80% coverage threshold (63 tests found)
  - `stage_event-system_proofing.sh` — CI stage wrapper
- Docs:
  - `docs/runbook-event-system.md` — startup, shutdown, failure modes, config, metrics
  - `docs/dr-plan-event-system.md` — RTO/RPO, backup, restore, failover, testing

### Changed
- `.pi/architecture/modules/event-system.md` — updated with final implementation details
- `engine/.pi/scripts/ci/run_hardening_stages.sh` — stage 15 (event-system_proofing) added
- `engine/src/lib.rs` — added `pub mod event_system;`

### Impact Analysis
- Files affected:
  - `engine/src/event_system/` (19 source files total)
  - `engine/.pi/scripts/ci/check_event-system_*.sh` (3 new scripts)
  - `engine/.pi/scripts/ci/run_hardening_stages.sh` (stage 15 added)
  - `docs/runbook-event-system.md`, `docs/dr-plan-event-system.md`
  - `.pi/architecture/modules/event-system.md`
  - `.pi/architecture/CHANGELOG.md`
- Validators required: ci, tests, security, architecture, canonical, operations

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [x] Implementation updated
- [x] Canonical refs updated
- [x] Validators run

---

## [2026-06-13] - Domain Exploration (Session 63c25384)

### Added
- Domain exploration document with 17 bounded contexts based on the rigorous core crate analysis
- 42-term ubiquitous language glossary with prohibited alias tracking
- Actor/role identification: Developer, LLM Provider, PlanValidator, RiskClassifier, ExecutionEnforcer, TemplateGenerator, Audit System
- 35 functional requirements and 17 non-functional requirements with priority/category mapping
- 48 entities with type classification (Aggregate Root / Entity / Value Object)
- 28 domain events tracking all state transitions across contexts
- 8 design assumptions with impact analysis and mitigations
- 9 open questions for future resolution

### Changed
- Module: N/A (domain artifacts only)
- RiskLevel: Kept superior RiskClassifier design with Low/Medium/High values (matching actual codebase)
- Enforcement: Updated to 3-preset model (Default/Advanced/Aggressive) matching actual enforcement.rs
- Persistence: Replaced generic StateSnapshot with ExecutionState/NodeState/StateManager matching actual state/persistence.rs

### Impact Analysis
- Canonical refs to update:
  - `.pi/domain/exploration.md`
  - `.pi/domain/ubiquitous-language.md`

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [ ] Implementation updated
- [x] Canonical refs updated
- [ ] Validators run

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

| Date | Change | Module | Sync Status | Validator Status |
|------|--------|--------|-------------|-----------------|
| 2026-06-13 | Initial Architecture Scaffold | All 17 modules | complete | pending |
| 2026-06-13 | Domain Exploration (Session 63c25384) | domain/exploration.md, domain/ubiquitous-language.md | complete | pending |

---

*Last updated: 2026-06-13*
*Architecture version: 1.0.0*
