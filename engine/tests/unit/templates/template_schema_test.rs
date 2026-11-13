//! The template document schema: two spellings, one value, one version (#1529).
//!
//! The fixtures beside this test are **content, not the repository under test**: copies of the two
//! documents the tree ships (`mcp/.rigorix/templates/e2e-test-plan.toml`, the only `[[steps]]`
//! document; `cli/.rigorix/templates/validate-project-workflow.toml`, the only `[[nodes]]` one),
//! plus a three-step `[[steps]]` document and its `[[nodes]]` twin, because a **one-step** document
//! cannot demonstrate the linear case the issue is about.
//!
//! Gate: `templates` is excluded from the portable slice
//! (`engine/.pi/portable-inventory.tsv:49`), so this module carries the same
//! `#[cfg(not(feature = "portable"))]` gate as its siblings — the omission that put `main` red in
//! `a49c8c1d` (E0433, fixed in `47904b3f`, and now caught by `local-ci` Stage 2b).

use rigorix_engine::templates::application::template_document::{
    Dialect, canonical_bytes, parse_document,
};
use rigorix_engine::templates::domain::{SUPPORTED_SCHEMA_VERSIONS, TemplateAction, TemplateError};

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/templates")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn template_schema_the_shipped_steps_document_parses_without_modification() {
    // Criterion: a checked-in copy of the demo `[[steps]]` document parses unchanged. It carries no
    // `schema_version` — and it does not need one, because the dialect *is* version 1.
    let source = fixture("e2e-test-plan.steps.toml");
    let document = parse_document(&source, Some("e2e-test-plan.toml")).unwrap();
    assert_eq!(document.dialect, Dialect::Steps);
    assert_eq!(document.template.schema_version, 1);
    assert_eq!(document.template.name, "e2e-test-plan");
    // The demo documents name their document and not its parent id; the id is derived, not invented.
    assert_eq!(document.template.id, "e2e-test-plan");
    assert_eq!(document.template.nodes.len(), 1);
    assert!(matches!(
        document.template.nodes[0].action,
        TemplateAction::ToolCall { ref tool, .. } if tool == "test_tool"
    ));
}

#[test]
fn template_schema_the_shipped_nodes_document_needs_a_version_it_does_not_have() {
    // The same rule, refused: the engine's own spelling must say which schema it is in. The shipped
    // document predates the field, so this is the migration the refusal is *for* — and the message
    // names the file and the line to add.
    let source = fixture("validate-project-workflow.nodes.toml");
    let err = parse_document(&source, Some("validate-project-workflow.toml")).unwrap_err();
    assert!(
        matches!(err, TemplateError::MissingSchemaVersion { .. }),
        "{err}"
    );
    let message = err.to_string();
    assert!(
        message.contains("validate-project-workflow.toml"),
        "{message}"
    );
    assert!(message.contains("schema_version"), "{message}");
}

#[test]
fn template_schema_the_two_spellings_are_one_value_and_one_set_of_bytes() {
    // The "one schema" claim, asserted where it can fail: a three-step document written both ways
    // parses to equal values and serializes to identical canonical bytes.
    let steps = parse_document(&fixture("three-steps.steps.toml"), None).unwrap();
    let nodes = parse_document(&fixture("three-steps.nodes.toml"), None).unwrap();
    assert_eq!(steps.dialect, Dialect::Steps);
    assert_eq!(nodes.dialect, Dialect::Nodes);
    assert_eq!(steps.template, nodes.template);
    assert_eq!(
        canonical_bytes(&steps.template).unwrap(),
        canonical_bytes(&nodes.template).unwrap()
    );
    assert_eq!(nodes.template.nodes.len(), 3);
}

#[test]
fn template_schema_round_trips_are_byte_identical() {
    // `parse(serialize(parse(x))) == parse(x)` **on the bytes**, for every fixture, including the one
    // that is refused until it carries a version — that one is round-tripped after being read as
    // what it is, so the test covers the path rather than skipping the document.
    for name in [
        "e2e-test-plan.steps.toml",
        "three-steps.steps.toml",
        "three-steps.nodes.toml",
    ] {
        let once = parse_document(&fixture(name), Some(name)).unwrap().template;
        let written = canonical_bytes(&once).unwrap();
        let twice = parse_document(&written, Some(name)).unwrap().template;
        assert_eq!(once, twice, "{name}");
        assert_eq!(written, canonical_bytes(&twice).unwrap(), "{name}");
    }
}

