//! External interfaces for the Action Output bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — HTTP API contracts for output endpoints
//! Issue: issue-contract-freeze
//!
//! This module defines the external-facing contracts for the action-output
//! module, including HTTP API endpoints and request/response schemas.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

pub mod http;

pub use http::*;
