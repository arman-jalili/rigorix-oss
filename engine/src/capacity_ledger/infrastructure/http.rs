//! HttpLedger adapter.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#httpledger
//! Implements: ISSUE-CAPACITY-LEDGER-10 — opt-in distributed adapter
//! Issue: #1031
//! ADR-018: distributed capacity ledger
//!
//! Opt-in distributed mode. Reaches the enterprise ledger service over HTTP with
//! the same transport discipline as the precondition provider client: bounded
//! body, no redirect amplification, explicit deadline, credentials never logged.
//! Maps every ambiguity to `Unavailable` so the caller fails closed (I4).
//!
//! # Wire contract
//! The request/response bodies are the frozen DTOs from `application::dto` and
//! the domain types; the paths come from [`interfaces::http`](super::super::interfaces::http).

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;

use crate::capacity_ledger::application::dto::{
    CommitCapacityInput, CommitCapacityOutput, ExposureOutput, ReleaseCapacityInput,
    ReleaseCapacityOutput, ReserveCapacityInput, ReserveCapacityOutput,
};
use crate::capacity_ledger::domain::{
    Exposure, Grant, IdempotencyKey, LedgerError, Reservation, ResourceId, ResourceSpec,
};
use crate::capacity_ledger::interfaces::CapacityLedger;
use crate::capacity_ledger::interfaces::http::{
    COMMIT_PATH, DECLARE_RESOURCE_PATH, EXPOSURE_PATH, RELEASE_PATH, RESERVE_PATH,
};

/// Construction parameters for [`HttpLedger`].
///
/// `Debug` redacts the bearer token so configuration can be logged safely.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpLedgerConfig {
    /// Base URL of the ledger service.
    pub base_url: String,

    /// Per-request deadline in milliseconds.
    pub timeout_ms: u64,

    /// Maximum accepted response body size in bytes.
    pub max_response_body_bytes: usize,

    /// Optional bearer token; never logged.
    auth_token: Option<String>,
}

impl HttpLedgerConfig {
    /// Construct a config.
    pub fn new(
        base_url: impl Into<String>,
        timeout_ms: u64,
        max_response_body_bytes: usize,
        auth_token: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            timeout_ms,
            max_response_body_bytes,
            auth_token,
        }
    }

    /// The bearer token, if configured.
    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }
}

impl fmt::Debug for HttpLedgerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpLedgerConfig")
            .field("base_url", &self.base_url)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_response_body_bytes", &self.max_response_body_bytes)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// The HTTP capacity-ledger adapter.
pub struct HttpLedger {
    config: HttpLedgerConfig,
    /// `None` when the HTTP client could not be built; every call then fails
    /// closed rather than panicking.
    client: Option<reqwest::Client>,
}

impl HttpLedger {
    /// Create an adapter from its frozen config.
    pub fn new(config: HttpLedgerConfig) -> Self {
        let client = build_client(&config);
        Self { config, client }
    }

    /// The active configuration.
    pub fn config(&self) -> &HttpLedgerConfig {
        &self.config
    }

    fn unavailable(detail: impl Into<String>) -> LedgerError {
        LedgerError::Unavailable {
            detail: detail.into(),
        }
    }

    fn client(&self) -> Result<&reqwest::Client, LedgerError> {
        self.client
            .as_ref()
            .ok_or_else(|| Self::unavailable("HTTP ledger client is not available"))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }

    /// Send a request and decode a bounded JSON body, mapping every ambiguity
    /// (transport error, non-2xx, oversized body, malformed JSON) to
    /// [`LedgerError::Unavailable`].
    async fn send_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, LedgerError> {
        let response = request
            .send()
            .await
            .map_err(|err| Self::unavailable(format!("ledger request failed: {err}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(Self::unavailable(format!(
                "ledger service returned HTTP {status}"
            )));
        }

        if let Some(length) = response.content_length()
            && length > self.config.max_response_body_bytes as u64
        {
            return Err(Self::unavailable("ledger response body exceeds the bound"));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|err| Self::unavailable(format!("failed to read ledger response: {err}")))?;
        if bytes.len() > self.config.max_response_body_bytes {
            return Err(Self::unavailable("ledger response body exceeds the bound"));
        }