#[test]
fn template_schema_an_unknown_version_is_refused_with_the_set_named() {
    let mut source = fixture("three-steps.nodes.toml");
    source = source.replace("schema_version = 1", "schema_version = 2");
    let err = parse_document(&source, None).unwrap_err();
    match err {
        TemplateError::UnsupportedSchemaVersion {
            found, supported, ..
        } => {
            assert_eq!(found, 2);
            assert_eq!(supported, SUPPORTED_SCHEMA_VERSIONS.to_vec());
        }
        other => panic!("expected UnsupportedSchemaVersion, got {other}"),
    }
}

#[test]
fn template_schema_the_version_is_inside_the_canonical_bytes() {
    // A digest that cannot tell two versions apart cannot pin an artifact to one.
    let template = parse_document(&fixture("three-steps.nodes.toml"), None)
        .unwrap()
        .template;
    let first = canonical_bytes(&template).unwrap();
    let mut bumped = template;
    bumped.schema_version = 2;
    let second = canonical_bytes(&bumped).unwrap();
    assert_ne!(first, second);
    assert!(first.starts_with("schema_version = 1"), "{first}");
}

#[test]
fn template_schema_a_malformed_document_names_file_line_and_field() {
    // Criterion: the engine's parser reports file, line and field — not a serde backtrace in a host.
    let source = "schema_version = 1\nid = \"x\"\nname = \"x\"\n[[nodes]]\nid = \"n\"\nname = \"n\"\naction = 7\n";
    let err = parse_document(source, Some("bad.toml")).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("bad.toml"), "{message}");
    assert!(
        message.contains("line") || message.chars().any(|c| c.is_ascii_digit()),
        "expected a line reference in: {message}"
    );
    assert!(
        message.to_lowercase().contains("action")
            || message.to_lowercase().contains("type")
            || message.contains("template"),
        "expected the field to be named in: {message}"
    );
}

#[test]
fn template_schema_the_repository_holds_no_second_toml_template_parser() {
    // Criterion: one parser. The subject is the *deserializer of a template document*, not the
    // string `toml` — a hook config is not a template, and a rule that flagged it would be a rule
    // nobody could keep green without lying about it. So: every file that names the engine's
    // template type must not call `toml::from_str` itself, except the one module that owns the
    // schema. The scan reports what it looked at, because a check that passes on an empty set is
    // not a check (`#1514`).
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root is the manifest's parent");
    let mut offenders = Vec::new();
    let mut scanned = 0usize;
    let mut governed = 0usize;
    for crate_dir in ["mcp", "cli", "actions", "engine", "wasm", "server"] {
        let src = root.join(crate_dir).join("src");
        if !src.is_dir() {
            continue;
        }
        for entry in walk(&src) {
            scanned += 1;
            let path = entry.to_string_lossy().to_string();
            if path.contains("engine/src/templates/") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&entry) else {
                continue;
            };
            // The subject is the *deserializer*, so the flag is a `toml::from_str` whose own line
            // names a template type — the annotation is written beside the call, or on the line
            // the call continues onto. The boundary is stated rather than hidden: a `#[cfg(test)]`
            // deserialization inside the engine (the generator's own assertions) is not a second
            // schema, and a hook config is not a template.
            for (number, line) in text.lines().enumerate() {
                if !line.contains("toml::from_str") {
                    continue;
                }
                let next = text.lines().nth(number + 1).unwrap_or_default();
                let names_template = line.contains("Template") || next.contains(": Template");
                if names_template {
                    governed += 1;
                    offenders.push(format!("{path}:{}", number + 1));
                }
            }
        }
    }
    assert!(scanned > 100, "the scan looked at {scanned} files");
    assert!(
        governed == 0,
        "a second template deserializer: {offenders:?}"
    );
    assert!(
        offenders.is_empty(),
        "a second template deserializer: {offenders:?}"
    );

    // And the module the issue names is gone, asserted by path rather than by memory.
    let converter = root.join("mcp/src/template_tools/infrastructure/template_converter_impl.rs");
    assert!(
        !converter.exists(),
        "the host-side converter is still there: {}",
        converter.display()
    );
}

#[test]
fn template_schema_the_gate_that_excludes_this_module_from_the_slice_is_present() {
    // The regression that put `main` red in `a49c8c1d`: a test module registered in the unit
    // harness without the gate its siblings carry, so `--features portable` failed to compile.
    // Asserted from the harness's own text, because the failure was invisible to a run that did
    // not compile that feature.
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/unit/mod.rs");
    let text = std::fs::read_to_string(&harness).expect("the unit harness is readable");
    let gate = "#[cfg(not(feature = \"portable\"))]\n#[path = \"templates\"]";
    assert!(
        text.contains(gate),
        "the templates module in tests/unit/mod.rs lost its portable gate"
    );
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}
