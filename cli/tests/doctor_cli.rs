//! `rigorix doctor` / `rigorix init` / `rigorix key` behaviour (#1101, #1102, #1103).
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Canonical Reference: .pi/architecture/modules/cli-boundary.md
//! Implements: ISSUE-NO-DOCTOR-COMMAND — the report is a CLI contract, not just a
//!   function, so it is exercised through the real binary
//! Issue: #1103
//!
//! These run the compiled binary because the properties being tested are about
//! the *command*: its exit code, and whether it still answers when the rest of
//! the CLI cannot start.
#![allow(clippy::disallowed_types)] // #1083: test-only spawn of the binary under test

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rigorix-cli"))
}

/// Remove the variables that arm seams, so the report is deterministic regardless
/// of the machine and of the other tests in this file.
fn isolated(cmd: &mut Command) {
    for var in [
        "RIGORIX_APPROVAL_BINDING",
        "RIGORIX_HMAC_KEY",
        "RIGORIX_APPROVAL_STORE",
        "RIGORIX_APPROVAL_CONSUMPTION_LEDGER",
        "RIGORIX_ANCHOR_URL",
        "RIGORIX_ANCHOR_PUBLIC_KEY",
        "RIGORIX_SEQUENCE_POLICY",
        "RIGORIX_SEQUENCE_POLICY_BUNDLE",
        "RIGORIX_SEQUENCE_POLICY_BUNDLE_PATH",
        "RIGORIX_SEQUENCE_POLICY_PATH",
    ] {
        cmd.env_remove(var);
    }
}

#[test]
fn doctor_lists_every_seam_and_exits_zero_when_nothing_is_misconfigured() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cmd = bin();
    cmd.current_dir(dir.path()).args(["doctor"]);
    isolated(&mut cmd);
    let out = cmd.output().expect("run rigorix doctor");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success(), "unarmed is a valid default: {stdout}");
    for seam in [
        "precondition gate",
        "approval binding",
        "sequence policy",
        "capacity ledger",
        "history anchor",
        "envelope signing",
        "effect scope",
        "effect commitment",
        "budget limits",
        "recovery recipes",
    ] {
        assert!(stdout.contains(seam), "report must name `{seam}`: {stdout}");
    }
    assert!(
        stdout.contains("1 of 10 seams armed"),
        "an empty directory arms only the budget limits, which every entry point attaches: {stdout}"
    );
    assert!(
        stdout.contains("budget limits"),
        "budget enforcement must be reported with its source: {stdout}"
    );
    assert!(
        stdout.contains("not the same as armed"),
        "the report must not let unarmed read as armed: {stdout}"
    );
}

/// The falsifier for #1103: a seam that is requested and cannot be honoured must
/// be reported **and** must make the command fail — and the report has to appear
/// even though the normal startup path aborts on this configuration.
#[test]
fn doctor_reports_a_requested_but_unbuildable_seam_and_exits_non_zero() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cmd = bin();
    cmd.current_dir(dir.path()).args(["doctor"]);
    // Isolate first, then request the binding: the other order removes the very
    // variable this test is about (which is what made it fail the first time).
    isolated(&mut cmd);
    cmd.env("RIGORIX_APPROVAL_BINDING", "1");
    let out = cmd.output().expect("run rigorix doctor");
    // A non-zero exit is an error, so the harness prints the report on stderr.
    // Assert on both streams: the property is "the operator is told", not "the
    // bytes landed on one particular fd".
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        !out.status.success(),
        "a seam that refuses every run must fail the command: {report}"
    );
    assert!(report.contains("MISCONFIGURED"), "{report}");
    assert!(
        report.contains("RIGORIX_HMAC_KEY"),
        "the report must say which variable is missing: {report}"
    );
}

/// #1099: the effect-scope oracle is not armed on its own. It is reported as
/// unarmed until the approval binding exists — the two are one control, and
/// claiming the oracle alone would be the "configured, never consulted" shape.
#[test]
fn effect_scope_is_reported_as_dependent_on_the_approval_binding() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cmd = bin();
    cmd.current_dir(dir.path()).args(["doctor"]);
    isolated(&mut cmd);
    let out = cmd.output().expect("run rigorix doctor");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("effect scope"), "{stdout}");
    assert!(
        stdout.contains("needs `approval binding`"),
        "the dependency must be stated, not implied: {stdout}"
    );
}

