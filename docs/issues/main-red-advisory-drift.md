---
guardian_issue:
  id: "ISSUE-MAIN-RED-ADVISORY-DRIFT"
  title: "main is red for dependency advisories, not code: ctutils 0.4.2 (patch available) fails Audit, and Hardening's --deny warnings fails on five unmaintained/unsound crates"
  epic: "release hygiene"
  epic_id: "EPIC-OPERATIONAL-USABILITY"
  tracking_issue: 1198
  component: "Docs"
  module: "operations"
  status: done
  closed_reason: "Implemented in #1201: ctutils 0.4.4, ratatui 0.30.2 drops lru 0.12/paste, and the remaining unmaintained advisories are reviewed ignores in .cargo/audit.toml tracked by #1199/#1200."
  priority: critical
  created_at: "2026-10-24T07:30:00Z"
  updated_at: "2026-10-24T09:00:00Z"

  intent: |
    Restore a green main by fixing the one actionable vulnerability and making an explicit decision about
    the five advisory warnings that Hardening now denies.

    `CI` run 39923228344 on `8e21c6e0`: **every job green except `🔒 Audit`** — all Lint/Build/Test for
    actions, cli, engine, mcp, server pass, which confirms #1195/#1196 are sound. The Audit job fails at
    its first step, `cargo audit`. Reproduced locally:

    ```
    error: 1 vulnerability found!
      RUSTSEC-2026-0385  ctutils 0.4.2  non-constant-time mask generation  patched >=0.4.4
    ```

    Chain: `ctutils ← digest ← hmac ← {engine, actions, cli, mcp, server}` — i.e. every crate in the
    workspace. **A fix is available**: `cargo update -p ctutils --precise 0.4.4` resolves cleanly.

    `Hardening` also runs `cargo audit … --deny warnings` (`hardening.yml:222`), which additionally
    fails on five advisories:

    | crate | advisory | kind | chain |
    |---|---|---|---|
    | `unsafe-libyaml` | RUSTSEC-2024-0449 | unmaintained | `serde_yaml 0.9.34+deprecated ← rigorix-actions` |
    | `lru` | RUSTSEC-2026-0253 | unsound (UAF in `LruCache::pop`) | `ratatui 0.29 ← rigorix-cli` |
    | `zbus_names` | RUSTSEC-2026-0371 | unmaintained | Linux-only keyring chain, present in the lockfile |
    | `zvariant` | RUSTSEC-2026-0370 | unmaintained | same |
    | `zvariant_derive` | RUSTSEC-2026-0372 | unmaintained | same |

    `.cargo/audit.toml` states the project's position: *"Intentionally empty: no advisory ignores. The
    project prefers an honest failing audit over silently suppressing advisories. Remaining findings are
    documented in the gap-ledger."* That is a good policy, and it is why this issue must end in a
    decision rather than an ignore-by-reflex.

    **This is advisory drift, not a regression from #1195/#1196.** These advisories were published
    after the last green run; the batch's local verification cannot see them, because an advisory feed is
    a *time* input rather than a code input.

  dependencies:
    - name: "cargo audit in CI"
      type: internal
      note: "ci.yml:179 (vulnerabilities only); hardening.yml:222 (--deny warnings)."
    - name: ".cargo/audit.toml"
      type: internal
      note: "the no-ignore policy; any suppression has to be a documented decision, not a flag."
    - name: "dependency chains"
      type: internal
      note: "ctutils ← digest ← hmac; unsafe-libyaml ← serde_yaml ← actions; lru ← ratatui ← cli; zbus/zvariant ← keyring (Linux)."

  in_scope:
    - "Bump ctutils to >=0.4.4 (one `cargo update`), verify the workspace builds and tests pass"
    - "Decide per remaining advisory: upgrade, remove the dependency, or record a reviewed ignore in `.cargo/audit.toml` with a rationale and a tracking issue"
    - "If a warning class is deliberately tolerated, scope the deny list so Hardening does not fail forever on `unmaintained` while denying real vulnerabilities"
    - "Re-run Audit and Hardening to green on main"

  out_of_scope:
    - "Removing the TUI (ratatui) or the keyring feature"
    - "Replacing serde_yaml inside actions without a migration plan"
    - "Adding a scheduled advisory job (worth doing, but a separate change)"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: Cargo.lock (ctutils); possibly Cargo.toml for an upgrade"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/operations.md"
    - adr:
        - ".pi/architecture/decisions/ADR-009-error-handling.md"
    - last_sync: "2026-10-24"
    - evidence: "cargo audit: RUSTSEC-2026-0385 ctutils 0.4.2, patched >=0.4.4"
    - evidence: "cargo update -p ctutils --precise 0.4.4 → 'Updating ctutils v0.4.2 -> v0.4.4'"
    - evidence: "cargo audit --deny warnings: 5 denied advisories (unsafe-libyaml, lru, zbus_names, zvariant, zvariant_derive)"
    - evidence: ".cargo/audit.toml — 'no advisory ignores … documented in the gap-ledger'"
    - evidence: "ci.yml:179 and hardening.yml:222 — the two audit invocations"
    - evidence: "CI run 39923228344: only 🔒 Audit failed; all Lint/Build/Test jobs green"

  acceptance_criteria:
    - "`cargo audit --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0002` reports 0 vulnerabilities"
    - "`Hardening`'s audit step passes, or its deny scope is explicitly narrowed with a recorded reason"
    - "Every remaining advisory has a named disposition: upgraded, removed, or ignored with a rationale and an issue"
    - "No ignore is added without a comment saying why and when it expires"
    - "CI and Hardening are green on main"

  validators:
    - ci
    - security

  implementation_notes: |
    Fix `ctutils` first — it is a real vulnerability, a patch exists, and the diff is a lockfile line.
    Do not co-mingle that with the harder policy question; get the vulnerability cleared, then decide the
    warnings.

    The five warnings are three different problems: `serde_yaml` is deprecated (the dependency should go,
    not the advisory), `ratatui 0.29` pins `lru 0.12` (a minor bump may drop it), and the `zvariant`
    trio comes from the Linux keyring chain (a feature or version decision). Treating them as one
    "ignore list" would hide three separate follow-ups.

    If a suppression is genuinely right, put it in `.cargo/audit.toml` with a comment and a tracking
    issue rather than in a workflow command line — the toml is reviewed with the code, and the current
    flags are duplicated across two workflows where they can drift.

    `--deny warnings` is what makes `unmaintained` block a build. That is a legitimate policy, but it
    means every future unmaintained notice stops the train. If that is not intended, deny
    `vulnerability` and `unsound` explicitly and let `unmaintained` be advisory — and say so in the
    workflow comment.

  file_changes:
    - "modify: Cargo.lock (ctutils >= 0.4.4)"
    - "modify: .cargo/audit.toml (only if a reviewed ignore is chosen)"
    - "modify: .github/workflows/hardening.yml (only if the deny scope is narrowed)"
    - "modify: engine/.pi/architecture/gap-ledger.md (record any accepted advisory)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1198: red main from advisory drift, not from code

