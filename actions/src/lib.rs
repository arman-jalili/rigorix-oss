//! Rigorix GitHub Actions — thin adapter over `rigorix-engine`.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Blueprint: actions/.pi/ROADMAP.md (2026-06-20)
//!
//! # Architecture
//!
//! The actions crate wraps the engine as a GitHub Action. It supports:
//! - **Mode A**: Reactive governance — PR diff analysis + policy enforcement
//! - **Mode B**: Active execution — code generation with validation loop
//!
//! All business logic lives in `rigorix-engine`. This crate adds only
//! GitHub-specific I/O: input parsing, output formatting, CI integration,
//! and audit posting.
//!
//! # Module Structure
//!
//! ```text
//! actions/src/
//! ├── main.rs              # Binary entry point
//! ├── lib.rs               # Library root ← this file
//! ├── shared/              # Shared infrastructure (no module dependencies)
//! │   ├── mod.rs
//! │   └── github_client.rs # GitHub REST API client
//! ├── action_input/        # GitHub Action input parsing (Phase 1)
//! ├── security_config/     # Phase 0 security validation (Phase 2)
//! ├── diff_analyzer/       # PR diff analysis (Phase 3)
//! ├── policy_evaluator/    # Policy rule enforcement (Phase 3)
//! ├── action_output/       # GitHub-native output formatting (Phase 3-4)
//! ├── ci_integration/      # Status checks, PR comments, labels (Phase 4)
//! ├── audit_posting/       # HMAC-signed audit records (Phase 4)
//! └── action_entrypoint/   # Event routing + dispatch (Phase 5)
//! ```
//!
//! # Contract Freeze
//!
//! All module interfaces are frozen per their architecture docs in
//! `actions/.pi/architecture/modules/`. Implementation PRs must
//! reference these canonical specifications.

// ── Phase 1: Scaffold + Shared ──
pub mod shared;

// ── Phase 1: Contract Freeze — interface-only declarations ──
// ── Phase 5: action_entrypoint — interface-only (contract frozen) ──
pub mod action_entrypoint;
pub mod action_input;
pub mod action_output;
pub mod audit_posting; // Phase 1: Contract Freeze (issue-contract-freeze)
pub mod ci_integration;
pub mod diff_analyzer; // Phase 1: Contract Freeze (issue-contract-freeze)
pub mod policy_evaluator;
pub mod security_config; // Phase 1: Contract Freeze (issue-contract-freeze) // Phase 1: Contract Freeze (issue-contract-freeze) // Phase 1: Contract Freeze (issue-contract-freeze) // Phase 1: Contract Freeze (issue-contract-freeze)
