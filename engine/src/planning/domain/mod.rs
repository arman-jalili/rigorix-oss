//! Domain entities and interfaces for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#domain
//! Implements: Contract Freeze — domain entities UserIntent, PlanningResult, PlanningHash,
//! PlanOutput, ClassificationResult, Classifier trait, ParameterExtractor trait, PlanningError
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — `UserIntent`, `PlanningResult`,
//! `PlanningHash`, `PlanOutput`, `ClassificationResult`, `Classifier` trait,
//! `ParameterExtractor` trait, and `PlanningError`. These are pure domain objects
//! with no framework dependencies. They serve as the frozen contract that all
//! implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

pub mod classification;
pub mod claude_classifier;
pub mod error;
pub mod event;
pub mod extractor;
pub mod generator;
pub mod intent;
pub mod mock_classifier;
pub mod mock_extractor;
pub mod openai_classifier;
pub mod result;

pub use classification::*;
pub use claude_classifier::*;
pub use error::*;
pub use extractor::*;
pub use generator::{
    GeneratedTemplate, GeneratedTemplateCost, GeneratorError, InvalidSymbolReference,
    RepoContext, TemplateGenerator,
};
pub use intent::*;
pub use mock_classifier::MockClassifier;
pub use mock_extractor::MockParameterExtractor;
pub use openai_classifier::*;
pub use result::*;
