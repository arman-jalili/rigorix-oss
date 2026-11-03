//! Domain services — pure logic that does not belong on the aggregate.
//!
//! @canonical .pi/architecture/modules/portable-session.md#domain-services--application-services--infrastructure-services
//! Implements: Contract Freeze — RefusalVocabulary, EvidenceProjection, ArtifactStamp,
//!   ArtifactContentPolicy
//! Issue: #1294 (contract freeze)
//!
//! Three of these are pure projections with no I/O; the fourth is a policy the export
//! path consults. They are traits because each has (or will have) more than one
//! implementation: business wording differs per locale/tenant, the projection differs
//! between the engine binding and test doubles, and the content policy differs between
//! demo and partner builds.

mod artifact_content_policy;
mod artifact_stamp;
mod evidence_projection;
mod refusal_vocabulary;

pub use artifact_content_policy::{ArtifactContentPolicy, ContentRule};
pub use artifact_stamp::ArtifactStamp;
pub use evidence_projection::EvidenceProjection;
pub use refusal_vocabulary::RefusalVocabulary;
