//! RunCommandTool — execute shell commands with allowlist enforcement.
//!
//! @canonical .pi/architecture/modules/tool-system.md#run-cmd
//! Implements: Tool trait — RunCommand concrete tool
//! Issue: #125
//!
//! Executes shell commands with allowlist enforcement and timeout.
//! Commands must match an entry in the configured allowlist to execute.
//! High risk — dry-run by default in risk-gated mode.
//!
//! # Security
//!
//! The allowlist matches a single simple command. A command containing a shell
//! control operator (`;`, `&&`, `||`, `|`, `&`, backticks, `$(`, `<`, `>`, a
//! newline) is refused *before* the prefix check: matching a prefix and then
//! handing the whole string to `sh -c` lets `echo ok; curl evil` through. Split
//! a compound into separate steps, or put it in a script the allowlist permits.
//!
//! The child gets a cleared environment plus [`INHERITED_ENV_ALLOWLIST`], so it
//! cannot read `RIGORIX_HMAC_KEY` and forge audit envelopes. This is not a
//! sandbox: the child still runs as the engine's user with its filesystem
//! access (#1068).
//!
//! # Isolation (#1478)
//!
//! The tool cannot be constructed without a [`SandboxDeclaration`]: the
//! deployment must state `RIGORIX_SANDBOX=container` (or `vm`), and `none` is
//! refused with the reach named. Filtering is not isolation, and the register
//! says so in the `does_not` column (`.pi/architecture/sink-register.tsv`).
//!
//! A proven-single command is executed as argv, never through `sh -c`:
//! [`argv_from_simple`] refuses anything a shell would expand. The allowlist's
//! grain is therefore the *executable* — arguments are unconstrained, so
//! allowlisting `cargo`, `npm`, `python`, `git` or `make` for an untrusted agent
//! is allowlisting arbitrary code execution. See `docs/deployment.md`.
//!
//! # Environment overrides (#1489)
//!
//! A caller may not set a key that changes *what executes*: `PATH` and the
//! loader families (`LD_*`, `DYLD_*`) are refused by name, with the reason. Every
//! other key must be in the set the tool declared it needs
//! ([`RunCommandTool::with_declared_env`]) — an undeclared key is an error, not
//! an environment entry. The program is resolved against the *engine's* `PATH`
//! before any override is applied, and the resolved path is recorded on the
//! result, so the evidence names the file that ran rather than the word that was
//! approved.

// #1083: approved process-spawning sink — governed shell sink: allowlist + control-operator refusal + cleared env.
// See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
#![allow(clippy::disallowed_types)]

use async_trait::async_trait;
use std::collections::HashSet;
use std::time::Duration;
use std::time::Instant;

use crate::tools::application::dto::{SideEffect, ToolInput, ToolResult};
use crate::tools::application::shell_policy::{
    SandboxDeclaration, SandboxDeclarationError, apply_child_env, argv_from_simple,
    find_shell_control_operator, resolve_program, validate_env_overrides,
};
use crate::tools::domain::{Tool, ToolError};
use crate::tools::infrastructure::fs_at::resolve_dir_for_child;

/// Tool for executing shell commands with allowlist enforcement.
///
/// # Input Parameters
/// - `command` (required, string): The command to execute.
/// - `cwd` (optional, string): Working directory (default: workspace root).
/// - `timeout_secs` (optional, int): Timeout in seconds (default: 60).
/// - `env` (optional, object): Environment variable overrides.
///
/// # Risk Level
/// High — arbitrary command execution.
///
/// # Security
/// Commands are validated against a configured allowlist of permitted
/// command prefixes. Commands not matching any allowlist entry are rejected,
/// and so is any command containing a shell control operator.
pub struct RunCommandTool {
    /// Root directory for execution.
    workspace_root: String,
    /// Set of allowed command prefixes (e.g., "cargo", "git", "npm", "node").
    allowlist: HashSet<String>,
    /// Default timeout in seconds.
    default_timeout: u64,
    /// The isolation the deployment declared for spawned children.
    sandbox: SandboxDeclaration,
    /// The environment keys this tool's commands are allowed to receive (#1489).
    ///
    /// Empty by default: a caller-supplied environment is an input channel, and a
    /// channel nobody declared is how `PATH` became an executable selector.
    declared_env: HashSet<String>,
}

