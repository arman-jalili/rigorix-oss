//! Authority-fact projection — end-to-end offline verification properties
//! (ISSUE-AUTHORITY-FACT-PROJECTION).
//!
//! The projection must be verifiable offline: canonical bytes are deterministic,
//! a tampered projection changes the digest, the claim ceiling is machine
//! readable, and no absolute path / username appears in the output.

use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::projection::{
    BundleVerification, ProjectAuthorityFacts, ProjectionVerificationError,
    verify_authority_fact_bundle,
};
use rigorix_engine::audit::domain::authority_fact::{AuthorityFactBundle, AuthorityFactProjection};
use rigorix_engine::audit::domain::envelope::AuditEnvelope;

fn envelope(sequence: u64, producer: &str, precondition_id: &str, outcome: &str) -> AuditEnvelope {
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
            "summary": "precondition 'beneficiary-eligible' failed step 'pay'",
            "authority_digest": format!("sha256:authority-{sequence}")
        }]
    }))
    .expect("envelope parses")
}

fn history() -> Vec<AuditEnvelope> {
    vec![
        envelope(0, "local", "beneficiary-eligible", "passed"),
        envelope(1, "local", "beneficiary-eligible", "passed"),
        envelope(2, "local", "beneficiary-eligible", "failed"),
    ]
}

#[test]
fn projection_is_offline_deterministic() {
    let history = history();
    let a = ProjectAuthorityFacts::project(&history, &[]);
    let b = ProjectAuthorityFacts::project(&history, &[]);
    // Same facts over the same records → the same facts.
    assert_eq!(a.facts, b.facts);
    // Chain positions are carried and ordered.
    let positions: Vec<u64> = a.facts.iter().map(|f| f.chain_position).collect();
    assert_eq!(positions, vec![0, 1, 2]);
    // Anchor reference is the chain tip.
    assert_eq!(a.anchor_ref.as_ref().map(|r| r.head_sequence), Some(2));
}

#[test]
fn tampering_is_detected_by_the_digest() {
    let original = ProjectAuthorityFacts::project(&history(), &[]);
    let mut tampered = original.clone();
    // Flip a recorded outcome — the digest must change.
    tampered.facts[2].outcome = Some("passed".to_string());
    assert_ne!(original.digest(), tampered.digest());
}

#[test]
fn claim_ceiling_is_machine_readable_and_denies_standing() {
    let projection = ProjectAuthorityFacts::project(&history(), &[]);
    let value = serde_json::to_value(&projection).expect("serialize");
    assert_eq!(
        value["claim_ceiling"]["asserts_present_standing"],
        serde_json::json!(false)
    );
    assert!(
        value["claim_ceiling"]["does_not_establish"]
            .as_array()
            .is_some_and(|list| !list.is_empty())
    );
    assert_eq!(
        value["spec"],
        serde_json::json!("rigorix-authority-fact-projection")
    );
}

#[test]
fn absence_is_explicit_and_never_promoted() {
    let projection = ProjectAuthorityFacts::project(&history(), &["authority:revocation".into()]);
    let facts = &projection.facts;
    let absent = facts
        .iter()
        .find(|f| f.subject == "authority:revocation")
        .expect("absence recorded");
    assert_eq!(absent.epistemic_status.as_str(), "NOT_ESTABLISHED");
    // Never promoted to a positive.
    assert!(absent.outcome.is_none());
    assert_ne!(absent.epistemic_status.as_str(), "ESTABLISHED");
}

#[test]
fn no_absolute_paths_or_username_in_the_canonical_bytes() {
    let projection = ProjectAuthorityFacts::project(&history(), &[]);
    let text = String::from_utf8(projection.canonical_bytes()).expect("utf8");
    // No POSIX absolute path and no `file://` reference anywhere in the report.
    assert!(!text.contains("/Users/"), "username/path leak: {text}");
    assert!(!text.contains("/home/"), "username/path leak: {text}");
    assert!(!text.contains("file://"), "path leak: {text}");
}

// ---------------------------------------------------------------------------
// Re-derivation verification — the design that replaces a signature
// ---------------------------------------------------------------------------
//
// A signed projection would prove only "the holder of a key asserted this".
// Re-derivation proves "this is what the anchored history says", which also
// catches a signer asserting facts the chain does not support.

/// A genuinely **linked** chain: each envelope's `prev_hash` is the hash of its
/// predecessor, which is what `verify_chain` requires. `history()` above is
/// deliberately unlinked — the earlier tests exercise the projection, not the
/// chain.
fn chained_history() -> Vec<AuditEnvelope> {
    let mut out: Vec<AuditEnvelope> = Vec::new();
    for (sequence, outcome) in [(0_u64, "passed"), (1, "passed"), (2, "failed")] {
        let mut envelope = envelope(sequence, "local", "beneficiary-eligible", outcome);
        if let Some(previous) = out.last() {
            envelope.prev_hash =
                Some(AuditEnvelopeFactoryImpl::envelope_hash(previous).expect("hash"));
        }
        out.push(envelope);
    }
    out
}

