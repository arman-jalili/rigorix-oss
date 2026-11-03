//! Contract: the host verifies; the crate only produces (AC 12, 13; ADR-013).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::interfaces::{CONTRACT_FUNCTIONS, ContractVersion};

#[test]
fn the_crate_ships_no_verification_function() {
    // ADR-013: verification moves to the host, in a process that never loaded this
    // module. A verifier compiled into the producer makes the demo's claim false.
    assert!(
        !CONTRACT_FUNCTIONS.iter().any(|f| f.contains("verify")),
        "no exported function may verify: {CONTRACT_FUNCTIONS:?}"
    );
    assert_eq!(CONTRACT_FUNCTIONS.len(), 4);
}

#[test]
fn the_four_functions_are_the_frozen_surface() {
    assert_eq!(
        CONTRACT_FUNCTIONS,
        [
            "open_session",
            "attempt_run",
            "export_artifact_set",
            "contract_version"
        ]
    );
}

#[test]
fn the_host_is_told_the_three_versions() {
    // The host must be able to decide, before trusting the shape, which contract,
    // which engine and which container it is talking to.
    let version = ContractVersion {
        contract: "1.12.1".into(),
        engine: "1.12.1".into(),
        artifact_format: 1,
    };
    assert_eq!(version.contract, version.engine);
    assert_eq!(version.artifact_format, 1);
}
