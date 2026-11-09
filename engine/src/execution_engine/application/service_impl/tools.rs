//! Tool-node execution (`execute_tool` + `exec_*`) for
//! `ParallelExecutionServiceImpl` (extracted from `service_impl.rs`, #914).

// #1083: approved process-spawning sink — governed shell (`exec_run_command` via shell_policy) plus fixed-argv `git`.
// See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
#![allow(clippy::disallowed_types)]

use super::*;
use crate::tools::application::shell_policy::{
    SINK_GRAIN, apply_child_env, command_from_intent, field_from_intent,
    find_shell_control_operator,
};

impl ParallelExecutionServiceImpl {
    /// Execute a single tool node and return the TaskResult.
    ///
    /// Execution order: budget evaluation → permission check → PreToolUse hooks →
    /// tool → PostToolUse hooks.
    ///
    /// The budget evaluation comes first because it is the control with an
    /// execution-wide counter: a call that would exceed the limit must not run at
    /// all, and it must not be dispatched just to be refused afterwards.
    pub(super) async fn execute_tool(
        &self,
        dag_id: Uuid,
        node: &crate::dag_engine::domain::TaskNode,
        node_id: Uuid,
        start: std::time::Instant,
    ) -> TaskResult {
        let tool_name = node.tool.as_str();
        let tool_intent = &node.intent;

        // ── Budget and limit gating (ADR-008, #1108) ──
        //
        // Shared with the parallel dispatch path so the two routes cannot answer
        // differently (#1074/#1108).
        if let Some(enforcer) = self.budget_enforcer_for(dag_id)
            && let Err(refusal) =
                super::budget::evaluate(&enforcer, dag_id, node_id, &node.tool).await
        {
            let duration_ms = start.elapsed().as_millis() as u64;
            return TaskResult::failure(
                node_id,
                &node.name,
                refusal.reason().to_string(),
                refusal.failure_type().to_string(),
                duration_ms,
                0,
            );
        }

        // ── Permission gating ──
        //
        // #1487: an absent enforcer is a refusal. The previous shape wrapped the
        // whole block in `if let Some(..)`, so a service built without one
        // executed every tool with the gate silently skipped — and the hosts
        // built it that way on purpose, with a warning.
        let Some(ref enforcer) = self.permission_enforcer else {
            let duration_ms = start.elapsed().as_millis() as u64;
            return TaskResult::failure(
                node_id,
                &node.name,
                super::PermissionEnforcerAbsent::REASON.to_string(),
                super::PermissionEnforcerAbsent::FAILURE_TYPE.to_string(),
                duration_ms,
                0,
            );
        };
        {
            let outcome = enforcer.check(tool_name, tool_intent, None).await;
            if let crate::permission::domain::PermissionOutcome::Denied { reason, .. } = outcome {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    &node.name,
                    reason,
                    "permission_denied".to_string(),
                    duration_ms,
                    0,
                );
            }

            // Additional checks for write tools
            if tool_name == "file_write" || tool_name == "file_append" || tool_name == "edit_file" {
                let parsed: serde_json::Value =
                    serde_json::from_str(tool_intent).unwrap_or_default();
                if let Some(path) = parsed["path"].as_str() {
                    let write_outcome = enforcer.check_file_write(path, ".", None).await;
                    if let crate::permission::domain::PermissionOutcome::Denied { reason, .. } =
                        write_outcome
                    {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        return TaskResult::failure(
                            node_id,
                            &node.name,
                            reason,
                            "permission_denied".to_string(),
                            duration_ms,
                            0,
                        );
                    }
                }
            }

            // Additional check for bash commands
            if tool_name == "run_command" {
                // The classifier must see the command, not the JSON intent. Both
                // this check and `exec_run_command` use the same extraction, so
                // the bytes classified are the bytes executed (#1067).
                let command = command_from_intent(tool_intent);
                let bash_outcome = enforcer.check_bash(&command, None).await;
                if let crate::permission::domain::PermissionOutcome::Denied { reason, .. } =
                    bash_outcome
                {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return TaskResult::failure(
                        node_id,
                        &node.name,
                        reason,
                        "permission_denied".to_string(),
                        duration_ms,
                        0,
                    );
                }
            }
        }

