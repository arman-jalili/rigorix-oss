//! Application layer for the Policy Evaluator bounded context.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#application
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
//! - All public methods return `Result<_, PolicyError>`
//! - DTOs carry full documentation for each field
//! - No implementation — only contract signatures

pub mod compiled_rules_factory_impl;
pub mod dto;
pub mod factory;
pub mod org_policy_merger_impl;
pub mod policy_document_factory_impl;
pub mod policy_evaluation_pipeline_impl;
pub mod policy_evaluator_impl;
pub mod policy_loader_impl;
pub mod policy_report_generator_impl;
pub mod policy_result_factory_impl;
pub mod policy_tamper_detector_impl;
pub mod rules_factory_impl;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use service::*;
