---
guardian_issue:
  id: "ISSUE-RUNBOOK-GATE-COUNT-STALE"
  title: "The runbook says 'eight gates'; there are ten, and nothing compares the number to the script"
  epic: "portable-slice"
  component: "Docs"
  module: "wasm-seam"
  status: planned
  priority: low
  tracking_issue: 1368
  dependencies: []

  intent: |
    `runbook.md:5` reads "**Gates:** `wasm/ci/run-wasm-gates.sh` (one command, eight gates)". The
    script declares **10** (`grep -c '^gate "'`), and the run prints `10 gate(s) passed`.

    A count in a document is the cheapest kind of claim to keep true and the easiest to leave behind:
    the gate that makes it checkable already prints it. The risk is not the number — it is that a
    reader who counts eight and sees ten stops trusting the table, and the next stale number in the
    same table inherits that doubt.

  in_scope:
    - "The runbook line states the count derived from the script's own output, or avoids a literal by pointing at the command's printed count"
    - "A check asserts the runbook's stated count equals `grep -c '^gate \"' wasm/ci/run-wasm-gates.sh`, so the two cannot drift"

  out_of_scope:
    - "The gate list itself, which is accurate"
    - "Any change to `run-wasm-gates.sh`'s behaviour"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "wasm/ci (the count assertion)"
    interfaces:
      - "None"

  citations:
    - file: "wasm/.pi/architecture/runbook.md"
      note: ":5 the 'eight gates' line"
    - file: "wasm/ci/run-wasm-gates.sh"
      note: "10 `gate \"…\"` declarations; the summary prints the passed count"

  verification:
    - "`grep -c '^gate \"' wasm/ci/run-wasm-gates.sh` equals the number stated in runbook.md:5"
    - "Seed: change the runbook to 'nine gates' → the count check exits 1 naming both numbers; restored, it passes"
---

# Eight versus ten

Small, and worth one commit because it is the same defect as every other in this epic at its
smallest scale: a number in prose whose subject is a script that will keep changing.
