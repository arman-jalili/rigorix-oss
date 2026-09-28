//! Assembled helper methods for `OrchestratorServiceImpl` (extracted from
//! `orchestrator_impl.rs`, #916). A child module of the impl type, so it can
//! access its private fields; methods the coordinator calls are `pub(super)`.

use super::*;

impl OrchestratorServiceImpl {
    pub(super) fn gen_id(&self) -> Uuid {
        Uuid::now_v7()
    }

    /// Collect scoring results from the scored evaluation service for an execution.
    /// Returns an empty map if the service is not configured or if no results exist.
    pub(super) async fn collect_scoring_results(
        &self,
        execution_id: Uuid,
    ) -> std::collections::HashMap<String, crate::audit::domain::ScoringResultRef> {
        let Some(ref se_svc) = self.scored_evaluation_service else {
            return std::collections::HashMap::new();
        };
        let Ok(outputs) = se_svc.list_evaluations(execution_id).await else {
            return std::collections::HashMap::new();
        };
        outputs
            .into_iter()
            .map(|output| {
                let ref_map: std::collections::HashMap<
                    String,
                    crate::audit::domain::ScoreDimensionRef,
                > = output
                    .result
                    .dimensions
                    .into_iter()
                    .map(|(k, d)| {
                        (
                            k,
                            crate::audit::domain::ScoreDimensionRef {
                                score: d.score,
                                max: d.max,
                                label: d.label,
                                passed: d.passed,
                            },
                        )
                    })
                    .collect();
                (
                    output.node_id.to_string(),
                    crate::audit::domain::ScoringResultRef {
                        passed: output.result.passed,
                        backend: output.result.backend,
                        dimensions: ref_map,
                        duration_ms: output.result.duration_ms,
                    },
                )
            })
            .collect()
    }