        serde_json::from_slice(&bytes)
            .map_err(|err| Self::unavailable(format!("malformed ledger response: {err}")))
    }

    /// Send a request that returns no useful body.
    async fn send_empty(&self, request: reqwest::RequestBuilder) -> Result<(), LedgerError> {
        let response = request
            .send()
            .await
            .map_err(|err| Self::unavailable(format!("ledger request failed: {err}")))?;
        if !response.status().is_success() {
            return Err(Self::unavailable(format!(
                "ledger service returned HTTP {}",
                response.status()
            )));
        }
        Ok(())
    }

    fn grant_from(reservation: &Reservation) -> Grant {
        Grant::issue(
            reservation.id,
            reservation.resource.clone(),
            reservation.amount,
            reservation.lease,
            reservation.idempotency_key.clone(),
        )
    }
}

/// Build the HTTP client with the frozen transport discipline.
fn build_client(config: &HttpLedgerConfig) -> Option<reqwest::Client> {
    let mut headers = HeaderMap::new();
    if let Some(token) = config.auth_token()
        && let Ok(mut value) = HeaderValue::from_str(&format!("Bearer {token}"))
    {
        value.set_sensitive(true);
        headers.insert(AUTHORIZATION, value);
    }

    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(config.timeout_ms))
        .default_headers(headers)
        .build()
        .ok()
}

/// Percent-encode a path segment (RFC 3986 unreserved characters are kept).
fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[async_trait]
impl CapacityLedger for HttpLedger {
    async fn declare_resource(&self, spec: ResourceSpec) -> Result<(), LedgerError> {
        let request = self
            .client()?
            .post(self.url(DECLARE_RESOURCE_PATH))
            .json(&spec);
        self.send_empty(request).await
    }

    async fn reserve(
        &self,
        resource: &ResourceId,
        amount: u64,
        key: IdempotencyKey,
    ) -> Result<Grant, LedgerError> {
        let body = ReserveCapacityInput {
            resource: resource.clone(),
            amount,
            idempotency_key: key,
            requested_by: None,
        };
        let request = self.client()?.post(self.url(RESERVE_PATH)).json(&body);
        let output: ReserveCapacityOutput = self.send_json(request).await?;
        Ok(Self::grant_from(&output.reservation))
    }

    async fn commit(&self, grant: Grant) -> Result<Reservation, LedgerError> {
        let (id, _resource, _amount, _lease, _key) = grant.into_parts();
        let path = COMMIT_PATH.replace("{id}", &encode_path_segment(&id.to_string()));
        let body = CommitCapacityInput { reservation_id: id };
        let request = self.client()?.post(self.url(&path)).json(&body);
        let output: CommitCapacityOutput = self.send_json(request).await?;
        Ok(output.reservation)
    }

    async fn release(&self, grant: Grant) -> Result<Reservation, LedgerError> {
        let (id, _resource, _amount, _lease, _key) = grant.into_parts();
        let path = RELEASE_PATH.replace("{id}", &encode_path_segment(&id.to_string()));
        let body = ReleaseCapacityInput {
            reservation_id: id,
            reason: None,
        };
        let request = self.client()?.post(self.url(&path)).json(&body);
        let output: ReleaseCapacityOutput = self.send_json(request).await?;
        Ok(output.reservation)
    }

    async fn exposure(&self, resource: &ResourceId) -> Result<Exposure, LedgerError> {
        let path = EXPOSURE_PATH.replace("{resource}", &encode_path_segment(resource.as_str()));
        let request = self.client()?.get(self.url(&path));
        let output: ExposureOutput = self.send_json(request).await?;
        Ok(output.exposure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode_path_segment("gpu"), "gpu");
        assert_eq!(encode_path_segment("gpu pool/v1"), "gpu%20pool%2Fv1");
        assert_eq!(encode_path_segment("a?b=c"), "a%3Fb%3Dc");
    }

    #[test]
    fn config_debug_redacts_the_token() {
        let config = HttpLedgerConfig::new("https://ledger", 1_000, 1_024, Some("secret".into()));
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("secret"));
        assert!(rendered.contains("<redacted>"));
    }
}
