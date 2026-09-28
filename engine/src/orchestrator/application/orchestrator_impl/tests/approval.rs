//! orchestrator_impl tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_run_from_template_pauses_for_approval_and_resumes() {
    // Wire a REAL execution service into the orchestrator so the
    // requires_approval gate is exercised end to end (steps → graph →
    // pause → approve → resume).
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;

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
    );

    let step_def = |name: &str, requires_approval: bool| {
        crate::orchestrator::application::dto::TemplateStepDef {
            name: name.into(),
            tool: "bash".into(),
            description: name.into(),
            parameters: serde_json::json!({}),
            requires_approval,
            require_identity: false,
            timeout_secs: None,
            evaluate_score: false,
        }
    };

    let input = RunFromTemplateInput {
        steps: vec![step_def("build", false), step_def("deploy", true)],
        repo_root: "/tmp/t".into(),
        execution_id: None,
        template_name: "approval-test".into(),
        repository: None,
        author: None,
        identity: None,
        enforcement_preset: None,
    };

    // 1. First run pauses at the approval boundary — NOT terminal.
    let out = orch.run_from_template(input).await.unwrap();
    assert_eq!(
        out.record.status,
        ExecutionStatus::PendingApproval,
        "record must report PendingApproval, got {:?}",
        out.record.status
    );
    assert!(
        !out.record.status.is_terminal(),
        "pending approval is resumable"
    );

    // 2. Approve the deploy step → execution resumes and completes.
    let approve = orch
        .approve_execution(ApproveExecutionInput {
            execution_id: out.execution_id,
            step_names: vec!["deploy".into()],
            approver_id: None,
            authority: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(approve.approved, vec!["deploy".to_string()]);
    assert!(approve.still_pending.is_empty());
    assert!(approve.resumed, "execution should resume after approval");
}

// ── Sequence-policy R2 (plan-time) integration tests ────────────────────
// ISSUE-SEQUENCE-POLICY-6 (#844): the orchestrator evaluates an ordered
// runbook BEFORE the DAG graph is sealed (module spec: Orchestrator —
// Graph Build insertion point). These run the canonical conference rule
// (remove conf-2026 then add conf-2026) through the REAL executor and the
// REAL sequence-policy service: promote pauses the later step at the
// approval gate, approve executes it, a declined step is never dispatched,
// and a deny rule refuses the runbook before any step executes.

/// AC#6 (promote leg): a remove-then-add runbook is paused by the promote
/// rule — the later step is built `requires_approval = true` — and only an
/// explicit approval executes it.
#[tokio::test]
async fn test_sequence_policy_promote_pauses_runbook_and_approve_executes_later_step() {
    use crate::execution_engine::domain::NodeStatus;
    let orch = real_orchestrator_with_conference_policy(RuleAction::Promote);
    let input = RunFromTemplateInput {
        steps: conference_runbook(),
        repo_root: "/tmp/t".into(),
        execution_id: None,
        template_name: "conf-registration".into(),
        repository: None,
        identity: None,
        author: None,
        enforcement_preset: None,
    };

    // 1. First run pauses at the promoted step — the runbook declares no
    // approval flags, so the pause is caused by the promote rule.
    let out = orch.run_from_template(input).await.unwrap();
    assert_eq!(
        out.record.status,
        ExecutionStatus::PendingApproval,
        "promoted runbook must pause, got {:?}",
        out.record.status
    );

    // 2. The remove step WAS dispatched; the promoted add step was NOT —
    // it sits at the approval gate the promote rule added.
    let st = orch.execution_state(out.execution_id).await.unwrap();
    let remove = st
        .node_states
        .values()
        .find(|s| s.node_name == "registration_remove")
        .expect("remove node state");
    assert!(
        remove.started_at.is_some(),
        "the earlier (remove) step must execute before the pause"
    );
    let add = st
        .node_states
        .values()
        .find(|s| s.node_name == "registration_add")
        .expect("add node state");
    assert_eq!(
        add.status,
        NodeStatus::AwaitingApproval,
        "promote rule must gate the later step"
    );
    assert!(
        add.started_at.is_none(),
        "add must not dispatch pre-approval"
    );
    assert!(st.paused);
    assert!(!st.is_complete);

    // 3. Approve the add step → execution resumes and the add tool is
    // dispatched (executes).
    let approve = orch
        .approve_execution(ApproveExecutionInput {
            execution_id: out.execution_id,
            step_names: vec!["registration_add".into()],
            approver_id: None,
            authority: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(approve.approved, vec!["registration_add".to_string()]);
    assert!(approve.resumed, "execution must resume after approval");

    let st2 = orch.execution_state(out.execution_id).await.unwrap();
    let add2 = st2
        .node_states
        .values()
        .find(|s| s.node_name == "registration_add")
        .expect("add node state after approval");
    assert_eq!(
        add2.status,
        NodeStatus::Failed,
        "approved add must be dispatched (tool attempted)"
    );
    assert!(add2.started_at.is_some(), "approved add step must execute");
}

/// AC#6 (reject leg): when the human does not approve, the promoted later
/// step is never dispatched — the runbook is cut short (skipped).
#[tokio::test]
async fn test_sequence_policy_declined_later_step_is_never_dispatched() {
    use crate::execution_engine::domain::NodeStatus;
    let orch = real_orchestrator_with_conference_policy(RuleAction::Promote);
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
        .unwrap();
    assert_eq!(out.record.status, ExecutionStatus::PendingApproval);

    // A decline (nothing approval-worthy is approved) leaves the run
    // paused — the promoted add step is never dispatched.
    let decline = orch
        .approve_execution(ApproveExecutionInput {
            execution_id: out.execution_id,
            step_names: vec!["no_such_step".into()],
            approver_id: None,
            authority: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert!(decline.approved.is_empty(), "declined: nothing approved");
    assert!(
        !decline.resumed,
        "run stays paused when steps remain pending"
    );

    let st = orch.execution_state(out.execution_id).await.unwrap();
    let add = st
        .node_states
        .values()
        .find(|s| s.node_name == "registration_add")
        .expect("add node state");
    assert_eq!(
        add.status,
        NodeStatus::AwaitingApproval,
        "declined step remains pending — skipped, never executed"
    );
    assert!(
        add.started_at.is_none(),
        "declined add tool must never be called"
    );
    assert!(st.paused, "run remains resumable pending a human decision");
}

/// AC#11: NO config file → the run executes UNCHANGED — the same
/// remove-then-add runbook that a rule file would pause/refuse runs to
/// completion with zero gating (fail-open-absent, status quo).
#[tokio::test]
async fn test_sequence_policy_absent_config_run_executes_unchanged() {
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
    use crate::sequence_policy::infrastructure::TomlSequencePolicyRepository;

    // Absent config: repository path points at a file that does not exist
    // → Ok(None) → no rules → no gating (never an error).
    let absent = std::env::temp_dir().join(format!(
        "rigorix-sp-absent-{}/sequence-policy.toml",
        Uuid::new_v4()
    ));

    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::new(EventBusServiceImpl::default()),
        ));
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(
        TomlSequencePolicyRepository::new(&absent),
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

    // The SAME conference runbook the rules would pause (promote) or
    // refuse (deny) — with no config file it must run UNCHANGED.
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
        .expect("absent config must not refuse the run");
    assert_ne!(
        out.record.status,
        ExecutionStatus::PendingApproval,
        "no rule file ⇒ nothing pauses the run"
    );

    let st = orch.execution_state(out.execution_id).await.expect("state");
    assert!(!st.paused, "no gating without a rule file");
    assert!(st.is_complete, "run completes end-to-end");

    // Both steps were dispatched exactly as without any policy service —
    // the remove-then-add pair ran unchanged.
    let states = st.node_states;
    let remove = states
        .values()
        .find(|s| s.node_name == "registration_remove")
        .expect("remove node state");
    let add = states
        .values()
        .find(|s| s.node_name == "registration_add")
        .expect("add node state");
    assert!(
        remove.started_at.is_some() && add.started_at.is_some(),
        "both runbook steps must dispatch unchanged (no pause, no denial)"
    );
}

/// AC 19/21: `promote` promotes the matched step (approval pause) and the
/// signed envelope records a redacted `requirement_findings[]` entry —
/// pointer NAMES only, never parameter values.
#[tokio::test]
async fn test_requirement_promote_pauses_and_envelope_records_finding() {
    use crate::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
    use crate::audit::application::factory::AuditEnvelopeFactory;
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;

    let captured = Arc::new(std::sync::Mutex::new(None));
    let bus: Arc<dyn event_app::EventBusService> = Arc::new(EventBusServiceImpl::default());
    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::clone(&bus),
        ));
    let mut cfg = payout_requirement(RequirementAction::Promote);
    cfg.requirements[0].require_identity = false;
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
        config: Some(cfg),
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

    let out = orch
        .run_from_template(payout_input(false, false, None))
        .await
        .expect("promote requirement pauses (not an error)");
    assert_eq!(out.record.status, ExecutionStatus::PendingApproval);

    let input = captured
        .lock()
        .unwrap()
        .take()
        .expect("audit service must receive the envelope input");
    let promoted = input
        .events
        .iter()
        .find(|e| e.event_type == "requirement_promoted")
        .expect("promotion must be recorded as an envelope event");
    let payload = promoted
        .payload
        .as_ref()
        .expect("promote event carries a payload");
    assert_eq!(payload["requirement_id"], "payout-guard");
    assert_eq!(payload["action"], "promote");
    assert_eq!(
        payload["unmet"],
        serde_json::json!(["/beneficiary", "/effect_key"])
    );
    let event_json = serde_json::to_string(payload).expect("serialize payload");
    assert!(
        !event_json.contains("acct-1") && !event_json.contains("eff-1"),
        "SpanPrivacy: parameter values must not leak: {event_json}"
    );

    let envelope = AuditEnvelopeFactoryImpl::new(Some("test-key".to_string()))
        .build_envelope(input)
        .await
        .expect("envelope build");
    assert_eq!(envelope.requirement_findings.len(), 1);
    let finding = &envelope.requirement_findings[0];
    assert_eq!(finding.requirement_id, "payout-guard");
    assert_eq!(finding.step, "payout");
    assert_eq!(finding.action, "promote");
    assert_eq!(
        finding.unmet,
        vec!["/beneficiary".to_string(), "/effect_key".to_string()]
    );
    assert!(!finding.summary.contains("acct-1"));
}

/// AC#8 (engine side): plan preview surfaces the R2 decision as a
/// structured finding BEFORE a run — a matched promote sequence reports
/// the rule + later step and builds the later step approval-gated.
#[tokio::test]
async fn test_plan_from_template_reports_promote_finding_before_run() {
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
        config: Some(conference_policy(RuleAction::Promote)),
    })));
    let orch = OrchestratorServiceImpl::default_test().with_sequence_policy(policy);

    let out = orch
        .plan_from_template(PlanFromTemplateInput {
            steps: conference_runbook(),
            repo_root: "/tmp/t".into(),
            template_name: "conf-registration".into(),
            identity: None,
            author: None,
        })
        .await
        .unwrap();

    assert_eq!(
        out.sequence_findings.len(),
        1,
        "plan preview must carry the promote finding"
    );
    let f = &out.sequence_findings[0];
    assert_eq!(f.rule_id, "registration-remove-then-reassign");
    assert_eq!(f.later_step, "registration_add");
    assert_eq!(f.action, "promote");

    // The promoted step is built approval-gated in the preview graph —
    // the same graph the run would execute.
    let graph_json = serde_json::to_value(&out.graph).unwrap_or_default();
    let text = serde_json::to_string(&graph_json).unwrap_or_default();
    assert!(
        text.contains("registration_add") && text.contains("true"),
        "preview graph must gate the later step, got: {text}"
    );
}
