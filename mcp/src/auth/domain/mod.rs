//! Domain layer for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#domain
//! Implements: Contract Freeze — IdpConfig, TokenStatus, DeviceFlowState,
//! ClaimSummary, domain events, error types
//!
//! This module defines the core domain types for Auth:
//! - Value objects: `IdpConfig`, `Secret`, `ClaimSummary`
//! - Status enums: `TokenStatus`, `DeviceFlowStatus`
//! - Device flow state: `DeviceFlowState`
//! - Domain events: `AuthEvent`
//! - Error types: `AuthError`, `SseAuthError`
//!
//! These are pure domain types with no framework dependencies. They serve as
//! the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//!
//! - No implementation logic beyond constructors and field accessors
//! - All types are serializable (Serialize + Deserialize)
//! - Value objects are immutable (no pub fields, no setters)
//! - Secrets are redacted in Debug/Display/Serialize (SpanPrivacy pattern)
//! - Domain events carry session_id and timestamp for correlation
//! - Refresh-token material appears only inside `Secret` wrappers

pub mod config;
pub mod error;
pub mod event;
pub mod flow;
pub mod status;
pub mod value;

pub use config::IdpConfig;
pub use error::{AuthError, DeviceFlowPollError, SseAuthError};
pub use event::AuthEvent;
pub use flow::{DeviceFlowState, DeviceFlowStatus};
pub use status::TokenStatus;
pub use value::{ClaimSummary, Secret};
