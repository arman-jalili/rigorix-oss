//! Infrastructure layer for the Enterprise bounded context.
//!
//! HTTP client for communicating with the enterprise API.

pub mod http_client;

pub use http_client::HttpEnterpriseClient;
