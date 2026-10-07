//! PreconditionRunner — deterministic argv execution of an operator check.
//!
//! @canonical .pi/architecture/modules/precondition.md#command-contract
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — PreconditionRunner trait + ProcessPreconditionRunner
//! Issue: #941 (ISSUE-CONSEQUENCE-GATING-3); contract frozen in #938
//!
//! The runner executes one precondition's `command` deterministically. The
//! contract is frozen (ADR-017 §R1):
//!
//! - **argv only**: `command` is an argv array, never a shell; no string
//!   interpolation of step values into argv. The engine never wraps the command
//!   in a shell (the operator may still choose to run `sh` explicitly).
//! - **stdin**: the JSON object `{ "precondition_id", "execution_id", "step",
//!   "tool", "parameters" }` (see
//!   [`PreconditionCheckInput`](crate::precondition::application::PreconditionCheckInput))
//! - **env**: `RIGORIX_PRECONDITION_ID`, `RIGORIX_EXECUTION_ID`,
//!   `RIGORIX_STEP_NAME`, `RIGORIX_TOOL`
//! - **exit 0** → `passed`; **non-zero** → `failed`; a signal termination →
//!   `error`
//! - **trust boundary**: `command[0]` must resolve **outside the
//!   agent-writable workspace**; if it resolves inside, the run is refused with
//!   [`PreconditionError::TrustBoundary`] (a check the agent can edit is no
//!   check)
//! - **boundary strength** (`require_immutable_check`): when set, the resolved
//!   check program and any declared `authority_path` are assessed for
//!   writability by the engine's effective UID (POSIX owner/group/world mode);
//!   a writable — or unreadable — boundary is refused with
//!   [`PreconditionError::Boundary`]. Default `false`: the assessment is still
//!   traced, but the run proceeds (the default boundary is path + hook)
//! - **timeout / spawn failure** → [`PreconditionError::Timeout`] /
//!   [`PreconditionError::Spawn`] (indeterminate → refuse, never pass)
//! - stdout is captured only when the precondition sets `capture_output = true`
//!   (truncated to [`MAX_CAPTURED_STDOUT_BYTES`]); it is never captured by
//!   default

use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::precondition::application::PreconditionCheckInput;
use crate::precondition::domain::{Precondition, PreconditionError, PreconditionOutcome};

/// Maximum number of stdout bytes recorded when `capture_output = true`.
pub const MAX_CAPTURED_STDOUT_BYTES: usize = 4_096;

/// The result of running one precondition check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreconditionRun {
    /// The distinct outcome: `passed` (exit 0), `failed` (non-zero exit or
    /// signal), or `error` (signal termination).
    pub outcome: PreconditionOutcome,
    /// Process exit code, when the process exited normally. `None` for a
    /// signal termination.
    pub exit_code: Option<i32>,
    /// Captured (truncated) stdout — present only when the precondition sets
    /// `capture_output = true`.
    pub stdout: Option<String>,
    /// Whether the resolved check program was writable by the engine's
    /// effective UID. `None` when the assessment could not be made (non-unix,
    /// or unreadable metadata with `require_immutable_check = false`).
    pub check_writable: Option<bool>,
    /// Whether the operator-declared `authority_path` was writable, when one
    /// was declared. `None` when absent or unassessable.
    pub authority_writable: Option<bool>,
    /// SHA-256 (`sha256:<hex>`) of the resolved check program bytes — the
    /// immutable identity of the check that ran, for signed attribution
    /// (ADR-017). `None` when the program could not be read.
    pub check_digest: Option<String>,
    /// SHA-256 (`sha256:<hex>`) of the operator-declared authority artifact,
    /// when one was declared. Contents are never recorded (SpanPrivacy); only
    /// the digest. `None` when absent or unreadable.
    pub authority_digest: Option<String>,
}

