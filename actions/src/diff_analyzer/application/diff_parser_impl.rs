//! Implementation of `DiffParsingService`.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#parser
//! Implements: DiffParsingService trait — parses raw git diff output into structured PrDiff
//! Issue: #553
//!
//! The DiffParser reads raw unified diff output (as returned by `git diff`)
//! and converts it into a structured `PrDiff` with `ChangedFile` entries,
//! `DiffHunk` sections, and line-level `DiffLine` items. It detects binary
//! files by scanning for NUL bytes and handles edge cases like rename-only
//! files, empty diffs, and corrupted hunk headers.

use async_trait::async_trait;

use crate::diff_analyzer::application::dto::{ParseDiffInput, ParseDiffOutput};
use crate::diff_analyzer::application::service::DiffParsingService;
use crate::diff_analyzer::domain::{
    ChangedFile, DiffAnalyzerError, DiffHunk, DiffLine, DiffLineType, DiffParseResult, FileStatus,
    PrDiff,
};

/// Implementation of `DiffParsingService` that parses raw unified diff output.
///
/// Splits raw diff output by `diff --git` headers, extracts metadata for each
/// file (path, status, binary flag), parses hunk headers and content lines,
/// and assembles the result into a `PrDiff`.
pub struct DiffParserImpl;

impl DiffParserImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DiffParserImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DiffParsingService for DiffParserImpl {
    async fn parse(&self, input: ParseDiffInput) -> Result<ParseDiffOutput, DiffAnalyzerError> {
        let raw = &input.raw_diff;
        if raw.is_empty() {
            return Ok(ParseDiffOutput {
                result: DiffParseResult {
                    diff: PrDiff {
                        files: Vec::new(),
                        total_size_bytes: 0,
                        excluded_files: Vec::new(),
                        limits_exceeded: false,
                        policy_modified: false,
                        ai_signals: None,
                        metadata: None,
                    },
                    errors: Vec::new(),
                    files_parsed: 0,
                    files_failed: 0,
                },
                has_binary_files: false,
                encoding: "utf-8".to_string(),
            });
        }

        let total_size_bytes = raw.len() as u64;
        let mut files = Vec::new();
        let mut errors = Vec::new();
        let mut has_binary = false;
        let mut policy_modified = false;

        // Split by "diff --git " headers
        let mut sections: Vec<&str> = Vec::new();
        let mut start = 0;
        let search_str = "\ndiff --git ";
        while let Some(pos) = raw[start..].find(search_str) {
            let abs_pos = start + pos;
            if sections.is_empty() {
                // First section: find the start of the first "diff --git "
                let first_start = raw.find("diff --git ").unwrap_or(0);
                sections.push(&raw[first_start..abs_pos]);
            } else {
                sections.push(&raw[start..abs_pos]);
            }
            start = abs_pos + 1;
        }
        // Last section
        if start < raw.len() {
            if sections.is_empty() {
                if let Some(first) = raw.find("diff --git ") {
                    sections.push(&raw[first..]);
                }
            } else {
                sections.push(&raw[start..]);
            }
        }

        for section in &sections {
            match self.parse_file_section(section).await {
                Ok(file) => {
                    if file.is_binary {
                        has_binary = true;
                    }
                    if file.path.contains("policy") || file.path.contains(".rigorix") {
                        policy_modified = true;
                    }
                    files.push(file);
                }
                Err(e) => {
                    errors.push(format!("Failed to parse file section: {}", e));
                }
            }
        }

        let metadata = input.pr_number.map(|pr_number| {
            crate::diff_analyzer::domain::DiffMetadata {
                pr_number,
                base_branch: input.base_branch.clone().unwrap_or_default(),
                head_branch: input.head_branch.clone().unwrap_or_default(),
                head_sha: input.head_sha.clone().unwrap_or_default(),
            }
        });

        let files_parsed = files.len();
        let files_failed = errors.len();

        Ok(ParseDiffOutput {
            result: DiffParseResult {
                diff: PrDiff {
                    files,
                    total_size_bytes,
                    excluded_files: Vec::new(),
                    limits_exceeded: false,
                    policy_modified,
                    ai_signals: None,
                    metadata,
                },
                errors,
                files_parsed,
                files_failed,
            },
            has_binary_files: has_binary,
            encoding: "utf-8".to_string(),
        })
    }

