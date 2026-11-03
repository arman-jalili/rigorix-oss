//! Contract: `ReplaySession::open` rejects rather than defaults (module AC 1, 2).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::errors::SessionOpenError;
use rigorix_gate_wasm::domain::session::{OpenInputs, ReplaySession};
use rigorix_gate_wasm::domain::value::ScenarioFormatVersion;

#[test]
fn open_returns_a_typed_refusal_never_a_default() {
    let open: fn(OpenInputs) -> Result<ReplaySession, SessionOpenError> = ReplaySession::open;
    let _ = open;
    // The shape is the guarantee: there is no overload returning a bare session, so a
    // caller cannot obtain one without handling a refusal.
}

#[test]
fn every_open_failure_mode_is_representable() {
    // One variant per way the input can be wrong. No `Other`, no catch-all string: a
    // catch-all is where "the scenario did not parse" becomes "something happened".
    let modes = [
        SessionOpenError::ScenarioUnparseable,
        SessionOpenError::UnsupportedScenarioFormat {
            found: ScenarioFormatVersion(99),
        },
        SessionOpenError::FixtureIncomplete { field: "approvals" },
        SessionOpenError::FixtureUnparseable,
        SessionOpenError::ClockSeriesEmpty,
    ];
    assert_eq!(modes.len(), 5);

    // Wording is business-facing and value-free: a field *name*, never its value.
    assert!(!modes[2].to_string().contains('{'));
    assert!(!modes[1].to_string().contains("sha256"));
    assert!(!modes[1].to_string().contains('/'));
}
