//! The fixture world: what a host would observe, declared as data, consumed here.
//!
//! @canonical .pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md#2
//! @canonical .pi/architecture/modules/gate.md
//! Implements: the declaration → ports → decision path both runtimes share
//! Issue: #1414
//!
//! # Why this is engine code and not a test helper
//!
//! A conformance vector declares its world — providers with determinations, an observation
//! instant, a clock, id sequences — and until #1414 nothing in the engine read that
//! declaration. The crate's world was `wasm/tests/unit/support.rs`'s `FakeGate`, a
//! `GateEvaluator` that answered from pre-baked findings and never called `evaluate_gate`;
//! the CLI had no world at all. So two runtimes could not construct the same world, and the
//! `gate` label on a vector certified plumbing rather than a decision.
//!
//! The interpretation of a declaration is therefore a single implementation, here, in the
//! portable slice. `gate::infrastructure` is `cfg(not(feature = "portable"))`
//! ([`super::super`] module doc), so an adapter that must exist in both runtimes cannot live
//! there; [`application`](self) is in the slice, and this file adds no `cfg` gate.
//!
//! # What is declared, and what is refused
//!
//! Every value a decision needs is declared. Nothing here reads a clock, a file, a random
//! source or the environment:
//!
//! - **The observation instant** is `fixtures.providers.<id>.observed_at`, falling back to
//!   `fixtures.observed_at` (the shape `wasm/src/infrastructure/engine_ports.rs` read before
//!   this module existed). It is *not* `clock[0].instant`: the precondition finding's
//!   `checked_at` is `observation.observed_at`
//!   (`precondition/application/service_impl.rs:177`), that finding becomes
//!   `GateFinding.evidence` (`gate/application/compose.rs:81`), and the digest covers it
//!   (`gate/domain/decision.rs:64`) — so the two instants are two pins, and collapsing them
//!   would make an observation look like a decision timestamp.
//! - **The endpoint** is declared, and must be `http(s)`
//!   (`precondition/domain/provider.rs:143-155`): a vector names
//!   `https://<provider-id>.fixture.invalid`, which is a reserved, never-dialled coordinate
//!   that redacts to something obviously fake rather than to a synthesised host.
//! - **Absence is a configuration error**, never `now()`: a world with a missing instant or
//!   endpoint fails to build and names what is missing.
//!
//! # What is deliberately *not* interpreted
//!
//! `fixtures.approvals`, `fixtures.ids`, `clock` and `seed` are consumed as data. The
//! sequence port answers with **no rules** unless the declaration carries a rule set (the
//! vectors do not): an approval recorded in a fixture is history a rule *may* consult, and
//! synthesising an obligation from it would invent a requirement the operator never wrote —
//! the "no policy configured" answer is `SequencePolicyConfig::default`'s empty set, which
//! the service already treats as fail-open-absent (ADR-017). `ids.step` / `ids.envelope`
//! have no slot in [`PinnedInputs`], which pins only the instant and the execution id; they
//! are deserialised so a vector that declares them is not silently truncated.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::application::dto::PinnedInputs;
use crate::gate::application::request::GatePorts;
use crate::precondition::application::dto::PreconditionCheckInput;
use crate::precondition::application::service::PreconditionService;
use crate::precondition::application::service_impl::PreconditionServiceImpl;
use crate::precondition::domain::{
    Precondition, PreconditionConfig, PreconditionError, ProviderDetermination, ProviderMethod,
    ProviderObservation, ProviderSpec,
};
use crate::precondition::infrastructure::provider_client::ProviderClient;
use crate::precondition::infrastructure::repository::PreconditionRepository;
use crate::precondition::infrastructure::runner::{PreconditionRun, PreconditionRunner};
use crate::sequence_policy::application::service::SequencePolicyService;
use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
use crate::sequence_policy::domain::{SequencePolicyConfig, SequencePolicyError, StepPredicate};
use crate::sequence_policy::infrastructure::repository::SequencePolicyRepository;