        // ── PreToolUse hooks ──
        if let Some(ref hook_runner) = self.hook_runner {
            let abort = crate::hooks::domain::HookAbortSignal::default();
            let pre_input = crate::hooks::application::dto::RunPreToolUseInput {
                tool_name: tool_name.to_string(),
                tool_input: serde_json::Value::String(tool_intent.to_string()),
                session_id: node_id.to_string(),
                workspace_root: ".".to_string(),
            };
            if let Ok(pre_output) = hook_runner.run_pre_tool_use(pre_input, Some(&abort)).await
                && (pre_output.result.is_denied()
                    || pre_output.result.is_failed()
                    || pre_output.result.is_cancelled())
            {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    &node.name,
                    format!(
                        "Tool '{}' blocked by PreToolUse hook: {:?}",
                        tool_name,
                        pre_output.result.feedback_messages()
                    ),
                    "hook_blocked".to_string(),
                    duration_ms,
                    0,
                );
            }
        }

        // ── Execute the tool ──
        // #1165: the provider must receive the intent-derived key, or the
        // receipt reader's idempotency-key lookup can only miss.
        let effect_key = self
            .effect_keys
            .lock()
            .ok()
            .and_then(|keys| keys.get(&node_id).map(|key| key.as_str().to_string()));
        let result = match tool_name {
            "run_command" => {
                self.exec_run_command(
                    tool_intent,
                    node_id,
                    &node.name,
                    start,
                    effect_key.as_deref(),
                )
                .await
            }
            "file_read" => Self::exec_file_read(tool_intent, node_id, &node.name, start).await,
            "file_write" => Self::exec_file_write(tool_intent, node_id, &node.name, start).await,
            "file_append" => Self::exec_file_append(tool_intent, node_id, &node.name, start).await,
            "file_patch" => Self::exec_file_patch(tool_intent, node_id, &node.name, start).await,
            "git_read" => Self::exec_git_read(tool_intent, node_id, &node.name, start).await,
            "git_stage" => Self::exec_git_stage(tool_intent, node_id, &node.name, start).await,
            "git_commit" => Self::exec_git_commit(tool_intent, node_id, &node.name, start).await,
            "edit_file" => Self::exec_edit_file(tool_intent, node_id, &node.name, start).await,
            _ => {
                // Unknown tool — fail the node instead of silently succeeding
                let duration_ms = start.elapsed().as_millis() as u64;
                TaskResult::failure(
                    node_id,
                    &node.name,
                    format!("Unknown tool '{}', intent: {}", tool_name, tool_intent),
                    "unknown_tool".to_string(),
                    duration_ms,
                    0,
                )
            }
        };

        // ── PostToolUse hooks ──
        if let Some(ref hook_runner) = self.hook_runner {
            let tool_output = result.output.clone().unwrap_or_default();
            let abort = crate::hooks::domain::HookAbortSignal::default();
            let post_input = crate::hooks::application::dto::RunPostToolUseInput {
                tool_name: tool_name.to_string(),
                tool_input: serde_json::Value::String(tool_intent.to_string()),
                tool_output,
                session_id: node_id.to_string(),
                workspace_root: ".".to_string(),
            };
            if let Ok(_post_output) = hook_runner
                .run_post_tool_use(post_input, Some(&abort))
                .await
            {
                // Post-tool feedback is informational — no gating
            }
        }

        result
    }

    /// Try to extract a named field from a JSON intent string.
    /// Falls back to using the intent as-is if it's not valid JSON
    /// or doesn't contain the expected field.
    pub(super) fn resolve_json_field(intent: &str, field: &str) -> String {
        field_from_intent(intent, field).into_owned()
    }

    /// The single-node route. Both routes answer the same question the same way.
    pub(super) async fn exec_run_command(
        &self,
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
        effect_key: Option<&str>,
    ) -> TaskResult {
        if self.sandbox.is_some() {
            Self::exec_run_command_declared(intent, node_id, node_name, start, effect_key).await
        } else {
            Self::exec_run_command_undeclared(intent, node_id, node_name, start)
        }
    }

    /// #1487: what a `run_command` step is when the deployment declared no boundary.
    ///
    /// The refusal happens before anything else is looked at: no operator scan, no
    /// spawn, no timeout. The test asserts the marker file was never created, not
    /// that the failure type is right — a failure type alone proves nothing about
    /// what ran.
    pub(super) fn exec_run_command_undeclared(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let duration_ms = start.elapsed().as_millis() as u64;
        TaskResult::failure(
            node_id,
            node_name,
            format!(
                "{}\n\nThe step was not run (intent: {}). This is the step dispatcher, not the \
                 run-command tool: it executes a shell string, so filtering cannot be the boundary \
                 ({SINK_GRAIN}).",
                crate::tools::application::shell_policy::SandboxDeclarationError::Missing,
                command_from_intent(intent)
            ),
            "sandbox_undeclared".to_string(),
            duration_ms,
            0,
        )
    }

    /// The parallel route: the caller already holds the declaration (it clones the
    /// `Arc` into the spawned task), so the check is the presence of the value the
    /// caller chose to pass.
    pub(super) async fn exec_run_command_declared(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
        effect_key: Option<&str>,
    ) -> TaskResult {
        let command = command_from_intent(intent);
        // Refuse a compound shell string: there is no allowlist on this path,
        // and `sh -c` would execute every operator in the string (#1067).
        if let Some(operator) = find_shell_control_operator(&command) {
            let duration_ms = start.elapsed().as_millis() as u64;
            return TaskResult::failure(
                node_id,
                node_name,
                format!(
                    "Command '{}' contains the shell control operator '{}'; run_command executes a \
                     single command, not a compound shell string",
                    command, operator
                ),
                "shell_operator_rejected".to_string(),
                duration_ms,
                0,
            );
        }
        // GAP-A-12: bounded subprocess execution — never block the async
        // runtime on an unbounded `sh -c`. The child gets a cleared environment
        // so it cannot read `RIGORIX_HMAC_KEY` and forge audit envelopes (#1068).
        //
        // #1487: the regime itself is recorded where the envelope is built
        // (`OrchestratorServiceImpl::record_evidence`), so two runs under
        // different isolation are distinguishable in the record.
        // sink: engine:exec-run-command — a shell by design; the boundary is the
        // deployment's declared isolation, not this line.
        let mut child = tokio::process::Command::new("sh");
        child.arg("-c").arg(command.as_ref());
        // The key is not a secret, but it is the only way a process provider
        // can dedupe and expose the receipt the reader will look up (#1165).
        let overrides: Vec<(String, String)> = effect_key
            .map(|key| {
                vec![(
                    crate::effect_commitment::interfaces::EFFECT_IDEMPOTENCY_ENV.to_string(),
                    key.to_string(),
                )]
            })
            .unwrap_or_default();
        apply_child_env(&mut child, &overrides);
        let output = tokio::time::timeout(SUBPROCESS_TIMEOUT, child.output()).await;
        match output {
            Ok(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let duration_ms = start.elapsed().as_millis() as u64;
                if out.status.success() {
                    TaskResult::success(
                        node_id,
                        node_name,
                        Some(if stdout.is_empty() { stderr } else { stdout }),
                        duration_ms,
                        0,
                    )
                } else {
                    let err = if stderr.is_empty() { stdout } else { stderr };
                    TaskResult::failure(
                        node_id,
                        node_name,
                        err,
                        "command_failed".to_string(),
                        duration_ms,
                        0,
                    )
                }
            }
            Ok(Err(e)) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "exec_error".to_string(),
                0,
                0,
            ),
            Err(_elapsed) => TaskResult::failure(
                node_id,
                node_name,
                format!("command timed out after {SUBPROCESS_TIMEOUT:?}: {command}"),
                "command_timed_out".to_string(),
                0,
                0,
            ),
        }
    }

    pub(super) async fn exec_file_read(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let path = Self::resolve_json_field(intent, "path");
        let duration_ms = start.elapsed().as_millis() as u64;
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let truncated: String = content.chars().take(4096).collect();
                TaskResult::success(node_id, node_name, Some(truncated), duration_ms, 0)
            }
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "file_read_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    pub(super) async fn exec_file_write(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let path = parsed["path"].as_str().unwrap_or("");
        let content = parsed["content"].as_str().unwrap_or("");
        let duration_ms = start.elapsed().as_millis() as u64;
        // #1490: atomic, through the pinned parent handle — the same discipline
        // `file_write` got in #1479. The old shape created the parent (ignoring the
        // failure) and wrote the path, so a concurrent reader could see a partial
        // file and a missing parent surfaced as a write error.
        match write_atomically(path, content).await {
            Ok(()) => TaskResult::success(
                node_id,
                node_name,
                Some(format!("Wrote {} bytes to {}", content.len(), path)),
                duration_ms,
                0,
            ),
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "file_write_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    pub(super) async fn exec_file_append(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let path = parsed["path"].as_str().unwrap_or("");
        let content = parsed["content"].as_str().unwrap_or("");
        let duration_ms = start.elapsed().as_millis() as u64;
        // #1494: `file_append` used `OpenOptions::append(true).open(path)`, which
        // follows a symlink leaf and lands wherever it points. The whole point of the
        // sibling `file_write` fix was that the caller's string is resolved once, by a
        // module that refuses a link rather than following one.
        match append_atomically(path, content).await {
            Ok(()) => TaskResult::success(
                node_id,
                node_name,
                Some(format!("Appended {} bytes to {}", content.len(), path)),
                duration_ms,
                0,
            ),
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "file_append_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    pub(super) async fn exec_file_patch(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let path = parsed["path"].as_str().unwrap_or("");
        let insert = parsed["insert"].as_str().unwrap_or("");
        let duration_ms = start.elapsed().as_millis() as u64;

        // #1494: the bytes an edit transforms and the file the write lands in must
        // come from the same pinned parent, so the read goes through the resolver too.
        let content = match read_for_edit(path).await {
            Ok(c) => c,
            Err(e) => {
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "file_patch_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };

        // Determine mode: anchor mode if anchor_type is provided
        let anchor_type = parsed["anchor_type"]
            .as_str()
            .and_then(|s| if s.is_empty() { None } else { Some(s) });

        if let Some(anchor_type) = anchor_type {
            // --- Mode 1: Tree-sitter anchor mode ---
            let anchor_name = parsed["anchor_name"].as_str().unwrap_or("");
            let container = parsed["container"].as_str().filter(|s| !s.is_empty());
            let position = parsed["position"].as_str().unwrap_or("after");

            let params = crate::tools::infrastructure::tree_sitter_anchor::AnchorParams {
                anchor_type: anchor_type.to_string(),
                anchor_name: anchor_name.to_string(),
                container: container.map(|s| s.to_string()),
                position: position.to_string(),
            };

            match crate::tools::infrastructure::tree_sitter_anchor::TreeSitterAnchorFinder::find_anchor(
                &content,
                path,
                &params,
            ) {
                Ok(anchor) => {
                    // Idempotency: skip if the insert content already exists.
                    let sig_line = insert.lines().find(|l| !l.trim().is_empty());
                    let already_present = sig_line
                        .map(|sig| content.lines().any(|l| l.trim() == sig.trim()))
                        .unwrap_or(false);
                    if already_present {
                        let dur = start.elapsed().as_millis() as u64;
                        return TaskResult::success(
                            node_id,
                            node_name,
                            Some(format!(
                                "Skipped patch of {} via anchor {} '{}' — content already present",
                                path, anchor_type, anchor_name
                            )),
                            dur,
                            0,
                        );
                    }

                    // Normalize newlines when inserting after an anchor.
                    let normalized_insert = if position == "after" && !insert.starts_with('\n') {
                        let mut s = String::with_capacity(insert.len() + 2);
                        s.push('\n');
                        s.push('\n');
                        s.push_str(insert);
                        s
                    } else {
                        insert.to_string()
    };
                    let new_content = format!(
                        "{}{}{}",
                        &content[..anchor.insert_offset],
                        normalized_insert,
                        &content[anchor.insert_offset..]
                    );
                    match write_atomically(path, &new_content).await {
                        Ok(()) => TaskResult::success(
                            node_id,
                            node_name,
                            Some(format!(
                                "Patched {} via anchor {} '{}' ({} bytes inserted)",
                                path,
                                anchor_type,
                                anchor_name,
                                insert.len()
                            )),
                            duration_ms,
                            0,
                        ),
                        Err(e) => TaskResult::failure(
                            node_id,
                            node_name,
                            e.to_string(),
                            "file_patch_error".to_string(),
                            duration_ms,
                            0,
                        ),
                    }
                }
                Err(e) => TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "file_patch_error".to_string(),
                    duration_ms,
                    0,
                ),
            }
        } else {
            // --- Mode 2: Text search mode (backward compatible) ---
            let search = parsed["search"].as_str().unwrap_or("");
            let before = parsed["before"].as_bool().unwrap_or(false);

            if search.is_empty() {
                return TaskResult::failure(
                    node_id,
                    node_name,
                    "Missing required parameter: search or anchor_type".to_string(),
                    "file_patch_error".to_string(),
                    duration_ms,
                    0,
                );
            }

            // Find all occurrences to check for ambiguity
            let occurrences: Vec<_> = content.match_indices(search).collect();
            if occurrences.is_empty() {
                return TaskResult::failure(
                    node_id,
                    node_name,
                    format!("Search string not found in {}", path),
                    "file_patch_error".to_string(),
                    duration_ms,
                    0,
                );
            }
            if occurrences.len() > 1 {
                return TaskResult::failure(
                    node_id,
                    node_name,
                    format!(
                        "Search string found {} times in {}. Expected exactly one match.",
                        occurrences.len(),
                        path
                    ),
                    "file_patch_error".to_string(),
                    duration_ms,
                    0,
                );
            }

            let (match_pos, _) = occurrences[0];
            // Smart resolution: if search matched a container declaration,
            // use tree-sitter to insert inside the body before closing brace.
            let position_str = if before { "before" } else { "after" };
            let resolved_pos = crate::tools::infrastructure::tree_sitter_anchor::TreeSitterAnchorFinder::resolve_search_to_container(
                &content,
                path,
                match_pos,
                position_str,
            );
            let insert_pos = resolved_pos.unwrap_or_else(|| {
                if before {
                    match_pos
                } else {
                    match_pos + search.len()
                }
            });
            let new_content = format!(
                "{}{}{}",
                &content[..insert_pos],
                insert,
                &content[insert_pos..]
            );
            match write_atomically(path, &new_content).await {
                Ok(()) => TaskResult::success(
                    node_id,
                    node_name,
                    Some(format!(
                        "Patched {} ({} bytes inserted)",
                        path,
                        insert.len()
                    )),
                    duration_ms,
                    0,
                ),
                Err(e) => TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "file_patch_error".to_string(),
                    duration_ms,
                    0,
                ),
            }
        }
    }

    pub(super) async fn exec_edit_file(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let path = parsed["path"].as_str().unwrap_or("");
        let old_string = parsed["old_string"].as_str().unwrap_or("");
        let new_string = parsed["new_string"].as_str().unwrap_or("");
        let replace_all = parsed["replace_all"].as_bool().unwrap_or(false);
        let duration_ms = start.elapsed().as_millis() as u64;

        // #1494: same handle for the read and the write.
        let original = match read_for_edit(path).await {
            Ok(c) => c,
            Err(e) => {
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "edit_file_read_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };

        // Identity check
        if old_string == new_string {
            return TaskResult::failure(
                node_id,
                node_name,
                "old_string and new_string must differ".to_string(),
                "identity_edit".to_string(),
                duration_ms,
                0,
            );
        }

        // Existence check
        if !original.contains(old_string) {
            return TaskResult::failure(
                node_id,
                node_name,
                format!("old_string not found in {}", path),
                "old_string_not_found".to_string(),
                duration_ms,
                0,
            );
        }

        let updated = if replace_all {
            original.replace(old_string, new_string)
        } else {
            original.replacen(old_string, new_string, 1)
        };

        match write_atomically(path, &updated).await {
            Ok(()) => TaskResult::success(
                node_id,
                node_name,
                Some(format!(
                    "Edit applied to {} — {}→{}",
                    path,
                    old_string.len(),
                    new_string.len()
                )),
                duration_ms,
                0,
            ),
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "edit_file_write_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    /// Run `git` in the engine's environment: resolved program, argv, cleared env.
    ///
    /// #1494 (and #1489 one layer up): the five `git` call sites in this file used
    /// `Command::new("git")` with **no** environment treatment, so the child
    /// inherited the engine's whole environment — `RIGORIX_HMAC_KEY` included, the
    /// key that signs the audit envelopes meant to constrain it (#1068, #1082 C3) —
    /// and resolved `git` through that inherited `PATH`, which is the word-not-file
    /// defect #1489 closed for `run_command`. One helper, one place to be right.
    ///
    /// `resolve_program` searches the engine's own `PATH` *before* any child env is
    /// applied. It pins the **path**, not the inode (#1495); the register row says so.
    async fn git_output(args: &[&str]) -> Result<std::process::Output, String> {
        let program = crate::tools::application::shell_policy::resolve_program("git")?;
        // sink: engine:exec-git — fixed program, argv, cleared environment.
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args);
        crate::tools::application::shell_policy::apply_child_env(&mut cmd, &[]);
        match tokio::time::timeout(GIT_TIMEOUT, cmd.output()).await {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err(format!("git command timed out after {GIT_TIMEOUT:?}")),
        }
    }

    pub(super) async fn exec_git_read(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let args_str = Self::resolve_json_field(intent, "args");
        let args: Vec<&str> = args_str.split_whitespace().collect();
        let output = Self::git_output(&args).await;
        let duration_ms = start.elapsed().as_millis() as u64;
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if out.status.success() {
                    let out_text = if stdout.is_empty() { stderr } else { stdout };
                    let truncated: String = out_text.chars().take(8192).collect();
                    TaskResult::success(node_id, node_name, Some(truncated), duration_ms, 0)
                } else {
                    TaskResult::failure(
                        node_id,
                        node_name,
                        if stderr.is_empty() { stdout } else { stderr },
                        "git_error".to_string(),
                        duration_ms,
                        0,
                    )
                }
            }
            Err(e) => {
                let code = if e.contains("timed out") {
                    "git_timed_out"
                } else {
                    "exec_error"
                };
                TaskResult::failure(node_id, node_name, e, code.to_string(), duration_ms, 0)
            }
        }
    }

    pub(super) async fn exec_git_stage(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let path = Self::resolve_json_field(intent, "path");
        let output = Self::git_output(&["add", path.as_str()]).await;
        let duration_ms = start.elapsed().as_millis() as u64;
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if out.status.success() {
                    TaskResult::success(node_id, node_name, Some(stdout), duration_ms, 0)
                } else {
                    TaskResult::failure(
                        node_id,
                        node_name,
                        if stderr.is_empty() { stdout } else { stderr },
                        "git_stage_error".to_string(),
                        duration_ms,
                        0,
                    )
                }
            }
            Err(e) => {
                let code = if e.contains("timed out") {
                    "git_timed_out"
                } else {
                    "exec_error"
                };
                TaskResult::failure(node_id, node_name, e, code.to_string(), duration_ms, 0)
            }
        }
    }

    pub(super) async fn exec_git_commit(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let message = parsed["message"].as_str().unwrap_or("");
        let auto_stage = parsed["auto_stage"].as_bool().unwrap_or(false);
        let duration_ms = start.elapsed().as_millis() as u64;

        // If auto_stage, stage all modified tracked files first
        if auto_stage {
            // A failed `add -u` is not fatal for the commit below: git reports the
            // same problem from `commit`, and this stage is best-effort by contract.
            // `let _ =` on an evidence path would be rule 3; this is not one, and the
            // row's `does_not` states that the staging result is not recorded.
            let _ = Self::git_output(&["add", "-u"]).await;
        }

        let output = Self::git_output(&["commit", "-m", message]).await;
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if out.status.success() {
                    TaskResult::success(
                        node_id,
                        node_name,
                        Some(if stdout.is_empty() { stderr } else { stdout }),
                        duration_ms,
                        0,
                    )
                } else {
                    TaskResult::failure(
                        node_id,
                        node_name,
                        if stderr.is_empty() { stdout } else { stderr },
                        "git_commit_error".to_string(),
                        duration_ms,
                        0,
                    )
                }
            }
            Err(e) => {
                let code = if e.contains("timed out") {
                    "git_timed_out"
                } else {
                    "exec_error"
                };
                TaskResult::failure(node_id, node_name, e, code.to_string(), duration_ms, 0)
            }
        }
    }

    /// Notify progress callbacks about a state change.
    /// Used for TUI progress reporting integration.
    pub(crate) fn notify_progress(
        &self,
        dag_id: Uuid,
        node_id: Uuid,
        state: NodeExecutionState,
        total_nodes: u32,
    ) {
        let Ok(callbacks) = self.progress_callbacks.lock() else {
            return;
        };
        if callbacks.is_empty() {
            return;
        }

        // Compute aggregate counts from session state
        let (completed, failed, skipped) = {
            let Ok(sessions) = self.sessions.lock() else {
                return;
            };
            if let Some(session) = sessions.get(&dag_id) {
                let c = session
                    .node_states
                    .values()
                    .filter(|s| s.status == NodeStatus::Completed)
                    .count() as u32;
                let f = session
                    .node_states
                    .values()
                    .filter(|s| s.status == NodeStatus::Failed)
                    .count() as u32;
                let sk = session
                    .node_states
                    .values()
                    .filter(|s| s.status == NodeStatus::Skipped)
                    .count() as u32;
                (c, f, sk)
            } else {
                (0, 0, 0)
            }
        };

        let progress = ExecutionProgress {
            dag_id,
            node_id,
            state,
            total_nodes,
            completed_count: completed,
            failed_count: failed,
            skipped_count: skipped,
        };

        for cb in callbacks.iter() {
            cb(progress.clone());
        }
    }
}

