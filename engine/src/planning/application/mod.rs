//! Application layer interfaces for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#application
//! Implements: Contract Freeze — PlanningPipelineService trait, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines application-level service traits and factory interfaces
//! for the planning pipeline. These traits orchestrate the 6-phase planning flow
//! and provide the public API for consumers.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All async methods use `async-trait` for trait object safety
//! - No implementation — only contract signatures

pub mod dto;
pub mod factory;
pub mod mock_classifier;
pub mod mock_extractor;
pub mod pipeline_factory_impl;
pub mod pipeline_impl;
pub mod service;
pub mod symbol_validation_impl;

pub use factory::*;
pub use mock_classifier::*;
pub use mock_extractor::*;
pub use pipeline_factory_impl::*;
pub use pipeline_impl::*;
pub use service::*;
pub use symbol_validation_impl::*;