/// Deterministic precondition-check runner.
#[async_trait]
pub trait PreconditionRunner: Send + Sync {
    /// Run `precondition`'s command for the given check input.
    ///
    /// # Returns
    /// - `Ok(PreconditionRun { outcome: Passed, .. })` — exit 0
    /// - `Ok(PreconditionRun { outcome: Failed, .. })` — non-zero exit
    /// - `Ok(PreconditionRun { outcome: Error, .. })` — signal termination
    ///
    /// # Errors
    /// - `PreconditionError::TrustBoundary` — `command[0]` resolves inside the
    ///   agent-writable workspace (fail closed)
    /// - `PreconditionError::Spawn` — the program could not be resolved or the
    ///   process could not be spawned (fail closed)
    /// - `PreconditionError::Timeout` — the wall-clock timeout elapsed (fail
    ///   closed)
    async fn run(
        &self,
        precondition: &Precondition,
        input: &PreconditionCheckInput,
    ) -> Result<PreconditionRun, PreconditionError>;
}

/// Production runner executing a real child process.
///
/// Constructed with the **agent-writable workspace root** so the trust-boundary
/// check (`command[0]` resolves outside it) can be enforced before spawn.
pub struct ProcessPreconditionRunner {
    /// Root of the agent-writable workspace. A `command[0]` resolving inside
    /// this root is refused.
    workspace_root: PathBuf,
}

impl ProcessPreconditionRunner {
    /// Create the runner over the agent-writable workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// The workspace root the trust-boundary check is relative to.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

/// Resolve `program` (argv[0]) to an absolute, canonical path.
///
/// - A path (contains `/`) is canonicalized directly.
/// - A bare name is searched on `PATH`.
///
/// A program that cannot be resolved is a [`PreconditionError::Spawn`] (fail
/// closed) — never an implicit pass.
fn resolve_program(program: &str, precondition_id: &str) -> Result<PathBuf, PreconditionError> {
    let spawn_error = |detail: String| PreconditionError::Spawn {
        precondition_id: precondition_id.to_string(),
        detail,
    };
    if program.contains('/') {
        return std::fs::canonicalize(program)
            .map_err(|error| spawn_error(format!("cannot resolve program '{program}': {error}")));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(program);
            if candidate.is_file()
                && let Ok(canonical) = std::fs::canonicalize(&candidate)
            {
                return Ok(canonical);
            }
        }
    }
    Err(spawn_error(format!(
        "program '{program}' not found on PATH"
    )))
}

/// Refuse a resolved program that lives inside the agent-writable workspace.
///
/// The workspace root is canonicalized when it exists so symlinked workspaces
/// are handled; if it cannot be canonicalized the lexical root is used as a
/// best-effort boundary (the program is already canonical).
fn ensure_outside_workspace(
    program: &Path,
    workspace_root: &Path,
    precondition_id: &str,
) -> Result<(), PreconditionError> {
    let workspace =
        std::fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
    if program.starts_with(&workspace) {
        return Err(PreconditionError::TrustBoundary {
            precondition_id: precondition_id.to_string(),
            command: program.display().to_string(),
        });
    }
    Ok(())
}

/// SHA-256 (`sha256:<hex>`) of a file's bytes, or `None` if it cannot be read.
///
/// Used for signed attribution: `check_digest` (the check program) and
/// `authority_digest` (the artifact the check consults). Contents never enter
/// the record — only the one-way digest (SpanPrivacy).
fn sha256_file(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).ok()?;
    let digest = Sha256::digest(&bytes);
    Some(format!("sha256:{}", hex::encode(digest)))
}

/// Whether `path` is writable by the engine's effective UID, from POSIX
/// metadata (owner/group/world mode bits).
///
/// - `Ok(true)`  — owner-write, group-write (euid is the egid or in a
///   supplementary group), or world-write
/// - `Ok(false)` — none of the above
/// - `Err(_)`    — metadata could not be read (the caller fails closed under
///   `require_immutable_check`)
///
/// Root is deliberately **not** special-cased: the assessment reports the
/// mode/ownership fact, and running the engine as root voids an immutability
/// claim anyway.
#[cfg(unix)]
fn writable_by_euid(path: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = std::fs::metadata(path)?;
    let mode = metadata.permissions().mode();
    if mode & 0o002 != 0 {
        return Ok(true);
    }
    let euid = unsafe { libc::geteuid() };
    if metadata.uid() == euid && mode & 0o200 != 0 {
        return Ok(true);
    }
    if mode & 0o020 != 0 && euid_in_group(metadata.gid()) {
        return Ok(true);
    }
    Ok(false)
}

