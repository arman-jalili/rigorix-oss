//! ShutdownSignal value object.
//!
//! @canonical .pi/architecture/modules/cancellation.md#signal
//! Implements: Contract Freeze — ShutdownSignal enum
//! Issue: issue-shutdownsignal
//!
//! Defines two shutdown levels for execution cancellation:
//! - `Graceful`: let running tasks finish, don't start new ones
//! - `Immediate`: abort all in-flight work immediately
//!
//! # Contract (Frozen)
//! - `ShutdownSignal` is a simple enum with two variants
//! - Used by `CancellationManager.request_shutdown()`
//! - Propagated via `tokio::sync::watch` channel to subscribers
//! - Implementations MUST NOT introduce new variants without architecture review
//! - Helper methods (`is_graceful`, `is_immediate`, `description`) are additive
//!   and do not change the contract

use serde::{Deserialize, Serialize};

/// Shutdown signal levels for execution cancellation.
///
/// Determines how aggressively running work is terminated when cancellation
/// is requested. The orchestrator and all concurrent tasks must respect this
/// signal level.
///
/// # Usage
/// - `Graceful`: Stop accepting new work, let in-flight tasks complete naturally
/// - `Immediate`: Abort all in-flight work via `JoinSet::abort()` or equivalent
///
/// # Cancellation Response Time (NFR-007)
/// The system SHALL support cancellation within 200ms of signal receipt,
/// regardless of which shutdown level is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShutdownSignal {
    /// Let running tasks finish naturally. No new tasks are started.
    /// Resources (file handles, network connections, etc.) are cleaned up
    /// by the running tasks themselves.
    Graceful,

    /// Abort all in-flight work immediately using task abort mechanisms
    /// (e.g., `JoinSet::abort()`). In-flight work may leave resources in
    /// an inconsistent state; cleanup handlers MUST be registered.
    Immediate,
}

impl ShutdownSignal {
    /// Returns `true` if this is a graceful shutdown signal.
    ///
    /// Graceful signals allow running tasks to finish naturally.
    /// No new tasks should be started.
    pub fn is_graceful(&self) -> bool {
        matches!(self, ShutdownSignal::Graceful)
    }

    /// Returns `true` if this is an immediate shutdown signal.
    ///
    /// Immediate signals abort all in-flight work.
    /// Cleanup handlers should be registered to handle resource cleanup.
    pub fn is_immediate(&self) -> bool {
        matches!(self, ShutdownSignal::Immediate)
    }

    /// Human-readable description of this shutdown signal level.
    pub fn description(&self) -> &'static str {
        match self {
            ShutdownSignal::Graceful => {
                "Let running tasks finish naturally. No new tasks started."
            }
            ShutdownSignal::Immediate => {
                "Abort all in-flight work immediately. Cleanup handlers must run."
            }
        }
    }

    /// All possible shutdown signal variants.
    pub const fn all() -> [ShutdownSignal; 2] {
        [ShutdownSignal::Graceful, ShutdownSignal::Immediate]
    }
}

impl std::fmt::Display for ShutdownSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShutdownSignal::Graceful => write!(f, "Graceful"),
            ShutdownSignal::Immediate => write!(f, "Immediate"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_format() {
        assert_eq!(format!("{:?}", ShutdownSignal::Graceful), "Graceful");
        assert_eq!(format!("{:?}", ShutdownSignal::Immediate), "Immediate");
    }

    #[test]
    fn test_display_format() {
        assert_eq!(format!("{}", ShutdownSignal::Graceful), "Graceful");
        assert_eq!(format!("{}", ShutdownSignal::Immediate), "Immediate");
    }

    #[test]
    fn test_is_graceful() {
        assert!(ShutdownSignal::Graceful.is_graceful());
        assert!(!ShutdownSignal::Immediate.is_graceful());
    }

    #[test]
    fn test_is_immediate() {
        assert!(!ShutdownSignal::Graceful.is_immediate());
        assert!(ShutdownSignal::Immediate.is_immediate());
    }

    #[test]
    fn test_description() {
        assert!(ShutdownSignal::Graceful.description().contains("finish"));
        assert!(ShutdownSignal::Immediate.description().contains("Abort"));
    }

    #[test]
    fn test_partial_eq() {
        assert_eq!(ShutdownSignal::Graceful, ShutdownSignal::Graceful);
        assert_eq!(ShutdownSignal::Immediate, ShutdownSignal::Immediate);
        assert_ne!(ShutdownSignal::Graceful, ShutdownSignal::Immediate);
    }

    #[test]
    fn test_clone() {
        let signal = ShutdownSignal::Graceful;
        let cloned = signal;
        assert_eq!(signal, cloned);
    }

    #[test]
    fn test_copy() {
        let signal = ShutdownSignal::Immediate;
        let copied = signal;
        let also_copied = signal; // Would fail if not Copy
        assert_eq!(copied, also_copied);
    }

    #[test]
    fn test_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher1 = DefaultHasher::new();
        ShutdownSignal::Graceful.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        ShutdownSignal::Graceful.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_serde_roundtrip() {
        let graceful_json = serde_json::to_string(&ShutdownSignal::Graceful).unwrap();
        assert_eq!(graceful_json, "\"Graceful\"");

        let deserialized: ShutdownSignal = serde_json::from_str(&graceful_json).unwrap();
        assert_eq!(deserialized, ShutdownSignal::Graceful);

        let immediate_json = serde_json::to_string(&ShutdownSignal::Immediate).unwrap();
        assert_eq!(immediate_json, "\"Immediate\"");

        let deserialized: ShutdownSignal = serde_json::from_str(&immediate_json).unwrap();
        assert_eq!(deserialized, ShutdownSignal::Immediate);
    }

    #[test]
    fn test_serde_unknown_variant_fails() {
        let result: Result<ShutdownSignal, _> = serde_json::from_str("\"Unknown\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_all_contains_both() {
        let all = ShutdownSignal::all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&ShutdownSignal::Graceful));
        assert!(all.contains(&ShutdownSignal::Immediate));
    }
}
