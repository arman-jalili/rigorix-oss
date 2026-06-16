# Architecture Change Log

## [2026-06-16] — Architecture Simplification (Single-Module CLI)

### Change
Simplified the CLI crate from 10 modules (with mirror wrappers for engine domains) to a single module: `cli_boundary`.

### Rationale
Per ADR-002, the CLI is a thin binary wrapper around `rigorix-engine`. The previous architecture created parallel domain layers in the CLI:
- CLI-specific service traits wrapping engine services (`ExecutionCommandService`, `PlanCommandService`, etc.)
- CLI-specific DTOs, errors, and events mirroring engine types
- Empty Repository and HTTP interface stubs
- 15 module docs describing engine concepts

All of these were deleted. The CLI now calls engine APIs directly.

### Files Deleted
- `cli/src/` — all source code (60+ files) removed pending regeneration
- `.pi/architecture/modules/` — 15 engine module docs removed
- `.pi/architecture/diagrams/` — 9 engine flow diagrams removed
- `.pi/architecture/decisions/` — 9 engine-internal ADRs removed
- `.pi/scripts/ci/` — 23 proofing scripts for removed modules removed

### Files Created/Updated
- `.pi/architecture/modules/cli-boundary.md` — single module doc
- `.pi/architecture/diagrams/system-context.md` — simplified diagram
- `.pi/domain/ubiquitous-language.md` — CLI terms only
- `.pi/domain/exploration.md` — reflects single-module architecture
- `.pi/architecture/CHANGELOG.md` — this file

### Status
- Architecture: ✅ Defined
- Source code: ❌ Removed (pending regeneration)
- CI proofing: ❌ Pending (regeneration)
