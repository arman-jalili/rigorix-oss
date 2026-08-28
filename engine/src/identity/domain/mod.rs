//! Domain entities and interfaces for the Identity Attestation bounded context.
//!
//! @canonical .pi/architecture/modules/identity.md#domain
//! Implements: Contract Freeze — domain entities IdentityClaim, IdentitySource,
//!   Authority, IdentityError
//! Issue: #700 (identity epic — contract freeze)
//!
//! This module defines the core domain types — the shared identity claim value
//! object, its source enum, the structured authority field, and all identity
//! error modes. These are pure domain objects with zero framework imports
//! (`thiserror` only).
//!
//! # Contract (Frozen)
//! - `IdentityClaim` is the single shared identity type across orchestrator,
//!   approval, audit, and the MCP auth module
//! - Identity is an **attributed, time-bound claim** — evidence of who was
//!   presented as authorizing, not proof of who the person is
//! - Degradation (`IdentitySource::Unverified`) is explicit, never silent
//! - Raw tokens are referenced (`token_ref`), never embedded in serialized form
//! - No authorization judgment lives in this module — OSS attests, Enterprise
//!   authorizes (ADR-012)

pub mod authority;
pub mod claim;
pub mod error;

pub use authority::Authority;
pub use claim::{IdentityClaim, IdentitySource};
pub use error::IdentityError;
