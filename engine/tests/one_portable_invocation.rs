//! #1641: the portable slice has one owner, and the pipeline calls it.
//!
//! `.pi/scripts/validate-portable-slice.sh` owns the subject — the crate as the slice compiles it,
//! with the warning policy that makes a dead producer visible. Before this, `ci.yml`'s `portable`
//! job restated the same `cargo check` and the same `RUSTFLAGS`, and `local-ci` ran the command
//! without the flag. Two owners of one subject disagreed silently for a week (#1626's outage).
//!
//! These tests are the source guard, in the shape `one_assembly.rs` established: a **registered
//! list** decides what may appear, not a clever pattern, and every allow-list row carries a reason.
//! A row that matches nothing is itself a failure, so the list cannot rot into a set of permissions
//! for code that no longer exists.
//!
//! What the scan does *not* do: it does not read comments as claims. A comment mentioning
//! `--features portable` is prose about the subject, and the check for prose is that it must be
//! true, not that it must be absent. The scan skips comment lines and says so here rather than
//! pretending it can parse YAML.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine/ has a parent")
        .to_path_buf()
}

/// Lines that legitimately name the slice's target or feature.
///
/// A row is `path<TAB>needle<TAB>reason`. Every row must match at least one line; a row that matches
/// nothing fails the test, and a line that matches no row fails too.
const ALLOWLIST: &str = "engine/.pi/architecture/portable-invocation-allowlist.tsv";

fn allowlist() -> Vec<(String, String, String)> {
    let path = workspace_root().join(ALLOWLIST);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} must exist: {e}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.splitn(3, '\t');
            match (parts.next(), parts.next(), parts.next()) {
                (Some(path), Some(needle), Some(reason)) if !reason.trim().is_empty() => {
                    (path.to_string(), needle.to_string(), reason.to_string())
                }
                _ => panic!("{ALLOWLIST}: a row needs path<TAB>needle<TAB>reason — got {line:?}"),
            }
        })
        .collect()
}

/// The lines of a file that are code, not comment.
fn code_lines(path: &Path) -> Vec<(usize, String)> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line.to_string()))
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.starts_with('#') && !trimmed.starts_with("//")
        })
        .collect()
}

/// Every code line in the pipeline that names the slice's target or its feature.
fn slice_invocations() -> Vec<(String, usize, String)> {
    let root = workspace_root();
    let workflow = root.join(".github/workflows/ci.yml");
    code_lines(&workflow)
        .into_iter()
        .filter(|(_, line)| {
            line.contains("features portable") || line.contains("wasm32-unknown-unknown")
        })
        .map(|(n, line)| (".github/workflows/ci.yml".to_string(), n, line))
        .collect()
}

#[test]
fn the_pipeline_calls_the_owner_instead_of_restating_it() {
    let rows = allowlist();
    let mut unmatched_rows = Vec::new();
    let mut unexplained = Vec::new();

    for (path, needle, reason) in &rows {
        let matched = code_lines(&workspace_root().join(path))
            .iter()
            .any(|(_, line)| line.contains(needle.as_str()));
        if !matched {
            unmatched_rows.push(format!("{path}: {needle:?} ({reason})"));
        }
    }

    for (path, number, line) in slice_invocations() {
        let explained = rows
            .iter()
            .any(|(row_path, needle, _)| *row_path == path && line.contains(needle.as_str()));
        if !explained {
            unexplained.push(format!("{path}:{number}: {}", line.trim()));
        }
    }

    assert!(
        unmatched_rows.is_empty(),
        "allow-list row(s) that match nothing — a permission for code that is gone:\n{}",
        unmatched_rows.join("\n")
    );
    assert!(
        unexplained.is_empty(),
        "the pipeline restates an invocation the script owns (#1641). Call \
         `.pi/scripts/validate-portable-slice.sh`, or add a row to {ALLOWLIST} that says why:\n{}",
        unexplained.join("\n")
    );
}

#[test]
fn the_warning_policy_lives_in_the_script_not_only_in_the_workflow() {
    // The defect: the job's `RUSTFLAGS: "-D warnings"` armed the pipeline's compile, and the local
    // script did not carry it — same command, different subject, opposite verdict. The workflow flag
    // stays (it arms every step in the job), so what must hold is that the *script* carries its own
    // copy: if someone deletes the workflow's flag, the portable check must still see a dead producer.
    let script =
        fs::read_to_string(workspace_root().join(".pi/scripts/validate-portable-slice.sh"))
            .expect("the script exists");
    assert!(
        script.contains(r#"RUSTFLAGS="-D warnings""#),
        "the script must carry the pipeline's warning policy itself, not inherit it"
    );
    assert!(
        !script.contains("RUSTFLAGS=\"${RUSTFLAGS"),
        "and it must not inherit silently either — a silent inheritance is the same second owner"
    );
}

#[test]
fn a_restated_invocation_would_be_named_with_its_file_and_line() {
    // The guard must fail on the defect it exists for, and say where. This asserts the *shape* of the
    // report against a synthetic line, so the check below cannot pass merely because the tree is
    // currently clean: a scan that reports nothing on a tree with a violation is decoration.
    let rows: Vec<(String, String, String)> = vec![];
    let planted = (
        ".github/workflows/ci.yml".to_string(),
        243usize,
        "          cargo check --locked -p rigorix-engine --features portable".to_string(),
    );
    let explained = rows
        .iter()
        .any(|(path, needle, _)| *path == planted.0 && planted.2.contains(needle.as_str()));
    assert!(!explained, "a restated invocation is unexplained");
    let report = format!("{}:{}: {}", planted.0, planted.1, planted.2.trim());
    assert!(
        report.starts_with(".github/workflows/ci.yml:243:")
            && report.contains("--features portable"),
        "the report names the file, the line and the subject: {report}"
    );
}