/// Whether the engine's effective UID is the egid or in the file's group.
#[cfg(unix)]
fn euid_in_group(gid: u32) -> bool {
    // SAFETY: `getgroups` is called with a buffer sized by the first call.
    unsafe {
        if gid == libc::getegid() {
            return true;
        }
        let count = libc::getgroups(0, std::ptr::null_mut());
        if count <= 0 {
            return false;
        }
        let mut groups = vec![0 as libc::gid_t; count as usize];
        let written = libc::getgroups(count, groups.as_mut_ptr());
        if written < 0 {
            return false;
        }
        groups.truncate(written as usize);
        groups.contains(&gid)
    }
}

/// Best-effort on non-unix: the boundary strength cannot be assessed, so the
/// caller treats it as unknown (fail closed under `require_immutable_check`).
#[cfg(not(unix))]
fn writable_by_euid(_path: &Path) -> std::io::Result<bool> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "writability assessment requires unix metadata",
    ))
}

/// Assess one boundary path and trace the fact. Under
/// `require_immutable_check`, an unreadable/unknown assessment is a
/// [`PreconditionError::Boundary`] (fail closed); otherwise it is `None` and
/// the run proceeds unchanged.
fn assess_boundary(
    precondition: &Precondition,
    path: &Path,
    label: &str,
) -> Result<Option<bool>, PreconditionError> {
    match writable_by_euid(path) {
        Ok(writable) => {
            tracing::debug!(
                precondition = %precondition.id,
                path = %path.display(),
                boundary = label,
                writable,
                "precondition: boundary writability assessment"
            );
            Ok(Some(writable))
        }
        Err(error) => {
            if precondition.require_immutable_check {
                return Err(PreconditionError::Boundary {
                    precondition_id: precondition.id.clone(),
                    path: path.display().to_string(),
                    detail: format!("cannot assess {label} writability: {error}"),
                });
            }
            tracing::warn!(
                precondition = %precondition.id,
                path = %path.display(),
                boundary = label,
                %error,
                "precondition: boundary writability unknown; proceeding"
            );
            Ok(None)
        }
    }
}

/// The fail-closed refusal for a writable boundary under
/// `require_immutable_check`.
fn boundary_error(precondition: &Precondition, path: &Path, label: &str) -> PreconditionError {
    PreconditionError::Boundary {
        precondition_id: precondition.id.clone(),
        path: path.display().to_string(),
        detail: format!(
            "{label} is writable by the engine's effective UID and require_immutable_check=true"
        ),
    }
}

#[async_trait]
impl PreconditionRunner for ProcessPreconditionRunner {
    async fn run(
        &self,
        precondition: &Precondition,
        input: &PreconditionCheckInput,
    ) -> Result<PreconditionRun, PreconditionError> {
        let Some(program) = precondition.command.first() else {
            return Err(PreconditionError::Spawn {
                precondition_id: precondition.id.clone(),
                detail: "command is empty (missing argv[0])".to_string(),
            });
        };

        // Trust boundary BEFORE spawn: a check the agent can edit is no check.
        let resolved = resolve_program(program, &precondition.id)?;
        ensure_outside_workspace(&resolved, &self.workspace_root, &precondition.id)?;

        // Boundary strength: is the check — and any declared authority —
        // writable by the engine's UID? Recorded always; refused only when the
        // operator opted in (`require_immutable_check`).
        let check_writable = assess_boundary(precondition, &resolved, "check")?;
        let authority_writable = match precondition.authority_path.as_deref() {
            Some(path) => assess_boundary(precondition, Path::new(path), "authority")?,
            None => None,
        };
        if precondition.require_immutable_check {
            if check_writable == Some(true) {
                return Err(boundary_error(precondition, &resolved, "check"));
            }
            if authority_writable == Some(true)
                && let Some(path) = precondition.authority_path.as_deref()
            {
                return Err(boundary_error(precondition, Path::new(path), "authority"));
            }
        }

        // Signed attribution: bind the check program and the authority artifact
        // by one-way digest. Best-effort — unreadable files omit the digest.
        let check_digest = sha256_file(&resolved);
        let authority_digest = precondition
            .authority_path
            .as_deref()
            .and_then(|path| sha256_file(Path::new(path)));
        tracing::debug!(
            precondition = %precondition.id,
            check_digest = ?check_digest,
            authority_digest = ?authority_digest,
            "precondition: attribution digests"
        );

        let payload = serde_json::to_vec(input).map_err(|error| PreconditionError::Spawn {
            precondition_id: precondition.id.clone(),
            detail: format!("failed to serialize check input: {error}"),
        })?;

        tracing::debug!(
            precondition = %precondition.id,
            program = %resolved.display(),
            timeout_ms = precondition.timeout_ms,
            "precondition: spawning check (argv-only, no shell)"
        );

        let mut child = Command::new(&resolved)
            .args(&precondition.command[1..])
            .env("RIGORIX_PRECONDITION_ID", &precondition.id)
            .env("RIGORIX_EXECUTION_ID", input.execution_id.to_string())
            .env("RIGORIX_STEP_NAME", &input.step)
            .env("RIGORIX_TOOL", &input.tool)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| PreconditionError::Spawn {
                precondition_id: precondition.id.clone(),
                detail: format!("failed to spawn '{}': {error}", resolved.display()),
            })?;

