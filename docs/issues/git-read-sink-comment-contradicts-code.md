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

## What you are looking at

The repository's strongest structural guard is an inverted default: `disallowed_types = "deny"` in
`[workspace.lints.clippy]`, with roughly twenty sites carrying `#[allow(clippy::disallowed_types)]`
and a written reason each. **Those comments are the audit trail.** They are what a reviewer, an
auditor, or a future agent reads to decide whether an exception is justified.

One of them is wrong. `engine/src/tools/application/git_read_tool.rs:22`:

```rust
// #1083: approved process-spawning sink — `sh -c` for a git read; the caller-supplied path must be argv, not interpolated (#1082).
// See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
#![allow(clippy::disallowed_types)]
```

The code is not `sh -c`, and the same file says so twenty lines earlier:

```rust
//! run: `path = "x; touch pwned"` passed the check and then executed. argv
//! removes the interpreter from the path, so a metacharacter is just a byte in
//! an argument.                                            (lines 18-20)
```

The implementation agrees with the module doc, not with the allow comment:

```rust
let mut cmd = tokio::process::Command::new("git");   // :142 — argv, no interpreter
// --output, -c and --exec are refused
```

So the file contradicts itself three ways, and the wrong statement is the one attached to the
security exception.

## Why a comment counts as a defect

The guard works by making every exception visible and requiring a justification. That only holds if
the justification is true. A wrong one is worse than none: an absent reason prompts a question; a
confident wrong reason answers it incorrectly.

In this case the wrong reason describes exactly the vulnerability class the guard exists to prevent
(shell interpolation), attached to a comment claiming that is what the code does. Anyone auditing
sink behaviour from the comments — which is the intended workflow — will conclude this tool runs a
shell, and will either "fix" working code or fail to notice a real shell sink written in the same
style.

## In Scope

- Correct the comment at `git_read_tool.rs:22` to describe argv construction
- Verify `.pi/architecture/modules/tool-system.md#process-spawning-sinks` says the same for git reads
  (the comment points there; if the inventory repeats the error, fix both in this MR)
- Sweep the other `#[allow(clippy::disallowed_types)]` sites: read each comment against its code, and
  report the result in the MR description (expected: they agree; say so explicitly)

## Out of Scope

- **Any implementation change.** The code is correct.
- Reducing the number of allow sites
- Adding a guard that text-matches comments against code (see Pitfalls)
- The `#1082` / `#1083` history

## Affected Layers

### Domain / Application / Infrastructure / API
- None. This is a comment and, at most, one inventory row.

## Canonical References

| what | where |
|---|---|
| the wrong claim | `engine/src/tools/application/git_read_tool.rs:22` |
| the right claim, same file | `git_read_tool.rs:18-20` |
| the code | `git_read_tool.rs:142` (and the argv assembly through `:210`) |
| the inventory the comment cites | `.pi/architecture/modules/tool-system.md#process-spawning-sinks` |

## Acceptance Criteria

| # | Criterion | How it is checked |
|---|-----------|-------------------|
| 1 | The comment describes argv, not `sh -c` | review |
| 2 | The inventory entry agrees with the comment | `validate-canonical.sh` |
| 3 | Other allow sites checked; the result is stated in the MR | review |
| 4 | No implementation change appears in the diff | `validate-ci.sh` + diff review |

## Implementation

1. Read `git_read_tool.rs:1-45` — module docs and the allow comment, in one view.
2. Read `git_read_tool.rs:130-215` — the command construction and the refused flags.
3. Rewrite the comment to describe argv. Keep the issue references (`#1083`, `#1082`) and the
   inventory pointer — the only thing that is wrong is the mechanism.
4. Open `tool-system.md#process-spawning-sinks`; compare the git-read entry.
5. `grep -rn 'allow(clippy::disallowed_types)' --include=*.rs .` and read each comment against its
   code. Record the list in the MR description with one line per site.
6. Run validators.

## Pitfalls

- **Do not add a comment-vs-code text guard.** It would be brittle (comments and code change
  independently) and would produce a false green: a comment can name `argv` while the code
  interpolates. The real guard is the deny-by-default lint plus a reason that review keeps honest.
- **Do not "fix" the code.** It is argv and correct. The defect is the description.
- **Do not let the diff grow.** If the sweep finds another wrong comment, decide explicitly: fix it
  here (still comment-only) or file it separately and say which.
- **If the inventory is also wrong, fix both now** — otherwise the citation and the file disagree
  again, which is the state this issue is about.

## Why this is medium

No code is wrong, and nothing behaves badly. But the file that justifies an exception to the
repository's strongest guard tells a reviewer the opposite of what the file does — and a trusted
justification is the one mechanism that guard has.