#[test]
fn init_always_reports_the_seam_inventory_even_when_the_config_exists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut first = bin();
    first.current_dir(dir.path()).args(["init"]);
    isolated(&mut first);
    let first = first.output().expect("first init");
    assert!(first.status.success());
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    assert!(first_stdout.contains("Initialized .rigorix/"));
    assert!(
        first_stdout.contains("envelope signing"),
        "init must tell a fresh repo which seams exist: {first_stdout}"
    );

    // Second run: the scaffold is left alone, but the advisory still runs. The
    // old early return meant an existing repo never heard any of it.
    let mut second = bin();
    second.current_dir(dir.path()).args(["init"]);
    isolated(&mut second);
    let second = second.output().expect("second init");
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    assert!(second.status.success());
    assert!(
        second_stdout.contains("already exists"),
        "must not re-scaffold: {second_stdout}"
    );
    assert!(
        second_stdout.contains("envelope signing"),
        "an existing repo must still get the advisory: {second_stdout}"
    );
}

#[test]
fn init_names_the_variables_that_arm_each_seam() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cmd = bin();
    cmd.current_dir(dir.path()).args(["init"]);
    isolated(&mut cmd);
    cmd.output().expect("init");

    let config = std::fs::read_to_string(dir.path().join(".rigorix").join("rigorix.toml"))
        .expect("rigorix.toml written");
    for needle in [
        ".rigorix/preconditions.toml",
        ".rigorix/sequence-policy.toml",
        ".rigorix/capacity.toml",
        "RIGORIX_APPROVAL_BINDING",
        "RIGORIX_ANCHOR_URL",
        "RIGORIX_HMAC_KEY",
    ] {
        assert!(config.contains(needle), "scaffold must name `{needle}`");
    }
    assert!(
        config.contains("ALL UNARMED AS WRITTEN"),
        "the scaffold must not imply the seams are armed"
    );
}

/// #1102: the command named `key` must produce the key the engine consumes.
#[test]
fn key_produces_the_hmac_key_the_engine_reads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cmd = bin();
    cmd.current_dir(dir.path()).args(["key"]);
    isolated(&mut cmd);
    let out = cmd.output().expect("run rigorix key");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("export RIGORIX_HMAC_KEY="),
        "the key must be named with the variable the engine reads: {stdout}"
    );
    let key = stdout
        .split("export RIGORIX_HMAC_KEY=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("key value on the export line");
    assert_eq!(key.len(), 64, "32-byte HMAC key, hex-encoded: {key}");
    assert!(
        key.chars().all(|c| c.is_ascii_hexdigit()),
        "the key must be hex: {key}"
    );
    assert!(
        !stdout.contains("API key"),
        "the command must not call a UUID an API key: {stdout}"
    );
}

/// #1237: doctor prints the configured store and whether its records verify.
#[test]
fn doctor_reports_the_approval_store_and_key_agreement() {
    let dir = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside store dir");
    let store = outside.path().join("approvals.json");
    let ledger = outside.path().join("consumed.json");
    let mut cmd = bin();
    cmd.current_dir(dir.path()).args(["doctor"]);
    isolated(&mut cmd);
    cmd.env("RIGORIX_APPROVAL_BINDING", "1")
        .env("RIGORIX_HMAC_KEY", "doctor-test-run-key")
        .env("RIGORIX_APPROVAL_STORE", &store)
        .env("RIGORIX_APPROVAL_CONSUMPTION_LEDGER", &ledger);
    let out = cmd.output().expect("run rigorix doctor");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains(&store.display().to_string()),
        "doctor must print the store path: {stdout}"
    );
    assert!(
        stdout.contains("0 record(s) verify"),
        "an empty store verifies: {stdout}"
    );
    assert!(
        stdout.contains("outside agent-writable workspace"),
        "an external store must be classified as such: {stdout}"
    );
    assert!(
        stdout.contains(&ledger.display().to_string()),
        "doctor must name the consumption ledger: {stdout}"
    );
    assert!(
        stdout.contains("no history yet"),
        "a first run has no ledger history and must say so: {stdout}"
    );
    assert!(stdout.contains("keys agree"), "{stdout}");
    assert!(
        !stdout.contains("doctor-test-run-key"),
        "never print the key"
    );
}

