# Planning Pipeline

## Module Status

**Status:** Planned — CLI integration over engine contracts
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

6-phase planning flow: Budget Pre-check → Intent Classification (LLM) → Parameter Extraction → DAG Generation → Plan Validation → Hash Computation.

The engine crate provides the `PlanningPipelineService` trait and implementation. The CLI exposes `rigorix plan` (preview without execution) and integrates the pipeline into `rigorix run`.

When no template matches with sufficient confidence, the pipeline falls back to `Template Generation` to create a template on-the-fly, which is then persisted for future reuse.

## Components

**CLI-facing:**
| Component | File (planned) | Module | Purpose |
|-----------|---------------|--------|---------|
| PlanCommandService (trait) | `cli/src/planning/infrastructure/service.rs` | planning | Service trait for plan command |
| PlanEngineHandler | `cli/src/planning/infrastructure/plan_handler_impl.rs` | planning | Implements PlanCommandService via engine PlanningPipelineService |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| PlanningPipelineService (trait) | `engine/src/planning/application/service.rs` | `# Contract (Frozen)` |
| UserIntent | `engine/src/planning/domain/intent.rs` | Value object with raw intent + clarification history |
| PlanningResult | `engine/src/planning/domain/result.rs` | Aggregate root: matched template, params, graph, hash |
| PlanningHash | `engine/src/planning/domain/result.rs` | Deterministic SHA-256 hash for audit |
| ClassificationResult | `engine/src/planning/domain/classification.rs` | Template ID + confidence + extracted params |
| PlanningError | `engine/src/planning/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| PlanningStarted | Execution plan generation has begun. Carries raw user intent. | PlanningPipelineService |
| PlanningCompleted | Plan generated successfully. Carries template, confidence, params. | PlanningPipelineService |
| TemplateClassificationRequested | LLM is being asked to classify intent against templates. | IntentClassifier |
| TemplateMatched | A matching template was found for the intent. | Classifier |
| TemplateGenerationRequested | No match found; fallback to LLM-based template generation. | PlanningPipelineService |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| UserIntent | Raw natural-language input from the user describing what they want to accomplish. |
| PlanningResult | Complete output of planning: matched template, resolved params, validated TaskGraph, hash. |
| PlanningHash | Deterministic SHA-256 hash of the full plan for replay auditing. |
| ClassificationResult | LLM classification output: matched template ID, confidence score, extracted parameters. |

## Dependencies

- Depends on: `engine::planning` (pipeline service, domain entities)
- Depends on: `Templates` (template registry for classification and graph generation)
- Depends on: `Template Generation` (fallback path when no match)
- Depends on: `DAG Engine` (generates TaskGraph from template + params)
- Depends on: `Configuration` (LLM provider config, template directory)
- Depends on: `Budget Tracking` (LLM call budget pre-check)
- Used by: `CLI Boundary` (exposes `rigorix plan` and `rigorix run`)

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/planning/infrastructure/service.rs` | PlanCommandService trait (planned) |
| `cli/src/planning/infrastructure/plan_handler_impl.rs` | PlanEngineHandler implementation (planned) |
| `engine/src/planning/application/service.rs` | PlanningPipelineService trait |
| `engine/src/planning/application/pipeline_impl.rs` | Pipeline implementation with fallback wiring |
| `engine/src/planning/domain/` | Core domain entities (frozen contracts) |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Proposed |
