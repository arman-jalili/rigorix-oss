//! Action Input — GitHub Action input parsing bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md
//! Implements: Contract Freeze — all component interfaces for action-input epic
//! Issue: issue-contract-freeze
//!
//! This module parses GitHub Actions environment variables, event payloads,
//! and workflow inputs into typed Rust structs that the engine can consume.
//! It handles the impedance mismatch between GitHub's string-based `INPUT_*`
//! environment variables and the engine's structured types.
//!
//! # Components
//!
//! | Component | Domain | Application | Infrastructure | Interfaces |
//! |-----------|--------|-------------|----------------|------------|
//! | ActionInputs | `domain::types::ActionInputs` | `application::dto` | — | `interfaces::http` |
//! | InputParser | — | `application::service::InputParsingService` | `infrastructure::repository::InputRepository` | — |
//! | EventPayloadParser | `domain::types::GitHubEvent` | `application::service::EventParsingService` | `infrastructure::repository::EventRepository` | — |
//! | CommentParser | `domain::types::CommentCommand` | `application::service::CommentParsingService` | — | — |
//! | CiDetector | `domain::types::CiEnvironment` | `application::service::CiDetectionService` | — | — |
//! | ConfigLoader | `domain::types::ActionConfig` | `application::service::ConfigLoadingService` | `infrastructure::repository::ConfigRepository` | — |
//!
//! # Layer Structure
//!
//! ```text
//! action_input/
//! ├── mod.rs                          # Module root
//! ├── domain/                         # Domain entities and interfaces
//! │   ├── mod.rs
//! │   ├── types.rs                    # ActionInputs, ActionConfig, CommentCommand, CiEnvironment, GitHubEvent
//! │   ├── error.rs                    # ActionInputError
//! │   └── event/
//! │       └── mod.rs                  # ActionInputEvent payloads
//! ├── application/                    # Application service interfaces and DTOs
//! │   ├── mod.rs
//! │   ├── service.rs                  # Service traits (InputParsingService, CommentParsingService, etc.)
//! │   ├── dto/
//! │   │   └── mod.rs                  # Input/output DTO schemas
//! │   └── factory.rs                  # Factory interfaces
//! ├── infrastructure/                 # Infrastructure layer
//! │   ├── mod.rs
//! │   └── repository/
//! │       └── mod.rs                  # Repository interfaces (InputRepository, ConfigRepository, EventRepository)
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
//! See `actions/.pi/architecture/modules/action-input.md` for the canonical spec.
//!
//! # Related Issues
//!
//! - Issue #521: Contract Freeze (this issue)
//! - Issue #520: Epic "action-input"

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
