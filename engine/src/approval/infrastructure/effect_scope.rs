//! Effect-scope oracle (ADR-011 R5) — records effects via git.
//!
//! @canonical .pi/architecture/modules/approval.md#scopeviolation
//! Implements: ISSUE slice — R5 effect-scope verification oracle
//!
//! The approved intent declares the effect scope (`declared_scope`). The
//! oracle snapshots the repository's actual changed-path set via git and
//! reports it so `ScopeViolation::out_of_scope` can flag effects outside the
//! declared scope. Git is the honest oracle: engine-visible `file_paths`
//! alone would miss side-effects from `run_command` scripts.
//!
//! # Contract
//! - `snapshot` captures the current changed-path set (tracked changes in the
//!   working tree **and** the index, plus untracked files), `diff` returns the
//!   paths changed between two snapshots — wire as: snapshot once pre-dispatch
//!   (post-approval) and compare after each consequential step, then
//!   `diff(baseline, post)` is the effect set of the run so far.
//! - When git is unavailable / not a repository the oracle returns
//!   `ApprovalError::ScopeVerificationUnavailable` (retriable) — callers skip
//!   with an explicit marker, never silently.
//!
//! # What this oracle is not (and what its baseline means)
//!
//! **Wired, and fed by plan creation (#1099, #1175).** All three entry points
//! (CLI, GitHub Action, MCP host) set
//! `ParallelExecutionFactoryConfig::approval_repo_path`, so the oracle runs for
//! approval-gated steps. Plan creation now writes `declared_scope` into the
//! resolved intent for actions that name a file (`file_write`, `file_append`,
//! `file_patch`, `git_stage`), so the check has evidence without guessing. A
//! `run_command` step declares no scope: a command string is not a path list,
//! and a guessed scope would create false violations. The check is still gated
//! on the evidence — enabling the oracle cannot invent a violation.
//!
//! **What #1072 left out, and how that was recorded.** #1072 was closed
//! `COMPLETED` with three parts; two landed (the oracle, the staged-changes
//! snapshot) and the third — "disabled in every production entry point" — did
//! not. Closing it without recording the split made the tracker assert something
//! the code contradicted. The wiring is #1099; `rigorix doctor` reports the
//! `effect scope` row so the wiring is readable rather than a flag in three
//! builders.
//!
//! **Per-loop baseline, not per-step attribution.** The baseline is taken once
//! at the start of `run_dispatch_loop`. Effects are therefore attributable to the
//! run, and only soundly to a specific step when steps execute one at a time. With
//! concurrent approved steps the check reports the union, and the violating step
//! is whichever completes first. A pre-existing dirty tree appears in every
//! comparison because the baseline is dirty too — it is *excluded* by the diff,
//! but a file the author keeps editing during a run is reported as the run's effect.
//!
//! **Paths, not content.** A modification inside an already-changed file is
//! indistinguishable from the change that was already there. Content-level
//! attribution needs per-step commits, which this oracle deliberately does not do.

// #1083: approved process-spawning sink — fixed-argv `git diff` for the effect snapshot.
// See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
#![allow(clippy::disallowed_types)]

use std::path::Path;
use std::process::Command;

use crate::approval::domain::ApprovalError;

/// A point-in-time set of changed paths in the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeSnapshot {
    /// Paths with tracked changes — staged (`git diff --cached`) **and**
    /// unstaged (`git diff`). A step that stages its edit and commits later is
    /// visible; checking only the working tree made it invisible (#1072).
    pub modified: Vec<String>,
    /// Untracked paths (`git ls-files --others --exclude-standard`).
    pub untracked: Vec<String>,
}

impl ChangeSnapshot {
    /// All changed paths (modified ∪ untracked), sorted for determinism.
    pub fn all(&self) -> Vec<String> {
        let mut all: Vec<String> = self.modified.clone();
        all.extend(self.untracked.iter().cloned());
        all.sort();
        all.dedup();
        all
    }
}

/// Effect-scope oracle: records actual effects via the git working tree.
#[derive(Debug, Default)]
pub struct GitDiffEffectOracle;

impl GitDiffEffectOracle {
    /// Snapshot the repository's current changed-path set.
    ///
    /// # Errors
    /// - `ApprovalError::ScopeVerificationUnavailable` — git missing, the
    ///   path is not a repository, or a git call failed.
    pub fn snapshot(&self, repo: &Path) -> Result<ChangeSnapshot, ApprovalError> {
        // Both halves of the tracked change set: the working tree and the index.
        // `git diff` alone reports only unstaged modifications, so a step that
        // stages its edit slipped past the oracle (#1072).
        let mut modified = run_git(repo, &["diff", "--name-only"])?;
        modified.extend(run_git(repo, &["diff", "--cached", "--name-only"])?);
        modified.sort();
        modified.dedup();
        let untracked = run_git(repo, &["ls-files", "--others", "--exclude-standard"])?;
        Ok(ChangeSnapshot {
            modified,
            untracked,
        })
    }

