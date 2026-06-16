//! Application layer interfaces for the Orchestrator bounded context.
//!
//! @canonical .pi/architecture/modules/orchestrator.md
//! Implements: Contract Freeze — service traits, builder, DTOs
//! Issue: #338
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - OrchestratorBuilder for constructing an OrchestratorService from Config
//! - Input/Output DTOs with validation
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, OrchestratorError>`
//! - DTOs include validation documentation
//! - No implementation logic — only trait definitions

pub mod builder;
pub mod dto;
pub mod service;

pub use builder::*;
pub use dto::*;
pub use service::*;
