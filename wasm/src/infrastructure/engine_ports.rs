//! The four ports the portable slice drops, answered from the session's fixtures.
//!
//! @canonical .pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md
//! Issue: #1383 (Phase C.3)
//!
//! Every adapter here returns *data*; none decides anything. The rule sets are parsed into the
//! engine's own types (`PreconditionConfig`, `SequencePolicyConfig`), the determination is
//! parsed as `ProviderDetermination`'s serialisation, and the one thing a browser genuinely
//! cannot do — spawn a check program — fails closed and says so.
//!
//! `Ok(None)` from a repository means **no policy configured**, which is what both traits
//! document. It never means "no rule matched" — that distinction is a rule outcome, and a
//! repository that confused the two would turn an empty file into a permit.

#![cfg(feature = "engine")]

use std::sync::Arc;

use rigorix_engine::precondition::application::dto::PreconditionCheckInput;
use rigorix_engine::precondition::application::service::PreconditionService;
use rigorix_engine::precondition::application::service_impl::PreconditionServiceImpl;
use rigorix_engine::precondition::domain::{
    Precondition, PreconditionConfig, PreconditionError, ProviderDetermination,
    ProviderObservation, ProviderSpec,
};
use rigorix_engine::precondition::infrastructure::provider_client::ProviderClient;
use rigorix_engine::precondition::infrastructure::repository::PreconditionRepository;
use rigorix_engine::precondition::infrastructure::runner::{PreconditionRun, PreconditionRunner};
use rigorix_engine::sequence_policy::application::service::SequencePolicyService;
use rigorix_engine::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
use rigorix_engine::sequence_policy::domain::{SequencePolicyConfig, SequencePolicyError};
use rigorix_engine::sequence_policy::infrastructure::repository::SequencePolicyRepository;

/// The precondition service, over fixture-backed ports.
pub fn precondition_service(transcript: &serde_json::Value) -> Arc<dyn PreconditionService> {
    let repository = Box::new(FixturePreconditionRepository::new(
        transcript.get("preconditions").cloned(),
    ));
    let runner = Box::new(TranscriptRunner {
        transcript: transcript.clone(),
    });
    let client = Box::new(TranscriptProviderClient {
        transcript: transcript.clone(),
    });

    Arc::new(PreconditionServiceImpl::new(repository, runner).with_provider_client(client))
}

/// The sequence-policy service, over fixture-backed rule data.
pub fn sequence_service(transcript: &serde_json::Value) -> Arc<dyn SequencePolicyService> {
    Arc::new(SequencePolicyServiceImpl::new(Box::new(
        FixtureSequencePolicyRepository::new(transcript.get("sequence_policy").cloned()),
    )))
}

// ── rules as data ────────────────────────────────────────────────────────────

/// A precondition set carried in the vector, not read from `.rigorix/preconditions.toml`.
///
/// A browser has no filesystem, so the operator document arrives with the session. It is
/// parsed into the engine's type — the same type the TOML repository produces — so the two
/// hosts evaluate the same rules, not two models of them.
pub struct FixturePreconditionRepository {
    document: Option<serde_json::Value>,
}

impl FixturePreconditionRepository {
    /// Wrap the document, which may be absent.
    pub const fn new(document: Option<serde_json::Value>) -> Self {
        Self { document }
    }
}

#[async_trait::async_trait]
impl PreconditionRepository for FixturePreconditionRepository {
    async fn load_config(&self) -> Result<Option<PreconditionConfig>, PreconditionError> {
        let Some(document) = &self.document else {
            // Absent means "no dispatch-time gating": the operator's decision, not an error.
            return Ok(None);
        };
        let config: PreconditionConfig =
            serde_json::from_value(document.clone()).map_err(|error| {
                // Present but unparseable is fail-closed: the engine refuses rather than
                // evaluating a partially understood rule set.
                PreconditionError::ConfigInvalid(format!(
                    "the session's preconditions document is not a valid precondition set: {error}"
                ))
            })?;
        Ok(Some(config))
    }
}

/// A sequence-policy rule set carried in the vector.
pub struct FixtureSequencePolicyRepository {
    document: Option<serde_json::Value>,
}

impl FixtureSequencePolicyRepository {
    /// Wrap the document, which may be absent.
    pub const fn new(document: Option<serde_json::Value>) -> Self {
        Self { document }
    }
}

#[async_trait::async_trait]
impl SequencePolicyRepository for FixtureSequencePolicyRepository {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        let Some(document) = &self.document else {
            return Ok(None);
        };
        let config: SequencePolicyConfig = serde_json::from_value(document.clone())
            .map_err(|error| SequencePolicyError::InvalidConfig(error.to_string()))?;
        Ok(Some(config))
    }
}

// ── no programs ──────────────────────────────────────────────────────────────

