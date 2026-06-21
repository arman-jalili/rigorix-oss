//! CI Integration — GitHub-specific CI/CD primitives.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md
//! Implements: Contract Freeze — all component interfaces for ci-integration epic
//! Issue: issue-contract-freeze
//!
//! Bridges engine execution with GitHub's CI/CD primitives: commit status checks,
//! PR review comments, issue labels, and workflow orchestration. The engine doesn't
//! know about GitHub — this module is the adapter that maps GitHub concepts to
//! engine execution outcomes.
//!
//! # Components
//!
//! | Component | Domain | Application | Infrastructure | Interfaces |
//! |-----------|--------|-------------|----------------|------------|
//! | StatusCheckManager | `domain::types::StatusCheckState` | `application::service::StatusCheckService` | `infrastructure::repository::StatusCheckRepository` | `interfaces::http` |
//! | PrCommentManager | `domain::types::PrComment` | `application::service::PrCommentService` | `infrastructure::repository::PrCommentRepository` | — |
//!
//! # Layer Structure
//!
//! ```text
//! ci_integration/
//! ├── mod.rs                          # Module root
//! ├── domain/                         # Domain entities and interfaces
//! │   ├── mod.rs
//! │   ├── types.rs                    # StatusCheckState, PrComment, etc.
//! │   ├── error.rs                    # CiIntegrationError
//! │   └── event/
//! │       └── mod.rs                  # CiIntegrationEvent payloads
//! ├── application/                    # Application service interfaces and DTOs
//! │   ├── mod.rs
//! │   ├── service.rs                  # Service traits (StatusCheckService, PrCommentService)
//! │   ├── dto/
//! │   │   └── mod.rs                  # Input/output DTO schemas
//! │   └── factory.rs                  # Factory interfaces
//! ├── infrastructure/                 # Infrastructure layer
//! │   ├── mod.rs
//! │   └── repository/
//! │       └── mod.rs                  # Repository interfaces
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
//! See `actions/.pi/architecture/modules/ci-integration.md` for the canonical spec.
//!
//! # Related Issues
//!
//! - Issue #590: Contract Freeze (this issue)
//! - Issue #589: Epic "ci-integration"

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
