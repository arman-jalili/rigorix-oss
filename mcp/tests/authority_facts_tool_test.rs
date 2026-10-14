//! #1010 — the `rigorix_audit_authority_facts` tool.
//!
//! The tool name is part of the published contract: it must be byte-identical to
//! `mcpTool` for `rigorix.audit.authorityFacts` in the frozen SDK catalog. The
//! cross-repo half of that pinning is enforced in
//! `server/tests/catalog_parity.rs`
//! (`every_mcp_tool_maps_one_to_one_to_a_catalog_method` +
//! `oss_catalog_is_a_subset_of_the_sdk_catalog`); this file pins the name and the
//! behaviour of the tool itself.
//!
//! The load-bearing test is [`a_call_returns_a_bundle_that_verifies_by_re_derivation`]:
//! "returns JSON" is not the claim — the claim is that a third party can verify
//! the result by re-derivation, with no engine and no key.

use std::sync::Arc;

use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::projection::verify_authority_fact_bundle;
use rigorix_engine::audit::domain::authority_fact::{AuthorityFactBundle, EpistemicStatus};
use rigorix_engine::audit::domain::envelope::AuditEnvelope;
use rigorix_engine::audit::infrastructure::LocalAuditEnvelopeRepository;
use rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository;

use rigorix_mcp::audit_tools::application::dto::AuthorityFactsInput;
use rigorix_mcp::audit_tools::application::service::AuditAuthorityFactsHandler;
use rigorix_mcp::audit_tools::application::service_impl::AuditAuthorityFactsHandlerImpl;
use rigorix_mcp::audit_tools::interfaces::mcp::{
    AUDIT_TOOL_NAMES, RIGORIX_AUDIT_AUTHORITY_FACTS_INPUT_SCHEMA,
    rigorix_audit_authority_facts_tool_descriptor,
};
use rigorix_mcp::host::all_tool_descriptors;

/// Byte-identical to the frozen SDK catalog's `mcpTool`. Do not "fix" this
/// string to match a local convention — the catalog is published.
const TOOL_NAME: &str = "rigorix_audit_authority_facts";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// One signed envelope recording one precondition finding.
///
/// `execution_id` is a parameter because the local repository stores one envelope
/// per execution id — a chain therefore spans several executions, which is how a
/// real trail is shaped.
fn envelope(
    execution_id: uuid::Uuid,
    sequence: u64,
    producer: &str,
    precondition_id: &str,
    outcome: &str,
) -> AuditEnvelope {
    serde_json::from_value(serde_json::json!({
        "execution_id": execution_id,
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
            "summary": format!("precondition '{precondition_id}' went '{outcome}'"),
            "authority_digest": format!("sha256:authority-{sequence}")
        }]
    }))
    .expect("envelope parses")
}

/// Link a history into a genuine chain: `verify_chain` requires each successor to
/// carry its predecessor's hash, so a hand-built list is not enough.
fn linked(history: Vec<AuditEnvelope>) -> Vec<AuditEnvelope> {
    use std::collections::HashMap;

    // Chains are PER PRODUCER: every producer's genesis carries no `prev_hash`,
    // because a predecessor from another producer is not a predecessor. Inputs
    // must already be grouped per producer in sequence order.
    let mut tails: HashMap<String, AuditEnvelope> = HashMap::new();
    let mut out: Vec<AuditEnvelope> = Vec::new();
    for mut envelope in history {
        let producer = envelope.producer_id.clone().unwrap_or_default();
        if let Some(previous) = tails.get(&producer) {
            envelope.prev_hash =
                Some(AuditEnvelopeFactoryImpl::envelope_hash(previous).expect("hash"));
        }
        tails.insert(producer, envelope.clone());
        out.push(envelope);
    }
    out
}

/// A two-envelope chain for producer `local` answering `beneficiary-eligible`.
fn history() -> Vec<AuditEnvelope> {
    linked(vec![
        envelope(
            uuid::Uuid::new_v4(),
            0,
            "local",
            "beneficiary-eligible",
            "passed",
        ),
        envelope(
            uuid::Uuid::new_v4(),
            1,
            "local",
            "beneficiary-eligible",
            "failed",
        ),
    ])
}

