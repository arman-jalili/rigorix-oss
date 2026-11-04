//! Contract: the four exports reject unknown fields and never accept a verdict (AC 15, 16).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::interfaces::dto::{
    ArtifactSetDto, GatedActionDto, OpenSessionDto, RefusalDto, RunOutcomeDto, SessionHandleDto,
    StepOutcomeDto,
};

#[test]
fn unknown_fields_are_rejected_not_ignored() {
    // A typo must be a refusal. Silently ignoring `allow: true` is the failure mode this
    // guards: the host thinks it asked for something.
    let err = serde_json::from_str::<GatedActionDto>(
        r#"{"name":"payment_execute","params":{},"allow":true}"#,
    );
    assert!(err.is_err(), "unknown field accepted on GatedActionDto");

    let err = serde_json::from_str::<SessionHandleDto>(r#"{"handle":"h","verdict":"allowed"}"#);
    assert!(err.is_err(), "unknown field accepted on SessionHandleDto");

    let err = serde_json::from_str::<OpenSessionDto>(r#"{"scenario":{},"fixtures":{}}"#);
    assert!(err.is_err(), "incomplete OpenSessionDto accepted");
}

#[test]
fn no_dto_carries_a_verdict_shaped_field() {
    // Serialise each DTO's *shape* and check the keys the host sees. If a verdict field
    // were added, this fails before it can be misused.
    let forbidden = ["allow", "override", "approve", "verdict", "force"];

    let action = GatedActionDto {
        name: "payment_execute".into(),
        params: serde_json::Map::new(),
    };
    let json = serde_json::to_value(&action).expect("serialises");
    for key in forbidden {
        assert!(json.get(key).is_none(), "GatedActionDto exposes {key}");
    }

    let step = StepOutcomeDto {
        gate_id: "precondition.a".into(),
        decision: "refused".into(),
        kind: Some("precondition_failed".into()),
        reason: Some("the required check did not pass".into()),
        evidence: None,
    };
    let json = serde_json::to_value(&step).expect("serialises");
    for key in forbidden {
        assert!(json.get(key).is_none(), "StepOutcomeDto exposes {key}");
    }
}

#[test]
fn refusal_shape_is_redacted() {
    let refusal = RefusalDto {
        kind: "port_unavailable".into(),
        message: "a required check was unavailable".into(),
        port: Some("provider_transport".into()),
        evidence_ref: None,
    };
    let text = serde_json::to_string(&refusal).expect("serialises");
    for leak in ["/Users/", "/home/", "PRIVATE KEY", "Bearer "] {
        assert!(!text.contains(leak), "refusal leaks {leak}");
    }
}

#[test]
fn outcome_status_is_two_valued_on_the_wire() {
    let forbidden = ["allow", "override", "approve", "verdict", "force"];
    // Three decisions per step, two statuses per run — and the wire values are exactly
    // the vocabulary the engine uses.
    let outcome = RunOutcomeDto {
        status: "refused".into(),
        steps: vec![StepOutcomeDto {
            gate_id: "precondition.a".into(),
            decision: "not_reached".into(),
            kind: None,
            reason: None,
            evidence: None,
        }],
        evidence: vec![],
    };
    let json = serde_json::to_value(&outcome).expect("serialises");
    assert_eq!(json["status"], "refused");
    assert_eq!(json["steps"][0]["decision"], "not_reached");

    let artifact = ArtifactSetDto {
        artifact_format_version: 1,
        engine_version: "1.12.1".into(),
        scenario_digest: "sha256:a".into(),
        fixture_digest: "sha256:b".into(),
        session_id: "sha256:c".into(),
        key_ref: "demo:key".into(),
        envelopes: vec![],
        evidence: vec![],
        outcome_digest: "sha256:d".into(),
    };
    let json = serde_json::to_value(&artifact).expect("serialises");
    for key in forbidden {
        assert!(json.get(key).is_none(), "ArtifactSetDto exposes {key}");
    }
}

// ---------------------------------------------------------------------------
// The four functions, end to end (issue #1309)
// ---------------------------------------------------------------------------

fn scenario() -> String {
    serde_json::json!({ "format_version": 1, "steps": [] }).to_string()
}
fn fixtures() -> String {
    serde_json::json!({
        "approvals": [],
        "providers": {},
        // The transcript supplies the id sequence as data (invariant 5: never minted).
        "ids": { "attempt": ["attempt-0"], "step": ["step-0", "step-1"], "envelope": ["env-0"] }
    })
    .to_string()
}
fn clock() -> String {
    serde_json::json!([{ "instant": "2026-11-03T09:00:00Z", "index": 0 }]).to_string()
}
fn seed() -> String {
    format!(
        r#"{{"seed_digest":"sha256:{}","draws":["00"]}}"#,
        "d".repeat(64)
    )
}

fn open() -> String {
    rigorix_gate_wasm::interfaces::open_session(scenario(), fixtures(), clock(), seed())
        .expect("open never errors")
}

#[test]
fn open_session_returns_an_opaque_handle() {
    let handle: serde_json::Value = serde_json::from_str(&open()).expect("json");
    let handle = handle["handle"].as_str().expect("handle field");
    assert!(handle.starts_with("gate:sha256:"));
    // Opening twice from the same inputs yields the same handle: the identity is derived,
    // not minted.
    let again: serde_json::Value = serde_json::from_str(&open()).expect("json");
    assert_eq!(again["handle"].as_str().expect("handle"), handle);
}

#[test]
fn a_refusal_is_a_return_value_and_the_only_error_is_a_bad_handle() {
    let handle = open();

    // The gate is not linked in this build, so the run refuses — as a return value.
    let outcome = rigorix_gate_wasm::interfaces::attempt_run(
        handle.clone(),
        serde_json::json!({ "name": "payment_execute", "params": {} }).to_string(),
    )
    .expect("a refusal must not be an error");
    let outcome: serde_json::Value = serde_json::from_str(&outcome).expect("json");
    assert_eq!(outcome["kind"], "gate_unavailable");

    // A malformed handle is the programmer error, and it is an `Err`.
    assert!(
        rigorix_gate_wasm::interfaces::attempt_run(
            serde_json::json!({ "handle": "not-a-handle" }).to_string(),
            serde_json::json!({ "name": "x", "params": {} }).to_string(),
        )
        .is_err()
    );

    // An unknown but well-formed handle is also an error, not a refusal.
    assert!(
        rigorix_gate_wasm::interfaces::attempt_run(
            format!(r#"{{"handle":"gate:sha256:{}"}}"#, "0".repeat(64)),
            serde_json::json!({ "name": "x", "params": {} }).to_string(),
        )
        .is_err()
    );
}

#[test]
fn unknown_dto_fields_are_refusals_not_ignored() {
    // `allow: true` must not be silently dropped: it is a refusal saying the request could
    // not be read (criterion 15).
    let handle = open();
    let denied = rigorix_gate_wasm::interfaces::attempt_run(
        handle,
        serde_json::json!({ "name": "x", "params": {}, "allow": true }).to_string(),
    )
    .expect("returns");
    let denied: serde_json::Value = serde_json::from_str(&denied).expect("json");
    assert_eq!(denied["kind"], "request_invalid");
}

#[test]
fn open_session_refuses_unpinnable_input() {
    // An empty clock series cannot be defaulted.
    let refused = rigorix_gate_wasm::interfaces::open_session(
        scenario(),
        fixtures(),
        "[]".to_string(),
        seed(),
    )
    .expect("returns");
    let refused: serde_json::Value = serde_json::from_str(&refused).expect("json");
    assert_eq!(refused["kind"], "session_open_failed");

    // A scenario with no declared version is refused before anything runs.
    let refused = rigorix_gate_wasm::interfaces::open_session(
        serde_json::json!({ "steps": [] }).to_string(),
        fixtures(),
        clock(),
        seed(),
    )
    .expect("returns");
    let refused: serde_json::Value = serde_json::from_str(&refused).expect("json");
    assert_eq!(refused["kind"], "session_open_failed");
}

#[test]
fn export_before_an_attempt_is_a_refusal_and_contract_version_reports_three_numbers() {
    let handle = open();
    let refused = rigorix_gate_wasm::interfaces::export_artifact_set(handle).expect("returns");
    let refused: serde_json::Value = serde_json::from_str(&refused).expect("json");
    assert_eq!(refused["kind"], "seal_failed");

    let version: serde_json::Value =
        serde_json::from_str(&rigorix_gate_wasm::interfaces::contract_version()).expect("json");
    assert_eq!(version["artifact_format"], 2);
    assert_eq!(version["contract"], version["engine"]);
    assert!(version.get("verified").is_none());
}
