## Intent

Currently ClaudeClassifier, OpenAIClassifier, MockClassifier, and MockParameterExtractor live in `planning/domain/` alongside trait definitions. Clean Architecture requires domain to contain only interfaces and entities. Move concrete implementations to `infrastructure/` and mocks to `application/`.

## Epic
EPIC-002: Architecture & Code Quality (Milestone #3)

## In Scope
- Move ClaudeClassifier → planning/infrastructure/claude_classifier.rs
- Move OpenAIClassifier → planning/infrastructure/openai_classifier.rs
- Move MockClassifier → planning/application/mock_classifier.rs
- Move MockParameterExtractor → planning/application/mock_extractor.rs
- Update all imports in planning module and tests
- Delete old files from planning/domain/
- Verify full test suite passes

## Out of Scope
- Merge execution module (EPIC-002-ISSUE-002)
- Compiler warnings (EPIC-002-ISSUE-003)
- is_retriable() (EPIC-002-ISSUE-004)

## Acceptance Criteria
- [ ] ClaudeClassifier and OpenAIClassifier live in planning/infrastructure/
- [ ] MockClassifier and MockParameterExtractor live in planning/application/
- [ ] planning/domain/ contains only trait definitions and entities
- [ ] `cargo test` passes with all 889+ tests
- [ ] `cargo build` produces zero errors

## Implementation Notes
- Move files first, then update mod.rs declarations, then fix imports
- After this fix, update ADR-003 to reflect actual file layout
- Update import paths in tests

## Files Changed
- move: src/planning/domain/claude_classifier.rs → src/planning/infrastructure/claude_classifier.rs
- move: src/planning/domain/openai_classifier.rs → src/planning/infrastructure/openai_classifier.rs
- move: src/planning/domain/mock_classifier.rs → src/planning/application/mock_classifier.rs
- move: src/planning/domain/mock_extractor.rs → src/planning/application/mock_extractor.rs
- modify: src/planning/domain/mod.rs
- modify: src/planning/infrastructure/mod.rs
- modify: src/planning/application/mod.rs
- modify: src/planning/tests.rs

## Validators Required
- ci, tests, architecture
