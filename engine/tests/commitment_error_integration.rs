//! CommitmentError integration: typed failure ↔ HTTP error body.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#commitmenterror
//! Implements: ISSUE-EFFECT-COMMITMENT-6 — upstream/downstream integration
//! Issue: #1119
//! ADR-019: fail-closed transport and a closed error vocabulary

use rigorix_engine::effect_commitment::domain::{CommitmentError, EffectRecordId, EffectState};
use rigorix_engine::effect_commitment::interfaces::{
    CommitmentErrorResponse, EFFECT_COMMITMENT_ENDPOINTS,
};

#[test]
fn every_variant_has_a_stable_wire_code() {
    let id = EffectRecordId::new();
    let variants = [
        (
            CommitmentError::AlreadySettled {
                record_id: id,
                state: EffectState::Committed,
            },
            "already_settled",
        ),
        (
            CommitmentError::NotAmbiguous {
                record_id: id,
                state: EffectState::Dispatched,
            },
            "not_ambiguous",
        ),
        (
            CommitmentError::UnauthorizedResolver {
                resolver_id: "agent".to_string(),
                reason: "F4".to_string(),
            },
            "unauthorized_resolver",
        ),
        (
            CommitmentError::EvidenceMissing { record_id: id },
            "evidence_missing",
        ),
        (
            CommitmentError::Unavailable {
                detail: "timeout".to_string(),
            },
            "unavailable",
        ),
        (
            CommitmentError::Contended {
                record_id: Some(id),
                detail: "lost race".to_string(),
            },
            "contended",
        ),
    ];

    for (error, code) in variants {
        assert_eq!(error.code(), code);

        // The frozen HTTP body carries the same discriminator and the affected
        // record when the error has one.
        let body = CommitmentErrorResponse::from_error(&error);
        assert_eq!(body.code, code);
        assert_eq!(body.record_id, error.record_id());
        let json = serde_json::to_string(&body).unwrap();
        let decoded: CommitmentErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, body);
    }
}

#[test]
fn only_transport_faults_are_retriable() {
    let id = EffectRecordId::new();
    assert!(
        CommitmentError::Unavailable {
            detail: "connection refused".to_string()
        }
        .is_retriable()
    );
    assert!(
        CommitmentError::Contended {
            record_id: Some(id),
            detail: "lost race".to_string()
        }
        .is_retriable()
    );

    for error in [
        CommitmentError::AlreadySettled {
            record_id: id,
            state: EffectState::Committed,
        },
        CommitmentError::NotAmbiguous {
            record_id: id,
            state: EffectState::Dispatched,
        },
        CommitmentError::UnauthorizedResolver {
            resolver_id: "agent".to_string(),
            reason: "F4".to_string(),
        },
        CommitmentError::EvidenceMissing { record_id: id },
    ] {
        assert!(!error.is_retriable(), "{error:?} must not be retriable");
    }
}

#[test]
fn the_error_contract_covers_every_endpoint_operation() {
    // One endpoint per operation: begin, settle, mark ambiguous, resolve,
    // query, ambiguous query.
    assert_eq!(EFFECT_COMMITMENT_ENDPOINTS.len(), 6);
    assert_eq!(
        EFFECT_COMMITMENT_ENDPOINTS
            .iter()
            .filter(|e| e.method == "POST")
            .count(),
        4
    );
    assert_eq!(
        EFFECT_COMMITMENT_ENDPOINTS
            .iter()
            .filter(|e| e.method == "GET")
            .count(),
        2
    );
}
