//! Infrastructure layer interfaces for the Recovery Recipes bounded context.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #438 (recovery-recipes epic)
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.

pub mod in_memory_repository;
pub mod repository;

pub use in_memory_repository::InMemoryRecipeRepository;
pub use repository::RecoveryRecipeRepository;
