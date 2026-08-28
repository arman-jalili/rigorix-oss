//! Infrastructure layer interfaces for the Identity Attestation bounded context.
//!
//! @canonical .pi/architecture/modules/identity.md#infrastructure
//! Implements: Contract Freeze — TokenVerifier, NullVerifier, IdentityRepository
//! Issue: #700 (identity epic — contract freeze)
//!
//! This module defines the external-facing seams of the identity module:
//! - `TokenVerifier` trait + `NullVerifier` (offline default, ADR-012 option c)
//! - `IdentityRepository` trait (durable identity records alongside execution state)
//!
//! # Contract (Frozen)
//! - All repository methods are async and return domain error types
//! - Verification is best-effort: unreachable IdP → `Unverified`, never an error
//! - Raw tokens are stored by reference (`token_ref`), never embedded in
//!   serialized records
//! - No framework-specific annotations on trait definitions

pub mod repository;
pub mod verifier;

pub use repository::IdentityRepository;
pub use verifier::{JwksVerifier, NullVerifier, TokenVerifier};
