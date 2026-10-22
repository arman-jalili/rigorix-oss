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

fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rigorix-effect-setup-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(dir.join(".rigorix")).unwrap();
    dir
}

#[tokio::test]
async fn an_unreachable_http_backend_refuses_to_start() {
    let root = scratch_dir();
    std::fs::write(
        root.join(".rigorix/effect-commitment.toml"),
        r#"
[backend]
kind = "http"
base_url = "http://127.0.0.1:1"
timeout_ms = 500
"#,
    )
    .unwrap();

    let error =
        match rigorix_engine::effect_commitment::EffectCommitmentSetup::from_env(&root).await {
            Err(error) => error,
            Ok(_) => panic!("a backend pointing at nothing must not start"),
        };
    let detail = error.to_string();
    assert!(detail.contains("unreachable"), "detail: {detail}");
    assert!(
        detail.contains("L1"),
        "the error must name the honest ceiling: {detail}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn an_absent_declaration_still_means_no_seam() {
    let root = std::env::temp_dir().join(format!(
        "rigorix-effect-absent-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let setup = rigorix_engine::effect_commitment::EffectCommitmentSetup::from_env(&root)
        .await
        .expect("absent is not an error");
    assert!(
        setup.is_none(),
        "no declaration must leave the previous path unchanged"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn an_in_memory_declaration_arms_the_seam_without_network() {
    let root = scratch_dir();
    std::fs::write(
        root.join(".rigorix/effect-commitment.toml"),
        "[backend]\nkind = \"in_memory\"\n",
    )
    .unwrap();
    let setup = rigorix_engine::effect_commitment::EffectCommitmentSetup::from_env(&root)
        .await
        .expect("in-memory is always available")
        .expect("declared means armed");
    assert!(
        !setup.can_prove_commitment(),
        "no receipt reader means not L3"
    );
    assert_eq!(setup.reconcile().await.expect("empty sweep"), 0);
    let _ = std::fs::remove_dir_all(root);
}