impl RunCommandTool {
    /// Create a new RunCommandTool with the given workspace root, allowlist and
    /// declared isolation.
    ///
    /// There is no constructor without a [`SandboxDeclaration`]: a caller that
    /// has not stated the boundary cannot enable the sink (#1478).
    pub fn new(
        workspace_root: impl Into<String>,
        allowlist: Vec<impl Into<String>>,
        sandbox: SandboxDeclaration,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            allowlist: allowlist.into_iter().map(|s| s.into()).collect(),
            default_timeout: 60,
            sandbox,
            declared_env: HashSet::new(),
        }
    }

    /// Read the declared isolation from the deployment (`RIGORIX_SANDBOX`).
    ///
    /// Unset, unrecognized, or `none` is an error naming what the child can
    /// reach — the tool is not constructed.
    pub fn from_env(
        workspace_root: impl Into<String>,
        allowlist: Vec<impl Into<String>>,
    ) -> Result<Self, SandboxDeclarationError> {
        let sandbox = SandboxDeclaration::from_env()?;
        Ok(Self::new(workspace_root, allowlist, sandbox))
    }

    /// Create a new RunCommandTool with a custom default timeout.
    pub fn with_timeout(
        workspace_root: impl Into<String>,
        allowlist: Vec<impl Into<String>>,
        default_timeout: u64,
        sandbox: SandboxDeclaration,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            allowlist: allowlist.into_iter().map(|s| s.into()).collect(),
            default_timeout,
            sandbox,
            declared_env: HashSet::new(),
        }
    }

    /// The isolation the deployment declared.
    pub fn sandbox(&self) -> &SandboxDeclaration {
        &self.sandbox
    }

    /// Declare the environment keys this tool's commands may receive (#1489).
    ///
    /// Anything outside the set is refused rather than passed through: the
    /// reserved list alone cannot cover every variable a loader, a runtime or a
    /// shell reads, and "whatever the caller sends" was the shape of the defect.
    pub fn with_declared_env(mut self, keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.declared_env = keys.into_iter().map(|key| key.into()).collect();
        self
    }

    /// The declared environment key set.
    pub fn declared_env(&self) -> &HashSet<String> {
        &self.declared_env
    }

    /// Check if the command is allowed by the allowlist.
    ///
    /// Matches the command against allowed prefixes with word boundary checking.
    /// For example, "cargo" matches "cargo build" but not "cargoes".
    fn is_command_allowed(&self, command: &str) -> bool {
        let trimmed = command.trim();
        // A compound string is never a single allowlisted command, even when it
        // starts with an allowed prefix (defence in depth; `execute` also
        // reports the operator specifically).
        if find_shell_control_operator(trimmed).is_some() {
            return false;
        }
        for prefix in &self.allowlist {
            if trimmed == prefix.as_str()
                || trimmed.starts_with(&format!("{} ", prefix))
                || trimmed.starts_with(&format!("{}/", prefix))
            {
                return true;
            }
        }
        false
    }
}

