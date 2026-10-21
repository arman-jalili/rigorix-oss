//! HttpEffectLedger — the L2 adapter.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#httpeffectledger
//! Implements: ISSUE-EFFECT-COMMITMENT-11 — opt-in distributed adapter
//! Issue: #1124
//! ADR-019: a contested store owns cross-process atomicity
//!
//! Opt-in distributed mode. Reaches a contested store that owns atomicity, with
//! the same transport discipline as the capacity ledger's HTTP adapter: bounded
//! body, no redirect amplification, explicit deadline, credentials never logged.
//! Maps every ambiguity (transport error, non-2xx, oversized body, malformed
//! JSON) to [`CommitmentError::Unavailable`] so the caller fails closed.
//!
//! # Wire contract
//! The request/response bodies are the frozen DTOs from `application::dto` and
//! the domain types; the paths come from [`interfaces::http`](super::super::interfaces::http).

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;

use crate::effect_commitment::application::dto::{
    BeginEffectInput, BeginEffectOutput, MarkAmbiguousInput, MarkAmbiguousOutput,
    QueryEffectOutput, ResolveEffectInput, ResolveEffectOutput, SettleDefiniteInput,
    SettleDefiniteOutput,
};
use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome,
    ResolverIdentity,
};
use crate::effect_commitment::interfaces::EffectCommitmentLedger;
use crate::effect_commitment::interfaces::http::{
    AMBIGUOUS_PATH, AMBIGUOUS_QUERY_PATH, BEGIN_PATH, QUERY_PATH, RESOLVE_PATH, SETTLE_PATH,
};

/// Construction parameters for [`HttpEffectLedger`].
///
/// `Debug` redacts the bearer token so configuration can be logged safely.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpEffectLedgerConfig {
    /// Base URL of the commitment service.
    pub base_url: String,

    /// Per-request deadline in milliseconds.
    pub timeout_ms: u64,

    /// Maximum accepted response body size in bytes.
    pub max_response_body_bytes: usize,

    /// Optional bearer token; never logged.
    auth_token: Option<String>,
}

impl HttpEffectLedgerConfig {
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

impl fmt::Debug for HttpEffectLedgerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpEffectLedgerConfig")
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

/// The HTTP commitment-ledger adapter.
pub struct HttpEffectLedger {
    config: HttpEffectLedgerConfig,
    /// `None` when the HTTP client could not be built; every call then fails
    /// closed rather than panicking.
    client: Option<reqwest::Client>,
}

impl HttpEffectLedger {
    /// Create an adapter from its frozen config.
    pub fn new(config: HttpEffectLedgerConfig) -> Self {
        let client = build_client(&config);
        Self { config, client }
    }

    /// The active configuration.
    pub fn config(&self) -> &HttpEffectLedgerConfig {
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
            .ok_or_else(|| Self::unavailable("HTTP effect ledger client is not available"))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }

    /// Send a request and decode a bounded JSON body, mapping every ambiguity
    /// (transport error, non-2xx, oversized body, malformed JSON) to
    /// [`CommitmentError::Unavailable`].
    async fn send_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, CommitmentError> {
        let response = request
            .send()
            .await
            .map_err(|err| Self::unavailable(format!("effect ledger request failed: {err}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(Self::unavailable(format!(
                "effect ledger service returned HTTP {status}"
            )));
        }

        if let Some(length) = response.content_length()
            && length > self.config.max_response_body_bytes as u64
        {
            return Err(Self::unavailable(
                "effect ledger response body exceeds the bound",
            ));
        }

        let bytes = response.bytes().await.map_err(|err| {
            Self::unavailable(format!("failed to read effect ledger response: {err}"))
        })?;
        if bytes.len() > self.config.max_response_body_bytes {
            return Err(Self::unavailable(
                "effect ledger response body exceeds the bound",
            ));
        }

        serde_json::from_slice(&bytes)
            .map_err(|err| Self::unavailable(format!("malformed effect ledger response: {err}")))
    }
}

/// Build the HTTP client with the frozen transport discipline.
fn build_client(config: &HttpEffectLedgerConfig) -> Option<reqwest::Client> {
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
impl EffectCommitmentLedger for HttpEffectLedger {
    async fn begin(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        let body = BeginEffectInput { intent };
        let request = self.client()?.post(self.url(BEGIN_PATH)).json(&body);
        let output: BeginEffectOutput = self.send_json(request).await?;
        Ok(output.record)
    }

    async fn settle_definite(
        &self,
        record_id: EffectRecordId,
        outcome: ObservedOutcome,
    ) -> Result<EffectRecord, CommitmentError> {
        let path = SETTLE_PATH.replace("{id}", &encode_path_segment(&record_id.to_string()));
        let body = SettleDefiniteInput { record_id, outcome };
        let request = self.client()?.post(self.url(&path)).json(&body);
        let output: SettleDefiniteOutput = self.send_json(request).await?;
        Ok(output.record)
    }

    async fn mark_ambiguous(
        &self,
        record_id: EffectRecordId,
        reason: String,
    ) -> Result<EffectRecord, CommitmentError> {
        let path = AMBIGUOUS_PATH.replace("{id}", &encode_path_segment(&record_id.to_string()));
        let body = MarkAmbiguousInput { record_id, reason };
        let request = self.client()?.post(self.url(&path)).json(&body);
        let output: MarkAmbiguousOutput = self.send_json(request).await?;
        Ok(output.record)
    }

    async fn resolve(
        &self,
        record_id: EffectRecordId,
        outcome: ObservedOutcome,
        resolver: ResolverIdentity,
    ) -> Result<EffectRecord, CommitmentError> {
        let path = RESOLVE_PATH.replace("{id}", &encode_path_segment(&record_id.to_string()));
        let body = ResolveEffectInput {
            record_id,
            outcome,
            resolver,
        };
        let request = self.client()?.post(self.url(&path)).json(&body);
        let output: ResolveEffectOutput = self.send_json(request).await?;
        Ok(output.record)
    }

    async fn query(&self, query: EffectQuery) -> Result<Vec<EffectRecord>, CommitmentError> {
        let client = self.client()?;
        let request = match &query {
            EffectQuery::ById { record_id } => client
                .get(self.url(QUERY_PATH))
                .query(&[("record_id", record_id.to_string())]),
            EffectQuery::ByDag { dag_id } => client
                .get(self.url(QUERY_PATH))
                .query(&[("dag_id", dag_id.to_string())]),
            EffectQuery::Ambiguous { dag_id } => {
                let request = client.get(self.url(AMBIGUOUS_QUERY_PATH));
                match dag_id {
                    Some(dag_id) => request.query(&[("dag_id", dag_id.to_string())]),
                    None => request,
                }
            }
        };
        let output: QueryEffectOutput = self.send_json(request).await?;
        Ok(output.records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode_path_segment("record"), "record");
        assert_eq!(encode_path_segment("a b/c"), "a%20b%2Fc");
        assert_eq!(encode_path_segment("a?b=c"), "a%3Fb%3Dc");
    }

    #[test]
    fn config_debug_redacts_the_token() {
        let config =
            HttpEffectLedgerConfig::new("https://effects", 1_000, 1_024, Some("secret".into()));
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("secret"));
        assert!(rendered.contains("<redacted>"));
    }
}
