# Diff Analyzer Architecture

<!--
Canonical Reference: .pi/architecture/modules/diff-analyzer.md
Blueprint Source: Ported from original Rigorix docs/ARCHITECTURE_GITHUB_ACTIONS.md §2.3 (2026-04-27)
Rationale: Analyze PR diffs — validate paths, enforce limits, detect AI-generated signals
-->

## Overview

The Diff Analyzer parses GitHub Pull Request diffs into a structured `PrDiff` type and enforces resource limits. It validates file paths against security rules, detects AI-generated code signals, and classifies file changes by risk level. This is the input layer for the Policy Evaluator.

## Responsibilities

- Parse GitHub PR diff output into structured `ChangedFile` entries
- Validate file paths: no traversal (`../`), no symlinks, no absolute paths
- Enforce resource limits: max diff size, max files, max lines per file
- Detect binary files in the diff (skip policy checking)
- Detect AI-generated code signals (Mode A — applies different policy to AI-authored code)
- Classify changed files by risk: Low (docs), Medium (source), High (migrations), Critical (auth)
- Progressive degradation: if a limit is exceeded, process what we can and flag the rest

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| PrDiff | `actions/src/diff_analyzer/diff.rs` | Structured representation of a PR diff | #diff |
| ChangedFile | `actions/src/diff_analyzer/file.rs` | Single changed file: path, status, hunks, risk | #file |
| DiffParser | `actions/src/diff_analyzer/parser.rs` | Parses raw git diff output → PrDiff | #parser |
| PathValidator | `actions/src/diff_analyzer/path_validator.rs` | Path traversal, symlink, binary detection | #path-validator |
| LimitEnforcer | `actions/src/diff_analyzer/limits.rs` | Enforces max diff size, max files, max lines | #limits |
| AiSignalDetector | `actions/src/diff_analyzer/ai_signals.rs` | Detects AI-generated code patterns | #ai-signals |
| RiskClassifier | `actions/src/diff_analyzer/risk.rs` | Classifies files by risk level from path | #risk |
| DiffAnalyzerError | `actions/src/diff_analyzer/error.rs` | Typed errors: PathTraversal, DiffTooLarge | #error |

---

## Component Details

### PrDiff

```rust
/// Structured representation of a Pull Request diff.
///
/// Contains all changed files with their hunks, plus metadata about
/// the diff size and any exceeded limits.
#[derive(Debug, Clone)]
pub struct PrDiff {
    /// All changed files in the PR.
    pub files: Vec<ChangedFile>,

    /// Total diff size in bytes.
    pub total_size_bytes: u64,

    /// Files that were excluded due to limit enforcement.
    pub excluded_files: Vec<String>,

    /// Whether any limit was exceeded.
    pub limits_exceeded: bool,

    /// Whether the policy file itself was modified.
    pub policy_modified: bool,
}

impl PrDiff {
    /// Iterate over changed files (excluding binary and excluded).
    pub fn changed_files(&self) -> impl Iterator<Item = &ChangedFile> {
        self.files.iter().filter(|f| !f.is_binary)
    }
}
```

### ChangedFile

```rust
/// A single file changed in a PR.
#[derive(Debug, Clone)]
pub struct ChangedFile {
    /// File path relative to repository root.
    pub path: String,

    /// Change status: added, modified, deleted, renamed.
    pub status: FileStatus,

    /// Number of lines added.
    pub additions: usize,

    /// Number of lines deleted.
    pub deletions: usize,

    /// Whether the file is binary (detected via NUL byte).
    pub is_binary: bool,

    /// The diff hunks for this file.
    pub hunks: Vec<DiffHunk>,

    /// Risk classification.
    pub risk: FileRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed { previous_path: String },
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub header: String,
}
```

### PathValidator

