//! Domain services — pure logic that does not belong on the aggregate.
//!
//! @canonical .pi/architecture/modules/portable-session.md#domain-services--application-services--infrastructure-services
//! Implements: RefusalVocabulary, EvidenceProjection, ArtifactStamp,
//!   ArtifactContentPolicy and their shipped implementations
//! Issue: #1294 (contract freeze), #1296 (domain layer)
//!
//! Three of these are pure projections with no I/O; the fourth is a policy the export
//! path consults. They are traits because each has (or will have) more than one
//! implementation: business wording differs per locale/tenant, the projection differs
//! between the engine binding and test doubles, and the content policy differs between
//! demo and partner builds.

mod artifact_content_policy;
mod artifact_stamp;
mod business_wording;
mod engine_artifact_stamp;
mod evidence_projection;
mod finding_projection;
mod refusal_vocabulary;

pub use artifact_content_policy::{ArtifactContentPolicy, ContentRule};
pub use artifact_stamp::ArtifactStamp;
pub use business_wording::BusinessWording;
pub use engine_artifact_stamp::EngineArtifactStamp;
pub use evidence_projection::EvidenceProjection;
pub use finding_projection::EngineFindingsProjection;
pub use refusal_vocabulary::RefusalVocabulary;
