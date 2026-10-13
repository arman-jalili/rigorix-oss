//! ProviderClient — the HTTP transport for an operator-configured external
//! determination service (ISSUE-PRECONDITION-PROVIDERS).
//!
//! @canonical .pi/architecture/modules/precondition.md
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: ISSUE-PRECONDITION-PROVIDERS (infrastructure)
//!
//! The client is deliberately conservative — it treats the provider as an
//! **untrusted input source**:
//!
//! - **bounded body** — the cap is enforced *while reading*, so an oversized
//!   response is refused without first being buffered in full (see
//!   `MAX_RESPONSE_BYTES`);
//! - **deadline** — the operator's `timeout_ms` is applied per request;
//! - **no redirects** — `Policy::none()`; a provider cannot bounce the engine to
//!   an arbitrary host;
//! - **no credential in evidence** — the credential value is read from the
//!   environment named by `credential_ref` and sent as a bearer token; it never
//!   appears in an error, a summary, or the signed record;
//! - **fail closed** — unreachable, timeout, non-2xx, unparseable, and unknown
//!   vocabulary all map to [`ProviderDetermination::Unknown`] (or an error),
//!   which refuses.

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;

use crate::precondition::application::dto::PreconditionCheckInput;
use crate::precondition::domain::{
    PreconditionError, ProviderDetermination, ProviderMethod, ProviderObservation, ProviderSpec,
};
use crate::precondition::infrastructure::sha256_hex;

/// Maximum response body the client will read (bytes). A larger body is refused
/// (fail closed) — a determination service must answer tersely.
///
/// Enforced *during* the read, not after it: [`reqwest::Response::bytes`] would
/// buffer an arbitrarily large body before the cap could be applied, which would
/// bound the recorded evidence but not the engine's memory.
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Consult an external determination service.
#[async_trait]
pub trait ProviderClient: Send + Sync {
    /// Ask the provider for a determination on `input`.
    ///
    /// # Errors
    /// - `PreconditionError::Provider` — the provider could not be consulted
    ///   (client init, unset credential, unreachable, timeout, unreadable body,
    ///   oversized body). Fail closed; refuses the step.
    ///
    /// A provider that *answers* — including with a non-2xx status or an
    /// unknown vocabulary — returns `Ok` with
    /// [`ProviderDetermination::Unknown`]; the caller refuses either way, but
    /// distinguishes "consulted and indeterminate" from "never reached".
    async fn determine(
        &self,
        spec: &ProviderSpec,
        input: &PreconditionCheckInput,
    ) -> Result<ProviderObservation, PreconditionError>;
}

/// Production client over `reqwest` (rustls, redirects disabled).
#[derive(Debug, Default, Clone, Copy)]
pub struct HttpProviderClient;

impl HttpProviderClient {
    /// Redaction-safe endpoint reference for the signed record: the scheme and
    /// host stay (operability), the rest is replaced by a one-way hash of the
    /// whole endpoint. No query string, no path, no credential.
    pub(crate) fn redact_endpoint(endpoint: &str) -> String {
        match endpoint.split_once("://") {
            Some((scheme, rest)) => {
                let host = rest.split(['/', '?', '#']).next().unwrap_or("");
                format!(
                    "{scheme}://{host}/<path {}>",
                    sha256_hex(endpoint.as_bytes())
                )
            }
            None => format!("<endpoint {}>", sha256_hex(endpoint.as_bytes())),
        }
    }
}

