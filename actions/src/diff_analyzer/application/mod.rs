//! Application layer for the Diff Analyzer bounded context.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#application
//! Implements: Contract Freeze — service traits, DTO schemas, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines the application-level contracts:
//! - Service interfaces (use cases) in `service.rs`
//! - Input/output DTO schemas in `dto/`
//! - Factory interfaces in `factory.rs`
//!
//! # Contract (Frozen)
//! - All service traits are async (use `async-trait`)
//! - All public methods return `Result<_, DiffAnalyzerError>`
//! - DTOs carry full documentation for each field
//! - No implementation — only contract signatures

pub mod ai_signal_detector_impl;
pub mod diff_analysis_pipeline_impl;
pub mod diff_parser_impl;
pub mod dto;
pub mod factory;
pub mod limit_enforcer_impl;
pub mod path_validator_impl;
pub mod risk_classifier_impl;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use service::*;
