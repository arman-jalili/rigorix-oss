//! Infrastructure layer for the Scored Evaluation bounded context.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md
//! Implements: Contract Freeze — infrastructure interfaces
//! Issue: #673 (scored-evaluation epic)
//!
//! This layer contains scoring backend implementations and repository
//! interfaces. All implementations must satisfy domain-level contracts
//! (`ScoringBackend` trait, `EvaluationRepository` trait).
//!
//! # Contract Freeze
//! - Backend implementations must satisfy the `ScoringBackend` trait
//! - Repository interfaces abstract data access behind traits
//! - No framework-specific annotations on trait definitions

pub mod backends;
pub mod repository;

pub use repository::EvaluationRepository;
