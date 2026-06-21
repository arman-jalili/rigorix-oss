//! Action Entrypoint — Event routing + dispatch for the Rigorix GitHub Action.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Implements: Contract Freeze — all component interfaces for action-entrypoint epic
//! Issue: issue-contract-freeze
//!
//! This module handles GitHub Action event routing — mapping workflow triggers
//! (`workflow_dispatch`, `issue_comment`, `pull_request`) to engine orchestrator calls.
//! All business logic lives in `rigorix-engine`; this module is a thin dispatch layer.
//!
//! # Components
//!
//! | Component | Domain | Application | Infrastructure | Interfaces |
//! |-----------|--------|-------------|----------------|------------|
//! | ActionRouter | — | `application::service::ActionRouter` | — | — |
//! | ActionContext | `domain::types::ActionContext` | - | `infrastructure::repository::ContextRepository` | — |
//! | ActionMode | `domain::types::ActionMode` | — | — | — |
//! | ActionError | `domain::error::ActionError` | — | — | — |
//!
//! # Layer Structure
//!
//! ```text
//! action_entrypoint/
//! ├── mod.rs                          # Module root
//! ├── domain/                         # Domain entities and interfaces
//! │   ├── mod.rs
//! │   ├── types.rs                    # ActionContext, ActionMode, ActionOutput, GitHubEvent
//! │   ├── error.rs                    # ActionError
//! │   └── event/
//! │       └── mod.rs                  # ActionEntrypointEvent payloads
//! ├── application/                    # Application service interfaces and DTOs
//! │   ├── mod.rs
//! │   ├── service.rs                  # Service traits (ActionRouter, ModeResolver)
//! │   ├── dto/
//! │   │   └── mod.rs                  # Input/output DTO schemas
//! │   └── factory.rs                  # Factory interfaces
//! ├── infrastructure/                 # Infrastructure layer
//! │   ├── mod.rs
//! │   └── repository/
//! │       └── mod.rs                  # Repository interfaces (ContextRepository)
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
//! See `actions/.pi/architecture/modules/action-entrypoint.md` for the canonical spec.
//!
//! # Related Issues
//!
//! - Issue #613: Contract Freeze (this issue)
//! - Issue #612: Epic "action-entrypoint"

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
