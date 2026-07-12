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

pub mod filesystem_repository;
pub mod repository;
pub mod template_converter_impl;

pub use filesystem_repository::FilesystemTemplateRepository;
pub use repository::TemplateRepositoryConfig;
pub use template_converter_impl::FilesystemTemplateConverter;
