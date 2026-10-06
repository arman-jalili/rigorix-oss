//! Concrete implementation of the AuditFormatter domain service.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#auditformatter
//! Implements: AuditFormatter — formats audit data for MCP consumption
//!
//! Produces human-readable markdown text or structured JSON from audit data.

use crate::audit_tools::domain::entity::AuditFormatter;
use crate::audit_tools::domain::value::{AuditEnvelope, AuditSummary};

/// Formats audit data for MCP consumption.
///
/// All formatting methods are pure functions with no side effects.
pub struct AuditFormatterImpl;

impl AuditFormatterImpl {
    /// Create a new AuditFormatterImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AuditFormatterImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditFormatter for AuditFormatterImpl {
    fn format_audit_text(&self, envelope: &AuditEnvelope) -> String {
        let mut out = String::new();
        out.push_str(&format!("## Audit: `{}`\n\n", envelope.execution_id()));
        out.push_str(&format!("- **Status:** {:?}\n", envelope.status()));
        if let Some(tn) = envelope.template_name() {
            out.push_str(&format!("- **Template:** {}\n", tn));
        }
        out.push_str(&format!("- **Started:** {}\n", envelope.started_at()));
        out.push_str(&format!("- **Completed:** {}\n", envelope.completed_at()));
        out.push_str(&format!("- **Duration:** {} ms\n", envelope.duration_ms()));
        if let Some(tokens) = envelope.tokens_used() {
            out.push_str(&format!("- **Tokens Used:** {}\n", tokens));
        }
        out.push_str(&format!("- **HMAC:** `{}`\n", envelope.hmac()));

        // Steps
        if !envelope.steps().is_empty() {
            out.push_str("\n### Steps\n\n");
            for (i, step) in envelope.steps().iter().enumerate() {
                let status_icon = if step.is_success() { "✅" } else { "❌" };
                out.push_str(&format!(
                    "{} **{}.** {} — {} ms\n",
                    status_icon,
                    i + 1,
                    step.step_name(),
                    step.duration_ms()
                ));
                if let Some(err) = step.error() {
                    out.push_str(&format!("   Error: {}\n", err));
                }
            }
        }

        // Events
        if !envelope.events().is_empty() {
            out.push_str("\n### Events\n\n");
            for event in envelope.events() {
                out.push_str(&format!(
                    "- `{}` — {} ({})\n",
                    event.event_type(),
                    event.summary(),
                    event.occurred_at()
                ));
            }
        }

        // ADR-017 precondition findings
        if !envelope.precondition_findings().is_empty() {
            out.push_str("\n### Precondition Findings\n\n");
            for f in envelope.precondition_findings() {
                out.push_str(&format!(
                    "- **{}** on step `{}` — {}",
                    f.precondition_id(),
                    f.step(),
                    f.outcome()
                ));
                if let Some(code) = f.exit_code() {
                    out.push_str(&format!(" (exit {}).", code));
                } else {
                    out.push('.');
                }
                if !f.summary().is_empty() {
                    out.push_str(&format!(" {}", f.summary()));
                }
                out.push('\n');
            }
        }

        // ADR-013 sequence-policy findings
        if !envelope.sequence_policy_findings().is_empty() {
            out.push_str("\n### Sequence Policy Findings\n\n");
            for f in envelope.sequence_policy_findings() {
                out.push_str(&format!(
                    "- **{}** (`{}`) — step `{}`\n",
                    f.rule_id(),
                    f.action(),
                    f.later_step()
                ));
            }
        }

        // ADR-015 requirement findings
        if !envelope.requirement_findings().is_empty() {
            out.push_str("\n### Requirement Findings\n\n");
            for f in envelope.requirement_findings() {
                out.push_str(&format!(
                    "- **{}** (`{}`) — step `{}`; unmet: {}\n",
                    f.requirement_id(),
                    f.action(),
                    f.step(),
                    f.unmet().join(", ")
                ));
            }
        }

        out
    }