/// Persist the history and hand back the repository the handler reads.
async fn store(
    envelopes: &[AuditEnvelope],
) -> (tempfile::TempDir, Arc<dyn AuditEnvelopeRepository>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let repository: Arc<dyn AuditEnvelopeRepository> =
        Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));
    for envelope in envelopes {
        repository.save(envelope).await.expect("save envelope");
    }
    (dir, repository)
}

/// Call the handler and decode the bundle it returned.
async fn call(
    repository: &Arc<dyn AuditEnvelopeRepository>,
    input: AuthorityFactsInput,
) -> AuthorityFactBundle {
    let handler = AuditAuthorityFactsHandlerImpl::new(repository.clone());
    let result = handler.handle(input).await.expect("handler succeeds");
    assert!(!result.is_error, "the tool must not report an error");
    serde_json::from_str(&result.content[0].text).expect("the payload is a bundle")
}

// ---------------------------------------------------------------------------
// Descriptor / registry
// ---------------------------------------------------------------------------

#[test]
fn the_tool_name_is_the_frozen_catalog_name() {
    assert_eq!(
        rigorix_audit_authority_facts_tool_descriptor()["name"],
        TOOL_NAME,
        "the tool name is frozen by the SDK catalog and must not be renamed"
    );
    assert!(
        AUDIT_TOOL_NAMES.contains(&TOOL_NAME),
        "AUDIT_TOOL_NAMES must carry the tool: {AUDIT_TOOL_NAMES:?}"
    );
    assert_eq!(AUDIT_TOOL_NAMES.len(), 4, "audit tools 3 -> 4");
}

#[test]
fn the_tool_appears_in_the_host_tool_list() {
    let names: Vec<String> = all_tool_descriptors()
        .iter()
        .filter_map(|d| d["name"].as_str().map(str::to_string))
        .collect();
    assert!(
        names.contains(&TOOL_NAME.to_string()),
        "tools/list must include the tool: {names:?}"
    );
}

/// The disclosure is mandated by spec §4.3.3: a bundle carries the envelopes, so
/// the caller receives the signed history, not a summary. It must be declared in
/// the description, not discovered by reading the response.
#[test]
fn the_descriptor_discloses_the_envelope_exposure() {
    let descriptor = rigorix_audit_authority_facts_tool_descriptor();
    let description = descriptor["description"]
        .as_str()
        .expect("description is a string");
    assert!(
        description.contains("DISCLOSES"),
        "the description must disclose the exposure: {description}"
    );
    assert!(
        description.to_lowercase().contains("envelope"),
        "the description must name what is disclosed: {description}"
    );
}

/// The input schema mirrors the frozen SDK `authorityFactsParams` `$defs`.
#[test]
fn the_input_schema_mirrors_the_frozen_params() {
    let schema: serde_json::Value =
        serde_json::from_str(RIGORIX_AUDIT_AUTHORITY_FACTS_INPUT_SCHEMA).expect("valid JSON");
    assert_eq!(schema["required"], serde_json::json!(["subjects"]));
    assert_eq!(schema["properties"]["subjects"]["type"], "array");
    assert_eq!(
        schema["properties"]["subjects"]["minItems"], 1,
        "a projection with no subjects carries no attestation"
    );
    assert_eq!(schema["properties"]["producer_id"]["type"], "string");
    assert_eq!(
        schema["additionalProperties"], false,
        "the frozen params forbid additional properties"
    );
    // The schema the descriptor ships is the schema above, not a near copy.
    assert_eq!(
        rigorix_audit_authority_facts_tool_descriptor()["inputSchema"],
        schema
    );
}

// ---------------------------------------------------------------------------
// Behaviour
// ---------------------------------------------------------------------------

/// The load-bearing property: the returned bundle must survive the engine's
/// re-derivation check, offline.
#[tokio::test]
async fn a_call_returns_a_bundle_that_verifies_by_re_derivation() {
    let history = history();
    let (_dir, repository) = store(&history).await;

    let bundle = call(
        &repository,
        AuthorityFactsInput {
            subjects: vec!["authority:beneficiary-eligible".into()],
            producer_id: None,
        },
    )
    .await;

    assert_eq!(bundle.envelopes.len(), 2, "the whole chain is carried");
    verify_authority_fact_bundle(&bundle).expect("a returned bundle must verify by re-derivation");
}

