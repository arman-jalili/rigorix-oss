# DAG Engine

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Two-phase DAG construction (add_unchecked → seal), Kahn's topological sort with cycle detection, O(1) ready queue. Core graph data structure consumed by the Execution Engine.

The CLI does not directly interact with this module — it is used internally by the Planning Pipeline (graph generation) and Execution Engine (execution).

## Components

**CLI-facing:** None — CLI wraps engine contracts directly. No CLI-specific interface files needed.

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| TaskGraph (aggregate root) | `engine/src/dag_engine/domain/graph.rs` | `# Contract (Frozen)` |
| TaskNode | `engine/src/dag_engine/domain/graph.rs` | Single node with id, name, tool, dependencies, policy |
| ExecutionPolicy | `engine/src/dag_engine/domain/graph.rs` | Per-node retry/fallback/validation config |
| ValidationRule | `engine/src/dag_engine/domain/graph.rs` | Post-execution validation (TypeCheck, TestPass, etc.) |
| PlanDiff | `engine/src/dag_engine/domain/plan.rs` | Structural diff between two plans |
| DagError | `engine/src/dag_engine/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| (uses ExecutionEngine events) | — | — |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| TaskGraph | Core DAG structure with two-phase construction (add_unchecked → seal). Contains TaskNodes and execution state. |
| TaskNode | A single unit of work: UUID, name, tool binding, dependencies, ExecutionPolicy, intent. |
| ExecutionPolicy | Per-node config for retry, fallback, and post-execution validation. |
| ValidationRule | Post-execution check: TypeCheck, TestPass, LintPass, or Custom. |
| PlanDiff | Structural diff between two execution plans (added/removed/changed nodes). |
| SealedGraph | A TaskGraph that has passed topological sort and cycle detection, ready for execution. |

## Dependencies

- Depends on: `engine::dag_engine` (all contracts frozen)
- Used by: `Planning Pipeline` (generates TaskGraph from template)
- Used by: `Execution Engine` (consumes sealed TaskGraph for execution)
