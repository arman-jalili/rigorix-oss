# Architecture Generator

Generate canonical architecture modules from intent or existing documents.

## Usage

### From Intent

```
/architect-generate --intent "Build an auth system with JWT validation, OAuth2 SSO, Redis session management, and full observability"
```

### From Existing Documents

```
/architect-generate --from "docs/prd.md,docs/auth-design.md,docs/api-spec.md" --module "auth-system"
```

## Process

1. **Analyze** the intent or existing documents
2. **Identify** components, their responsibilities, and dependencies
3. **Structure** the architecture module with proper status, dependencies, and descriptions
4. **Validate** the module against Guardian's architecture conformance rules
5. **Write** the canonical module to `.pi/architecture/modules/<name>.md`

## Module Structure

```markdown
# Module Name

## Component Name
status: planned
description: Clear, specific description of what this component does.
depends: Other Component Name
```

## Rules

- Each component must have a `status` (planned, in-progress, implemented, deprecated)
- Each component must have a `description` that is specific and actionable
- Dependencies must reference other component names in the same module or other modules
- Components should be ordered by dependency (leaf dependencies first)
- One module per bounded context or subsystem
- Cross-module dependencies are documented in the `depends` field using `module:component` syntax

## Architecture Conformance

The generated module must satisfy these checks:

1. **No orphaned components** — every component has at least one dependency or is a root
2. **No circular dependencies** — components don't form dependency cycles
3. **Clear separation of concerns** — components don't overlap in responsibility
4. **Observable interfaces** — each component has clear inputs and outputs
5. **DR-ready** — failure modes and recovery paths are considered

## Examples

### Intent-Based

```
You: /architect-generate --intent "Build a payment processing system with Stripe integration, webhook handling, idempotency, and reconciliation"

Guardian generates: .pi/architecture/modules/payment-system.md
  - Stripe API Integration (depends: none)
  - Webhook Handler (depends: Stripe API Integration)
  - Idempotency Layer (depends: Webhook Handler)
  - Reconciliation Engine (depends: Idempotency Layer)
  - Architecture Observability (depends: all above)
```

### Document-Based

```
You: /architect-generate --from "docs/prd-auth.md,docs/oauth2-spec.md" --module "auth-system"

Guardian reads the documents, extracts components, and generates:
  .pi/architecture/modules/auth-system.md
```
