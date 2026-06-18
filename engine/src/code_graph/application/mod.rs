//! Application layer for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md#application
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines the application-level interfaces for code graph
//! operations. These are trait contracts with no implementation.
//!
//! # Contract (Frozen)
//! - Service traits define use cases with input/output DTOs
//! - Factory traits encapsulate service construction
//! - DTOs are serializable and carry validation documentation
//! - No implementation — only contracts

pub mod dto;
pub mod factory;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use service::*;
