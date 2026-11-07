//! Contract: findings project to steps in engine order, `NotReached` survives (AC 9).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::engine_seam::{
    EngineEvaluation, EngineFinding, EngineFindingOutcome,
};
use rigorix_gate_wasm::domain::outcome::StepDecision;
use rigorix_gate_wasm::domain::services::EvidenceProjection;

#[test]
fn projection_is_a_single_injected_dependency() {
    // One method: the projection cannot grow a second, divergent ordering rule.
    let _project: fn(
        &dyn EvidenceProjection,
        &[EngineFinding],
    ) -> Vec<rigorix_gate_wasm::domain::outcome::StepOutcome> = |p, findings| p.project(findings);
}

#[test]
fn not_reached_is_distinct_from_refused() {
    // Three decisions, not two. Collapsing them is how a record stops explaining itself.
    assert_ne!(StepDecision::NotReached, StepDecision::Refused);
    assert_ne!(StepDecision::NotReached, StepDecision::Allowed);
}

#[test]
fn engine_findings_carry_their_own_outcome_and_evidence() {
    let findings = vec![
        EngineFinding {
            gate_id: "precondition.a".into(),
            outcome: EngineFindingOutcome::Passed,
            refusal: None,
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "precondition.b".into(),
            outcome: EngineFindingOutcome::Refused,
            refusal: Some("precondition_failed".into()),
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "requirement.c".into(),
            outcome: EngineFindingOutcome::NotReached,
            refusal: None,
            evidence: None,
            opaque: None,
        },
    ];
    let evaluation = EngineEvaluation {
        decision: Vec::new(),
        findings,
        envelopes: vec![],
        port_failures: vec![],
    };
    assert_eq!(evaluation.findings.len(), 3);
    assert_eq!(
        evaluation.findings[2].outcome,
        EngineFindingOutcome::NotReached
    );
    // The carrier is not a model: whatever the engine said is kept verbatim in `refusal`.
    assert_eq!(
        evaluation.findings[1].refusal.as_deref(),
        Some("precondition_failed")
    );
}

// ---------------------------------------------------------------------------
// Shipped projection (issue #1296): criterion 9
// ---------------------------------------------------------------------------

#[test]
fn the_shipped_projection_preserves_order_and_not_reached() {
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

    let findings = vec![
        EngineFinding {
            gate_id: "a.first".into(),
            outcome: EngineFindingOutcome::Passed,
            refusal: None,
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "b.second".into(),
            outcome: EngineFindingOutcome::Error,
            refusal: Some("precondition_timeout".into()),
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "c.third".into(),
            outcome: EngineFindingOutcome::NotReached,
            refusal: None,
            evidence: None,
            opaque: None,
        },
    ];

    let projection = EngineFindingsProjection::new(BusinessWording::new());
    let steps = projection.project(&findings);

    assert_eq!(
        steps.iter().map(|s| s.gate_id.as_str()).collect::<Vec<_>>(),
        vec!["a.first", "b.second", "c.third"],
        "order must be the engine's order, not a sorted one"
    );
    assert_eq!(steps[1].decision, StepDecision::Refused);
    assert_eq!(
        steps[1].kind,
        Some(rigorix_gate_wasm::domain::outcome::RefusalKind::PreconditionTimeout)
    );
    assert_eq!(
        steps[1].reason.as_deref(),
        Some("the required check did not finish in time")
    );
    assert_eq!(steps[2].decision, StepDecision::NotReached);
    assert!(steps[2].kind.is_none());
}

#[test]
fn an_unknown_engine_label_becomes_the_neutral_refusal_never_an_allow() {
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

    let findings = vec![EngineFinding {
        gate_id: "x".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some("some_future_kind".into()),
        evidence: None,
        opaque: None,
    }];
    let steps = EngineFindingsProjection::new(BusinessWording::new()).project(&findings);
    assert_eq!(steps[0].decision, StepDecision::Refused);
    assert_eq!(
        steps[0].kind,
        Some(rigorix_gate_wasm::domain::outcome::RefusalKind::PreconditionError)
    );
}

// ---------------------------------------------------------------------------
// The projection is a pure, total, order-preserving map (issue #1303)
// ---------------------------------------------------------------------------

#[test]
fn duplicate_gate_ids_are_preserved_not_deduplicated() {
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

    // The engine can evaluate the same gate twice (a retried step, two matched
    // preconditions with one id). Grouping or de-duplicating would silently drop a
    // decision the record is supposed to contain.
    let findings = vec![
        EngineFinding {
            gate_id: "precondition.a".into(),
            outcome: EngineFindingOutcome::Passed,
            refusal: None,
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "precondition.a".into(),
            outcome: EngineFindingOutcome::Refused,
            refusal: Some("precondition_failed".into()),
            evidence: None,
            opaque: None,
        },
    ];
    let steps = EngineFindingsProjection::new(BusinessWording::new()).project(&findings);
    assert_eq!(steps.len(), 2, "a decision was dropped");
    assert_eq!(steps[0].decision, StepDecision::Allowed);
    assert_eq!(steps[1].decision, StepDecision::Refused);
}

#[test]
fn no_findings_yields_no_steps() {
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

    let steps = EngineFindingsProjection::new(BusinessWording::new()).project(&[]);
    assert!(steps.is_empty());
    // The projection never invents a step to explain an empty run; whether an empty run is
    // allowed is the aggregate's decision, not the projector's.
}

