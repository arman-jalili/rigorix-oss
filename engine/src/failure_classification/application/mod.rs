//! Application layer interfaces for the Failure Classification bounded context.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #33
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing strategy objects
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, FailureClassificationError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod factory;
pub mod failure_classifier_service_impl;
pub mod failure_mapping_service_impl;
pub mod retry_strategy_integration_tests;
pub mod service;
pub mod strategy_factory_impl;

pub use dto::*;
pub use factory::*;
pub use failure_classifier_service_impl::*;
pub use failure_mapping_service_impl::*;
pub use service::*;
pub use strategy_factory_impl::*;