    fn format_audit_json(&self, envelope: &AuditEnvelope) -> serde_json::Value {
        serde_json::json!({
            "execution_id": envelope.execution_id().to_string(),
            "status": format!("{:?}", envelope.status()),
            "template_name": envelope.template_name(),
            "started_at": envelope.started_at().to_rfc3339(),
            "completed_at": envelope.completed_at().to_rfc3339(),
            "duration_ms": envelope.duration_ms(),
            "tokens_used": envelope.tokens_used(),
            "steps": envelope.steps().iter().map(|s| {
                serde_json::json!({
                    "step_name": s.step_name(),
                    "success": s.is_success(),
                    "error": s.error(),
                    "duration_ms": s.duration_ms(),
                })
            }).collect::<Vec<_>>(),
            "hmac": envelope.hmac(),
            "events": envelope.events().iter().map(|e| {
                serde_json::json!({
                    "event_type": e.event_type(),
                    "summary": e.summary(),
                    "occurred_at": e.occurred_at().to_rfc3339(),
                    "status": format!("{:?}", e.status()),
                })
            }).collect::<Vec<_>>(),
            "precondition_findings": envelope.precondition_findings().iter().map(|f| {
                serde_json::json!({
                    "precondition_id": f.precondition_id(),
                    "step": f.step(),
                    "outcome": f.outcome(),
                    "exit_code": f.exit_code(),
                    "inputs_hash": f.inputs_hash(),
                    "checked_at": f.checked_at().to_rfc3339(),
                    "summary": f.summary(),
                })
            }).collect::<Vec<_>>(),
            "sequence_policy_findings": envelope.sequence_policy_findings().iter().map(|f| {
                serde_json::json!({
                    "rule_id": f.rule_id(),
                    "action": f.action(),
                    "later_step": f.later_step(),
                    "matched_indices": f.matched_indices(),
                    "summary": f.summary(),
                })
            }).collect::<Vec<_>>(),
            "requirement_findings": envelope.requirement_findings().iter().map(|f| {
                serde_json::json!({
                    "requirement_id": f.requirement_id(),
                    "step": f.step(),
                    "action": f.action(),
                    "unmet": f.unmet(),
                    "summary": f.summary(),
                })
            }).collect::<Vec<_>>(),
        })
    }

    fn format_list_text(&self, audits: &[AuditEnvelope]) -> String {
        if audits.is_empty() {
            return "No audit records found.\n".to_string();
        }
        let mut out = String::new();
        out.push_str(&format!("## Audit Records ({} total)\n\n", audits.len()));
        for (i, a) in audits.iter().enumerate() {
            let status_icon = match a.status() {
                crate::execution_tools::domain::value::ExecutionStatus::Completed => "✅",
                crate::execution_tools::domain::value::ExecutionStatus::Failed => "❌",
                crate::execution_tools::domain::value::ExecutionStatus::PartialFailed => "⚠️",
                crate::execution_tools::domain::value::ExecutionStatus::Cancelled => "🚫",
                crate::execution_tools::domain::value::ExecutionStatus::EnforcementBlocked => "🔒",
                crate::execution_tools::domain::value::ExecutionStatus::PendingApproval => "⏸️",
            };
            out.push_str(&format!(
                "{}. {} `{}` — {} ms",
                i + 1,
                status_icon,
                a.execution_id(),
                a.duration_ms()
            ));
            if let Some(tn) = a.template_name() {
                out.push_str(&format!(" (template: {})", tn));
            }
            out.push('\n');
        }
        out
    }

    fn format_list_json(&self, audits: &[AuditEnvelope]) -> serde_json::Value {
        serde_json::json!({
            "total_count": audits.len(),
            "audits": audits.iter().map(|a| {
                serde_json::json!({
                    "execution_id": a.execution_id().to_string(),
                    "status": format!("{:?}", a.status()),
                    "template_name": a.template_name(),
                    "started_at": a.started_at().to_rfc3339(),
                    "duration_ms": a.duration_ms(),
                    "tokens_used": a.tokens_used(),
                })
            }).collect::<Vec<_>>(),
        })
    }

