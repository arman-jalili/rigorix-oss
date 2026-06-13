# Architecture Change Log

<!--
Canonical Reference: .pi/architecture/CHANGELOG.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT GENERATED FILES - Modify this source only
-->

This document tracks all architecture changes requiring implementation updates.

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
