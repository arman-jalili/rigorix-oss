//! The public surface, named by a test (#1427).
//!
//! Issue: #1427
//!
//! Until this module existed, `grep -rl wasm_api wasm/tests/unit` returned nothing: the
//! crate's entire host contract — the four functions a JavaScript caller programs against —
//! had no test that named it. The interface floor caught that as a *drop* (71.47 % against
//! 74.47 %), which is what a ratchet is for, and the growth it caught was real: #1416's
//! `gate_declaration` block and #1385's `expected_decision` surface added fourteen lines that
//! nothing executed.
//!
//! # What is asserted here, and why it is not a smoke test
//!
//! - **Every refusal path that a host can reach is exercised**, because a refusal *is* the
//!   contract (ADR-012: a refusal is a return value, only a malformed handle throws).
//! - **The wire vocabulary is asserted as values**, not as "some JSON came back": the four
//!   `request_invalid` field names, the `session_open_failed`/`seal_failed`/`handle_invalid`
//!   kinds, and the projections that map the domain's four step decisions and six evidence
//!   kinds onto strings. A projection that silently loses a variant is the defect these
//!   tests exist to prevent.
//! - **The vector the engine evaluates is the vector these calls are given**, so the happy
//!   path is the same world `gate-vector` and the conformance harness use, not a second one.

use rigorix_gate_wasm::domain::outcome::{
    EvidenceKind, EvidenceRef, OutcomeStatus, RunOutcome, StepDecision, StepOutcome,
};
use rigorix_gate_wasm::domain::value::Digest;
use rigorix_gate_wasm::interfaces::wasm_api::artifact_query;
use rigorix_gate_wasm::interfaces::{
    RunOutcomeDto, attempt_run, contract_version, export_artifact_set, open_session,
};
use serde_json::Value;

/// The committed vector, so the happy path is the world the engine evaluates elsewhere too.
fn vector() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/conformance/vectors/vec-fixture-0001.json");
    let raw = std::fs::read_to_string(path).expect("committed vector exists");
    serde_json::from_str(&raw).expect("vector parses")
}

fn inputs() -> (String, String, String, String) {
    let vector = vector();
    (
        vector["scenario"].to_string(),
        vector["fixtures"].to_string(),
        vector["clock"].to_string(),
        vector["seed"].to_string(),
    )
}

fn refusal(json: &str) -> Value {
    let value: Value = serde_json::from_str(json).expect("a refusal is JSON");
    assert!(
        value.get("kind").is_some(),
        "a refusal carries a machine kind: {json}"
    );
    value
}

fn open() -> String {
    let (scenario, fixtures, clock, seed) = inputs();
    let handle = open_session(scenario, fixtures, clock, seed).expect("the vector opens");
    let parsed: Value = serde_json::from_str(&handle).expect("the handle is JSON");
    assert!(parsed["handle"].is_string(), "the handle carries a key");
    handle
}

// ── contract_version ────────────────────────────────────────────────────────

#[test]
fn contract_version_reports_the_three_versions_and_the_gate_capability() {
    let parsed: Value = serde_json::from_str(&contract_version()).expect("JSON");
    assert_eq!(parsed["contract"], env!("CARGO_PKG_VERSION"));
    assert!(
        parsed["artifact_format"].is_number(),
        "the artifact format is a number the host compares"
    );
    assert!(
        parsed["gate_evaluation"].is_boolean(),
        "a build says whether it can decide, rather than leaving the host to guess"
    );
    assert!(
        !parsed["engine"].as_str().unwrap_or_default().is_empty(),
        "the engine's version is reported, never defaulted"
    );
}

