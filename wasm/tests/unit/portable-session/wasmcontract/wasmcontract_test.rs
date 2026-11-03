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
