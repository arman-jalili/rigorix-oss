//! Rigorix — Template-driven DAG execution engine with bounded autonomy.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — configuration crate root
//! Issue: #2
//!
//! # Library Structure
//!
//! This library follows Clean Architecture with bounded contexts (DDD).
//! Each module is independently evolvable with well-defined interfaces.
//!
//! ## Phase 0: Foundation
//! - `configuration` — Config loading, multi-source merging, Secret wrapper
//! - `cancellation` — Graceful and immediate cancellation management
//! - `audit` — Execution audit trails, typed envelopes, circuit breaker
//! - `failure_classification` — Failure type classification and retry strategy selection
//! - `execution_engine` — Parallel DAG execution, retry logic, session management
//! - `error` — CoreOrchestratorError root error type with #[from] for all sub-errors
//!
//! ## The receipt, and the `portable` feature (#1591)
//!
//! The receipt (`receipt::Receipt`, `receipt::verify`, `receipt::Attestation`) is **not available to a
//! `portable` consumer.** The module needs `quality_gates`, which the portable slice removes, so it is
//! compiled out rather than stubbed: a consumer that enables `portable` and imports
//! `rigorix_engine::receipt` gets `unresolved import` at build time, naming the module — not a silent
//! absence discovered at run time.
//!
//! That boundary is asserted, not just documented: `.github/scripts/check-packaged-surface.sh` packages
//! the crate, builds a consumer against the packaged tarball twice — default features (must resolve the
//! three paths) and `features = ["portable"]` (must fail on this module) — and gates the release on
//! both answers. A host that needs to *verify* a receipt therefore depends on `rigorix-engine` with
//! default features, and a host that only runs the gate slice does not.
//!
//! ## Running a receipt (#1619)
//!
//! Verifying a declaration is half the boundary; a host also has to **submit** one. `receipt::run_receipt`
//! is that door: workspace root, `Receipt`, the steps it declares, and a `RunReceiptConfig` for the three
//! things only a host can decide — where the seal lives, whether it is signed, and which permission mode
//! the enforcer runs in. It returns the sealed `Attestation` read back through `verify_attestation`, so a
//! caller holds what any verifier would compute from the file rather than a second copy of it.
//!
//! It does not plan and it does not need a classifier: the steps are supplied by the receipt, and the
//! planning pipeline it wires (`planning::application::SuppliedStepsPlanning`) refuses every planning
//! method with the reason. Before this existed, the only worked composition in the workspace was
//! `mcp::host::build_real_engine` — a host — so a consumer could author a receipt, verify it, and have no
//! call to make. The packaged-surface check compiles that call from the tarball a host downloads.
//!
//! ## One governed assembly (#1625)
//!
//! `gate::infrastructure::assemble` is the crate's door for a host's *engine*: it reads the operator's
//! `.rigorix/` files and attaches the precondition gate, the sequence policy, the capacity ledger, the
//! effect commitment, the approval binding, the permission enforcer, the sandbox declaration and the
//! recovery recipes — in that order, because a gate attached after the dispatcher is built is not
//! consulted — then constructs the orchestrator and the seal (attestation store, receipt store, sealed
//! state directory) from them.
//!
//! The four things a host may not delegate have no `Default`: the workspace root (which operator files
//! are read), the state directory (a seal inside the workspace is one the run can rewrite, ADR-024 §6),
//! the signing key (`None` is an unsigned run), and the permission mode (an enforcer that was never
//! built is a skipped gate, #1487). What a host brings is data with no `with_*` chain: the planning
//! pipeline, and the extras that are the host's own business.
//!
//! Before this, the tree had four assemblies of one machine — the MCP host wired nine seams by hand,
//! the Action eight, the CLI used `GateSetup` and wired the rest, and `receipt::run_receipt` attached
//! no gate at all. A host that reached the last one could not tell "no findings" from "no gate",
//! because the two are the same output. `assemble` is native-only, like `receipt`, and the
//! packaged-surface check names it.
//!
//! ## Architecture
//! - `domain/` — Core domain entities and interfaces
//! - `application/` — Service traits, DTOs, factory interfaces
//! - `infrastructure/` — Repository interfaces
//! - `interfaces/` — API contracts (HTTP, events)

/// The rigorix-engine crate version, exposed for hosts (e.g. the native API
/// `rigorix.system.version`) that report the engine version without depending
/// on their own package version.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod approval;
pub mod audit;
#[cfg(not(feature = "portable"))]
pub mod backend;
#[cfg(not(feature = "portable"))]
pub mod budget_tracking;
#[cfg(not(feature = "portable"))]
pub mod cancellation;
// #1443: the module is unconditional; its `domain` layer is what the slice needs (the
// capacity arithmetic the gate consults at the moment of consequence). `application`,
// `infrastructure` and `interfaces` stay gated inside the module root.
pub mod capacity_ledger;
#[cfg(not(feature = "portable"))]
pub mod code_gen;
#[cfg(not(feature = "portable"))]
pub mod code_graph;
pub mod common;
pub mod configuration;
pub mod dag_engine;
// #1443: as with `capacity_ledger`, only the domain layer crosses into the slice.
pub mod effect_commitment;
pub mod enforcement;
pub mod error;
pub mod event_system;
#[cfg(not(feature = "portable"))]
pub mod execution_engine;
pub mod failure_classification;
#[cfg(not(feature = "portable"))]
pub mod failure_parser;
pub mod gate;

#[cfg(not(feature = "portable"))]
pub mod hooks;
pub mod identity;
#[cfg(not(feature = "portable"))]
pub mod llm_step;
pub mod observability;
#[cfg(not(feature = "portable"))]
pub mod orchestrator;
pub mod permission;
#[cfg(not(feature = "portable"))]
pub mod plan_validation;
#[cfg(not(feature = "portable"))]
pub mod planning;
pub mod policy_engine;
pub mod precondition;
#[cfg(not(feature = "portable"))]
pub mod quality_gates;
// The receipt is deliberately outside the portable slice: it needs `quality_gates`, which the slice
// removes, and `Attestation`/`Receipt` are statements a host makes and an engine signs, not something a
// wasm gate level consumes. A `portable` consumer therefore has no `receipt` module at all — the
// consequence is asserted on the packaged artifact by `.github/scripts/check-packaged-surface.sh`
// (#1591), so the boundary is a build error naming the module rather than a surprise at run time.
#[cfg(not(feature = "portable"))]
pub mod receipt;
#[cfg(not(feature = "portable"))]
pub mod recovery_recipes;
#[cfg(not(feature = "portable"))]
pub mod repo_engine;
pub mod risk_gating;
#[cfg(not(feature = "portable"))]
pub mod scored_evaluation;
pub mod sequence_policy;
#[cfg(not(feature = "portable"))]
pub mod state_persistence;
#[cfg(not(feature = "portable"))]
pub mod template_generation;
#[cfg(not(feature = "portable"))]
pub mod templates;
#[cfg(not(feature = "portable"))]
pub mod tools;
