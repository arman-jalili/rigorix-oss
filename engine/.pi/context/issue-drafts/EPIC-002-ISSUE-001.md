---
guardian_issue:
  id: EPIC-002-ISSUE-001
  title: "Move classifiers out of domain layer (H-04)"
  epic: "Architecture & Code Quality"
  epic_id: EPIC-002
  status: planned
  priority: high
  created_at: "2026-06-15"

  intent: |
    Currently ClaudeClassifier, OpenAIClassifier, MockClassifier, and
    MockParameterExtractor live in planning/domain/ alongside trait definitions.
    Clean Architecture requires domain to contain only interfaces and entities.
    Move concrete implementations to infrastructure/ and mocks to application/
    or behind #[cfg(test)].

  dependencies:
    - name: none
      type: internal
      note: "Root issue — no internal dependencies"

  in_scope:
    - Move ClaudeClassifier → planning/infrastructure/claude_classifier.rs
    - Move OpenAIClassifier → planning/infrastructure/openai_classifier.rs
    - Move MockClassifier → planning/application/mock_classifier.rs or behind #[cfg(test)]
    - Move MockParameterExtractor → planning/application/mock_extractor.rs or behind #[cfg(test)]
    - Update all imports in planning module and tests
    - Delete old files from planning/domain/
    - Verify full test suite passes with adjusted imports

  out_of_scope:
    - Merge execution module (ISSUE-002)
    - Compiler warnings (ISSUE-003)
    - is_retriable() (ISSUE-004)

  affected_layers:
    domain:
      - "Remove: planning/domain/claude_classifier.rs"
      - "Remove: planning/domain/openai_classifier.rs"
      - "Remove: planning/domain/mock_classifier.rs"
      - "Remove: planning/domain/mock_extractor.rs"
    infrastructure:
      - "New: planning/infrastructure/claude_classifier.rs"
      - "New: planning/infrastructure/openai_classifier.rs"
    application:
      - "New: planning/application/mock_classifier.rs"
      - "New: planning/application/mock_extractor.rs"

  acceptance_criteria:
    - "ClaudeClassifier and OpenAIClassifier live in planning/infrastructure/"
    - "MockClassifier and MockParameterExtractor live in planning/application/"
    - "planning/domain/ contains only trait definitions and entities"
    - "cargo test passes with all 889+ tests"
    - "cargo build produces zero errors"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - Move files first, then update mod.rs declarations, then fix imports
    - Update import paths in tests (e.g., use crate::planning::infrastructure::claude_classifier::ClaudeClassifier)
    - Check planning/domain/mod.rs to remove re-exports of moved files
    - Check planning/infrastructure/mod.rs to add re-exports
    - After this fix, update ADR-003 to reflect actual file layout

  file_changes:
    - "move: src/planning/domain/claude_classifier.rs → src/planning/infrastructure/claude_classifier.rs"
    - "move: src/planning/domain/openai_classifier.rs → src/planning/infrastructure/openai_classifier.rs"
    - "move: src/planning/domain/mock_classifier.rs → src/planning/application/mock_classifier.rs"
    - "move: src/planning/domain/mock_extractor.rs → src/planning/application/mock_extractor.rs"
    - "modify: src/planning/domain/mod.rs"
    - "modify: src/planning/infrastructure/mod.rs"
    - "modify: src/planning/application/mod.rs"
    - "modify: src/planning/tests.rs"
---

# EPIC-002-ISSUE-001: Move classifiers out of domain layer (H-04)

## Intent

Fix Clean Architecture violation by moving LLM implementations from domain/ to infrastructure/.

## Dependencies

No internal dependencies. Root issue for EPIC-002.

## In Scope

- Move 4 files from domain/ → infrastructure/ or application/
- Update all imports across codebase
- Verify full test suite

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Domain contains only trait definitions | Architecture |
| 2 | All 889+ tests pass | Tests |
| 3 | `cargo build` zero errors | CI |
