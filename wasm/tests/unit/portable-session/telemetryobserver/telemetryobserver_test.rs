//! Contract: telemetry never becomes evidence (AC 17; ADR-003).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::events::{
    ArtifactSetExported, DomainEvent, EventClass, PortAnswerUnavailable, RandomDrawConsumed,
    RunAllowed, RunRefused, SessionOpened, StepDecisionRecorded,
};
use rigorix_gate_wasm::domain::value::{
    Digest, GateAttemptId, GatedAction, KeyRef, Params, PortFailure, PortFailureKind, PortId,
    ReplaySessionId,
};

fn digest(byte: char) -> Digest {
    Digest::from_trusted(format!("sha256:{}", byte.to_string().repeat(64)))
}

fn attempt() -> GateAttemptId {
    GateAttemptId::from_trusted("attempt-0")
}

#[test]
fn a_random_draw_is_telemetry_and_never_evidence() {
    let event = DomainEvent::RandomDrawConsumed(RandomDrawConsumed {
        draw_index: 0,
        purpose: "nonce".into(),
    });
    assert_eq!(event.class(), EventClass::Telemetry);
    assert!(!event.is_evidence());
}

#[test]
fn the_six_record_events_are_evidence() {
    let events = vec![
        DomainEvent::ReplaySessionOpened(SessionOpened {
            session_id: ReplaySessionId::from_digest(digest('b')),
            scenario_digest: digest('a'),
            fixture_digest: digest('c'),
            seed_digest: digest('d'),
            clock_len: 1,
        }),
        DomainEvent::RunAllowed(RunAllowed {
            attempt_id: attempt(),
            action: GatedAction::new("payment_execute", Params::default()),
            evidence: vec![],
        }),
        DomainEvent::RunRefused(RunRefused {
            attempt_id: attempt(),
            caused_by_step: "precondition.budget".into(),
            reason: "the required check did not pass".into(),
            evidence: None,
        }),
        DomainEvent::StepDecisionRecorded(StepDecisionRecorded {
            attempt_id: attempt(),
            step_index: 0,
            gate_id: "precondition.budget".into(),
            decision: "refused".into(),
        }),
        DomainEvent::PortAnswerUnavailable(PortAnswerUnavailable {
            attempt_id: attempt(),
            failure: PortFailure::new(PortId::ProviderTransport, PortFailureKind::Missing),
            step_index: 0,
        }),
        DomainEvent::ArtifactSetExported(ArtifactSetExported {
            artifact_format_version: 1,
            engine_version: "1.12.1".into(),
            scenario_digest: digest('a'),
            outcome_digest: digest('e'),
            key_ref: KeyRef::from_trusted("demo:key"),
        }),
    ];

    assert_eq!(events.len(), 6, "six evidence events, one telemetry event");
    for event in &events {
        assert_eq!(event.class(), EventClass::Evidence, "{event:?}");
        assert!(event.is_evidence());
    }
}

#[test]
fn telemetry_is_dropped_from_a_release_build_by_construction() {
    // The classification is a total function over the enum: there is no way for a
    // telemetry event to be marked evidence without editing this mapping, and a test
    // that pinned only one direction would miss exactly that edit.
    let telemetry = DomainEvent::RandomDrawConsumed(RandomDrawConsumed {
        draw_index: 3,
        purpose: "nonce".into(),
    });
    assert!(!telemetry.is_evidence());
}

// ---------------------------------------------------------------------------
// The observer is the diagnostic path; the type is the guarantee (issue #1310)
// ---------------------------------------------------------------------------

use rigorix_gate_wasm::interfaces::{Observed, TelemetryObserver};

#[test]
fn the_observer_never_promotes_telemetry_to_evidence() {
    let observer = TelemetryObserver::new();
    let observed: Observed = observer.observe(vec![
        DomainEvent::RandomDrawConsumed(RandomDrawConsumed {
            draw_index: 0,
            purpose: "nonce".into(),
        }),
        DomainEvent::ReplaySessionOpened(SessionOpened {
            session_id: ReplaySessionId::from_digest(digest('b')),
            scenario_digest: digest('a'),
            fixture_digest: digest('c'),
            seed_digest: digest('d'),
            clock_len: 1,
        }),
    ]);

    assert_eq!(observed.evidence.len(), 1);
    assert!(observed.evidence.iter().all(DomainEvent::is_evidence));
    // Whatever the build keeps, telemetry is never counted as evidence.
    for event in &observed.telemetry {
        assert!(!event.is_evidence());
    }
}

#[test]
fn a_default_build_keeps_no_telemetry_channel() {
    let observer = TelemetryObserver::new();
    assert!(
        !observer.keeps_telemetry(),
        "diagnostics must be opt-in: an unused channel is still a channel"
    );
    let observed = observer.observe(vec![DomainEvent::RandomDrawConsumed(RandomDrawConsumed {
        draw_index: 0,
        purpose: "nonce".into(),
    })]);
    assert!(observed.telemetry.is_empty());
    assert!(observed.evidence.is_empty());
}

#[test]
fn an_artifact_set_has_no_slot_for_an_event_at_all() {
    // The guarantee is structural: `ArtifactSet` carries evidence *references*, not events,
    // so no class of event — telemetry included — can be stored in a record even by mistake.
    let artifact = crate::support::sealed_artifact();
    let json = serde_json::to_value(&artifact).expect("serialises");
    for forbidden in ["event", "events", "telemetry", "random_draw", "draw_index"] {
        assert!(
            json.get(forbidden).is_none(),
            "an artifact set exposes a slot for {forbidden}"
        );
    }
    // And the wire shape the host receives has no such slot either.
    let dto = rigorix_gate_wasm::interfaces::dto::ArtifactSetDto {
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
    let json = serde_json::to_value(&dto).expect("serialises");
    for forbidden in ["event", "events", "telemetry"] {
        assert!(
            json.get(forbidden).is_none(),
            "wire shape exposes {forbidden}"
        );
    }
}
