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

use chrono::Utc;

use crate::audit::domain::authority_fact::{
    AnchorRef, AuthorityFact, AuthorityFactProjection, EpistemicStatus, EvidenceRef,
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

        for subject in subjects_without_record {
            facts.push(AuthorityFact::not_established(subject.clone(), "unknown"));
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
}
