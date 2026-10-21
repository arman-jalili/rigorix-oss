//! HttpProviderReceiptReader — the L3 anti-corruption adapter.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#providerreceiptreader
//! Implements: ISSUE-EFFECT-COMMITMENT-12 — provider read-back
//! Issue: #1125
//! ADR-019: commitment authority is external to the engine
//!
//! The anti-corruption adapter over a foreign payment system. It reads a
//! provider receipt and produces an [`ObservedOutcome`]; it never invents a
//! verdict.
//!
//! # Provider wire contract (Frozen)
//!
//! ```text
//! GET {base_url}/v1/receipts/{receipt_id}
//! GET {base_url}/v1/receipts?idempotency_key={intent_key}
//!
//! 200 { "receipt_id": "re_123", "status": "succeeded",
//!       "observed_at": "2026-10-21T12:00:00Z" }   // observed_at optional
//! ```
//!
//! `status` mapping:
//! - `succeeded` / `success` / `committed` -> `ObservedOutcome::committed`
//! - `failed` / `declined` / `not_committed` -> `ObservedOutcome::not_committed`
//! - `pending` / `processing` / anything else -> `Unavailable` (no verdict yet)
//!
//! The read-back digest is SHA-256 over the exact response body, so the recorded
//! evidence is tied to the payload the provider returned.
//!
//! # Transport discipline (Frozen)
//! Bounded body, no redirects, explicit deadline, credentials never logged.
//! Every ambiguity maps to [`CommitmentError::Unavailable`].

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::effect_commitment::application::config::ProviderReceiptConfig;
use crate::effect_commitment::application::receipt::ProviderReceiptReader;
use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, ObservedOutcome, OutcomeVerdict,
};

/// Provider receipts endpoint.
const RECEIPTS_PATH: &str = "/v1/receipts";

/// The subset of the provider response this adapter understands.
///
/// Unknown fields are tolerated: this is an anti-corruption boundary, not the
/// provider's schema.
#[derive(Debug, Deserialize)]
struct ProviderReceipt {
    #[serde(default)]
    receipt_id: Option<String>,
    status: String,
    #[serde(default)]
    observed_at: Option<DateTime<Utc>>,
}

/// HTTP read-back adapter for a provider's receipt API.
pub struct HttpProviderReceiptReader {
    config: ProviderReceiptConfig,
    /// `None` when the HTTP client could not be built; every call then fails
    /// closed rather than panicking.
    client: Option<reqwest::Client>,
}

impl HttpProviderReceiptReader {
    /// Create an adapter from its frozen config.
    pub fn new(config: ProviderReceiptConfig) -> Self {
        let client = build_client(&config);
        Self { config, client }
    }

    /// The active configuration.
    pub fn config(&self) -> &ProviderReceiptConfig {
        &self.config
    }

    fn unavailable(detail: impl Into<String>) -> CommitmentError {
        CommitmentError::Unavailable {
            detail: detail.into(),
        }
    }

    fn client(&self) -> Result<&reqwest::Client, CommitmentError> {
        self.client
            .as_ref()
            .ok_or_else(|| Self::unavailable("provider receipt client is not available"))
    }

    fn url(&self, receipt_id: Option<&str>) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        match receipt_id {
            Some(id) => format!("{base}{RECEIPTS_PATH}/{}", encode_path_segment(id)),
            None => format!("{base}{RECEIPTS_PATH}"),
        }
    }

    /// Fetch the bounded response and parse the provider payload.
    async fn fetch(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<(Vec<u8>, ProviderReceipt), CommitmentError> {
        let response = request
            .send()
            .await
            .map_err(|err| Self::unavailable(format!("provider receipt request failed: {err}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(Self::unavailable(format!(
                "provider receipt service returned HTTP {status}"
            )));
        }

        if let Some(length) = response.content_length()
            && length > self.config.max_response_body_bytes as u64
        {
            return Err(Self::unavailable(
                "provider receipt response body exceeds the bound",
            ));
        }

        let bytes = response.bytes().await.map_err(|err| {
            Self::unavailable(format!("failed to read provider receipt response: {err}"))
        })?;
        if bytes.len() > self.config.max_response_body_bytes {
            return Err(Self::unavailable(
                "provider receipt response body exceeds the bound",
            ));
        }

        let receipt: ProviderReceipt = serde_json::from_slice(&bytes)
            .map_err(|err| Self::unavailable(format!("malformed provider receipt: {err}")))?;
        Ok((bytes.to_vec(), receipt))
    }
}

/// Build the HTTP client with the frozen transport discipline.
fn build_client(config: &ProviderReceiptConfig) -> Option<reqwest::Client> {
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

fn verdict_for(status: &str) -> Option<OutcomeVerdict> {
    match status.trim().to_ascii_lowercase().as_str() {
        "succeeded" | "success" | "committed" => Some(OutcomeVerdict::Committed),
        "failed" | "declined" | "not_committed" | "not-committed" => {
            Some(OutcomeVerdict::NotCommitted)
        }
        // `pending`/`processing` and unknown words are not evidence of anything.
        _ => None,
    }
}

#[async_trait]
impl ProviderReceiptReader for HttpProviderReceiptReader {
    async fn observe(
        &self,
        intent: &EffectIntent,
        receipt_id: Option<&str>,
    ) -> Result<ObservedOutcome, CommitmentError> {
        let client = self.client()?;
        let request = match receipt_id {
            Some(id) => client.get(self.url(Some(id))),
            None => client
                .get(self.url(None))
                .query(&[("idempotency_key", intent.idempotency_key.as_str())]),
        };

        let (body, receipt) = self.fetch(request).await?;

        let verdict = verdict_for(&receipt.status).ok_or_else(|| {
            Self::unavailable(format!(
                "provider receipt status '{}' is not a settled outcome",
                receipt.status
            ))
        })?;

        let receipt_id = receipt
            .receipt_id
            .or_else(|| receipt_id.map(str::to_string))
            .unwrap_or_else(|| intent.idempotency_key.as_str().to_string());
        if receipt_id.trim().is_empty() {
            return Err(Self::unavailable(
                "provider receipt has no receipt id or idempotency key",
            ));
        }

        // The digest binds the evidence to the exact bytes the provider returned.
        let readback_digest = hex::encode(Sha256::digest(&body));

        Ok(ObservedOutcome {
            receipt_id,
            readback_digest,
            observed_at: receipt.observed_at.unwrap_or_else(Utc::now),
            source: self.config.provider_id.clone(),
            verdict,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode_path_segment("re_123"), "re_123");
        assert_eq!(encode_path_segment("re 1/2"), "re%201%2F2");
    }

    #[test]
    fn status_mapping_is_closed() {
        assert_eq!(verdict_for("succeeded"), Some(OutcomeVerdict::Committed));
        assert_eq!(verdict_for("SUCCESS"), Some(OutcomeVerdict::Committed));
        assert_eq!(verdict_for("failed"), Some(OutcomeVerdict::NotCommitted));
        assert_eq!(verdict_for("pending"), None);
        assert_eq!(verdict_for("unknown"), None);
    }
}
