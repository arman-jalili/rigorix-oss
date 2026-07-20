//! Domain entities and interfaces for the Scored Evaluation bounded context.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md
//! Implements: Contract Freeze — ScoredEvaluationNode, Rubric, ScoringResult,
//!              ScoringBackend, ScoredEvaluationEvent, ScoredEvaluationError
//! Issue: #673 (scored-evaluation epic)
//!
//! This module defines the core domain types — `ScoredEvaluationNode`, `Rubric`,
//! `ScoringResult`, `ScoreDimension`, `ScoringBackend` (trait),
//! `ScoredEvaluationEvent`, and `ScoredEvaluationError`. These are pure domain
//! objects with no framework dependencies. They serve as the frozen contract
//! that all implementations must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond enum variants, accessors, and constructors
//! - All evaluation orchestration logic must happen in the application layer
//! - All persistence must happen behind repository interfaces
//! - The ScoringBackend trait is the core domain contract — all backends
//!   (MCP, HTTP, Local) must satisfy this interface

pub mod backend;
pub mod error;
pub mod event;
pub mod node;
pub mod result;
pub mod rubric;

pub use backend::ScoringBackend;
pub use error::ScoredEvaluationError;
pub use event::ScoredEvaluationEvent;
pub use node::ScoredEvaluationNode;
pub use result::ScoreDimension;
pub use result::ScoringResult;
pub use rubric::Rubric;
pub use rubric::RubricSource;
