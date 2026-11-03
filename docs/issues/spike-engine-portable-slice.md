---
guardian_issue:
  id: "ISSUE-SPIKE-ENGINE-PORTABLE-SLICE"
  title: "Spike: can rigorix-engine's gate slice compile to wasm32, produce a byte-identical envelope, and fit a demo-viable bundle — measured, not argued"
  epic: "portable-slice"
  component: "Engine"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1351
  dependencies:
    - "FR-011 of wasm (blocked on this; do not start Phase B before this reports)"
    - "ADR-009 (wasm) — the prerequisite-ordering section this spike honours"
    - "ADR-011 (wasm) — the claim that a `portable` feature removes the native dependency, which this spike checks"

  intent: |
    Everything that has been built for the browser — `rigorix-gate-wasm`, 181 tests, ten gates, a smoke gate that
    loads the module and calls all four functions — is built **around** a hole: `SeamGate::not_linked()` refuses every
    run with `gate_unavailable`, and `rigorix-engine` has never been compiled for `wasm32-unknown-unknown`. The crate
    is a good box with the contents still to come, and 24 satisfied criteria make the missing contents look like a
    detail.

    Before any of the 151-site seam work begins, three questions must be answered with numbers:

    1. **Can the engine's gate slice compile for wasm32 at all**, and what exactly must be excluded to get there?
    2. **Can one envelope be produced byte-identically** by the CLI and by an in-crate call given the same pinned
       clock and ids — and how many call sites does that one operation touch?
    3. **What does the module weigh with the engine inside it**, against a ceiling the demo can afford?

    A "yes, obviously" answer to any of these is worth nothing: the tree-sitter build script fails before a single
    Rust error appears, the bundle today has no engine in it, and the seam inventory has never been written down.

    The output is not code. It is **measurements, an inventory file, and a decision** — which is why it is a spike
    and not an epic.

  in_scope:
    - "M1 — dependency surface: exclude the measured blockers from the wasm32 graph and record the exclusion list as a machine-readable inventory; `cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown` must compile"
    - "M2 — one operation end to end: a precondition refusal reaching a signed envelope, produced twice with identical bytes — once by the CLI given pinned inputs, once by an in-crate call given the same; record how many `Utc::now()` / `Uuid::new_v4()` sites that one operation touched"
    - "M3 — bundle size with the engine linked: the `wasm-bindgen --target nodejs` artifact measured in bytes, compared to a decided ceiling of 1 500 000 B"
    - "M4 — determinism across processes: the same operation in two processes produces identical bytes"
    - "A committed measurement report with the exact command and output for each of M1–M4, and a draft inventory file the Phase-A guard can later enforce"

  out_of_scope:
    - "Threading the seam through all 151 gate-path sites (Phase B). The spike touches only what the single operation of M2 needs, and records the rest as inventory"
    - "The conformance flip to `gate: \"cli\"`, deleting `normalised_comparison`, or regenerating goldens (that is Phase C, and it depends on the answer here)"
    - "Implementing `EngineGate` in production form; the spike's adapter is throwaway and stays unmerged"
    - "The demo, the npm release, and publishing — the crate is already on the train (#1346) and stays held by the `engine` feature being off"
    - "Raising or lowering the 1 500 000 B ceiling: it is decided here, and branch 3 below says what happens if the measurement exceeds it"

  affected_layers:
    domain:
      - "None — no engine domain type changes in the spike"
    application:
      - "Experimental only, on a throwaway branch: an adapter implementing `GateEvaluator` (wasm/src/domain/engine_seam.rs:137) against the engine's gate functions"
    infrastructure:
      - "Measured only: feature gates and `cfg` exclusions that make the wasm32 graph expressible, plus the CLI's pinned-input flags"
    interfaces:
      - "None"

  citations:
    - file: "wasm/src/infrastructure/seam_gate.rs"
      note: "`not_linked()` — the refusal the spike exists to replace, and the comment that states the two-implementation plan"
    - file: "wasm/src/domain/engine_seam.rs"
      note: ":98 GateEvaluationRequest, :78 EngineEvaluation, :59 EngineFinding, :115 EngineBindingError, :137 GateEvaluator — the carrier the engine must fill"
    - file: "engine/src/lib.rs"
      note: ":29 `ENGINE_VERSION = env!(\"CARGO_PKG_VERSION\")` — the symbol the stamp must use instead of CARGO_PKG_VERSION"
    - file: "engine/Cargo.toml"
      note: ":10-16 `[features]` — no `portable` feature exists today"
    - file: "engine/src/precondition/infrastructure/runner.rs"
      note: ":318-336 `geteuid`/`getegid`/`getgroups` — a libc blocker inside a gate-path module"
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: ":85-91 `flock` — the other libc blocker, in an adapter the crate already replaces"
    - file: "engine/src/audit/application/audit_sender_impl.rs"
      note: ":103-140 and audit/infrastructure/anchor.rs:262-277 — the reqwest sites on the gate path (delivery, not envelope building)"
    - file: "engine/src/identity/infrastructure/verifier.rs"
      note: ":113-121 the JWKS reqwest client — the identity edge the crate stubs"
    - file: "cli/src/cli_boundary/orchestrator.rs"
      note: ":198, :274, :281, :309 `create_default()` — where a pinned clock/id source would be supplied to the CLI"

  verification:
    - "M1: `cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown` exits 0, and `cargo tree -p rigorix-engine --features portable --target wasm32-unknown-unknown` contains none of tree-sitter, reqwest, ring, libc, prometheus, tracing-appender"
    - "M1b: the four published crates' default-feature suites pass unchanged from the same commit (engine, mcp, cli, actions) — the portable feature must be invisible to them"
    - "M2: two envelopes, CLI and in-crate, are byte-identical; the command pair and the output hash are in the report"
    - "M3: the measured bundle size is recorded in bytes with the exact command that produced it"
    - "M4: the same operation in two separate processes produces identical bytes"
    - "The report is committed; the throwaway branch is not merged"