    async fn parse_file_section(
        &self,
        section: &str,
    ) -> Result<ChangedFile, DiffAnalyzerError> {
        let lines: Vec<&str> = section.lines().collect();
        if lines.is_empty() {
            return Err(DiffAnalyzerError::DiffParseError {
                detail: "Empty file section".to_string(),
                line: None,
            });
        }

        // Parse header line: "diff --git a/<old> b/<new>"
        let header = lines[0];
        let paths = self.extract_paths(header).await?;
        let path = if paths.1 != "/dev/null" {
            paths.1.clone()
        } else {
            paths.0.clone()
        };

        // Determine status
        let status = self.determine_status(&lines, &paths.0, &paths.1);

        // Check for binary
        let is_binary = lines.iter().any(|l| l.starts_with("Binary files "));

        // Parse hunks
        let mut hunks = Vec::new();
        let mut current_hunk_lines: Vec<&str> = Vec::new();
        let mut in_hunk = false;

        for line in &lines[1..] {
            if line.starts_with("@@") {
                if in_hunk && !current_hunk_lines.is_empty() {
                    let hunk_text = current_hunk_lines.join("\n");
                    match self.parse_hunk(&hunk_text).await {
                        Ok(hunk) => hunks.push(hunk),
                        Err(e) => {
                            return Err(DiffAnalyzerError::DiffParseError {
                                detail: format!("Failed to parse hunk: {}", e),
                                line: None,
                            });
                        }
                    }
                    current_hunk_lines.clear();
                }
                in_hunk = true;
                current_hunk_lines.push(line);
            } else if in_hunk {
                current_hunk_lines.push(line);
            }
        }

        // Parse last hunk
        if in_hunk && !current_hunk_lines.is_empty() {
            let hunk_text = current_hunk_lines.join("\n");
            match self.parse_hunk(&hunk_text).await {
                Ok(hunk) => hunks.push(hunk),
                Err(e) => {
                    return Err(DiffAnalyzerError::DiffParseError {
                        detail: format!("Failed to parse hunk: {}", e),
                        line: None,
                    });
                }
            }
        }

        // Calculate additions/deletions
        let mut additions = 0usize;
        let mut deletions = 0usize;
        for hunk in &hunks {
            for line in &hunk.lines {
                match line.line_type {
                    DiffLineType::Added => additions += 1,
                    DiffLineType::Deleted => deletions += 1,
                    _ => {}
                }
            }
        }

        Ok(ChangedFile {
            path: path.clone(),
            status,
            additions,
            deletions,
            is_binary,
            hunks,
            risk: crate::diff_analyzer::domain::FileRisk::Medium,
            raw_diff: Some(section.to_string()),
        })
    }

