//! orchestrator_impl tests (split by concern, #917).

use super::*;

/// Identity AC#6: `RunInput.identity` flows into the envelope identity
/// block (redacted) through the real run pipeline.
#[tokio::test]
async fn test_run_input_identity_flows_into_envelope_identity_block() {
    let captured = Arc::new(std::sync::Mutex::new(None));
    let orch = OrchestratorServiceImpl::default_test().with_audit_service(Arc::new(
        CapturingAuditService {
            captured: captured.clone(),
        },
    ));

    let claim = crate::identity::domain::IdentityClaim {
        subject: "user@org".to_string(),
        issuer: "https://idp.example.com".to_string(),
        authority: Some("admin".to_string()),
        source: crate::identity::domain::IdentitySource::IdpToken,
        auth_method: Some("device_code".to_string()),
        issued_at: chrono::Utc::now(),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(15)),
        token_ref: Some("keychain://default/rigorix/idp-token".to_string()),
    };

    orch.run(RunInput {
        intent: "test identity attestation".to_string(),
        config: serde_json::json!({}),
        repo_root: "/tmp/t".to_string(),
        author: Some("legacy-author".to_string()),
        identity: Some(claim),
        repository: None,
        enforcement_preset: None,
    })
    .await
    .expect("run should succeed with mock services");

    let envelope_input = captured
        .lock()
        .unwrap()
        .take()
        .expect("audit service must receive the built envelope input");

    let identity_ref = envelope_input
        .identity
        .expect("envelope identity block must be populated from RunInput.identity");
    assert_eq!(identity_ref.subject, "user@org");
    assert_eq!(identity_ref.issuer, "https://idp.example.com");
    assert_eq!(
        identity_ref.source,
        crate::identity::domain::IdentitySource::IdpToken
    );
    assert_eq!(identity_ref.authority, Some("admin".to_string()));

    // Redacted: no token_ref and no token locator survive into the block.
    let json = serde_json::to_string(&identity_ref).expect("serialize identity ref");
    assert!(!json.contains("token_ref"));
    assert!(!json.contains("keychain"));
}

#[tokio::test]
async fn test_run_returns_execution_id() {
    let orch = OrchestratorServiceImpl::default_test();
    let out = orch
        .run(RunInput {
            intent: "test".into(),
            config: serde_json::json!({}),
            repo_root: "/tmp/t".into(),
            author: None,
            identity: None,
            repository: None,
            enforcement_preset: None,
        })
        .await
        .unwrap();
    assert_ne!(out.execution_id, Uuid::nil());
    assert_eq!(out.record.execution_id, out.execution_id);
    assert_eq!(out.record.status, ExecutionStatus::Completed);
}

