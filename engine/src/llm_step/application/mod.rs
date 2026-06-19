//! Application layer for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md#application
//! Implements: Contract Freeze — service interfaces, DTOs, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines the application-level contracts for LLM-based code
//! generation during DAG execution. It contains:
//! - `service`: Trait definitions for LLM step operations
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
