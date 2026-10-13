//! ProjectAuthorityFacts — derive attributable authority facts from our own
//! signed, hash-chained history (ISSUE-AUTHORITY-FACT-PROJECTION).
//!
//! @canonical docs/spec/consequence-gating-v1.md
//! @canonical .pi/architecture/decisions/ADR-016-audit-integrity-anchor.md
//! Implements: ISSUE-AUTHORITY-FACT-PROJECTION (application)
//!
//! The projection is the **producer** side of the precondition-provider seam:
//! it hands a third party attributable present-state facts, without asserting
//! present standing and without running the engine. A subject with no record
//! yields an explicit `NOT_ESTABLISHED` fact — absence is recorded, never
//! silence, and never promoted to a positive.

use chrono::{DateTime, Utc};

use crate::audit::application::verify_chain;
use crate::audit::domain::authority_fact::{
    AnchorRef, AuthorityFact, AuthorityFactBundle, AuthorityFactProjection, EpistemicStatus,
    EvidenceRef,
};
use crate::audit::domain::envelope::AuditEnvelope;

/// The chain position and anchor marker of the tip of a chain.
fn anchor_ref(envelopes: &[AuditEnvelope]) -> Option<AnchorRef> {
    let tip = envelopes
        .iter()
        .filter_map(|envelope| envelope.sequence.map(|sequence| (sequence, envelope)))
        .max_by_key(|(sequence, _)| *sequence)?;
    Some(AnchorRef {
        head_sequence: tip.0,
        history_integrity: tip.1.history_integrity.and_then(|integrity| {
            serde_json::to_value(integrity)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
        }),
        producer_id: tip.1.producer_id.clone(),
    })
}

/// Project authority facts out of signed history.
pub struct ProjectAuthorityFacts;

impl ProjectAuthorityFacts {
    /// Project the authority facts recorded in `envelopes`.
    ///
    /// Every `precondition_findings[]` entry becomes an `ESTABLISHED` fact —
    /// for any outcome (`passed` / `failed` / `error`): the record establishes
    /// that the check was consulted and what it answered, not that the authority
    /// is valid.
    ///
    /// `subjects_without_record` names subjects the consumer asked about for
    /// which there is no record; each yields an explicit `NOT_ESTABLISHED` fact.
    pub fn project(
        envelopes: &[AuditEnvelope],
        subjects_without_record: &[String],
    ) -> AuthorityFactProjection {
        let mut facts: Vec<AuthorityFact> = Vec::new();

        for envelope in envelopes {
            let sequence = envelope.sequence.unwrap_or(0);
            for finding in &envelope.precondition_findings {
                facts.push(AuthorityFact {
                    subject: format!("authority:{}", finding.precondition_id),
                    scope: finding.step.clone(),
                    established_at: finding.checked_at,
                    epistemic_status: EpistemicStatus::Established,
                    outcome: Some(finding.outcome.clone()),
                    authority_digest: finding.authority_digest.clone(),
                    evidence_refs: vec![EvidenceRef::precondition_finding(
                        envelope.execution_id,
                        &finding.precondition_id,
                        sequence,
                    )],
                    chain_position: sequence,
                    producer_id: envelope.producer_id.clone(),
                });
            }
        }

        // Absence is asserted *relative to the history we were given*, so stamp it
        // at the chain head (or the epoch for an empty history) rather than
        // `Utc::now()`. Otherwise re-derivation would produce a different
        // `established_at` and every bundle carrying a NOT_ESTABLISHED fact would
        // fail its own verification.
        let as_of = envelopes
            .iter()
            .map(|envelope| envelope.timestamp)
            .max()
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH);

        for subject in subjects_without_record {
            facts.push(AuthorityFact::not_established_at(
                subject.clone(),
                "unknown",
                as_of,
            ));
        }

        // Deterministic ordering: chain position, then subject.
        facts.sort_by(|a, b| {
            a.chain_position
                .cmp(&b.chain_position)
                .then_with(|| a.subject.cmp(&b.subject))
        });

        AuthorityFactProjection::new(facts, anchor_ref(envelopes), Utc::now())
    }
}

/// Why a bundle failed verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionVerificationError {
    /// The envelope chain is not intact — a gap, a reorder, an insertion, or a
    /// broken `prev_hash`.
    Chain(String),
    /// Re-deriving from the bundled history produced different facts than the
    /// projection claims.
    FactsDiffer {
        /// How many facts the projection claims.
        claimed: usize,
        /// How many facts re-derivation produced.
        derived: usize,
    },
    /// Re-derivation produced a different anchor reference.
    AnchorDiffers,
}

impl std::fmt::Display for ProjectionVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chain(detail) => write!(f, "envelope chain is not intact: {detail}"),
            Self::FactsDiffer { claimed, derived } => write!(
                f,
                "re-derivation disagrees with the projection: {claimed} fact(s) claimed, \
                 {derived} derived"
            ),
            Self::AnchorDiffers => write!(f, "re-derivation disagrees on the anchor reference"),
        }
    }
}

impl std::error::Error for ProjectionVerificationError {}

