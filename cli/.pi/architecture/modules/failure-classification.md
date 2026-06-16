# Failure Classification

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Classifies execution failures into typed categories for retry routing. Maps error messages to `FailureType` (7 categories) via pattern matching, each mapping to a recommended `RetryStrategy`. Used by the DAG executor to decide how to recover from failures.

## Components

**CLI-facing:** None — CLI wraps engine contracts directly. No CLI-specific interface files needed.

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| FailureType | `engine/src/failure_classification/domain/failure_type.rs` | Enum: Transient, LspConflict, CompileError, TestFailure, MissingDependency, PlanConflict, Permanent, Unknown |
| RetryStrategy | `engine/src/failure_classification/domain/retry_strategy.rs` | Strategy mapping per FailureType |
| FailureClassifierService (trait) | `engine/src/failure_classification/application/service.rs` | Classification service |
| FailureMappingService (trait) | `engine/src/failure_classification/application/service.rs` | Error → FailureType mapping |
| FailureClassificationError | `engine/src/failure_classification/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| FailureClassified | A failure was classified with FailureType and RetryStrategy | FailureClassifierService |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| FailureType | Categorized failure variant: Transient, LspConflict, CompileError, TestFailure, MissingDependency, PlanConflict, Permanent, Unknown. |

## Dependencies

- Depends on: `engine::failure_classification` (all contracts frozen)
- Used by: `Execution Engine` (retry decision routing)
