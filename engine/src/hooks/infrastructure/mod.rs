//! Infrastructure layer interfaces for the Hook System.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #410
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. The primary repository is for hook command persistence
//! and retrieval.

pub mod repository;

pub use repository::*;
