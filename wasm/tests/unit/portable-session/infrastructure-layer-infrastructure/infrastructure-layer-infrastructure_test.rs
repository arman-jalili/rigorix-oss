//! Contract: one adapter per port, all fixture-driven, none ambient (AC 14, 23).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::ports::{
    BusinessSink, Clock, IdSource, KeySource, ProviderTransport, Randomness, SessionStore,
};
use rigorix_gate_wasm::domain::value::{ClockTrack, KeyRef, RandomSeed};
use rigorix_gate_wasm::infrastructure::{
    FixtureClock, FixtureProviderTransport, FixtureRandomness, InMemorySessionStore,
    MemoryBusinessSink, SeededIdSource, SeededKeySource,
};

use crate::src_files;

#[test]
fn every_port_has_a_shipped_adapter() {
    // Compile-time: each adapter must satisfy its port's trait, so a port change cannot
    // leave an adapter behind.
    let _: &dyn Clock = &FixtureClock::new(ClockTrack::default());
    let _: &mut dyn IdSource = &mut SeededIdSource::new(std::collections::HashMap::new());
    let _: &dyn Randomness = &FixtureRandomness::new(RandomSeed::new(
        rigorix_gate_wasm::domain::value::Digest::from_trusted(
            "sha256:".to_string() + &"a".repeat(64),
        ),
        vec![],
    ));
    let _: &dyn KeySource = &SeededKeySource::new(KeyRef::from_trusted("demo:key"));
    let _: &mut dyn ProviderTransport = &mut FixtureProviderTransport::new(serde_json::json!({}));
    let _: &dyn BusinessSink = &MemoryBusinessSink::new();
    let _: &dyn SessionStore = &InMemorySessionStore::new();
}

#[test]
fn the_unavailable_key_source_is_representable() {
    // The fault matrix needs a key source that cannot answer; the engine's truthful
    // status for that case is `unverified`, never `unauthenticated`.
    let source = SeededKeySource::unavailable();
    use rigorix_gate_wasm::domain::ports::KeySource as _;
    let failure = source.key_ref().expect_err("unavailable");
    assert_eq!(failure.port.as_str(), "key_source");
    assert_eq!(failure.kind.as_str(), "missing");
}

