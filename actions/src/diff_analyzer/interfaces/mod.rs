//! External interfaces for the Diff Analyzer bounded context.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: Contract Freeze — HTTP API contracts and interface definitions
//! Issue: issue-contract-freeze
//!
//! This module defines the external API contracts for the Diff Analyzer.
//! Currently supports HTTP endpoints for debugging, testing, and introspection.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

pub mod http;

pub use http::*;
