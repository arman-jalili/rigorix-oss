---
guardian_issue:
  id: "ISSUE-CHANGELOG-UNRELEASED-EMPTY"
  title: "the CHANGELOG's Unreleased section is empty although a whole context, an enforcement fix and three issue batches landed since 1.10.0"
  epic: "release hygiene"
  epic_id: "EPIC-OPERATIONAL-USABILITY"
  tracking_issue: 1178
  component: "Docs"
  module: "effect-commitment"
  status: planned
  priority: low
  created_at: "2026-10-23T15:00:00Z"
  updated_at: "2026-10-23T15:00:00Z"

  intent: |
    Record what shipped since 1.10.0 in the CHANGELOG's `[Unreleased]` section, before the next bump
    turns it into a release note someone has to reconstruct.

    `CHANGELOG.md:16` is `## [Unreleased]` followed immediately by `## [1.10.0]` — no entries. Since
    1.10.0 the repository gained: the `effect-commitment` bounded context (ADR-019, 24 issues
    #1112-#1135), four runtime-gap fixes (#1163-#1166), the recovery-outcome change (#1157), the
    `--enforcement` refusal (#1156, #1171) and the allow-comment correction (#1159).

    This is not the "no 1.10.0 section" claim, which is false — that section exists and is complete
    (`:18-46`, ten closed issues). The gap is the section *after* it.

  dependencies:
    - name: "CHANGELOG.md"
      type: internal
      note: "Unreleased at :16; 1.10.0 at :18 — the format to follow is directly below."
    - name: "release process"
      type: internal
      note: "RELEASING.md / publish workflow — decide whether the note is written at merge time or at release time."

  in_scope:
    - "Add Unreleased entries for the effect-commitment context and the three fix batches"
    - "Group as Added / Fixed, with issue and PR links, matching the 1.10.0 section's shape"
    - "State in RELEASING.md when the Unreleased section is expected to be filled"

  out_of_scope:
    - "Writing the 1.10.0 notes (already there)"
    - "Choosing the next version number"
    - "Retroactive notes for releases before 1.9.5"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "CHANGELOG.md:16 — '## [Unreleased]' with nothing under it"
    - evidence: "CHANGELOG.md:18-46 — the 1.10.0 section, complete with PR links and closed issues"
    - evidence: "#1112-#1135 (effect-commitment epic), #1156, #1157, #1159, #1163-#1166, #1171"

  acceptance_criteria:
    - "Unreleased lists the shipped changes with issue/PR links"
    - "Format matches the 1.10.0 section (Added / Fixed; grouped detail block)"
    - "RELEASING.md says when the section is filled"
    - "No claim appears in the CHANGELOG that the code does not support"

  validators:
    - architecture-validator
    - canonical

  implementation_notes: |
    Write the entries from issue titles and PR numbers, not from memory: `gh issue list --state closed`
    and the merge commits give the exact set. The 1.10.0 section is the template — it includes a
    collapsible list of closed issues, which is what makes a release note checkable later.

    Keep the wording at the level the file uses: what a user can now do, not which files changed. And
    do not promote anything to a headline claim that the runbook qualifies (effect detection being the
    live example — see #1175).

  file_changes:
    - "modify: CHANGELOG.md"
    - "modify: RELEASING.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1178: the release note for the current cycle does not exist

## What you are looking at

```
## [Unreleased]          ← :16, empty

## [1.10.0] - 2026-10-14 ← :18, complete: Added, Fixed, 10 closed issues
```

Everything since 1.10.0 — a new bounded context, a crash-window fix, a security-flag refusal, a
recovery-outcome type, and an allow-comment correction — is in git history and in issues, not in the
file a release is cut from.

## In Scope

- Unreleased entries with links
- A stated expectation in RELEASING.md

## Out of Scope

- Rewriting 1.10.0 (present and correct); version choice; older releases

## Affected Layers

### API
- None — documentation

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **Evidence:** `CHANGELOG.md:16`, `:18-46`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Unreleased entries present with links | `validate-canonical.sh` |
| 2 | Format matches 1.10.0 | review |
| 3 | RELEASING.md states the expectation | review |

## Implementation

1. `gh issue list --state closed` since 2026-10-14; collect PR numbers from the merge commits.
2. Write Added / Fixed; add the collapsible closed-issues block.
3. Note the expectation in RELEASING.md.

## Pitfalls

- **Do not claim more than the runbook qualifies** — #1175 is the cautionary case.
- **Write from the issue list, not from memory**; the count is part of the record.
- **Do not include the unpublished `rigorix-server`** in the release train.

## Why this is low

Nothing is wrong with the code or the docs' accuracy. But a release cut from an empty Unreleased
section means the notes are reconstructed after the fact — which is how the 1.10.0 section came to be
the only complete record of that release.