    fn format_summary_text(&self, summary: &AuditSummary) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "## Audit Summary ({} to {})\n\n",
            summary.since().format("%Y-%m-%d %H:%M UTC"),
            summary.until().format("%Y-%m-%d %H:%M UTC"),
        ));
        out.push_str(&format!(
            "- **Total Executions:** {}\n",
            summary.total_executions()
        ));
        out.push_str(&format!(
            "- **Successful:** {} ({:.1}%)\n",
            summary.success_count(),
            summary.success_rate() * 100.0
        ));
        out.push_str(&format!("- **Failed:** {}\n", summary.failure_count()));
        out.push_str(&format!(
            "- **Total Duration:** {} ms\n",
            summary.total_duration_ms()
        ));
        if let Some(tokens) = summary.total_tokens() {
            out.push_str(&format!("- **Total Tokens:** {}\n", tokens));
        }

        // Top failures
        if !summary.top_failures().is_empty() {
            out.push_str("\n### Top Failures\n\n");
            for (i, f) in summary.top_failures().iter().enumerate() {
                out.push_str(&format!(
                    "{}. **{}** ({} occurrences)",
                    i + 1,
                    f.description(),
                    f.count()
                ));
                if let Some(tn) = f.template_name() {
                    out.push_str(&format!(" — template: {}", tn));
                }
                out.push('\n');
            }
        }

        // Top templates
        if !summary.top_templates().is_empty() {
            out.push_str("\n### Top Templates\n\n");
            for (i, t) in summary.top_templates().iter().enumerate() {
                out.push_str(&format!(
                    "{}. **{}** — {} runs, avg {} ms\n",
                    i + 1,
                    t.name(),
                    t.count(),
                    t.avg_duration_ms()
                ));
            }
        }

        out
    }

    fn format_summary_json(&self, summary: &AuditSummary) -> serde_json::Value {
        serde_json::json!({
            "since": summary.since().to_rfc3339(),
            "until": summary.until().to_rfc3339(),
            "total_executions": summary.total_executions(),
            "success_count": summary.success_count(),
            "failure_count": summary.failure_count(),
            "success_rate": summary.success_rate(),
            "total_duration_ms": summary.total_duration_ms(),
            "total_tokens": summary.total_tokens(),
            "top_failures": summary.top_failures().iter().map(|f| {
                serde_json::json!({
                    "description": f.description(),
                    "count": f.count(),
                    "template_name": f.template_name(),
                })
            }).collect::<Vec<_>>(),
            "top_templates": summary.top_templates().iter().map(|t| {
                serde_json::json!({
                    "name": t.name(),
                    "count": t.count(),
                    "avg_duration_ms": t.avg_duration_ms(),
                })
            }).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use crate::audit_tools::domain::entity::AuditFormatter;
    use crate::audit_tools::domain::formatter_impl::AuditFormatterImpl;
    use crate::audit_tools::domain::value::{
        AuditEnvelope, AuditSummary, ExecutionStep, TopFailure, TopTemplate,
    };
    use crate::execution_tools::domain::value::ExecutionStatus;

    fn sample_envelope() -> AuditEnvelope {
        let now = Utc::now();
        AuditEnvelope::new(
            Uuid::nil(),
            ExecutionStatus::Completed,
            Some("test-template".into()),
            now - Duration::hours(1),
            now,
            5000,
            vec![ExecutionStep::new(
                "build".into(),
                true,
                None,
                serde_json::json!({"output": "ok"}),
                2000,
            )],
            Some(150),
            "abc123hmac".into(),
            vec![],
        )
    }

    fn sample_summary() -> AuditSummary {
        let now = Utc::now();
        AuditSummary::new(
            now - Duration::days(7),
            now,
            10,
            7,
            3,
            0.7,
            50000,
            Some(1500),
            vec![TopFailure::new(
                "Timeout error".into(),
                2,
                Some("code-review".into()),
            )],
            vec![TopTemplate::new("code-review".into(), 5, 10000)],
        )
    }

    #[test]
    fn test_format_audit_text_includes_all_fields() {
        let f = AuditFormatterImpl::new();
        let text = f.format_audit_text(&sample_envelope());
        assert!(text.contains(&Uuid::nil().to_string()));
        assert!(text.contains("Completed"));
        assert!(text.contains("5000 ms"));
        assert!(text.contains("build"));
    }

    #[test]
    fn test_format_audit_json_has_all_keys() {
        let f = AuditFormatterImpl::new();
        let json = f.format_audit_json(&sample_envelope());
        assert_eq!(
            json["execution_id"].as_str().unwrap(),
            Uuid::nil().to_string()
        );
        assert!(json["steps"].is_array());
        assert!(json["hmac"].as_str().unwrap() == "abc123hmac");
    }

    /// ISSUE-EVIDENCE-PRESERVATION AC #4: the read surface renders every
    /// finding array — text and JSON — so `rigorix_read_audit` can show the
    /// signed evidence that was previously absent entirely.
    #[test]
    fn test_format_audit_surfaces_findings() {
        use crate::audit_tools::domain::value::{
            PreconditionFinding, RequirementFinding, SequencePolicyFinding,
        };

        let now = Utc::now();
        let env = AuditEnvelope::new_with_findings(
            Uuid::nil(),
            ExecutionStatus::Failed,
            Some("precondition-demo".into()),
            now - Duration::minutes(1),
            now,
            1000,
            vec![],
            None,
            "sig".into(),
            vec![],
            vec![PreconditionFinding::new(
                "preflight".into(),
                "pay".into(),
                "failed".into(),
                Some(7),
                "sha256:abc".into(),
                now,
                "precondition refused".into(),
            )],
            vec![SequencePolicyFinding::new(
                "rule-1".into(),
                "deny".into(),
                "pay".into(),
                vec![1],
                "sequence denied".into(),
            )],
            vec![RequirementFinding::new(
                "req-1".into(),
                "pay".into(),
                "deny".into(),
                vec!["identity".into()],
                "requirement unmet".into(),
            )],
        );

        let f = AuditFormatterImpl::new();
        let json = f.format_audit_json(&env);
        assert_eq!(json["precondition_findings"][0]["outcome"], "failed");
        assert_eq!(json["precondition_findings"][0]["exit_code"], 7);
        assert_eq!(json["sequence_policy_findings"][0]["rule_id"], "rule-1");
        assert_eq!(json["requirement_findings"][0]["unmet"][0], "identity");

        let text = f.format_audit_text(&env);
        assert!(text.contains("Precondition Findings"));
        assert!(text.contains("preflight"));
        assert!(text.contains("Sequence Policy Findings"));
        assert!(text.contains("Requirement Findings"));
    }

    #[test]
    fn test_format_list_text_empty() {
        let f = AuditFormatterImpl::new();
        let text = f.format_list_text(&[]);
        assert!(text.contains("No audit records"));
    }

    #[test]
    fn test_format_list_text_with_envelopes() {
        let f = AuditFormatterImpl::new();
        let text = f.format_list_text(&[sample_envelope(), sample_envelope()]);
        assert!(text.contains("2 total"));
    }

    #[test]
    fn test_format_summary_text_includes_stats() {
        let f = AuditFormatterImpl::new();
        let text = f.format_summary_text(&sample_summary());
        assert!(text.contains("10"));
        assert!(text.contains("70.0%"));
    }

    #[test]
    fn test_format_summary_json_has_all_counts() {
        let f = AuditFormatterImpl::new();
        let json = f.format_summary_json(&sample_summary());
        assert_eq!(json["total_executions"].as_u64().unwrap(), 10);
        assert_eq!(json["success_count"].as_u64().unwrap(), 7);
    }
}