/// #1238: two configured key sources that disagree are a composition error,
/// reported without either value.
#[test]
fn doctor_refuses_two_different_run_keys_without_printing_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("rigorix.toml"),
        "audit_hmac_key = \"config-key-value\"\n",
    )
    .expect("write rigorix.toml");
    let mut cmd = bin();
    cmd.current_dir(dir.path()).args(["doctor"]);
    isolated(&mut cmd);
    cmd.env("RIGORIX_APPROVAL_BINDING", "1")
        .env("RIGORIX_HMAC_KEY", "env-key-value");
    let out = cmd.output().expect("run rigorix doctor");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(!out.status.success(), "a key mismatch must fail: {report}");
    assert!(report.contains("MISCONFIGURED"), "{report}");
    assert!(report.contains("RIGORIX_HMAC_KEY"), "{report}");
    assert!(report.contains("audit_hmac_key"), "{report}");
    assert!(!report.contains("config-key-value"), "{report}");
    assert!(!report.contains("env-key-value"), "{report}");
}

/// #1247: migration is explicit, reports what it adopted, and writes the form
/// the loader will accept. The loader itself refuses a bare map.
#[test]
fn approve_ledger_migrate_adopts_a_legacy_map_and_reports_the_count() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("legacy-consumption.json");
    let store = dir.path().join("approvals.json");
    let node = "11111111-1111-1111-1111-111111111111";
    let nonce = "22222222-2222-2222-2222-222222222222";
    std::fs::write(
        &ledger,
        format!(
            r#"{{"{node}:{nonce}":{{"node_id":"{node}","nonce":"{nonce}","status":"consumed","recorded_at":"2026-10-31T00:00:00Z"}}}}"#
        ),
    )
    .expect("write legacy ledger");

    let run = |args: &[&str]| {
        let mut cmd = bin();
        cmd.current_dir(dir.path()).args(args);
        isolated(&mut cmd);
        cmd.env("RIGORIX_HMAC_KEY", "migration-test-key")
            .env("RIGORIX_APPROVAL_STORE", &store)
            .env("RIGORIX_APPROVAL_CONSUMPTION_LEDGER", &ledger)
            .output()
            .expect("run rigorix")
    };

    let out = run(&["approve", "ledger", "migrate"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "migration must succeed: {stdout}");
    assert!(
        stdout.contains("adopted 1 marker(s); the store was not found at"),
        "the command must say the comparison could not run: {stdout}"
    );
    assert!(
        stdout.contains("nothing was compared"),
        "the command must not imply a comparison happened: {stdout}"
    );
    assert!(
        !stdout.contains("of 0"),
        "1 of 0 is not a readable comparison: {stdout}"
    );
    let sealed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ledger).expect("read sealed"))
            .expect("sealed json");
    assert_eq!(sealed["version"], 1);
    assert!(sealed["mac"].as_str().is_some_and(|mac| !mac.is_empty()));
    assert!(sealed["entries"].get(format!("{node}:{nonce}")).is_some());

    let again = run(&["approve", "ledger", "migrate"]);
    let stdout = String::from_utf8_lossy(&again.stdout);
    assert!(again.status.success(), "{stdout}");
    assert!(
        stdout.contains("already sealed; 1 marker(s) verify; the store was not found at"),
        "an idempotent migration must say the MAC was checked and the store compared: {stdout}"
    );

    let accepted = run(&["approve", "ledger", "migrate", "--accept-missing"]);
    assert!(accepted.status.success(), "the flag must be accepted");
    assert!(
        String::from_utf8_lossy(&accepted.stdout).contains("already sealed"),
        "a sealed ledger remains sealed even with the flag"
    );
}

#[test]
fn approve_ledger_migrate_with_an_empty_store_says_nothing_was_compared() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("legacy-consumption.json");
    let store = dir.path().join("approvals.json");
    std::fs::write(&store, "{}").expect("store present and empty");
    std::fs::write(
        &ledger,
        r#"{"11111111-1111-1111-1111-111111111111:22222222-2222-2222-2222-222222222222":{"node_id":"11111111-1111-1111-1111-111111111111","nonce":"22222222-2222-2222-2222-222222222222","status":"consumed","recorded_at":"2026-10-31T00:00:00Z"}}"#,
    )
    .expect("write legacy ledger");

    let mut cmd = bin();
    cmd.current_dir(dir.path())
        .args(["approve", "ledger", "migrate"]);
    isolated(&mut cmd);
    let out = cmd
        .env("RIGORIX_HMAC_KEY", "migration-test-key")
        .env("RIGORIX_APPROVAL_STORE", &store)
        .env("RIGORIX_APPROVAL_CONSUMPTION_LEDGER", &ledger)
        .output()
        .expect("run rigorix");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("the store holds no terminal record(s), so nothing was compared"),
        "{stdout}"
    );
    assert!(!stdout.contains("of 0"), "{stdout}");
}