```rust
/// Validates file paths for security and correctness.
pub struct PathValidator;

impl PathValidator {
    /// Validate all paths in a diff. Returns the first security violation.
    pub fn validate_paths(diff: &PrDiff) -> Result<(), DiffAnalyzerError> {
        for file in &diff.files {
            Self::validate_single_path(&file.path)?;
        }
        Ok(())
    }

    fn validate_single_path(path: &str) -> Result<(), DiffAnalyzerError> {
        // Reject path traversal
        if path.contains("..") {
            return Err(DiffAnalyzerError::PathTraversal {
                path: path.to_string(),
            });
        }

        // Reject absolute paths
        if path.starts_with('/') {
            return Err(DiffAnalyzerError::AbsolutePath {
                path: path.to_string(),
            });
        }

        // Reject null bytes (path injection)
        if path.contains('\0') {
            return Err(DiffAnalyzerError::PathInjection {
                path: path.to_string(),
            });
        }

        Ok(())
    }

    /// Detect if a file is binary from its first bytes.
    pub fn detect_binary(content: &[u8]) -> bool {
        content.iter().take(8192).any(|&b| b == 0)
    }
}
```

### LimitEnforcer

```rust
/// Enforces resource limits on PR diffs.
///
/// Limits prevent DoS via massive diffs. When limits are exceeded,
/// processing continues with the files that fit within limits,
/// and the excess files are flagged in `PrDiff.excluded_files`.
pub struct LimitEnforcer {
    max_diff_size: u64,
    max_files: usize,
    max_lines_per_file: usize,
}

impl LimitEnforcer {
    pub fn new(limits: PolicyLimits) -> Self { ... }

    /// Enforce limits on a parsed diff. Files exceeding limits are excluded
    /// and recorded in `PrDiff.excluded_files`.
    pub fn enforce(&self, diff: &mut PrDiff) {
        // Check total size
        if diff.total_size_bytes > self.max_diff_size {
            diff.limits_exceeded = true;
            // Progressive degradation: keep files within limit
            let mut kept = Vec::new();
            let mut excluded = Vec::new();
            let mut running_size = 0u64;

            for file in diff.files.drain(..) {
                let file_size = (file.additions + file.deletions) as u64;
                if running_size + file_size <= self.max_diff_size {
                    running_size += file_size;
                    kept.push(file);
                } else {
                    excluded.push(file.path);
                }
            }
            diff.files = kept;
            diff.excluded_files.extend(excluded);
        }

        // Check per-file line limits
        for file in &mut diff.files {
            if file.additions + file.deletions > self.max_lines_per_file {
                file.hunks.truncate(self.max_lines_per_file);
            }
        }
    }
}
```

### RiskClassifier

```rust
/// Classifies changed files by risk level based on path patterns.
pub struct RiskClassifier;

impl RiskClassifier {
    pub fn classify(path: &str) -> FileRisk {
        if path.contains("migrations/") || path.ends_with(".sql") {
            FileRisk::High
        } else if path.contains("auth/") || path.contains("security/") {
            FileRisk::Critical
        } else if path.ends_with(".rs") || path.ends_with(".ts") || path.ends_with(".py") {
            FileRisk::Medium
        } else if path.ends_with(".md") || path.ends_with(".txt") {
            FileRisk::Low
        } else {
            FileRisk::Medium // Safe default
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileRisk {
    Low,       // Documentation, config
    Medium,    // Source code
    High,      // Migrations, SQL
    Critical,  // Auth, security
}
```

### AiSignalDetector

