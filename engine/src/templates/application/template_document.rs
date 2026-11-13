//! The template **document** schema: one artifact, two dialects, one parser (#1529).
//!
//! @canonical .pi/architecture/modules/template-system.md#parser
//! Implements: ADR-002 (TOML) with ADR-024 §4 D3's version policy; `SchemaVersion`
//!
//! Before this module the word "template" named two artifacts with two parsers. The engine's
//! registry held `Template { nodes }`, parsed from `[[nodes]]` by `toml::from_str`. Every demo
//! document held `[[steps]]` with `tool` and `[steps.parameters]`, parsed by a converter in the
//! *host* crate into a different type. Neither carried a version, so a field added to one was
//! ignored by the other `serde(default)` — a drift that reads as compatibility.
//!
//! # One value, two dialects
//!
//! `[[steps]]` is the *linear case* of `[[nodes]]`: same node, no `depends_on`, and the invocation
//! written as `tool` + `parameters` instead of a typed `action`. The dialect is therefore a spelling
//! of the same document, and both parse into one `Template`. A second node type would recreate the
//! problem; a second *field* would not (the field would be empty in one of them).
//!
//! # The version rule, and why the dialect decides for the legacy form
//!
//! `schema_version` is required, one integer, and inside the canonical bytes. But the legacy
//! `[[steps]]` dialect predates the field, and its documents are what the demo repositories ship —
//! they must keep working unchanged. So the rule is by dialect, not by field default:
//!
//! | document | version in it | result |
//! |---|---|---|
//! | `[[nodes]]` | absent | `MissingSchemaVersion` |
//! | `[[nodes]]` | `1` | parses, version 1 |
//! | `[[steps]]` | absent | parses as version 1 — **the legacy dialect is v1 by definition**, and the report says so |
//! | either | unknown integer | `UnsupportedSchemaVersion`, naming the supported set |
//! | either | both `[[nodes]]` and `[[steps]]` | `AmbiguousDialect` |
//!
//! This is not the silent `serde(default)` drift the issue forbids: the rule is typed, named in the
//! report, and decided by which dialect the document was written in — not by a per-field default
//! that would also accept a `[[nodes]]` document with no version.

use crate::templates::domain::{
    SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSIONS, Template, TemplateAction, TemplateError,
    TemplateNode,
};
use serde::Deserialize;

/// Which spelling the document used. Recorded, not inferred twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `[[nodes]]` — the engine's own form, version required.
    Nodes,
    /// `[[steps]]` — the demo form, the linear case, version 1 by definition.
    Steps,
}

impl Dialect {
    /// The dialect's name, for a report that has to say what it read.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nodes => "nodes",
            Self::Steps => "steps",
        }
    }
}

/// What a document is: the value plus how it was spelled.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateDocument {
    /// The one schema's value.
    pub template: Template,
    /// The spelling it arrived in.
    pub dialect: Dialect,
}

// ---------------------------------------------------------------------------
// Wire types — the only place TOML's shape is known
// ---------------------------------------------------------------------------

/// The document as written. Every field is optional here and required there: the refusal for a
/// missing one has to be the domain's, with a file and a line, not serde's "missing field".
#[derive(Debug, Default, Deserialize)]
struct RawDocument {
    schema_version: Option<u16>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    parameters: Vec<crate::templates::domain::ParameterDef>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    author: Option<String>,
    /// The engine's spelling.
    #[serde(default)]
    nodes: Option<Vec<RawNode>>,
    /// The demo spelling.
    #[serde(default)]
    steps: Option<Vec<RawNode>>,
}

/// One node, before the invocation has been resolved to an action.
#[derive(Debug, Default, Deserialize)]
struct RawNode {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    action: Option<TemplateAction>,
    /// The demo form's invocation: a tool name…
    #[serde(default)]
    tool: Option<String>,
    /// …and its arguments.
    #[serde(default)]
    parameters: Option<serde_json::Value>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    retry: crate::templates::domain::RetryConfig,
    #[serde(default)]
    validate: Vec<crate::templates::domain::ValidationRule>,
    #[serde(default)]
    requires_approval: bool,
    #[serde(default)]
    require_identity: bool,
    #[serde(default)]
    intent: Option<String>,
}

