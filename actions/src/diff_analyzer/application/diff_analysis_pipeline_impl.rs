//! Implementation of `DiffAnalysisPipelineService`.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: DiffAnalysisPipelineService trait — orchestrates the full analysis pipeline
//! Issue: #553, #554, #555, #556, #557
//!
//! Orchestrates the complete diff analysis flow:
//! 1. Parse raw diff → PrDiff
//! 2. Validate file paths
//! 3. Enforce resource limits
//! 4. Classify by risk
//! 5. Detect AI signals
//!
//! Returns a comprehensive analysis result with all intermediate outputs.

use async_trait::async_trait;
use std::time::Instant;

use crate::diff_analyzer::application::dto::{
    AnalyzeDiffInput, AnalyzeDiffOutput, ClassifyRiskInput, DetectAiSignalsInput,
    EnforceLimitsInput, ParseDiffInput, ValidatePathsInput,
};
use crate::diff_analyzer::application::service::{
    AiSignalDetectionService, DiffAnalysisPipelineService, DiffParsingService,
    LimitEnforcementService, PathValidationService, RiskClassificationService,
};
use crate::diff_analyzer::domain::{DiffAnalyzerError, PolicyLimits, PrDiff};

/// Implementation of `DiffAnalysisPipelineService`.
///
/// Orchestrates all analysis steps in sequence, tracking timing
/// and intermediate results for full transparency.
pub struct DiffAnalysisPipelineImpl {
    parser: Box<dyn DiffParsingService>,
    path_validator: Box<dyn PathValidationService>,
    limit_enforcer: Box<dyn LimitEnforcementService>,
    risk_classifier: Box<dyn RiskClassificationService>,
    ai_detector: Box<dyn AiSignalDetectionService>,
}

impl DiffAnalysisPipelineImpl {
    pub fn new(
        parser: Box<dyn DiffParsingService>,
        path_validator: Box<dyn PathValidationService>,
        limit_enforcer: Box<dyn LimitEnforcementService>,
        risk_classifier: Box<dyn RiskClassificationService>,
        ai_detector: Box<dyn AiSignalDetectionService>,
    ) -> Self {
        Self {
            parser,
            path_validator,
            limit_enforcer,
            risk_classifier,
            ai_detector,
        }
    }
}

impl Default for DiffAnalysisPipelineImpl {
    fn default() -> Self {
        Self::new(
            Box::new(super::diff_parser_impl::DiffParserImpl::new()),
            Box::new(super::path_validator_impl::PathValidatorImpl::new()),
            Box::new(super::limit_enforcer_impl::LimitEnforcerImpl::new()),
            Box::new(super::risk_classifier_impl::RiskClassifierImpl::new()),
            Box::new(super::ai_signal_detector_impl::AiSignalDetectorImpl::new()),
        )
    }
}

#[async_trait]
impl DiffAnalysisPipelineService for DiffAnalysisPipelineImpl {
    async fn analyze(&self, input: AnalyzeDiffInput) -> Result<AnalyzeDiffOutput, DiffAnalyzerError> {
        let start = Instant::now();

        // Step 1: Parse the raw diff
        let parse_input = ParseDiffInput {
            raw_diff: input.raw_diff,
            pr_number: input.pr_number,
            base_branch: input.base_branch,
            head_branch: input.head_branch,
            head_sha: input.head_sha,
            detect_binary: Some(true),
        };
        let parse_result = self.parser.parse(parse_input).await?;

        // Step 2: Validate file paths
        let validate_input = ValidatePathsInput {
            diff: parse_result.result.diff,
            allow_symlinks: input.allow_symlinks,
            allow_patterns: None,
        };
        let path_validation = self.path_validator.validate(validate_input).await?;

        // Step 3: Enforce limits
        let enforce_input = EnforceLimitsInput {
            diff: path_validation.diff.clone(),
            limits: input.limits,
            progressive_degradation: input.progressive_degradation,
        };
        let limit_enforcement = self.limit_enforcer.enforce(enforce_input).await?;

        // Step 4: Classify by risk
        let classify_input = ClassifyRiskInput {
            diff: limit_enforcement.diff.clone(),
            custom_patterns: input.custom_risk_patterns,
        };
        let risk_classification = self.risk_classifier.classify(classify_input).await?;

        // Step 5: Detect AI signals
        let detect_input = DetectAiSignalsInput {
            diff: risk_classification.diff.clone(),
            threshold: input.ai_threshold,
            check_indentation: input.check_indentation,
            check_comments: input.check_comments,
            custom_patterns: None,
        };
        let ai_detection = self.ai_detector.detect(detect_input).await?;

        // Build the final diff with AI signals
        let mut final_diff = risk_classification.diff.clone();
        final_diff.ai_signals = Some(ai_detection.result.clone());

        let processing_time_ms = start.elapsed().as_millis() as u64;

        Ok(AnalyzeDiffOutput {
            diff: final_diff,
            path_validation,
            limit_enforcement,
            risk_classification,
            ai_detection,
            processing_time_ms,
        })
    }

