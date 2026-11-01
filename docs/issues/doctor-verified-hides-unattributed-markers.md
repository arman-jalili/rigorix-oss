---
guardian_issue:
  id: "ISSUE-DOCTOR-VERIFIED-HIDES-UNATTRIBUTED-MARKERS"
  title: "doctor reports 'verified, N marker(s)' and never says how many name no settlement, so a pre-#1268 ledger reads as fully protected"
  epic: "authority-boundary"
  component: "Cli"
  module: "approval"
  status: planned
  priority: medium
  tracking_issue: 1272
  dependencies:
    - "#1267 — markers gained the settlement"
    - "#1269 — the legacy marker is now a tested shape"

  intent: |
    `doctor` summarises the consumption ledger as:

    ```rust
    // cli/src/cli_boundary/doctor.rs:298
    let ledger_state = if setup.consumption_ledger_path.is_file() {
        format!("verified, {markers} marker(s)")
    ```

    `markers` is `consumption_marker_count()`, which counts entries. It says nothing about which
    entries name the settlement that consumed the approval. Since #1267 a marker may carry
    `(dag_id, attempt)` and, when it does not, the classifier treats it as **another settlement** — a
    refusal. Every marker written before #1268, and every marker adopted by
    `rigorix approve ledger migrate` (which cannot know the settlement), is in that population.

    So one word covers two populations that behave differently the first time a run settles again:

    | population | rule 2 of the classifier |
    |---|---|
    | attributed | benign when it names this settlement |
    | unattributed | always "another settlement" — a refusal and a `NodeFailed` |

    The reader is told `verified`, which is true of the MAC and silent about the attribution. This is
    the same shape as #1253 (`adopted 2 of 1`) and #1251 (`adopted 1 of 0`): a number that is not
    comparable to the number it should be — here, a count with no denominator.

  in_scope:
    - "doctor reports the split: markers verified, of which N name a settlement and M do not (M is the pre-#1268 / adopted population)"
    - "the port exposes the population count (`settlements_known` / `settlements_unknown`) rather than only `consumption_marker_count`"
    - "an unattributed marker is reported as a capability gap, the way an absent ledger is (#1264), not as a warning inside a `verified` line"
    - "Seeds: a ledger with 1 attributed and 1 legacy marker prints `1 attributed, 1 without`; the legacy fixture from #1269 is the input"

  out_of_scope:
    - "Writing settlements into legacy markers, which the ledger cannot do without a settlement"
    - "The classifier's rule, which #1267 settled"
    - "`mac_verified` in the enterprise, which is a different field (#472)"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: an attributed/unattributed population query on the repository port"
    infrastructure:
      - "Modified: the count walks the markers it already reads"

  citations:
    - file: "cli/src/cli_boundary/doctor.rs"
      note: ":287-305 the ledger line and 'verified, N marker(s)'"
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: ":847-868 consumption_settlement returns None for an unattributed marker"
    - file: "engine/src/execution_engine/application/service_impl/dispatch.rs"
      note: "a returned settlement that is not this one is a refusal (#1263, #1267)"

  verification:
    - "one attributed and one legacy marker: doctor prints both counts"
    - "all attributed: doctor prints zero unattributed"
    - "the plain count restored: the first seed fails"
---

# ISSUE-DOCTOR-VERIFIED-HIDES-UNATTRIBUTED-MARKERS

## Measured

`doctor` prints (from the code path above):

```
approval binding   ARMED   verified, 1 marker(s)
```

for a ledger whose single marker may be either population. The distinction that matters appeared in
#1267 and is invisible here.

## Why the word matters

`verified` was a fair word when every marker was one kind of thing. It is now a **count with no
denominator**: the reader needs to know whether the ledger's entries can classify a re-settlement as
benign (attributed) or will always call it another settlement (unattributed), because the second case
turns a legitimate idempotent settlement into a failed node with published evidence.

## The required end state

`verified, N marker(s): A attributed, B without a settlement`, with the unattributed count reported as
a capability gap rather than folded into a success line — the same treatment an absent ledger gets.