/// The tool a declared provider precondition governs: every step the gate evaluates.
///
/// A declaration names *determinations*, not predicates — the vector's action is the step,
/// and a provider that governed only some tools would need a `match` the vectors do not
/// spell. `*` is the matcher's own glob, so this is a declaration of "all steps", not a
/// special case in the matcher.
const ALL_TOOLS: &str = "*";

// ── the declaration ─────────────────────────────────────────────────────────

/// One vector's world, as `wasm/tests/conformance/vectors/*.json` spell it.
///
/// Deserialised from the whole vector file: the reader ignores the vector's own keys
/// (`id`, `gate`, `note`, `scenario`, `expected`), so the same bytes that name a vector's
/// expectations also carry the world that produced them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureDeclaration {
    /// Provider determinations, id sequences and approvals.
    pub fixtures: Fixtures,
    /// Clock readings, **including** the decision instant at `index == 0`.
    pub clock: Vec<ClockEntry>,
    /// The determinism seed, consumed as data. The gate draws no randomness; the field
    /// exists because a vector's replay may, and a world that dropped it would let two
    /// differing runs look equal.
    pub seed: SeedDeclaration,
}

/// The `fixtures` object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fixtures {
    /// Approvals recorded before the step. History a declared rule may consult; the
    /// sequence port does not turn them into obligations of its own.
    #[serde(default)]
    pub approvals: Vec<serde_json::Value>,
    /// The determinations this world answers, keyed by provider id.
    pub providers: BTreeMap<String, ProviderDeclaration>,
    /// Declared id sequences, per role.
    #[serde(default)]
    pub ids: IdSequences,
    /// The observation instant every provider falls back to (D1). Not the decision instant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
}

/// One declared provider: what it answers, and when it was observed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderDeclaration {
    /// The engine's own vocabulary — `"passed"`, `"denied"`, `"unknown"`. Mapped through
    /// [`ProviderSpec::map_determination`], so a token outside the vocabulary is `Unknown`
    /// and refuses (absence has never been authority).
    pub determination: String,
    /// Evidence the provider returned, recorded verbatim in the observation's digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    /// When this determination was observed (D1). Falls back to [`Fixtures::observed_at`];
    /// absent from both is a [`FixtureError::MissingObservationInstant`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    /// The provider's version, when it declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_version: Option<String>,
    /// The declared endpoint (D2). Must be `http(s)`; absent is a
    /// [`FixtureError::MissingEndpoint`] naming the provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// Declared id sequences, per role. `attempt` is the execution id the gate pins.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdSequences {
    /// Execution ids. The first entry is the one [`FixtureWorld::pins`] pins.
    #[serde(default)]
    pub attempt: Vec<String>,
    /// Step ids. Deserialised so a vector's declaration is not silently truncated; the
    /// engine's [`PinnedInputs`] has no slot for them (see the module doc).
    #[serde(default)]
    pub step: Vec<String>,
    /// Envelope ids, same standing as [`IdSequences::step`].
    #[serde(default)]
    pub envelope: Vec<String>,
}

/// One clock reading. `index == 0` is the decision instant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockEntry {
    /// The instant itself.
    pub instant: DateTime<Utc>,
    /// Position in the declared sequence.
    pub index: u32,
}

/// The declared seed. Consumed, not drawn from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedDeclaration {
    /// Digest of the seed material, as declared.
    pub seed_digest: String,
    /// The draws, in order.
    #[serde(default)]
    pub draws: Vec<String>,
}

// ── the errors ──────────────────────────────────────────────────────────────