---

# Spike: the engine's portable slice, measured

> **This issue produces measurements, an inventory, and a decision. It produces no shipping code.**
> If it ends with "it works, we'll finish it later", it has failed — the deliverables are a report, a file, and
> one of the four branches below.

## 1. Why it comes before everything else

The wasm crate is complete against a contract whose other side does not exist. Its own gates say so, quietly:

```
wasm/src/infrastructure/seam_gate.rs   SeamGate::not_linked() → every run refuses gate_unavailable
wasm/ci/bundle-budget.txt              measured_bytes: 243366   ← a module with no engine in it
wasm/.pi/architecture/runbook.md:124   "Engine seam unlinked → FR-007 blocked on FR-011"
```

Two estimates are in circulation and neither is evidence: "it is an additive feature" (ADR-011) and "the seam is
151 sites" (a grep). Both need to survive a compiler and a scale. The last three months of this repository's
defects were words that outran their evidence — `verified`, `derived`, `adopted`, `anchored`, `unavailable`,
`measured`. This spike exists so the next words in that list are not `portable` and `byte-identical`.

## 2. The baseline, measured before the spike starts

Recorded so the implementer starts from data rather than from a story. Every number below was measured on
`dfabc89c` with the command shown; the spike re-measures rather than trusting them.

| Fact | Value | How it was measured |
|---|---|---|
| engine compiles for wasm32 | **no** | `cargo check -p rigorix-engine --target wasm32-unknown-unknown` → fails in `tree-sitter v0.25.10`'s build script, before any Rust error |
| `portable` feature | **absent** | `engine/Cargo.toml:10-16` has `live-tests` and `conformance` only |
| `tree_sitter::` sites | 31 | `grep -rnE '(^|[^A-Za-z_])tree_sitter::'` → code_gen, repo_engine, tools |
| `reqwest::` sites | 64 | same pattern → audit 5, identity 2, precondition 4, plus 7 modules not on the slice |
| `libc::` sites | 8 | same pattern → approval 4 (flock), precondition 4 (geteuid/getegid/getgroups) |
| `ring::` sites | **1** | same pattern → identity. *(An earlier count of 270 was a substring match on `String::`; the boundary pattern is the only honest one)* |
| `prometheus::` sites | 14 | same pattern → observability |
| gate-path `Utc::now()` | 94 | precondition 4, sequence_policy 20, audit 31, approval 7, identity 9, orchestrator 23 |
| gate-path `Uuid::new_v4()` | 57 | precondition 2, sequence_policy 4, audit 11, approval 11, identity 5, orchestrator 24 |
| current wasm bundle | 243 366 B | `wasm-bindgen --target nodejs` output, `wasm/ci/bundle-budget.txt`; reproducible exactly |
| crate's seam carrier | 5 types | `wasm/src/domain/engine_seam.rs` — the trait the engine must fill |

