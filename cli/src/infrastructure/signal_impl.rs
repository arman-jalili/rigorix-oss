//! SignalHandler implementation.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#signal
//! Implements: CLI signal handling — Ctrl+C double-press detection
//! Issue: #237
//!
//! Uses tokio::signal to capture SIGINT (Ctrl+C). Single press sends
//! Graceful shutdown. A second press within 2s sends Immediate shutdown.
//!
//! Per ADR-007, the CLI is ephemeral — signal handling is per-process.

use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::watch;
use tracing::info;

use crate::domain::error::CliError;
use crate::infrastructure::signal::{ShutdownLevel, SignalHandler};

/// Default double-press window in seconds.
const DEFAULT_DOUBLE_PRESS_WINDOW_SECS: u64 = 2;

/// Handles OS signal capture with double-press detection.
pub struct SignalHandlerImpl {
    double_press_window: Duration,
}

impl SignalHandlerImpl {
    /// Create a new signal handler with the default double-press window (2s).
    pub fn new() -> Self {
        Self {
            double_press_window: Duration::from_secs(DEFAULT_DOUBLE_PRESS_WINDOW_SECS),
        }
    }

    /// Create a signal handler with a custom double-press window.
    pub fn with_window(window_secs: u64) -> Self {
        Self {
            double_press_window: Duration::from_secs(window_secs),
        }
    }
}

impl Default for SignalHandlerImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SignalHandler for SignalHandlerImpl {
    async fn install(&self) -> Result<watch::Receiver<ShutdownLevel>, CliError> {
        let (tx, rx) = watch::channel(ShutdownLevel::Graceful);

        // Spawn a task to listen for SIGINT
        let window = self.double_press_window;
        tokio::spawn(async move {
            // Wait for first SIGINT
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("SIGINT received — initiating graceful shutdown");
                    let _ = tx.send(ShutdownLevel::Graceful);

                    // Wait for double-press window
                    tokio::select! {
                        _ = tokio::time::sleep(window) => {
                            // No second press — graceful shutdown is already sent
                            info!("Graceful shutdown proceeding (no second SIGINT within window)");
                        }
                        _ = tokio::signal::ctrl_c() => {
                            // Second press within window — immediate abort
                            info!("Second SIGINT received — initiating immediate abort");
                            let _ = tx.send(ShutdownLevel::Immediate);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to install signal handler: {}", e);
                }
            }
        });

        Ok(rx)
    }

    fn double_press_window_secs(&self) -> u64 {
        self.double_press_window.as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_double_press_window() {
        let handler = SignalHandlerImpl::new();
        assert_eq!(handler.double_press_window_secs(), 2);
    }

    #[test]
    fn test_custom_double_press_window() {
        let handler = SignalHandlerImpl::with_window(5);
        assert_eq!(handler.double_press_window_secs(), 5);
    }

    #[test]
    fn test_shutdown_level_debug() {
        assert_eq!(format!("{:?}", ShutdownLevel::Graceful), "Graceful");
        assert_eq!(format!("{:?}", ShutdownLevel::Immediate), "Immediate");
    }

    #[test]
    fn test_shutdown_level_partial_eq() {
        assert_eq!(ShutdownLevel::Graceful, ShutdownLevel::Graceful);
        assert_eq!(ShutdownLevel::Immediate, ShutdownLevel::Immediate);
        assert_ne!(ShutdownLevel::Graceful, ShutdownLevel::Immediate);
    }
}