/// Parse a document into the one value, refusing the three ways its version can be wrong.
///
/// `source` is the path when the caller has one; it appears in every refusal, because a refusal
/// that does not say which file it is about is a refusal the author has to reproduce by hand.
///
/// # Errors
/// `TemplateError::MissingSchemaVersion`, `UnsupportedSchemaVersion`, `AmbiguousDialect`,
/// `InvalidInvocation`, or `Parse` with the line serde reported.
pub fn parse_document(
    toml_content: &str,
    source: Option<&str>,
) -> Result<TemplateDocument, TemplateError> {
    let raw: RawDocument = toml::from_str(toml_content).map_err(|e| TemplateError::Parse {
        detail: e.message().to_string(),
        line: Some(u32::try_from(e.span().map_or(0, |s| s.start)).unwrap_or(0)),
        path: source.map(str::to_string),
    })?;

    let dialect = match (raw.nodes.is_some(), raw.steps.is_some()) {
        (true, false) => Dialect::Nodes,
        (false, true) => Dialect::Steps,
        (true, true) => {
            return Err(TemplateError::AmbiguousDialect {
                path: source.map(str::to_string),
            });
        }
        // A document with no nodes at all is a parse result, not a version refusal: the sequence
        // validator reports it. Treated as the node dialect so the version rule still applies.
        (false, false) => Dialect::Nodes,
    };

    let schema_version = match raw.schema_version {
        Some(version) if SUPPORTED_SCHEMA_VERSIONS.contains(&version) => version,
        Some(version) => {
            return Err(TemplateError::UnsupportedSchemaVersion {
                found: version,
                supported: SUPPORTED_SCHEMA_VERSIONS.to_vec(),
                path: source.map(str::to_string),
            });
        }
        None => match dialect {
            // The legacy dialect is v1 by definition — and the value records it, so a caller never
            // has to ask which reading was used.
            Dialect::Steps => SCHEMA_VERSION,
            Dialect::Nodes => {
                return Err(TemplateError::MissingSchemaVersion {
                    path: source.map(str::to_string),
                });
            }
        },
    };

    let name = raw.name.unwrap_or_default();
    // An id is derived only for the legacy dialect, which names its document and not its steps'
    // parent. In the node dialect a missing id stays missing, so the validator reports it — the
    // derivation must not silence an error that exists today.
    let id = match (raw.id, dialect) {
        (Some(id), _) => id,
        (None, Dialect::Steps) => slug_from(&name),
        (None, Dialect::Nodes) => String::new(),
    };

    let raw_nodes = raw.nodes.or(raw.steps).unwrap_or_default();
    let mut nodes = Vec::with_capacity(raw_nodes.len());
    for (index, node) in raw_nodes.into_iter().enumerate() {
        nodes.push(resolve_node(node, index, source)?);
    }

    Ok(TemplateDocument {
        template: Template {
            schema_version,
            id,
            name,
            description: raw.description.unwrap_or_default(),
            version: raw.version.unwrap_or_default(),
            parameters: raw.parameters,
            nodes,
            tags: raw.tags,
            category: raw.category,
            author: raw.author,
        },
        dialect,
    })
}

/// Resolve one node's invocation: an action, or a named tool call, but not both and not neither.
fn resolve_node(
    node: RawNode,
    index: usize,
    source: Option<&str>,
) -> Result<TemplateNode, TemplateError> {
    let name = node.name.clone().unwrap_or_else(|| format!("step-{index}"));
    let action = match (node.action, node.tool) {
        // Both spellings of one invocation is ambiguous: the document says two things.
        (Some(_), Some(_)) => {
            return Err(TemplateError::InvalidInvocation {
                node: name,
                reason: "both `action` and `tool` describe one invocation".to_string(),
                path: source.map(str::to_string),
            });
        }
        (Some(action), None) => action,
        (None, Some(tool)) => TemplateAction::ToolCall {
            tool,
            parameters: node
                .parameters
                .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new())),
        },
        (None, None) => {
            return Err(TemplateError::InvalidInvocation {
                node: name,
                reason: "neither `action` nor `tool` describes an invocation".to_string(),
                path: source.map(str::to_string),
            });
        }
    };

    Ok(TemplateNode {
        id: node.id.unwrap_or_else(|| slug_from(&name)),
        name,
        depends_on: node.depends_on,
        action,
        description: node.description,
        retry: node.retry,
        validate: node.validate,
        requires_approval: node.requires_approval,
        require_identity: node.require_identity,
        intent: node.intent,
    })
}

