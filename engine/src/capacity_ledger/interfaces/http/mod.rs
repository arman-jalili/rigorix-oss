//! HTTP endpoint contracts for the distributed capacity-ledger adapter.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#httpledger
//! Implements: ISSUE-CONTRACT-FREEZE — REST endpoint contracts
//! Issue: #1021
//!
//! These constants and records freeze the wire contract between `HttpLedger`
//! and the enterprise ledger service. They are framework-agnostic: the adapter
//! maps them onto `reqwest`; no server framework type leaks here.
//!
//! # Transport discipline (Frozen)
//! - Bounded response body
//! - No redirects (no redirect amplification)
//! - Explicit deadline on every call
//! - Credentials are never logged or emitted in error messages
//! - Every ambiguity maps to `LedgerError::Unavailable` (I4)

/// Path for declaring a resource.
pub const DECLARE_RESOURCE_PATH: &str = "/v1/capacity/resources";

/// Path for creating a reservation.
pub const RESERVE_PATH: &str = "/v1/capacity/reservations";

/// Path template for committing a reservation (`{id}` = reservation UUID).
pub const COMMIT_PATH: &str = "/v1/capacity/reservations/{id}/commit";

/// Path template for releasing a reservation (`{id}` = reservation UUID).
pub const RELEASE_PATH: &str = "/v1/capacity/reservations/{id}/release";

/// Path template for reading exposure (`{resource}` = resource label).
pub const EXPOSURE_PATH: &str = "/v1/capacity/exposure/{resource}";

/// A frozen endpoint contract.
///
/// `request_schema` / `response_schema` name the JSON shapes defined in
/// `application::dto` and `domain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointContract {
    /// HTTP method (`"GET"`, `"POST"`, ...).
    pub method: &'static str,
    /// Path or path template.
    pub path: &'static str,
    /// Name of the request DTO (`"-"` when there is no body).
    pub request_schema: &'static str,
    /// Name of the response DTO / domain type.
    pub response_schema: &'static str,
}

/// The complete set of endpoints the HTTP adapter may call.
pub const CAPACITY_LEDGER_ENDPOINTS: &[EndpointContract] = &[
    EndpointContract {
        method: "POST",
        path: DECLARE_RESOURCE_PATH,
        request_schema: "ResourceSpec",
        response_schema: "-",
    },
    EndpointContract {
        method: "POST",
        path: RESERVE_PATH,
        request_schema: "ReserveCapacityInput",
        response_schema: "ReserveCapacityOutput",
    },
    EndpointContract {
        method: "POST",
        path: COMMIT_PATH,
        request_schema: "CommitCapacityInput",
        response_schema: "CommitCapacityOutput",
    },
    EndpointContract {
        method: "POST",
        path: RELEASE_PATH,
        request_schema: "ReleaseCapacityInput",
        response_schema: "ReleaseCapacityOutput",
    },
    EndpointContract {
        method: "GET",
        path: EXPOSURE_PATH,
        request_schema: "-",
        response_schema: "ExposureOutput",
    },
];