/// `subjects_without_record` drives the `NOT_ESTABLISHED` facts, and the verifier
/// re-derives with that list verbatim. Reporting an *answered* subject there would
/// self-verify while asserting that an established fact had no record — so this
/// checks the computed value, not just the outcome.
#[tokio::test]
async fn an_answered_subject_is_not_reported_as_without_record() {
    let (_dir, repository) = store(&history()).await;

    let bundle = call(
        &repository,
        AuthorityFactsInput {
            subjects: vec![
                "authority:beneficiary-eligible".into(),
                "authority:never-checked".into(),
            ],
            producer_id: None,
        },
    )
    .await;

    assert_eq!(
        bundle.subjects_without_record,
        vec!["authority:never-checked".to_string()],
        "only the unanswered subject may be listed as without record"
    );

    let answered = bundle
        .projection
        .facts
        .iter()
        .find(|f| f.subject == "authority:beneficiary-eligible")
        .expect("the answered subject has facts");
    assert_eq!(answered.epistemic_status, EpistemicStatus::Established);

    let unanswered: Vec<_> = bundle
        .projection
        .facts
        .iter()
        .filter(|f| f.subject == "authority:never-checked")
        .collect();
    assert_eq!(unanswered.len(), 1, "absence is stated exactly once");
    assert_eq!(
        unanswered[0].epistemic_status,
        EpistemicStatus::NotEstablished
    );

    verify_authority_fact_bundle(&bundle).expect("still verifies");
}

/// An empty subject list is refused rather than answered with an empty bundle:
/// the SDK schema says `minItems: 1`.
#[tokio::test]
async fn an_empty_subject_list_is_refused() {
    let (_dir, repository) = store(&history()).await;
    let handler = AuditAuthorityFactsHandlerImpl::new(repository);
    let result = handler
        .handle(AuthorityFactsInput {
            subjects: vec![],
            producer_id: None,
        })
        .await;
    assert!(result.is_err(), "subjects: [] must be rejected");
}

/// `producer_id` scopes the projection to one chain. Without it the handler would
/// answer for a producer the caller did not ask about.
#[tokio::test]
async fn producer_id_restricts_the_projection_to_one_chain() {
    let history = linked(vec![
        envelope(
            uuid::Uuid::new_v4(),
            0,
            "alpha",
            "beneficiary-eligible",
            "passed",
        ),
        envelope(
            uuid::Uuid::new_v4(),
            1,
            "alpha",
            "beneficiary-eligible",
            "passed",
        ),
        envelope(
            uuid::Uuid::new_v4(),
            0,
            "beta",
            "beneficiary-eligible",
            "failed",
        ),
    ]);
    let (_dir, repository) = store(&history).await;

    let bundle = call(
        &repository,
        AuthorityFactsInput {
            subjects: vec!["authority:beneficiary-eligible".into()],
            producer_id: Some("beta".into()),
        },
    )
    .await;

    assert_eq!(bundle.envelopes.len(), 1, "only beta's chain is carried");
    assert_eq!(
        bundle.envelopes[0].producer_id.as_deref(),
        Some("beta"),
        "no other producer's history may leak into a scoped bundle"
    );
    assert_eq!(
        bundle
            .projection
            .anchor_ref
            .as_ref()
            .map(|a| a.head_sequence),
        Some(0),
        "the anchor is beta's tip, not the global tip"
    );
    verify_authority_fact_bundle(&bundle).expect("a scoped bundle still verifies");
}

/// An empty store is a legitimate state: every requested subject is answered with
/// an explicit absence, and the bundle still verifies.
#[tokio::test]
async fn an_empty_history_yields_an_explicit_absence_that_verifies() {
    let (_dir, repository) = store(&[]).await;

    let bundle = call(
        &repository,
        AuthorityFactsInput {
            subjects: vec!["authority:beneficiary-eligible".into()],
            producer_id: None,
        },
    )
    .await;

    assert_eq!(bundle.projection.facts.len(), 1);
    assert_eq!(
        bundle.projection.facts[0].epistemic_status,
        EpistemicStatus::NotEstablished
    );
    verify_authority_fact_bundle(&bundle).expect("an absence-only bundle must verify too");
}
