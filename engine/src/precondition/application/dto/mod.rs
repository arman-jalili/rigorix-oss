//! Boundary DTOs for the Consequence Gating module.
//!
//! @canonical .pi/architecture/modules/precondition.md#command-contract
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — DispatchStep, PreconditionCheckInput
//! Issue: #938 (consequence-gating epic — contract freeze)
//!
//! Two wire shapes are frozen here:
//!
//! - [`DispatchStep`] — the about-to-dispatch node view fed to
//!   `PreconditionService::evaluate` / `DispatchGate::assess`. Derived from the
//!   same node the executor is about to spawn.
//! - [`PreconditionCheckInput`] — the **exact** JSON object written to the
//!   check process's stdin (ADR-017 §R1 command contract):
//!   `{ "precondition_id", "execution_id", "step", "tool", "parameters" }`.
//!
//! # Contract (Frozen)
//! - Field names and types are frozen — the runner, the checks, and the
//!   evidence integrations depend on them
//! - `parameters` is the full JSON parameter object; only its one-way hash is
//!   ever recorded (SpanPrivacy)
//! - DTOs are serializable (JSON)

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A step about to dispatch — the precondition match input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchStep {
    /// Step name — the identity used across the engine and evidence.
    pub name: String,
    /// The tool/action this step will invoke.
    pub tool: String,
    /// Full JSON parameter object of the step.
    pub parameters: Value,
}

/// The stdin payload handed to a precondition check (ADR-017 §R1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreconditionCheckInput {
    /// Stable id of the precondition being evaluated.
    pub precondition_id: String,
    /// Globally unique execution identifier.
    pub execution_id: uuid::Uuid,
    /// Name of the matched step.
    pub step: String,
    /// The tool the step will invoke.
    pub tool: String,
    /// The step's full parameter object.
    pub parameters: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn check_input_wire_shape_is_frozen() {
        let input = PreconditionCheckInput {
            precondition_id: "beneficiary-eligible".to_string(),
            execution_id: uuid::Uuid::nil(),
            step: "pay".to_string(),
            tool: "payment_execute".to_string(),
            parameters: json!({ "beneficiary": "acct-1" }),
        };
        let value = serde_json::to_value(&input).expect("serialize");
        assert_eq!(value["precondition_id"], "beneficiary-eligible");
        assert_eq!(value["execution_id"], uuid::Uuid::nil().to_string());
        assert_eq!(value["step"], "pay");
        assert_eq!(value["tool"], "payment_execute");
        assert_eq!(value["parameters"]["beneficiary"], "acct-1");
    }

    #[test]
    fn dispatch_step_round_trips() {
        let step = DispatchStep {
            name: "pay".to_string(),
            tool: "payment_execute".to_string(),
            parameters: json!({ "amount": 100 }),
        };
        let encoded = serde_json::to_string(&step).expect("serialize");
        let decoded: DispatchStep = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, step);
    }
}
