# ADR-004: Template Format — TOML

**Status:** Accepted
**Date:** 2026-06-16

## Context

Workflow templates define reusable DAG patterns. They need to be human-readable, version-controllable, and machine-parseable.

## Decision

**Use TOML** as the template definition format.

Rationale:
- **Existing in engine** — `TemplateParser` already parses TOML via `toml::from_str`
- **Human-readable** — no unnecessary syntax, easy to write by hand
- **Well-typed** — serde deserialization with tagged unions for `TemplateAction`
- **Version-controllable** — templates are `.rigorix/templates/*.toml` files in the repo
- **Comparison to YAML**: YAML is more complex (multiline strings, anchors) and has security concerns

## Template Example

```toml
id = "add-api-endpoint"
name = "Add API Endpoint"
description = "Scaffolds a new REST API endpoint"
version = "1.0.0"

[[parameters]]
name = "module"
description = "Module to add the endpoint to"
required = true
param_type = "string"

[[parameters]]
name = "method"
description = "HTTP method"
required = false
param_type = "enum"
default = "GET"

[[nodes]]
id = "read-module"
name = "Read module file"
depends_on = []
[nodes.action]
type = "file_read"
path = "src/{{ module }}.rs"

[[nodes]]
id = "write-endpoint"
name = "Write endpoint code"
depends_on = ["read-module"]
[nodes.action]
type = "file_patch"
path = "src/{{ module }}.rs"
search = "// ENDPOINTS"
insert = "app.{{ method }}('/{{ module }}', handler);"
before = false
```

## Alternatives

| Format | Reason Rejected |
|--------|----------------|
| YAML | More complex syntax, security issues with `!!python/tag`, less typed serde support |
| JSON | Less human-readable for hand-authoring, no comments |
| Custom format | Maintenance burden, no ecosystem tooling |
| RON (Rusty Object Notation) | Less known, no TOML's ecosystem maturity |

*Affects: Templates, Template Generation*
