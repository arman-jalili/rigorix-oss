//! Wire DTOs — the frozen JSON shapes.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! Implements: Contract Freeze — OpenSessionDto, SessionHandleDto, GatedActionDto,
//!   RunOutcomeDto, StepOutcomeDto, RefusalDto, ArtifactSetDto
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - Every DTO is `#[serde(deny_unknown_fields)]`: an unknown field is a refusal, never
//!   an ignored one (criterion 15)
//! - No DTO has an optional field that changes a decision. Optional fields exist only
//!   for data that is genuinely absent (a reason on a refusal)
//! - No DTO carries a verdict, an `allow`, an `override` or an `approve` flag (FR-009)
//! - Field names are `snake_case`; the host sees exactly these keys

mod artifact_set;
mod attempt;
mod open_session;
mod outcome;
mod refusal;

pub use artifact_set::ArtifactSetDto;
pub use attempt::{GatedActionDto, SessionHandleDto};
pub use open_session::{ClockReadingDto, OpenSessionDto, SeedDto};
pub use outcome::{EvidenceRefDto, RunOutcomeDto, StepOutcomeDto};
pub use refusal::RefusalDto;
