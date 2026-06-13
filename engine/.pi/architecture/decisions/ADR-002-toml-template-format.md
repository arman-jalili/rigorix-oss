# ADR-002: TOML Template Format for Workflow Definitions

**Status:** Accepted
**Date:** 2026-06-13
**Session:** 63c25384-1902-4b72-83bb-257f3f682af5

**Tech Stack:** Rust

## Context

Rigorix needs a format for defining workflow templates. The format must support nodes, dependencies, parameters, retry configuration, and validation rules. It must be human-writable, machine-parseable, and easily embeddable in LLM prompts.

## Decision

Use **TOML** as the template definition language.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **TOML** | Native Rust serde support via `toml` crate; human-readable; inline tables for concise syntax; well-defined schema; excellent for config-like structures | Less expressive for data structures | **Chosen** |
| **YAML** | More expressive, widely used in CI/CD | Ambiguous edge cases; complex schema validation; parsing performance concerns | Rejected — ambiguity risk for LLM-generated templates |
| **JSON** | Ubiquitous, strict schema | Verbose, harder for humans to write and LLMs to generate validly | Rejected — verbosity increases token cost for prompts |
| **RON** (Rusty Object Notation) | Rust-native syntax | Not widely known; poor LLM generation quality | Rejected — niche format |

## Consequences

### Positive
- Serde enables direct `toml::from_str()` deserialization
- LLMs generate valid TOML more reliably than YAML/JSON
- Inline tables (`retry = { on = [...], max = 3 }`) keep templates concise
- TOML schema is self-documenting with `[[nodes]]` arrays

### Negative
- No built-in merge/override semantics (resolved via TemplateEngine)
- Parameter substitution uses `{{ param_name }}` patterns (custom, not TOML-native)

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/template-system.md`
- `.pi/architecture/modules/template-generation.md`

**Files to Update:**
- `rigorix/src/templates/parser.rs` — Template struct with serde Deserialize

**Key Schema:**
```toml
id = "unique-kebab-case"
name = "Human Name"
description = "What it does"
version = "1.0.0"

[[parameters]]
name = "target_file"
description = "File to modify"
required = true
param_type = "path"

[[nodes]]
id = "read-file"
name = "Read file"
depends_on = []
[nodes.action]
type = "file_read"
path = "{{ target_file }}"
```

---

*Decision date: 2026-06-13*
