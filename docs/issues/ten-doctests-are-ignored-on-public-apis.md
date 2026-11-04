---
guardian_issue:
  id: "ISSUE-TEN-DOCTESTS-ARE-IGNORED_ON_PUBLIC_APIS"
  title: "10 of rigorix_engine's 16 doctests are `ignore` — including the example for `with_pinned_inputs`, the documented way to use ADR-014's carrier — so they never compile"
  epic: "portable-slice"
  component: "Engine"
  module: "wasm-seam"
  status: planned
  priority: medium
  tracking_issue: 1379
  dependencies:
    - "#1373 (introduced the doctest floor over *executed* examples)"

  intent: |
    `bash .pi/scripts/run-doctests.sh` reports `rigorix_engine 16 discovered 6 executed 10 ignored`,
    and the floor in `.pi/doctest-floors.tsv` ratchets the *executed* count at 6 for exactly this
    reason: `cargo test --doc` prints "running 16 tests" and then runs six.

    An `ignore` fence is not a slow example; it is an example that is **never compiled**. So the ten
    are documentation that cannot be wrong in public, which is the one property a doc example is for.
    Among them:

    ```
    engine/src/audit/application/envelope_factory_impl.rs:69   AuditEnvelopeFactoryImpl::with_pinned_inputs
    engine/src/configuration/domain/secret.rs:31               Secret
    engine/src/recovery_recipes/application/context.rs:25      RecoveryContext
    engine/src/tools/infrastructure/tree_sitter_anchor.rs:36   AnchorParams
    engine/src/budget_tracking/application/service.rs:106      LlmBudgetReservation…
    engine/src/failure_classification/application/classify.rs  classify
    ```

    The first is the sharpest: `with_pinned_inputs` is the method ADR-014 §2 introduced as the way to
    supply a pinned instant, its usage example is `ignore`, and nothing compiles it. The engine's
    public surface documents the decision's carrier with a snippet that has never been built.

    #1373 recorded the gap and named it as worth an issue; it was not filed, so the floor records a
    number without an owner for it.

  in_scope:
    - "Every `ignore` fence on a public API is converted to a compiling example: a plain fence when it can run, `no_run` when it needs a network, a database or a filesystem, and `ignore` only with a stated reason recorded in `.pi/doctest-exemptions.tsv` (path, item, why)"
    - "The floor in `.pi/doctest-floors.tsv` is raised to the new executed count, so the conversions cannot be undone silently"
    - "`with_pinned_inputs`'s example compiles and, if it can run without a key, runs"

  out_of_scope:
    - "Adding examples to items that have none. This is about examples that exist and do not compile"
    - "A doctest for every public item — the floor is a ratchet, not a target"
    - "The `run-doctests.sh` mechanics, which work: floors, per-crate reporting, and a failure that names the crate"

  affected_layers:
    domain:
      - "Documentation examples on public domain types (`Secret`, `AnchorParams`)"
    application:
      - "`AuditEnvelopeFactoryImpl::with_pinned_inputs`, `RecoveryContext`, `classify`, `LlmBudgetReservation`"
    infrastructure:
      - ".pi/doctest-floors.tsv (raised), .pi/doctest-exemptions.tsv (new, if any exemption survives)"
    interfaces:
      - "None"

  citations:
    - file: ".pi/doctest-floors.tsv"
      note: "the rationale comment and the measured 16 / 6 / 10 line; floor `rigorix_engine 6`"
    - file: "engine/src/audit/application/envelope_factory_impl.rs"
      note: ":69 the `ignore` fence on `with_pinned_inputs`"
    - file: ".pi/scripts/run-doctests.sh"
      note: "reports discovered / executed / ignored per crate and fails below the floor"

  verification:
    - "`bash .pi/scripts/run-doctests.sh` reports an executed count above 6 and the floor matches it"
    - "`cargo test --doc -p rigorix-engine -- --list | grep -c ': test'` equals discovered, and every ignored one has a row in the exemptions file or a compiling fence"
    - "Seed: rename `with_pinned_inputs` to `with_pinned_input` → the doc example fails to compile and `run-doctests.sh` fails naming the crate; restored, it passes"
    - "A converted example that would have caught a real drift is demonstrated: the seed above is that demonstration"
    - "`grep -rc '```ignore' engine/src` drops to the exemption count"
---

# Ten examples that cannot be wrong

```
$ bash .pi/scripts/run-doctests.sh
crate              discovered executed ignored  status
rigorix_engine             16        6      10  ok
```

`ok` is correct — six ran and the floor is six. But ten examples on public APIs are marked `ignore`,
which means rustdoc skips compilation entirely. A doc example that never compiles is prose with code
formatting; it cannot fail when the API changes, which is the only thing it is for.

The zero-risk conversion is most of them: `no_run` compiles the example and skips execution, which
is exactly right for anything needing a key, a socket or a database. `ignore` should mean "this
genuinely cannot be compiled" — and then it needs a reason on record, not a bare fence.
