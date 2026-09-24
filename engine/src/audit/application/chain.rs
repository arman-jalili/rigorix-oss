//! ADR-016 Phase A — local per-producer chain: linking + verification.
//!
//! @canonical .pi/architecture/decisions/ADR-016-audit-integrity-anchor.md
//! Implements: GAP-A-30 (Phase A) — chain envelopes so interior deletion,
//!   reorder, and insertion in the local cache are detectable.
//!
//! The chain is **the same canonicalization the HMAC signs**: `prev_hash` is
//! SHA-256 over the predecessor's canonical bytes (`signature` nulled), so the
//! chain and the signature can never disagree. Legacy envelopes without chain
//! fields are grandfathered (no chain claim) and excluded from verification.
//!
//! Scope boundary (ADR-016): a purely local chain **cannot** detect deletion of
//! the most recent envelope (the tail). This module detects interior gaps
//! (missing/duplicate/reordered sequence) and head deletion (a group whose
//! earliest sequence is not the genesis `0`), and `prev_hash` mismatches. Tail
//! deletion is closed only by the anchor (Phase C).

use crate::audit::domain::{AuditEnvelope, AuditError};

use super::envelope_factory_impl::AuditEnvelopeFactoryImpl;

/// Resolve the next `(sequence, prev_hash)` for `producer_id` from the given
/// envelopes. Genesis is `(0, None)`.
///
/// # Errors
/// - `AuditError::Internal` — the predecessor's canonical bytes could not be
///   hashed.
pub fn next_link_from(
    envelopes: &[AuditEnvelope],
    producer_id: &str,
) -> Result<(u64, Option<String>), AuditError> {
    let last = envelopes
        .iter()
        .filter(|e| e.producer_id.as_deref() == Some(producer_id))
        .filter_map(|e| e.sequence.map(|s| (s, e)))
        .max_by_key(|(sequence, _)| *sequence);
    match last {
        Some((sequence, envelope)) => Ok((
            sequence + 1,
            Some(AuditEnvelopeFactoryImpl::envelope_hash(envelope)?),
        )),
        None => Ok((0, None)),
    }
}

/// Verify the local per-producer chain over `envelopes`.
///
/// For each producer with chained envelopes (both `producer_id` and `sequence`
/// present), ordered by `sequence`:
/// - sequence numbers are contiguous **from genesis `0`** (a missing head or
///   interior number is a deletion/gap; a duplicate or out-of-order number is
///   an insertion/reorder);
/// - each successor's `prev_hash` equals SHA-256 of the predecessor's canonical
///   bytes.
///
/// Legacy envelopes (`producer_id`/`sequence` = `None`) carry no chain claim and
/// are grandfathered — they are not verified here (no behavior change for
/// pre-phase-A data).
///
/// # Errors
/// - `AuditError::ChainBroken` — a gap, duplicate, missing `prev_hash`, or
///   `prev_hash` mismatch was detected.
pub fn verify_chain(envelopes: &[AuditEnvelope]) -> Result<(), AuditError> {
    use std::collections::HashMap;

    let mut by_producer: HashMap<&str, Vec<&AuditEnvelope>> = HashMap::new();
    for envelope in envelopes {
        if let (Some(producer), Some(_)) = (envelope.producer_id.as_deref(), envelope.sequence) {
            by_producer.entry(producer).or_default().push(envelope);
        }
    }

    // Deterministic iteration for reproducible error messages.
    let mut producers: Vec<&str> = by_producer.keys().copied().collect();
    producers.sort_unstable();

    for producer in producers {
        let chain = by_producer.get_mut(producer).expect("key present");
        chain.sort_by_key(|e| e.sequence.expect("filtered on Some(sequence)"));

        // Genesis must be present: sequence 0 for the producer's first
        // envelope. A group that starts later means the head was deleted.
        let base = chain[0].sequence.expect("filtered on Some(sequence)");
        if base != 0 {
            return Err(AuditError::ChainBroken {
                detail: format!(
                    "producer '{producer}': chain starts at sequence {base}, not genesis 0 — \
                     the earliest envelope(s) were deleted"
                ),
            });
        }

        // Genesis has no predecessor: a non-empty `prev_hash` at sequence 0 means
        // content was moved to the genesis slot (a reorder).
        if chain[0].prev_hash.is_some() {
            return Err(AuditError::ChainBroken {
                detail: format!(
                    "producer '{producer}': genesis envelope carries a prev_hash — \
                     content was reordered into the genesis slot"
                ),
            });
        }

        for (index, envelope) in chain.iter().enumerate() {
            let expected = index as u64;
            let actual = envelope.sequence.expect("filtered on Some(sequence)");
            if actual != expected {
                return Err(AuditError::ChainBroken {
                    detail: format!(
                        "producer '{producer}': expected sequence {expected}, found {actual} — \
                         interior deletion, reorder, or insertion"
                    ),
                });
            }
        }

        for pair in chain.windows(2) {
            let (prev, next) = (pair[0], pair[1]);
            let expected = AuditEnvelopeFactoryImpl::envelope_hash(prev)?;
            let next_sequence = next.sequence.expect("filtered on Some(sequence)");
            match next.prev_hash.as_deref() {
                Some(actual) if actual == expected => {}
                Some(actual) => {
                    return Err(AuditError::ChainBroken {
                        detail: format!(
                            "producer '{producer}': prev_hash mismatch at sequence \
                             {next_sequence} (expected {}…, got {}…)",
                            truncate8(&expected),
                            truncate8(actual),
                        ),
                    });
                }
                None => {
                    return Err(AuditError::ChainBroken {
                        detail: format!(
                            "producer '{producer}': missing prev_hash at sequence {next_sequence}"
                        ),
                    });
                }
            }
        }
    }

    Ok(())
}

