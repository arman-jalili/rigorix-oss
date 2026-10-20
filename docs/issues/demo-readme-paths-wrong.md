---
guardian_issue:
  id: "ISSUE-DEMO-README-PATHS-WRONG"
  title: "demo/capacity-ledger/README.md points at two paths that do not exist — both are missing the engine/ prefix"
  epic: "docs hygiene"
  component: "Docs"
  module: "governance"
  status: planned
  priority: low
  dependencies:
    - "#1018 — the demo docs this lands in"

  intent: |
    Fix two prose paths in the demo README so a reader who follows them finds something.

    Both reference authoritative documents, both omit the `engine/` prefix, and both therefore
    resolve to nothing from the repo root. #1018's own acceptance criterion is "commands executed
    and output recorded; no non-derivable numbers" — the same discipline applies to the paths a doc
    tells a reader to open.

  in_scope:
    - "Correct `.pi/architecture/modules/capacity-ledger.md` to `engine/.pi/architecture/modules/capacity-ledger.md`"
    - "Correct `docs/runbook-capacity-ledger.md` to `engine/docs/runbook-capacity-ledger.md`"
    - "Consider making them markdown links, so validate-docs.sh covers them"

  out_of_scope:
    - "Extending validate-docs.sh to all backticked paths (see note: it would false-positive)"
    - "The demo itself"
    - "Moving the runbook or module doc to the repo root"

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
    - evidence: "demo/capacity-ledger/README.md:75 — `See `.pi/architecture/modules/capacity-ledger.md` §\"Guarantee ceiling\"`"
    - evidence: "demo/capacity-ledger/README.md:80 — `` `docs/runbook-capacity-ledger.md` §\"Observing refusals\" ``"
    - evidence: "actual locations: engine/.pi/architecture/modules/capacity-ledger.md and engine/docs/runbook-capacity-ledger.md"
    - evidence: ".pi/scripts/validate-docs.sh:12 — the broken-link check covers markdown link/image targets only; these are backticked prose, so it passes"

  acceptance_criteria:
    - "Both paths resolve from the repo root"
    - "validate-docs.sh still passes"
    - "No other relative path in demo/ is wrong (checked by resolving each, not by reading)"
    - "The fix does not introduce an absolute path"

  validators:
    - canonical
    - ci

  implementation_notes: |
    Prefer markdown links over backticks here. The documents being referenced are repository files,
    not illustrative names, so a link is both more useful to a reader and covered by the existing
    broken-link guard — which is precisely why the defect survived: backticks are invisible to it.

    Do **not** extend the validator to all backticked paths without care. `.rigorix/capacity.toml`
    appears in the same file and is legitimately absent from the repo — the demo creates it at
    runtime. A blanket rule would flag it, and the first allowlist entry would be the start of a
    rule nobody trusts. If this is worth guarding, it needs a way to mark "this path is created, not
    committed", which is a design decision rather than a one-line change.

  file_changes:
    - "modify: demo/capacity-ledger/README.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-DEMO-README-PATHS-WRONG

## The two paths

```markdown
# demo/capacity-ledger/README.md:75
first. See `.pi/architecture/modules/capacity-ledger.md` §"Guarantee ceiling".

# :80
`docs/runbook-capacity-ledger.md` §"Observing refusals".
```

Neither exists from the repo root:

| referenced | actual |
|---|---|
| `.pi/architecture/modules/capacity-ledger.md` | `engine/.pi/architecture/modules/capacity-ledger.md` |
| `docs/runbook-capacity-ledger.md` | `engine/docs/runbook-capacity-ledger.md` |

Both are missing `engine/`. The rest of the demo README's paths are correct, and the main
`README.md`'s links all resolve — this is limited to these two lines.

## Why the guard did not catch it

`validate-docs.sh` checks relative **markdown links** and passes:

```
✅ every relative doc link resolves
```

These are backticked prose, not links, so they are outside its stated scope. That is the correct
behaviour for the guard as written — but it means "the doc tells you where to look" is unverified,
and a wrong path is invisible until a reader hits it.

## Why not just extend the guard

The same file contains `` `.rigorix/capacity.toml` ``, which is legitimately absent — the demo
creates it at runtime. A blanket "backticked path must exist" rule flags it immediately, and the
allowlist that follows is the beginning of a check nobody trusts.

So: fix these two by turning them into markdown links (which the existing guard then covers), and
treat the general case as a separate design question if it turns out to recur.