/// Verify a projection by re-deriving it from the bundled history.
///
/// This is the re-derivation design: verification does not trust a signature, it
/// re-runs the derivation against history that must first be an intact chain.
/// That detects a signer asserting facts the anchored history does not support —
/// which a signature over the projection could not detect.
///
/// `generated_at` is deliberately **not** compared. It records when the
/// projection was produced, so it is not reproducible; everything that carries a
/// claim — the facts and the anchor reference — is compared exactly.
///
/// # Errors
/// - [`ProjectionVerificationError::Chain`] — the bundled history is not an
///   intact chain, so there is nothing trustworthy to re-derive from.
/// - [`ProjectionVerificationError::FactsDiffer`] /
///   [`ProjectionVerificationError::AnchorDiffers`] — the projection does not
///   match what the history says.
pub fn verify_authority_fact_bundle(
    bundle: &AuthorityFactBundle,
) -> Result<(), ProjectionVerificationError> {
    // 1. The history must be genuine before anything derived from it means
    //    anything. Unverified history re-derives to unverified facts.
    verify_chain(&bundle.envelopes)
        .map_err(|error| ProjectionVerificationError::Chain(error.to_string()))?;

    // 2. Re-derive with the same pure function the producer used.
    let derived =
        ProjectAuthorityFacts::project(&bundle.envelopes, &bundle.subjects_without_record);

    // 3. Everything that carries a claim must match exactly.
    if derived.facts != bundle.projection.facts {
        return Err(ProjectionVerificationError::FactsDiffer {
            claimed: bundle.projection.facts.len(),
            derived: derived.facts.len(),
        });
    }
    if derived.anchor_ref != bundle.projection.anchor_ref {
        return Err(ProjectionVerificationError::AnchorDiffers);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(
        sequence: u64,
        producer: &str,
        precondition_id: &str,
        outcome: &str,
    ) -> AuditEnvelope {
        serde_json::from_value(serde_json::json!({
            "execution_id": uuid::Uuid::nil(),
            "timestamp": chrono::Utc::now(),
            "template_id": "t",
            "planning_hash": "sha256:plan",
            "source": null,
            "repository": null,
            "author": null,
            "total_tokens": 0,
            "duration_ms": 0,
            "git_commit": null,
            "git_branch": null,
            "model_version": null,
            "planning_prompt": null,
            "events": [],
            "scoring_results": {},
            "signature": null,
            "producer_id": producer,
            "sequence": sequence,
            "precondition_findings": [{
                "precondition_id": precondition_id,
                "step": "pay",
                "outcome": outcome,
                "inputs_hash": "sha256:inputs",
                "checked_at": chrono::Utc::now().to_rfc3339(),
                "summary": "redacted",
                "authority_digest": format!("sha256:authority-{sequence}")
            }]
        }))
        .expect("envelope parses")
    }

    #[test]
    fn each_finding_becomes_an_established_fact_with_provenance() {
        let projection = ProjectAuthorityFacts::project(
            &[envelope(0, "local", "beneficiary-eligible", "passed")],
            &[],
        );
        assert_eq!(projection.facts.len(), 1);
        let fact = &projection.facts[0];
        assert_eq!(fact.epistemic_status, EpistemicStatus::Established);
        assert_eq!(fact.outcome.as_deref(), Some("passed"));
        assert_eq!(fact.chain_position, 0);
        assert_eq!(fact.authority_digest.as_deref(), Some("sha256:authority-0"));
        assert_eq!(fact.evidence_refs.len(), 1);
        assert_eq!(fact.evidence_refs[0].kind, "precondition_finding");
    }

    #[test]
    fn a_negative_outcome_is_still_established_not_adverse() {
        // The record establishes the check ran and answered; it does not assert
        // standing, and a `failed` outcome is not a projection-level refusal.
        let projection =
            ProjectAuthorityFacts::project(&[envelope(1, "local", "standing", "failed")], &[]);
        assert_eq!(
            projection.facts[0].epistemic_status,
            EpistemicStatus::Established
        );
        assert_eq!(projection.facts[0].outcome.as_deref(), Some("failed"));
    }

    #[test]
    fn a_subject_without_a_record_is_explicitly_not_established() {
        let projection = ProjectAuthorityFacts::project(
            &[envelope(0, "local", "beneficiary-eligible", "passed")],
            &["authority:revocation-status".to_string()],
        );
        let absent = projection
            .facts
            .iter()
            .find(|fact| fact.subject == "authority:revocation-status")
            .expect("absence is recorded, never silence");
        assert_eq!(absent.epistemic_status, EpistemicStatus::NotEstablished);
        assert!(absent.outcome.is_none());
    }

    #[test]
    fn anchor_ref_reports_the_chain_tip() {
        let projection = ProjectAuthorityFacts::project(
            &[
                envelope(0, "local", "a", "passed"),
                envelope(2, "local", "b", "passed"),
                envelope(1, "local", "c", "passed"),
            ],
            &[],
        );
        let anchor = projection.anchor_ref.expect("anchor ref");
        assert_eq!(anchor.head_sequence, 2);
        assert_eq!(anchor.producer_id.as_deref(), Some("local"));
    }

    #[test]
    fn an_absent_subject_re_derives_byte_identically_so_the_bundle_verifies() {
        // Regression: `not_established` used to stamp `Utc::now()`, so a bundle
        // carrying an absence failed its own re-derivation check.
        let envelopes = vec![envelope(0, "local", "beneficiary-eligible", "passed")];
        let subjects = vec!["authority:revocation-status".to_string()];
        let projection = ProjectAuthorityFacts::project(&envelopes, &subjects);
        let bundle = AuthorityFactBundle {
            projection,
            envelopes,
            subjects_without_record: subjects,
        };
        verify_authority_fact_bundle(&bundle).expect("bundle verifies by re-derivation");
    }
}
