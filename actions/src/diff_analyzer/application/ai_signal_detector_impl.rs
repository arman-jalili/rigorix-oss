//! Implementation of `AiSignalDetectionService`.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#ai-signals
//! Implements: AiSignalDetectionService trait — detects AI-generated code signals
//! Issue: #557
//!
//! The AiSignalDetector uses heuristic pattern matching to detect common
//! AI-generation artifacts in PR diffs:
//!
//! - AI-style comment patterns ("Here's the implementation...", "This function...")
//! - Unusually uniform indentation (characteristic of LLM output)
//! - Custom patterns for domain-specific detection
//!
//! Note: This is a heuristic, not a forensic tool. False positives are possible.
//! Results are advisory — they flag code for extra review, not block it.
//!
//! # Confidence Scoring
//!
//! Overall confidence = flagged_hunks / total_hunks
//! - < 0.3: Low confidence (likely no AI involvement)
//! - 0.3 - 0.7: Medium confidence (possible AI involvement)
//! - >= 0.7: High confidence (likely AI-generated)

use std::collections::HashMap;

use async_trait::async_trait;

use crate::diff_analyzer::application::dto::{DetectAiSignalsInput, DetectAiSignalsOutput};
use crate::diff_analyzer::application::service::AiSignalDetectionService;
use crate::diff_analyzer::domain::{AiSignal, AiSignalResult, DiffAnalyzerError};

/// Default AI comment patterns to detect.
const DEFAULT_AI_COMMENT_PATTERNS: &[&str] = &[
    "Here's the",
    "This function",
    "This implementation",
    "As requested",
    "I've added",
    "I've created",
    "I've updated",
    "I've modified",
    "I've implemented",
    "Let me",
    "We need to",
    "We should",
    "Note that",
    "Please note",
    "It's worth noting",
    "In other words",
    "Essentially",
    "Basically",
    "To clarify",
    "For context",
];

/// Implementation of `AiSignalDetectionService`.
///
/// Analyzes PR diffs for AI-generated code signals using heuristic
/// pattern matching on hunk content and indentation uniformity.
pub struct AiSignalDetectorImpl;

impl AiSignalDetectorImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AiSignalDetectorImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiSignalDetectionService for AiSignalDetectorImpl {
    async fn detect(
        &self,
        input: DetectAiSignalsInput,
    ) -> Result<DetectAiSignalsOutput, DiffAnalyzerError> {
        let threshold = input.threshold.unwrap_or(0.7);
        let check_indentation = input.check_indentation.unwrap_or(true);
        let check_comments = input.check_comments.unwrap_or(true);
        let custom_patterns = input.custom_patterns.unwrap_or_default();

        let mut signals = Vec::new();
        let mut total_hunks = 0usize;
        let mut flagged_hunks = 0usize;

        for file in input.diff.changed_files() {
            for hunk in &file.hunks {
                total_hunks += 1;
                let content = hunk.content();

                // Pattern 1: AI comment patterns
                if check_comments && self.has_ai_comment_pattern(&content).await {
                    flagged_hunks += 1;
                    signals.push(AiSignal {
                        file: file.path.clone(),
                        hunk_start: hunk.new_start,
                        pattern: "ai_comment".to_string(),
                        confidence: 0.7,
                        description: Some("AI-style explanatory comment detected".to_string()),
                    });
                }

                // Pattern 2: Uniform indentation
                if check_indentation && self.has_uniform_indentation(&content).await {
                    signals.push(AiSignal {
                        file: file.path.clone(),
                        hunk_start: hunk.new_start,
                        pattern: "uniform_indentation".to_string(),
                        confidence: 0.4,
                        description: Some("Unusually uniform indentation pattern".to_string()),
                    });
                }

                // Pattern 3: Custom patterns
                if !custom_patterns.is_empty() {
                    let custom_matches =
                        self.check_custom_patterns(&content, &custom_patterns).await;
                    for (pattern, conf) in &custom_matches {
                        signals.push(AiSignal {
                            file: file.path.clone(),
                            hunk_start: hunk.new_start,
                            pattern: pattern.clone(),
                            confidence: *conf,
                            description: Some(format!("Custom pattern '{}' matched", pattern)),
                        });
                    }
                    if !custom_matches.is_empty() {
                        flagged_hunks += 1;
                    }
                }
            }
        }

        let confidence = self.compute_confidence(flagged_hunks, total_hunks).await;

        Ok(DetectAiSignalsOutput {
            result: AiSignalResult {
                signals,
                confidence,
                total_hunks,
                flagged_hunks,
            },
            exceeds_threshold: confidence >= threshold,
            threshold,
            hunks_analyzed: total_hunks,
            hunks_flagged: flagged_hunks,
        })
    }

    async fn has_ai_comment_pattern(&self, content: &str) -> bool {
        let lower = content.to_lowercase();
        DEFAULT_AI_COMMENT_PATTERNS
            .iter()
            .any(|pattern| lower.contains(&pattern.to_lowercase()))
    }

