//! Audit Posting — HMAC-signed audit records bounded context.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md
//! Implements: Contract Freeze — all component interfaces for audit-posting epic
//! Issue: issue-contract-freeze
//!
//! This module posts HMAC-signed audit records to a remote audit backend from
//! within GitHub Actions. It sits on top of the engine's audit envelope system,
//! providing the Actions-specific wrapper for audit record delivery.
//!
//! # Components
//!
//! | Component | Domain | Application | Infrastructure | Interfaces |
//! |-----------|--------|-------------|----------------|------------|
//! | SignedAuditRecord | `domain::SignedAuditRecord` | `application::dto` | — | `interfaces::http` |
//! | AuditBackend | — | — | `infrastructure::repository::AuditBackend` | — |
//! | FilesystemAuditBackend | — | — | `infrastructure::repository::FilesystemAuditBackend` | — |
//! | AuditPoster | — | `application::service::AuditPostingService` | — | — |
//! | AuditRecordQueue | — | `application::service::AuditRecordQueue` | — | — |
//!
//! # Layer Structure
//!
//! ```text
//! audit_posting/
//! ├── mod.rs                          # Module root
//! ├── domain/                         # Domain entities and interfaces
//! │   ├── mod.rs
//! │   ├── signed_audit_record.rs      # SignedAuditRecord value object
//! │   ├── error.rs                    # AuditPostingError
//! │   └── event/
//! │       └── mod.rs                  # AuditPostingEvent payloads
//! ├── application/                    # Application service interfaces and DTOs
//! │   ├── mod.rs
//! │   ├── service.rs                  # Service traits (AuditPostingService, AuditRecordQueue)
//! │   ├── dto/
//! │   │   └── mod.rs                  # Input/output DTO schemas
//! │   └── factory.rs                  # Factory interfaces
//! ├── infrastructure/                 # Infrastructure layer
//! │   ├── mod.rs
//! │   └── repository/
//! │       └── mod.rs                  # Repository interfaces (AuditBackend, FilesystemAuditBackend)
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
//! See `actions/.pi/architecture/modules/audit-posting.md` for the canonical spec.
//!
//! # Related Issues
//!
//! - Issue #600: Contract Freeze (this issue)
//! - Issue #599: Epic "audit-posting"

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
