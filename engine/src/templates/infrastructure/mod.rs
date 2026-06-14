//! Infrastructure layer interfaces for the Template System bounded context.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #101
//!
//! This module defines repository interfaces that abstract template storage
//! behind traits. Implementations handle filesystem access, caching, and
//! alternate storage backends.

pub mod repository;
