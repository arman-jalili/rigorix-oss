//! Failure Classification bounded context.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — failure-classification
//! Issue: #33
//!
//! This module classifies execution failures into typed categories for
//! retry routing. It maps error messages to `FailureType` via pattern
//! matching, and each `FailureType` maps to a recommended `RetryStrategy`.
//! Used by the DAG executor to decide how to recover from failures.
//!
//! # Architecture
//!
//! ```text
//! failure_classification/
//! ├── domain/               # Domain entities (FailureType, RetryStrategy), errors, events
//! │   ├── failure_type.rs   # FailureType enum (7 categories)
//! │   ├── retry_strategy.rs # RetryStrategy enum (5 variants)
//! │   ├── error.rs          # FailureClassificationError enum
//! │   └── event/            # FailureClassified event payload schemas
//! ├── application/          # Service traits, DTOs, factory interfaces
//! │   ├── service.rs        # FailureClassifierService, FailureMappingService traits
//! │   ├── factory.rs        # StrategyFactory trait
//! │   └── dto/              # Input/Output DTOs with validation
//! ├── infrastructure/       # Repository interfaces
//! │   └── repository/       # PatternRepository trait (extensible for custom rules)
//! └── interfaces/           # API contracts
//!     └── http/             # REST endpoint contracts
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
