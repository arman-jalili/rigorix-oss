//! Configuration module — CLI-side config loading and merging.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Multi-source configuration loading with layered merging.
//! CLI flags → env vars → rigorix.toml → engine defaults.

pub mod application;
pub mod domain;
pub mod infrastructure;