#[tokio::test]
async fn test_run_planning_metadata() {
    let orch = OrchestratorServiceImpl::default_test();
    let out = orch
        .run(RunInput {
            intent: "test".into(),
            config: serde_json::json!({}),
            repo_root: "/tmp/t".into(),
            author: None,
            identity: None,
            repository: None,
            enforcement_preset: None,
        })
        .await
        .unwrap();
    assert_eq!(out.record.planning.template_id, "mock-template");
    assert!((out.record.planning.confidence - 0.95).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_run_timestamps() {
    let orch = OrchestratorServiceImpl::default_test();
    let out = orch
        .run(RunInput {
            intent: "test".into(),
            config: serde_json::json!({}),
            repo_root: "/tmp/t".into(),
            author: None,
            identity: None,
            repository: None,
            enforcement_preset: None,
        })
        .await
        .unwrap();
    assert!(out.record.completed_at.is_some());
}

#[tokio::test]
async fn test_run_context() {
    let orch = OrchestratorServiceImpl::default_test();
    let out = orch
        .run(RunInput {
            intent: "test".into(),
            config: serde_json::json!({}),
            repo_root: "/tmp/repo".into(),
            author: None,
            identity: None,
            repository: None,
            enforcement_preset: None,
        })
        .await
        .unwrap();
    assert_eq!(out.record.context.repo_root, "/tmp/repo");
}

#[tokio::test]
async fn test_status_after_run() {
    let orch = OrchestratorServiceImpl::default_test();
    let _ = orch
        .run(RunInput {
            intent: "test".into(),
            config: serde_json::json!({}),
            repo_root: "/tmp/t".into(),
            author: None,
            identity: None,
            repository: None,
            enforcement_preset: None,
        })
        .await
        .unwrap();
    assert_eq!(
        orch.status().await.unwrap().status,
        ExecutionStatus::Completed
    );
}

/// AC#7: a `deny` rule refuses the runbook before any step executes — the
/// denied later step's tool is never called (no node is dispatched).
#[tokio::test]
async fn test_sequence_policy_deny_refuses_runbook_before_any_step() {
    let orch = real_orchestrator_with_conference_policy(RuleAction::Deny);
    let eid = Uuid::new_v4();
    let err = orch
        .run_from_template(RunFromTemplateInput {
            steps: conference_runbook(),
            repo_root: "/tmp/t".into(),
            execution_id: Some(eid),
            template_name: "conf-registration".into(),
            repository: None,
            identity: None,
            author: None,
            enforcement_preset: None,
        })
        .await
        .unwrap_err();
    match &err {
        OrchestratorError::SequencePolicyDenied {
            later_step,
            rule_id,
        } => {
            assert_eq!(later_step, "registration_add");
            assert_eq!(rule_id, "registration-remove-then-reassign");
            assert!(
                err.to_string().contains("Sequence policy denied"),
                "structured deny error: {err}"
            );
        }
        other => panic!("expected SequencePolicyDenied, got {other}"),
    }

    // Fail-closed: the refusal happened before the graph was built, so no
    // engine session exists and nothing could have been dispatched.
    let state = orch.execution_state(eid).await;
    assert!(
        state.is_err(),
        "no engine session ⇒ no step of the denied runbook executed"
    );
}

/// AC#10: a CORRUPT rule config refuses the plan end-to-end — real TOML
/// repository over a corrupt operator file → service load error → R2
/// gate propagates the evaluation failure and no step executes.
#[tokio::test]
async fn test_sequence_policy_corrupt_config_refuses_run_no_steps_execute() {
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
    use crate::sequence_policy::infrastructure::TomlSequencePolicyRepository;

    // Corrupt operator file (unterminated rule table) on disk — the
    // repository must surface Err, and the R2 gate must refuse the plan.
    let path = std::env::temp_dir().join(format!("rigorix-sp-corrupt-{}.toml", Uuid::new_v4()));
    tokio::fs::write(&path, "fail_closed = true\n[[rules]]\nid = 'unterminated")
        .await
        .expect("write corrupt config");

    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::new(EventBusServiceImpl::default()),
        ));
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(
        TomlSequencePolicyRepository::new(&path),
    )));
    let orch = OrchestratorServiceImpl::new(
        OrchestratorConfig::default(),
        Arc::new(super::super::super::orchestrator_mocks::MockPlanningService::new()),
        executor,
        Arc::new(super::super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::super::orchestrator_mocks::MockCancellationService),
        Arc::new(super::super::super::orchestrator_mocks::MockEventBusService::new()),
        None,
        Arc::new(super::super::super::orchestrator_mocks::MockBudgetService),
        None,
    )
    .with_sequence_policy(policy);
    let eid = Uuid::new_v4();

    let err = orch
        .run_from_template(RunFromTemplateInput {
            steps: conference_runbook(),
            repo_root: "/tmp/t".into(),
            execution_id: Some(eid),
            template_name: "conf-registration".into(),
            repository: None,
            identity: None,
            author: None,
            enforcement_preset: None,
        })
        .await
        .unwrap_err();
    match &err {
        OrchestratorError::SequencePolicyEvaluationFailed { detail } => {
            assert!(
                detail.contains("Rule config invalid"),
                "evaluation failure must surface the config error: {detail}"
            );
        }
        other => panic!("expected SequencePolicyEvaluationFailed, got {other}"),
    }

    // Fail closed before graph build: no engine session ⇒ no step of the
    // corrupt-config runbook ever executed.
    let state = orch.execution_state(eid).await;
    assert!(
        state.is_err(),
        "no engine session ⇒ no steps executed under a corrupt rule file"
    );
    let _ = tokio::fs::remove_file(&path).await;
}