fn bundle_with(subjects: &[String]) -> AuthorityFactBundle {
    let envelopes = chained_history();
    let projection = ProjectAuthorityFacts::project(&envelopes, subjects);
    AuthorityFactBundle {
        projection,
        envelopes,
        subjects_without_record: subjects.to_vec(),
    }
}

#[test]
fn an_intact_bundle_verifies_by_re_derivation() {
    assert_eq!(
        verify_authority_fact_bundle(&bundle_with(&[])),
        Ok(BundleVerification::ChainConsistent)
    );
}

/// The property a signature over the projection could NOT provide: a projection
/// asserting facts the chain does not support is caught, because the check
/// re-derives rather than trusting the artifact.
#[test]
fn a_forged_digest_is_detected() {
    let mut bundle = bundle_with(&[]);
    bundle.projection.facts[0].authority_digest = Some("sha256:forged".to_string());

    assert_eq!(
        verify_authority_fact_bundle(&bundle),
        Err(ProjectionVerificationError::FactsDiffer {
            claimed: 3,
            derived: 3,
        }),
        "a forged digest must be caught even though the fact count matches",
    );
}

#[test]
fn a_dropped_fact_is_detected() {
    let mut bundle = bundle_with(&[]);
    bundle.projection.facts.pop();

    assert_eq!(
        verify_authority_fact_bundle(&bundle),
        Err(ProjectionVerificationError::FactsDiffer {
            claimed: 2,
            derived: 3,
        }),
    );
}

/// `subjects_without_record` drives the `NOT_ESTABLISHED` facts, so dropping it
/// would silently make the check incomplete. Carrying it in the bundle is what
/// makes re-derivation reproducible.
#[test]
fn a_projection_that_omits_an_asked_about_subject_is_detected() {
    let subjects = vec!["authority:revocation".to_string()];
    let mut bundle = bundle_with(&subjects);
    // The consumer strips the absence fact but the bundle still claims it asked:
    // re-derivation puts it back, so the mismatch is visible.
    bundle
        .projection
        .facts
        .retain(|fact| fact.subject != "authority:revocation");

    assert_eq!(
        verify_authority_fact_bundle(&bundle),
        Err(ProjectionVerificationError::FactsDiffer {
            claimed: 3,
            derived: 4,
        }),
    );
}

#[test]
fn a_tampered_anchor_reference_is_detected() {
    let mut bundle = bundle_with(&[]);
    assert!(
        bundle.projection.anchor_ref.is_some(),
        "chained history anchors"
    );
    bundle.projection.anchor_ref = None;

    assert_eq!(
        verify_authority_fact_bundle(&bundle),
        Err(ProjectionVerificationError::AnchorDiffers),
    );
}

/// Unverified history re-derives to unverified facts, so the chain is checked
/// FIRST — a broken chain is reported as such, not as a fact mismatch.
#[test]
fn a_broken_chain_is_rejected_before_re_derivation() {
    let mut bundle = bundle_with(&[]);
    bundle.envelopes[2].prev_hash = Some("deadbeef".to_string());

    assert!(
        matches!(
            verify_authority_fact_bundle(&bundle),
            Err(ProjectionVerificationError::Chain(_))
        ),
        "a broken chain must be reported as a chain failure",
    );
}

#[test]
fn an_interior_deletion_is_rejected() {
    let mut bundle = bundle_with(&[]);
    bundle.envelopes.remove(1);

    assert!(matches!(
        verify_authority_fact_bundle(&bundle),
        Err(ProjectionVerificationError::Chain(_))
    ));
}

/// The bundle is a report, not a gate: it serializes with the projection and the
/// history it came from, so a third party has everything the check needs.
#[test]
fn the_bundle_round_trips_through_json() {
    let bundle = bundle_with(&[]);
    let json = serde_json::to_string(&bundle).expect("serialize");
    let back: AuthorityFactBundle = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.projection.facts, bundle.projection.facts);
    assert_eq!(back.envelopes.len(), bundle.envelopes.len());
    assert_eq!(
        verify_authority_fact_bundle(&back),
        Ok(BundleVerification::ChainConsistent)
    );
}

#[test]
fn the_projection_is_the_one_the_bundle_carries() {
    let projection: AuthorityFactProjection = bundle_with(&[]).projection;
    assert_eq!(projection.spec, "rigorix-authority-fact-projection");
}
