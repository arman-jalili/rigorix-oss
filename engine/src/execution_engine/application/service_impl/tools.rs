//! Tool-node execution (`execute_tool` + `exec_*`) for
//! `ParallelExecutionServiceImpl` (extracted from `service_impl.rs`, #914).

use super::*;

impl ParallelExecutionServiceImpl {
    /// Execute a single tool node and return the TaskResult.
    ///
    /// Execution order: permission check → PreToolUse hooks → tool → PostToolUse hooks.
    pub(super) async fn execute_tool(
        &self,
        node: &crate::dag_engine::domain::TaskNode,
        node_id: Uuid,
        start: std::time::Instant,
    ) -> TaskResult {
        let tool_name = node.tool.as_str();
        let tool_intent = &node.intent;

        // ── Permission gating ──
        if let Some(ref enforcer) = self.permission_enforcer {
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
                let bash_outcome = enforcer.check_bash(tool_intent, None).await;
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
        let result = match tool_name {
            "run_command" => Self::exec_run_command(tool_intent, node_id, &node.name, start).await,
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
        serde_json::from_str::<serde_json::Value>(intent)
            .ok()
            .and_then(|v| v.get(field).and_then(|v| v.as_str().map(String::from)))
            .unwrap_or_else(|| intent.to_string())
    }

    pub(super) async fn exec_run_command(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let command = Self::resolve_json_field(intent, "command");
        // GAP-A-12: bounded subprocess execution — never block the async
        // runtime on an unbounded `sh -c`.
        let output = tokio::time::timeout(
            SUBPROCESS_TIMEOUT,
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output(),
        )
        .await;
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
        // Ensure parent dir exists
        if let Some(parent) = std::path::Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        match tokio::fs::write(path, content).await {
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
        match tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .await
        {
            Ok(mut file) => {
                use tokio::io::AsyncWriteExt;
                match file.write_all(format!("{}\n", content).as_bytes()).await {
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

        let content = match tokio::fs::read_to_string(path).await {
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
                    match tokio::fs::write(path, &new_content).await {
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
            match tokio::fs::write(path, &new_content).await {
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

        let original = match tokio::fs::read_to_string(path).await {
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

        match tokio::fs::write(path, &updated).await {
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

    pub(super) async fn exec_git_read(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let args_str = Self::resolve_json_field(intent, "args");
        let args: Vec<&str> = args_str.split_whitespace().collect();
        let output = if args.is_empty() {
            tokio::time::timeout(GIT_TIMEOUT, tokio::process::Command::new("git").output()).await
        } else {
            tokio::time::timeout(
                GIT_TIMEOUT,
                tokio::process::Command::new("git").args(&args).output(),
            )
            .await
        };
        let duration_ms = start.elapsed().as_millis() as u64;
        match output {
            Ok(Ok(out)) => {
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
            Ok(Err(e)) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "exec_error".to_string(),
                duration_ms,
                0,
            ),
            Err(_elapsed) => TaskResult::failure(
                node_id,
                node_name,
                format!("git command timed out after {GIT_TIMEOUT:?}"),
                "git_timed_out".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    pub(super) async fn exec_git_stage(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let path = Self::resolve_json_field(intent, "path");
        let output = tokio::time::timeout(
            GIT_TIMEOUT,
            tokio::process::Command::new("git")
                .args(["add", path.as_str()])
                .output(),
        )
        .await;
        let duration_ms = start.elapsed().as_millis() as u64;
        match output {
            Ok(Ok(out)) => {
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
            Ok(Err(e)) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "exec_error".to_string(),
                duration_ms,
                0,
            ),
            Err(_elapsed) => TaskResult::failure(
                node_id,
                node_name,
                format!("git command timed out after {GIT_TIMEOUT:?}"),
                "git_timed_out".to_string(),
                duration_ms,
                0,
            ),
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
            let _ = tokio::time::timeout(
                GIT_TIMEOUT,
                tokio::process::Command::new("git")
                    .args(["add", "-u"])
                    .output(),
            )
            .await;
        }

        let output = tokio::time::timeout(
            GIT_TIMEOUT,
            tokio::process::Command::new("git")
                .args(["commit", "-m", message])
                .output(),
        )
        .await;
        match output {
            Ok(Ok(out)) => {
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
            Ok(Err(e)) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "exec_error".to_string(),
                duration_ms,
                0,
            ),
            Err(_elapsed) => TaskResult::failure(
                node_id,
                node_name,
                format!("git command timed out after {GIT_TIMEOUT:?}"),
                "git_timed_out".to_string(),
                duration_ms,
                0,
            ),
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
