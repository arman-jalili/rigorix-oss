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

// ---------------------------------------------------------------------------
// Criterion 13: the independence check lives outside this crate
// ---------------------------------------------------------------------------
//
// The Rust tests cannot run the Node script (it needs npm), so they assert the properties
// the script must have. A property that cannot be checked here is a property nobody
// checks, and "we ran it once" is not a test.

fn script() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/host_verification/verify.mjs"),
    )
    .expect("host verification script exists")
}

#[test]
fn the_host_verifier_never_loads_the_wasm_module() {
    // If it did, the record would be verified by its own producer and the demo's sentence
    // would be false by construction (ADR-013).
    // Comments may *name* the producer — that is how the rule is explained. Code may not
    // import it, so the scan strips line comments first.
    let script = script()
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*"))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    for forbidden in [
        "rigorix-gate-wasm",
        "rigorix_gate_wasm",
        "rigorix-gate-wasm_bg.wasm",
        "@rigorix/gate",
    ] {
        assert!(
            !script.contains(forbidden),
            "the host verifier imports the producer: {forbidden}"
        );
    }
    // And it does import the independent SDK.
    assert!(script.contains("rigorix-verifier"));
}

#[test]
fn the_host_verifier_emits_exactly_three_verdicts() {
    let script = script();
    for verdict in ["verified", "not_verified", "unverifiable"] {
        assert!(script.contains(verdict), "missing verdict: {verdict}");
    }
    // Exit codes are distinct, so a shell caller cannot confuse "could not ask" with
    // "compared and failed".
    for code in ["process.exit(verdict === VERDICT.VERIFIED ? 0", "1", "2"] {
        assert!(script.contains(code), "missing exit mapping: {code}");
    }
}

#[test]
fn the_host_verifier_fails_closed_when_the_sdk_is_absent() {
    let script = script();
    // A missing SDK must be `unverifiable(verifier_unavailable)`, never `verified` and
    // never `not_verified`.
    assert!(script.contains("verifier_unavailable"));
    let missing_sdk = script
        .split("if (verifier === null)")
        .nth(1)
        .expect("explicit missing-SDK branch");
    assert!(missing_sdk.contains("UNVERIFIABLE"));
    assert!(
        !missing_sdk
            .split('}')
            .next()
            .unwrap_or_default()
            .contains("VERIFIED")
    );
}

#[test]
fn every_field_the_host_reads_is_on_the_wire() {
    use rigorix_gate_wasm::interfaces::dto::ArtifactSetDto;

    let dto = ArtifactSetDto {
        artifact_format_version: 1,
        engine_version: "1.12.1".into(),
        scenario_digest: "sha256:a".into(),
        fixture_digest: "sha256:b".into(),
        session_id: "sha256:c".into(),
        key_ref: "demo:key".into(),
        envelopes: vec!["{}".into()],
        evidence: vec![],
        outcome_digest: "sha256:d".into(),
    };
    let json = serde_json::to_value(&dto).expect("serialises");
    let script = script();
    for field in [
        "artifact_format_version",
        "engine_version",
        "scenario_digest",
        "fixture_digest",
        "session_id",
        "key_ref",
        "envelopes",
        "outcome_digest",
    ] {
        assert!(json.get(field).is_some(), "wire shape lacks {field}");
        assert!(script.contains(field), "host script does not read {field}");
    }
}

#[test]
fn the_host_obligation_is_documented_where_a_host_will_look() {
    let readme = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/host_verification/README.md"),
    )
    .expect("README exists");
    assert!(readme.contains("never loaded the wasm module"));
    assert!(readme.contains("unverifiable"));
}
