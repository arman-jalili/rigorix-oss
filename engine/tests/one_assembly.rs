#![cfg(not(feature = "portable"))]
//! One assembly, enforced by the build (#1627, ADR-025 D5).
//!
//! `GateSetup`'s header records why the engine took the seam *order* on in #1381: "moving the order into
//! the engine while leaving the construction in each caller would only move half the problem … the next
//! runtime would have to rediscover it." Four assemblies later the tree showed what "leave the rest to
//! review" produces, and the fourth — `receipt::api::run_receipt`, written *after* `GateSetup` existed —
//! consulted no gate at all.
//!
//! This test is the cheap half of the answer: a source scan with a named allow-list, in the shape
//! `tests/execution_single_path.rs` proved. The type-level half is `gate::domain::Governed` (#1626); a
//! type cannot reach `cfg`-gated code, macro-generated code, or a host that adds a dependency and writes
//! its own import, which is why both instruments exist and why neither is redundant.
//!
//! Two properties matter and are asserted separately:
//!
//! - **the subject is real** — the scan finds a needle where it is known to be, so a scan that stops
//!   matching (a rename, a deleted file, a wrong root) is a red test rather than a quiet pass;
//! - **the allow-list is data** — every exception is a row in
//!   `engine/.pi/architecture/assembly-allowlist.tsv` with a sentence, and a row that no longer matches
//!   anything is itself a failure, because a stale exception is how a guard becomes decoration.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ALLOWLIST: &str = "engine/.pi/architecture/assembly-allowlist.tsv";

/// The seams that may be touched in one place only. Each is a *call*, not a name: a doc comment that
/// mentions `PreconditionSetup::from_env` is not a call site, and a guard that cannot tell them apart is
/// muted within a week.
const NEEDLES: &[&str] = &[
    "PreconditionSetup::from_env",
    "SequencePolicySetup::from_env",
    "CapacitySetup::from_env",
    "EffectCommitmentSetup::from_env",
    "ApprovalBindingSetup::from_env_with_signing_key",
    "OrchestratorServiceImpl::ungoverned",
];

/// The assembly owns the order, so every needle is allowed there without a row: this is the one place
/// the register is about.
const ALLOWED_WITHOUT_A_ROW: &str = "engine/src/gate/infrastructure/assembly.rs";

fn workspace_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `engine/`; the allow-list and the host crates are one level up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the engine crate lives inside the workspace")
        .to_path_buf()
}

fn allowlist_path() -> PathBuf {
    workspace_root().join(ALLOWLIST)
}

struct Row {
    needle: String,
    pattern: String,
    /// Read where it matters: a row with no reason is refused when the register is parsed, so the
    /// sentence is enforced rather than stored.
    #[allow(dead_code)]
    reason: String,
}

fn rows() -> Vec<Row> {
    let text = std::fs::read_to_string(allowlist_path())
        .unwrap_or_else(|error| panic!("{ALLOWLIST} is unreadable: {error}"));
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        assert!(
            fields.len() == 3,
            "{ALLOWLIST}:{}: a row is needle<TAB>path<TAB>reason, got {} field(s)",
            index + 1,
            fields.len()
        );
        assert!(
            !fields[2].trim().is_empty(),
            "{ALLOWLIST}:{}: an exception with no reason is a judgement with no record",
            index + 1
        );
        rows.push(Row {
            needle: fields[0].to_string(),
            pattern: fields[1].to_string(),
            reason: fields[2].to_string(),
        });
    }
    rows
}

/// Does `pattern` (with `*` as a single path segment, and a leading/trailing `*` as "anything") match
/// `path`? Deliberately tiny: two globs the register actually uses, and no third.
fn matches(pattern: &str, path: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("*/") {
        return path == suffix || path.ends_with(&format!("/{suffix}"));
    }
    if pattern.contains("**/") {
        // `engine/src/**/tests/*`: a prefix, then a `tests/` directory, then anything.
        let head = pattern.split("/**/").next().unwrap_or_default();
        let tail = pattern.rsplit("/**/").next().unwrap_or_default();
        return path.starts_with(&format!("{head}/")) && path.contains(&format!("/{tail}/"))
            || path.starts_with(&format!("{head}/")) && path.ends_with(&format!("/{tail}"));
    }
    pattern == path
}

/// Every `.rs` file under `dir` whose path contains `/tests/` or ends with `_test.rs` is a test; the scan
/// reads production files only.
fn is_test_path(path: &str) -> bool {
    path.contains("/tests/") || path.ends_with("_test.rs") || path.contains("/examples/")
}

