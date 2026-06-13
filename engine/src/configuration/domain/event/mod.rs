//! Event payload schemas for the Configuration bounded context.
//!
//! These events are emitted on the `EventBus` whenever configuration is
//! loaded, changed, or validated. Consumers (audit, console printer, TUI)
//! subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `sequence` is populated by EventBus at emission time

use serde::{Deserialize, Serialize};

/// Events emitted by the Configuration module.
///
/// Wrapped in `ExecutionEvent::Configuration(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigurationEvent {
    /// Configuration was successfully loaded from all sources.
    ConfigLoaded {
        /// Human-readable description of which sources contributed.
        sources_used: Vec<String>,
        /// Validation outcome.
        valid: bool,
    },

    /// Configuration failed to load from a source, but a fallback was used.
    ConfigFallback {
        /// The source that failed.
        failed_source: String,
        /// Which source was used as fallback.
        fallback_source: String,
        /// Error detail for diagnostics.
        error: String,
    },

    /// Configuration validation failed — system cannot start.
    ConfigValidationFailed {
        /// List of validation errors.
        errors: Vec<String>,
    },

    /// A Secret value was accessed (for audit logging).
    SecretAccessed {
        /// Identifier for which secret was accessed (not the value).
        key_hint: String,
        /// Context describing why it was accessed.
        context: String,
    },

    /// A new Secret was loaded (from env var).
    SecretLoaded {
        /// Hint identifying the secret source.
        source_hint: String,
    },
}
