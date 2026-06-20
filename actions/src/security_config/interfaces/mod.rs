//! Interfaces layer for the Security Configuration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#interfaces
//! Implements: Contract Freeze — HTTP API contracts
//! Issue: issue-contract-freeze
//!
//! This module defines the external-facing API contracts for the
//! Security Configuration module. HTTP endpoints are framework-agnostic
//! contracts that any HTTP server implementation must satisfy.
//!
//! # Contract (Frozen)
//! - All endpoint paths, methods, requests, and responses are documented
//! - Error responses follow a unified format
//! - No framework-specific annotations

pub mod http;

pub use http::*;
