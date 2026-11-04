---
guardian_issue:
  id: "ISSUE-M2-DIGEST-IS-THE-APPARATUSES"
  title: "The report's M2 digest belongs to the discarded apparatus, not the shipped operation: 8b2cc83d… does not reproduce, merged main produces c47b27f9…, and no test pins either"
  epic: "portable-slice"
  component: "Engine"
  module: "wasm-seam"
  status: planned
  priority: medium
  tracking_issue: 1357
  dependencies:
    - "#1355 (produced the operation and the digest this corrects)"
    - "#1354 (the report the digest appears in)"

  intent: |
    The corrected report states branch D on the strength of one witness: "CLI and in-crate both
    `8b2cc83d…`, 853 B, pinned". Re-run on merged main with the pins the example's own usage block
    documents, **neither entry point produces `8b2cc83d…`** — both produce `c47b27f9…`, still
    853 B, still identical to each other.

    The property holds. The witness does not, and the reason is mundane: the operation was moved
    from the apparatus (`template_id: "spike-m2-precondition-refusal"`, `effect_key:
    Some("sha256:spike-effect")`, `producer_id: "spike-producer"`) into
    `engine/src/audit/application/pinned.rs` with renamed and dropped fields (`template_id:
    "m2-precondition-refusal"`, `effect_key: None`, `producer_id: "m2-producer"`). Different input,
    different bytes — at exactly the same length, which is why size alone would not have caught it.

    Nothing failed loudly because no test pins the value: `pinnedinputs_test.rs` asserts that two
    runs agree and that one second of difference changes the digest, which is the *property* and not
    the *witness*. A fixture change is therefore invisible until someone re-runs the documented
    command and compares — which is what the report asks a reader to do, and what a reader cannot do
    once the operation's input moves.

  in_scope:
    - "The report's §2 CLI-half passage and the M2 table row cite the digest the shipped operation produces, with the exact pins and both commands"
    - "One test pins the literal digest for those pins, so a fixture edit fails CI and forces the report's number to move in the same commit; the report then cites the test by name instead of holding a second authority for the value"
    - "The grid that ruled out the old value is recorded in the report as the reason the correction was needed, not as an apology"

  out_of_scope:
    - "Re-deriving the digest by re-running until two values agree. The value is a function of the input; if it changed, the input changed, and that is the finding"
    - "Changing the shipped fixture to reproduce the apparatus's bytes. The shipped names (`m2-precondition-refusal`, `m2-producer`) are the ones a reader of the merged tree should see"
    - "The M4 and seed claims, which were re-verified and stand: unpinned runs differ (031232ce… vs 6ee9a632…), pinned runs agree, one second off and a different id both change the digest"

  affected_layers:
    domain:
      - "None"
    application:
      - "None — no runtime change"
    infrastructure:
      - "engine/tests/unit/pinnedinputs/pinnedinputs_test.rs — one golden assertion added"
    interfaces:
      - "None"

  citations:
    - file: "docs/spike-portable-slice-2026-11.md"
      note: ":12 the verdict paragraph's 8b2cc83d…; :33 the M2 row"
    - file: "engine/src/audit/application/pinned.rs"
      note: ":57 template_id, :67 source, :71 effect_key, :72 producer_id — the shipped fixture"
    - file: "engine/examples/pinned_refusal.rs"
      note: ":10-11 the documented pins the digest must correspond to"
    - file: "engine/tests/unit/pinnedinputs/pinnedinputs_test.rs"
      note: ":44-50 equality and sensitivity assertions; no literal digest anywhere"

  verification:
    - "`cargo run -q -p rigorix-engine --example pinned_refusal -- --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001` prints sha256=c47b27f9… and bytes=853, and the report's §2 records that same value"
    - "`cargo run -q -p rigorix-cli --features spike-m2 -- pinned-refusal --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001` prints the same sha256 and the same 853 bytes"
    - "`grep -c 8b2cc83d docs/` returns 0"
    - "Seed: change `template_id` in `engine/src/audit/application/pinned.rs` by one character → the golden test fails naming the documented digest, and the report's number no longer matches. Restored, it passes"
    - "`cargo test -q -p rigorix-engine --test unit pinned` runs 7 tests (6 + the golden) and passes"
---

# The witness moved, the property didn't

## Measured on merged main (24e6dd6f)

```
in-crate  853 B  sha256=c47b27f9300ed91b06e9712ac5df60714aa77a2a23d9bf41aa7d52f87393bc96
CLI       853 B  sha256=c47b27f9300ed91b06e9712ac5df60714aa77a2a23d9bf41aa7d52f87393bc96
unpinned  in-crate 031232ce…  860 B     CLI 6ee9a632…  860 B     ← differ, as M4 requires
one second off                      662e92b6…
different id                        d5fdedae…
```

A grid over `--pinned-ids` `…0000`–`…0003` against `12:00:00Z` and `18:51:18Z` produced
`19a0352a`, `0b7a4f49`, `c47b27f9`, `5a2f6536`, `d5fdedae`, `00a20df9`, `8a8371f8`, `16e19303`
— **no `8b2cc83d`**. The value was not unreachable by luck; it belongs to an input that was
replaced during the port.

## Why size alone would have missed it

The apparatus's envelope and the shipped envelope are **both 853 bytes**. Renamed fields of equal
length, one field dropped and compensated: byte-for-byte different, length-identical. This is the
report's own lesson from 884 vs 877 turned inward — a length is not a fingerprint, and a digest
comparison is the only one that notices.

## The fix is one assertion, not one more re-run

```
$ grep -n 'assert' engine/tests/unit/pinnedinputs/pinnedinputs_test.rs
:46  assert_eq!(first, second)                      ← the property
:52  assert_ne!(base, shifted)                       ← the sensitivity
```

Neither pins a value, so the fixture can drift and every check stays green. One golden assertion —
the literal digest for the documented pins — makes the report's number a *checked* fact: edit
`pinned.rs`, the test fails, and the report must move in the same commit. The value then lives in
one place and the report cites the test rather than duplicating it.
