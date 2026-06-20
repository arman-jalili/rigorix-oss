//! Domain entities and interfaces for the Diff Analyzer bounded context.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#domain
//! Implements: Contract Freeze — domain entities PrDiff, ChangedFile, DiffHunk,
//! FileStatus, FileRisk, PolicyLimits, AiSignal, AiSignalResult,
//! DiffAnalyzerError, DiffAnalyzerEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types for PR diff analysis.
//! These are pure domain objects with no framework dependencies.
//! They serve as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

pub mod error;
pub mod event;
pub mod types;

pub use error::*;
pub use event::*;
pub use types::*;