/// AC#12: the matched rule + promotion are recorded as first-class
/// envelope events — the run's drained events carry `sequence_rule_matched`
/// (rule id, action, later step, matched indices) and the envelope's
/// `sequence_policy_findings[]` derives redacted decision summaries
/// (SpanPrivacy: parameter VALUES never captured by default).
#[tokio::test]
async fn test_sequence_policy_promotion_recorded_in_envelope_events_redacted() {
    use crate::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
    use crate::audit::application::factory::AuditEnvelopeFactory;
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;

    // Real engine + policy service + REAL shared event bus so the
    // promotion evidence lands in the run's drained events.
    let captured = Arc::new(std::sync::Mutex::new(None));
    let bus: Arc<dyn event_app::EventBusService> = Arc::new(EventBusServiceImpl::default());
    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::clone(&bus),
        ));
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
        config: Some(conference_policy(RuleAction::Promote)),
    })));
    let orch = OrchestratorServiceImpl::new(
        OrchestratorConfig::default(),
        Arc::new(super::super::super::orchestrator_mocks::MockPlanningService::new()),
        executor,
        Arc::new(super::super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::super::orchestrator_mocks::MockCancellationService),
        Arc::clone(&bus),
        Some(Arc::new(CapturingAuditService {
            captured: captured.clone(),
        })),
        Arc::new(super::super::super::orchestrator_mocks::MockBudgetService),
        None,
    )
    .with_sequence_policy(policy);

    // The promote rule pauses the runbook at the later step; the R2 gate
    // published the SequenceRuleMatched evidence BEFORE any step ran.
    let out = orch
        .run_from_template(RunFromTemplateInput {
            steps: conference_runbook(),
            repo_root: "/tmp/t".into(),
            execution_id: None,
            template_name: "conf-registration".into(),
            repository: None,
            identity: None,
            author: None,
            enforcement_preset: None,
        })
        .await
        .expect("promoted runbook pauses (not an error)");
    assert_eq!(out.record.status, ExecutionStatus::PendingApproval);

    // 1. The run's events carry the structured match — rule, action,
    // later step, indices — and NO parameter values anywhere.
    let input = captured
        .lock()
        .unwrap()
        .take()
        .expect("audit service must receive the envelope input");
    let matched = input
        .events
        .iter()
        .find(|e| e.event_type == "sequence_rule_matched")
        .expect("promotion must be recorded as an envelope event");
    let payload = matched
        .payload
        .as_ref()
        .expect("match event carries a structured payload");
    assert_eq!(
        payload["rule_id"], "registration-remove-then-reassign",
        "envelope event names the matched rule"
    );
    assert_eq!(payload["later_step"], "registration_add");
    assert_eq!(payload["action"], "promote");
    assert_eq!(payload["matched_indices"], serde_json::json!([0, 1]));
    let event_json = serde_json::to_string(payload).expect("serialize event payload");
    assert!(
        !event_json.contains("conf-2026"),
        "SpanPrivacy: parameter values must not leak into the event payload: {event_json}"
    );

    // 2. The envelope derives the redacted sequence_policy_findings[].
    let envelope = AuditEnvelopeFactoryImpl::new(Some("test-key".to_string()))
        .build_envelope(input)
        .await
        .expect("envelope build");
    assert_eq!(envelope.sequence_policy_findings.len(), 1);
    let finding = &envelope.sequence_policy_findings[0];
    assert_eq!(finding.rule_id, "registration-remove-then-reassign");
    assert_eq!(finding.action, "promote");
    assert_eq!(finding.later_step, "registration_add");
    assert_eq!(finding.matched_indices, vec![0, 1]);
    assert!(
        finding.summary.contains("registration_add")
            && finding.summary.contains("parameter values redacted"),
        "decision summary is redacted by default: {}",
        finding.summary
    );
    assert!(
        !finding.summary.contains("conf-2026"),
        "redacted summary must not carry parameter values"
    );
}

