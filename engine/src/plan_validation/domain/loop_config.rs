//! ValidationLoopConfig — configuration for the plan validation retry loop.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#config
//! Implements: Contract Freeze — ValidationLoopConfig
//! Issue: issue-contract-freeze
//!
//! Defines the configuration for the self-correcting validation loop.
//! Controls maximum iterations, required quality level, token budget,
//! and caching behaviour.
//!
//! # Contract (Frozen)
//! - All fields are public for direct construction
//! - Sensible defaults provided via `Default` impl
//! - Serialization support for configuration files and API transport
//! - No implementation logic beyond field accessors

use serde::{Deserialize, Serialize};

use crate::quality_gates::domain::QualityLevel;

/// Configuration for the plan validation retry loop.
///
/// Controls how many iterations the validation loop will attempt,
/// what quality level is required for success, and budget constraints
/// across all retries.
///
/// # Defaults
///
/// | Field | Default | Rationale |
/// |-------|---------|-----------|
/// | max_iterations | 3 | One initial attempt + two retries with augmented context |
/// | required_quality | Package | At least crate-level tests must pass |
/// | max_cumulative_tokens | 50_000 | Budget for 3 iterations of typical LLM generation |
/// | cache_successful_templates | true | Production-grade templates are reusable assets |
///
/// # Contract (Frozen)
/// - `max_iterations` must be >= 1 (minimum one attempt)
/// - `required_quality` defaults to `Package` for general use
/// - `max_cumulative_tokens` prevents budget exhaustion across retries
/// - `cache_successful_templates` enables template caching on success
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationLoopConfig {
    /// Maximum validation iterations (1 initial + N retries).
    ///
    /// Default: 3 — one initial attempt + up to two retries with
    /// augmented context from failure analysis.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// Quality level required for validation to be considered successful.
    ///
    /// Default: `Package` — at least the affected crate/package tests
    /// must pass. Set to `Workspace` for stricter validation (cross-crate
    /// impact detection) or `MergeReady` for full CI gating.
    #[serde(default = "default_quality_level")]
    pub required_quality: QualityLevel,

    /// Maximum cumulative LLM tokens across all validation iterations.
    ///
    /// Default: 50,000 — sufficient for 3 iterations of typical
    /// template generation with Claude-class models.
    #[serde(default = "default_max_tokens")]
    pub max_cumulative_tokens: u64,

    /// Whether to cache validated templates for replay.
    ///
    /// When true, successfully validated templates are cached by the
    /// `GeneratedTemplateRepository` so that subsequent identical or
    /// similar intents can reuse the validated prompt without re-running
    /// the full validation loop.
    #[serde(default = "default_cache")]
    pub cache_successful_templates: bool,
}

fn default_max_iterations() -> u32 {
    3
}

fn default_quality_level() -> QualityLevel {
    QualityLevel::Package
}

fn default_max_tokens() -> u64 {
    50_000
}

fn default_cache() -> bool {
    true
}

impl Default for ValidationLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            required_quality: default_quality_level(),
            max_cumulative_tokens: default_max_tokens(),
            cache_successful_templates: default_cache(),
        }
    }
}

impl ValidationLoopConfig {
    /// Create a new validation loop configuration with the specified max iterations.
    ///
    /// All other fields use their defaults (Package quality, 50K tokens, caching enabled).
    ///
    /// # Panics
    ///
    /// Panics if `max_iterations` is 0 (at least one attempt is required).
    pub fn new(max_iterations: u32) -> Self {
        assert!(
            max_iterations >= 1,
            "ValidationLoopConfig: max_iterations must be >= 1"
        );
        Self {
            max_iterations,
            ..Default::default()
        }
    }

    /// Returns `true` if retries are still available for the given iteration number.
    ///
    /// # Arguments
    ///
    /// * `current_iteration` — The current 1-indexed iteration number.
    pub fn can_retry(&self, current_iteration: u32) -> bool {
        current_iteration < self.max_iterations
    }

    /// Returns the maximum number of retry attempts (iterations - 1).
    pub fn max_retries(&self) -> u32 {
        self.max_iterations.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ValidationLoopConfig::default();
        assert_eq!(config.max_iterations, 3);
        assert_eq!(config.required_quality, QualityLevel::Package);
        assert_eq!(config.max_cumulative_tokens, 50_000);
        assert!(config.cache_successful_templates);
    }

    #[test]
    fn test_new_with_max_iterations() {
        let config = ValidationLoopConfig::new(5);
        assert_eq!(config.max_iterations, 5);
    }

    #[test]
    fn test_can_retry() {
        let config = ValidationLoopConfig::new(3);
        assert!(config.can_retry(1));
        assert!(config.can_retry(2));
        assert!(!config.can_retry(3));
        assert!(!config.can_retry(4));
    }

    #[test]
    fn test_max_retries() {
        let config = ValidationLoopConfig::new(3);
        assert_eq!(config.max_retries(), 2);

        let single = ValidationLoopConfig::new(1);
        assert_eq!(single.max_retries(), 0);
    }

    #[test]
    #[should_panic(expected = "max_iterations must be >= 1")]
    fn test_zero_iterations_panics() {
        ValidationLoopConfig::new(0);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let config = ValidationLoopConfig::new(5);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ValidationLoopConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_iterations, 5);
        assert_eq!(deserialized.max_cumulative_tokens, 50_000);
    }
}