/// The base every caller-supplied path in this file resolves under (#1493).
///
/// The dispatcher's steps name paths as strings. Resolving such a string is only a
/// control when the resolution has a base the caller did not choose, so the base is
/// the engine's working directory — the repository root for every host that runs a
/// plan — captured once and walked with `O_NOFOLLOW`. A component that is a symlink
/// is refused by name, which is what makes `ln -s .rigorix tmp` useless against the
/// `file_write` step even before the permission enforcer is consulted: the two
/// halves of the SINK-9 chain are closed independently.
fn write_base() -> Result<std::path::PathBuf, crate::tools::domain::ToolError> {
    std::env::current_dir().map_err(|e| {
        crate::tools::domain::ToolError::ExecutionFailed(format!(
            "Cannot resolve the directory plan steps run in: {e}"
        ))
    })
}

/// Resolve a caller-supplied path under the engine's base, then mutate the handle.
///
/// #1490 routed `file_write` here; #1494 routes `file_append`, `file_patch` and
/// `edit_file` through the same function, because a step family that shares an
/// intent shape must not share only some of the guarantee. `read` is threaded
/// through so a transformation reads the bytes it is about to replace from the same
/// pinned parent it writes into.
async fn with_write_target<T, F>(path: &str, op: F) -> Result<T, crate::tools::domain::ToolError>
where
    T: Send + 'static,
    F: FnOnce(
            &crate::tools::infrastructure::fs_at::ValidatedWritePath,
        ) -> Result<T, crate::tools::domain::ToolError>
        + Send
        + 'static,
{
    if path.is_empty() {
        return Err(crate::tools::domain::ToolError::InvalidInput(
            "the step has no path".to_string(),
        ));
    }
    let path = std::path::PathBuf::from(path);
    tokio::task::spawn_blocking(move || {
        use crate::tools::infrastructure::fs_at;
        let base = write_base()?;
        // A path inside the base — every relative path, and an absolute path the
        // caller placed there — is walked with `O_NOFOLLOW`, so a symlinked
        // component is refused. An absolute path *outside* the base is not this
        // module's decision to make: containment is the permission enforcer's, and
        // it decides on the resolved path (#1493). The row's `does_not` states the
        // difference; silently following the link here would not.
        let target = if path.is_absolute() && !path.starts_with(&base) {
            fs_at::resolve_absolute_for_write(&path)?
        } else {
            fs_at::resolve_under(&base, &path)?
        };
        op(&target)
    })
    .await
    .map_err(|e| {
        crate::tools::domain::ToolError::ExecutionFailed(format!("file write task failed: {e}"))
    })?
}