    async fn parse_hunk(&self, hunk_text: &str) -> Result<DiffHunk, DiffAnalyzerError> {
        let lines: Vec<&str> = hunk_text.lines().collect();
        if lines.is_empty() {
            return Err(DiffAnalyzerError::DiffParseError {
                detail: "Empty hunk".to_string(),
                line: None,
            });
        }

        // Parse hunk header: "@@ -old_start,old_lines +new_start,new_lines @@ optional_header"
        let header_line = lines[0];
        let header = header_line.to_string();

        // Extract the @@ ... @@ parts
        let hunk_parts = header_line
            .strip_prefix("@@")
            .and_then(|s| s.strip_suffix("@@").or_else(|| {
                // Try finding @@ at the end
                s.rfind("@@").map(|i| &s[..i])
            }))
            .map(|s| s.trim())
            .unwrap_or("");

        // Parse "-old_start,old_lines +new_start,new_lines"
        let parts: Vec<&str> = hunk_parts.split_whitespace().collect();
        let old_part = parts.first().unwrap_or(&"-0,0");
        let new_part = parts.get(1).unwrap_or(&"+0,0");

        let (old_start, old_lines) = Self::parse_hunk_range(old_part);
        let (new_start, new_lines) = Self::parse_hunk_range(new_part);

        // Parse hunk lines
        let mut hunk_lines = Vec::new();
        let mut old_lineno = old_start;
        let mut new_lineno = new_start;

        for line_ref in &lines[1..] {
            let line: &str = line_ref;
            let (line_type, content) = if line.starts_with('+') {
                let content = &line[1..];
                let lt = if content == "\\ No newline at end of file" {
                    DiffLineType::NoNewline
                } else {
                    DiffLineType::Added
                };
                (lt, content)
            } else if line.starts_with('-') {
                let content = &line[1..];
                let lt = if content == "\\ No newline at end of file" {
                    DiffLineType::NoNewline
                } else {
                    DiffLineType::Deleted
                };
                (lt, content)
            } else if line.starts_with(' ') {
                (DiffLineType::Context, &line[1..])
            } else if line.starts_with('\\') {
                (DiffLineType::NoNewline, line)
            } else {
                continue;
            };

            let (old_no, new_no) = match line_type {
                DiffLineType::Added => (None, Some(new_lineno)),
                DiffLineType::Deleted => (Some(old_lineno), None),
                _ => (Some(old_lineno), Some(new_lineno)),
            };

            let diff_line = DiffLine {
                content: content.to_string(),
                line_type,
                old_lineno: old_no,
                new_lineno: new_no,
            };

            match line_type {
                DiffLineType::Added => new_lineno += 1,
                DiffLineType::Deleted => old_lineno += 1,
                DiffLineType::Context => {
                    old_lineno += 1;
                    new_lineno += 1;
                }
                DiffLineType::NoNewline => {}
            }

            hunk_lines.push(diff_line);
        }

        Ok(DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            header,
            lines: hunk_lines,
        })
    }

    async fn detect_binary(&self, content: &[u8]) -> bool {
        content.iter().take(8192).any(|&b| b == 0)
    }

    async fn extract_paths(&self, header: &str) -> Result<(String, String), DiffAnalyzerError> {
        // Parse "diff --git a/<old> b/<new>"
        let without_prefix = header
            .strip_prefix("diff --git ")
            .ok_or_else(|| DiffAnalyzerError::DiffParseError {
                detail: format!("Invalid diff header format: {}", header),
                line: None,
            })?;

        let parts: Vec<&str> = without_prefix.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return Err(DiffAnalyzerError::DiffParseError {
                detail: format!("Cannot parse paths from header: {}", header),
                line: None,
            });
        }

        let old = parts[0].strip_prefix("a/").unwrap_or(parts[0]).to_string();
        let new = parts[1].strip_prefix("b/").unwrap_or(parts[1]).to_string();

        Ok((old, new))
    }
}

impl DiffParserImpl {
    /// Parse a hunk range string like "-1,3" or "+1" into (start, count).
    fn parse_hunk_range(range: &str) -> (usize, usize) {
        let trimmed = range.trim_start_matches('-').trim_start_matches('+');
        let parts: Vec<&str> = trimmed.splitn(2, ',').collect();
        let start = parts[0].parse::<usize>().unwrap_or(1);
        let count = if parts.len() > 1 {
            parts[1].parse::<usize>().unwrap_or(0)
        } else {
            0
        };
        (start, count)
    }

