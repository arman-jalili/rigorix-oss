## Intent

Add integration tests that exercise ClaudeClassifier, OpenAIClassifier, and ClaudeTemplateGenerator against real LLM APIs. Currently all classifiers and generators are tested with mocks only. Add behind a feature flag and CI-optional manual stage.

## Epic
EPIC-003: Testing Hardening (Milestone #1)

## In Scope
- Add #[cfg(feature = "live-tests")] feature flag to Cargo.toml
- ClaudeClassifier: Test classify_with_alternatives() against real Anthropic API
- OpenAIClassifier: Test against real OpenAI API
- ClaudeTemplateGenerator: Test generate() against real API
- Error handling: Mock reqwest client to test timeout, 429, 500
- API keys sourced from environment variables only (CLAUDE_API_KEY, OPENAI_API_KEY)
- Add CI job as optional manual stage (requires secrets)

## Security Condition (from validator)
API keys MUST be sourced from environment variables only. Never hardcoded in source files.

## Out of Scope
- Full e2e integration suite (ISSUE-002)
- Concurrent-safety tests (ISSUE-001)

## Acceptance Criteria
- [ ] live-tests feature compiles: cargo build --features live-tests
- [ ] ClaudeClassifier integration test passes against real API (when CLAUDE_API_KEY set)
- [ ] OpenAIClassifier integration test passes against real API (when OPENAI_API_KEY set)
- [ ] TemplateGenerator integration test produces valid TOML (when CLAUDE_API_KEY set)
- [ ] Error handling tests pass without real API (mock reqwest)
- [ ] No API keys hardcoded in any source file

## Implementation Notes
- Use #[cfg(feature = "live-tests")] and skip tests when env var not set
- For error handling tests, use a mock HTTP server (wiremock or httpmock)
- Security: never log API keys, never print request bodies that contain keys

## Files Changed
- modify: engine/Cargo.toml (add [features] live-tests = [])
- create: engine/src/planning/tests/live_claude_classifier_tests.rs
- create: engine/src/planning/tests/live_openai_classifier_tests.rs
- create: engine/src/template_generation/tests/live_generator_tests.rs

## Validators Required
- ci, tests, security
