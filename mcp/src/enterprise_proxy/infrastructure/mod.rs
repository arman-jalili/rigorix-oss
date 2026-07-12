//! Infrastructure layer for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#infrastructure
//! Implements: EnterpriseProxyImpl — concrete EnterpriseProxy aggregate root
//!
//! This module provides:
//! - EnterpriseProxyImpl: concrete implementation of the EnterpriseProxy trait
//! - SchemaCacheRepository: interface for persisting schema cache state
//!
//! # Contract (Frozen)
//!
//! - All implementations match domain-level abstractions
//! - Thread-safe (Send + Sync)
//! - Error types wrap ProxyError variants

pub mod enterprise_proxy_impl;
pub mod repository;

pub use enterprise_proxy_impl::EnterpriseProxyImpl;
pub use repository::SchemaCacheRepository;