```rust
/// Detects AI-generated code signals in PR diffs.
///
/// Looks for common patterns in AI-generated code:
/// - Comment patterns ("Here's the implementation...", "This function...")
/// - Overly verbose variable names
/// - Hallucination indicators (non-existent APIs)
///
/// Note: This is a heuristic, not a forensic tool. False positives are possible.
/// Results are advisory — they flag code for extra review, not block it.
pub struct AiSignalDetector;

impl AiSignalDetector {
    /// Detect AI signals in a PR diff. Returns a confidence score (0.0-1.0)
    /// and a list of detected patterns with file locations.
    pub async fn detect(diff: &PrDiff) -> AiSignalResult {
        let mut signals = Vec::new();
        let mut total_hunks = 0usize;
        let mut flagged_hunks = 0usize;

        for file in diff.changed_files() {
            for hunk in &file.hunks {
                total_hunks += 1;

                // Pattern 1: AI-generated comment patterns
                if Self::has_ai_comment_pattern(&hunk.content()) {
                    flagged_hunks += 1;
                    signals.push(AiSignal {
                        file: file.path.clone(),
                        hunk_start: hunk.new_start,
                        pattern: "ai_comment".to_string(),
                        confidence: 0.7,
                    });
                }

                // Pattern 2: Unusually uniform indentation (AI hallmark)
                if Self::has_uniform_indentation(&hunk.content()) {
                    signals.push(AiSignal {
                        file: file.path.clone(),
                        hunk_start: hunk.new_start,
                        pattern: "uniform_indentation".to_string(),
                        confidence: 0.4,
                    });
                }
            }
        }

        let confidence = if total_hunks > 0 {
            flagged_hunks as f64 / total_hunks as f64
        } else {
            0.0
        };

        AiSignalResult { signals, confidence }
    }

    fn has_ai_comment_pattern(content: &str) -> bool {
        let patterns = [
            "Here's the",
            "This function",
            "This implementation",
            "As requested",
            "I've added",
            "I've created",
        ];
        patterns.iter().any(|p| content.contains(p))
    }

    fn has_uniform_indentation(content: &str) -> bool {
        let indent_lengths: Vec<usize> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
            .collect();

        if indent_lengths.len() < 5 {
            return false;
        }

        // Count how many lines share the same indentation
        let mode = indent_lengths.iter()
            .filter(|&&i| i > 0)
            .fold(HashMap::new(), |mut acc, &i| {
                *acc.entry(i).or_insert(0) += 1;
                acc
            })
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(_, count)| count)
            .unwrap_or(0);

        mode as f64 / indent_lengths.len() as f64 > 0.6
    }
}

#[derive(Debug, Clone)]
pub struct AiSignal {
    pub file: String,
    pub hunk_start: usize,
    pub pattern: String,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct AiSignalResult {
    pub signals: Vec<AiSignal>,
    /// Overall AI-like confidence score.
    pub confidence: f64,
}
```

---

## Data Flow

```
PR opened → GitHub API returns diff
        │
        ▼
DiffParser::parse(raw_diff)
  - Splits into files by "diff --git" headers
  - Extracts hunks with line numbers
  - Detects binary files
        │
        ▼
PrDiff { files, total_size_bytes, ... }
        │
        ▼
PathValidator::validate_paths(&diff)
  - Rejects ../, absolute paths, null bytes
  - Detects policy file modifications
        │
        ▼
LimitEnforcer::enforce(&mut diff)
  - Applies max_diff_size, max_files, max_lines_per_file
  - Progressive degradation: keeps what fits
        │
        ▼
RiskClassifier::classify(path) per file
  - Low (docs) / Medium (source) / High (migrations) / Critical (auth)
        │
        ▼
AiSignalDetector::detect(&diff)
  - Heuristic AI pattern detection
  - Returns confidence score + signal list
        │
        ▼
Structured PrDiff fed to PolicyEvaluator
```

---

## Dependencies

### Depends On
- **GitHub API**: Fetching PR diff via `GitHubClient`
- **Globset**: Glob pattern compilation (shared with policy-evaluator)

### Used By
- **policy-evaluator**: `PrDiff` is the input to policy evaluation
- **ci-integration**: Risk classifications feed into status checks
- **audit-posting**: Diff metadata included in audit records

---

## Related ADRs

- **Actions ADR-101** (`actions/.pi/architecture/decisions/ADR-101-actions-as-thin-adapter.md`): Diff analysis is external to engine
- **Actions ADR-102** (`actions/.pi/architecture/decisions/ADR-102-github-event-routing.md`): PR opened triggers diff analysis

---

*Last updated: 2026-06-20*
*Module version: 1.0.0 (Planned)*
*Ported from: original Rigorix docs/ARCHITECTURE_GITHUB_ACTIONS.md §2.3*

---

**Status:** Planned
**Engine modules reused:** None (standalone diff parsing — engine doesn't do PR diff analysis)