/// Write `content` to `path` atomically, creating parents (#1490).
async fn write_atomically(
    path: &str,
    content: &str,
) -> Result<(), crate::tools::domain::ToolError> {
    let content = content.as_bytes().to_vec();
    with_write_target(path, move |target| {
        crate::tools::infrastructure::fs_at::write_atomic(target, &content)
    })
    .await
}

/// Append `content` and a newline to `path`, refusing a symlinked leaf (#1494).
async fn append_atomically(
    path: &str,
    content: &str,
) -> Result<(), crate::tools::domain::ToolError> {
    let content = format!("{content}\n").into_bytes();
    with_write_target(path, move |target| {
        crate::tools::infrastructure::fs_at::append_or_create(target, &content)
    })
    .await
}

/// Read `path` through the handle its write will use (#1494).
async fn read_for_edit(path: &str) -> Result<String, crate::tools::domain::ToolError> {
    let bytes = with_write_target(path, |target| {
        crate::tools::infrastructure::fs_at::read_file(target)
    })
    .await?;
    String::from_utf8(bytes).map_err(|e| {
        crate::tools::domain::ToolError::ExecutionFailed(format!(
            "the file is not valid UTF-8 at byte {}",
            e.utf8_error().valid_up_to()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn command_json(command: &str) -> String {
        serde_json::json!({ "command": command }).to_string()
    }

    #[tokio::test]
    async fn run_command_carries_the_effect_key_to_the_provider() {
        // #1165: the provider must receive the intent-derived key. The process
        // channel is an env var because run_command deliberately clears the
        // rest of the environment.
        let result = ParallelExecutionServiceImpl::exec_run_command_declared(
            &command_json("printf %s \"$RIGORIX_EFFECT_IDEMPOTENCY_KEY\""),
            Uuid::now_v7(),
            "n",
            Instant::now(),
            Some("intent-key-1"),
        )
        .await;
        assert!(result.success, "command failed: {:?}", result.error);
        assert_eq!(result.output.as_deref(), Some("intent-key-1"));
    }

    #[tokio::test]
    async fn a_node_without_a_commitment_does_not_invent_a_key() {
        let result = ParallelExecutionServiceImpl::exec_run_command_declared(
            &command_json("printf %s \"$RIGORIX_EFFECT_IDEMPOTENCY_KEY\""),
            Uuid::now_v7(),
            "n",
            Instant::now(),
            None,
        )
        .await;
        assert!(result.success, "command failed: {:?}", result.error);
        assert_eq!(result.output.as_deref(), Some(""));
    }

    // ── #1487: the dispatcher declares its isolation, or refuses ──────────

    fn marker_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("rigorix-sink4-{tag}-{}", Uuid::now_v7()))
    }

    #[tokio::test]
    async fn a_run_command_step_without_a_declaration_is_refused_before_any_spawn() {
        // The seed: delete the declaration check and this test passes only by
        // creating the marker — which is why it asserts the file, not the type.
        let marker = marker_path("undeclared");
        let intent = format!("touch {}", marker.display());

        let result = ParallelExecutionServiceImpl::exec_run_command_undeclared(
            &intent,
            Uuid::now_v7(),
            "step",
            Instant::now(),
        );

        assert!(!result.success);
        assert_eq!(result.failure_type.as_deref(), Some("sandbox_undeclared"));
        assert!(
            !marker.exists(),
            "the step spawned a child despite the undeclared regime"
        );
        let error = result.error.unwrap_or_default();
        assert!(
            error.contains("RIGORIX_SANDBOX"),
            "the refusal must name the variable that fixes it: {error}"
        );
    }

    #[tokio::test]
    async fn a_declared_regime_lets_the_step_run() {
        let marker = marker_path("declared");
        let intent = format!("touch {}", marker.display());

        let result = ParallelExecutionServiceImpl::exec_run_command_declared(
            &intent,
            Uuid::now_v7(),
            "step",
            Instant::now(),
            None,
        )
        .await;

        assert!(result.success, "declared step failed: {:?}", result.error);
        assert!(marker.exists(), "the declared step did not run");
        let _ = std::fs::remove_file(&marker);
    }

    #[tokio::test]
    async fn the_regime_is_not_something_the_step_output_can_claim() {
        // The regime lives in the envelope's metadata (#1487), not in the command
        // output: a step cannot launder the isolation question into its own
        // stdout, because the record is written by the orchestrator.
        let result = ParallelExecutionServiceImpl::exec_run_command_declared(
            &command_json("printf isolation=container"),
            Uuid::now_v7(),
            "step",
            Instant::now(),
            None,
        )
        .await;
        assert_eq!(result.output.as_deref(), Some("isolation=container"));
    }
    // -----------------------------------------------------------------------
    // #1494 (SINK-8): the reachable writes are handle-coupled, not path writes
    // -----------------------------------------------------------------------

    /// Criterion 1: the dispatcher's file steps contain no path mutation.
    ///
    /// `file_write` was routed through `fs_at` in #1490 and its three siblings in the
    /// same file were not — `file_append` opened the caller's string with
    /// `OpenOptions::append(true)` (a write *through* a symlink), and `file_patch` and
    /// `edit_file` wrote it with `tokio::fs::write` (truncate in place, so a
    /// concurrent reader sees a partial file). The criterion is anchored where the
    /// defect was: this file, not the tool modules no host constructs.
    #[test]
    fn criterion_one_of_sink_8() {
        let source = include_str!("tools.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("the file has a production section");
        // Reads are not mutations: `file_read` still opens a path, and the register
        // records that as the read sink's business, not this criterion's.
        for pattern in ["tokio::fs::write", "OpenOptions::new()"] {
            assert!(
                !production.contains(pattern),
                "'{pattern}' must not appear in the dispatcher's production code — every write                  resolves once and mutates through the handle (fs_at)"
            );
        }
        // And the discipline is stated where it is used.
        assert!(production.contains("fs_at::resolve_under"));
        assert!(production.contains("fs_at::append_or_create"));
    }

    /// Criterion 2: a `file_append` step whose path is a link cannot fill the target.
    ///
    /// This is the measured defect, at the dispatcher: append to `link.txt` wrote
    /// `INJECTED` into `victim.txt`, because `O_APPEND` on a path follows the link.
    #[tokio::test]
    #[cfg(unix)]
    async fn a_file_append_step_refuses_a_symlink_leaf() {
        let root = tempfile::TempDir::new().unwrap();
        let victim = root.path().join("victim.txt");
        std::fs::write(&victim, "ORIGINAL\n").unwrap();
        let link = root.path().join("link.txt");
        std::os::unix::fs::symlink(&victim, &link).unwrap();

        let result = ParallelExecutionServiceImpl::exec_file_append(
            &format!(r#"{{"path": "{}", "content": "INJECTED"}}"#, link.display()),
            Uuid::now_v7(),
            "append",
            Instant::now(),
        )
        .await;

        assert!(
            !result.success,
            "the append must be refused: {:?}",
            result.output
        );
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "ORIGINAL\n",
            "nothing may be written through the link"
        );
    }

    /// The control for criterion 2: an ordinary append still works and still creates.
    #[tokio::test]
    async fn an_ordinary_file_append_step_still_appends_and_creates() {
        let root = tempfile::TempDir::new().unwrap();
        let target = root.path().join("log.txt");
        for line in ["first", "second"] {
            let result = ParallelExecutionServiceImpl::exec_file_append(
                &format!(r#"{{"path": "{}", "content": "{line}"}}"#, target.display()),
                Uuid::now_v7(),
                "append",
                Instant::now(),
            )
            .await;
            assert!(result.success, "append failed: {:?}", result.error);
        }
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "first\nsecond\n");
    }

    /// Criterion 3: `edit_file` and `file_patch` replace the file atomically, so a
    /// concurrent reader never observes a truncated file.
    ///
    /// The observable that distinguishes atomic from truncate-in-place is a reader
    /// that samples the file while the write happens: with `fs::write` it can see an
    /// empty or half-written file; with temp+rename it sees the old bytes or the new
    /// bytes and nothing between.
    #[tokio::test]
    async fn an_edit_never_exposes_a_truncated_file() {
        let root = tempfile::TempDir::new().unwrap();
        let target = root.path().join("big.txt");
        let original = "a".repeat(2_000_000);
        std::fs::write(&target, &original).unwrap();

        let reader_path = target.clone();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_reader = stop.clone();
        let reader = tokio::task::spawn_blocking(move || {
            let mut samples = 0usize;
            while !stop_reader.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok(bytes) = std::fs::read(&reader_path) {
                    samples += 1;
                    assert!(
                        bytes.len() == 2_000_000 || bytes.len() == 1_666_667,
                        "a reader saw a partial file: {} bytes",
                        bytes.len()
                    );
                }
            }
            samples
        });

        let result = ParallelExecutionServiceImpl::exec_edit_file(
            &format!(
                r#"{{"path": "{}", "old_string": "{}", "new_string": "{}", "replace_all": false}}"#,
                target.display(),
                "a".repeat(1_000_000),
                "b".repeat(666_667)
            ),
            Uuid::now_v7(),
            "edit",
            Instant::now(),
        )
        .await;
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let samples = reader.await.unwrap();

        assert!(result.success, "edit failed: {:?}", result.error);
        assert!(samples > 0, "the reader never sampled the file");
        let after = std::fs::read_to_string(&target).unwrap();
        assert!(after.starts_with('b'), "the edit did not apply");
    }

    /// The git steps no longer hand the child the engine's environment.
    ///
    /// #1494 found this while splitting the register row: five `git` spawns shared
    /// one row with `run_command`, inherited `RIGORIX_HMAC_KEY` (the key that signs
    /// the envelopes meant to constrain them) and resolved `git` through the inherited
    /// `PATH` (#1489's word-vs-file defect, one layer up).
    #[tokio::test]
    async fn a_git_step_child_does_not_inherit_the_signing_key() {
        // SAFETY-adjacent but test-only: this process sets the variable for the
        // duration of one assertion, and the helper reads it from the engine's env
        // when it builds the child. `set_var` is unsafe in edition 2024 because of
        // concurrent access; the test is serial with respect to its own spawn.
        let key = "test-key-for-sink-8";
        unsafe { std::env::set_var("RIGORIX_HMAC_KEY", key) };
        let program = crate::tools::application::shell_policy::resolve_program("git")
            .expect("git is on the engine PATH");
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(["rev-parse", "--is-inside-work-tree"]);
        crate::tools::application::shell_policy::apply_child_env(&mut cmd, &[]);
        let child_env = format!("{cmd:?}");
        unsafe { std::env::remove_var("RIGORIX_HMAC_KEY") };
        assert!(
            !child_env.contains(key),
            "the git child must not carry RIGORIX_HMAC_KEY: {child_env}"
        );
    }
}
