//! Identity Attestation bounded context.
//!
//! @canonical .pi/architecture/modules/identity.md
//! Implements: Contract Freeze — identity module root
//! Issue: #700 (identity epic — contract freeze)
//!
//! Makes the human behind a run or an approval a **first-class, recorded fact**.
//! The shared `IdentityClaim` value type and the attestation service convert a
//! presented IdP token (or local principal) into a structured, time-bound
//! identity claim that flows into runs (`author`), approvals (`approver_id`),
//! and the signed audit envelope (redacted `identity` block).
//!
//! # The Seam (ADR-012)
//!
//! > **OSS attests; Enterprise authorizes.** OSS records who was presented as
//! > acting (a captured, attributed fact). Enterprise evaluates whether that
//! > identity may act (scope/RBAC enforcement, policy, JWKS verification). OSS
//! > never makes authorization judgments.
//!
//! # Architecture
//!
//! ```text
//! identity/
//! ├── domain/             # Domain entities: IdentityClaim, IdentitySource,
//! │   │                     Authority, IdentityError
//! │   ├── claim.rs        # IdentityClaim value object + IdentitySource enum
//! │   ├── authority.rs    # Authority value object (role / policy id)
//! │   └── error.rs        # IdentityError enum (thiserror)
//! ├── application/        # Service traits and DTOs
//! │   ├── service.rs      # IdentityAttestationService trait + VerificationOutcome
//! │   └── dto/            # AttestInput / AttestOutput DTO schemas
//! └── infrastructure/     # Repository and verifier interfaces
//!     ├── verifier.rs     # TokenVerifier trait + NullVerifier (offline default)
//!     └── repository/     # IdentityRepository trait
//! ```
//!
//! **Note:** This module has **no `interfaces/` layer** — the client-side IdP
//! flow (device authorization, keychain custody, MCP tools) lives in the MCP
//! crate's `auth` module. The engine module is the shared contract + attestation
//! core; it is consumed via service trait by `orchestrator`, `execution_engine`,
//! `approval`, and `audit`.
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//! - Domain method bodies are `todo!()` stubs — behavior lands in the
//!   implementation issues (ISSUE-IDENTITY-1 … 6)
//!
//! # Related Components
//!
//! - `audit` — envelope `identity` block carries a redacted `IdentityRef`
//! - `orchestrator` — `RunInput.author` / `RunInput.identity` (claim supersedes
//!   the self-asserted string when present)
//! - `approval` — `ApproverInput.approver_id` + `token_claims_ref` binding
//! - MCP `auth` module — client-side OIDC device flow (separate crate)

pub mod application;
pub mod domain;
pub mod infrastructure;

pub use application::*;
pub use domain::*;
pub use infrastructure::*;
