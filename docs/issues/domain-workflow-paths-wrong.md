---
guardian_issue:
  id: "ISSUE-DOMAIN-WORKFLOW-PATHS-WRONG"
  title: ".pi/context/domain-workflow.md points at two .pi/ paths that live under engine/ — one also names the wrong file"
  epic: "docs hygiene"
  component: "Docs"
  module: "governance"
  status: planned
  priority: low
  dependencies:
    - "#1094 — same defect class, fixed in demo/capacity-ledger/README.md"

  intent: |
    Two lines in the domain-workflow table point at architecture docs with the wrong prefix, and one
    names a file that does not exist under any prefix. Both are the same defect class as #1094
    (missing `engine/`), which #1094's delivery report said had been scanned for and found nowhere
    else. The scan checked markdown links — the guard's scope — not backticked paths, which is the
    exact gap #1094 identified, so it could not have found these.

  in_scope:
    - "Correct `.pi/architecture/decisions/ADR-001.md` to `engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md` (prefix *and* filename)"
    - "Correct `.pi/architecture/diagrams/system-context.md` to `engine/.pi/architecture/diagrams/system-context.md`"
    - "Leave the `<context>` placeholder row alone — it is a pattern, not a path"

  out_of_scope:
    - "The four `.pi/INDEX.md` / architecture-coordinator references that exist under no prefix at all (.opencode/context.md, .github/copilot-instructions.md, .github/copilot/settings.json, .github/workflows/01-planning-workflow.md) — a different class, see below"
    - "Widening validate-docs.sh to backticked paths (recorded in #1094 as requiring a design decision)"

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
    - evidence: ".pi/context/domain-workflow.md:76 — `| `.pi/architecture/decisions/ADR-001.md` | Architecture pattern decision |`"
    - evidence: ".pi/context/domain-workflow.md:77 — `| `.pi/architecture/diagrams/system-context.md` | Mermaid bounded context flow |`"
    - evidence: "actual: engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md, engine/.pi/architecture/diagrams/system-context.md"
    - evidence: ".pi/architecture/decisions/ADR-001.md does not exist at root or under engine/ — the filename is wrong independently of the prefix"

  acceptance_criteria:
    - "Both paths resolve from the repo root"
    - "The ADR row names the file that exists (ADR-001-architecture-pattern.md), not a truncated name"
    - "The scan that closes this resolves backticked paths too, not only markdown links — otherwise it repeats the miss"
    - "validate-docs.sh still passes"

  validators:
    - canonical
    - ci

  implementation_notes: |
    Two independent errors on the ADR row: the missing `engine/` prefix **and** a truncated filename
    (`ADR-001.md` for `ADR-001-architecture-pattern.md`). Fixing only the prefix leaves a dead path,
    which is how this class tends to survive a first pass.

    On the scan: the honest method is to resolve every backticked path against the doc's own
    directory, the repo root, and each crate subtree, then look at what remains. Manifesting only
    markdown links tests the guard, not the docs.

  file_changes:
    - "modify: .pi/context/domain-workflow.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-DOMAIN-WORKFLOW-PATHS-WRONG

## The two rows

```markdown
# .pi/context/domain-workflow.md:76-77
| `.pi/architecture/decisions/ADR-001.md`       | Architecture pattern decision  |
| `.pi/architecture/diagrams/system-context.md` | Mermaid bounded context flow   |
```

| referenced | actual | error |
|---|---|---|
| `.pi/architecture/decisions/ADR-001.md` | `engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md` | wrong prefix **and** truncated name |
| `.pi/architecture/diagrams/system-context.md` | `engine/.pi/architecture/diagrams/system-context.md` | wrong prefix |

The `ADR-001.md` row fails twice over: there is no `ADR-001.md` at the root *or* under `engine/`.
Correcting the prefix alone still leaves a dead path.

Row 75 (`.pi/architecture/modules/<context>.md`) is a pattern rather than a path and is correct as
written.

## Why the #1094 scan missed them

The #1094 report said the repo had been scanned for the same class and that it was "really limited
to those two lines". The methods differ in what they can see:

| method | finds |
|---|---|
| manifest markdown links | what `validate-docs.sh` checks — i.e. the guard's scope |
| resolve backticked paths against doc-dir, root, crate subtrees | what the docs actually claim |

The second method is what produced #1094. The first cannot find a backticked path by construction,
so a scan using it returns clean whether or not the defect is present. That is worth stating
plainly, because "I scanned and it was limited to N" invites the reader to stop looking.

## The other class, recorded but not actioned

Four references point at files that exist under no prefix at all:

```
.opencode/context.md
.github/copilot-instructions.md
.github/copilot/settings.json
.github/workflows/01-planning-workflow.md
```

These look like a scaffold index for a layout that was never instantiated, not a wrong prefix. Whether
to delete them or make the scaffold real is a decision, not a path fix, so it is out of scope here —
and it is the kind of decision that should not be made by silently correcting the strings.
