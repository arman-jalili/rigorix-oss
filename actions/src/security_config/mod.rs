//! Security Configuration — pre-flight security validation bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md
//! Implements: Contract Freeze — all component interfaces for security-config epic
//! Issue: issue-contract-freeze
//!
//! This module enforces operational security for the GitHub Action. It validates
//! the execution environment before any operation begins — detecting fork PRs
//! (to prevent secret exposure), masking sensitive values from logs, validating
//! token permissions, and verifying that policies haven't been tampered with.
//!
//! This is a **Phase 0** module — it runs before any diff analysis, policy
//! evaluation, or engine execution.
//!
//! # Components
//!
//! | Component | Domain | Application | Infrastructure | Interfaces |
//! |-----------|--------|-------------|----------------|------------|
//! | SecurityContext | `domain::types::SecurityContext` | `application::dto` | — | `interfaces::http` |
//! | SecurityValidator | — | `application::service::SecurityValidationService` | — | — |
//! | ForkDetector | `domain::types` | `application::service::ForkDetectionService` | `infrastructure::repository::ForkRepository` | — |
//! | SecretMasker | — | `application::service::SecretMaskingService` | — | — |
//! | TokenValidator | — | `application::service::TokenValidationService` | `infrastructure::repository::TokenRepository` | — |
//! | UrlAllowlist | — | `application::service::UrlAllowlistService` | `infrastructure::repository::AllowlistRepository` | — |
//! | HmacSigner | `domain::types::HmacKey` | `application::service::HmacSigningService` | `infrastructure::repository::HmacKeyRepository` | — |
//! | OrgPolicyLoader | `domain::types::SecurityPolicy` | `application::service::PolicyLoadingService` | `infrastructure::repository::PolicyRepository` | — |
//!
//! # Layer Structure
//!
//! ```text
//! security_config/
//! ├── mod.rs                          # Module root
//! ├── domain/                         # Domain entities and interfaces
//! │   ├── mod.rs
//! │   ├── types.rs                    # SecurityContext, SecurityLevel, ActionMode, HmacKey, SecurityPolicy
//! │   ├── error.rs                    # SecurityError
//! │   └── event/
//! │       └── mod.rs                  # SecurityEvent payloads
//! ├── application/                    # Application service interfaces and DTOs
//! │   ├── mod.rs
//! │   ├── service.rs                  # Service traits (SecurityValidationService, etc.)
//! │   ├── dto/
//! │   │   └── mod.rs                  # Input/output DTO schemas
//! │   └── factory.rs                  # Factory interfaces
//! ├── infrastructure/                 # Infrastructure layer
//! │   ├── mod.rs
//! │   └── repository/
//! │       └── mod.rs                  # Repository interfaces (ForkRepository, TokenRepository, etc.)
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
//! See `actions/.pi/architecture/modules/security-config.md` for the canonical spec.
//!
//! # Related Issues
//!
//! - Issue #538: Contract Freeze (this issue)
//! - Issue #537: Epic "security-config"

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
