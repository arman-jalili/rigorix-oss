---
guardian_issue:
  id: "ISSUE-VERIFIED-LINE-STILL-LEADS-WITH-THE-GAP-APPENDED"
  title: "the ledger row reads 'verified, N marker(s): 0 attributed, N without a settlement' and still renders as a pass"
  epic: "authority-boundary"
  component: "Cli"
  module: "approval"
  status: planned
  priority: low
  tracking_issue: 1275
  dependencies:
    - "#1272 — the attribution split"
    - "#1264 — how an absent ledger is reported"

  intent: |
    #1272 asked for the unattributed population to be reported as a capability gap *"the way an absent
    ledger is (#1264), not as a warning inside a `verified` line"*. The split is computed correctly and
    printed in full; the gap is still the tail of a line that begins `verified`, on a row that renders
    as a pass with exit 0.

    Measured against a store with two terminal records and a ledger whose two markers carry no
    settlement:

    ```
    ✓ approval binding  armed  … consumption ledger /tmp/…/consumed.json (outside agent-writable
      workspace; verified, 2 marker(s): 0 attributed, 2 without a settlement; the 2 unattributed
      marker(s) are a capability gap — a re-settlement named by one is refused)
    exit = 0
    ```

    Every fact is present and the verdict is wrong: `verified` is the first word a reader or a grep
    sees, and it is the one word the row cannot support when *every* marker is unattributed. Compare
    #1264's absent ledger, which renders `✗ approval binding MISCONFIGURED` and fails the row.

    This is the repo's own doctrine applied to its own surface: a check that did not run must not print
    like one that passed. Verification did run — so the exit stays 0 — but the verdict must describe
    the weakest property the row carries.

  in_scope:
    - "The verdict word follows the weakest property: `verified` only when every marker is attributed, `verified with gaps` when some are not"
    - "The capability gap is its own line (or its own field), not a trailing clause after `verified`"
    - "The exit code is unchanged: verification succeeded, so the check does not fail; nothing here turns doctor red"
    - "Seeds: all-attributed → `verified`; mixed → `verified with gaps`; all-unattributed → `verified with gaps` and the count; the verdict line never begins `verified` when any marker is unattributed"

  out_of_scope:
    - "Making unattributed markers an error or a non-zero exit"
    - "Backfilling settlements into legacy markers"
    - "The enterprise's `mac_verified`, which is a different field (#472)"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: the doctor row's verdict computation and its rendering"

  citations:
    - file: "cli/src/cli_boundary/doctor.rs"
      note: ":298-311 the ledger_state string; the row's verdict is chosen above it"
    - file: "docs/issues/doctor-verified-hides-unattributed-markers.md (closed by #1274)"
      note: "the criterion the sentence did not follow"

  verification:
    - "all-unattributed ledger: the row does not read `verified` alone, exit 0"
    - "all-attributed ledger: unchanged"
---

# ISSUE-VERIFIED-LINE-STILL-LEADS-WITH-THE-GAP-APPENDED

## Measured

```
verified, 2 marker(s): 0 attributed, 2 without a settlement; the 2 unattributed marker(s) are a
capability gap — a re-settlement named by one is refused
exit = 0
```

The information is right; the verdict is `verified`. A reader who greps for `verified` counts this row
as protective, and the population that can produce a false `NodeFailed` on an idempotent settlement is
the whole ledger.

## The required end state

The row's verdict is decided by the weakest property it carries. `verified` means every marker can
classify a re-settlement; `verified with gaps` means some cannot. The gap gets its own line. The exit
stays 0, because the check that ran did pass.
