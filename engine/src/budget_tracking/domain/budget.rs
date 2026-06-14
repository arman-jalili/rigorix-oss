//! LlmBudget domain entity.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md#budget
//! Implements: Contract Freeze — LlmBudget value object with call/token tracking
//! Issue: #68
//!
//! The root aggregate for LLM budget tracking. Tracks call count and token
//! consumption per execution, enforces hard caps (`max_llm_calls` and
//! `max_llm_tokens`), and provides RAII reservation via `LlmBudgetReservation`.
//!
//! # Contract (Frozen)
//! - `LlmBudget` is the value object for all budget tracking state
//! - All fields are public for direct construction/observation by the application layer
//! - Construction happens via `LlmBudgetFactory`
//! - Runtime state updates (used_calls, used_tokens) are managed by the implementation

use serde::{Deserialize, Serialize};

/// Tracks and enforces LLM usage budgets with hard caps.
///
/// Every LLM call must `reserve()` budget before invocation. The returned
/// `LlmBudgetReservation` auto-rolls back on Drop if not explicitly committed,
/// ensuring no leakage on panic or early return.
///
/// # Presets
///
/// Three built-in presets match the enforcement modes:
/// - `default_mode`  — 5 calls, 10K tokens
/// - `advanced_mode` — 20 calls, 100K tokens
/// - `aggressive_mode` — 50 calls, 500K tokens
///
/// # Thread Safety
///
/// The concrete implementation MUST be `Send + Sync`. Counter mutations
/// (used_calls, used_tokens) are expected to use atomics or interior
/// mutability controlled by the implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBudget {
    /// Maximum number of LLM calls allowed.
    pub max_calls: u32,

    /// Maximum number of LLM tokens allowed (input + output).
    pub max_tokens: u32,

    /// Number of LLM calls used so far.
    pub used_calls: u32,

    /// Number of LLM tokens consumed so far.
    pub used_tokens: u32,

    /// Human-readable label for this budget (e.g. "default", "advanced").
    pub label: String,
}

impl LlmBudget {
    /// Check whether a reservation for `tokens` would exceed the call limit.
    ///
    /// Returns `true` if `used_calls + 1 > max_calls`.
    pub fn would_exceed_calls(&self) -> bool {
        self.used_calls >= self.max_calls
    }

    /// Check whether a reservation for `tokens` would exceed the token limit.
    ///
    /// Returns `true` if `used_tokens + tokens > max_tokens`.
    pub fn would_exceed_tokens(&self, tokens: u32) -> bool {
        self.used_tokens.saturating_add(tokens) > self.max_tokens
    }

    /// Compute remaining call capacity.
    pub fn remaining_calls(&self) -> u32 {
        self.max_calls.saturating_sub(self.used_calls)
    }

    /// Compute remaining token capacity.
    pub fn remaining_tokens(&self) -> u32 {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    /// Check whether any capacity remains (calls or tokens).
    pub fn has_capacity(&self) -> bool {
        self.remaining_calls() > 0 && self.remaining_tokens() > 0
    }

    /// Compute usage as a fraction of the limit (0.0–1.0).
    ///
    /// Returns 1.0 if the limit is 0 to avoid division by zero.
    pub fn call_usage_ratio(&self) -> f64 {
        if self.max_calls == 0 {
            return 1.0;
        }
        self.used_calls as f64 / self.max_calls as f64
    }

    /// Compute token usage as a fraction of the limit (0.0–1.0).
    ///
    /// Returns 1.0 if the limit is 0 to avoid division by zero.
    pub fn token_usage_ratio(&self) -> f64 {
        if self.max_tokens == 0 {
            return 1.0;
        }
        self.used_tokens as f64 / self.max_tokens as f64
    }
}
