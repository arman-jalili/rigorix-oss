//! Policy/config conformance (F-20260907-02): real parsed policy vs
//! rigorix-sdk `schemas/policy.json` v1.
//!
//! Loads the REAL conference-demo `.rigorix/sequence-policy.toml` (pinned
//! copy in `fixtures/` — provenance in the fixture README) through the real
//! `TomlSequencePolicyRepository`, serializes the parsed
//! `SequencePolicyConfig` with engine serde, and validates against the SDK
//! schema. Covers the R7 `history` rule (cross-run predicate).

use rigorix_engine::sequence_policy::domain::SequencePolicyConfig;
use rigorix_engine::sequence_policy::infrastructure::TomlSequencePolicyRepository;
use rigorix_engine::sequence_policy::infrastructure::repository::SequencePolicyRepository;

use crate::{assert_valid, load_schema};

fn fixture_toml() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/conformance/fixtures/conference-demo-sequence-policy.toml")
}

async fn load_real_config() -> SequencePolicyConfig {
    let repo = TomlSequencePolicyRepository::new(fixture_toml());
    repo.load_config()
        .await
        .expect("real conference-demo policy TOML loads")
        .expect("config file exists — not the fail-open-absent case")
}

#[tokio::test]
async fn real_conference_policy_conforms_to_schema() {
    let config = load_real_config().await;

    // Root semantics match the operator file: fail_closed = true (the file
    // declares it explicitly) and the ordered rule set is non-empty.
    assert!(
        config.fail_closed,
        "conference-demo declares fail_closed = true"
    );
    assert_eq!(config.rules.len(), 3, "conference-demo has three rules");

    // Serialize through engine serde (the export surface the Execution API /
    // enterprise policy-bundle alignment consumes) → validate vs policy.json.
    let value = serde_json::to_value(&config).expect("SequencePolicyConfig serializes");
    assert_valid(
        &load_schema("policy.json"),
        &value,
        "parsed SequencePolicyConfig",
    );

    // Rules semantics match the file: deny action on the composition attack,
    // promote on the critical transfer, and the R7 cross-run history rule.
    let json = serde_json::to_value(&config).unwrap();
    let rules = json["rules"].as_array().expect("rules array");
    assert_eq!(rules[0]["id"], "no-remove-then-reassign");
    assert_eq!(rules[0]["action"], "deny");
    assert_eq!(rules[1]["id"], "transfer-seat-is-critical");
    assert_eq!(rules[1]["action"], "promote");
    let r7 = &rules[2];
    assert_eq!(r7["id"], "no-cross-run-remove-reassign");
    assert_eq!(r7["action"], "deny");
    assert_eq!(
        r7["history"]["prior_node"], "attendance-remove",
        "R7 history predicate survives parse → schema → JSON"
    );
    assert_eq!(r7["history"]["same_principal"], true);
    assert_eq!(r7["history"]["window_secs"], 900);
}

#[tokio::test]
async fn export_surface_serializes_deterministically() {
    // The parsed config is a serde-serializable path — serialize twice and
    // assert byte-identical output (deterministic export for consumers).
    let config = load_real_config().await;
    let a = serde_json::to_string(&config).expect("serialize");
    let b = serde_json::to_string(&config).expect("serialize");
    assert_eq!(a, b, "policy export must be deterministic");

    let value: serde_json::Value = serde_json::from_str(&a).expect("parse");
    assert_valid(
        &load_schema("policy.json"),
        &value,
        "deterministic policy export",
    );
}
