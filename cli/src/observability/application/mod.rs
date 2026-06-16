//! Observability application layer — service traits, DTOs for CLI observability.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — TracingInitializer trait, DTO schemas
//! Issue: issue-contract-freeze
//!
//! Defines the application-level contracts for CLI observability operations.
//! Service traits (use cases) live here, implementations live in infrastructure.
//!
//! # Contract (Frozen)
//! - Service traits define all use cases for CLI observability
//! - DTOs define input/output schemas for each operation
//! - No implementation — only contract signatures and type definitions

pub mod dto;
pub mod service;

pub use service::TracingInitializer;