/// Whether any envelope carries the Phase A chain/integrity claim. Used to
/// decide whether the stricter unanchored regime applies (legacy-only history
/// is grandfathered — no behavior change).
pub fn any_chain_tagged(envelopes: &[AuditEnvelope]) -> bool {
    envelopes
        .iter()
        .any(|e| e.history_integrity.is_some() || e.producer_id.is_some() || e.sequence.is_some())
}

fn truncate8(s: &str) -> &str {
    &s[..s.len().min(8)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::application::dto::BuildEnvelopeInput;
    use crate::audit::application::factory::AuditEnvelopeFactory;
    use crate::audit::domain::{EventStatus, ExecutionEventRef, HistoryIntegrity};

    fn input(execution_id: uuid::Uuid) -> BuildEnvelopeInput {
        BuildEnvelopeInput {
            execution_id,
            template_id: "t".to_string(),
            planning_prompt: "p".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "node_completed".to_string(),
                summary: "s".to_string(),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: EventStatus::Success,
                payload: None,
            }],
            source: None,
            repository: None,
            author: None,
            identity: None,
            effect_key: None,
            producer_id: Some("local".to_string()),
            sequence: None,
            prev_hash: None,
            history_policy: None,
            total_tokens: 0,
            duration_ms: 0,
            git_commit: None,
            git_branch: None,
            model_version: None,
            planning_prompt_content: None,
            file_paths: vec![],
            metadata: None,
            scoring_results: std::collections::HashMap::new(),
            sign: true,
        }
    }

    /// Build a contiguous signed chain of `n` envelopes for one producer.
    async fn chain(n: usize) -> Vec<AuditEnvelope> {
        let factory = AuditEnvelopeFactoryImpl::new(Some("test-key".to_string()));
        let mut out: Vec<AuditEnvelope> = Vec::new();
        for seq in 0..n {
            let mut input = input(uuid::Uuid::new_v4());
            let (next_seq, prev) = next_link_from(&out, "local").expect("link");
            assert_eq!(next_seq, seq as u64);
            input.sequence = Some(next_seq);
            input.prev_hash = prev;
            out.push(factory.build_envelope(input).await.expect("build"));
        }
        out
    }

    #[tokio::test]
    async fn contiguous_chain_verifies() {
        let envelopes = chain(3).await;
        assert!(verify_chain(&envelopes).is_ok());
        assert!(any_chain_tagged(&envelopes));
    }

    #[tokio::test]
    async fn interior_deletion_is_detected() {
        let mut envelopes = chain(3).await;
        // Delete the middle envelope (interior gap).
        envelopes.remove(1);
        let err = verify_chain(&envelopes).expect_err("gap must be detected");
        assert!(matches!(err, AuditError::ChainBroken { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn head_deletion_is_detected() {
        let mut envelopes = chain(3).await;
        envelopes.remove(0);
        let err = verify_chain(&envelopes).expect_err("head deletion must be detected");
        assert!(matches!(err, AuditError::ChainBroken { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn reorder_is_detected() {
        let mut envelopes = chain(3).await;
        // A logical reorder moves content to a different sequence position
        // (merely reordering files is normalized by the sequence numbers).
        let first = envelopes[0].sequence;
        envelopes[0].sequence = envelopes[1].sequence;
        envelopes[1].sequence = first;
        let err = verify_chain(&envelopes).expect_err("reorder must be detected");
        assert!(matches!(err, AuditError::ChainBroken { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn inserted_envelope_is_detected() {
        let mut envelopes = chain(2).await;
        // Duplicate the second envelope's sequence → non-contiguous.
        let mut inserted = envelopes[1].clone();
        inserted.execution_id = uuid::Uuid::new_v4();
        envelopes.push(inserted);
        let err = verify_chain(&envelopes).expect_err("insertion must be detected");
        assert!(matches!(err, AuditError::ChainBroken { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn legacy_unchained_envelopes_are_grandfathered() {
        let factory = AuditEnvelopeFactoryImpl::new(Some("test-key".to_string()));
        let mut input = input(uuid::Uuid::new_v4());
        input.producer_id = None;
        input.sequence = None;
        let mut legacy = factory.build_envelope(input).await.expect("build");
        legacy.history_integrity = None; // simulate a pre-Phase-A envelope
        assert!(verify_chain(std::slice::from_ref(&legacy)).is_ok());
        assert!(!any_chain_tagged(std::slice::from_ref(&legacy)));
    }

    #[tokio::test]
    async fn tagged_integrity_is_detected_as_unanchored_regime() {
        let envelopes = chain(1).await;
        assert_eq!(
            envelopes[0].history_integrity,
            Some(HistoryIntegrity::LocalUnanchored)
        );
    }
}
