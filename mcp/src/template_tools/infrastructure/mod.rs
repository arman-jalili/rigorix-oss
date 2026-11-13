//! Infrastructure layer for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#infrastructure
//! Implements: FilesystemTemplateRepository — filesystem-backed atomic TOML storage
//!
//! This module provides:
//! - FilesystemTemplateRepository: concrete filesystem-backed implementation of TemplateRepository
//! - TemplateRepositoryConfig: configuration for template filesystem paths
//!
//! # Contract (Frozen)
//!
//! - All repository methods match the TemplateRepository trait
//! - Writes are atomic: temp file → fsync → rename
//! - All error types wrap TemplateError variants
//! - Thread-safe (Send + Sync)

pub mod engine_template_parser;
pub mod filesystem_repository;
pub mod repository;

pub use engine_template_parser::FilesystemTemplateConverter;
pub use filesystem_repository::FilesystemTemplateRepository;
pub use repository::TemplateRepositoryConfig;
