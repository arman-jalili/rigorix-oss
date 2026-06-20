# Runbook: diff-analyzer

**Module:** `actions/src/diff_analyzer/`
**Epic:** diff-analyzer
**Last Updated:** 2026-06-20

## Overview

The diff-analyzer module parses GitHub Pull Request diffs into structured types,
validates file paths, enforces resource limits, classifies files by risk, and
detects AI-generated code signals. It is the input layer for the Policy Evaluator.

## Startup Sequence

### Dependencies

| Dependency | Type | Required | Description |
|-----------|------|----------|-------------|
| GitHub API | External | Yes | Fetching PR diffs |
| rigorix-engine | Internal | Yes | Core engine types |
| serde | Cargo | Yes | Serialization |
| async-trait | Cargo | Yes | Async trait support |

### Startup Procedure

1. **Configuration loading**: The module uses `PolicyLimits::default()` for limits.
   Override via `AnalyzeDiffInput.limits`.
2. **Service initialization**:
   ```rust
   use diff_analyzer::application::diff_analysis_pipeline_impl::DiffAnalysisPipelineImpl;

   let pipeline = DiffAnalysisPipelineImpl::default();
   ```
3. **Initialization check**: Call `pipeline.analyze_default("")` to verify
   all services initialize without processing any real diff.

## Graceful Shutdown

The diff-analyzer module is stateless — there are no in-memory caches,
open connections, or background tasks to clean up. Shutdown is immediate.

For long-running diff analysis, the cancellation pattern should be used:
- Pass `CancellationToken` to `AiSignalDetectionService::detect()`
- Check cancellation between hunk processing steps

## Common Failure Modes

### 1. Diff Parse Failure

**Symptoms:** `DiffAnalyzerError::DiffParseError` returned.

**Causes:**
- Malformed git diff output (corrupted headers, missing hunk markers)
- Unsupported diff format (not unified diff)

**Recovery:**
- Validate the diff source (GitHub API response, file content)
- Log the raw diff at DEBUG level for forensic analysis
- Return partial results if some files parsed successfully

### 2. Path Traversal Attack

**Symptoms:** `DiffAnalyzerError::PathTraversal` returned.

**Causes:**
- Malicious PR with `../` paths attempting directory traversal
- Accidental path construction errors

**Recovery:**
- Reject the entire diff — security violations are blocking
- Alert the security team
- Log the offending path (never log the full diff in production)

### 3. Limit Exceeded

**Symptoms:** `PrDiff.limits_exceeded = true`, files in `excluded_files`.

**Causes:**
- PR diff exceeds `max_diff_size` (default: 10 MB)
- PR has more files than `max_files` (default: 100)
- Single file exceeds `max_lines_per_file` (default: 5000)

**Recovery:**
- Progressive degradation is automatic — files within limits are processed
- Flag the excluded files to the caller for manual review
- If limits are too restrictive, adjust via `PolicyLimits`

### 4. GitHub API Error

**Symptoms:** `DiffAnalyzerError::GitHubApi` returned.

**Causes:**
- Network timeout
- Rate limiting (GitHub API)
- PR not found or access denied

**Recovery:**
- Retry with exponential backoff (max 3 retries)
- Check rate limit headers: `X-RateLimit-Remaining`
- Fall back to local diff if available (for testing)

### 5. AI Signal Detection Timeout

**Symptoms:** Long processing time on large diffs.

**Causes:**
- Very large diffs with many hunks (1000+)
- Complex pattern matching

**Recovery:**
- Limit hunks analyzed to first 5000 per file
- Check cancellation signal between hunks
- Reduce threshold for early exit

## Configuration Reference

### PolicyLimits

| Field | Default | Description |
|-------|---------|-------------|
| `max_diff_size` | 10,000,000 (10 MB) | Maximum diff size in bytes |
| `max_files` | 100 | Maximum number of files |
| `max_lines_per_file` | 5000 | Maximum lines per file |

### AI Detection

| Parameter | Default | Description |
|-----------|---------|-------------|
| `threshold` | 0.7 | Confidence threshold for flagging |
| `check_indentation` | true | Enable uniform indentation analysis |
| `check_comments` | true | Enable AI comment pattern detection |

## Monitoring

### Key Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| diff_parse_time_ms | Pipeline | Time to parse raw diff |
| diff_validation_time_ms | Pipeline | Time to validate paths |
| diff_enforcement_time_ms | Pipeline | Time to enforce limits |
| diff_classification_time_ms | Pipeline | Time to classify risk |
| diff_ai_detection_time_ms | Pipeline | Time for AI detection |
| total_files_parsed | Counter | Files successfully parsed |
| total_files_excluded | Counter | Files excluded by limits |
| total_path_violations | Counter | Security violations detected |
| total_ai_signals | Counter | AI signals detected |

### Log Levels

| Level | Usage |
|-------|-------|
| ERROR | Parse failures, security violations, API errors |
| WARN | Limit exceeded, partial parse, low-confidence AI signals |
| INFO | Analysis start/complete, file counts |
| DEBUG | Individual hunk parsing, path validation results |
| TRACE | Raw diff content, intermediate DTO values |