#[test]
fn the_projection_is_total_over_the_engine_outcome_enum() {
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

    // Every outcome maps: Passed→Allowed, Promoted→Promoted, Refused/Error→Refused,
    // NotReached→NotReached.
    let outcomes = [
        (EngineFindingOutcome::Passed, StepDecision::Allowed),
        (EngineFindingOutcome::Promoted, StepDecision::Promoted),
        (EngineFindingOutcome::Refused, StepDecision::Refused),
        (EngineFindingOutcome::Error, StepDecision::Refused),
        (EngineFindingOutcome::NotReached, StepDecision::NotReached),
    ];
    let projection = EngineFindingsProjection::new(BusinessWording::new());
    for (engine_outcome, expected) in outcomes {
        let findings = vec![EngineFinding {
            gate_id: "g".into(),
            outcome: engine_outcome,
            refusal: None,
            evidence: None,
            opaque: None,
        }];
        assert_eq!(
            projection.project(&findings)[0].decision,
            expected,
            "{engine_outcome:?}"
        );
    }
}

#[test]
fn only_refused_steps_carry_a_reason() {
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

    let findings = vec![
        EngineFinding {
            gate_id: "a".into(),
            outcome: EngineFindingOutcome::Passed,
            refusal: None,
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "b".into(),
            outcome: EngineFindingOutcome::NotReached,
            refusal: Some("ignored_label".into()),
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "c".into(),
            outcome: EngineFindingOutcome::Refused,
            refusal: Some("precondition_failed".into()),
            evidence: None,
            opaque: None,
        },
    ];
    let steps = EngineFindingsProjection::new(BusinessWording::new()).project(&findings);
    assert!(steps[0].reason.is_none());
    // A `NotReached` step has no reason even if the engine sent a stray label: we did not
    // ask the question, so we must not answer it.
    assert!(steps[1].reason.is_none());
    assert!(steps[1].kind.is_none());
    assert!(steps[2].reason.is_some());
}

#[test]
fn a_promotion_is_never_an_error_and_never_an_allow() {
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

    // #1383 step 1. The third gate outcome had no home in `EngineFindingOutcome`, so a
    // draft adapter mapped it to `Error` — absorbing it into the terminal value, exactly as
    // `Unknown` became `Failed` (#1393) and `Promote` became `Passed` (#1392).
    let projection = EngineFindingsProjection::new(BusinessWording::new());
    let findings = vec![EngineFinding {
        gate_id: "requirements".into(),
        outcome: EngineFindingOutcome::Promoted,
        refusal: None,
        evidence: None,
        opaque: None,
    }];
    let projected = &projection.project(&findings)[0];

    assert_eq!(projected.decision, StepDecision::Promoted);
    assert_ne!(projected.decision, StepDecision::Allowed);
    assert_ne!(projected.decision, StepDecision::Refused);
    // Nothing was refused, so no refusal kind and no business wording may be attached: a
    // reason here would say a check said no about a check that said "ask a human".
    assert!(projected.kind.is_none(), "a promotion has no refusal kind");
    assert!(
        projected.reason.is_none(),
        "a promotion has no refusal reason"
    );
    // And the step-level convenience follows: a paused run is not a stopped one.
    assert!(!projected.is_refusal());
    assert!(projected.awaits_approval());
}

// ── #1456 · the two capabilities reach the crate's vocabulary ──────────────────────
//
// **Delta, recorded rather than invented.** The child asked for "the four numbers in the wire
// JSON". They cannot be there: `EngineFinding.evidence` is an `EvidenceRef` — a digest and a
// kind, "never the evidence body" (contract freeze), and `StepOutcome.evidence` is the same
// type. The operands live in the engine's `opaque` payload and in the engine's own decision
// bytes; putting them on this wire would break the freeze in the one direction it exists to
// protect. What the crate can honestly say is *which obligation refused*, which is the whole
// difference between `capacity_exceeded` and "a required check did not pass".

#[test]
fn a_capacity_refusal_becomes_capacity_exceeded_and_quotes_no_host_data() {
    use rigorix_gate_wasm::domain::outcome::RefusalKind;
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};
    let projection = EngineFindingsProjection::new(BusinessWording::new());

    // The gate's *identity* decides the kind — the reason string is prose, and matching on
    // prose is how a projection breaks when somebody rephrases a sentence.
    let findings = vec![EngineFinding {
        gate_id: "capacity".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some(
            "capacity refused: requested 18420, claimed 1427420, cap 1420000, overage 7420".into(),
        ),
        evidence: None,
        // #1462: the engine's own payload carries the operands; the projection reads them.
        opaque: Some(serde_json::json!({
            "evidence": {
                "requested": 18_420u64,
                "claimed": 1_427_420u64,
                "hard_limit": 1_420_000u64,
                "overage": 7_420u64
            }
        })),
    }];
    let projected = &projection.project(&findings)[0];

    assert_eq!(projected.kind, Some(RefusalKind::CapacityExceeded));
    let reason = projected.reason.as_deref().expect("a refusal has wording");
    assert!(
        !reason.contains("1427420") && !reason.contains("7420"),
        "the wording is static and value-free (NFR-003): {reason}"
    );
}

#[test]
fn a_committed_effect_becomes_duplicate_effect_not_a_precondition_error() {
    use rigorix_gate_wasm::domain::outcome::RefusalKind;
    use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};
    let projection = EngineFindingsProjection::new(BusinessWording::new());

    let findings = vec![EngineFinding {
        gate_id: "effects".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some("effect refused: already committed as QR-2041".into()),
        evidence: None,
        opaque: Some(serde_json::json!({
            "evidence": { "record_id": "QR-2041", "instant": "2026-11-04T14:02:00Z" }
        })),
    }];
    let projected = &projection.project(&findings)[0];

    assert_eq!(projected.kind, Some(RefusalKind::DuplicateEffect));
    let reason = projected.reason.as_deref().expect("a refusal has wording");
    assert!(
        !reason.contains("QR-2041"),
        "identifiers are not wording: {reason}"
    );
}
