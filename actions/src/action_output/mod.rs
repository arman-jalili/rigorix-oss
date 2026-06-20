//! Action Output — GitHub Actions-native output formatting bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — all component interfaces for action-output epic
//! Issue: issue-contract-freeze
//!
//! This module formats `rigorix-engine` execution results as GitHub Actions-native
//! outputs — annotations, step summaries, output variables, and PR comments.
//! It converts structured engine types into GitHub workflow commands without
//! adding business logic (pure presentation adapter).
//!
//! # Components
//!
//! | Component | Domain | Application | Infrastructure | Interfaces |
//! |-----------|--------|-------------|----------------|------------|
//! | OutputFormatter | `domain::types::FormattedOutput` | `application::service::OutputFormattingService` | — | `interfaces::http` |
//! | AnnotationWriter | `domain::types::WorkflowAnnotation` | `application::service::AnnotationWritingService` | `infrastructure::repository::OutputRepository` | — |
//! | StepSummaryWriter | `domain::types::StepSummary` | `application::service::StepSummaryWritingService` | `infrastructure::repository::SummaryRepository` | — |
//! | OutputVariableWriter | `domain::types::OutputVariable` | `application::service::OutputVariableService` | — | — |
//! | PrCommentWriter | `domain::types::PrComment` | `application::service::PrCommentService` | `infrastructure::repository::GitHubClient` | — |
//!
//! # Layer Structure
//!
//! ```text
//! action_output/
//! ├── mod.rs                          # Module root
//! ├── domain/                         # Domain entities and interfaces
//! │   ├── mod.rs
//! │   ├── types.rs                    # FormattedOutput, WorkflowAnnotation, StepSummary, OutputVariable, PrComment
//! │   ├── error.rs                    # ActionOutputError
//! │   └── event/
//! │       └── mod.rs                  # ActionOutputEvent payloads
//! ├── application/                    # Application service interfaces and DTOs
//! │   ├── mod.rs
//! │   ├── service.rs                  # Service traits (OutputFormattingService, AnnotationWritingService, etc.)
//! │   ├── dto/
//! │   │   └── mod.rs                  # Input/output DTO schemas
//! │   └── factory.rs                  # Factory interfaces
//! ├── infrastructure/                 # Infrastructure layer
//! │   ├── mod.rs
//! │   └── repository/
//! │       └── mod.rs                  # Repository interfaces (OutputRepository, SummaryRepository, GitHubClient)
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
//! See `actions/.pi/architecture/modules/action-output.md` for the canonical spec.
//!
//! # Related Issues
//!
//! - Issue #577: Contract Freeze (this issue)
//! - Issue #576: Epic "action-output"

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
