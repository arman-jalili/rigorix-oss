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
pub mod service;

pub use factory::*;
pub use service::*;