#[async_trait]
impl Tool for RunCommandTool {
    fn name(&self) -> &str {
        "run-command"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let command = input.require_string("command")?;
        // #1488: `cwd` is part of the boundary, not an input. `cargo build` runs the
        // `build.rs` of the directory it starts in, `npm run` its lifecycle scripts,
        // and `git` honours repo-local config — so an allowlisted word plus a
        // caller-chosen directory is still arbitrary execution.
        let requested_cwd = input
            .get_string("cwd")
            .unwrap_or_else(|| self.workspace_root.clone());
        let cwd = resolve_dir_for_child(&self.workspace_root, &requested_cwd)?;
        let timeout_secs = input
            .get_u64("timeout_secs")
            .unwrap_or(self.default_timeout);
        let timeout_duration = Duration::from_secs(timeout_secs);
        let env_overrides: Vec<(String, String)> = input
            .params
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Refuse a compound shell string before the allowlist check: the
        // allowlist sees a prefix, but `sh -c` would execute everything.
        if let Some(operator) = find_shell_control_operator(&command) {
            return Err(ToolError::PathDenied(format!(
                "Command '{}' contains the shell control operator '{}'. run_command executes a single \
                 command, not a compound shell string; use separate steps or an allowlisted script.",
                command, operator
            )));
        }

        // Enforce allowlist
        if !self.is_command_allowed(&command) {
            return Err(ToolError::PathDenied(format!(
                "Command '{}' is not in the allowlist. Allowed prefixes: {:?}",
                command,
                self.allowlist
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        let start = Instant::now();

        // #1478: execute argv, not a shell string. The command was proven single
        // above (no control operator) and is proven shell-free here: anything a
        // shell would expand was refused, so `sh -c` would add nothing but the
        // injection surface it was already filtering by hand.
        let argv = argv_from_simple(&command).map_err(|e| {
            ToolError::InvalidInput(format!(
                "Command '{}' cannot be executed without a shell: {}. run_command executes one \
                 executable with literal arguments; quote a metacharacter with single quotes if \
                 it is meant literally.",
                command, e
            ))
        })?;
        let (program, arguments) = argv.split_first().ok_or_else(|| {
            ToolError::InvalidInput(format!("Command '{}' has no executable", command))
        })?;

        // #1489: refuse an environment that could choose the executable, then
        // resolve the executable in the engine's own PATH — before any override is
        // applied. `Command::new("echo")` resolves through the child's PATH, which
        // a caller-supplied `PATH` used to decide.
        validate_env_overrides(&env_overrides, &self.declared_env)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
        let resolved = resolve_program(program).map_err(ToolError::InvalidInput)?;

        // The child inherits only the explicit allowlist; `RIGORIX_HMAC_KEY` and
        // anchor credentials stay in the engine process. The declaration this
        // sink relies on is held on the tool (`self.sandbox`): filtering alone is
        // not isolation, and the deployment stated the boundary at construction.
        // sink: tools:run-command-spawn
        let mut cmd = tokio::process::Command::new(&resolved);
        cmd.args(arguments);
        // #1488: the child enters the validated inode, not a re-resolved path.
        // `current_dir(path)` would re-resolve in the child, so the directory is
        // applied by descriptor before `execve`.
        #[cfg(unix)]
        {
            let fd = cwd.raw_fd();
            // SAFETY: `fchdir` only changes the child's working directory. `fd` is a
            // live read-only directory descriptor owned by `cwd`, which outlives the
            // spawn (it is a local of this frame, and `cmd.output()` is awaited
            // before it drops). `fchdir` is async-signal-safe, which `pre_exec`
            // requires. `cwd.display()` records which directory this was.
            unsafe {
                cmd.pre_exec(move || {
                    if libc::fchdir(fd) == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                });
            }
        }
        #[cfg(not(unix))]
        return Err(ToolError::ExecutionFailed(
            "cwd needs fchdir semantics; this target has none, and a path-based fallback \
             would re-resolve (#1488)"
                .to_string(),
        ));
        apply_child_env(&mut cmd, &env_overrides);

        let output = tokio::time::timeout(timeout_duration, cmd.output())
            .await
            .map_err(|_| {
                ToolError::ExecutionFailed(format!(
                    "Command timed out after {} seconds: {}",
                    timeout_secs, command
                ))
            })?
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to execute command '{}': {}",
                    command, e
                ))
            })?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let result_output = if output.status.success() {
            stdout
        } else {
            format!("STDERR:\n{}\n\nSTDOUT:\n{}", stderr, stdout)
        };

        let exit_code = output.status.code().unwrap_or(-1);

        let result = ToolResult {
            output: result_output,
            exit_code,
            side_effects: vec![SideEffect::new(
                &command,
                "run_command",
                format!("Exit code: {}", exit_code),
            )],
            duration_ms,
            dry_run: false,
            // #1489: the file that ran, not the word the allowlist approved.
            resolved_executable: Some(resolved.to_string_lossy().to_string()),
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_input(command: &str) -> ToolInput {
        let mut params = HashMap::new();
        params.insert(
            "command".to_string(),
            serde_json::Value::String(command.to_string()),
        );
        ToolInput::new(params)
    }

    fn allowlist_tool() -> (RunCommandTool, TempDir) {
        let dir = TempDir::new().unwrap();
        let tool = RunCommandTool::new(
            dir.path().to_str().unwrap(),
            vec!["echo", "cat", "ls", "cargo", "git", "env", "pwd"],
            SandboxDeclaration::container(),
        );
        (tool, dir)
    }

    fn make_simple_tool() -> (RunCommandTool, TempDir) {
        let dir = TempDir::new().unwrap();
        let tool = RunCommandTool::new(
            dir.path().to_str().unwrap(),
            vec!["echo"],
            SandboxDeclaration::container(),
        );
        (tool, dir)
    }

    #[tokio::test]
    async fn test_run_allowed_command() {
        let (tool, _dir) = allowlist_tool();
        let result = tool.execute(&make_input("echo hello")).await.unwrap();

        assert!(result.is_success());
        assert_eq!(result.output.trim(), "hello");
    }

    #[tokio::test]
    async fn test_run_disallowed_command() {
        let (tool, _dir) = allowlist_tool();
        let result = tool.execute(&make_input("rm -rf /")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::PathDenied(_)));
    }

    #[tokio::test]
    async fn test_run_failing_command() {
        let (tool, _dir) = allowlist_tool();
        let result = tool
            .execute(&make_input("cat /nonexistent_file_xyz"))
            .await
            .unwrap();

        assert!(!result.is_success());
        assert!(result.exit_code != 0);
    }

    #[tokio::test]
    async fn test_missing_command_parameter() {
        let (tool, _dir) = allowlist_tool();
        let result = tool.execute(&ToolInput::new(HashMap::new())).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_tool_name() {
        let (tool, _dir) = make_simple_tool();
        assert_eq!(tool.name(), "run-command");
    }

    #[tokio::test]
    async fn test_allowlist_prefix_matching() {
        let (tool, _dir) = make_simple_tool();

        assert!(tool.is_command_allowed("echo build"));
        assert!(tool.is_command_allowed("echo"));
        assert!(!tool.is_command_allowed("echobuild"));
        assert!(!tool.is_command_allowed("npm install"));
    }

    #[tokio::test]
    async fn test_run_with_timeout() {
        let (tool, _dir) = allowlist_tool();
        let result = tool.execute(&make_input("echo quick")).await.unwrap();

        assert!(result.is_success());
        assert!(result.duration_ms < 1000);
    }

    #[tokio::test]
    async fn test_run_with_cwd() {
        let dir = TempDir::new().unwrap();
        let tool = RunCommandTool::new(
            dir.path().to_str().unwrap(),
            vec!["echo", "pwd"],
            SandboxDeclaration::container(),
        );

        let mut params = HashMap::new();
        params.insert(
            "command".to_string(),
            serde_json::Value::String("pwd".to_string()),
        );
        params.insert(
            "cwd".to_string(),
            serde_json::Value::String(dir.path().to_str().unwrap().to_string()),
        );
        let input = ToolInput::new(params);

        let result = tool.execute(&input).await.unwrap();
        assert!(result.is_success());
        assert!(result.output.trim().ends_with(dir.path().to_str().unwrap()));
    }

    /// #1067: the allowlist matched a prefix, then `sh -c` ran the whole
    /// compound. A string containing a control operator must be refused.
    #[tokio::test]
    async fn test_compound_shell_string_is_refused() {
        let (tool, _dir) = allowlist_tool();
        for command in [
            "echo ok; rm -rf /",
            "echo ok && rm -rf /",
            "echo ok || rm -rf /",
            "echo ok | sh",
            "echo ok & ",
            "echo `id`",
            "echo $(id)",
            "echo ok > /tmp/rigorix-escape",
            "echo ok < /etc/passwd",
            "echo ok\nrm -rf /",
        ] {
            let result = tool.execute(&make_input(command)).await;
            assert!(
                matches!(result, Err(ToolError::PathDenied(_))),
                "{command:?} must be refused before execution"
            );
        }
    }

    /// #1068: the child must not inherit the engine's environment. The engine
    /// signs audit envelopes with `RIGORIX_HMAC_KEY`, so handing it to a child
    /// lets the child forge the records that constrain it.
    ///
    /// #1478: measured with `env` rather than `sh -c 'echo "[$VAR]"'` — the argv
    /// path expands nothing, so the observation has to be the child's own
    /// environment dump.
    #[tokio::test]
    async fn test_child_does_not_inherit_the_engine_environment() {
        assert!(
            std::env::var("CARGO_MANIFEST_DIR").is_ok(),
            "test precondition: cargo must set a parent-only variable"
        );
        let (tool, _dir) = allowlist_tool();
        let result = tool.execute(&make_input("env")).await.unwrap();
        assert!(
            !result.output.contains("CARGO_MANIFEST_DIR="),
            "the engine environment must not reach the child: {}",
            result.output
        );
        assert!(
            !result.output.contains("RIGORIX_HMAC_KEY="),
            "the signing key must never reach a child: {}",
            result.output
        );

        // Explicit per-call overrides still apply after the clear.
        let mut params = HashMap::new();
        params.insert(
            "command".to_string(),
            serde_json::Value::String("env".to_string()),
        );
        params.insert(
            "env".to_string(),
            serde_json::json!({ "RIGORIX_TEST_OVERRIDE": "visible" }),
        );
        // #1489: a declared key reaches the child; the same key without a
        // declaration is refused (see the #1489 tests).
        let tool = tool.with_declared_env(["RIGORIX_TEST_OVERRIDE"]);
        let result = tool.execute(&ToolInput::new(params)).await.unwrap();
        assert!(
            result.output.contains("RIGORIX_TEST_OVERRIDE=visible"),
            "an explicit override must reach the child: {}",
            result.output
        );
    }

    // -----------------------------------------------------------------------
    // #1478 (SINK-1)
    // -----------------------------------------------------------------------

    /// The isolation requirement is a construction precondition, not a comment:
    /// there is no `RunCommandTool` without a declaration, and `none` is not one.
    #[test]
    fn the_sink_cannot_be_built_without_a_declared_boundary() {
        assert!(SandboxDeclaration::from_value("none").is_err());
        assert_eq!(
            RunCommandTool::new("/tmp", vec!["echo"], SandboxDeclaration::vm())
                .sandbox()
                .requires(),
            "deployment isolation (RIGORIX_SANDBOX=vm)"
        );
    }

    /// #1478 acceptance 3: the simple path executes argv, not `sh -c`.
    #[test]
    fn the_source_path_has_no_shell_command_flag() {
        let source = include_str!("run_command_tool.rs");
        let code: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("\"-c\""),
            "run-command must not invoke a shell; the argv path replaced `-c`"
        );
    }

    /// A command that needs a shell is refused instead of silently passed
    /// through unexpanded — the caller who wrote `$HOME` expected expansion.
    #[tokio::test]
    async fn a_command_needing_expansion_is_refused_by_name() {
        let (tool, _dir) = allowlist_tool();
        let error = tool.execute(&make_input("echo $HOME")).await.unwrap_err();
        assert!(matches!(error, ToolError::InvalidInput(_)), "{error:?}");
    }

    // ── #1489: the environment cannot choose the executable ──────────────

    fn input_with_env(command: &str, env: &[(&str, &str)]) -> ToolInput {
        let mut params: HashMap<String, serde_json::Value> = HashMap::new();
        params.insert(
            "command".to_string(),
            serde_json::Value::String(command.to_string()),
        );
        let map: serde_json::Map<String, serde_json::Value> = env
            .iter()
            .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
            .collect();
        params.insert("env".to_string(), serde_json::Value::Object(map));
        ToolInput::new(params)
    }

    /// The seed for #1489: a caller-supplied `PATH` pointing at a fake `echo`.
    ///
    /// With the refusal removed, the fake binary runs and creates the marker —
    /// which is why this asserts the **file**, not the failure type or the exit
    /// code. A refusal that still ran the binary would pass a status check.
    #[tokio::test]
    async fn a_caller_supplied_path_cannot_choose_the_executable() {
        let dir = TempDir::new().unwrap();
        let evil_dir = dir.path().join("evil");
        std::fs::create_dir_all(&evil_dir).unwrap();
        let marker = dir.path().join("marker");
        let fake = evil_dir.join("echo");
        std::fs::write(
            &fake,
            format!("#!/bin/sh\n: > {}\nprintf pwned\n", marker.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let tool = RunCommandTool::new(
            dir.path().to_str().unwrap(),
            vec!["echo"],
            SandboxDeclaration::container(),
        );
        let input = input_with_env("echo hi", &[("PATH", evil_dir.to_str().unwrap())]);

        let error = tool.execute(&input).await.unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("PATH"),
            "the refusal must name the key: {message}"
        );
        assert!(
            !marker.exists(),
            "the fake binary ran — the allowlist approved a word, the kernel ran another file"
        );
    }

    /// The loader is the same class one step earlier (#1489).
    #[tokio::test]
    async fn the_loader_families_are_refused_by_name() {
        let (tool, _dir) = allowlist_tool();
        for key in ["LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES"] {
            let error = tool
                .execute(&input_with_env("echo hi", &[(key, "/tmp/x.dylib")]))
                .await
                .unwrap_err();
            let message = error.to_string();
            assert!(matches!(error, ToolError::InvalidInput(_)), "{error:?}");
            assert!(
                message.contains(key),
                "the refusal must name {key}: {message}"
            );
            assert!(
                message.contains("loader"),
                "the refusal must say why: {message}"
            );
        }
    }

    /// The interpreter hooks are refused for the interpreters the allowlist is
    /// meant to admit.
    #[tokio::test]
    async fn interpreter_startup_hooks_are_refused_by_name() {
        let (tool, _dir) = allowlist_tool();
        for key in ["BASH_ENV", "PYTHONSTARTUP", "NODE_OPTIONS", "RUBYOPT"] {
            let error = tool
                .execute(&input_with_env("echo hi", &[(key, "/tmp/hook")]))
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains(key),
                "the refusal must name {key}: {error}"
            );
        }
    }

    /// An unknown key is an error naming the declared set, not a silent entry.
    #[tokio::test]
    async fn an_undeclared_key_is_refused_and_the_declared_set_is_named() {
        let (tool, _dir) = allowlist_tool();
        let error = tool
            .execute(&input_with_env("echo hi", &[("SOMETHING_ELSE", "x")]))
            .await
            .unwrap_err();
        let message = error.to_string();
        assert!(matches!(error, ToolError::InvalidInput(_)), "{error:?}");
        assert!(message.contains("SOMETHING_ELSE"), "{message}");
        assert!(
            message.contains("none — this tool accepts no caller environment"),
            "the message must state the declared set: {message}"
        );
    }

    /// A declared key is accepted, and the result names the file that ran.
    #[tokio::test]
    async fn a_declared_key_passes_and_the_result_names_the_executable() {
        let (tool, _dir) = allowlist_tool();
        let tool = tool.with_declared_env(["GREETING"]);
        let input = input_with_env("echo hi", &[("GREETING", "hello")]);
        let result = tool.execute(&input).await.unwrap();

        assert!(result.is_success(), "{:?}", result.output);
        let resolved = result
            .resolved_executable
            .expect("the result must name the executable that ran");
        assert!(
            resolved.contains("echo"),
            "recorded path does not look like echo: {resolved}"
        );
        assert!(
            std::path::Path::new(&resolved).is_absolute(),
            "the record must be the resolved path, not the approved word: {resolved}"
        );
    }

    /// The resolution happens against the engine's `PATH`, so the recorded path is
    /// independent of anything the caller sent (and a caller cannot send `PATH`).
    #[test]
    fn a_bare_name_resolves_in_the_engine_path_not_the_child_environment() {
        let resolved = resolve_program("echo").unwrap();
        assert!(resolved.is_absolute(), "{resolved:?}");
        let expected = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|dir| dir.join("echo"))
            .find(|candidate| candidate.is_file());
        assert_eq!(Some(resolved), expected);
    }

