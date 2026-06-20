//! Implementation of `PolicyEvaluationPipelineService`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Issue: issue-policyevaluator

use async_trait::async_trait;

use crate::policy_evaluator::domain::PolicyError;

use super::dto::{
    GenerateReportInput, GenerateReportOutput, RunPolicyEvaluationInput,
    RunPolicyEvaluationOutput, PolicyPipelineSummary,
};
use super::org_policy_merger_impl::OrgPolicyMergingServiceImpl;
use super::policy_evaluator_impl::PolicyEvaluationServiceImpl;
use super::policy_loader_impl::PolicyLoadingServiceImpl;
use super::policy_report_generator_impl::PolicyReportGenerationServiceImpl;
use super::policy_tamper_detector_impl::PolicyTamperDetectionServiceImpl;
use super::service::{
    OrgPolicyMergingService, PolicyEvaluationPipelineService, PolicyEvaluationService,
    PolicyLoadingService, PolicyReportGenerationService, PolicyTamperDetectionService,
};
use super::dto::{
    DetectTamperInput, EvaluatePolicyInput, LoadPolicyInput, MergePoliciesInput,
};

/// Default implementation of `PolicyEvaluationPipelineService`.
///
/// Orchestrates the end-to-end policy evaluation workflow:
/// 1. Load policy from base branch
/// 2. Detect policy tampering
/// 3. Load and merge org policy (if configured)
/// 4. Evaluate PR diff against merged policy
/// 5. Generate violation report
pub struct PolicyEvaluationPipelineServiceImpl;

#[async_trait]
impl PolicyEvaluationPipelineService for PolicyEvaluationPipelineServiceImpl {
    async fn run(
        &self,
        input: RunPolicyEvaluationInput,
    ) -> Result<RunPolicyEvaluationOutput, PolicyError> {
        let start = std::time::Instant::now();

        let loader = PolicyLoadingServiceImpl;
        let tamper_detector = PolicyTamperDetectionServiceImpl;
        let merger = OrgPolicyMergingServiceImpl;
        let evaluator = PolicyEvaluationServiceImpl;

        // Step 1: Load policy from base branch
        let load_input = LoadPolicyInput {
            policy_path: input.policy_path,
            base_ref: input.base_ref.clone(),
            repo: input.repo.clone(),
            log_content: Some(false),
        };

        let load_output = loader.load(load_input).await.map_err(|e| {
            tracing::warn!("Failed to load policy: {}", e);
            e
        })?;

        let mut policy = load_output.policy;
        let mut compiled_rules = load_output.compiled_rules;
        let policy_loaded = true;

        // Step 2: Detect policy tampering
        let tamper_input = DetectTamperInput {
            diff: input.diff.clone(),
            policy_path: ".rigorix/policy.toml".to_string(), // Use the path from config
        };
        let tamper_output = tamper_detector.detect(tamper_input).await?;
        let tamper_detected = tamper_output.tamper_detected;

        // Step 3: Load and merge org policy
        let mut org_policy_merged = false;

        if let Some(org_config) = &input.org_policy_config {
            let org_load_input = crate::policy_evaluator::application::dto::LoadOrgPolicyInput {
                org_config: org_config.clone(),
                base_ref: input.base_ref.clone(),
                repo: input.repo.clone(),
                require_org_policy: None,
            };

            if let Ok(org_output) = merger.load_org_policy(org_load_input).await {
                if let Some(org_policy) = org_output.org_policy {
                    let merge_input = MergePoliciesInput {
                        repo_policy: policy,
                        org_policy: Some(org_policy),
                        merge_strategy: "restrictive".to_string(),
                    };

                    let merge_output = merger.merge(merge_input).await?;
                    policy = merge_output.merged_policy;
                    compiled_rules = loader.compile_patterns(&policy).await?;
                    org_policy_merged = merge_output.org_rules_added;
                }
            }
        }

        // Step 4: Evaluate PR diff
        let eval_input = EvaluatePolicyInput {
            diff: input.diff,
            policy: policy.clone(),
            compiled_rules,
            fail_on_violation: input.fail_on_violation,
            include_details: input.include_details,
        };

        let eval_output = evaluator.evaluate(eval_input).await?;
        let processing_time_ms = start.elapsed().as_millis() as u64;

        let summary = PolicyPipelineSummary {
            policy_loaded,
            org_policy_merged,
            tamper_detected,
            files_evaluated: eval_output.files_evaluated,
            violation_count: eval_output.result.violations.len(),
            blocking_count: eval_output.result.counts.deny,
            is_blocking: eval_output.result.has_blocking_violations,
        };

        Ok(RunPolicyEvaluationOutput {
            result: eval_output.result,
            policy_loaded,
            org_policy_merged,
            processing_time_ms,
            summary,
        })
    }

    async fn run_with_report(
        &self,
        input: RunPolicyEvaluationInput,
    ) -> Result<(RunPolicyEvaluationOutput, GenerateReportOutput), PolicyError> {
        let run_output = self.run(input).await?;

        let reporter = PolicyReportGenerationServiceImpl;

        let report_input = GenerateReportInput {
            result: run_output.result.clone(),
            diff: crate::diff_analyzer::domain::PrDiff {
                files: vec![],
                total_size_bytes: 0,
                excluded_files: vec![],
                limits_exceeded: false,
                policy_modified: run_output.summary.tamper_detected,
                ai_signals: None,
                metadata: None,
            },
            github_format: Some(true),
            include_violations: Some(true),
        };

        let report_output = reporter.generate_report(report_input).await?;

        Ok((run_output, report_output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_analyzer::domain::{ChangedFile, FileRisk, FileStatus, PrDiff};

    #[tokio::test]
    async fn test_pipeline_load_failure_returns_error() {
        let pipeline = PolicyEvaluationPipelineServiceImpl;
        let diff = PrDiff {
            files: vec![ChangedFile {
                path: "src/main.rs".to_string(),
                status: FileStatus::Modified,
                additions: 1,
                deletions: 0,
                is_binary: false,
                hunks: vec![],
                risk: FileRisk::Low,
                raw_diff: None,
            }],
            total_size_bytes: 100,
            excluded_files: vec![],
            limits_exceeded: false,
            policy_modified: false,
            ai_signals: None,
            metadata: None,
        };

        let input = RunPolicyEvaluationInput {
            diff,
            policy_path: ".rigorix/policy.toml".to_string(),
            base_ref: "origin/main".to_string(),
            repo: Some("org/repo".to_string()),
            org_policy_config: None,
            fail_on_violation: false,
            include_details: Some(false),
        };

        // Should fail because no GitHub API is available
        let result = pipeline.run(input).await;
        assert!(result.is_err());
    }
}