## 3. The four measurements

Each has one command, one expected output, and one pass condition. Record the command **and** its output; a
measurement without its command is a rumour.

### M1 — the dependency surface

```
cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown
cargo tree  -p rigorix-engine --features portable --target wasm32-unknown-unknown
```

**Pass:** exit 0, and the tree names none of `tree-sitter`, `reqwest`, `ring`, `libc`, `prometheus`,
`tracing-appender`.

**Also required (M1b):** the four published crates' default-feature suites pass **unmodified** from the same
commit — `cargo test -p rigorix-engine`, `-p rigorix-mcp`, `-p rigorix-cli`, `-p rigorix-actions`. If any test
needed an edit to pass, the feature is not additive and M1 has failed.

**Produce:** `engine/.pi/portable-inventory.tsv` — one row per exclusion, columns
`module · item · why-excluded · replaces-with · measured-sites`. This file is the Phase-A guard's subject; without
it, "everything is fine under `portable`" is a claim nobody can diff.

### M2 — one operation, two runtimes, identical bytes

Pick **one** operation and stay on it: *a precondition refusal that produces a signed envelope*. Not a full run,
not the five use cases — one decision and its evidence.

```
# CLI, given pinned inputs (the flags land in the spike branch, behind a non-default feature)
rigorix-cli run --template <fixture> --pinned-clock <tsv> --pinned-ids <tsv> > /tmp/cli.json
sha256sum /tmp/cli.json

# in-crate, same fixture, same pinned inputs (throwaway test / example)
cargo test -p rigorix-gate-wasm --features engine,spike-m2 -- --nocapture
sha256sum /tmp/wasm.json
```

**Pass:** the two digests are equal.

**Also record:** how many `Utc::now()` and `Uuid::new_v4()` sites the one operation touched. This number is the
spike's most useful output for planning Phase B: 151 is the crate-wide inventory, and the rate (sites threaded per
day, measured) is what turns it into a schedule.

**If byte equality is not reached inside the timebox**, that is branch B — do not keep going, and do not reach for a
normalised comparison. Recording "not reached, because N sites, because of X" is a valid, useful result.

### M3 — the bundle with the engine inside it

```
cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm --features engine
wasm-bindgen --target nodejs --out-dir /tmp/pkg target/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm
wc -c /tmp/pkg/rigorix_gate_wasm_bg.wasm
```

**Pass:** ≤ **1 500 000 bytes**.

That ceiling is decided here, not discovered later: a demo that loads a multi-megabyte module before showing a
button is a demo that does not start. 1.5 MB is roughly six times today's 243 366 B — enough headroom for
serde/chrono/uuid plus the gate modules, and small enough to stay a lazy-loaded island.

**Also record:** the same number for the native-built crate for contrast, and the ten largest contributors from
`cargo tree` or `wasm-objdump -h` if available.

### M4 — determinism across processes

Run the M2 in-crate operation twice in two fresh processes; compare digests. **Pass:** identical.

This is not the same as criterion 25's second-process test, which runs the *stub* gate: it must be re-run against
the linked engine, because process-global state and hash-map iteration order first become observable when a real
policy evaluation runs.

