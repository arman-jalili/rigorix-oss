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
| 0.4.0 | 2026-11-10 | **Major change class, drafted as a minor of the 0.x review series** (the index records the status; 1.0.0 is the first publication). Eight requirements added as §K (RS-099–RS-108): the declared isolation before a step that can execute (RS-099); the declaration is not containment (RS-100); the spawn set derivable from source (RS-101); an unrecognized regime is a startup error (RS-102); every boundary crossing inventoried with what it does not do (RS-103); validation and mutation share a handle (RS-104); evidence completeness as a status (RS-105); the artifact verified at load with the digest's build context (RS-106). Two threat classes added (T13 repository-configured execution, T14 artifact substitution), five non-claims (21–25), a binding form in `09-conformance.md` §1b, and `05-evidence-model.md` §5a. Conformance impact: **no level is added or withdrawn** — the new requirements are the obligations behind capabilities Level 1–6 already named, and a deployment that met them by construction does not move level. Companion: Consequence Gating **v2.0** (new document; v1.0 remains valid for precondition-only claims). Trace: ADR-022, ADR-023; `rigorix-oss` #1478–#1502, #1455/#1456 (gates 3 and 4). The published copy on the publishing site must be re-synced in the same release, with its claim-coverage check re-run. |
| 0.3.0 | 2026-11-05 | **Minor** — records what Phase C of the reference implementation made true, with no obligation added or made stricter. `09-conformance.md` §1a defines the producer and verifier deployment modes and binds a producer-only deployment (the wasm build, ADR-013) to *not* claiming Level 4; `05-evidence-model.md` §2 states that a decision record's `time` is the instant it was evaluated with and that verification replays with that instant, and that a refusal is a decision while an unavailable evaluation is not; `01-terminology.md` gains *Refusal* and the corresponding note on *Precondition*; `11-traceability.md`'s RS-004 row cites ADR-014 §2, the golden digest and the `gate: "cli"` vectors. Conformance impact: none — the new text states what the requirements already implied (`RS-004`'s determinism, `RS-080`'s fail-closed reading of absence) and adds no level. Trace: #1385 (conformance produces CLI-labelled evidence) and #1387 (this record); the editable source is `docs/spec/execution-governance/`, and the published copy on the publishing site must be re-synced in the same release, with its claim-coverage check re-run. |
| 0.2.0 | 2026-10-25 | Requirements renumbered to the `RS-NNN` prefix; two-letter prefixes read as abbreviations in a citation, and a specification's identifiers should not be ambiguous. No obligation changed. |
| 0.1.1 | 2026-10-25 | Editorial correction, no obligation changed: the level table in `00-scope.md` now lists all six levels and defers to this document; the claim format accepts `1-6`; `09-conformance.md` gains a Level 6 evidence row and binds RS-080–RS-082 to every level; the unsettled interval enters scope (`00-scope.md` §2, §4); `10-non-claims.md` §4 renumbered 16–19; the stale RS-052 'not implemented' row and the matching T11 residual are corrected against `rigorix doctor`; `05-evidence-model.md` no longer implies PyPI verifies effect evidence; requirement count corrected to 55. |
| 0.1 | 2026-10-19 | Initial draft: terminology, model, 55 numbered requirements, evidence levels, threat model, conformance levels, non-claims. |
