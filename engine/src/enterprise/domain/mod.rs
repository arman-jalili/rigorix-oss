//! Domain entities for the Enterprise bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise.md#domain

pub mod config;
pub mod bundle;
pub mod error;

pub use config::EnterpriseConfig;
pub use bundle::{PolicyBundle, PolicyBundleEntry};
pub use error::EnterpriseError;