/// Why a declaration cannot become a world.
///
/// Every variant names the subject: a missing instant or endpoint is an operator-visible
/// configuration error, and "which provider" is the first thing a reader needs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FixtureError {
    /// A provider has no observation instant on its own entry or on `fixtures.observed_at`.
    #[error(
        "provider '{provider}' declares no observation instant — neither on its entry nor on \
         `fixtures.observed_at`, and the engine records when a provider was consulted; this \
         world will not invent one"
    )]
    MissingObservationInstant {
        /// The provider the instant is missing for.
        provider: String,
    },
    /// A provider declares no endpoint.
    #[error(
        "provider '{provider}' declares no endpoint — the engine records a redacted endpoint in evidence"
    )]
    MissingEndpoint {
        /// The provider the endpoint is missing for.
        provider: String,
    },
    /// A provider's endpoint is not `http(s)`.
    #[error("provider '{provider}': endpoint '{endpoint}' must be http(s)")]
    InvalidEndpoint {
        /// The provider.
        provider: String,
        /// The declared endpoint.
        endpoint: String,
    },
    /// No clock entry at index 0, so the decision instant is undeclared.
    #[error(
        "the declaration has no clock entry at index 0 — the decision instant must be declared"
    )]
    MissingDecisionInstant,
    /// No attempt id, so nothing can be pinned as the execution id.
    #[error(
        "the declaration has no `fixtures.ids.attempt` entry — the execution id is required, never minted"
    )]
    MissingExecutionId,
    /// The attempt id is not a UUID.
    #[error("`fixtures.ids.attempt[0]` is not an execution id: {value}")]
    InvalidExecutionId {
        /// The declared value.
        value: String,
    },
    /// A provider's id is not a usable DNS label for a `.invalid` endpoint.
    #[error("provider id '{provider}' is not usable in an endpoint")]
    InvalidProviderId {
        /// The declared id.
        provider: String,
    },
}

// ── the world ───────────────────────────────────────────────────────────────

/// A declaration, validated: the ports, the pins, and nothing read from the host.
#[derive(Debug, Clone)]
pub struct FixtureWorld {
    declaration: FixtureDeclaration,
    providers: BTreeMap<String, ResolvedProvider>,
    decision_instant: DateTime<Utc>,
    execution_id: Uuid,
}

/// A provider declaration with every optional default resolved.
#[derive(Debug, Clone)]
struct ResolvedProvider {
    spec: ProviderSpec,
    declaration: ProviderDeclaration,
    observed_at: DateTime<Utc>,
}

impl FixtureWorld {
    /// Validate a declaration and resolve everything it leaves optional.
    ///
    /// Total over its inputs: the only failures are the declaration's own gaps, and each
    /// names the subject. No I/O, no clock read, no `rand`.
    ///
    /// # Errors
    /// - [`FixtureError::MissingObservationInstant`] — D1
    /// - [`FixtureError::MissingEndpoint`] / [`FixtureError::InvalidEndpoint`] — D2
    /// - [`FixtureError::MissingDecisionInstant`] — no `clock[index == 0]`
    /// - [`FixtureError::MissingExecutionId`] / [`FixtureError::InvalidExecutionId`]
    pub fn from_declaration(declaration: &FixtureDeclaration) -> Result<Self, FixtureError> {
        let decision_instant = declaration
            .clock
            .iter()
            .find(|entry| entry.index == 0)
            .map(|entry| entry.instant)
            .ok_or(FixtureError::MissingDecisionInstant)?;

        let attempt = declaration
            .fixtures
            .ids
            .attempt
            .first()
            .ok_or(FixtureError::MissingExecutionId)?;
        let execution_id =
            Uuid::parse_str(attempt).map_err(|_| FixtureError::InvalidExecutionId {
                value: attempt.clone(),
            })?;

        let mut providers = BTreeMap::new();
        for (id, provider) in &declaration.fixtures.providers {
            let observed_at = provider
                .observed_at
                .or(declaration.fixtures.observed_at)
                .ok_or_else(|| FixtureError::MissingObservationInstant {
                    provider: id.clone(),
                })?;
            let endpoint = provider
                .endpoint
                .as_deref()
                .filter(|endpoint| !endpoint.trim().is_empty())
                .ok_or_else(|| FixtureError::MissingEndpoint {
                    provider: id.clone(),
                })?;
            if !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) {
                return Err(FixtureError::InvalidEndpoint {
                    provider: id.clone(),
                    endpoint: endpoint.to_string(),
                });
            }
            let spec = ProviderSpec {
                id: id.clone(),
                endpoint: endpoint.to_string(),
                method: ProviderMethod::Post,
                timeout_ms: crate::precondition::domain::DEFAULT_TIMEOUT_MS,
                credential_ref: None,
                determination_field: "determination".to_string(),
                permit_values: Vec::new(),
                deny_values: Vec::new(),
            };
            spec.validate(spec.timeout_ms)
                .map_err(|_| FixtureError::InvalidProviderId {
                    provider: id.clone(),
                })?;
            providers.insert(
                id.clone(),
                ResolvedProvider {
                    spec,
                    declaration: provider.clone(),
                    observed_at,
                },
            );
        }

