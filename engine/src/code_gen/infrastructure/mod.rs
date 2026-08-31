//! Infrastructure layer interfaces for the Code Generation Pipeline.
//!
//! @canonical .pi/architecture/modules/code-generation.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #424
//!
//! This module defines repository interfaces that abstract data access
//! behind traits.

pub mod in_memory_code_gen_event_repository;
pub mod repository;

pub use in_memory_code_gen_event_repository::InMemoryCodeGenEventRepository;

pub use repository::*;
