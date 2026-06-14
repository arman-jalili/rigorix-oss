//! Application layer for the DAG Engine bounded context.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#application
//! Implements: Contract Freeze — service interfaces, DTOs, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines the application-level contracts for DAG construction,
//! validation, and planning. It contains:
//! - `service`: Trait definitions for DAG operations
//! - `dto`: Input/output DTOs with validation and documentation
//! - `factory`: Factory traits for constructing service instances
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

pub mod dto;
pub mod factory;
pub mod service;
