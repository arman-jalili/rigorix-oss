//! PreconditionError — typed error enum for every consequence-gating failure
//! mode.
//!
//! @canonical .pi/architecture/modules/precondition.md#fail-modes
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — PreconditionError enum + `Display` +
//!   `is_retriable()` (all variants non-retriable)
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-11 (PreconditionError)
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `PreconditionError` is the single error type for this bounded context
//! - **Every variant is non-retriable** — a denied authority is not retriable;
//!   `is_retriable()` always returns `false` (ADR-017 §R1)
//! - `ConfigInvalid` is fail-closed: a corrupt / over-cap `.rigorix/preconditions.toml`
//!   refuses a matching step, it never degrades to "no gating"
//! - `NotArmed` is fail-closed: configured-but-unarmed refuses a matching step
//!   (`unarmed`), never a silent skip
//! - `Spawn` / `Timeout` / `TrustBoundary` map to the `error` outcome:
//!   indeterminate → refuse, never pass
//! - `Denied` maps to the `failed` outcome: the operator's check explicitly
//!   returned non-zero
//!
//! # Recovery
//! - None of the variants are retriable. The operator fixes the check, the
//!   config, or the trust boundary and re-runs. Retrying an indeterminate
//!   check would reintroduce the T₀→Tₙ gap this module closes.

use thiserror::Error;

/// Errors raised across the precondition lifecycle (config load, match,
/// execution, verdict).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PreconditionError {
    /// The precondition config file is unparseable, structurally invalid, or
    /// exceeds the safety caps. **Fail closed**: a matching step is refused;
    /// the run never silently proceeds without gating.
    #[error("Precondition config invalid: {0}")]
    ConfigInvalid(String),

    /// A precondition matched but could not be armed (malformed entry, missing
    /// executable at evaluation time). **Fail closed**: the matching step is
    /// refused with an `unarmed` finding — never skipped (ADR-017).
    #[error("Precondition '{precondition_id}' is configured but unarmed: {detail}")]
    NotArmed {
        /// Stable id of the precondition that could not arm.
        precondition_id: String,
        /// Why it could not arm.
        detail: String,
    },

    /// A precondition's `StepPredicate` could not be evaluated (e.g. an
    /// operator-authored `regex` predicate failed to compile). **Fail closed**.
    #[error("Precondition '{precondition_id}' match failed: {detail}")]
    Match {
        /// Stable id of the precondition whose match failed.
        precondition_id: String,
        /// The match failure detail.
        detail: String,
    },

    /// The check process could not be spawned. Maps to the `error` outcome and
    /// refuses the step — never a pass.
    #[error("Precondition '{precondition_id}' failed to spawn: {detail}")]
    Spawn {
        /// Stable id of the precondition whose command failed to spawn.
        precondition_id: String,
        /// OS / spawn failure detail.
        detail: String,
    },

    /// The check process exceeded its wall-clock timeout. Maps to the `error`
    /// outcome and refuses the step — never a pass.
    #[error("Precondition '{precondition_id}' timed out after {timeout_ms}ms")]
    Timeout {
        /// Stable id of the precondition that timed out.
        precondition_id: String,
        /// The configured wall-clock timeout.
        timeout_ms: u64,
    },

    /// The operator's check explicitly denied (non-zero exit). Maps to the
    /// `failed` outcome; the tool is never called.
    #[error("Precondition '{precondition_id}' denied step '{step}'")]
    Denied {
        /// Stable id of the precondition that denied.
        precondition_id: String,
        /// Name of the step that was denied.
        step: String,
    },

    /// The check program resolves **inside the agent-writable workspace** — a
    /// check the agent can edit is no check. Maps to the `error` outcome and
    /// refuses the step (ADR-017 §R1 trust boundary).
    #[error(
        "Precondition '{precondition_id}' command resolves inside the agent-writable \
         workspace: {command}"
    )]
    TrustBoundary {
        /// Stable id of the precondition whose command violated the boundary.
        precondition_id: String,
        /// The offending command program (`command[0]`).
        command: String,
    },
}

impl PreconditionError {
    /// Whether the failed operation should be retried with backoff.
    ///
    /// # Contract (Frozen)
    /// Always `false`. Every precondition failure is a determination: a denied
    /// authority, a corrupt config, an indeterminate check, or a trust-boundary
    /// violation. Retrying would not change the verdict deterministically
    /// (ADR-017 §R1 — "no retries that change the verdict").
    pub fn is_retriable(&self) -> bool {
        false
    }

    /// Stable machine-readable error code (for the host error taxonomy).
    ///
    /// Distinct per variant so MCP/HTTP hosts can map a precondition failure
    /// to a structured refusal without parsing `Display`.
    pub fn error_code(&self) -> &'static str {
        match self {
            PreconditionError::ConfigInvalid(_) => "PRECONDITION_CONFIG_INVALID",
            PreconditionError::NotArmed { .. } => "PRECONDITION_NOT_ARMED",
            PreconditionError::Match { .. } => "PRECONDITION_MATCH",
            PreconditionError::Spawn { .. } => "PRECONDITION_SPAWN",
            PreconditionError::Timeout { .. } => "PRECONDITION_TIMEOUT",
            PreconditionError::Denied { .. } => "PRECONDITION_DENIED",
            PreconditionError::TrustBoundary { .. } => "PRECONDITION_TRUST_BOUNDARY",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_variants() -> Vec<PreconditionError> {
        vec![
            PreconditionError::ConfigInvalid("bad toml".to_string()),
            PreconditionError::NotArmed {
                precondition_id: "beneficiary-eligible".to_string(),
                detail: "missing executable".to_string(),
            },
            PreconditionError::Match {
                precondition_id: "beneficiary-eligible".to_string(),
                detail: "regex failed to compile".to_string(),
            },
            PreconditionError::Spawn {
                precondition_id: "beneficiary-eligible".to_string(),
                detail: "ENOENT".to_string(),
            },
            PreconditionError::Timeout {
                precondition_id: "beneficiary-eligible".to_string(),
                timeout_ms: 5000,
            },
            PreconditionError::Denied {
                precondition_id: "beneficiary-eligible".to_string(),
                step: "pay".to_string(),
            },
            PreconditionError::TrustBoundary {
                precondition_id: "beneficiary-eligible".to_string(),
                command: "./check".to_string(),
            },
        ]
    }

    #[test]
    fn every_variant_is_non_retriable() {
        for variant in all_variants() {
            assert!(
                !variant.is_retriable(),
                "precondition errors must never be retriable: {variant:?}"
            );
        }
    }

    #[test]
    fn display_names_the_failure_mode() {
        for variant in all_variants() {
            let rendered = variant.to_string();
            assert!(!rendered.is_empty());
            assert!(rendered.contains("Precondition") || rendered.contains("precondition"));
        }
    }

    #[test]
    fn error_codes_are_distinct_and_non_empty() {
        let mut codes: Vec<&str> = all_variants().iter().map(|v| v.error_code()).collect();
        for code in &codes {
            assert!(!code.is_empty());
            assert!(code.starts_with("PRECONDITION_"));
        }
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(before, codes.len(), "each variant needs a distinct code");
    }
}
