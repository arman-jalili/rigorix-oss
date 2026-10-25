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
| 0.1 | 2026-10-19 | Initial draft: terminology, model, 60+ requirements, evidence levels, threat model, conformance levels, non-claims. |
