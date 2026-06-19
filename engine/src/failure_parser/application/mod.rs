//! Application layer interfaces for the Failure Parser bounded context.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #495
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing parser and service instances
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, FailureParserError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod factory;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use service::*;
