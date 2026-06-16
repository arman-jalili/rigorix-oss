# Architecture Change Log

<!--
Canonical Reference: .pi/architecture/CHANGELOG.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT GENERATED FILES - Modify this source only
-->

This document tracks all architecture changes requiring implementation updates.

---

## Entries

## [2026-06-16] - Configuration Module Implementation (Issues #245, #246, #247, #248)

### Changes
- Added `api_key_configured` field to `CliConfig` for API key presence tracking
- Added `validate_api_key_for_command()` — fails fast with clear error for missing API key
- Added `build_engine_cli_overrides()` — bridges CLI config to engine ConfigService
- Wired engine `ConfigService` into CLI startup sequence (init_engine_config)
- Created proofing scripts: check_config_contracts.sh (17 checks), check_config_coverage.sh
- Created stage_config_proofing.sh — CI stage wrapper
- Integrated stage 12 (config_proofing) into CI hardening pipeline
- Updated configuration module architecture doc with final file paths and contracts

### Files Created
- `cli/.pi/scripts/ci/check_config_contracts.sh` — 17 automated config contract checks
- `cli/.pi/scripts/ci/check_config_coverage.sh` — Coverage threshold enforcement
- `cli/.pi/scripts/ci/stage_config_proofing.sh` — CI stage wrapper

### Files Modified
- `cli/src/domain/config.rs` — Added api_key_configured field
- `cli/src/infrastructure/config_impl.rs` — API key tracking, validation helpers, engine bridge
- `cli/src/main.rs` — Startup validation + engine ConfigService integration
- `cli/.pi/architecture/modules/configuration.md` — Updated with final implementation details
- `cli/.pi/scripts/ci/run_hardening_stages.sh` — Added stage 12

### Status
- Configuration module: IMPLEMENTED
- 38 tests passing, clippy clean, fmt clean
- CI proofing scripts: stage 12 — config_proofing — ALL PASS

## [2026-06-16] - Phase 1 Implementation Complete (Issues #237, #238, #239)

### Changes
- Implemented CliConfigLoaderImpl: multi-source config merging (flags → env → file → defaults)
- Implemented SignalHandlerImpl: Ctrl+C double-press detection (2s window)
- Implemented LogFormatterImpl: pretty, JSON, and quiet output for all 10 commands
- Implemented init_tracing: tracing-subscriber with pretty/json format support
- Created proofing scripts: check_cli_contracts.sh, check_cli_coverage.sh, stage_cli_proofing.sh
- Integrated stage 11 (cli_proofing) into CI hardening pipeline
- Created docs/runbook.md and docs/dr-plan.md

### Files Created
- `cli/src/infrastructure/config_impl.rs` — Config loader implementation
- `cli/src/infrastructure/signal_impl.rs` — Signal handler implementation
- `cli/src/infrastructure/output_impl.rs` — Output formatter implementation
- `cli/src/tracing.rs` — Tracing initialization
- `cli/.pi/scripts/ci/check_cli_contracts.sh` — Contract proofing
- `cli/.pi/scripts/ci/check_cli_coverage.sh` — Coverage proofing
- `cli/.pi/scripts/ci/stage_cli_proofing.sh` — CI stage wrapper
- `cli/docs/runbook.md` — Operations runbook
- `cli/docs/dr-plan.md` — Disaster recovery plan

### Status
- CLI boundary: IMPLEMENTED (Phase 1 complete)
- All validators pass: ci, tests, security, architecture, canonical, operations

## [2026-06-16] - Initial Architecture Scaffold (Session 71e2b81a)

### Added
- ADR-001: Domain-Driven Design with Bounded Contexts (Accepted)
- ADR-002: CLI/Engine Architecture Split (Accepted)
- ADR-003: TUI Framework — ratatui (Accepted)
- ADR-004: Template Format — TOML (Accepted)
- ADR-005: EventBus for Cross-Context Communication (Accepted)
- ADR-006: Plugin System Deferral to v2 (Accepted)
- ADR-007: Ephemeral CLI — No Daemon for v1 (Accepted)
- ADR-008: Atomic Write-Rename for State Persistence (Accepted)
- ADR-009: LLM Provider — Anthropic Claude (Accepted)
- ADR-010: Template Generation Persistence Strategy (Accepted)
- ADR-011: Retry and Backoff Strategy (Accepted)
- ADR-012: Risk Gating Levels and Policies (Accepted)
- 18 module docs under `.pi/architecture/modules/` — one per bounded context
- Updated system-overview.md — replaced generic template with rigorix-specific architecture
- 10 per-module diagram files under `.pi/architecture/diagrams/`

### Changed
- Module: ALL
  - Component: all 18 module docs populated with Components, Domain Events, Ubiquitous Language, Dependencies, Key Files, ADRs
  - Fixed: filenames had `**` markdown bold markers — renamed to clean names
  - Fixed: system-context.md mermaid — removed `**` inside nodes (mermaid compat), simplified edge set
  - Fixed: system-overview.md — was a generic template with Web/Mobile/Auth/Gateway content, now has actual rigorix layered architecture, execution flow, and security boundaries

### Added Diagram Files
- `cli-boundary-flow.md` — command dispatch tree
- `planning-pipeline-flow.md` — 6-phase flow with fallback path
- `execution-engine-flow.md` — parallel executor loop and per-node execution
- `template-generation-flow.md` — explicit vs fallback trigger paths
- `event-system-flow.md` — pub-sub architecture with 11 event variants
- `dag-engine-lifecycle.md` — two-phase construction lifecycle
- `enforcement-gate-flow.md` — tool execution gating flow with policy matrix
- `state-persistence-flow.md` — atomic write-rename and crash recovery
- `audit-flow.md` — envelope lifecycle from collection to delivery
- `cancellation-flow.md` — two-level shutdown signal processing

### Impact Analysis
- Files affected:
  - `.pi/architecture/modules/` — all 18 module docs
  - `.pi/architecture/decisions/` — 12 ADR files
  - `.pi/architecture/diagrams/` — 12 diagram files (2 original + 10 new)
  - `.pi/architecture/CHANGELOG.md`
- Canonical refs to update:
  - All module docs reference their engine crate sources (frozen contracts)
  - ADRs reference affected modules
- Validators required:
  - architecture-validator (verify module doc + ADR completeness)
  - canonical (verify references match engine crate)

### Migration Steps
1. Review each module doc alignment with engine crate modules
2. Verify `# Contract (Frozen)` references match actual engine source files
3. Confirm dependency direction matches the system-context diagram
4. Review ADR decisions against implementation planning

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [ ] Implementation updated (pending)
- [ ] Canonical refs updated (pending)
- [ ] Validators run (pending)

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
| 2026-06-16 | Initial scaffold (18 module docs, ADR-001, diagram) | ALL | pending | pending |

---

*Last updated: 2026-06-16*
*Framework version: 1.2.0*