/// A kebab-case identifier from a human name — the demo dialect's id, derived rather than invented.
fn slug_from(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// The canonical bytes of a template: what a digest is taken over (#1529, ADR-024 §4).
///
/// `schema_version` is first, so two documents differing only in version differ in their first
/// field — a digest that could not tell them apart could not pin an artifact to a version.
///
/// # Errors
/// `TemplateError::Parse` if the value cannot be written, which would mean the value is not of this
/// schema.
pub fn canonical_bytes(template: &Template) -> Result<String, TemplateError> {
    toml::to_string(template).map_err(|e| TemplateError::Parse {
        detail: format!("the value cannot be written as this schema: {e}"),
        line: None,
        path: None,
    })
}

// ---------------------------------------------------------------------------
// Tests: the schema's own contract, beside the code that decides it
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_STEPS: &str = r#"
name = "e2e-test-plan"
description = "End-to-end test plan"
version = "1.0.0"
tags = []

[[steps]]
name = "step-1"
description = "Test step"
tool = "test_tool"
requires_approval = false
require_identity = false
evaluate_score = false

[steps.parameters]
"#;

    const NODES_FORM: &str = r#"
schema_version = 1
id = "e2e-test-plan"
name = "e2e-test-plan"
description = "End-to-end test plan"
version = "1.0.0"
tags = []

[[nodes]]
id = "step-1"
name = "step-1"
description = "Test step"
tool = "test_tool"
"#;

    #[test]
    fn the_legacy_dialect_is_version_one_and_says_so() {
        let doc = parse_document(LEGACY_STEPS, Some("demo.toml")).unwrap();
        assert_eq!(doc.dialect, Dialect::Steps);
        assert_eq!(doc.template.schema_version, 1);
        assert_eq!(doc.template.id, "e2e-test-plan");
        assert_eq!(doc.template.nodes.len(), 1);
        assert!(matches!(
            doc.template.nodes[0].action,
            TemplateAction::ToolCall { ref tool, .. } if tool == "test_tool"
        ));
    }

    #[test]
    fn the_node_dialect_refuses_a_document_with_no_version() {
        let no_version = NODES_FORM.replace("schema_version = 1\n", "");
        let err = parse_document(&no_version, Some("x.toml")).unwrap_err();
        assert!(
            matches!(err, TemplateError::MissingSchemaVersion { .. }),
            "{err}"
        );
        assert!(err.to_string().contains("x.toml"), "{err}");
    }

    #[test]
    fn an_unknown_version_is_refused_and_the_supported_set_is_named() {
        let err = parse_document(
            &NODES_FORM.replace("schema_version = 1", "schema_version = 7"),
            None,
        )
        .unwrap_err();
        match err {
            TemplateError::UnsupportedSchemaVersion {
                found, supported, ..
            } => {
                assert_eq!(found, 7);
                assert_eq!(supported, vec![1]);
            }
            other => panic!("expected UnsupportedSchemaVersion, got {other}"),
        }
    }

    #[test]
    fn both_spellings_in_one_document_is_ambiguous() {
        let both = format!("{NODES_FORM}\n[[steps]]\nname = \"s\"\ntool = \"t\"\n");
        let err = parse_document(&both, None).unwrap_err();
        assert!(
            matches!(err, TemplateError::AmbiguousDialect { .. }),
            "{err}"
        );
    }

    #[test]
    fn a_node_with_two_invocations_is_refused_and_names_the_node() {
        let doc = NODES_FORM.replace(
            "tool = \"test_tool\"",
            "tool = \"test_tool\"\n[nodes.action]\ntype = \"run_command\"\ncommand = \"ls\"",
        );
        let err = parse_document(&doc, None).unwrap_err();
        match err {
            TemplateError::InvalidInvocation { node, .. } => assert_eq!(node, "step-1"),
            other => panic!("expected InvalidInvocation, got {other}"),
        }
    }

    #[test]
    fn the_two_dialects_have_one_canonical_form() {
        // The whole "one schema" claim in one assertion: two spellings, one value, one set of bytes.
        let legacy = parse_document(LEGACY_STEPS, None).unwrap().template;
        let nodes = parse_document(NODES_FORM, None).unwrap().template;
        assert_eq!(
            canonical_bytes(&legacy).unwrap(),
            canonical_bytes(&nodes).unwrap()
        );
    }

    #[test]
    fn the_version_is_inside_the_canonical_bytes() {
        // Two documents differing only in version must not be able to share a digest.
        let mut bumped = parse_document(NODES_FORM, None).unwrap().template;
        let first = canonical_bytes(&bumped).unwrap();
        bumped.schema_version = 2;
        let second = canonical_bytes(&bumped).unwrap();
        assert_ne!(first, second);
        assert!(second.starts_with("schema_version = 2"), "{second}");
    }

    #[test]
    fn a_round_trip_is_byte_identical() {
        for source in [LEGACY_STEPS, NODES_FORM] {
            let once = parse_document(source, None).unwrap().template;
            let written = canonical_bytes(&once).unwrap();
            let twice = parse_document(&written, None).unwrap().template;
            assert_eq!(once, twice);
            assert_eq!(written, canonical_bytes(&twice).unwrap());
        }
    }

    #[test]
    fn the_slug_is_derived_from_the_name_not_invented() {
        assert_eq!(slug_from("End-to-end test plan"), "end-to-end-test-plan");
        assert_eq!(slug_from("  spaced  "), "spaced");
    }
}
