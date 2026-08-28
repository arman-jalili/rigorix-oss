//! Repository interfaces for the Identity Attestation bounded context.
//!
//! @canonical .pi/architecture/modules/identity.md#identityrepository
//! Implements: Contract Freeze — IdentityRepository trait
//! Issue: #700 (identity epic — contract freeze)
//!
//! Durable identity records are attached to execution state for continuity
//! and audit. The repository abstracts storage behind an interface so
//! implementation issues can choose filesystem, database, or in-memory
//! backends without touching the contract.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return `IdentityError`
//! - Raw tokens stored by reference (`token_ref`), never embedded in records

pub mod identity_repository;

pub use identity_repository::IdentityRepository;