fn base64url(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[async_trait]
impl ProviderClient for HttpProviderClient {
    async fn determine(
        &self,
        spec: &ProviderSpec,
        input: &PreconditionCheckInput,
    ) -> Result<ProviderObservation, PreconditionError> {
        let endpoint = Self::redact_endpoint(&spec.endpoint);
        let fail = |detail: String| PreconditionError::Provider {
            precondition_id: spec.id.clone(),
            detail,
        };

        // Credential value: read the operator-held secret from the environment
        // named by `credential_ref`. It is never logged and never returned.
        let credential = match &spec.credential_ref {
            Some(name) => match std::env::var(name) {
                Ok(value) if !value.is_empty() => Some(value),
                _ => {
                    return Err(fail(format!(
                        "credential '{name}' is not set in the environment"
                    )));
                }
            },
            None => None,
        };

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| fail(format!("http client init failed: {error}")))?;

        let body = serde_json::to_vec(input)
            .map_err(|error| fail(format!("request encode failed: {error}")))?;
        let request_digest = sha256_hex(&body);

        let mut request = match spec.method {
            ProviderMethod::Post => client
                .post(&spec.endpoint)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone()),
            ProviderMethod::Get => client
                .get(&spec.endpoint)
                .query(&[("input", base64url(&body))]),
        };
        request = request.timeout(Duration::from_millis(spec.timeout_ms));
        if let Some(value) = credential {
            request = request.bearer_auth(value);
        }

        let observed_at = Utc::now();

        // Never forward the raw transport error: a reqwest error string can
        // carry the URL (and its query). The engine gets a fixed, redacted
        // reason; the operator gets the full detail in the local log.
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(
                    precondition = %spec.id,
                    provider = %spec.id,
                    %error,
                    "precondition provider unreachable"
                );
                return Err(fail(
                    "provider unreachable or timed out (see the operator log)".to_string(),
                ));
            }
        };

        let status = response.status();

        // Enforce the cap DURING the read, not after it. `Response::bytes()`
        // buffers the whole body first, so a hostile or misconfigured provider
        // could make the engine allocate an unbounded buffer before we ever got
        // to refuse it — the cap would then bound the *evidence*, not the
        // *memory*. A provider is an untrusted input source, so bound what we
        // allocate. `content_length` is a cheap early exit when the provider
        // declares a size; the streaming loop is the actual guarantee, because
        // a chunked response omits it.
        if let Some(declared) = response.content_length()
            && declared > MAX_RESPONSE_BYTES as u64
        {
            return Err(fail(format!(
                "provider response declares {declared} bytes, over the {MAX_RESPONSE_BYTES}-byte cap"
            )));
        }
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                        return Err(fail(format!(
                            "provider response exceeds the {MAX_RESPONSE_BYTES}-byte cap"
                        )));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(provider = %spec.id, %error, "provider body unreadable");
                    return Err(fail("provider response could not be read".to_string()));
                }
            }
        }
        let response_digest = sha256_hex(&bytes);

        // Non-2xx → indeterminate, but the bytes were bound (Unknown).
        if !status.is_success() {
            return Ok(ProviderObservation {
                determination: ProviderDetermination::Unknown,
                raw_value: None,
                observed_at,
                provider_version: None,
                request_digest,
                response_digest,
                endpoint_redacted: endpoint,
            });
        }

        // Parse + map. Unparseable / missing / unknown vocabulary → Unknown:
        // unknown must never become authority.
        let parsed: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        let raw_value = parsed
            .get(&spec.determination_field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let determination = raw_value
            .as_deref()
            .map(|raw| spec.map_determination(raw))
            .unwrap_or(ProviderDetermination::Unknown);
        let provider_version = parsed
            .get("provider_version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        Ok(ProviderObservation {
            determination,
            raw_value,
            observed_at,
            provider_version,
            request_digest,
            response_digest,
            endpoint_redacted: endpoint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_redaction_keeps_scheme_and_host_only() {
        let redacted = HttpProviderClient::redact_endpoint(
            "https://determine.example/v1/standing?tenant=acme",
        );
        assert!(redacted.starts_with("https://determine.example/<path sha256:"));
        assert!(!redacted.contains("tenant"));
        assert!(!redacted.contains("/v1/"));
    }

    #[test]
    fn endpoint_redaction_handles_opaque_input() {
        let redacted = HttpProviderClient::redact_endpoint("not-a-url");
        assert!(redacted.starts_with("<endpoint sha256:"));
    }
}
