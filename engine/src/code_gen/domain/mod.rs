//! Domain entities for the Code Generation Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/code-generation.md#domain
//! Implements: Contract Freeze — SyntaxGateResult, SyntaxError, CodeGenError
//! Issue: #424
//!
//! This module defines the core domain types:
//! - `CodeGenError` — Typed error enum for code generation failures
//! - `CodeGenEvent` — Event payload schemas for code generation lifecycle
//! - `SyntaxGateResult` — Outcome of post-edit tree-sitter syntax verification
//! - `SyntaxError` — Individual syntax error with location context
//!
//! These are pure domain objects with no framework dependencies.
//! They serve as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod error;
pub mod event;
pub mod result;

pub use error::CodeGenError;
pub use event::CodeGenEvent;
pub use result::{SyntaxError, SyntaxGateResult};
