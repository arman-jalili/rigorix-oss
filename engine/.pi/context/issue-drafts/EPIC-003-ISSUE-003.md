---
guardian_issue:
  id: EPIC-003-ISSUE-003
  title: "Live LLM API integration tests (H-03)"
  epic: "Testing Hardening"
  epic_id: EPIC-003
  status: planned
  priority: medium
  created_at: "2026-06-15"

  intent: |
    Add integration tests that exercise ClaudeClassifier, OpenAIClassifier, and
    ClaudeTemplateGenerator against real LLM APIs. Currently all classifiers
    and generators are tested with mocks only. Add behind a feature flag and
    CI-optional manual stage.

  dependencies:
    - name: "EPIC-002-ISSUE-001"
      type: internal
      note: "Classifier files may move in ISSUE-001; do live tests after files settle"

  in_scope:
    - Add `#[cfg(feature = "live-tests")]` feature flag to Cargo.toml
    - **ClaudeClassifier:** Test classify_with_alternatives() against real Anthropic API.
      Verify: structured JSON response, confidence in [0,1], token usage populated.
    - **OpenAIClassifier:** Test against real OpenAI API. Same contract verification.
    - **ClaudeTemplateGenerator:** Test generate() against real API. Verify: valid TOML,
      template registers successfully.
    - **Error handling:** Mock reqwest client to test: timeout → LlmError, 429 → retry,
      500 → retry exhaustion.
    - API keys sourced from environment variables only (CLAUDE_API_KEY, OPENAI_API_KEY)
    - Add CI job as optional manual stage (requires secrets)

  out_of_scope:
    - Full e2e integration suite (ISSUE-002)
    - Concurrent-safety tests (ISSUE-001)

  affected_layers:
    testing:
      - "New: engine/src/planning/tests/live_claude_classifier_tests.rs"
      - "New: engine/src/planning/tests/live_openai_classifier_tests.rs"
      - "New: engine/src/template_generation/tests/live_generator_tests.rs"
    infrastructure:
      - "Modified: Cargo.toml (add live-tests feature flag)"

  acceptance_criteria:
    - "live-tests feature compiles: `cargo build --features live-tests`"
    - "ClaudeClassifier integration test passes against real API (when CLAUDE_API_KEY set)"
    - "OpenAIClassifier integration test passes against real API (when OPENAI_API_KEY set)"
    - "TemplateGenerator integration test produces valid TOML (when CLAUDE_API_KEY set)"
    - "Error handling tests pass without real API (mock reqwest)"
    - "No API keys hardcoded in any source file"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - Use `#[cfg(feature = "live-tests")]` and skip tests when env var not set:
      ```rust
      #[test]
      #[cfg(feature = "live-tests")]
      fn test_claude_classifier_live() {
          let api_key = std::env::var("CLAUDE_API_KEY")
              .expect("Set CLAUDE_API_KEY env var for live tests");
          // ...
      }
      ```
    - For error handling tests (timeout, 429, 500), use a mock HTTP server
      (e.g., httpmock or wiremock) to simulate API failures without real calls
    - These tests go in the existing module test files, not in engine/tests/
    - CI job: optional manual trigger, requires repo secrets for API keys
    - Security: never log API keys, never print request bodies that contain keys

  file_changes:
    - "modify: engine/Cargo.toml (add [features] live-tests = [])"
    - "create: engine/src/planning/tests/live_claude_classifier_tests.rs"
    - "create: engine/src/planning/tests/live_openai_classifier_tests.rs"
    - "create: engine/src/template_generation/tests/live_generator_tests.rs"
    - "modify: CI config for optional manual live-tests stage"
---

# EPIC-003-ISSUE-003: Live LLM API integration tests (H-03)

## Intent

Test ClaudeClassifier, OpenAIClassifier, and ClaudeTemplateGenerator against real LLM APIs, behind a feature flag.

## Dependencies

```
EPIC-002-ISSUE-001 (classifier file moves) → EPIC-003-ISSUE-003
```

## In Scope

- Live API tests for ClaudeClassifier, OpenAIClassifier, TemplateGenerator
- Error handling tests with mock HTTP server
- Feature-gated behind `#[cfg(feature = "live-tests")]`
- Environment-variable-only API keys

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | `cargo build --features live-tests` compiles | CI |
| 2 | Claude live test passes (with API key) | Tests |
| 3 | OpenAI live test passes (with API key) | Tests |
| 4 | TemplateGenerator produces valid TOML | Tests |
| 5 | No API keys in source code | Security |
| 6 | Error handling (timeout, 429, 500) works with mock server | Tests |