    /// Build a module dependency graph string from the repo root.
    ///
    /// Uses CodeGraphBuilder to scan the workspace and CodeGraphFormatter
    /// to produce compact output. Returns None if CodeGraphService is not
    /// configured or if any step fails (non-fatal — the pipeline continues
    /// without module deps).
    pub(super) async fn build_module_deps(&self, repo_root: &str) -> Option<String> {
        let code_graph_service = self.code_graph_service.as_ref()?.clone();
        let root = std::path::PathBuf::from(repo_root);
        if !root.exists() {
            return None;
        }

        // 1. Use CodeGraphBuilder to scan the workspace
        let extensions = vec![
            "rs".to_string(),
            "ts".to_string(),
            "tsx".to_string(),
            "js".to_string(),
            "py".to_string(),
        ];
        let builder = crate::code_graph::application::builder::CodeGraphBuilder::new(
            code_graph_service.clone(),
            vec![root.clone()],
            extensions,
            false,
        );
        let build_out = builder.build().await.ok()?;

        // 2. Format as compact citations (FastContext <final_answer> pattern)
        let formatter = CodeGraphFormatterImpl::new();
        let formatted = CodeGraphFormatterTrait::format(
            &formatter,
            crate::code_graph::application::dto::FormatGraphInput {
                graph: build_out.graph,
                format: crate::code_graph::application::dto::OutputFormat::Compact,
                include_metadata: false,
            },
        )
        .await
        .ok()?;

        Some(formatted.output)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_record(
        &self,
        execution_id: Uuid,
        started_at: chrono::DateTime<chrono::Utc>,
        status: ExecutionStatus,
        planning_meta: Option<PlanningMetadata>,
        task_results: Vec<TaskResult>,
        context: ExecutionContext,
        events: Vec<ExecutionEventInfo>,
    ) -> ExecutionRecord {
        let now = chrono::Utc::now();
        let duration_ms = now
            .signed_duration_since(started_at)
            .num_milliseconds()
            .max(0) as u64;
        let completed_at = Some(now);
        ExecutionRecord {
            execution_id,
            planning: planning_meta.unwrap_or(PlanningMetadata {
                template_id: String::new(),
                confidence: 0.0,
                llm_calls: 0,
                total_tokens: 0,
                prompt_hash: String::new(),
                parameters: std::collections::HashMap::new(),
                generated_toml: None,
                node_order: vec![],
                model_version: None,
            }),
            task_results,
            events,
            context,
            started_at,
            completed_at,
            duration_ms,
            status,
        }
    }

    pub(super) fn make_pending_state(execution_id: Uuid) -> state_dto::SaveStateInput {
        let mut state =
            crate::state_persistence::domain::ExecutionState::new(execution_id, String::new());
        state.status = crate::state_persistence::domain::ExecutionStatus::Pending;
        state_dto::SaveStateInput { state }
    }

    pub(super) fn make_final_state(
        execution_id: Uuid,
        status: ExecutionStatus,
        exec_states: Option<
            &std::collections::HashMap<
                uuid::Uuid,
                crate::execution_engine::domain::NodeExecutionState,
            >,
        >,
    ) -> state_dto::SaveStateInput {
        use crate::state_persistence::domain::ExecutionStatus as SpStatus;
        let sp_status = match status {
            ExecutionStatus::Completed => SpStatus::Completed,
            ExecutionStatus::PartialFailure | ExecutionStatus::Failed => SpStatus::Failed,
            ExecutionStatus::Cancelled => SpStatus::Cancelled,
            // Not terminal — persisted as Pending so approve/resume continues it.
            ExecutionStatus::PendingApproval => SpStatus::Pending,
        };
        let mut state =
            crate::state_persistence::domain::ExecutionState::new(execution_id, String::new());
        state.status = sp_status;
        state.completed_at = Some(chrono::Utc::now());
        // GAP-M-15: exec_node_states is the single persisted node-state
        // representation — final states persist it too (pause states already
        // did), so every state file carries the canonical vocabulary.
        state.exec_node_states = exec_states.cloned();
        state_dto::SaveStateInput { state }
    }

    pub(super) fn planning_started_event(
        execution_id: Uuid,
        intent: String,
    ) -> event_app::PublishEventInput {
        event_app::PublishEventInput {
            event: crate::event_system::domain::ExecutionEvent::PlanningStarted {
                execution_id,
                intent,
                timestamp: chrono::Utc::now(),
            },
        }
    }

    pub(super) fn planning_completed_event(
        execution_id: Uuid,
        pr: &crate::planning::domain::result::PlanningResult,
    ) -> event_app::PublishEventInput {
        event_app::PublishEventInput {
            event: crate::event_system::domain::ExecutionEvent::PlanningCompleted {
                execution_id,
                template_id: pr.template_id.clone(),
                confidence: pr.confidence,
                parameters: std::collections::HashMap::new(),
                timestamp: chrono::Utc::now(),
            },
        }
    }

    /// Build a sealed TaskGraph directly from pre-resolved template steps.
    ///
    /// Each step becomes a TaskNode with no cross-node dependencies (sequential
    /// or single-step execution). The step's parameters are serialized as the
    /// node's intent string.
    pub(super) fn build_graph_from_steps(
        &self,
        steps: &[super::super::dto::TemplateStepDef],
    ) -> Result<crate::dag_engine::domain::TaskGraph, OrchestratorError> {
        let mut graph = crate::dag_engine::domain::TaskGraph::new();
        // Step order is significant (frozen contract, template-tools value.rs):
        // each step depends on its predecessor, so a template executes as a
        // sequential runbook — validate → backup → migrate → verify — instead
        // of a parallel batch where the migrate step could race ahead of the
        // backup step. Parallel DAGs are expressed via explicit dependencies
        // (engine [[nodes]] format); template steps are ordered by definition.
        let mut prev: Option<Uuid> = None;
        for step in steps {
            let node_id = Uuid::new_v4();
            let intent = serde_json::to_string(&step.parameters).unwrap_or_default();
            let deps = prev.map(|p| vec![p]).unwrap_or_default();
            let node = crate::dag_engine::domain::TaskNode::new(
                node_id,
                step.name.clone(),
                step.tool.clone(),
                deps,
                intent,
            )
            .with_requires_approval(step.requires_approval);
            graph
                .add_unchecked(node)
                .map_err(|e| OrchestratorError::Internal {
                    detail: format!("Failed to add graph node: {e}"),
                    source_module: "orchestrator".into(),
                })?;
            prev = Some(node_id);
        }
        graph.seal().map_err(|e| OrchestratorError::Internal {
            detail: format!("Failed to seal graph: {e}"),
            source_module: "orchestrator".into(),
        })?;
        Ok(graph)
    }

    /// Extract file paths from task result outputs.
    /// Uses simple heuristics to find file path patterns in node output text.
    pub(super) fn extract_file_paths(task_results: &[TaskResult]) -> Vec<String> {
        let mut paths: Vec<String> = Vec::new();
        let path_re = regex::Regex::new(
            r#"(?:^|\s)((?:\.[/\\])?[a-zA-Z0-9_\-./\\]+\.(?:rs|ts|js|py|go|rb|java|kt|swift|c|cpp|h|hpp|toml|json|yaml|yml|md|css|scss|html|svelte|vue))(?::\d+(?::\d+)?)?"#
        ).ok();
        for task in task_results {
            if let Some(ref output) = task.output
                && let Some(ref re) = path_re
            {
                for cap in re.captures_iter(output) {
                    let path = cap.get(1).map(|m| m.as_str().to_string());
                    if let Some(p) = path {
                        // Remove trailing line/column numbers
                        let clean = p.split(':').next().unwrap_or(&p).to_string();
                        if !paths.contains(&clean) {
                            paths.push(clean);
                        }
                    }
                }
            }
        }
        paths
    }

    /// GAP-M-13: deterministic capture of the resolved planning inputs for
    /// the audit envelope, gated on `capture_planning_prompt`. The planning
    /// service retains only the hash; the canonical inputs (template +
    /// resolved parameters) are serialized as the prompt-content evidence.
    pub(super) fn planning_prompt_content(&self, planning: &PlanningMetadata) -> Option<String> {
        if !self.config.capture_planning_prompt {
            return None;
        }
        serde_json::to_string(&serde_json::json!({
            "template_id": planning.template_id,
            "parameters": planning.parameters,
        }))
        .ok()
    }

    /// Detect git commit and branch from the working directory.
    pub(super) fn detect_git_info(repo_root: &str) -> (Option<String>, Option<String>) {
        let run_git = |args: &[&str]| -> Option<String> {
            std::process::Command::new("git")
                .args(args)
                .current_dir(repo_root)
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
        };
        let commit = run_git(&["rev-parse", "HEAD"]);
        let branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"]);
        (commit, branch)
    }

    pub(super) fn planning_meta(
        pr: &crate::planning::domain::result::PlanningResult,
        graph: Option<&crate::dag_engine::domain::TaskGraph>,
        model_version: &Option<String>,
    ) -> PlanningMetadata {
        let node_order = match graph {
            Some(g) => match g.topological_order() {
                Some(order) => order
                    .iter()
                    .map(|id| {
                        g.get_node(*id)
                            .map(|n| n.name.clone())
                            .unwrap_or_else(|| id.to_string())
                    })
                    .collect::<Vec<_>>(),
                None => vec![],
            },
            None => vec![],
        };

        PlanningMetadata {
            template_id: pr.template_id.clone(),
            confidence: pr.confidence,
            llm_calls: pr.llm_calls_used,
            total_tokens: pr.llm_tokens_used,
            prompt_hash: pr.planning_hash.0.clone(),
            parameters: pr.parameters.clone(),
            generated_toml: pr.generated_toml.clone(),
            node_order,
            model_version: model_version.clone(),
        }
    }
}
