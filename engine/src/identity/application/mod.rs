//! Application layer interfaces for the Identity Attestation bounded context.
//!
//! @canonical .pi/architecture/modules/identity.md#application
//! Implements: Contract Freeze — service traits, DTOs
//! Issue: #700 (identity epic — contract freeze)
//!
//! This module defines:
//! - The `IdentityAttestationService` use-case trait
//! - The `VerificationOutcome` marker enum
//! - Input/Output DTOs (`AttestInput`, `AttestOutput`)
//!
//! # Contract (Frozen)
//! - `attest` and `verify` are async (use `async-trait` for trait-object safety)
//! - `extract_claims` is synchronous — it only decodes claims, never verifies
//! - All public methods return domain types (`IdentityClaim`, `IdentityError`)
//! - Verification is best-effort; degradation is explicit, never silent
//! - No implementation logic — only contract signatures (stubs land in
//!   ISSUE-IDENTITY-3)

pub mod dto;
pub mod service;

pub use dto::{AttestInput, AttestOutput};
pub use service::{IdentityAttestationService, VerificationOutcome};
