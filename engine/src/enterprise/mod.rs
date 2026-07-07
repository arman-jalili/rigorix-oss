//! Enterprise bounded context.
//!
//! Handles fetching policy bundles from Rigorix Enterprise, verifying
//! HMAC signatures, merging enterprise policies with local enforcement
//! config, and posting audit records.
//!
//! # Architecture
//!
//! ```text
//! enterprise/
//! ├── domain/           # Domain entities (EnterpriseConfig, PolicyBundle, errors)
//! │   ├── config.rs     # EnterpriseConfig struct
//! │   ├── bundle.rs     # PolicyBundle, PolicyBundleEntry structs
//! │   └── error.rs      # EnterpriseError enum
//! ├── application/      # Service traits, DTOs, implementations
//! │   ├── service.rs    # EnterpriseService trait
//! │   ├── dto/mod.rs    # Input/Output DTOs
//! │   └── service_impl.rs  # EnterpriseServiceImpl
//! └── infrastructure/   # HTTP client
//!     └── http_client.rs  # HttpEnterpriseClient
//! ```

pub mod application;
pub mod domain;
pub mod infrastructure;