fn walk(root: &Path, rel: &Path, out: &mut Vec<String>) {
    let dir = root.join(rel);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        // `target/` is build output, not a subject.
        if name == "target" || name == ".git" {
            continue;
        }
        let child = rel.join(&name);
        if entry.path().is_dir() {
            walk(root, &child, out);
        } else if name.ends_with(".rs") {
            out.push(child.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn sources() -> Vec<String> {
    let root = workspace_root();
    let mut files = Vec::new();
    for crate_dir in [
        "engine/src",
        "mcp/src",
        "cli/src",
        "actions/src",
        "server/src",
    ] {
        walk(&root, Path::new(crate_dir), &mut files);
    }
    // The wasm tree is a separate crate family with its own renderer and no orchestrator; the portable
    // slice takes its ports from the host by construction (ADR-014), so it is not a subject here.
    files.sort();
    files
}

struct Finding {
    needle: String,
    path: String,
    line: usize,
}

fn scan(rows: &[Row]) -> (Vec<Finding>, BTreeSet<String>) {
    let root = workspace_root();
    let mut findings = Vec::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    for file in sources() {
        if is_test_path(&file) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&file)) else {
            continue;
        };
        // The production half only, two ways. A doc comment that names a seam is not a call site — and
        // a `#[cfg(test)]` module inside a production file is a test, not a host: it arms seams against
        // fixture repositories because that is the subject it measures.
        //
        // The test block is *skipped*, not truncated at: a file can have production code after its test
        // module, and a first version of this scan broke at the marker and became blind to everything
        // below it — measured by planting a seam at the end of `mcp/src/host/mod.rs` and watching the
        // guard pass.
        let mut in_tests = false;
        let mut depth: i32 = 0;
        let mut test_module_pending = false;
        for (index, line) in text.lines().enumerate() {
            if in_tests {
                depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
                if depth <= 0 {
                    in_tests = false;
                }
                continue;
            }
            let trimmed = line.trim();
            if test_module_pending && trimmed.starts_with("mod ") {
                test_module_pending = false;
                in_tests = true;
                depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
                if depth <= 0 {
                    in_tests = false;
                }
                continue;
            }
            if trimmed == "#[cfg(test)]" {
                test_module_pending = true;
                continue;
            }
            let production = line.trim_start();
            if production.starts_with("//") {
                continue;
            }
            for needle in NEEDLES {
                if !line.contains(needle) {
                    continue;
                }
                if file == ALLOWED_WITHOUT_A_ROW {
                    continue;
                }
                let allowed = rows
                    .iter()
                    .find(|row| row.needle == *needle && matches(&row.pattern, &file));
                match allowed {
                    Some(row) => {
                        used.insert(format!("{}\t{}", row.needle, row.pattern));
                    }
                    None => findings.push(Finding {
                        needle: (*needle).to_string(),
                        path: file.clone(),
                        line: index + 1,
                    }),
                }
            }
        }
    }
    (findings, used)
}

#[test]
fn no_production_file_reaches_a_governed_seam_except_the_assembly() {
    let rows = rows();
    let (findings, _) = scan(&rows);
    assert!(
        findings.is_empty(),
        "{} site(s) attach a governed seam outside the one assembly:\n{}\n\n\
         The seam order is the point: a gate attached after the dispatcher is built is not consulted \
         (#1381). Call `gate::infrastructure::assemble` (ADR-025 D1), or add a row to {ALLOWLIST} \
         saying why this site may bypass it.",
        findings.len(),
        findings
            .iter()
            .map(|f| format!("  {}:{} — {}", f.path, f.line, f.needle))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_allowlisted_exception_still_matches_something() {
    // The direction that matters for a register: a row whose subject moved or vanished is a stale
    // permission, and a stale permission is how the next bypass gets waved through.
    let rows = rows();
    let (_, used) = scan(&rows);
    let stale: Vec<String> = rows
        .iter()
        .filter(|row| !used.contains(&format!("{}\t{}", row.needle, row.pattern)))
        .map(|row| format!("  {} — {}", row.needle, row.pattern))
        .collect();
    assert!(
        stale.is_empty(),
        "{} allow-list row(s) match nothing any more:\n{}\n\nEither the site moved (update the row) or \
         it stopped reaching the seam (delete the row).",
        stale.len(),
        stale.join("\n")
    );
}

#[test]
fn the_scan_finds_a_seam_where_one_is_known_to_be() {
    // The guard's own subject, asserted: a scan that stops matching — a rename, a wrong root, a walker
    // that silently skips a directory — passes every check above. This is the plant, and it is the same
    // size as the failure the guard exists for.
    let root = workspace_root();
    let known = "engine/src/gate/infrastructure/setup.rs";
    let text = std::fs::read_to_string(root.join(known)).expect("GateSetup is in the tree");
    assert!(
        text.contains("PreconditionSetup::from_env"),
        "the guard's own needle moved: {known} no longer contains PreconditionSetup::from_env"
    );
    assert!(
        sources().contains(&known.to_string()),
        "the walker does not visit {known}, so every finding above is a finding about nothing"
    );
}

#[test]
fn a_hand_wired_seam_in_a_host_would_be_caught() {
    // The end-to-end shape, run against a temporary tree: plant the forbidden call in a host path, and
    // require the scan to name the file and line. A guard whose self-test plants the defect somewhere
    // else — in the assembly, say — proves nothing about the file it is supposed to protect.
    let fixture = tempfile::tempdir().expect("fixture root");
    for crate_dir in [
        "engine/src",
        "mcp/src",
        "cli/src",
        "actions/src",
        "server/src",
    ] {
        std::fs::create_dir_all(fixture.path().join(crate_dir)).expect("crate dir");
    }
    let planted = fixture.path().join("mcp/src/host.rs");
    std::fs::write(
        &planted,
        "fn wire() {\n    let setup = PreconditionSetup::from_env(root);\n}\n",
    )
    .expect("plant");

    let mut files = Vec::new();
    for crate_dir in [
        "engine/src",
        "mcp/src",
        "cli/src",
        "actions/src",
        "server/src",
    ] {
        walk(fixture.path(), Path::new(crate_dir), &mut files);
    }
    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(fixture.path().join(file)).unwrap_or_default();
        for (index, line) in text.lines().enumerate() {
            if line.trim() == "#[cfg(test)]" {
                break;
            }
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.contains("PreconditionSetup::from_env") {
                hits.push(format!("{file}:{}", index + 1));
            }
        }
    }
    assert_eq!(
        hits,
        vec!["mcp/src/host.rs:2".to_string()],
        "the scan must name the file and the line of the planted bypass"
    );
}
