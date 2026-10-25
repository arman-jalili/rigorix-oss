# 12 — Versioning and change control

**Status: normative framing.**

---

## 1. Version identifiers

`execution-governance <major>.<minor>.<patch>`

- **Major**: a requirement is added, removed or made stricter; a term's meaning
  changes; an identifier is withdrawn.
- **Minor**: clarification that changes no obligation; new informative material; new
  fixtures; new non-claims.
- **Patch**: editorial only.

## 2. Requirement stability

Identifiers are permanent. A withdrawn requirement keeps its number, is marked
withdrawn, and states why. Numbers are never reused, because tests and claims cite
them.

## 3. Change classes

| Change | Class | Requires |
|---|---|---|
| New requirement | major | rationale, trace, conformance impact |
| Stricter requirement | major | migration note for implementers |
| Weaker requirement | major | justification; it removes an obligation |
| New conformance level | major | evidence requirements per level |
| Terminology change affecting an obligation | major | glossary diff |
| New example, threat, or domain | minor | none |
| Editorial | patch | none |

## 4. Deprecation

A requirement is deprecated before it is withdrawn. Deprecation states the successor
and the earliest version in which withdrawal may occur. At least one minor version
must elapse.

## 5. Publication

A published version is immutable. Corrections are published as a new patch version
with a changelog entry. A DOI is minted per published version; the concept DOI
resolves to the latest.

## 6. Relationship to the implementation

The reference implementation declares which specification version it targets. A
mismatch between implementation and specification version is a defect to be recorded,
not resolved by editing the specification to match the code.

## 7. Changelog

| Version | Date | Change |
|---|---|---|
| 0.1.1 | 2026-10-25 | Editorial correction, no obligation changed: the level table in `00-scope.md` now lists all six levels and defers to this document; the claim format accepts `1-6`; `09-conformance.md` gains a Level 6 evidence row and binds EG-080–EG-082 to every level; the unsettled interval enters scope (`00-scope.md` §2, §4); `10-non-claims.md` §4 renumbered 16–19; the stale EG-052 'not implemented' row and the matching T11 residual are corrected against `rigorix doctor`; `05-evidence-model.md` no longer implies PyPI verifies effect evidence; requirement count corrected to 55. |
| 0.1 | 2026-10-19 | Initial draft: terminology, model, 55 numbered requirements, evidence levels, threat model, conformance levels, non-claims. |