    async fn analyze_default(&self, raw_diff: String) -> Result<AnalyzeDiffOutput, DiffAnalyzerError> {
        let input = AnalyzeDiffInput {
            raw_diff,
            limits: PolicyLimits::default(),
            pr_number: None,
            base_branch: None,
            head_branch: None,
            head_sha: None,
            ai_threshold: Some(0.7),
            check_indentation: Some(true),
            check_comments: Some(true),
            custom_risk_patterns: None,
            allow_symlinks: Some(false),
            progressive_degradation: Some(true),
        };
        self.analyze(input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_analyzer::application::diff_parser_impl::DiffParserImpl;
    use crate::diff_analyzer::application::path_validator_impl::PathValidatorImpl;
    use crate::diff_analyzer::application::limit_enforcer_impl::LimitEnforcerImpl;
    use crate::diff_analyzer::application::risk_classifier_impl::RiskClassifierImpl;
    use crate::diff_analyzer::application::ai_signal_detector_impl::AiSignalDetectorImpl;
    use crate::diff_analyzer::domain::FileRisk;

    fn make_pipeline() -> DiffAnalysisPipelineImpl {
        DiffAnalysisPipelineImpl::new(
            Box::new(DiffParserImpl::new()),
            Box::new(PathValidatorImpl::new()),
            Box::new(LimitEnforcerImpl::new()),
            Box::new(RiskClassifierImpl::new()),
            Box::new(AiSignalDetectorImpl::new()),
        )
    }

    #[tokio::test]
    async fn test_analyze_simple_diff() {
        let pipeline = make_pipeline();
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1 +1,2 @@
 fn main() {}
+fn new_fn() {}
";
        let result = pipeline.analyze_default(diff.to_string()).await.unwrap();
        assert_eq!(result.diff.files.len(), 1);
        assert_eq!(result.diff.files[0].path, "src/main.rs");
        assert!(result.path_validation.all_valid);
        assert!(!result.limit_enforcement.any_exceeded);
        assert!(result.risk_classification.classifications.iter().any(|c| c.risk == FileRisk::Medium));
    }

    #[tokio::test]
    async fn test_analyze_empty_diff() {
        let pipeline = make_pipeline();
        let result = pipeline.analyze_default(String::new()).await.unwrap();
        assert!(result.diff.files.is_empty());
        assert_eq!(result.diff.total_size_bytes, 0);
    }

    #[tokio::test]
    async fn test_analyze_path_traversal_blocked() {
        let pipeline = make_pipeline();
        let diff = "\
diff --git a/../etc/passwd b/../etc/passwd
new file mode 100644
index 000..abc 100644
--- /dev/null
+++ b/../etc/passwd
@@ -0,0 +1 @@
+root:x:0:0:root:/root:/bin/bash
";
        let result = pipeline.analyze_default(diff.to_string()).await.unwrap();
        assert!(!result.path_validation.all_valid);
        assert!(result.path_validation.violation_count > 0);
    }

    #[tokio::test]
    async fn test_analyze_with_limits() {
        let pipeline = make_pipeline();
        let diff = "\
diff --git a/src/big.rs b/src/big.rs
index abc..def 100644
--- /dev/null
+++ b/src/big.rs
@@ -0,0 +1,100 @@
+fn big() {}
+// 100 lines
";
        let input = AnalyzeDiffInput {
            raw_diff: diff.to_string(),
            limits: PolicyLimits::new(10_000_000, 100, 5000),
            pr_number: Some(1),
            base_branch: Some("main".to_string()),
            head_branch: Some("feature".to_string()),
            head_sha: Some("abc".to_string()),
            ai_threshold: Some(0.7),
            check_indentation: Some(true),
            check_comments: Some(true),
            custom_risk_patterns: None,
            allow_symlinks: Some(false),
            progressive_degradation: Some(true),
        };
        let result = pipeline.analyze(input).await.unwrap();
        assert_eq!(result.diff.files.len(), 1);
        let meta = result.diff.metadata.as_ref().unwrap();
        assert_eq!(meta.pr_number, 1);
    }

    #[tokio::test]
    async fn test_analyze_mixed_risk_files() {
        let pipeline = make_pipeline();
        let diff = "\
diff --git a/src/auth/login.rs b/src/auth/login.rs
new file mode 100644
index 000..abc 100644
--- /dev/null
+++ b/src/auth/login.rs
@@ -0,0 +1 @@
+fn login() {}
diff --git a/README.md b/README.md
index abc..def 100644
--- a/README.md
+++ b/README.md
@@ -1 +1 @@
-# Project
+## Project
";
        let result = pipeline.analyze_default(diff.to_string()).await.unwrap();
        let critical_count = result.risk_classification.critical_files.len();
        let low_count = result.risk_classification.classifications.iter().filter(|c| c.risk == FileRisk::Low).count();
        assert_eq!(critical_count, 1);
        assert_eq!(low_count, 1);
    }

    #[tokio::test]
    async fn test_ai_signals_attached() {
        let pipeline = make_pipeline();
        let diff = "\
diff --git a/src/ai_code.rs b/src/ai_code.rs
new file mode 100644
index 000..abc 100644
--- /dev/null
+++ b/src/ai_code.rs
@@ -0,0 +1,3 @@
+/// Here's the implementation
+fn process() {
+    let x = 1;
+}
";
        let result = pipeline.analyze_default(diff.to_string()).await.unwrap();
        assert!(result.diff.ai_signals.is_some());
        let signals = result.diff.ai_signals.unwrap();
        assert!(signals.has_signals());
    }
}
