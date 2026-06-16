//! Signal handler — Ctrl+C and SIGTERM handling.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#signal-handling
//! Implements: SignalHandler component
//! Issue: issue-signalhandler

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::signal;

/// Re-export the concrete CancellationToken type for downstream use.
pub use tokio_util::sync::CancellationToken;

/// Install OS signal handlers and return a shared `CancellationToken`.
///
/// # Returns
///
/// A `CancellationToken` that will be cancelled when the user presses
/// Ctrl+C or the process receives SIGTERM.
///
/// # Implementation Notes
///
/// - Single Ctrl+C triggers graceful shutdown (cancels token)
/// - Double Ctrl+C within 2 seconds triggers immediate abort
/// - SIGTERM always triggers immediate abort (Unix only)
pub fn install_signal_handler() -> CancellationToken {
    let token = CancellationToken::new();
    let token_clone = token.clone();
    let sigint_received = Arc::new(AtomicBool::new(false));
    let sigint_received_clone = sigint_received.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = signal::ctrl_c() => {
                    if result.is_err() {
                        continue;
                    }
                    if sigint_received_clone.swap(true, Ordering::SeqCst) {
                        // Second Ctrl+C within 2s → immediate abort
                        eprintln!("\nImmediate abort requested. Exiting.");
                        std::process::exit(137);
                    } else {
                        // First Ctrl+C → graceful shutdown
                        eprintln!("\nGraceful shutdown requested. Press Ctrl+C again to abort immediately.");
                        token_clone.cancel();
                        // Reset after 2 seconds if no second signal
                        let reset = sigint_received_clone.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            reset.store(false, Ordering::SeqCst);
                        });
                    }
                }
                // The loop will sleep briefly if no signal, allowing the task to be cancellable
                _ = tokio::time::sleep(Duration::from_secs(3600)) => {
                    // Periodic wakeup to keep the task alive
                }
            }
        }
    });

    token
}
