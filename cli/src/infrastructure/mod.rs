//! Infrastructure module root for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — infrastructure module
//! Issue: issue-contract-freeze
//!
//! Interfaces for I/O concerns: config loading, output formatting,
//! signal handling, and (future) CLI state persistence.

pub mod config;
pub mod output;
pub mod repository;
pub mod signal;
