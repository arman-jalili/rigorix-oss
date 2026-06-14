//! Interfaces layer for the Tool System bounded context.
//!
//! @canonical .pi/architecture/modules/tool-system.md#interfaces
//! Implements: Contract Freeze — HTTP API contracts
//! Issue: #124
//!
//! This module defines external interface contracts:
//! - HTTP API endpoints, request/response schemas, error formats
//!
//! # Contract (Frozen)
//! - Framework-agnostic endpoint contracts
//! - Unified error response format
//! - No implementation logic

pub mod http;