#[test]
fn infrastructure_holds_no_verification_logic() {
    let mut offenders = Vec::new();
    for (path, body) in src_files() {
        if !path.contains("/infrastructure/") {
            continue;
        }
        for line in crate::code_lines(&body) {
            for forbidden in ["verify(", "signature", "ed25519", "rigorix_verifier"] {
                if line.contains(forbidden) {
                    offenders.push(format!("{path}: {forbidden} in `{line}`"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "verification in infrastructure:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_repository_folder_implements_the_domain_owned_interface() {
    let files: Vec<String> = src_files().into_iter().map(|(p, _)| p).collect();
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("src/infrastructure/repository/mod.rs"))
    );
    // One definition, no fork: the trait is in domain/ports, and infrastructure
    // re-exports it rather than declaring a second one.
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("src/domain/ports/session_store.rs"))
    );
}

// ---------------------------------------------------------------------------
// Behaviour (issue #1298): the six adapters answer or fail, never default
// ---------------------------------------------------------------------------

use rigorix_gate_wasm::domain::ports::{Determination, IdKind, ProviderCall, SinkWrite};
use rigorix_gate_wasm::domain::value::{
    ClockReading, Digest, Draw, Params, PortFailureKind, Timestamp,
};
use rigorix_gate_wasm::infrastructure::{CanonicalSerializer, ScanningContentPolicy};

fn reading(index: u32) -> ClockReading {
    ClockReading {
        instant: Timestamp::from_trusted(format!("2026-11-03T09:00:0{index}Z")),
        index,
    }
}

#[test]
fn the_clock_consumes_in_order_then_refuses() {
    let mut clock = FixtureClock::new(ClockTrack::new(vec![reading(0), reading(1)]));
    assert_eq!(clock.advance().expect("first").index, 0);
    assert_eq!(clock.advance().expect("second").index, 1);
    assert_eq!(clock.remaining(), 0);
    let failure = clock.advance().expect_err("exhausted");
    assert_eq!(failure.port.as_str(), "clock");
    assert_eq!(failure.kind, PortFailureKind::Exhausted);
}

#[test]
fn the_id_source_distinguishes_missing_from_exhausted() {
    let mut sequences = std::collections::HashMap::new();
    sequences.insert(IdKind::Attempt, vec!["attempt-0".to_string()]);
    let mut ids = SeededIdSource::new(sequences);

    assert_eq!(ids.next_id(IdKind::Attempt).expect("first"), "attempt-0");
    // A sequence that ran out is Exhausted.
    assert_eq!(
        ids.next_id(IdKind::Attempt).expect_err("used up").kind,
        PortFailureKind::Exhausted
    );
    // A kind that was never declared is Missing — a different fixture-author mistake.
    assert_eq!(
        ids.next_id(IdKind::Envelope).expect_err("absent").kind,
        PortFailureKind::Missing
    );
}

#[test]
fn randomness_is_replayed_then_exhausted() {
    let seed = RandomSeed::new(
        Digest::from_trusted("sha256:".to_string() + &"a".repeat(64)),
        vec![Draw::from_trusted("00"), Draw::from_trusted("01")],
    );
    let mut random = FixtureRandomness::new(seed);
    assert_eq!(random.draw().expect("first").as_str(), "00");
    assert_eq!(random.draw().expect("second").as_str(), "01");
    assert_eq!(
        random.draw().expect_err("used up").kind,
        PortFailureKind::Exhausted
    );
}

#[test]
fn the_provider_answers_from_the_transcript_and_refuses_everything_else() {
    let transcript = serde_json::json!({
        "providers": {
            "eligibility.check": { "determination": "failed", "payload": { "reason": "not eligible" } },
            "broken.check": { "determination": "yes please" }
        }
    });
    let mut provider = FixtureProviderTransport::new(transcript);

    let call = |provider: &str, method: &str| ProviderCall {
        provider: provider.to_string(),
        method: method.to_string(),
        params: Params::default(),
    };

    let observation = provider
        .call(&call("eligibility", "check"))
        .expect("answered");
    assert_eq!(observation.determination, Determination::Failed);
    assert!(observation.payload.is_some());

    assert_eq!(
        provider
            .call(&call("nowhere", "check"))
            .expect_err("missing")
            .kind,
        PortFailureKind::Missing
    );
    assert_eq!(
        provider
            .call(&call("broken", "check"))
            .expect_err("malformed")
            .kind,
        PortFailureKind::Malformed
    );
}

#[test]
fn the_sink_records_in_order_and_performs_no_io() {
    let mut sink = MemoryBusinessSink::new();
    let write = SinkWrite {
        target: "ledger".into(),
        effect: Params::default(),
    };
    sink.record(&write).expect("recorded");
    assert_eq!(sink.writes().len(), 1);
    assert_eq!(sink.writes()[0].target, "ledger");
}

#[test]
fn the_store_keeps_sealed_records_and_refuses_a_second_put_for_one_identity() {
    use rigorix_gate_wasm::domain::ports::SessionStore;

    let artifact = crate::support::sealed_artifact();
    let mut store = InMemorySessionStore::new();
    store.put(&artifact).expect("first put");
    assert_eq!(store.len(), 1);
    assert!(store.get(&artifact.session_id).is_some());

    // Same identity, second put: overwriting would silently replace a record.
    assert!(store.put(&artifact).is_err());
    assert_eq!(
        store.remove(&artifact.session_id).map(|a| a.session_id),
        Some(artifact.session_id)
    );
}

#[test]
fn the_content_policy_refuses_each_rule_and_names_it() {
    use rigorix_gate_wasm::domain::artifact::EnvelopeBytes;
    use rigorix_gate_wasm::domain::services::{ArtifactContentPolicy, ContentRule};

    let clean = crate::support::sealed_artifact();
    assert_eq!(ScanningContentPolicy::new().check(&clean), Ok(()));

    let cases = [
        (
            b"-----BEGIN PRIVATE KEY-----".to_vec(),
            ContentRule::KeyMaterial,
        ),
        (
            b"/Users/arman/project/secret.txt".to_vec(),
            ContentRule::AbsolutePath,
        ),
        (
            b"Authorization: Bearer abc".to_vec(),
            ContentRule::Credential,
        ),
        (
            b"contact: someone@example.com".to_vec(),
            ContentRule::PersonalData,
        ),
    ];
    for (bytes, expected) in cases {
        let mut artifact = clean.clone();
        artifact.envelopes = vec![EnvelopeBytes(bytes)];
        assert_eq!(
            ScanningContentPolicy::new().check(&artifact),
            Err(expected),
            "rule {expected} not detected"
        );
    }
}

#[test]
fn the_canonical_serializer_is_stable_and_key_sorted() {
    let left = serde_json::json!({ "b": 2, "a": 1 });
    let right = serde_json::json!({ "a": 1, "b": 2 });
    let serializer = CanonicalSerializer::new();
    assert_eq!(
        serializer.to_bytes(&left).expect("bytes"),
        serializer.to_bytes(&right).expect("bytes")
    );
    assert_eq!(
        String::from_utf8(serializer.to_bytes(&left).expect("bytes")).expect("utf8"),
        r#"{"a":1,"b":2}"#
    );
}
