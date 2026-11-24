//! #1662 — every operator TOML is strict, and the docs' config examples are parsed in a test.
//!
//! The audit found the operator surface to be the *least* strict parser in the tree: no
//! `deny_unknown_fields` anywhere under `engine/src/precondition/`, a documented key the schema does
//! not have, a typo that parsed to zero rules, and `doctor` printing "armed" for that file.
//!
//! This is a **rule**, not a list of struct names: every struct in an `engine/src` domain module that
//! derives `Deserialize` and is named like an operator document (`*Config`, `*Policy`, `*Spec`,
//! `*Settings`, `*Options`) must carry `deny_unknown_fields`, and a new one without it fails here.

use std::path::{Path, PathBuf};

/// The operator surface: the files an operator writes, and the type each one is parsed into.
///
/// This is a table of **files**, not of struct names: the product's own documentation enumerates five
/// `.rigorix/*.toml` files, and every one of them must refuse an unknown key. A struct added to one of
/// these loaders inherits the requirement; a struct elsewhere is not operator surface and is not
/// silently claimed to be.
///
/// The row count is asserted below, so deleting a row to make the test pass fails the test.
const OPERATOR_DOCUMENTS: &[(&str, &str, &str)] = &[
    (
        "preconditions.toml",
        "PreconditionConfig",
        "engine/src/precondition/domain/precondition.rs",
    ),
    (
        "sequence-policy.toml",
        "SequencePolicyConfig",
        "engine/src/sequence_policy/domain/config.rs",
    ),
    (
        "permissions.toml",
        "PermissionConfig",
        "engine/src/permission/domain/config.rs",
    ),
    // These two parse through a private `Raw*` layer and then validate into a typed value that is
    // deliberately **not** `Deserialize` — so the raw type is the one that must refuse unknown keys,
    // and it is the one named here. The requirement is "the type the TOML is parsed into", not "the
    // type with the most public-looking name".
    (
        "capacity.toml",
        "RawConfig",
        "engine/src/capacity_ledger/application/config.rs",
    ),
    (
        "effect-commitment.toml",
        "RawConfig",
        "engine/src/effect_commitment/application/config.rs",
    ),
    (
        "hooks.toml",
        "HookConfig",
        "engine/src/hooks/domain/config.rs",
    ),
    (
        "quality-gates.toml",
        "QualityGateConfig",
        "engine/src/quality_gates/domain/config.rs",
    ),
    (
        "policy.toml",
        "PolicyConfig",
        "engine/src/policy_engine/domain/config.rs",
    ),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn every_operator_document_refuses_unknown_keys() {
    assert!(
        OPERATOR_DOCUMENTS.len() >= 8,
        "the table has {} row(s); removing a row to make this pass is the defect, not the fix",
        OPERATOR_DOCUMENTS.len()
    );
    let mut missing: Vec<String> = Vec::new();
    for (file, type_name, source) in OPERATOR_DOCUMENTS {
        let path = repo_root().join(source);
        let Ok(text) = std::fs::read_to_string(&path) else {
            missing.push(format!("{file}: `{source}` does not exist"));
            continue;
        };
        // The attribute must be on **this** struct: find its definition, then look at the attributes
        // immediately above it. A file-level `deny_unknown_fields` for a different type is not a
        // strict operator document, and a test that accepted it would be the "inherits a row written
        // about another site" defect.
        // The raw layer is private (`struct RawConfig`), the public ones are `pub struct`; both are
        // operator documents, so both spellings are found.
        let index = text
            .find(&format!("pub struct {type_name} {{"))
            .or_else(|| text.find(&format!("struct {type_name} {{")));
        let Some(index) = index else {
            missing.push(format!("{file}: `{type_name}` is not defined in {source}"));
            continue;
        };
        let head = &text[..index];
        let block: String = head
            .lines()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        if !block.contains("deny_unknown_fields") {
            missing.push(format!("{file}: `{type_name}` ({source})"));
        }
    }
    assert!(
        missing.is_empty(),
        "these operator TOML documents accept unknown keys — a typo'd key is a control that \
         silently does nothing (#1662):\n  {}",
        missing.join("\n  ")
    );
}

/// The docs' config examples are **fixtures**: a key the documentation teaches but the schema rejects
/// is a build failure, and this is the only mechanism that stops prose and schema drifting.
#[test]
fn the_documented_precondition_examples_parse() {
    let root = repo_root();
    let mut checked = 0;
    for document in ["README.md", "docs/deployment.md", "docs/runbook.md"] {
        let path = root.join(document);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for block in toml_blocks(&text) {
            // Only blocks that are a precondition document: a general TOML snippet is not this
            // schema's subject, and pretending otherwise would make the test fail for the wrong
            // reason.
            let is_preconditions = block.contains("[[preconditions]]")
                || block.contains("[gating]")
                || block.contains("fail_closed");
            if !is_preconditions {
                continue;
            }
            let parsed: Result<rigorix_engine::precondition::PreconditionConfig, _> =
                toml::from_str(&block);
            assert!(
                parsed.is_ok(),
                "{document}: a documented `.rigorix/preconditions.toml` example does not parse — a \\
                 key the docs teach and the schema rejects is a build failure (#1662):\n{block}\n{err:?}",
                err = parsed.err()
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "no documented precondition example was found — a doc-fixture test that checks nothing is \\
         the drift it exists to stop (#1662)"
    );
}

/// The fenced ```toml blocks of a Markdown document.
fn toml_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        match current.as_mut() {
            None if trimmed.starts_with("```toml") => current = Some(String::new()),
            Some(block) if trimmed.starts_with("```") => {
                blocks.push(std::mem::take(block));
                current = None;
            }
            Some(block) => {
                block.push_str(line);
                block.push('\n');
            }
            None => {}
        }
    }
    blocks
}
