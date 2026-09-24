//! GAP-A-29 (#895) / ADR-014 — the retention coupling is enforced on the REAL
//! composition path.
//!
//! These tests do **not** call `validate_retention` directly (that predicate is
//! already covered by the domain unit tests in `config.rs`). They build the
//! service exactly as the MCP/CLI/server composition roots do — through
//! `SequencePolicySetup::from_env` — and prove that a config whose
//! effect-keyed window outlives the configured audit retention is refused when
//! the service loads its rules, for both the local TOML and the enterprise
//! bundle source.
//!
//! Env vars are process-global, so every test shares one lock and fully
//! controls the sequence-policy env surface for its duration.

use rigorix_engine::execution_engine::application::factory::SequencePolicySetup;
use rigorix_engine::sequence_policy::domain::SequencePolicyError;

/// All tests read/clear the same sequence-policy env vars — serialize them.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Env vars this test file owns. Cleared before each test so ambient CI
/// configuration (or a leaked value from a previous test) cannot affect it.
const OWNED_ENV: &[&str] = &[
    "RIGORIX_SEQUENCE_POLICY",
    "RIGORIX_SEQUENCE_POLICY_PATH",
    "RIGORIX_SEQUENCE_POLICY_BUNDLE",
    "RIGORIX_SEQUENCE_POLICY_BUNDLE_PATH",
    "RIGORIX_SEQUENCE_POLICY_PRECEDENCE",
    "RIGORIX_AUDIT_RETENTION_SECS",
];

fn reset_env() {
    for key in OWNED_ENV {
        unsafe { std::env::remove_var(key) };
    }
}

/// An operator rule with a `history` predicate (R7/R8) of `window_secs`,
/// optionally effect-keyed.
fn policy_toml(window_secs: u64, effect_key: bool) -> String {
    format!(
        r#"fail_closed = true

[[rules]]
id = "payout-effect-guard"
name = "No repeat payout for the same effect key"
description = "cross-run payout guard"
steps = [
  {{ tool = "run_command", params = [{{ pointer = "/command", kind = "glob", value = "*execute_payout*" }}] }},
]
history = {{ prior_node = "payout", same_principal = true, window_secs = {window_secs}, effect_key = {effect_key} }}
action = "deny"
"#
    )
}

/// Write the rule file into `<dir>/.rigorix/sequence-policy.toml`.
fn write_policy(dir: &std::path::Path, contents: &str) {
    let rigorix = dir.join(".rigorix");
    std::fs::create_dir_all(&rigorix).expect("create .rigorix");
    std::fs::write(rigorix.join("sequence-policy.toml"), contents).expect("write rule file");
}

/// The same effect-keyed rule as a `policy.json` v1 enterprise bundle.
fn effect_keyed_bundle(window_secs: u64) -> String {
    serde_json::json!({
        "fail_closed": true,
        "rules": [{
            "id": "payout-effect-guard",
            "name": "No repeat payout for the same effect key",
            "description": "cross-run payout guard",
            "steps": [{
                "tool": "run_command",
                "params": [{ "pointer": "/command", "kind": "glob", "value": "*execute_payout*" }]
            }],
            "history": {
                "prior_node": "payout",
                "same_principal": true,
                "window_secs": window_secs,
                "effect_key": true
            },
            "action": "deny"
        }]
    })
    .to_string()
}

/// Run `evaluate_plan` through the composition-built service on an empty plan
/// — enough to drive the per-run repository load (which is where the retention
/// coupling is enforced).
async fn evaluate_after_composition(dir: &std::path::Path) -> Result<(), SequencePolicyError> {
    let svc = SequencePolicySetup::from_env(dir).expect("gate armed");
    svc.evaluate_plan(&[], None).await.map(|_| ())
}

#[tokio::test]
async fn composition_refuses_effect_keyed_window_over_retention() {
    let _guard = ENV_LOCK.lock().await;
    reset_env();
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), &policy_toml(900, true));
    unsafe { std::env::set_var("RIGORIX_AUDIT_RETENTION_SECS", "600") };

    let result = evaluate_after_composition(dir.path()).await;

    reset_env();
    let err = result.expect_err("window (900s) > retention (600s) must fail closed");
    assert!(
        matches!(err, SequencePolicyError::InvalidConfig(_)),
        "{err:?}"
    );
}

#[tokio::test]
async fn composition_accepts_window_within_retention() {
    let _guard = ENV_LOCK.lock().await;
    reset_env();
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), &policy_toml(900, true));
    unsafe { std::env::set_var("RIGORIX_AUDIT_RETENTION_SECS", "900") };

    let result = evaluate_after_composition(dir.path()).await;

    reset_env();
    assert!(result.is_ok(), "window == retention loads: {result:?}");
}

#[tokio::test]
async fn composition_unset_retention_is_status_quo() {
    let _guard = ENV_LOCK.lock().await;
    reset_env();
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), &policy_toml(900, true));
    // RIGORIX_AUDIT_RETENTION_SECS deliberately unset → unlimited.

    let result = evaluate_after_composition(dir.path()).await;

    reset_env();
    assert!(
        result.is_ok(),
        "unset retention is unlimited and must not change behavior: {result:?}"
    );
}

#[tokio::test]
async fn composition_non_effect_keyed_rule_is_unaffected() {
    let _guard = ENV_LOCK.lock().await;
    reset_env();
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), &policy_toml(900, false));
    unsafe { std::env::set_var("RIGORIX_AUDIT_RETENTION_SECS", "60") };

    let result = evaluate_after_composition(dir.path()).await;

    reset_env();
    assert!(
        result.is_ok(),
        "non-effect-keyed history has no retention coupling: {result:?}"
    );
}

#[tokio::test]
async fn composition_bundle_source_is_equally_coupled() {
    let _guard = ENV_LOCK.lock().await;
    reset_env();
    let dir = tempfile::tempdir().expect("tempdir");
    // No local TOML: the enterprise bundle is the only source.
    unsafe { std::env::set_var("RIGORIX_SEQUENCE_POLICY_BUNDLE", effect_keyed_bundle(900)) };
    unsafe { std::env::set_var("RIGORIX_AUDIT_RETENTION_SECS", "600") };

    let result = evaluate_after_composition(dir.path()).await;

    reset_env();
    let err = result.expect_err("bundle effect-keyed window > retention must fail closed");
    assert!(
        matches!(err, SequencePolicyError::InvalidConfig(_)),
        "{err:?}"
    );
}

#[tokio::test]
async fn composition_invalid_retention_value_fails_closed() {
    let _guard = ENV_LOCK.lock().await;
    reset_env();
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), &policy_toml(60, true));
    unsafe { std::env::set_var("RIGORIX_AUDIT_RETENTION_SECS", "not-a-number") };

    let result = evaluate_after_composition(dir.path()).await;

    reset_env();
    let err = result.expect_err("a misconfigured retention value must fail closed");
    assert!(
        matches!(err, SequencePolicyError::InvalidConfig(_)),
        "{err:?}"
    );
}
