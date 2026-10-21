//! EffectCommitmentConfig integration: declaration -> adapters.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentconfig
//! Implements: ISSUE-EFFECT-COMMITMENT-13 — upstream/downstream integration
//! Issue: #1126
//! ADR-019: absent declaration means no adapter; malformed declaration fails closed

use rigorix_engine::effect_commitment::application::{
    EffectCommitmentBackendConfig, EffectCommitmentConfig,
};

#[test]
fn a_declared_http_backend_and_receipt_reader_carry_their_credentials() {
    let var = format!("RIGORIX_TEST_EFFECT_TOKEN_{}", uuid::Uuid::new_v4());
    // Unique variable name, so this test cannot race another test's env.
    unsafe { std::env::set_var(&var, "super-secret") };

    let text = format!(
        r#"
[backend]
kind = "http"
base_url = "https://ledger.example"
timeout_ms = 1000
max_response_body_bytes = 4096
auth_token_env = "{var}"

[receipt_reader]
base_url = "https://payments.example"
provider_id = "stripe"
auth_token_env = "{var}"
"#
    );

    let config = EffectCommitmentConfig::parse(&text).expect("valid declaration");
    match config.backend.clone() {
        EffectCommitmentBackendConfig::Http { auth_token, .. } => {
            assert_eq!(auth_token.as_deref(), Some("super-secret"));
        }
        other => panic!("expected http backend, got {other:?}"),
    }
    assert_eq!(
        config.receipt_reader.as_ref().and_then(|r| r.auth_token()),
        Some("super-secret")
    );

    // Debug must never render the credential.
    let rendered = format!("{:?}", config.backend);
    assert!(!rendered.contains("super-secret"));
    assert!(rendered.contains("<redacted>"));

    unsafe { std::env::remove_var(&var) };
}

#[test]
fn the_declaration_can_be_read_from_the_repository_path() {
    let scratch = std::env::temp_dir().join(format!(
        "rigorix-effect-config-file-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(scratch.join(".rigorix")).unwrap();
    let path = scratch.join(".rigorix/effect-commitment.toml");
    std::fs::write(
        &path,
        r#"
[backend]
kind = "in_memory"

[receipt_reader]
base_url = "https://payments.example"
provider_id = "stripe"
"#,
    )
    .unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let config = EffectCommitmentConfig::parse(&text).expect("valid declaration");
    assert!(matches!(
        config.backend,
        EffectCommitmentBackendConfig::InMemory
    ));
    assert!(config.can_prove_commitment());
}

#[test]
fn an_empty_or_malformed_declaration_is_handled_explicitly() {
    // Empty is valid: no backend means the inert in-process default.
    let config = EffectCommitmentConfig::parse("").expect("empty declaration");
    assert!(matches!(
        config.backend,
        EffectCommitmentBackendConfig::InMemory
    ));

    // Malformed is refused, never silently ignored.
    assert!(EffectCommitmentConfig::parse("not = = toml").is_err());
    assert!(
        EffectCommitmentConfig::parse(
            r#"
[backend]
kind = "in_memory"
unknown = true
"#
        )
        .is_err()
    );
}