    // ── #1488: cwd is part of the boundary ───────────────────────────────

    fn input_with_cwd(command: &str, cwd: &str) -> ToolInput {
        let mut params: HashMap<String, serde_json::Value> = HashMap::new();
        params.insert(
            "command".to_string(),
            serde_json::Value::String(command.to_string()),
        );
        params.insert(
            "cwd".to_string(),
            serde_json::Value::String(cwd.to_string()),
        );
        ToolInput::new(params)
    }

    #[tokio::test]
    async fn an_absolute_cwd_outside_the_workspace_is_denied_before_any_spawn() {
        let (tool, _dir) = allowlist_tool();
        let error = tool
            .execute(&input_with_cwd("echo hi", "/tmp"))
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PathDenied(_)), "{error:?}");
        assert!(error.to_string().contains("workspace root"), "{error}");
    }

    #[tokio::test]
    async fn a_parent_component_in_cwd_is_denied() {
        let (tool, dir) = allowlist_tool();
        // Even when it happens to point back inside, `..` is refused: the check
        // would have to re-resolve to answer, which is the defect.
        let escape = format!("{}/../", dir.path().display());
        let error = tool
            .execute(&input_with_cwd("echo hi", &escape))
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PathDenied(_)), "{error:?}");
        assert!(error.to_string().contains(".."), "{error}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_symlinked_cwd_cannot_be_used_to_escape() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let link = dir.path().join("escape");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();

        let tool = RunCommandTool::new(
            dir.path().to_str().unwrap(),
            vec!["pwd"],
            SandboxDeclaration::container(),
        );
        let error = tool
            .execute(&input_with_cwd("pwd", "escape"))
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PathDenied(_)), "{error:?}");
        assert!(
            error.to_string().contains("symlink"),
            "the refusal must say why: {error}"
        );
    }

    #[tokio::test]
    async fn a_cwd_inside_the_workspace_still_works() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let tool = RunCommandTool::new(
            dir.path().to_str().unwrap(),
            vec!["pwd"],
            SandboxDeclaration::container(),
        );

        let result = tool.execute(&input_with_cwd("pwd", "sub")).await.unwrap();
        assert!(result.is_success(), "{:?}", result.output);
        let reported = std::path::Path::new(result.output.trim())
            .canonicalize()
            .unwrap();
        assert_eq!(reported, sub.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn the_default_cwd_is_the_workspace_root() {
        let (_allowlist_tool, dir) = allowlist_tool();
        let tool = RunCommandTool::new(
            dir.path().to_str().unwrap(),
            vec!["pwd"],
            SandboxDeclaration::container(),
        );
        let result = tool.execute(&make_input("pwd")).await.unwrap();
        assert!(result.is_success(), "{:?}", result.output);
        let reported = std::path::Path::new(result.output.trim())
            .canonicalize()
            .unwrap();
        assert_eq!(reported, dir.path().canonicalize().unwrap());
    }
}
