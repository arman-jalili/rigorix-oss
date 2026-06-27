# ADR-009: Error Handling Pattern

## Status

Accepted (applied across all engine modules)

## Context

All engine modules need consistent error handling with typed errors, retry support,
and HTTP status code mapping.

## Decision

- Every bounded context defines its own error enum implementing `thiserror::Error`
- All errors derive into `CoreOrchestratorError` via `#[from]`
- Each error type implements `is_retriable()` and `error_code()`
- No `anyhow` in library code

## Consequences

- Strong type safety for error recovery
- Consistent error reporting across all contexts
- Slightly more code per context
