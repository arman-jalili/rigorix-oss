//! Interfaces layer for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#interfaces
//! Implements: Contract Freeze — MCP tool handler contracts, SSE auth gate
//!
//! This module defines the external interface contracts:
//! - MCP tool handler traits + JSON schemas for `rigorix_auth_login`,
//!   `rigorix_auth_status`, `rigorix_auth_logout`
//! - SSE auth gate contract for optional non-localhost transport auth
//!   (`mcp.sse.auth = "none" | "idp" | "api_key"`, ADR-008/ADR-005)
//!
//! # Contract (Frozen)
//!
//! - Framework-agnostic handler contracts (no axum/axum-middleware types)
//! - Tool outputs are always redacted — never a raw token
//! - Unified error format via `AuthError` (MCP error responses format in the
//!   transport layer)
//! - No implementation logic

pub mod mcp;
pub mod sse_auth;
pub mod sse_auth_impl;

pub use sse_auth_impl::SseAuthGateImpl;
