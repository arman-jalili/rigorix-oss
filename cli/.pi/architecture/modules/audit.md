# Audit

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Records execution audit trails via typed envelopes for governance, replay, and external audit backends. Provides the audit envelope format, sender with retry logic, queue management, and circuit breaker for resilient delivery.

The CLI exposes `rigorix audit list` and `rigorix audit show` to inspect past audit trails.

## Components

**CLI-facing:**
| Component | File (planned) | Module | Purpose |
|-----------|---------------|--------|---------|
| AuditCommandService (trait) | `cli/src/audit/infrastructure/service.rs` | audit | Service trait for audit commands |
| AuditEngineHandler | `cli/src/audit/infrastructure/audit_handler_impl.rs` | audit | Implements AuditCommandService via engine AuditService |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| AuditEnvelope (aggregate root) | `engine/src/audit/domain/envelope.rs` | Typed envelope with execution_id, planning_hash, events |
| AuditService (trait) | `engine/src/audit/application/service.rs` | Audit trail management |
| AuditSender (trait) | `engine/src/audit/application/service.rs` | Remote delivery with retry |
| AuditQueue (trait) | `engine/src/audit/application/service.rs` | Queue management with circuit breaker |
| AuditError | `engine/src/audit/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| AuditEnvelopeSent | Audit envelope delivered to backend | AuditSender |
| AuditQueueFull | Queue reached capacity, dropping oldest | AuditQueue |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| AuditEnvelope | Typed governance-grade audit record with execution_id, planning_hash, and ordered events. |

## Dependencies

- Depends on: `engine::audit` (all contracts frozen)
- Depends on: `Event System` (drains events into audit envelopes)
- Used by: `CLI Boundary` (exposes `rigorix audit` commands)
