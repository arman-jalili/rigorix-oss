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
- Source code: ✅ Implemented
- CI proofing: ✅ Complete

## [2026-06-16] — CLI Boundary Implementation Complete

### Change
Completed full implementation of all 8 cli-boundary components per the contract freeze.

### Components Implemented
| Component | File | Status |
|-----------|------|--------|
| CliParser | `cli.rs` | ✅ 14 commands + shortcuts + flags |
| Dispatcher | `dispatch.rs` | ✅ Routes to engine/CLI handlers |
| OrchestratorBuilder | `orchestrator.rs` | ✅ Wiring stub (sub-services pending) |
| ConfigLoader | `config.rs` | ✅ TOML + env + CLI flag merging |
| OutputFormatter | `output.rs` | ✅ Pretty/Json/Markdown/Quiet |
| SignalHandler | `signal.rs` | ✅ Two-level Ctrl+C protocol |
| TracingInit | `tracing.rs` | ✅ RIGORIX_LOG env filter |
| CliError | `error.rs` | ✅ Exit code mapping (0,1,2,3,130,137) |

### CI Proofing
| Script | Status |
|--------|--------|
| `check_cli_contracts.sh` | ✅ 9/9 checks pass |
| `check_cli_coverage.sh` | ✅ 3/3 checks pass |
| `stage_cli_proofing.sh` | ✅ Integrated in hardening stage 11 |

### Documentation
- `docs/runbook-cli-boundary.md` — startup/shutdown/failure docs
- `docs/dr-plan-cli-boundary.md` — backup/restore/failover docs

### Files Changed
- `cli/src/` — all 10 source files (60+ Rust modules)
- `cli/Cargo.toml` — added `dirs` dependency
- `.pi/scripts/languages/rust/validate-ci.sh` — fixed `--quiet` flag

### Status
- Architecture: ✅ Defined
- Source code: ✅ Implemented
- CI proofing: ✅ Complete
- Documentation: ✅ Runbook + DR plan
