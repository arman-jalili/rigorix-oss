//! Infrastructure module root for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — infrastructure module
//! Issue: issue-contract-freeze
//!
//! Interfaces for I/O concerns: config loading, output formatting,
//! signal handling, and (future) CLI state persistence.

pub mod config;
pub mod config_impl;
pub mod observability;
pub mod output;
pub mod output_impl;
pub mod repository;
pub mod signal;
pub mod signal_impl;