        Ok(Self {
            declaration: declaration.clone(),
            providers,
            decision_instant,
            execution_id,
        })
    }

    /// The declaration this world was built from.
    #[must_use]
    pub const fn declaration(&self) -> &FixtureDeclaration {
        &self.declaration
    }

    /// The ports: the precondition service over the declared determinations, and the
    /// sequence service over the declared (empty) rule set.
    ///
    /// Both are configured — never `None` — because a world that answers from a declaration
    /// has an answer for every declared provider, and "no policy" is a rule set with no
    /// rules rather than a missing port.
    #[must_use]
    pub fn ports(&self) -> GatePorts {
        GatePorts {
            precondition: Some(self.precondition_service()),
            sequence: Some(self.sequence_service()),
            // #1455: the portable fixture world declares neither a cap nor a prior effect
            // yet, so both ports are absent — which is "no obligation declared", and produces
            // no finding. CONSEQ-2 (#1456) adds the declaration sections.
            capacity: None,
            effect: None,
        }
    }

    /// The pins: the declared decision instant and the declared execution id.
    ///
    /// The observation instant is deliberately not here — it is a provider's fact, and it
    /// enters the decision through the finding's `checked_at`.
    #[must_use]
    pub fn pins(&self) -> PinnedInputs {
        PinnedInputs::none()
            .with_timestamp(self.decision_instant)
            .with_execution_id(self.execution_id)
    }

    /// The decision instant, for a caller that renders rather than evaluates.
    #[must_use]
    pub const fn decision_instant(&self) -> DateTime<Utc> {
        self.decision_instant
    }

    fn precondition_service(&self) -> Arc<dyn PreconditionService> {
        let config = PreconditionConfig {
            fail_closed: true,
            preconditions: self
                .providers
                .values()
                .map(|provider| Precondition {
                    id: provider.spec.id.clone(),
                    r#match: StepPredicate {
                        tool: ALL_TOOLS.to_string(),
                        params: Vec::new(),
                    },
                    require_params: Vec::new(),
                    command: Vec::new(),
                    provider: Some(provider.spec.clone()),
                    timeout_ms: provider.spec.timeout_ms,
                    failure: crate::precondition::domain::FailureAction::Deny,
                    capture_output: false,
                    require_immutable_check: false,
                    authority_path: None,
                })
                .collect(),
            gating: crate::precondition::domain::GatingMode::default(),
        };
        let repository = Box::new(DeclaredPreconditions::new(config));
        let runner = Box::new(NoProcess);
        let client = Box::new(DeclaredProviders {
            providers: self.providers.clone(),
        });
        Arc::new(PreconditionServiceImpl::new(repository, runner).with_provider_client(client))
    }

    fn sequence_service(&self) -> Arc<dyn SequencePolicyService> {
        // No rules: the vectors declare determinations and approvals, not a rule set. The
        // service already treats an empty set as "no policy configured" (fail-open-absent,
        // ADR-017) — which is the honest answer, where a requirement synthesised from an
        // approval would be an obligation nobody wrote.
        Arc::new(SequencePolicyServiceImpl::new(Box::new(NoDeclaredRules(
            SequencePolicyConfig::default(),
        ))))
    }
}

// ── the ports ───────────────────────────────────────────────────────────────