    /// Effects recorded between two snapshots (pre-dispatch → post-execution).
    ///
    /// Returns paths present in `post` but absent in `pre` — the net change
    /// set attributable to the executed run. Deletions are reported by git
    /// diff in the `modified` list only when the file was tracked.
    pub fn diff(&self, pre: &ChangeSnapshot, post: &ChangeSnapshot) -> Vec<String> {
        let pre_set: std::collections::HashSet<String> = pre.all().into_iter().collect();
        let mut net: Vec<String> = post
            .all()
            .into_iter()
            .filter(|p| !pre_set.contains(p))
            .collect();
        net.sort();
        net
    }
}

fn run_git(repo: &Path, args: &[&str]) -> Result<Vec<String>, ApprovalError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| {
            ApprovalError::ScopeVerificationUnavailable(format!(
                "git unavailable for {}: {e}",
                repo.display()
            ))
        })?;
    if !output.status.success() {
        return Err(ApprovalError::ScopeVerificationUnavailable(format!(
            "git {args:?} failed in {}: {}",
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn temp_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let init = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .arg(dir.path().to_str().unwrap())
            .output()
            .expect("git init");
        assert!(init.status.success());
        Command::new("git")
            .args([
                "-C",
                dir.path().to_str().unwrap(),
                "config",
                "user.email",
                "t@t",
            ])
            .status()
            .expect("config");
        Command::new("git")
            .args([
                "-C",
                dir.path().to_str().unwrap(),
                "config",
                "user.name",
                "t",
            ])
            .status()
            .expect("config");
        dir
    }

    fn commit_all(repo: &Path) {
        let repo = repo.to_str().unwrap();
        Command::new("git")
            .args(["-C", repo, "add", "-A"])
            .status()
            .unwrap();
        Command::new("git")
            .args(["-C", repo, "commit", "-q", "-m", "snapshot"])
            .status()
            .unwrap();
    }

    #[test]
    fn oracle_reports_run_effects_outside_declared_scope() {
        let repo = temp_repo();
        std::fs::create_dir_all(repo.path().join("docs")).unwrap();
        std::fs::write(repo.path().join("docs/readme.md"), "x").unwrap();
        commit_all(repo.path());
        let oracle = GitDiffEffectOracle;

        // Pre-dispatch snapshot: clean tree.
        let pre = oracle.snapshot(repo.path()).unwrap();
        assert!(pre.all().is_empty());

        // The run side-effects src/auth.ts (a run_command script) — git sees it.
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::write(repo.path().join("src/auth.ts"), "tampered").unwrap();
        let post = oracle.snapshot(repo.path()).unwrap();

        let effects = oracle.diff(&pre, &post);
        assert!(effects.contains(&"src/auth.ts".to_string()));

        // Declared scope was docs/ only → the oracle feeds the violation check.
        let declared = vec!["docs/".to_string()];
        let violation = crate::approval::domain::ScopeViolation::detect(
            uuid::Uuid::new_v4(),
            "run_script".into(),
            &declared,
            &effects,
            chrono::Utc::now(),
        )
        .expect("side-effect outside declared scope");
        assert_eq!(violation.out_of_scope, vec!["src/auth.ts".to_string()]);
    }

    /// #1072: a step that stages its edit (`git add`) and commits later was
    /// invisible, because `git diff --name-only` reports only unstaged changes.
    #[test]
    fn staged_changes_are_part_of_the_snapshot() {
        let repo = temp_repo();
        std::fs::write(repo.path().join("tracked.txt"), "original").unwrap();
        commit_all(repo.path());
        let oracle = GitDiffEffectOracle;

        let pre = oracle.snapshot(repo.path()).unwrap();
        assert!(pre.all().is_empty(), "clean tree");

        // Modify and stage, but do not commit: the working tree is now clean
        // relative to HEAD, so `git diff` alone sees nothing.
        std::fs::write(repo.path().join("tracked.txt"), "staged tamper").unwrap();
        let status = Command::new("git")
            .args(["-C", repo.path().to_str().unwrap(), "add", "tracked.txt"])
            .status()
            .expect("git add");
        assert!(status.success());

        let post = oracle.snapshot(repo.path()).unwrap();
        assert_eq!(post.modified, vec!["tracked.txt".to_string()]);
        assert_eq!(
            oracle.diff(&pre, &post),
            vec!["tracked.txt".to_string()],
            "a staged effect must not be invisible"
        );
    }

    #[test]
    fn oracle_is_unavailable_outside_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        let oracle = GitDiffEffectOracle;
        let err = oracle.snapshot(dir.path()).unwrap_err();
        assert!(matches!(
            err,
            ApprovalError::ScopeVerificationUnavailable(_)
        ));
    }
}
