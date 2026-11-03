# ADR-008: Proportional Modeling Depth

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

Guardian blueprints assume a multi-context system where every module could grow
aggregates, events and endpoints. Applied uniformly, that produces speculative
modeling: aggregates with no invariants, events nobody consumes, endpoints over a
domain that has none.

The exploration captured exactly one context and classified it. Modeling effort must
follow that classification, and the depth header of a module doc is a contract: it is
never rewritten by later enrichment, and an existing `Modeling Depth Note (ADR-008)`
section is never deleted.

## Decision

Three depths, decided by subdomain type:

| Depth | Applies to | Required | Explicitly forbidden |
|-------|-----------|----------|----------------------|
| **tactical** | core subdomain | Aggregate roots as consistency boundaries with the exploration's invariants (rule + illegalStatePrevented), behaviour methods (no setters), repository interface owned by `domain`, value objects with entity-vs-VO justification, triaged domain events with payloads, explicit domain/application/infrastructure service classification, full module diagram set | An anemic aggregate; setters on the root; a repository trait in `application`/`infrastructure` |
| **service** | supporting subdomain | Service + persistence contract: inputs, outputs, transaction boundary, error cases, data-flow and sequence diagrams | New aggregates, value objects, domain events beyond the exploration |
| **adapter** | generic subdomain | The narrow interface the domain consumes, the OSS/vendor choice, the ACL translation, its context-map edge, an integration diagram | Domain entities or aggregates of any kind |

Application rules:

1. Depth comes from `subdomainType` in the exploration, recorded in the module doc's
   `**Modeling Depth:**` header. `Portable Session` is `core` ⇒ **tactical**.
2. No abstraction before a second variant exists. One fixture-driven provider transport
   is a struct; a port exists because the engine *declares* it, not because we imagine
   alternatives.
3. Every modeled concept must trace to an entity, event, invariant or requirement the
   exploration captured. An untraceable concept is deleted, not documented.
4. Enrichment adds sections; it never rewrites the depth header or the subdomain
   rationale.

## Consequences

- The one core context gets full treatment (12 value objects, 7 events, 8 invariants),
  while delivery concerns (verifier bridge, conformance harness) stay thin.
- Reviewers have a written rule for rejecting speculative modeling.
- A future supporting/generic context arrives with a smaller, honest document.

### Negative
- A concept that starts supporting and later becomes core needs an ADR to promote it,
  with the modeling work re-done. Accepted: promotion is rare and visible.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| Uniform tactical depth everywhere | Simple rule | Invented aggregates, dead events, wasted effort | Proportionality is the point |
| Depth by line count | Mechanical | Size ≠ importance; a 40-line policy is core, a 400-line adapter is not | Wrong axis |
| Defer depth until implementation | Fast start | Domain decisions get made ad hoc in code | The exploration already classified the context |

## Implementation

**Affected Modules:** `.pi/architecture/modules/portable-session.md` (depth header and Modeling Depth Note)

**Enforcement:**
- Guard: `validate-architecture.sh` checks that every module doc has a `**Modeling Depth:**` header and that the header value matches the exploration's `subdomainType`.
- Reviewer rule: enrichment may add sections but must not rewrite the depth header, delete a `Modeling Depth Note`, or introduce a concept with no trace to an exploration entity/event/invariant.

**Canonical References:** `// @canonical .pi/architecture/decisions/ADR-008-proportional-modeling-depth.md`

## Validation

- **architecture-validator**: depth header present and consistent with `subdomainType`; tactical module has aggregate + invariants + repository trait in `domain/`; service module has no invented aggregates; adapter module has no domain entities.

## References

- Related ADRs: ADR-001, ADR-004, ADR-009
- Module doc depth headers: `.pi/architecture/modules/portable-session.md`

---

*Decision date: 2026-11-03*