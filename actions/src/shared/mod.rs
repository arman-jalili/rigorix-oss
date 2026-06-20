//! Shared infrastructure for the actions crate.
//!
//! Contains modules used by multiple action modules to avoid
//! circular dependencies.

pub mod github_client;

pub use github_client::GitHubClient;
