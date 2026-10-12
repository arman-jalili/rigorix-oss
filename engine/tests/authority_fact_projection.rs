//! Authority-fact projection — end-to-end offline verification properties
//! (ISSUE-AUTHORITY-FACT-PROJECTION).
//!
//! The projection must be verifiable offline: canonical bytes are deterministic,
//! a tampered projection changes the digest, the claim ceiling is machine
//! readable, and no absolute path / username appears in the output.

use rigorix_engine::audit::application::projection::ProjectAuthorityFacts;
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
