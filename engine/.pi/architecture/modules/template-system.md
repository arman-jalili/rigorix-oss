# Template System Architecture

<!--
Canonical Reference: .pi/architecture/modules/template-system.md
Blueprint Source: Domain Exploration Session 63c25384
Last Updated: 2026-06-14
Module version: 2.0.0
-->

## Overview

Manages workflow template definitions stored as TOML files. Handles parsing, schema validation, template engine instantiation (TOML → executable graph), and loading of built-in templates.

## Responsibilities

- Parse and validate TOML template files against schema
- Maintain a runtime registry of loaded templates (TemplateEngine)
- Instantiate templates into executable graphs with parameter substitution
- Load built-in templates + project-local templates from `templates/`
- Expose template metadata for classification and audit
- Structural validation: unique node IDs, dependency integrity, cycle detection (Kahn's algorithm)

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| TemplateParserImpl | `src/templates/application/template_parser_impl.rs` | Parse TOML files into Template structs, validate schema | #parser |
| TemplateEngineImpl | `src/templates/application/template_engine_impl.rs` | Runtime registry: register, lookup, generate graphs | #engine |
| Template | `src/templates/domain/template.rs` | Template aggregate with metadata, parameters, and nodes | #template |
| TemplateNode | `src/templates/domain/template.rs` | Single DAG node with action, dependencies, retry, validation | #node |
| TemplateAction | `src/templates/domain/template.rs` | 9 action variants (FileRead, FileWrite, FileAppend, FilePatch, RunCommand, LspQuery, GitRead, GitStage, GitCommit) | #action |
| ParameterDef | `src/templates/domain/template.rs` | Parameter definition with name, type, required, default, constraints | #params |
| TemplateError | `src/templates/domain/error.rs` | 10 error variants with structured context | #errors |
| TemplateEvent | `src/templates/domain/event/mod.rs` | 7 event payload schemas | #events |
| TemplateRepository | `src/templates/infrastructure/repository/mod.rs` | Repository trait for template file access | #repository |
| InMemoryTemplateRepository | `src/templates/infrastructure/repository/mod.rs` | In-memory test double for TemplateRepository | #inmemory |
| HTTP API | `src/templates/interfaces/http/mod.rs` | 6 REST endpoints under /api/v1/templates | #api |

---

## Component Details

### TemplateParser (TemplateParserImpl)

**Purpose:** Parse TOML template files into validated Template structs

**Implementation File:** `src/templates/application/template_parser_impl.rs`

**Canonical Reference:** `.pi/architecture/modules/template-system.md#parser`

**Dependencies:**
- `toml` crate (TOML deserialization)
- `TemplateRepository` trait (file access)
- TemplateAction enum variants

**Service Trait (`TemplateParserService`):**

```rust
#[async_trait]
pub trait TemplateParserService: Send + Sync {
    async fn parse_file(&self, input: ParseFileInput) -> Result<ParseOutput, TemplateError>;
    async fn parse_str(&self, input: ParseStrInput) -> Result<ParseOutput, TemplateError>;
    async fn load_directory(&self, path: &str) -> Result<LoadDirectoryOutput, TemplateError>;
    async fn validate_template(
        &self, input: ValidateTemplateInput
    ) -> Result<ValidateTemplateOutput, TemplateError>;
    async fn load_builtins(
        &self, input: LoadBuiltinsInput
    ) -> Result<LoadBuiltinsOutput, TemplateError>;
}
```

**Key behaviors:**
- TOML deserialization via `toml::from_str`
- Structural validation: required fields, unique node IDs, dependency integrity
- Cycle detection using Kahn's algorithm
- Directory loading aggregates successes and failures
- Built-in template loading from embedded definitions

### TemplateEngine (TemplateEngineImpl)

**Purpose:** Runtime registry that holds registered templates and instantiates them into executable graphs

**Implementation File:** `src/templates/application/template_engine_impl.rs`

**Dependencies:**
- Template struct (for registry and generation)
- serde_json::Value (for parameter values)

**Service Trait (`TemplateEngineService`):**

```rust
#[async_trait]
pub trait TemplateEngineService: Send + Sync {
    async fn register(&self, input: RegisterInput) -> Result<RegisterOutput, TemplateError>;
    async fn generate(&self, input: GenerateInput) -> Result<GenerateOutput, TemplateError>;
    async fn get_template(
        &self, input: GetTemplateInput
    ) -> Result<Option<TemplateSummary>, TemplateError>;
    async fn list_templates(&self) -> Result<ListTemplatesOutput, TemplateError>;
    async fn has_template(&self, template_id: &str) -> bool;
    async fn template_count(&self) -> usize;
}
```

**Key behaviors:**
- In-memory registry via `RwLock<HashMap<String, RegisteredEntry>>`
- `{{ param_name }}` substitution with `\{{ param_name }}` and `{{param_name}}` syntax support
- Required parameter validation before generation
- Topological sort via Kahn's algorithm with cycle detection
- Duplicate detection with optional overwrite

---

## Data Flow

```mermaid
flowchart LR
    A["TOML File<br/>templates/*.toml"] -->|parse_file| B[TemplateParserImpl]
    B -->|validated| C[Template struct]
    C -->|register| D[TemplateEngineImpl]
    E["Planning Pipeline"] -->|generate(id, params)| D
    D -->|substitute params| F[GenerateOutput]
    F -->|nodes + edges| G["DAG Engine"]
    G -->|ready| H["Execution Engine"]
```

**Flow Description:**
1. TOML template files are loaded from `templates/*.toml` or built-in definitions
2. TemplateParserImpl validates schema (structural + cycle detection) and produces Template structs
3. TemplateEngineImpl registers templates by kebab-case ID in an in-memory registry
4. Planning Pipeline calls `generate(id, params)` with extracted parameters
5. TemplateEngineImpl substitutes `{{ param_name }}` placeholders and produces a GenerateOutput
6. GenerateOutput contains resolved nodes and edges for DAG engine consumption
7. Results validated and passed to execution

---

## Dependencies

### Depends On
- **Configuration**: Template directory paths
- **Error Handling**: TemplateError for structured error types
- **DAG Engine**: Consumes graph output from generation (via GenerateOutput DTO until DAG module exists)

### Used By
- **Planning Pipeline**: Template selection, parameter extraction, graph generation
- **Template Generation**: Registers newly generated templates

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Malformed TOML injection | Schema validation on parse; reject unknown fields | security-validator |
| Template parameter injection | Parameter substitution uses serde_json::Value, not string interpolation | security-validator |
| Path traversal in template files | Repository validates file paths (planned for FileSystemTemplateRepository) | security-validator |

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 90%+ | `src/templates/application/template_parser_impl.rs` (21 tests) |
| Unit | 90%+ | `src/templates/application/template_engine_impl.rs` (10 tests) |

**Key Test Scenarios (all passing):**
- Parse valid TOML template → Template struct
- Parse invalid TOML → ParseOutput with errors
- Parse minimal template (no nodes, no parameters)
- Parse missing required field → parse error
- Duplicate node IDs → validation error
- Missing dependency reference → validation error
- Cycle detection → validation error
- Parameter reference validation → warning
- Register template → success, duplicate detection, overwrite
- Generate graph with parameter substitution
- Missing required parameter → MissingParameter error
- Template not found → NotFound error
- List, check, count templates
- Load built-in templates
- Load empty directory
- Parse file from repository

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum TemplateError {
    Parse { detail: String, line: Option<u32>, path: Option<String> },
    MissingParameter { template: String, param: String, description: Option<String> },
    NotFound { id: String, available: Vec<String> },
    InvalidParameter { param: String, expected: String, actual: String, value: Option<String> },
    ValidationFailed { field: String, reason: String, value: Option<String> },
    DuplicateTemplate { id: String },
    Io { io_error: std::io::Error },
    CycleDetected { template: String, nodes: Vec<String> },
    DependencyNotFound { template: String, dependency: String },
    GenerationFailed { detail: String, attempts: u8 },
}
```

## Performance Considerations

| Metric | Target | Monitoring |
|--------|--------|------------|
| Template parse | < 10ms per file | Tracing spans |
| Graph generation | < 50ms | Tracing spans |
| Template registration | < 1ms | Tracing spans |

## CI Proofing

| Script | Purpose | Status |
|--------|---------|--------|
| `check_template-system_contracts.sh` | Verifies all 12 contract interfaces have implementations | ✅ |
| `check_template-system_coverage.sh` | Enforces 80% coverage (21 tests, falling back to count) | ✅ |
| `stage_template-system_proofing.sh` | CI stage wrapper (Stage 20) | ✅ integrated |

---
*Last updated: 2026-06-14*
*Module version: 2.0.0*