## 4. The four branches — decided now, so the spike cannot drift

| branch | fires when | decided next step |
|---|---|---|
| **A** | M1 fails even after the exclusions, or the exclusion list is larger than the inventory predicts | extract `rigorix-gate-core`: the gate modules move to their own crate with no `reqwest`/`libc`/`tree-sitter`/`observability`, and both the engine and the wasm crate depend on it. Own ADR; ~86 files to move |
| **B** | M2 cannot reach byte equality inside the timebox | Phase B (the seam) becomes its own epic on the measured inventory, and the spike returns to M2 only after it lands. The crate stays unlinked and the demo stays blocked — stated, not worked around |
| **C** | M3 exceeds 1 500 000 B and the excess is not a single trimable dependency | the demo ships a **narrowed slice**: verify-only in the browser (no gate evaluation), with the gate staying server/CLI-side. The product plan is amended before any demo work starts |
| **D** | M1, M2, M3 and M4 all pass | write the decision ADR — *portable slice: what `portable` removes and how the determinism seam is supplied* — amending ADR-001/ADR-011, and begin Phase A/B as separate epics |

Branches are not exclusive: M1 passing and M3 failing is branch C with M1's inventory kept.

## 5. Timebox and stop conditions

- **Ten working days**, matching ADR-009's prerequisite ordering. Day 10 produces the report whatever state it is in.
- **Stop at the first branch that fires.** Continuing to "make it work" past a branch converts a spike into an
  unfunded epilogue.
- **Merge nothing but the report and the inventory.** The adapter, the `spike-m2` feature and the CLI flags live on
  a branch that is linked from the report and otherwise discarded; the CLI's pinned-input work is re-implemented
  properly in Phase C if branch D holds.

## 6. Deliverables

1. `docs/spike-portable-slice-2026-11.md` — the report: four measurements with commands and outputs, the four
   branch verdicts, the site count, the bundle number, and the recommendation.
2. `engine/.pi/portable-inventory.tsv` — the exclusion list, machine-readable, with a `measured-sites` column.
3. The decision that follows the spike, recorded as an ADR **only if** branch D holds (branches A and C produce
   their own ADR as their first step, not this one).
4. A link to the throwaway branch in the report, and its deletion note if it is dropped.

## 7. What this is not

- **Not Phase B.** Threading 151 sites is not a spike; the spike measures how expensive it is.
- **Not a conformance flip.** `gate: "fixture"` stays until a real CLI diff exists; `normalised_comparison` is never
  introduced here, in any form, not even to "prove the harness works".
- **Not a demo task.** The demo cannot be designed against `gate_unavailable`, and this issue does not make it
  possible — it establishes whether it ever will be.
- **Not a reason to touch the release train.** #1346 already put the crate on the train and the `engine` feature
  being off is what keeps it unpublished; that arrangement is correct and stays.

## 8. Traps already found, so they are not rediscovered

1. **tree-sitter fails first, at its build script** — no Rust error is visible until it is out of the graph. The
   first hour of M1 is dependency surgery, not code reading.
2. **A grep without word boundaries lies.** `ring` appeared in 270 places because `String::from_utf8_lossy`
   contains "ring". Use `(^|[^A-Za-z_])NAME::`.
3. **The current bundle number has no engine in it.** 243 366 B is a ceiling over a module that links almost
   nothing; it will move by an order of magnitude and the budget file must be re-measured, not adjusted.
4. **`ENGINE_VERSION` exists** (`engine/src/lib.rs:29`) and the crate currently stamps `CARGO_PKG_VERSION`. They are
   equal only while both crates share a version. M2's envelope comparison is the moment to switch to the symbol.
5. **The four published crates must not notice.** `portable` is not "additive" if a default-feature test had to
   change to keep passing; M1b is the check, and it is a pass/fail, not a note.
6. **The smoke gate's `gate_unavailable` assertion is the marker.** Today it asserts the refusal; branch D means
   that assertion is replaced by a real decision. If it still asserts `gate_unavailable` after Phase C, the link
   did not happen.