/// AC 18: an operator `require_identity` refuses an unauthenticated plan
/// even though the matched step does not declare `require_identity`, and
/// is never promotable.
#[tokio::test]
async fn test_requirement_identity_refuses_unauthenticated_composed_plan() {
    // `promote` action still cannot substitute for an identity.
    let orch =
        real_orchestrator_with_requirement(payout_requirement(RequirementAction::Promote));
    let err = orch
        .run_from_template(payout_input(false, true, None))
        .await
        .expect_err("identity requirement must refuse");
    assert!(
        matches!(err, OrchestratorError::IdentityRequired { ref step, ref status } if step == "payout" && status == "unauthenticated"),
        "unexpected: {err:?}"
    );
}

/// AC 19/20: an operator `require_params` deny refuses a raw composed
/// `run_command` that omits the canonical parameters; when the parameters
/// are present the same plan is allowed.
#[tokio::test]
async fn test_requirement_params_deny_refuses_composed_plan_and_allows_when_present() {
    let mut cfg = payout_requirement(RequirementAction::Deny);
    cfg.requirements[0].require_identity = false; // isolate the params
    let orch = real_orchestrator_with_requirement(cfg);

    let err = orch
        .run_from_template(payout_input(false, false, None))
        .await
        .expect_err("missing required params must refuse");
    assert!(
        matches!(err, OrchestratorError::RequirementUnmet { ref requirement_id, ref step, ref unmet }
            if requirement_id == "payout-guard"
                && step == "payout"
                && unmet == &vec!["/beneficiary".to_string(), "/effect_key".to_string()]),
        "unexpected: {err:?}"
    );

    // Same command WITH the canonical parameters passes the operator gate.
    let out = orch
        .run_from_template(payout_input(false, true, None))
        .await
        .expect("satisfied requirement must not refuse");
    assert!(out.record.status != ExecutionStatus::PendingApproval);
}

/// AC 20: the plan preview (`rigorix_validate_plan` engine side) surfaces
/// requirement findings for an agent-composed plan WITHOUT executing it.
#[tokio::test]
async fn test_plan_from_template_reports_requirement_promote_finding() {
    let mut cfg = payout_requirement(RequirementAction::Promote);
    cfg.requirements[0].require_identity = false;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
        config: Some(cfg),
    })));
    let orch = OrchestratorServiceImpl::default_test().with_sequence_policy(policy);
    let out = orch
        .plan_from_template(PlanFromTemplateInput {
            steps: payout_runbook(false),
            repo_root: "/tmp/t".into(),
            template_name: "payout".into(),
            identity: None,
            author: None,
        })
        .await
        .expect("preview succeeds with a promote finding");
    assert_eq!(out.requirement_findings.len(), 1);
    let f = &out.requirement_findings[0];
    assert_eq!(f.requirement_id, "payout-guard");
    assert_eq!(f.step, "payout");
    assert_eq!(f.action, "promote");
    assert_eq!(
        f.unmet,
        vec!["/beneficiary".to_string(), "/effect_key".to_string()]
    );
}

