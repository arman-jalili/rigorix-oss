//! Interface adapters for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md#interfaces
//! Implements: Contract Freeze — HTTP API contracts and event schemas
//! Issue: issue-contract-freeze
//!
//! This module defines the external-facing contracts for the Code Graph
//! module — HTTP endpoints, request/response schemas, and error formats.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (added by implementation)

pub mod http;