/// The declared precondition set. Parsed into the engine's own type, so the two hosts
/// evaluate the same rules rather than two models of them.
struct DeclaredPreconditions {
    config: PreconditionConfig,
}

impl DeclaredPreconditions {
    const fn new(config: PreconditionConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl PreconditionRepository for DeclaredPreconditions {
    async fn load_config(&self) -> Result<Option<PreconditionConfig>, PreconditionError> {
        Ok(Some(self.config.clone()))
    }
}

/// No programs: a host that answers from a declaration has no process to spawn.
///
/// Returning `Passed` would fabricate an authority decision; returning `Failed` would claim a
/// check ran. `Spawn` is fail-closed and names the boundary (ADR-012).
struct NoProcess;

#[async_trait::async_trait]
impl PreconditionRunner for NoProcess {
    async fn run(
        &self,
        precondition: &Precondition,
        _input: &PreconditionCheckInput,
    ) -> Result<PreconditionRun, PreconditionError> {
        Err(PreconditionError::Spawn {
            precondition_id: precondition.id.clone(),
            detail: "this world answers from a declaration and has no process to spawn; \
                     declare the check as a provider precondition"
                .to_string(),
        })
    }
}

/// The declared determinations.
///
/// Total for every declared provider: `determine` is called with a [`ProviderSpec`] this
/// module built, so the lookup cannot miss. That is what keeps the decision off
/// `service_impl.rs:214`'s `Utc::now()` branch — the branch for a provider that answered
/// nothing (D4).
struct DeclaredProviders {
    providers: BTreeMap<String, ResolvedProvider>,
}

#[async_trait::async_trait]
impl ProviderClient for DeclaredProviders {
    async fn determine(
        &self,
        spec: &ProviderSpec,
        _input: &PreconditionCheckInput,
    ) -> Result<ProviderObservation, PreconditionError> {
        let Some(provider) = self.providers.get(&spec.id) else {
            // A spec this world did not build: fail closed and name it, rather than
            // answering for a provider that was never declared.
            return Err(PreconditionError::ConfigInvalid(format!(
                "provider '{}' is not declared in this world",
                spec.id
            )));
        };
        let raw = provider.declaration.determination.clone();
        Ok(ProviderObservation {
            determination: spec.map_determination(&raw),
            raw_value: Some(raw),
            observed_at: provider.observed_at,
            provider_version: provider.declaration.provider_version.clone(),
            request_digest: digest_of(&serde_json::json!({
                "provider": spec.id,
                "endpoint": spec.endpoint,
                "method": format!("{:?}", spec.method).to_lowercase(),
            })),
            response_digest: digest_of(
                provider
                    .declaration
                    .payload
                    .as_ref()
                    .unwrap_or(&serde_json::Value::Null),
            ),
            endpoint_redacted: redact_endpoint(&spec.endpoint),
        })
    }
}

/// The declared rule set — empty in every vector this issue adds.
struct NoDeclaredRules(SequencePolicyConfig);

#[async_trait::async_trait]
impl SequencePolicyRepository for NoDeclaredRules {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        Ok(Some(self.0.clone()))
    }
}

// ── digests, in the engine's form ────────────────────────────────────────────

/// SHA-256 over the canonical JSON of `value`, hex — the same form the crate used before
/// this module existed (`wasm/src/infrastructure/engine_ports.rs`), so the evidence a vector
/// records does not change when the world moves here.
fn digest_of(value: &serde_json::Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Scheme and host, everything after them dropped. Never a query string, never a path.
fn redact_endpoint(endpoint: &str) -> String {
    match endpoint.split_once("://") {
        Some((scheme, rest)) => {
            let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
            format!("{scheme}://{host}")
        }
        None => "<unparseable endpoint>".to_string(),
    }
}

/// Documentation guard: the vocabulary this module constructs, named once so a signature
/// delta from the domain shows up here rather than in a default.
const _: fn(ProviderDetermination) -> ProviderDetermination = |determination| determination;
