//! Infrastructure interfaces for the Orchestrator bounded context.
//!
//! @canonical .pi/architecture/modules/orchestrator.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #338
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use local filesystem, database, or mock storage
//! without coupling domain logic to infrastructure.

pub mod repository;

pub use repository::*;
