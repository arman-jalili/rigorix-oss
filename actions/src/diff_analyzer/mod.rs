//! Diff Analyzer — PR diff parsing, validation, risk classification, and AI signal detection.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: Contract Freeze — all component interfaces for diff-analyzer epic
//! Issue: issue-contract-freeze
//!
//! This module parses GitHub Pull Request diffs into structured `PrDiff` types,
//! validates file paths against security rules, enforces resource limits,
//! classifies file changes by risk level, and detects AI-generated code signals.
//! It is the input layer for the Policy Evaluator.
//!
//! # Components
//!
//! | Component | Domain | Application | Infrastructure | Interfaces |
//! |-----------|--------|-------------|----------------|------------|
//! | PrDiff | `domain::types::PrDiff` | `application::dto` | — | `interfaces::http` |
//! | ChangedFile | `domain::types::ChangedFile` | — | — | — |
//! | DiffParser | — | `application::service::DiffParsingService` | — | — |
//! | PathValidator | — | `application::service::PathValidationService` | `infrastructure::repository::DiffRepository` | — |
//! | LimitEnforcer | — | `application::service::LimitEnforcementService` | — | — |
//! | RiskClassifier | — | `application::service::RiskClassificationService` | — | — |
//! | AiSignalDetector | — | `application::service::AiSignalDetectionService` | — | — |
//!
//! # Layer Structure
//!
//! ```text
//! diff_analyzer/
//! ├── mod.rs                          # Module root
//! ├── domain/                         # Domain entities and interfaces
//! │   ├── mod.rs
//! │   ├── types.rs                    # PrDiff, ChangedFile, DiffHunk, FileStatus, FileRisk, AiSignal, AiSignalResult
//! │   ├── error.rs                    # DiffAnalyzerError
//! │   └── event/
//! │       └── mod.rs                  # DiffAnalyzerEvent payloads
//! ├── application/                    # Application service interfaces and DTOs
//! │   ├── mod.rs
//! │   ├── service.rs                  # Service traits (DiffParsingService, PathValidationService, etc.)
//! │   ├── dto/
//! │   │   └── mod.rs                  # Input/output DTO schemas
//! │   └── factory.rs                  # Factory interfaces
//! ├── infrastructure/                 # Infrastructure layer
//! │   ├── mod.rs
//! │   └── repository/
//! │       └── mod.rs                  # Repository interfaces (DiffRepository)
//! └── interfaces/                     # External interfaces
//!     ├── mod.rs
//!     └── http/
//!         └── mod.rs                  # HTTP API contracts
//! ```
//!
//! # Contract Freeze
//!
//! All public interfaces, DTO schemas, and contracts in this module are
//! frozen. Implementation must satisfy these contracts, not the other way around.
//! See `actions/.pi/architecture/modules/diff-analyzer.md` for the canonical spec.
//!
//! # Dependencies
//!
//! - **GitHub API**: Fetching PR diff via `GitHubClient` (from `crate::shared`)
//! - **Globset**: Glob pattern compilation (shared with policy-evaluator)
//!
//! # Related Issues
//!
//! - Issue #552: Contract Freeze (this issue)
//! - Issue #551: Epic "diff-analyzer"

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