    /// Determine the file change status from diff metadata lines.
    fn determine_status(&self, lines: &[&str], _old_path: &str, new_path: &str) -> FileStatus {
        let has_rename_from = lines.iter().any(|l| l.starts_with("rename from "));
        let has_rename_to = lines.iter().any(|l| l.starts_with("rename to "));
        let is_new_file = lines.iter().any(|l| l.starts_with("new file mode"));
        let is_deleted_file = lines.iter().any(|l| l.starts_with("deleted file mode"));

        if is_new_file {
            FileStatus::Added
        } else if is_deleted_file {
            FileStatus::Deleted
        } else if has_rename_from && has_rename_to {
            let prev = lines
                .iter()
                .find(|l| l.starts_with("rename from "))
                .map(|l| l["rename from ".len()..].to_string())
                .unwrap_or_default();
            FileStatus::Renamed {
                previous_path: prev,
            }
        } else if new_path == "/dev/null" {
            FileStatus::Deleted
        } else {
            FileStatus::Modified
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_empty_diff() {
        let parser = DiffParserImpl::new();
        let input = ParseDiffInput {
            raw_diff: String::new(),
            pr_number: None,
            base_branch: None,
            head_branch: None,
            head_sha: None,
            detect_binary: None,
        };
        let result = parser.parse(input).await.unwrap();
        assert_eq!(result.result.files_parsed, 0);
        assert_eq!(result.result.files_failed, 0);
        assert!(result.result.diff.files.is_empty());
    }

    #[tokio::test]
    async fn test_parse_single_file_diff() {
        let parser = DiffParserImpl::new();
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
-    println!(\"Hello\");
+    println!(\"Hello, World!\");
+    println!(\"Added line\");
 }
";
        let input = ParseDiffInput {
            raw_diff: diff.to_string(),
            pr_number: Some(42),
            base_branch: Some("main".to_string()),
            head_branch: Some("feature".to_string()),
            head_sha: Some("abc123".to_string()),
            detect_binary: None,
        };
        let result = parser.parse(input).await.unwrap();
        assert_eq!(result.result.files_parsed, 1);
        assert_eq!(result.result.files_failed, 0);
        assert!(!result.has_binary_files);

        let diff = result.result.diff;
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].path, "src/main.rs");
        assert_eq!(diff.files[0].status, FileStatus::Modified);
        assert_eq!(diff.files[0].additions, 2);
        assert_eq!(diff.files[0].deletions, 1);
        assert!(!diff.files[0].is_binary);
        assert_eq!(diff.files[0].hunks.len(), 1);

        let hunk = &diff.files[0].hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.lines.len(), 5); // context + deleted + added + added + context

        // Verify metadata
        assert!(diff.metadata.is_some());
        let meta = diff.metadata.unwrap();
        assert_eq!(meta.pr_number, 42);
        assert_eq!(meta.base_branch, "main");
        assert_eq!(meta.head_branch, "feature");
        assert_eq!(meta.head_sha, "abc123");
    }

    #[tokio::test]
    async fn test_parse_new_file() {
        let parser = DiffParserImpl::new();
        let diff = "\
diff --git a/src/new.rs b/src/new.rs
new file mode 100644
index 000..abc 100644
--- /dev/null
+++ b/src/new.rs
@@ -0,0 +1,3 @@
+fn new_function() {
+    println!(\"new\");
+}
";
        let input = ParseDiffInput {
            raw_diff: diff.to_string(),
            pr_number: None,
            base_branch: None,
            head_branch: None,
            head_sha: None,
            detect_binary: None,
        };
        let output = parser.parse(input).await.unwrap();
        assert_eq!(output.result.files_parsed, 1);
        assert_eq!(output.result.diff.files[0].status, FileStatus::Added);
        assert_eq!(output.result.diff.files[0].additions, 3);
        assert_eq!(output.result.diff.files[0].deletions, 0);
    }

    #[tokio::test]
    async fn test_parse_deleted_file() {
        let parser = DiffParserImpl::new();
        let diff = "\
diff --git a/src/old.rs b/src/old.rs
deleted file mode 100644
index abc..000 100644
--- a/src/old.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-fn old_function() {
-}
";
        let input = ParseDiffInput {
            raw_diff: diff.to_string(),
            pr_number: None,
            base_branch: None,
            head_branch: None,
            head_sha: None,
            detect_binary: None,
        };
        let output = parser.parse(input).await.unwrap();
        assert_eq!(output.result.files_parsed, 1);
        assert_eq!(output.result.diff.files[0].status, FileStatus::Deleted);
        assert_eq!(output.result.diff.files[0].additions, 0);
        assert_eq!(output.result.diff.files[0].deletions, 2);
    }

    #[tokio::test]
    async fn test_parse_renamed_file() {
        let parser = DiffParserImpl::new();
        let diff = "\
diff --git a/src/old.rs b/src/new.rs
similarity index 100%
rename from src/old.rs
rename to src/new.rs
";
        let input = ParseDiffInput {
            raw_diff: diff.to_string(),
            pr_number: None,
            base_branch: None,
            head_branch: None,
            head_sha: None,
            detect_binary: None,
        };
        let output = parser.parse(input).await.unwrap();
        assert_eq!(output.result.files_parsed, 1);
        match &output.result.diff.files[0].status {
            FileStatus::Renamed { previous_path } => {
                assert_eq!(previous_path, "src/old.rs");
            }
            _ => panic!("Expected Renamed status"),
        }
    }

    #[tokio::test]
    async fn test_detect_binary() {
        let parser = DiffParserImpl::new();
        let binary_content = vec![0u8, 137, 80, 78, 71]; // PNG header
        let text_content = b"Hello, world!";

        assert!(parser.detect_binary(&binary_content).await);
        assert!(!parser.detect_binary(text_content).await);
    }

    #[tokio::test]
    async fn test_extract_paths_valid() {
        let parser = DiffParserImpl::new();
        let (old, new) = parser
            .extract_paths("diff --git a/src/main.rs b/src/main.rs")
            .await
            .unwrap();
        assert_eq!(old, "src/main.rs");
        assert_eq!(new, "src/main.rs");
    }

    #[tokio::test]
    async fn test_extract_paths_different() {
        let parser = DiffParserImpl::new();
        let (old, new) = parser
            .extract_paths("diff --git a/src/old.rs b/src/new.rs")
            .await
            .unwrap();
        assert_eq!(old, "src/old.rs");
        assert_eq!(new, "src/new.rs");
    }

    #[tokio::test]
    async fn test_hunk_range_parsing() {
        assert_eq!(DiffParserImpl::parse_hunk_range("-1,3"), (1, 3));
        assert_eq!(DiffParserImpl::parse_hunk_range("+1,4"), (1, 4));
        assert_eq!(DiffParserImpl::parse_hunk_range("-0,0"), (0, 0));
        assert_eq!(DiffParserImpl::parse_hunk_range("+1"), (1, 0));
    }

    #[tokio::test]
    async fn test_parse_hunk_header() {
        let parser = DiffParserImpl::new();
        let hunk_text = "\
@@ -1,3 +1,4 @@
 fn main() {
-    println!(\"Hello\");
+    println!(\"Hello, World!\");
+    println!(\"Added\");
 }
";
        let hunk = parser.parse_hunk(hunk_text).await.unwrap();
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_lines, 3);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_lines, 4);
        assert_eq!(hunk.lines.len(), 5);
    }

    #[tokio::test]
    async fn test_parse_multiple_files() {
        let parser = DiffParserImpl::new();
        let diff = "\
diff --git a/src/file1.rs b/src/file1.rs
index abc..def 100644
--- a/src/file1.rs
+++ b/src/file1.rs
@@ -1 +1,2 @@
 fn a() {}
+fn b() {}
diff --git a/src/file2.rs b/src/file2.rs
new file mode 100644
index 000..abc 100644
--- /dev/null
+++ b/src/file2.rs
@@ -0,0 +1 @@
+fn c() {}
";
        let input = ParseDiffInput {
            raw_diff: diff.to_string(),
            pr_number: None,
            base_branch: None,
            head_branch: None,
            head_sha: None,
            detect_binary: None,
        };
        let output = parser.parse(input).await.unwrap();
        assert_eq!(output.result.files_parsed, 2);
        assert_eq!(output.result.diff.files[0].path, "src/file1.rs");
        assert_eq!(output.result.diff.files[1].path, "src/file2.rs");
    }

    #[tokio::test]
    async fn test_parse_policy_file_modified() {
        let parser = DiffParserImpl::new();
        let diff = "\
diff --git a/.rigorix/policy.toml b/.rigorix/policy.toml
index abc..def 100644
--- a/.rigorix/policy.toml
+++ b/.rigorix/policy.toml
@@ -1 +1,2 @@
 [policy]
+enabled = true
";
        let input = ParseDiffInput {
            raw_diff: diff.to_string(),
            pr_number: None,
            base_branch: None,
            head_branch: None,
            head_sha: None,
            detect_binary: None,
        };
        let output = parser.parse(input).await.unwrap();
        assert!(output.result.diff.policy_modified);
    }
}
