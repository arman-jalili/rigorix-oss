# ADR-022's citations resolve, and a guard makes the next one land

ADR-2. Every reference in ADR-022 was checked against the cited document's own title. All four in the Related block were wrong — not subtly, but pointing at unrelated decisions:

| cited | label given | that ADR actually is | where the subject lives |
|---|---|---|---|
| ADR-011 | crate identity and release train | Approval Binding | no ADR; `validate-release-train.sh` enforces it |
| ADR-012 | failure semantics | Identity Attestation | ADR-009 (Error Handling Pattern) |
| ADR-013 | who verifies | Sequence Policy | ADR-012 (OSS attests, enterprise authorizes) |
| ADR-014 | portable slice and pinned inputs | Effect Identity Matching | `docs/spike-portable-slice-2026-11.md` |

Two of the four subjects have **no ADR at all**; naming a number for them was the mistake. The block now states the two real ADRs, adds ADR-023 (the sink boundary this artifact sits inside), and points at the documents that hold the other two.

The body had the same failure in a sharper form: `ADR-012 tier 3 says a trap is fatal…`. No document has tiers — `grep -rn 'tier 3' engine/.pi docs` returned only that line — and "a trap is not a refusal" is §3 of that very record. Now §3, with ADR-009 named for the failure vocabulary and `docs/binding-responsibility.md` for the promise to embedders. The `proc_exit` row that repeated `(ADR-012)` says §3 too.

§6 claimed "the same six rows" over a five-row table: six is the **import** count, five the **behaviour** count (`wasi/ci/embedder-contract.json` has six entries, the prose table five). An implementer told "six rows" looks for a row that is not there. Now "five behaviours across six imports", borrowing the phrasing `docs/binding-responsibility.md` already gets right.

## The part that outlives the fix: `engine/.pi/scripts/validate-adr-references.sh`

The same defect appeared three times in one document and nothing was watching, so the acceptance's fourth criterion — "the fix is checked" — becomes a gating guard (registered in `.pi/guards.tsv`). Five rules, each with a seed:

1. a cited ADR exists;
2. the label names something in that ADR's **subject**, read from its `# ` title, its `## Title` section (ADR-014/015/016 are stubs whose subject is there), or its filename;
3. a label that names the *interaction* instead is allowed and must be **recorded with a reason** in `engine/.pi/architecture/adr-citation-labels.tsv` — a judgement with no record is the silence this rule removes. Two rows, each stating why;
4. `ADR-nnn §N` must resolve to a `## N.` heading (bare `§N` against its own document);
5. a cited `ADR-nnn-<slug>.md` must exist, resolved against the three roots it can mean (repo root, crate root, `decisions/`).

| seed | expected | observed |
|---|---|---|
| control: a label matching the target's title | pass | pass |
| a label that names nothing in the target | violation | violation |
| the same label, recorded with a reason | pass | pass |
| a recorded row with no reason | malformed (exit 2) | exit 2 |
| a citation to a missing ADR number | violation | violation |
| a `§` that exists / one that exists nowhere | pass / violation | pass / violation |
| a cited filename that exists / does not | pass / violation | pass / violation |
| an ADR directory with no ADRs | malformed | exit 2 |

Two of those caught **my own** work, which is the point: a citation wrapped across lines (`ADR-014 (portable slice and\npinned inputs)`) was being skipped — a scanner that misses a wrapped label is precisely the vacuous check this replaces — and my first replacement label (`ADR-009 (failure semantics)`) failed rule 2, because "failure semantics" is a *descriptive* label for Error Handling. That is what the recorded-labels file is for.

Sweeping the whole tree with the guard found three more, fixed here because the sentence was wrong in the same way:

- **ADR-012** cited `ADR-005` twice — "OSS MCP transport auth" and "the enterprise JWT deferral" — and in this repository `ADR-005` is Event Bus. The number resolved; the document did not. It now names the enterprise record as the enterprise's, and says why.
- **ADR-020 / ADR-021** labelled ADR-016 as "the chained-log pattern and its boundary", which is *their own* subject. Both now name ADR-016's real one: the ledger this log projects.

Result: **65 labelled citations resolve, 3 descriptive labels recorded, 9 seeds rejected.**

## What this does not do

- It does not unify ADR numbering. Each tree carries its own set — `wasm/.pi/architecture/decisions/ADR-014` is "portable slice and pinned inputs", `engine/.pi/.../ADR-014` is "Effect Identity Matching" — so a bare number is tree-local by design. Rule 5 states it, and a path-qualified citation resolves in the tree it names.
- It does not touch the eight `ADR-001-auth-pattern.md`-style paths in `*/prompts/issue-template.md` and `*/INDEX.md`. Those were checked: they are **placeholder examples in the Guardian framework's prompt templates**, not citations of this repository's records. A guard that failed them would be refusing the template's illustration.
- It does not change ADR-022's normative content, as filed.

## Validation

```
cargo build --workspace                                   PASS
cargo test --workspace --all-targets --no-fail-fast          PASS (98 test binaries)
cargo clippy --workspace --all-targets -- -D warnings        PASS
cargo fmt --check                                            PASS
cargo audit                                                  no advisories
engine/.pi/scripts/validate-adr-references.sh (+ --self-test) PASS (65 citations, 9 seeds)
engine/.pi/scripts/validate-adr-references.sh --report        PASS
.pi/scripts/validate-guards.sh                                PASS (new guard gating)
.pi/scripts/validate-docs.sh                                  PASS
.pi/scripts/validate-decision-expiry.sh                       PASS
engine/.pi/scripts/validate-canonical.sh                      PASS
engine validators (ci, tests, security, operations, integration, canonical, architecture)   PASS
```

Closes #1505.