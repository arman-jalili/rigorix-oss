//! Signal handler — Ctrl+C and SIGTERM handling.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#signal-handling
//! Implements: Contract Freeze — SignalHandler component
//! Issue: issue-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Signal handling follows a two-level cancellation protocol:
//!
//! | Action | Behaviour |
//! |--------|-----------|
//! | Single Ctrl+C | Graceful shutdown — finish in-flight node, exit 130 |
//! | Double Ctrl+C within 2s | Immediate abort — cancel all in-flight nodes, exit 137 |
//! | SIGTERM (Unix) | Immediate abort — exit 137 |
//!
//! The signal handler installs at startup and shares a `CancellationToken`
//! with the orchestrator. The same token is used by both `cli_boundary`
//! and `tui` modules.

/// Re-export the concrete CancellationToken type for downstream use.
pub use tokio_util::sync::CancellationToken;

/// Install OS signal handlers and return a shared `CancellationToken`.
///
/// # Returns
///
/// A `CancellationToken` that will be cancelled when the user presses
/// Ctrl+C or the process receives SIGTERM. This token is shared with
/// the orchestrator for cooperative cancellation.
///
/// # Implementation Notes
///
/// - Single Ctrl+C triggers graceful shutdown (cancels token)
/// - Double Ctrl+C within 2 seconds triggers immediate abort
/// - SIGTERM always triggers immediate abort
/// - On Windows, only Ctrl+C handling is available (no SIGTERM)
pub fn install_signal_handler() -> CancellationToken {
    // Placeholder: returns a new, uncancelled token.
    // Implementation issue: install crossterm or tokio signal handler,
    // implement two-level cancellation protocol, return the shared token.
    CancellationToken::new()
}