fn migration_command(
    dir: &std::path::Path,
    store: &std::path::Path,
    ledger: &std::path::Path,
    key: &str,
) -> Command {
    let mut cmd = bin();
    cmd.current_dir(dir).args(["approve", "ledger", "migrate"]);
    isolated(&mut cmd);
    cmd.env("RIGORIX_HMAC_KEY", key)
        .env("RIGORIX_APPROVAL_STORE", store)
        .env("RIGORIX_APPROVAL_CONSUMPTION_LEDGER", ledger);
    cmd
}

#[test]
fn approve_ledger_migrate_without_a_ledger_is_a_successful_no_op() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");

    let out = migration_command(dir.path(), &store, &ledger, "migration-test-key")
        .output()
        .expect("run rigorix");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "nothing to migrate is not a failure: {stdout}"
    );
    assert!(stdout.contains("there is no ledger at"), "{stdout}");
    assert!(stdout.contains("nothing to migrate"), "{stdout}");
    assert!(!ledger.exists(), "a no-op must not create the ledger");
}

#[test]
fn approve_ledger_migrate_with_an_empty_ledger_is_a_successful_no_op() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    std::fs::write(&ledger, "").expect("empty ledger");

    let out = migration_command(dir.path(), &store, &ledger, "migration-test-key")
        .output()
        .expect("run rigorix");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "an empty ledger is not a failure: {stdout}"
    );
    assert!(stdout.contains("is empty; nothing to migrate"), "{stdout}");
    assert_eq!(std::fs::read_to_string(&ledger).unwrap(), "");
}

#[test]
fn a_ledger_read_failure_names_the_ledger_not_the_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("ledger-as-directory");
    std::fs::create_dir(&ledger).expect("make unreadable ledger path");

    let out = migration_command(dir.path(), &store, &ledger, "migration-test-key")
        .output()
        .expect("run rigorix");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "an unreadable ledger is a failure");
    assert!(
        stderr.contains("approval ledger migration failed"),
        "the ledger, not the store, is the subject: {stderr}"
    );
    assert!(stderr.contains("ledger-as-directory"), "{stderr}");
    assert!(
        !stderr.contains("approval store unavailable"),
        "the store is not what failed: {stderr}"
    );
}

#[test]
fn a_corrupt_or_rekeyed_ledger_still_fails_and_names_the_ledger() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let corrupt = dir.path().join("corrupt.json");
    std::fs::write(&corrupt, "{ this is not json").expect("write corrupt ledger");

    let out = migration_command(dir.path(), &store, &corrupt, "migration-test-key")
        .output()
        .expect("run rigorix");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "damage is not a no-op");
    assert!(stderr.contains("corrupt.json"), "{stderr}");
    assert!(
        stderr.contains("approval ledger migration failed"),
        "{stderr}"
    );

    let rekeyed = dir.path().join("rekeyed.json");
    std::fs::write(
        &rekeyed,
        r#"{"11111111-1111-1111-1111-111111111111:22222222-2222-2222-2222-222222222222":{"node_id":"11111111-1111-1111-1111-111111111111","nonce":"22222222-2222-2222-2222-222222222222","status":"consumed","recorded_at":"2026-10-31T00:00:00Z"}}"#,
    )
    .expect("bare legacy map");
    let first = migration_command(dir.path(), &store, &rekeyed, "key-one")
        .output()
        .expect("seal under the first key");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let second = migration_command(dir.path(), &store, &rekeyed, "key-two")
        .output()
        .expect("run under a different key");
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        !second.status.success(),
        "a MAC mismatch must stay a failure"
    );
    assert!(stderr.contains("MAC mismatch"), "{stderr}");
    assert!(stderr.contains("rekeyed.json"), "{stderr}");
}
