//! Config loading interface — re-exported from the application layer.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! The `CliConfigLoader` trait is defined in `application/service.rs`
//! (its canonical Clean Architecture location). This module re-exports it
//! for backward compatibility with existing imports.
//!
//! # Contract (Frozen)
//! - `load()` returns the fully merged `CliConfig`
//! - Merge order: CLI flags override env vars, which override file config,
//!   which override engine defaults
//! - Missing non-critical values use sensible defaults
//! - Missing critical values (e.g., API key) return `MissingConfig` error
//!
//! # Migration
//! New code should import directly from
//! `crate::configuration::application::CliConfigLoader`.
//! This re-export will be removed in a future update.

pub use crate::configuration::application::service::CliConfigLoader;
