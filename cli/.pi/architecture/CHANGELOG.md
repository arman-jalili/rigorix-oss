# Architecture Change Log

<!--
Canonical Reference: .pi/architecture/CHANGELOG.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT GENERATED FILES - Modify this source only
-->

This document tracks all architecture changes requiring implementation updates.

---

## Entries

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