    async fn has_uniform_indentation(&self, content: &str) -> bool {
        // Strip diff markers (+/-/ ) before measuring indentation
        let indent_lengths: Vec<usize> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let stripped = l
                    .strip_prefix('+')
                    .or_else(|| l.strip_prefix('-'))
                    .or_else(|| l.strip_prefix(' '))
                    .unwrap_or(l);
                stripped.chars().take_while(|c| c.is_whitespace()).count()
            })
            .collect();

        if indent_lengths.len() < 5 {
            return false;
        }

        // Count how many lines share the same indentation depth
        let mut indent_counts: HashMap<usize, usize> = HashMap::new();
        for &indent in &indent_lengths {
            if indent > 0 {
                *indent_counts.entry(indent).or_insert(0) += 1;
            }
        }

        let max_count = indent_counts.values().max().copied().unwrap_or(0);
        let ratio = max_count as f64 / indent_lengths.len() as f64;

        // >60% of non-empty lines share the same indentation → uniform
        ratio > 0.6
    }

    async fn check_custom_patterns(
        &self,
        content: &str,
        patterns: &HashMap<String, Vec<String>>,
    ) -> Vec<(String, f64)> {
        let mut matches = Vec::new();
        let lower_content = content.to_lowercase();

        for (pattern_name, triggers) in patterns {
            let matched_count = triggers
                .iter()
                .filter(|trigger| lower_content.contains(&trigger.to_lowercase()))
                .count();

            if matched_count > 0 {
                let confidence = (matched_count as f64 / triggers.len() as f64).min(1.0);
                matches.push((pattern_name.clone(), confidence));
            }
        }

        matches
    }

    async fn compute_confidence(&self, flagged_hunks: usize, total_hunks: usize) -> f64 {
        if total_hunks == 0 {
            0.0
        } else {
            flagged_hunks as f64 / total_hunks as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_analyzer::domain::PrDiff;
    use crate::diff_analyzer::domain::{
        ChangedFile, DiffHunk, DiffLine, DiffLineType, FileRisk, FileStatus,
    };

    fn make_hunk(lines: Vec<&str>, new_start: usize) -> DiffHunk {
        let diff_lines: Vec<DiffLine> = lines
            .iter()
            .map(|&l| {
                let (line_type, content) = if l.starts_with('+') {
                    (DiffLineType::Added, l[1..].to_string())
                } else if l.starts_with('-') {
                    (DiffLineType::Deleted, l[1..].to_string())
                } else {
                    (DiffLineType::Context, l.to_string())
                };
                DiffLine {
                    content,
                    line_type,
                    old_lineno: None,
                    new_lineno: None,
                }
            })
            .collect();

        DiffHunk {
            old_start: 1,
            old_lines: lines.len(),
            new_start,
            new_lines: lines.len(),
            header: format!("@@ -1,{} +{},{}, @@", lines.len(), new_start, lines.len()),
            lines: diff_lines,
        }
    }

    fn make_file(path: &str, hunks: Vec<DiffHunk>) -> ChangedFile {
        let additions = hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| l.line_type == DiffLineType::Added)
            .count();
        let deletions = hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| l.line_type == DiffLineType::Deleted)
            .count();
        ChangedFile {
            path: path.to_string(),
            status: FileStatus::Modified,
            additions,
            deletions,
            is_binary: false,
            hunks,
            risk: FileRisk::Medium,
            raw_diff: None,
        }
    }

    fn make_diff(files: Vec<ChangedFile>) -> PrDiff {
        let total = files
            .iter()
            .map(|f| f.additions + f.deletions)
            .sum::<usize>() as u64;
        PrDiff {
            files,
            total_size_bytes: total,
            excluded_files: Vec::new(),
            limits_exceeded: false,
            policy_modified: false,
            ai_signals: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_detect_no_signals() {
        let detector = AiSignalDetectorImpl::new();
        let hunk = make_hunk(vec![" fn existing() {", "     return 42;", " }"], 1);
        let diff = make_diff(vec![make_file("src/main.rs", vec![hunk])]);
        let input = DetectAiSignalsInput {
            diff,
            threshold: Some(0.7),
            check_indentation: Some(true),
            check_comments: Some(true),
            custom_patterns: None,
        };
        let result = detector.detect(input).await.unwrap();
        assert!(!result.exceeds_threshold);
        assert!(result.result.signals.is_empty());
    }

    #[tokio::test]
    async fn test_detect_ai_comment_pattern() {
        let detector = AiSignalDetectorImpl::new();
        let hunk = make_hunk(
            vec![
                "+/// Here's the implementation of the feature",
                "+fn new_feature() {",
                "+     // This function handles the request",
                "+     unimplemented!()",
                "+}",
            ],
            1,
        );
        let diff = make_diff(vec![make_file("src/new.rs", vec![hunk])]);
        let input = DetectAiSignalsInput {
            diff,
            threshold: Some(0.7),
            check_indentation: Some(true),
            check_comments: Some(true),
            custom_patterns: None,
        };
        let result = detector.detect(input).await.unwrap();
        assert!(result.result.has_signals());
        let comment_signals: Vec<_> = result
            .result
            .signals
            .iter()
            .filter(|s| s.pattern == "ai_comment")
            .collect();
        assert!(!comment_signals.is_empty());
    }

    #[tokio::test]
    async fn test_detect_uniform_indentation() {
        let detector = AiSignalDetectorImpl::new();
        let hunk = make_hunk(
            vec![
                "+fn new_function() {",
                "+     let x = 1;",
                "+     let y = 2;",
                "+     let z = 3;",
                "+     let w = 4;",
                "+     let v = 5;",
                "+     println!(\"done\");",
                "+}",
            ],
            1,
        );
        let diff = make_diff(vec![make_file("src/uniform.rs", vec![hunk])]);
        let input = DetectAiSignalsInput {
            diff,
            threshold: Some(0.7),
            check_indentation: Some(true),
            check_comments: Some(true),
            custom_patterns: None,
        };
        let result = detector.detect(input).await.unwrap();
        let indent_signals: Vec<_> = result
            .result
            .signals
            .iter()
            .filter(|s| s.pattern == "uniform_indentation")
            .collect();
        assert!(!indent_signals.is_empty());
    }

    #[tokio::test]
    async fn test_has_ai_comment_pattern() {
        let detector = AiSignalDetectorImpl::new();
        assert!(
            detector
                .has_ai_comment_pattern("Here's the implementation")
                .await
        );
        assert!(
            detector
                .has_ai_comment_pattern("This function does X")
                .await
        );
        assert!(
            detector
                .has_ai_comment_pattern("As requested, I've added")
                .await
        );
        assert!(
            !detector
                .has_ai_comment_pattern("fn ordinary_function()")
                .await
        );
    }

    #[tokio::test]
    async fn test_uniform_indentation_threshold() {
        let detector = AiSignalDetectorImpl::new();
        // Code with varied indentation
        let varied = "\
fn main() {
let x = 1;
    let y = 2;
        let z = 3;
}";
        assert!(!detector.has_uniform_indentation(varied).await);

        // Code with highly uniform indentation
        let uniform = "\
fn main() {
    let x = 1;
    let y = 2;
    let z = 3;
    let w = 4;
    let v = 5;
}";
        assert!(detector.has_uniform_indentation(uniform).await);

        // Too few lines
        let few_lines = "let x = 1;";
        assert!(!detector.has_uniform_indentation(few_lines).await);
    }

    #[tokio::test]
    async fn test_custom_patterns() {
        let detector = AiSignalDetectorImpl::new();
        let mut patterns = HashMap::new();
        patterns.insert(
            "halucination".to_string(),
            vec!["nonexistent_api".to_string(), "fake_library".to_string()],
        );

        let matches = detector
            .check_custom_patterns("this uses nonexistent_api", &patterns)
            .await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "halucination");
        assert!(matches[0].1 > 0.0);
    }

    #[tokio::test]
    async fn test_compute_confidence() {
        let detector = AiSignalDetectorImpl::new();
        assert_eq!(detector.compute_confidence(0, 0).await, 0.0);
        assert_eq!(detector.compute_confidence(0, 10).await, 0.0);
        assert_eq!(detector.compute_confidence(5, 10).await, 0.5);
        assert_eq!(detector.compute_confidence(10, 10).await, 1.0);
    }

    #[tokio::test]
    async fn test_multiple_signals_in_one_hunk() {
        let detector = AiSignalDetectorImpl::new();
        // A hunk with both AI comments and uniform indentation
        let hunk = make_hunk(
            vec![
                "+/// Here's the implementation",
                "+fn process() {",
                "+     let a = 1;",
                "+     let b = 2;",
                "+     let c = 3;",
                "+     let d = 4;",
                "+     let e = 5;",
                "+     process(a, b);",
                "+}",
            ],
            1,
        );
        let diff = make_diff(vec![make_file("src/process.rs", vec![hunk])]);
        let input = DetectAiSignalsInput {
            diff,
            threshold: Some(0.5),
            check_indentation: Some(true),
            check_comments: Some(true),
            custom_patterns: None,
        };
        let result = detector.detect(input).await.unwrap();
        assert!(result.result.has_signals());
        assert!(result.result.signals.len() >= 2);
    }

    #[tokio::test]
    async fn test_threshold_filtering() {
        let detector = AiSignalDetectorImpl::new();
        let hunk = make_hunk(vec!["+fn test() {", "+     let x = 1;", "+}"], 1);
        let diff = make_diff(vec![make_file("src/test.rs", vec![hunk])]);
        let input = DetectAiSignalsInput {
            diff,
            threshold: Some(1.0), // Very high threshold
            check_indentation: Some(true),
            check_comments: Some(true),
            custom_patterns: None,
        };
        let result = detector.detect(input).await.unwrap();
        assert!(!result.exceeds_threshold);
    }
}
