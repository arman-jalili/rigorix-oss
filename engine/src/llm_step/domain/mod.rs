//! Domain entities and interfaces for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md#domain
//! Implements: Contract Freeze — domain entities LlmGenerateNode, LlmStepContext
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types for LLM-based code generation
//! during DAG execution. These are pure domain objects with no framework
//! dependencies. They serve as the frozen contract that all implementation
//! must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

pub mod error;
pub mod event;
pub mod generate_node;
pub mod step_context;

pub use error::*;
pub use generate_node::*;
pub use step_context::*;