/// AC#8 (engine side): plan preview of a denied sequence refuses the
/// plan before the run — no preview graph is produced for a forbidden
/// composition.
#[tokio::test]
async fn test_plan_from_template_refuses_denied_sequence() {
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
        config: Some(conference_policy(RuleAction::Deny)),
    })));
    let orch = OrchestratorServiceImpl::default_test().with_sequence_policy(policy);

    let err = orch
        .plan_from_template(PlanFromTemplateInput {
            steps: conference_runbook(),
            repo_root: "/tmp/t".into(),
            template_name: "conf-registration".into(),
            identity: None,
            author: None,
        })
        .await
        .unwrap_err();
    match &err {
        OrchestratorError::SequencePolicyDenied {
            later_step,
            rule_id,
        } => {
            assert_eq!(later_step, "registration_add");
            assert_eq!(rule_id, "registration-remove-then-reassign");
        }
        other => panic!("expected SequencePolicyDenied, got {other}"),
    }
}

/// GAP-M-13: planning_prompt_content is populated only when
/// capture_planning_prompt is enabled (config-gated, deterministic).
#[test]
fn test_planning_prompt_content_gated() {
    let mut planning = crate::orchestrator::domain::record::PlanningMetadata {
        template_id: "tpl".to_string(),
        ..Default::default()
    };
    planning
        .parameters
        .insert("key".to_string(), "value".to_string());

    // Disabled (default) -> None.
    let orch_off = OrchestratorServiceImpl::default_test();
    assert!(orch_off.planning_prompt_content(&planning).is_none());

    // Enabled -> deterministic JSON of template + resolved parameters.
    let orch_on = OrchestratorServiceImpl::new(
        OrchestratorConfig {
            capture_planning_prompt: true,
            ..Default::default()
        },
        Arc::new(super::super::super::orchestrator_mocks::MockPlanningService::new()),
        Arc::new(super::super::super::orchestrator_mocks::MockExecutionService),
        Arc::new(super::super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::super::orchestrator_mocks::MockCancellationService),
        Arc::new(super::super::super::orchestrator_mocks::MockEventBusService::new()),
        None,
        Arc::new(super::super::super::orchestrator_mocks::MockBudgetService),
        None,
    );
    let content = orch_on
        .planning_prompt_content(&planning)
        .expect("capture must be populated when enabled");
    assert!(content.contains("tpl"), "template_id must be captured");
    assert!(content.contains("value"), "parameters must be captured");
    // Deterministic: same input -> same output.
    assert_eq!(content, orch_on.planning_prompt_content(&planning).unwrap());
}

