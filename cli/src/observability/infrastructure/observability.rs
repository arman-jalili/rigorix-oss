//! Observability infrastructure interfaces — re-exported from application layer.
//!
//! @canonical .pi/architecture/modules/observability.md
//! The `TracingInitializer` trait is defined in `application/service.rs`
//! (its canonical Clean Architecture location). This module re-exports it
//! for backward compatibility with existing imports.
//!
//! # Contract (Frozen)
//! - `TracingInitializer` configures and initializes the tracing subscriber
//! - Accepts log level and format from CLI config
//! - Respects `RIGORIX_LOG` env var override
//! - Implementations must be idempotent (calling twice is a no-op after first)
//!
//! # Migration
//! New code should import directly from
//! `crate::observability::application::TracingInitializer`.
//! This re-export will be removed in a future update.

pub use crate::observability::application::service::TracingInitializer;
