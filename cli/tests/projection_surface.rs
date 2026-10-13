//! ISSUE-PROJECTION-SURFACES (#1006) — the authority-fact projection is
//! reachable and verifiable offline.
//!
//! `audit projection` emits a bundle (projection + the signed history it was
//! derived from); `audit verify-projection` re-derives from that history and
//! exits non-zero when the projection does not match, or when the history is not
//! an intact chain. No engine process and no operator key are involved.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rigorix-cli"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("spawn rigorix-cli")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn projection_is_emitted_verified_and_redacted() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bundle = dir.path().join("bundle.json");
    let bundle_arg = bundle.to_str().expect("utf8 path");

    // 1. Emit a bundle a third party can verify by re-derivation.
    let out = run(
        dir.path(),
        &[
            "audit",
            "projection",
            "--subject",
            "authority:revocation-status",
            "-o",
            bundle_arg,
        ],
    );
    assert!(out.status.success(), "emit failed: {}", stderr(&out));
    let text = fs::read_to_string(&bundle).expect("bundle written");

    // 2. Redaction (spec NC + ADR-017): the projection is shared with auditors,
    //    so a path or username leak is a real disclosure.
    assert!(!text.contains("/Users/"), "username/path leak in bundle");
    assert!(!text.contains("/home/"), "username/path leak in bundle");
    assert!(!text.contains("file://"), "path leak in bundle");

    // 3. Verify by re-derivation — no engine, no key.
    let out = run(dir.path(), &["audit", "verify-projection", bundle_arg]);
    assert!(out.status.success(), "verify failed: {}", stderr(&out));

    // 4. A tampered projection is DETECTED, and the command exits non-zero.
    let mut value: serde_json::Value = serde_json::from_str(&text).expect("bundle is json");
    value["projection"]["facts"][0]["epistemic_status"] =
        serde_json::Value::String("ESTABLISHED".into());
    value["projection"]["facts"][0]["outcome"] = serde_json::Value::String("passed".into());
    fs::write(&bundle, serde_json::to_string_pretty(&value).unwrap()).expect("write tampered");

    let out = run(dir.path(), &["audit", "verify-projection", bundle_arg]);
    assert!(
        !out.status.success(),
        "tampering was NOT detected; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn verify_rejects_a_bundle_that_is_not_json() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bundle = dir.path().join("bundle.json");
    fs::write(&bundle, b"not a bundle").expect("write garbage");

    let out = run(
        dir.path(),
        &["audit", "verify-projection", bundle.to_str().unwrap()],
    );
    assert!(!out.status.success(), "garbage bundle must not verify");
}