        let stdin = child.stdin.take();
        let timeout = std::time::Duration::from_millis(precondition.timeout_ms);
        let wait = async move {
            if let Some(mut stdin) = stdin {
                // Trailing newline so line-oriented checks see a complete line;
                // the JSON value itself is unchanged.
                let mut body = payload;
                body.push(b'\n');
                // A check may legitimately ignore stdin (or exit before reading
                // it), so a broken pipe is NOT a failure — the exit code is the
                // verdict. Writing errors are deliberately non-fatal here.
                let _ = stdin.write_all(&body).await;
                // Close stdin to signal EOF to the check.
                drop(stdin);
            }
            child.wait_with_output().await
        };

        let output = match tokio::time::timeout(timeout, wait).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                return Err(PreconditionError::Spawn {
                    precondition_id: precondition.id.clone(),
                    detail: format!("check I/O failed: {error}"),
                });
            }
            Err(_elapsed) => {
                // `kill_on_drop` terminates the child as the future is dropped.
                return Err(PreconditionError::Timeout {
                    precondition_id: precondition.id.clone(),
                    timeout_ms: precondition.timeout_ms,
                });
            }
        };

        let exit_code = output.status.code();
        let outcome = match exit_code {
            Some(0) => PreconditionOutcome::Passed,
            Some(_) => PreconditionOutcome::Failed,
            // Killed by a signal — indeterminate, refuse.
            None => PreconditionOutcome::Error,
        };
        let stdout = precondition.capture_output.then(|| {
            let text = String::from_utf8_lossy(&output.stdout);
            text.chars().take(MAX_CAPTURED_STDOUT_BYTES).collect()
        });
        Ok(PreconditionRun {
            outcome,
            exit_code,
            stdout,
            check_writable,
            authority_writable,
            check_digest,
            authority_digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precondition::domain::{FailureAction, Precondition};
    use serde_json::json;

    fn input() -> PreconditionCheckInput {
        PreconditionCheckInput {
            precondition_id: "p1".to_string(),
            execution_id: uuid::Uuid::nil(),
            step: "pay".to_string(),
            tool: "payment_execute".to_string(),
            parameters: json!({ "beneficiary": "acct-1" }),
        }
    }

    fn precondition(id: &str, command: Vec<&str>) -> Precondition {
        Precondition {
            id: id.to_string(),
            r#match: serde_json::from_value(json!({ "tool": "payment_execute" })).unwrap(),
            require_params: vec![],
            command: command.into_iter().map(str::to_string).collect(),
            timeout_ms: 5_000,
            failure: FailureAction::Deny,
            capture_output: false,
            require_immutable_check: false,
            authority_path: None,
        }
    }

    fn runner_outside_workspace() -> ProcessPreconditionRunner {
        // A real directory that does not contain /bin/sh.
        ProcessPreconditionRunner::new(std::env::temp_dir())
    }

    #[tokio::test]
    async fn exit_zero_is_passed() {
        let runner = runner_outside_workspace();
        let run = runner
            .run(
                &precondition("p1", vec!["/bin/sh", "-c", "exit 0"]),
                &input(),
            )
            .await
            .expect("run");
        assert_eq!(run.outcome, PreconditionOutcome::Passed);
        assert_eq!(run.exit_code, Some(0));
    }

    #[tokio::test]
    async fn non_zero_exit_is_failed() {
        let runner = runner_outside_workspace();
        let run = runner
            .run(
                &precondition("p1", vec!["/bin/sh", "-c", "exit 3"]),
                &input(),
            )
            .await
            .expect("run");
        assert_eq!(run.outcome, PreconditionOutcome::Failed);
        assert_eq!(run.exit_code, Some(3));
    }

    #[tokio::test]
    async fn json_stdin_and_env_are_set() {
        let runner = runner_outside_workspace();
        // Fails (exit 9) unless stdin has a line AND the env vars are set.
        let command = vec![
            "/bin/sh",
            "-c",
            "read -r line || exit 9; [ \"$RIGORIX_PRECONDITION_ID\" = p1 ] || exit 9; \
             [ \"$RIGORIX_STEP_NAME\" = pay ] || exit 9; \
             [ \"$RIGORIX_TOOL\" = payment_execute ] || exit 9; \
             [ -n \"$RIGORIX_EXECUTION_ID\" ] || exit 9",
        ];
        let run = runner
            .run(&precondition("p1", command), &input())
            .await
            .expect("run");
        assert_eq!(run.outcome, PreconditionOutcome::Passed);
    }

    #[tokio::test]
    async fn argv_is_not_shell_interpolated() {
        let runner = runner_outside_workspace();
        let mut precondition = precondition(
            "p1",
            vec!["/bin/sh", "-c", "printf '%s' \"$1\"", "sh", "a;b"],
        );
        precondition.capture_output = true;
        let run = runner.run(&precondition, &input()).await.expect("run");
        assert_eq!(run.outcome, PreconditionOutcome::Passed);
        assert_eq!(run.stdout.as_deref(), Some("a;b"));
    }

    #[tokio::test]
    async fn command_inside_workspace_is_refused() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let check = workspace.path().join("check.sh");
        std::fs::write(&check, "#!/bin/sh\nexit 0\n").expect("write check");
        let runner = ProcessPreconditionRunner::new(workspace.path());
        let precondition = precondition("p1", vec![check.to_str().unwrap()]);
        let error = runner
            .run(&precondition, &input())
            .await
            .expect_err("trust boundary");
        assert!(matches!(error, PreconditionError::TrustBoundary { .. }));
        assert!(!error.is_retriable());
    }

    #[tokio::test]
    async fn timeout_maps_to_refuse_error() {
        let runner = runner_outside_workspace();
        let mut precondition = precondition("p1", vec!["/bin/sh", "-c", "sleep 5"]);
        precondition.timeout_ms = 100;
        let error = runner
            .run(&precondition, &input())
            .await
            .expect_err("timeout");
        assert!(matches!(error, PreconditionError::Timeout { .. }));
        assert!(!error.is_retriable());
    }

    #[tokio::test]
    async fn spawn_failure_maps_to_refuse_error() {
        let runner = runner_outside_workspace();
        let precondition = precondition("p1", vec!["/nonexistent/rigorix-check-xyz"]);
        let error = runner
            .run(&precondition, &input())
            .await
            .expect_err("spawn");
        assert!(matches!(error, PreconditionError::Spawn { .. }));
        assert!(!error.is_retriable());
    }

    #[tokio::test]
    async fn check_that_ignores_stdin_is_judged_by_exit_code() {
        // Regression: a check that exits without reading stdin closes the pipe.
        // A large payload deterministically fills the pipe buffer so the write
        // fails with EPIPE; that must NOT be reported as a spawn failure — the
        // exit code is the verdict.
        let runner = runner_outside_workspace();
        let mut big = input();
        big.parameters = json!({ "blob": "x".repeat(1_000_000) });
        let run = runner
            .run(&precondition("p1", vec!["/bin/sh", "-c", "exit 7"]), &big)
            .await
            .expect("run must be judged by exit code, not a broken pipe");
        assert_eq!(run.outcome, PreconditionOutcome::Failed);
        assert_eq!(run.exit_code, Some(7));
    }

    #[test]
    fn metadata_error_is_fail_closed_under_immutable_flag() {
        let mut required = precondition("p1", vec!["/bin/true"]);
        required.require_immutable_check = true;
        let error = assess_boundary(
            &required,
            Path::new("/nonexistent/rigorix-check-xyz"),
            "check",
        )
        .expect_err("unknown boundary must fail closed under the flag");
        assert!(matches!(error, PreconditionError::Boundary { .. }));
        assert!(!error.is_retriable());

        // Flag off → unknown is recorded (`None`), never fatal (default unchanged).
        let permissive = precondition("p1", vec!["/bin/true"]);
        assert_eq!(
            assess_boundary(
                &permissive,
                Path::new("/nonexistent/rigorix-check-xyz"),
                "check"
            )
            .expect("flag off must not fail closed"),
            None
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn writable_check_is_refused_under_immutable_flag() {
        let workspace = tempfile::tempdir().expect("workspace");
        let check_home = tempfile::tempdir().expect("check home");
        let check = check_home.path().join("check.sh");
        std::fs::write(&check, "#!/bin/sh\nexit 0\n").expect("write check");
        // Default 0644, owned by the test euid → owner-writable.
        let runner = ProcessPreconditionRunner::new(workspace.path());
        let mut precondition = precondition("p1", vec![check.to_str().unwrap()]);
        precondition.require_immutable_check = true;
        let error = runner
            .run(&precondition, &input())
            .await
            .expect_err("writable check must be refused");
        assert!(matches!(error, PreconditionError::Boundary { .. }));
        assert!(!error.is_retriable());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn non_writable_check_passes_under_immutable_flag() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = tempfile::tempdir().expect("workspace");
        let check_home = tempfile::tempdir().expect("check home");
        let check = check_home.path().join("check.sh");
        std::fs::write(&check, "#!/bin/sh\nexit 0\n").expect("write check");
        std::fs::set_permissions(&check, std::fs::Permissions::from_mode(0o555))
            .expect("chmod 0555");
        let runner = ProcessPreconditionRunner::new(workspace.path());
        let mut precondition = precondition("p1", vec![check.to_str().unwrap()]);
        precondition.require_immutable_check = true;
        let run = runner.run(&precondition, &input()).await.expect("run");
        assert_eq!(run.outcome, PreconditionOutcome::Passed);
        assert_eq!(run.check_writable, Some(false));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn writable_authority_is_refused_under_immutable_flag() {
        let workspace = tempfile::tempdir().expect("workspace");
        let authority_home = tempfile::tempdir().expect("authority home");
        let authority = authority_home.path().join("authority.json");
        std::fs::write(&authority, "{\"acme\":\"active\"}\n").expect("write authority");
        let runner = ProcessPreconditionRunner::new(workspace.path());
        let mut precondition = precondition("p1", vec!["/bin/sh", "-c", "exit 0"]);
        precondition.require_immutable_check = true;
        precondition.authority_path = Some(authority.display().to_string());
        let error = runner
            .run(&precondition, &input())
            .await
            .expect_err("writable authority must be refused");
        assert!(matches!(error, PreconditionError::Boundary { .. }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn digests_bind_the_check_and_authority_for_attribution() {
        let workspace = tempfile::tempdir().expect("workspace");
        let home = tempfile::tempdir().expect("check home");
        let check = home.path().join("check.sh");
        std::fs::write(&check, "#!/bin/sh\nexit 0\n").expect("write check");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&check, std::fs::Permissions::from_mode(0o755))
                .expect("chmod +x");
        }
        let authority = home.path().join("authority.json");
        std::fs::write(&authority, "{\"acme\":\"active\"}\n").expect("write authority");

        let runner = ProcessPreconditionRunner::new(workspace.path());
        let mut precondition = precondition("p1", vec![check.to_str().unwrap()]);
        precondition.authority_path = Some(authority.display().to_string());

        let run = runner.run(&precondition, &input()).await.expect("run");
        assert_eq!(run.outcome, PreconditionOutcome::Passed);
        let check_digest = run.check_digest.clone().expect("check digest");
        let authority_digest = run.authority_digest.clone().expect("authority digest");
        assert!(check_digest.starts_with("sha256:"));
        assert!(authority_digest.starts_with("sha256:"));

        // Tampering the authority changes its digest; the check digest is stable.
        std::fs::write(&authority, "{\"acme\":\"frozen\"}\n").expect("tamper");
        let run = runner.run(&precondition, &input()).await.expect("run");
        assert_ne!(run.authority_digest, Some(authority_digest));
        assert_eq!(run.check_digest, Some(check_digest));
    }
}
