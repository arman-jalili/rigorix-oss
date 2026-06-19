//! Infrastructure layer for the Plan Validation bounded context.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: Contract Freeze — ValidationReportRepository, ValidatedTemplateRepository
//! Issue: issue-contract-freeze
//!
//! The infrastructure layer defines repository interfaces that abstract
//! storage behind traits. Concrete implementations (filesystem, database,
//! S3) are provided by implementors.
//!
//! # Architecture
//!
//! ```text
//! infrastructure/
//! ├── mod.rs                   # Module root
//! └── repository/              # Repository interfaces
//!     └── mod.rs               # ValidationReportRepository, ValidatedTemplateRepository
//! ```

pub mod repository;
