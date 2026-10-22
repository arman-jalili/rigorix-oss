---
guardian_issue:
  id: "ISSUE-GIT-READ-ALLOW-COMMENT-WRONG"
  title: "git_read_tool: the justification comment on its process-spawning allow says `sh -c`, while the code is argv — the audit trail for the repo's strongest guard describes the pattern the guard exists to prevent"
  epic: "guard integrity"
  epic_id: "EPIC-FAILURE-CLASS-GUARDS"
  tracking_issue: 1159
  component: "Tools"
  module: "tool-system"
  status: planned
  priority: medium
  created_at: "2026-10-23T06:00:00Z"
  updated_at: "2026-10-23T06:00:00Z"

  intent: |
    Correct the audit comment on an allowlist site, and make sure the inventory it points at agrees.

    The site, `engine/src/tools/application/git_read_tool.rs:22`:

    ```rust
    // #1083: approved process-spawning sink — `sh -c` for a git read; the caller-supplied path must be argv, not interpolated (#1082).
    // See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
    #![allow(clippy::disallowed_types)]
    ```

    The code is not `sh -c`. It is argv:

    ```rust
    let mut cmd = tokio::process::Command::new("git");   // :142
    ```

    and the same file's module docs say precisely why:

    > "…handed to a shell, so the first token allowlist validated a string the executor did not run
    > … **argv removes the interpreter from the path**, so a metacharacter is just a byte in an
    > argument."

    So the file contradicts itself, and the wrong half is the one attached to
    `#[allow(clippy::disallowed_types)]`.

    Why that matters more than a typo: the repo's highest-leverage guard is the inverted default
    (`disallowed_types = "deny"`), whose 20 allow sites are justified by comments that point at the
    inventory in `tool-system.md#process-spawning-sinks`. **The comment is the audit trail.** An
    auditor — or a future agent told to "prefer the type over the guard" — reads the justification
    and concludes this sink runs a shell. It does not. A wrong justification on an allow site is
    worse than no justification, because it is trusted.

  dependencies:
    - name: "clippy disallowed_types allowlist"
      type: internal
      note: "engine/Cargo.toml `[workspace.lints.clippy] disallowed_types = deny`; ~20 sites with reasoned allows."
    - name: "tool-system.md#process-spawning-sinks"
      type: internal
      note: "The inventory every allow comment points at; verify its entry for git reads matches argv too."
    - name: "guard_suite.rs"
      type: internal
      note: "engine/tests/guard_suite.rs — where a check that an allow comment matches the mechanism would live."

  in_scope:
    - "Correct the comment at git_read_tool.rs:22 to describe argv"
    - "Verify the corresponding entry in tool-system.md#process-spawning-sinks says the same"
    - "Check the other allow sites' comments for the same mismatch (claim: they describe their mechanism)"

  out_of_scope:
    - "Changing any implementation — the code is already correct"
    - "Reducing the number of allow sites"
    - "Adding a text-matching guard for comments (a comment cannot be reliably validated; see notes)"

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
    - module: ".pi/architecture/modules/tool-system.md"
    - adr:
        - ".pi/architecture/decisions/ADR-009-error-handling.md"
    - last_sync: "2026-10-23"
    - evidence: "engine/src/tools/application/git_read_tool.rs:22 — the `sh -c` claim"
    - evidence: "engine/src/tools/application/git_read_tool.rs:18-20 — the module doc: argv removes the interpreter"
    - evidence: "engine/src/tools/application/git_read_tool.rs:142 — `Command::new(\"git\")`"
    - evidence: "engine/src/tools/application/git_read_tool.rs:142-210 — argv construction; `--output`, `-c`, `--exec` refused"
    - evidence: "tool-system.md#process-spawning-sinks — the inventory the comment points at"

  acceptance_criteria:
    - "The comment at git_read_tool.rs:22 describes argv, not `sh -c`"
    - "The inventory entry for git reads agrees with the comment"
    - "No other allow-site comment claims a mechanism its code does not use (checked, with the check recorded in the MR)"
    - "validate-ci.sh and validate-canonical.sh pass; no implementation change is in the diff"

  validators:
    - architecture-validator
    - ci
    - canonical

  implementation_notes: |
    **A one-line fix, and the reason it is worth an issue.** This could have been a commit without a
    ticket. It gets one because the class it belongs to — *the justification for a security exception
    is wrong* — is not self-announcing, and recording it establishes that allow-site comments are
    maintained artifacts rather than decoration.

    Do not add a guard that text-matches the comment against the code. It would be brittle (comments
    and code change independently) and it would produce a false green: a comment can name `argv` and
    the code can still interpolate. The guard that matters already exists — `disallowed_types = deny`
    plus the per-site reason. Keep the reason honest by review.

    While here, confirm the inventory entry. If `tool-system.md#process-spawning-sinks` says `sh -c`
    for git reads, the same correction applies there, and the two must not drift — that inventory is
    what every other allow site points at.

  file_changes:
    - "modify: engine/src/tools/application/git_read_tool.rs (comment only)"
    - "modify: engine/.pi/architecture/modules/tool-system.md (only if the inventory entry agrees with the wrong comment)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1159: the allow-site comment says `sh -c`; the code is argv

## Intent

Correct the justification attached to a security allow, and check the inventory it cites.

## The three-way disagreement

| source | says |
|---|---|
| `git_read_tool.rs:22` (the allow comment) | "`sh -c` for a git read" |
| `git_read_tool.rs:18-20` (module doc) | "argv removes the interpreter from the path" |
| `git_read_tool.rs:142` (the code) | `Command::new("git")` with argv |

Two of three are right. The wrong one is the one a reviewer reads to justify the exception.

## In Scope

- Fix the comment
- Verify `tool-system.md#process-spawning-sinks` agrees
- Check the other allow sites for the same mismatch, report in the MR

## Out of Scope

- Any implementation change (the code is already correct)
- Text-matching guards for comments

## Affected Layers

### Domain / Application / Infrastructure / API
- None — comment-only change

## Canonical References

- **Module:** `.pi/architecture/modules/tool-system.md`
- **Evidence:** `git_read_tool.rs:18-22,142`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Comment describes argv | review |
| 2 | Inventory entry agrees | `validate-canonical.sh` |
| 3 | Other allow sites checked, result in MR | review |
| 4 | No implementation change in the diff | `validate-ci.sh` |

## Implementation

> **Agent:** the diff should be one comment. If it grows, something else got swept in.

### Steps

1. Read `git_read_tool.rs:14-40` and `:130-215`
2. Correct the comment to describe argv
3. Open `tool-system.md#process-spawning-sinks`; compare the git-read entry
4. Grep the other `#[allow(clippy::disallowed_types)]` sites; skim each comment against its code
5. Report the sweep in the MR description

## Notes

- The comment is the audit trail for a deny-by-default guard; a wrong one is worse than none.
- Do not add a comment-vs-code text guard — it would be brittle and could report a false green.
- If the inventory entry is also wrong, fix both in this MR so they cannot drift.
- Comment-only diff; anything else belongs in a different issue.

## Why medium

No code is wrong. But the file's own justification tells a reviewer the opposite of what the file
does, and this is the file that justifies an exception to the repository's strongest guard.
