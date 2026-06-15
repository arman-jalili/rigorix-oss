//! Execution bounded context.
//!
//! @canonical .pi/architecture/modules/error-handling.md#execution
//! Implements: Contract Freeze — ExecutionError
//! Issue: #186
//!
//! This module handles task execution lifecycle, including execution
//! errors, timeouts, and fallback handling. It follows Clean Architecture:
//!
//! - `domain/` — ExecutionError and related domain types
//! - `application/` — Execution service interfaces and DTOs
//! - `infrastructure/` — Execution infrastructure (planned)
//! - `interfaces/` — API contracts (planned)

pub mod domain;