/// The runner a browser can offer: none, and it says why.
///
/// A command check is a configuration error on this host, not a check that failed. Returning
/// `Passed` would fabricate an authority decision; returning `Failed` would claim a check ran.
/// `Err(Spawn)` is fail-closed and names the boundary (ADR-012: no process in the bundle).
pub struct TranscriptRunner {
    transcript: serde_json::Value,
}

impl TranscriptRunner {
    /// Wrap the session transcript.
    pub const fn new(transcript: serde_json::Value) -> Self {
        Self { transcript }
    }
}

#[async_trait::async_trait]
impl PreconditionRunner for TranscriptRunner {
    async fn run(
        &self,
        precondition: &Precondition,
        _input: &PreconditionCheckInput,
    ) -> Result<PreconditionRun, PreconditionError> {
        Err(PreconditionError::Spawn {
            precondition_id: precondition.id.clone(),
            detail: format!(
                "this host cannot run a check program (a browser has no process to spawn); \
                 declare the check as a provider precondition instead — the session carries {} \
                 provider determination(s)",
                self.transcript
                    .get("providers")
                    .and_then(|providers| providers.as_object())
                    .map_or(0, serde_json::Map::len)
            ),
        })
    }
}

// ── determinations from the transcript ──────────────────────────────────────

/// The provider client the fixture world can answer: a determination, and its evidence.
///
/// The transcript carries the engine's own vocabulary — `"permit"`, `"deny"`, `"unknown"` —
/// because that is `ProviderDetermination`'s serialisation. A typo parses to `Unknown`, and
/// `Unknown` refuses: absence has never been authority (#1393).
pub struct TranscriptProviderClient {
    transcript: serde_json::Value,
}

impl TranscriptProviderClient {
    /// Wrap the session transcript.
    pub const fn new(transcript: serde_json::Value) -> Self {
        Self { transcript }
    }
}

impl TranscriptProviderClient {
    /// The provider's entry, keyed by `ProviderSpec::id` — the stable id the engine records
    /// in evidence.
    fn entry(&self, spec: &ProviderSpec) -> Option<&serde_json::Value> {
        self.transcript
            .get("providers")
            .and_then(|providers| providers.get(&spec.id))
    }
}

#[async_trait::async_trait]
impl ProviderClient for TranscriptProviderClient {
    async fn determine(
        &self,
        spec: &ProviderSpec,
        _input: &PreconditionCheckInput,
    ) -> Result<ProviderObservation, PreconditionError> {
        let entry = self.entry(spec);
        let raw = entry
            .and_then(|entry| entry.get("determination"))
            .and_then(serde_json::Value::as_str);

        let determination = raw
            .map(|token| {
                serde_json::from_value::<ProviderDetermination>(serde_json::Value::String(
                    token.to_string(),
                ))
                // An unreadable token is `Unknown`, never a permit.
                .unwrap_or(ProviderDetermination::Unknown)
            })
            .unwrap_or(ProviderDetermination::Unknown);

        // `observed_at` is a fact only the fixture can supply, and the engine records it in
        // evidence. A synthesised clock reading here would be a fabricated instant inside
        // signed evidence, so its absence is a configuration error — not `now()`.
        let observed_at = entry
            .and_then(|entry| entry.get("observed_at"))
            .or_else(|| self.transcript.get("observed_at"))
            .and_then(serde_json::Value::as_str)
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
            .map(|parsed| parsed.with_timezone(&chrono::Utc))
            .ok_or_else(|| {
                PreconditionError::ConfigInvalid(format!(
                    "provider '{}' has no `observed_at` — neither on its entry nor on the \
                     transcript — and the engine records when a provider was consulted; this \
                     host will not invent it",
                    spec.id
                ))
            })?;

        Ok(ProviderObservation {
            determination,
            raw_value: raw.map(str::to_string),
            observed_at,
            provider_version: entry
                .and_then(|entry| entry.get("provider_version"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            request_digest: digest_of(&serde_json::json!({
                "provider": spec.id,
                "endpoint": spec.endpoint,
                "method": format!("{:?}", spec.method).to_lowercase(),
            })),
            response_digest: digest_of(entry.unwrap_or(&serde_json::Value::Null)),
            // The endpoint is recorded redacted (scheme + host only): never a query string,
            // never a path.
            endpoint_redacted: redact_endpoint(&spec.endpoint),
        })
    }
}

/// SHA-256 over the canonical JSON of `value`, hex — the same digest form the engine uses
/// for its request/response identities.
fn digest_of(value: &serde_json::Value) -> String {
    use sha2::{Digest, Sha256};

    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Scheme and host, with everything after them dropped.
fn redact_endpoint(endpoint: &str) -> String {
    match endpoint.split_once("://") {
        Some((scheme, rest)) => {
            let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
            format!("{scheme}://{host}")
        }
        None => "<unparseable endpoint>".to_string(),
    }
}

/// Documentation guard: the observation type this module constructs, named once so a
/// signature delta from the engine shows up here rather than at a `..Default::default()`.
const _: fn(ProviderObservation) = |_observation: ProviderObservation| {};