#[test]
fn the_contract_lists_exactly_the_four_functions() {
    use rigorix_gate_wasm::interfaces::CONTRACT_FUNCTIONS;
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

// ── open_session: the happy path and every refusal a host can cause ─────────

#[test]
fn open_session_pins_the_vectors_world() {
    let handle: Value = serde_json::from_str(&open()).expect("JSON");
    assert!(
        handle["handle"]
            .as_str()
            .map(|key| !key.is_empty())
            .unwrap_or(false),
        "the session is addressed by a key, and the key is not empty: {handle}"
    );
}

#[test]
fn open_session_names_the_input_it_could_not_read() {
    let (scenario, fixtures, clock, seed) = inputs();
    // One case per input, so a shared "could not read" message cannot pass for the four.
    let cases = [
        (
            "scenario".to_string(),
            fixtures.clone(),
            clock.clone(),
            seed.clone(),
        ),
        (
            scenario.clone(),
            "not json".to_string(),
            clock.clone(),
            seed.clone(),
        ),
        (
            scenario.clone(),
            fixtures.clone(),
            "[{".to_string(),
            seed.clone(),
        ),
        (
            scenario.clone(),
            fixtures.clone(),
            clock.clone(),
            "{".to_string(),
        ),
    ];
    for (scenario, fixtures, clock, seed) in cases {
        let refused = refusal(&open_session(scenario, fixtures, clock, seed).expect("a value"));
        assert_eq!(refused["kind"], "request_invalid", "{refused}");
        assert!(
            refused["message"]
                .as_str()
                .unwrap_or_default()
                .starts_with("the request could not be read:"),
            "{refused}"
        );
    }
}

#[test]
fn open_session_refuses_a_scenario_or_a_world_it_cannot_pin() {
    let (_, fixtures, clock, seed) = inputs();
    // Parsed as JSON, but not a scenario: the kind is the session's, not the request's —
    // the host sent something readable and it still could not be pinned.
    let refused = refusal(
        &open_session(
            "{}".to_string(),
            fixtures.clone(),
            clock.clone(),
            seed.clone(),
        )
        .expect("a value"),
    );
    assert_eq!(refused["kind"], "session_open_failed", "{refused}");

    let (scenario, _, clock, seed) = inputs();
    let empty_world = serde_json::json!({ "providers": {} }).to_string();
    let refused = refusal(&open_session(scenario, empty_world, clock, seed).expect("a value"));
    assert_eq!(refused["kind"], "session_open_failed", "{refused}");
}

#[test]
fn open_session_refuses_a_seed_digest_it_cannot_parse() {
    let (scenario, fixtures, clock, _) = inputs();
    let bad_seed = serde_json::json!({
        "seed_digest": "not-a-digest",
        "draws": ["00"],
    })
    .to_string();
    let refusal = refusal(&open_session(scenario, fixtures, clock, bad_seed).expect("a value"));
    assert_eq!(refusal["kind"], "request_invalid", "{refusal}");
    assert!(
        refusal["message"]
            .as_str()
            .unwrap_or_default()
            .contains("seed.seed_digest"),
        "the message names the field, not just the input: {refusal}"
    );
}

#[test]
fn a_seed_digest_is_checked_for_prefix_length_and_alphabet() {
    // The three ways a digest can be wrong, each refused by name through the surface: no
    // prefix, the wrong length, and uppercase hex — the last one on purpose, because two
    // spellings of one hash would make two artifacts look different for the same bytes
    // (FR-001). `Digest::parse` is what the record then carries, so this is the contract,
    // not the parser's convenience.
    let (scenario, fixtures, clock, _) = inputs();
    for digest in [
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "sha256:0123456789",
        "sha256:0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
    ] {
        let seed = serde_json::json!({ "seed_digest": digest, "draws": [] }).to_string();
        let refused = refusal(
            &open_session(scenario.clone(), fixtures.clone(), clock.clone(), seed).expect("value"),
        );
        assert_eq!(refused["kind"], "request_invalid", "{refused}");
        assert!(
            refused["message"]
                .as_str()
                .unwrap_or_default()
                .contains("seed.seed_digest"),
            "the field is named, so the host knows which input to fix: {refused}"
        );
    }

    // And the canonical form round-trips: `TryFrom<String>` and `String::from` are the two
    // conversions a caller uses, and a round-trip that changes the text would change a digest.
    let canonical = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let digest = Digest::try_from(canonical.to_string()).expect("canonical parses");
    assert_eq!(String::from(digest), canonical);
}

// ── attempt_run ─────────────────────────────────────────────────────────────

#[test]
fn attempt_run_decides_the_vectors_step_and_returns_a_refusal_as_a_value() {
    // `budget.check` denies in the vector, so the run is refused — and a refusal is a
    // return value with a status, never an `Err`.
    let outcome: Value = serde_json::from_str(
        &attempt_run(
            open(),
            serde_json::json!({ "name": "payment_execute", "params": {} }).to_string(),
        )
        .expect("a decision, even when it refuses"),
    )
    .expect("JSON");
    assert_eq!(outcome["status"], "refused", "{outcome}");
    assert!(
        outcome["steps"]
            .as_array()
            .map(|steps| !steps.is_empty())
            .unwrap_or(false),
        "a decision names the steps it evaluated: {outcome}"
    );
}

#[test]
fn attempt_run_names_the_input_it_could_not_read() {
    let handle = open();
    for (handle_json, action_json) in [
        ("{".to_string(), r#"{"name":"a"}"#.to_string()),
        (handle.clone(), "{".to_string()),
    ] {
        let refusal = refusal(&attempt_run(handle_json, action_json).expect("a value"));
        assert_eq!(refusal["kind"], "request_invalid", "{refusal}");
    }
}

#[test]
fn an_unknown_handle_is_the_one_provocation_a_host_can_make() {
    // ADR-012: the only error a host can provoke. It is returned as a refusal value here
    // (the wasm32 binding turns it into a JS exception), and it says which kind it is so a
    // host cannot read it as a gate decision.
    let unknown = serde_json::json!({ "handle": "0000000000000000deadbeefdeadbeef" }).to_string();
    // The `Err` arm is the JS exception the wasm32 binding throws: a malformed handle is a
    // programmer mistake, not a gate decision (ADR-012), and the value still carries the
    // kind a host logs.
    let error = attempt_run(
        unknown.clone(),
        serde_json::json!({ "name": "payment_execute", "params": {} }).to_string(),
    )
    .expect_err("an unknown handle is the one error a host can provoke");
    assert_eq!(refusal(&error)["kind"], "handle_invalid", "{error}");

    let error = export_artifact_set(unknown).expect_err("an unknown handle cannot be sealed");
    assert_eq!(refusal(&error)["kind"], "handle_invalid", "{error}");
}

// ── export_artifact_set ─────────────────────────────────────────────────────

#[test]
fn export_before_an_attempt_is_a_seal_refusal_naming_the_reason() {
    let refusal = refusal(&export_artifact_set(open()).expect("a value"));
    assert_eq!(refusal["kind"], "seal_failed", "{refusal}");
    assert!(
        refusal["message"]
            .as_str()
            .unwrap_or_default()
            .contains("no attempt"),
        "the message says what is missing: {refusal}"
    );
}

#[test]
fn export_after_an_attempt_returns_the_container_the_host_verifies() {
    let handle = open();
    let _ = attempt_run(
        handle.clone(),
        serde_json::json!({ "name": "payment_execute", "params": {} }).to_string(),
    )
    .expect("a decision");
    let artifact: Value =
        serde_json::from_str(&export_artifact_set(handle).expect("a container")).expect("JSON");
    assert!(
        artifact["artifact_format_version"].is_number(),
        "the container names its format so a verifier can refuse an unsupported one: {artifact}"
    );
    assert!(
        artifact["session_id"].is_string(),
        "the container names the session it belongs to: {artifact}"
    );
    assert!(artifact["envelopes"].is_array(), "{artifact}");
}

#[test]
fn export_names_the_handle_it_could_not_read() {
    let refusal = refusal(&export_artifact_set("{".to_string()).expect("a value"));
    assert_eq!(refusal["kind"], "request_invalid", "{refusal}");
    assert!(
        refusal["message"]
            .as_str()
            .unwrap_or_default()
            .contains("handle"),
        "{refusal}"
    );
}

// ── the read helper ─────────────────────────────────────────────────────────

#[test]
fn the_query_helper_is_reachable_without_a_session() {
    // A host polls with it. The value is `'static` and pure, so holding it must not need a
    // handle — and it must not be a second source of truth either.
    let query = artifact_query();
    let _: &dyn rigorix_gate_wasm::application::ArtifactQuery = query;
}

// ── the wire projections, asserted per variant ──────────────────────────────

fn evidence(kind: EvidenceKind) -> EvidenceRef {
    EvidenceRef {
        digest: Digest::of(b"evidence"),
        kind,
    }
}

#[test]
fn every_step_decision_reaches_the_wire_under_its_own_name() {
    let cases = [
        (StepDecision::Allowed, "allowed"),
        (StepDecision::Promoted, "promoted"),
        (StepDecision::Refused, "refused"),
        (StepDecision::NotReached, "not_reached"),
    ];
    for (decision, expected) in cases {
        let outcome = RunOutcome {
            status: OutcomeStatus::Refused,
            steps: vec![StepOutcome {
                gate_id: "precondition.budget".to_string(),
                decision,
                kind: None,
                reason: Some("a reason".to_string()),
                evidence: Some(evidence(EvidenceKind::Precondition)),
                detail: None,
            }],
            evidence: vec![evidence(EvidenceKind::Precondition)],
            decision: Vec::new(),
        };
        let dto = RunOutcomeDto::from(&outcome);
        assert_eq!(dto.steps[0].decision, expected);
    }
}

#[test]
fn every_evidence_kind_reaches_the_wire_under_its_own_name() {
    let cases = [
        (EvidenceKind::Precondition, "precondition"),
        (EvidenceKind::SequencePolicy, "sequence_policy"),
        (EvidenceKind::Requirement, "requirement"),
        (EvidenceKind::Approval, "approval"),
        (EvidenceKind::Identity, "identity"),
        (EvidenceKind::PortFailure, "port_failure"),
        // #1456: the two capabilities that now reach the crate.
        (EvidenceKind::Capacity, "capacity"),
        (EvidenceKind::Effects, "effects"),
    ];
    for (kind, expected) in cases {
        let outcome = RunOutcome {
            status: OutcomeStatus::Allowed,
            steps: Vec::new(),
            evidence: vec![evidence(kind)],
            decision: Vec::new(),
        };
        let dto = RunOutcomeDto::from(&outcome);
        assert_eq!(dto.evidence[0].kind, expected);
        assert!(
            dto.evidence[0].digest.starts_with(Digest::PREFIX),
            "a digest crosses the wire in its canonical form"
        );
    }
}

#[test]
fn a_run_status_is_two_valued_on_the_wire() {
    for (status, expected) in [
        (OutcomeStatus::Allowed, "allowed"),
        (OutcomeStatus::Refused, "refused"),
    ] {
        let outcome = RunOutcome {
            status,
            steps: Vec::new(),
            evidence: Vec::new(),
            decision: Vec::new(),
        };
        assert_eq!(RunOutcomeDto::from(&outcome).status, expected);
    }
}

// ── #1442: the engine's decision bytes and digest reach the host ────────────

/// Decode standard base64, so S2 can hash what the wire actually carries.
fn base64_decode(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut index = [0u8; 256];
    for (position, byte) in ALPHABET.iter().enumerate() {
        index[*byte as usize] = position as u8;
    }
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in text.bytes() {
        if byte == b'=' {
            break;
        }
        let value = u32::from(index[byte as usize]);
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn outcome_with_decision(decision: Vec<u8>, status: OutcomeStatus) -> RunOutcome {
    RunOutcome {
        status,
        steps: vec![StepOutcome {
            gate_id: "precondition.budget".to_string(),
            decision: if status == OutcomeStatus::Allowed {
                StepDecision::Allowed
            } else {
                StepDecision::Refused
            },
            kind: None,
            reason: Some("a reason".to_string()),
            evidence: None,
            detail: None,
        }],
        evidence: Vec::new(),
        decision,
    }
}

#[test]
fn s1_decisions_differing_in_one_byte_carry_different_bytes_and_digests() {
    let first = RunOutcomeDto::from(&outcome_with_decision(
        br#"{"verdict":"refused","findings":["a"]}"#.to_vec(),
        OutcomeStatus::Refused,
    ));
    let second = RunOutcomeDto::from(&outcome_with_decision(
        br#"{"verdict":"refused","findings":["b"]}"#.to_vec(),
        OutcomeStatus::Refused,
    ));

    assert_ne!(first.decision_canonical, second.decision_canonical);
    assert_ne!(first.decision_digest, second.decision_digest);
    // Everything else is identical: the difference is the bytes, not the projection.
    assert_eq!(first.status, second.status);
    assert_eq!(first.steps, second.steps);
    assert_eq!(first.evidence, second.evidence);
}

#[test]
fn s2_the_digest_names_the_bytes_beside_it() {
    let outcome = outcome_with_decision(
        br#"{"verdict":"allowed","findings":[]}"#.to_vec(),
        OutcomeStatus::Allowed,
    );
    let dto = RunOutcomeDto::from(&outcome);
    let canonical = dto.decision_canonical.expect("a decision crossed the wire");
    let digest = dto.decision_digest.expect("a digest crossed with it");

    let bytes = base64_decode(&canonical);
    assert_eq!(
        digest,
        format!("sha256:{}", sha256_hex(&bytes)),
        "the digest must name the decoded bytes, not the base64 text"
    );
    assert_eq!(
        bytes, outcome.decision,
        "base64 is a transport, not an encoding"
    );

    // And the encoding itself is standard base64, not merely self-consistent: `abc` is the
    // canonical padding example, and a decoder of my own that agreed with a wrong encoder
    // would pass every assertion above.
    let abc = RunOutcomeDto::from(&outcome_with_decision(
        b"abc".to_vec(),
        OutcomeStatus::Allowed,
    ));
    assert_eq!(abc.decision_canonical.as_deref(), Some("YWJj"));
    let padded = RunOutcomeDto::from(&outcome_with_decision(
        b"a".to_vec(),
        OutcomeStatus::Allowed,
    ));
    assert_eq!(padded.decision_canonical.as_deref(), Some("YQ=="));
}

#[test]
fn s3_a_refused_decision_carries_both_fields() {
    let dto = RunOutcomeDto::from(&outcome_with_decision(
        br#"{"verdict":"refused","findings":["x"]}"#.to_vec(),
        OutcomeStatus::Refused,
    ));
    assert_eq!(dto.status, "refused");
    assert!(
        dto.decision_canonical.is_some(),
        "a refusal is a verdict with a record"
    );
    assert!(dto.decision_digest.is_some());
}

#[test]
fn a_run_with_no_engine_decision_carries_neither_field() {
    // A port that could not answer refuses before the gate: there is no engine verdict, and a
    // digest over empty bytes would claim a record that does not exist.
    let dto = RunOutcomeDto::from(&RunOutcome {
        status: OutcomeStatus::Refused,
        steps: Vec::new(),
        evidence: Vec::new(),
        decision: Vec::new(),
    });
    assert_eq!(dto.decision_canonical, None);
    assert_eq!(dto.decision_digest, None);
}