## What you are looking at

| job | state on 8e21c6e0 |
|---|---|
| all Lint / Build / Test (5 crates) | ✅ |
| SDK-schema, Docs, Coverage, Workspace | ✅ |
| 🔒 **Audit** | ❌ `cargo audit` |
| Hardening `🔒 Security` | ❌ (same invocation, plus `--deny warnings`) |

```
RUSTSEC-2026-0385  ctutils 0.4.2  →  patched >=0.4.4          ← actionable now
RUSTSEC-2024-0449  unsafe-libyaml  unmaintained  ← serde_yaml
RUSTSEC-2026-0253  lru             unsound       ← ratatui 0.29
RUSTSEC-2026-037{0,1,2}  zvariant, zbus_names, zvariant_derive  unmaintained  ← keyring (Linux)
```

## In Scope

- ctutils bump; a named disposition per remaining advisory; green Audit + Hardening

## Out of Scope

- Dropping ratatui/keyring; migrating serde_yaml inside actions; a scheduled audit job

## Affected Layers

### Infrastructure
- `Cargo.lock`; possibly `.cargo/audit.toml`, `hardening.yml`

## Canonical References

- `.cargo/audit.toml` (the no-ignore policy)
- `ci.yml:179`, `hardening.yml:222`
- Evidence above

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | 0 vulnerabilities | `cargo audit` |
| 2 | Hardening audit passes or deny scope narrowed with a reason | CI |
| 3 | Every advisory has a disposition | review |
| 4 | No silent ignore | `.cargo/audit.toml` review |
| 5 | CI + Hardening green | CI |

## Implementation

1. `cargo update -p ctutils --precise 0.4.4`; build + test the workspace.
2. For each remaining advisory, choose upgrade / remove / reviewed ignore (+ gap-ledger entry).
3. Re-run both audit invocations until green; push and confirm CI + Hardening.

## Pitfalls

- **Do not add `--ignore` flags to the workflows** — the project's policy lives in `.cargo/audit.toml`,
  and duplicating flags across two files is how they drift.
- **Do not lump five advisories into one disposition**; they are three different dependency problems.
- **Do not narrow `--deny warnings` silently** — if `unmaintained` stops blocking, say so in the
  workflow and the gap ledger.
- **`ctutils` is a real vulnerability in every crate**; fix it before debating the rest.

## Why this is critical

Main is red and the release train is stopped, for reasons that have nothing to do with the latest
changes — and local verification cannot see them, because an advisory feed changes with time rather
than with the commit.
