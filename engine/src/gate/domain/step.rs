//! One step, as the gate reads it.
//!
//! @canonical .pi/architecture/modules/gate.md

use serde::{Deserialize, Serialize};

/// One step about to be dispatched, as the gate needs to see it.
///
/// The gate reads a step exactly as the two services do — name, tool, parameters —
/// and nothing else. Approvals, retries and graph position are the caller's, and
/// keeping them out is what lets a browser supply a step with no graph behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatedStep {
    /// Step name — the identity used across the engine and in evidence.
    pub name: String,
    /// The tool/action the step will invoke.
    pub tool: String,
    /// Full JSON parameter object of the step.
    pub parameters: serde_json::Value,
}

impl GatedStep {
    /// Build a step from its three parts.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        tool: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            tool: tool.into(),
            parameters,
        }
    }
}
