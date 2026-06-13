# Template System Architecture

<!--
Canonical Reference: .pi/architecture/modules/template-system.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Overview

Manages workflow template definitions stored as TOML files. Handles parsing, schema validation, template engine instantiation (TOML → TaskGraph), and loading of built-in templates.

## Responsibilities

- Parse and validate TOML template files against schema
- Maintain a runtime registry of loaded templates (TemplateEngine)
- Instantiate templates into TaskGraphs with parameter substitution
- Load 13 built-in templates + project-local templates from `templates/`
- Expose template metadata for classification and audit

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| TemplateParser | `rigorix/src/templates/parser.rs` | Parse TOML files into Template structs, validate schema | #parser |
| TemplateEngine | `rigorix/src/templates/parser.rs` | Runtime registry: register, lookup, generate TaskGraphs | #engine |
| BuiltinTemplates | `rigorix/src/templates/builtin.rs` | Load 13 built-in template definitions | #builtins |
| Template Node | `rigorix/src/templates/parser.rs` | TemplateNode struct with action, dependencies, retry, validation | #node |
| ParameterDef | `rigorix/src/templates/parser.rs` | Parameter definition with name, type, required, default | #params |

---

## Component Details

### TemplateParser

**Purpose:** Parse TOML template files into validated Template structs

**Implementation File:** `rigorix/src/templates/parser.rs`

**Canonical Reference:** `.pi/architecture/modules/template-system.md#parser`

**Dependencies:**
- serde (TOML deserialization)
- TemplateAction enum variants

**Interface:**

```rust
pub struct TemplateParser;

impl TemplateParser {
    pub fn parse_file(path: &Path) -> Result<Template, TemplateError>;
    pub fn parse_str(toml: &str) -> Result<Template, TemplateError>;
    pub fn load_directory(dir: &Path) -> Result<Vec<Template>, TemplateError>;
}
```

### TemplateEngine

**Purpose:** Runtime registry that holds registered templates and instantiates them into executable TaskGraphs

**Implementation File:** `rigorix/src/templates/parser.rs`

**Dependencies:**
- TemplateParser
- ParameterDef (for substitution)

**Interface:**

```rust
pub struct TemplateEngine { /* templates: HashMap<String, Template> */ }

impl TemplateEngine {
    pub fn new() -> Self;
    pub fn register(&mut self, template: Template);
    pub fn generate(&self, id: &str, params: &HashMap<String, Value>) -> Result<TaskGraph, TemplateError>;
    pub fn get_template(&self, id: &str) -> Option<&Template>;
    pub fn templates(&self) -> impl Iterator<Item = &Template>;
}
```

---

## Data Flow

```mermaid
flowchart LR
    A["TOML File<br/>templates/*.toml"] -->|parse_file| B[TemplateParser]
    B -->|validated| C[Template struct]
    C -->|register| D[TemplateEngine]
    E["Planning Pipeline"] -->|generate(id, params)| D
    D -->|substitute params| F[TaskGraph]
    F -->|validate| G["DAG Engine<br/>CompositeValidator"]
    G -->|ready| H["Execution Engine"]
```

**Flow Description:**
1. TOML template files are loaded from `templates/*.toml` or built-in definitions
2. TemplateParser validates schema and produces Template structs
3. TemplateEngine registers templates by kebab-case ID
4. Planning Pipeline calls `generate(id, params)` with extracted parameters
5. TemplateEngine substitutes `{{ param_name }}` placeholders and produces a TaskGraph
6. TaskGraph is validated and passed to execution

---

## Dependencies

### Depends On
- **Configuration**: Template directory paths
- **DAG Engine**: Consumes TaskGraph output from generation

### Used By
- **Planning Pipeline**: Template selection, parameter extraction, graph generation
- **Template Generation**: Registers newly generated templates

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Malformed TOML injection | Schema validation on parse; reject unknown fields | security-validator |
| Template parameter injection | Parameter substitution uses serde_json::Value, not string interpolation | security-validator |

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 95% | `rigorix/src/templates/parser.rs` (inline tests) |
| Integration | 90% | `rigorix/tests/integration.rs` |

**Key Test Scenarios:**
- Parse valid TOML template → Template struct
- Parse invalid TOML → TemplateError
- Register and generate TaskGraph with parameter substitution
- Load built-in templates successfully
- Missing parameter → MissingParameter error

---

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("Failed to parse template: {0}")]
    Parse(String),
    #[error("Template '{template}' requires parameter '{param}'")]
    MissingParameter { template: String, param: String },
    #[error("Template not found: {0}")]
    NotFound(String),
    #[error("Invalid parameter type: {0}")]
    InvalidParameter(String),
}
```

---

## Performance Considerations

| Metric | Target | Monitoring |
|--------|--------|------------|
| Template parse | < 10ms per file | Tracing spans |
| TaskGraph generation | < 50ms | Tracing spans |

---

*Last updated: 2026-06-13*
*Module version: 1.0.0*
