//! CLI boundary module — command dispatch, output, TUI, and shared types.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Per ADR-002 (CLI/engine split), this module contains all CLI-specific
//! types that don't belong to a specific domain module (configuration,
//! observability, cancellation).

pub mod application;
pub mod commands;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub mod tests;

pub mod tui;
