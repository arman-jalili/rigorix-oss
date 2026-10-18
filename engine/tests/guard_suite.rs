//! Guards for the failure classes catalogued in #1082.
//!
//! Each class is a property of the codebase, not a pattern of a known defect, so
//! each guard is written against the property and its own falsification is
//! recorded in `.pi/architecture/failure-class-ledger.md`.
//!
//! These run as ordinary tests on purpose: they need no workflow step, no
//! sibling checkout and no network, so they cannot rot into "CI-only" checks that
//! nobody notices are disabled.

use std::path::{Path, PathBuf};

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn sources() -> Vec<(String, String)> {
    let mut files = Vec::new();
    walk(&src_root(), &mut files);
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(src_root())
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            (rel, std::fs::read_to_string(&p).unwrap_or_default())
        })
        .collect()
}

/// Class C2 — the policy examines a different string than the executor.
///
/// The structural half of the guard: a shell interpreter may only be named in
/// the sites whose command string is validated *as executed*. argv sinks (git,
/// hooks with fixed argv) do not use `sh -c`. Adding a shell site is fine — but
/// it has to appear here, next to the policy that governs it, so the inventory
/// and the code move together.
#[test]
fn c2_shell_interpreters_are_confined_to_validated_sites() {
    // (file, why the string handed to the shell is the string that was checked)
    const APPROVED: &[(&str, &str)] = &[
        (
            "tools/application/run_command_tool.rs",
            "allowlist + control-operator refusal run on the same intent string",
        ),
        (
            "execution_engine/application/service_impl/tools.rs",
            "exec_run_command: allowlist + control-operator refusal, then the same string",
        ),
        (
            "hooks/application/hook_runner_impl.rs",
            "operator-configured hook command (trusted config), env cleared",
        ),
    ];

    let mut found: Vec<String> = sources()
        .into_iter()
        .filter(|(_, body)| {
            body.contains("Command::new(\"sh\")") || body.contains("Command::new(\"/bin/sh\")")
        })
        .map(|(path, _)| path)
        .collect();
    found.sort();

    let mut expected: Vec<String> = APPROVED.iter().map(|(p, _)| p.to_string()).collect();
    expected.sort();

    assert_eq!(
        found, expected,
        "a shell sink was added or removed without updating the inventory: \
         every `sh -c` site must be listed with the policy that validates its string \
         (see tool-system.md#process-spawning-sinks)"
    );
}

/// Class C6 — a requested guarantee silently degrades.
///
/// `*Setup::from_env` used to return `Option<Self>`, so "not requested" and
/// "requested but unsatisfiable" were the same value and the caller could not
/// tell them apart (#1070). The shape is now `Result<Option<Self>, _>`; this
/// guard fails if a new constructor reintroduces the ambiguous shape.
#[test]
fn c6_setup_constructors_do_not_hide_an_unsatisfiable_request() {
    let mut offenders = Vec::new();
    for (path, body) in sources() {
        if path.contains("tests") || path.contains("test_") {
            continue;
        }
        for line in body.lines() {
            let line = line.trim();
            if !line.starts_with("pub fn ") || !line.contains("from_env") {
                continue;
            }
            // Multi-line signatures put the return type on a later line; only
            // the single-line shape can be judged here, and that is the shape a
            // bare `Option` return actually takes in practice.
            if let Some(ret) = line.split("->").nth(1) {
                let ret = ret.trim();
                if ret.starts_with("Option<") {
                    offenders.push(format!("{path}: {line}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a `*from_env` constructor returns a bare Option, so a requested-but-unsatisfiable \
         guarantee is indistinguishable from an unrequested one (class C6):\n{}",
        offenders.join("\n")
    );
}

/// Class C5 — a field that is configured, echoed, and never consulted.
///
/// Each entry is a field that exists to change a *decision*. It must appear in at
/// least one place that is not a DTO, not `Display`, and not a test — otherwise
/// it is decoration that reads like a control.
#[test]
fn c5_decision_fields_are_reachable_from_a_decision() {
    const DECISION_FIELDS: &[(&str, &str)] = &[
        ("window_seconds", "capacity ledger: bounded window aging"),
        ("lease_seconds", "capacity ledger: reservation lease"),
        ("timeout_ms", "precondition runner: wall-clock budget"),
        ("max_calls", "enforcement: per-execution tool-call cap"),
        ("require_immutable_check", "precondition: boundary strength"),
        (
            "hard_limit",
            "capacity ledger / enforcement: the cap itself",
        ),
    ];

    let sources = sources();
    let mut orphans = Vec::new();
    for (field, purpose) in DECISION_FIELDS {
        let consulted = sources.iter().any(|(path, body)| {
            if path.contains("dto") || path.contains("test") {
                return false;
            }
            body.lines().any(|line| {
                let t = line.trim();
                // Skip the declaration line itself and pure serialization docs.
                if t.starts_with("///") || t.starts_with("//") {
                    return false;
                }
                t.contains(field)
                    && (t.contains("if ")
                        || t.contains("match ")
                        || t.contains("min(")
                        || t.contains("max(")
                        || t.contains("Duration::from_millis")
                        || t.contains("timeout(")
                        || t.contains("> ")
                        || t.contains("< ")
                        || t.contains("saturating")
                        || t.contains("for "))
            })
        });
        if !consulted {
            orphans.push(format!("{field} ({purpose})"));
        }
    }
    assert!(
        orphans.is_empty(),
        "declared decision fields with no reachable decision (class C5):\n{}",
        orphans.join("\n")
    );
}