/// R7 SCENE 9 (issue #871): two separate runs, minutes apart — run 1
/// removes a registrant (envelope persisted to the signed local trail),
/// run 2 adds the requester. Each run passes its own within-run gate; the
/// R7 history rule refuses run 2 AT PLAN TIME because the SAME principal
/// removed within the window. Uses the REAL EnvelopeHistoryAdapter over a
/// real envelope repository seeded with run 1's envelope.
#[tokio::test]
async fn test_r7_cross_run_same_principal_remove_then_add_refused_at_plan_time() {
    use crate::audit::application::dto::BuildEnvelopeInput;
    use crate::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
    use crate::audit::application::factory::AuditEnvelopeFactory;
    use crate::audit::domain::envelope::{EventStatus, ExecutionEventRef};
    use crate::audit::infrastructure::local_audit_repository::LocalAuditEnvelopeRepository;
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
    use crate::sequence_policy::domain::{HistoryPredicate, RuleAction};
    use crate::sequence_policy::infrastructure::EnvelopeHistoryAdapter;
    use serde_json::json;

    let dir = tempfile::tempdir().expect("tempdir");

    // ── Run 1's signed envelope: jeff@corp removed a registrant. ──
    let repo: std::sync::Arc<
        dyn crate::audit::infrastructure::repository::AuditEnvelopeRepository,
    > = std::sync::Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));
    let factory = AuditEnvelopeFactoryImpl::new(Some("test-key".to_string()));
    let run1 = factory
        .build_envelope(BuildEnvelopeInput {
            execution_id: uuid::Uuid::new_v4(),
            template_id: "attendance-remove".to_string(),
            planning_prompt: "run 1".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "node_completed".to_string(),
                summary: "remove completed".to_string(),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: EventStatus::Success,
                payload: Some(json!({ "step_name": "registration_remove" })),
            }],
            source: Some("rigorix_demo".to_string()),
            total_tokens: 0,
            duration_ms: 10,
            git_commit: None,
            git_branch: None,
            model_version: None,
            planning_prompt_content: None,
            file_paths: vec![],
            metadata: None,
            scoring_results: std::collections::HashMap::new(),
            sign: true,
            repository: None,
            author: Some("jeff@corp".to_string()),
            identity: None,
            effect_key: None,
            producer_id: None,
            sequence: None,
            prev_hash: None,
            history_policy: None,
        })
        .await
        .expect("run-1 envelope");
    repo.save(&run1).await.expect("run-1 envelope saved");

    // ── R7 rule: adding a seat is DENIED when the same principal removed
    // one within 15 minutes (single current-run predicate + history). ──
    let policy_cfg = crate::sequence_policy::domain::SequencePolicyConfig {
        fail_closed: true,
        requirements: Vec::new(),
        rules: vec![crate::sequence_policy::domain::SequenceRule {
            id: "no-cross-run-remove-reassign".to_string(),
            name: "No cross-run remove-then-reassign".to_string(),
            description: "d".to_string(),
            steps: vec![crate::sequence_policy::domain::StepPredicate {
                tool: "registration_add".to_string(),
                params: vec![crate::sequence_policy::domain::ParamPredicate {
                    pointer: "/event_id".to_string(),
                    kind: crate::sequence_policy::domain::ParamMatchKind::Exact,
                    value: Some("conf-2026".to_string()),
                    step: None,
                }],
            }],
            window: None,
            action: RuleAction::Deny,
            history: Some(HistoryPredicate {
                prior_node: "registration_remove".to_string(),
                same_principal: true,
                window_secs: 900,
                effect_key: false,
            }),
        }],
    };
    let policy = std::sync::Arc::new(
        SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
            config: Some(policy_cfg),
        }))
        .with_history(std::sync::Arc::new(EnvelopeHistoryAdapter::new(repo)))
        // ADR-016: this test exercises R7 matching; opt into best-effort
        // local (unanchored) history so the deny match is returned rather
        // than refused by the default fail-closed regime.
        .with_unanchored_history_allowed(true),
    );

    // ── Run 2: jeff's agent tries to add him to the full event. ──
    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::new(EventBusServiceImpl::default()),
        ));
    let orch = OrchestratorServiceImpl::new(
        OrchestratorConfig::default(),
        Arc::new(super::super::super::orchestrator_mocks::MockPlanningService::new()),
        executor,
        Arc::new(super::super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::super::orchestrator_mocks::MockCancellationService),
        Arc::new(super::super::super::orchestrator_mocks::MockEventBusService::new()),
        None,
        Arc::new(super::super::super::orchestrator_mocks::MockBudgetService),
        None,
    )
    .with_sequence_policy(policy);

    let step = crate::orchestrator::dto::TemplateStepDef {
        name: "registration_add".to_string(),
        tool: "registration_add".to_string(),
        description: "add jeff".to_string(),
        parameters: json!({ "event_id": "conf-2026" }),
        require_identity: false,
        requires_approval: false,
        timeout_secs: None,
        evaluate_score: false,
    };
    let err = orch
        .run_from_template(RunFromTemplateInput {
            steps: vec![step],
            repo_root: dir.path().to_string_lossy().into_owned(),
            execution_id: Some(uuid::Uuid::new_v4()),
            template_name: "attendance-add".into(),
            repository: None,
            identity: None,
            author: Some("jeff@corp".to_string()),
            enforcement_preset: None,
        })
        .await
        .expect_err("run 2 must be refused at plan time (cross-run R7)");

    match err {
        OrchestratorError::SequencePolicyDenied {
            later_step,
            rule_id,
        } => {
            assert_eq!(later_step, "registration_add");
            assert_eq!(rule_id, "no-cross-run-remove-reassign");
        }
        other => panic!("expected SequencePolicyDenied from the cross-run rule, got {other:?}"),
    }
}
