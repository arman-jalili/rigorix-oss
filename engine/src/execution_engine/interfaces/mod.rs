//! Interfaces layer for the Execution Engine bounded context.
//!
//! @canonical .pi/architecture/modules/execution-engine.md#interfaces
//! Implements: Contract Freeze — HTTP endpoint contracts
//! Issue: issue-contract-freeze
//!
//! This module contains the API contracts for external communication
//! with the execution engine (HTTP endpoints, event subscriptions).
//!
//! # Contract (Frozen)
//! - All endpoint contracts defined with method, path, request/response types
//! - Framework-agnostic — implementation details (axum/actix) added later

pub mod http;
