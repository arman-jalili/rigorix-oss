---
guardian_issue:
  id: "ISSUE-EFFECT-SCOPE-ORACLE-UNWIRED-AND-NARROW"
  title: "approval: the git effect-scope oracle is disabled in every production entry point, and its diff misses staged changes and uses one baseline per loop"
  epic: "consequence gating"
  component: "EffectScopeOracle"
  module: "approval"
  status: planned
  priority: high
  dependencies:
    - "ADR-011 — effect-scope verification via git-diff oracle"

  intent: |
    Stop advertising effect-scope evidence that no supplied entry point produces, and fix the
    oracle's blind spots before wiring it.

    `approval_repo_path` is `None` in the CLI and the GitHub Action and defaults to `None` in the
    MCP host. The dispatch loop performs the post-execution scope check only when that path is
    configured, so in every shipped composition root the check never runs. ADR-011 §5 describes
    effect-scope verification as the coverage for the gap that binding cannot close.

    The oracle itself is also narrower than it reads: it uses `git diff --name-only` (so staged
    changes are invisible), compares path **sets** rather than per-step content, and takes one
    baseline per dispatch loop rather than per step.

  in_scope:
    - "Wire approval_repo_path in CLI, MCP host and Action, or document it as unsupported there"
    - "Include staged changes in the snapshot (git diff --cached / git status --porcelain)"
    - "Decide and document: per-loop baseline or per-step baseline"
    - "End-to-end test: staged change, committed change, pre-existing dirty tree, concurrent steps"
    - "State the claim ceiling in ADR-011 and the module doc: evidence, not prevention"

  out_of_scope:
    - "Replacing git with a filesystem watcher"
    - "Content-level (not path-level) comparison — record it as a known limit unless it is cheap"
    - "Sandboxing the command (separate issue)"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: ChangeSnapshot — include staged changes"
      - "Modified: dispatch — baseline scope (per loop or per step), explicit"
    infrastructure:
      - "Modified: effect_scope git invocation"
    api:
      - "Modified: composition roots — configure the repo path"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - evidence: "engine/src/execution_engine/domain/parallel_executor.rs:78,92 — approval_repo_path: Option<String>, default None"
    - evidence: "cli/src/cli_boundary/orchestrator.rs:356 — approval_repo_path: None"
    - evidence: "actions/src/main.rs:430 — approval_repo_path: None"
    - evidence: "engine/src/approval/infrastructure/effect_scope.rs:29 — 'git diff --name-only' (no --cached), so staged changes are excluded"
    - evidence: "engine/src/execution_engine/application/service_impl/dispatch.rs:80 — one scope_baseline per loop, compared at :371 per step"

  acceptance_criteria:
    - "Either the shipped entry points configure the oracle, or the docs say they do not"
    - "A staged modification is detected as an out-of-scope change"
    - "The baseline scope (loop vs step) is explicit and tested"
    - "A test covers a pre-existing dirty tree (the baseline must not report the author's own edits)"
    - "Concurrent steps are attributed correctly, or the misattribution is documented"
    - "ADR-011 and the module doc state evidence-not-prevention"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - Wiring it without fixing the baseline will produce false positives on any repo that is
      dirty at startup. Fix the snapshot first, then wire.
    - `git diff --name-only` shows unstaged tracked modifications only. Staged edits and
      untracked files need `--cached` and `ls-files --others`. The current code already does the
      untracked half; the staged half is missing.
    - Per-step baselines are the correct semantics for attributing a change to a step, but they
      cost a snapshot per step and race with concurrent steps. If per-loop is kept, say that a
      reported violation identifies the loop, not the step.
    - The honest alternative in the issue is acceptable: document it as an optional, currently
      limited evidence capability and remove the implication that it is produced in normal
      deployments.

  file_changes:
    - "modify: engine/src/approval/infrastructure/effect_scope.rs"
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs"
    - "modify: mcp/src/host/mod.rs"
    - "modify: actions/src/main.rs"
    - "create: engine/tests/effect_scope_e2e.rs"
    - "modify: engine/.pi/architecture/decisions/ADR-011-approval-binding.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-EFFECT-SCOPE-ORACLE-UNWIRED-AND-NARROW

## 1. It never runs in a shipped configuration

| entry point | `approval_repo_path` |
|---|---|
| CLI (`cli_boundary/orchestrator.rs:356`) | `None` |
| GitHub Action (`actions/src/main.rs:430`) | `None` |
| MCP host (`parallel_executor.rs:92`) | default `None` |

`dispatch.rs:80` builds the baseline only when the path is set; `:371` performs the comparison
under the same condition. So the oracle exists, has unit tests, and is inactive everywhere it
would matter.

ADR-011 §5 presents effect-scope verification as the coverage for the gap binding cannot close
(binding governs dispatched tool calls, not arbitrary process behaviour). In the OSS default
that coverage is not produced.

## 2. The oracle is narrower than it reads

```rust
// effect_scope.rs:29
/// Paths with tracked modifications (`git diff --name-only`).
```

- **Staged changes are invisible.** `git diff --name-only` reports unstaged tracked
  modifications; `--cached` is needed for the index. A step that stages its edit and commits
  later slips past.
- **Paths, not content.** A modification inside an already-changed file is not distinguishable
  from the original change.
- **One baseline per loop** (`dispatch.rs:80`), compared for every step (`:371`). With concurrent
  steps, a violation cannot be attributed to a step; with a pre-existing dirty tree, the author's
  own uncommitted work is reported as the agent's.

## Suggested fix, in this order

1. Include staged and untracked changes in the snapshot.
2. Decide the baseline scope and state it (per-step attribution, or per-loop attribution).
3. Then wire the entry points — wiring a mis-baselined oracle produces false positives on any
   dirty repo.
4. Alternatively, keep it off and say so plainly in ADR-011 and the module doc.

Option 4 is legitimate. What is not legitimate is describing effect-scope verification as
coverage while every entry point leaves it `None`.
