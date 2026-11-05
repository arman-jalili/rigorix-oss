# ADR-014: The Portable Slice and the Pinned-Input Carrier

**Status:** Accepted
**Date:** 2026-11-04
**Session:** continuation of `3baf6e5c-e352-4127-af09-a5e7fe0a2d29`
**Evidence:** `docs/spike-portable-slice-2026-11.md` (the measurement; this record is the decision)

## Context

The portable-session module asserts something the whole demo rests on: *the browser decides the
same as the engine*. Both halves of that sentence were unproven. Nothing had compiled the engine's
gate slice for `wasm32-unknown-unknown`, and nothing had shown that a decision taken in a browser
produces byte-identical output to the same decision taken by the CLI.

Phase -1's feasibility spike (#1351) measured four things (M1 slice, M1b strictness, M2 byte
equality, M3 bundle size, M4 cross-process determinism) against a live candidate implementation
rather than against an argument. It reached **branch D: all four hold**. Two things about how it got
there are load-bearing for this record:

- **The size verdict was wrong once and was withdrawn (#1354).** M3 was first read off the emitted
  build output, which carries the `name` custom section — 341 785 B of debug symbols on an engine-linked
  module. With the size profile (#1386) the artifact a host loads is 1 334 325 B, i.e. 88.95 % of the 1 500 000 B ceiling, so the size condition
  never fired and the decision that depended on it (*"the browser verifies, the gate stays
  server-side"*) is withdrawn with it. A decision record written before that correction would have
  narrowed the product for a problem that did not exist. **This ADR is written after it, not instead
  of it.**
- **The witness for M2 moved silently once (#1357).** The published digest belonged to an input that
  no longer existed, and no test noticed because the tests asserted the property (two runs agree) and
  never a value. The value is now pinned by a test. §Evidence below states, for every number, the
  artifact it came from and whether a reader can reproduce it — a rule adopted because three of this
  epic's four mistakes were numbers whose *subject* was an older artifact.

## Decision

### 1. `portable` is a non-default feature whose exclusions are a manifest, not a paragraph

The engine compiles for `wasm32-unknown-unknown` with `--features portable`. What that feature
removes is listed **once**, in `engine/.pi/portable-inventory.tsv` — one row per excluded
dependency, module and inner item, with the reason and the number of sites the reason rests on.
This ADR cites that file and does not restate it; a restated list is a second copy, and two copies
drift. `.pi/scripts/validate-portable-inventory.sh` keeps the citation honest in both directions: the
manifest must name things that exist (a rename cannot leave a stale row), and this record must not
reproduce the list (a second authority cannot appear quietly).

Four counts are recorded here because they are the decision, not the detail: **74 crates** in the
`wasm32` graph (normal + build), **22 of 39** public modules excluded, **23 inner items** gated
inside the modules that stay, and **9 dependencies** removed. Of those 22, **only 10
leave for a forbidden crate**; 6 leave because they need a `tokio` feature the slice narrows away
and 6 because they name a symbol from an excluded module. So 12 of 22 leave for something that is
*not* a dependency statement, and any future "just remove `reqwest`" reasoning reaches only 10 of 22.
`CoreOrchestratorError` alone carries 14 `#[from]` variants across 85 gate sites; the slice's cost
is concentrated in error plumbing, not in lexing.

### 2. Determinism is witnessed by one operation and a pinned-input carrier, not by a clock abstraction

Both runtimes call **one** function to build the refusal envelope,
`audit::application::pinned::refusal_envelope_input`, and the factory stamps the pins through an
additive seam:

```rust
PinnedInputs::none()                        // #[non_exhaustive], empty by default
    .with_timestamp(instant)                // the clock, when there is one
    .with_execution_id(id)                  // the id source, when there is one
```

`AuditEnvelopeFactoryImpl::with_pinned_inputs(pins)` is the only way in. The pins are applied
**inside** the shared operation, so the runtime is the only variable between the two entry points.

**No `Clock` / `IdSource` trait pair is introduced on the gate path.** The roadmap anticipated one
(`implementation-roadmap.md`, "rigorix-engine (additive features)"); this record supersedes that line
for the engine's determinism seam, and the roadmap is annotated accordingly. The reason is that the
witness has exactly one site: a trait pair with one consumer is an abstraction with no second
implementor, and its cost is paid by every future caller. When `portable-session` needs ports for its
own reasons — replay divergence, seeded randomness, key material — those ports belong to that
module's domain, not to this seam, and they can be added where a second consumer exists.

The `PinnedInputs` type is `#[non_exhaustive]` from its first commit, and the seam is a method rather
than a field, because the alternative was a semver break that only surfaces for a downstream user:
`BuildEnvelopeInput` is public and literally constructed by two crates in this workspace
(`server/tests/audit_scope_check_verify.rs`, `mcp/src/host/mod.rs`), so a new field would have edited
eleven call sites here and broken everyone outside.

#### The decision's second consequence: one gate entry point, and it is the engine's

Phase C rests on a claim this record has to make explicitly: **both runtimes call the same
composition**, not the same parts.

`engine/src/gate/application/compose.rs::evaluate_gate` is that composition. It takes a
[`GateRequest`] — the step and its proposed action, the `PinnedInputs` above, and the ports as
injected trait objects — and returns a `GateDecision` with findings in evaluation order and a
`decision_digest` over the decision's canonical bytes. It owns the *order*
(`preconditions` → `requirements`, fail closed at the first refusal) and nothing else: no domain,
no configuration, no signing.

Two things follow, and both are checkable rather than asserted:

- **The order cannot drift.** `cli/src/cli_boundary/orchestrator.rs` no longer composes the two
  services; it asks `GateSetup::from_env` for them and hands them on
  (`grep -c sequence_policy cli/src/cli_boundary/orchestrator.rs` → `0`). A browser build calls
  the same function with its own ports, so "the browser decides the same as the engine" is the
  only possible outcome, not a maintained coincidence.
- **The decision has an identity.** `decision_digest` is SHA-256 over `[verdict, findings]` in
  declaration order, with no wall clock and no map iteration inside it. The pin
  (`pinned_timestamp`) travels *beside* the digest, not inside it, so a verifier replays with the
  recorded instant — the sentence the specification needs (`05-evidence-model.md`) and could not
  make until this existed.

`GateSetup` (native-only, `gate::infrastructure`) is the composition root: it is the one place
that knows a malformed `preconditions.toml` arms a fail-closed gate rather than none. It is gated
out of the portable slice — the slice takes ports from the host — and
`validate-portable-inventory.sh --gates` holds the manifest to that, in both directions: every gated
`pub mod` needs a row and every row needs a gate.

## Evidence

Every number states its artifact and whether it reproduces. `main` means the merged default branch;
a branch name means the apparatus is still there and the number cannot be reproduced from `main`.

| Fact | Value | Artifact | Reproducible |
|---|---|---|---|
| Empty seam (`--no-default-features`), before #1401 made the engine-linked module the default | 169 250 B (budget 211 562 B, i.e. 25 % over the measured 169 250 B) | `wasm/pkg/*.wasm` after `strip_name_section.py`; `ci/wasm-build.sh` profile; `wasm-bindgen` 0.2.130 | **yes** — `wasm/ci/check-bundle-size.sh`, presence assertion first, and `ACTUAL == MEASURED` |
| Engine-linked module, as shipped | 1 334 325 B = **88.95 %** of the 1 500 000 B ceiling (#1386: the size profile is the difference — 1 334 325 B is the ceiling minus 165 675 B; #1408: the build remaps the registry, the toolchain and the sysroot, so this number is host-independent); emitted 1 766 960 B = **117.80 %** of the 1 500 000 B ceiling before the strip | | `default`/`--features engine`, `ci/wasm-build.sh` (fat LTO, opt-level z, panic abort, cgu 1), stripped | **yes** — `wasm/ci/run-wasm-gates.sh`; the number the flip decision rests on |
| `name` section removed by the strip | 432 635 B = **24.48 %** of the 1 766 960 B emitted engine-linked module — larger under LTO, not smaller, because symbols survive inlining | same build | **yes** — `ci/strip_name_section.py --self-test`, then `--check` on the artifact |
| `wasm32` dependency graph under `portable` | 74 crates (normal + build), exit 0 | `cargo tree -p rigorix-engine --features portable --target wasm32-unknown-unknown -e normal,build` on `issue/1351` @ `c118cb06` | **no, yet** — `portable` is not on `main`; `Cargo.lock`-pinned when it is |
| Modules excluded by the slice | 22 of 39 public modules (10 forbidden crate, 6 narrowed `tokio`, 6 closure), 23 inner items | `engine/.pi/portable-inventory.tsv` | **yes** — the manifest's rows; every count recomputed by `validate-portable-inventory.sh` |
| M2 witness: pinned refusal, both entry points | `c47b27f9…`, 853 B, equal across two runs | `main`, `engine/examples/pinned_refusal.rs` and `rigorix-cli --features pinned-inputs pinned-refusal` | **yes** — `tests/unit/pinnedinputs::the_documented_pins_produce_the_golden_digest` |
| Unpinned control | **the digest differs every run**, 860 B | `main`, same two commands | the variance *is* the result; no value is claimed |
| Apparatus-era readings | 877 B pinned / 884 B unpinned, `32211b9d…` | `examples/spike_m2_envelope.rs` on `issue/1351` @ `c118cb06` | **no** — the fixture was renamed in the port (#1357); kept as history in §2 of the report |
| Remaining size lever | `data` = 416 784 B = **31.24 %** of the 1 334 325 B shipped engine-linked module — and 23.59 % of the 1 766 960 B emitted engine-linked module | section table, same branch | **no, yet** — named here so the next size question starts from the section table, not from a dependency hunt |

The witness command, so the claim above is one line:

```bash
cargo run -q -p rigorix-engine --example pinned_refusal -- \
  --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001
# pinned_clock=true timestamp=2026-11-03T12:00:00+00:00 bytes=853 sha256=c47b27f9…
```

## Alternatives considered

| Alternative | Pros | Cons | Why rejected |
|---|---|---|---|
| Thread `--pinned-clock` / `--pinned-ids` through the CLI composition root | the pin reaches every id minted beneath it; no new type | four factory sites (`cli/src/cli_boundary/orchestrator.rs`) plus every id source down the tree — Phase-B scope | it is Phase B work, and the witness does not need it: one operation, one site |
| Two hand-written fixtures, one per runtime | no shared code to argue about | compares two fixtures; a drift between them reads as an engine difference | it would have measured the author, not the runtime |
| `Clock` / `IdSource` traits in the engine now | matches the roadmap as written; general | one implementor per trait on the gate path; paid for by every caller | superseded (see Decision §2); revisit when a second consumer exists |
| Narrowed demo — browser verifies, gate stays server/CLI-side | sidesteps the size problem | the size problem did not exist on the shipped artifact | **withdrawn** with the M3 correction (#1354); gate evaluation in the browser stays on the table |
| Extract a `rigorix-gate-core` crate (branch A) | smaller dependency surface by construction | requires the exclusion list to be larger than predicted | did not fire: M1 exits 0 with 74 crates and no forbidden crate |

## Consequences

### Positive

- Browser gate evaluation is unblocked on evidence: 88.95 % of the 1 500 000 B ceiling with headroom, and byte
  equality that fails if the fixture moves.
- The determinism witness is one command and one test, so "the browser decides the same" can be
  re-checked by anyone in a minute instead of trusted.
- `PinnedInputs` is additive: no public DTO changed shape, and no downstream constructor breaks.
- The exclusion list has one home, so the slice's surface is reviewable in one file and cannot drift
  from the code silently.

### Negative

- 6 of 22 module exclusions follow only from another exclusion being named, and 6 because they need
  a `tokio` feature the slice drops — so the dependency list understates the real cost, and the graph
  is closed over error types and ports.
- The inventory is measured on a branch, not on `main`, until Phase C lands the wiring — the two
  rows marked "no, yet" above are the debt that Phase C converts into reproducible facts.
- `data` is 31.24 % of the shipped module (416 784 B of 1 334 325 B). If the demo ever needs to
  shrink, that is the lever, and it is not a dependency question.
- The golden digest is a ratchet: any deliberate fixture change now costs a test edit *and* a report
  edit in the same commit. That is the intent, and it is not free.

### Neutral

- `portable` is non-default, so nothing about the normal build changes and the wasm crate's contract
  tests keep their allow-list.
- The `pinned-inputs` feature on the CLI stays non-default and hidden: it is this record's
  carrier exposed at the CLI, so it is named for what it does rather than for the spike that first
  needed a measurement (#1369), and it exists to make the witness runnable from both entry points,
  not as a supported surface. `cli/tests/pinned_inputs_surface.rs` fails if the default build ever
  advertises it.
- `criterion 25`'s second-process test and M4's cross-process determinism remain distinct; M4 runs
  real policy evaluation, not the stub gate.

## Implementation

**Modules:**
- `.pi/architecture/modules/portable-session.md` (the slice's consumer)
- `engine/.pi/portable-inventory.tsv` (the manifest this record cites)

**Files:**
- `engine/src/audit/application/pinned.rs` — the shared operation
- `engine/src/audit/application/dto/pinned_inputs.rs` — `PinnedInputs`
- `engine/src/audit/application/envelope_factory_impl.rs` — `with_pinned_inputs`
- `engine/tests/unit/pinnedinputs/pinnedinputs_test.rs` — the golden witness
- `wasm/ci/strip_name_section.py`, `wasm/ci/check-bundle-size.sh` — the size subject
- `.pi/scripts/validate-portable-inventory.sh` — the guard behind §1

**Next, in this order:**

1. **§1 lands in the engine** (#1374). Until it does, `--features portable` does not exist in
   `engine/Cargo.toml` and `cargo check --features portable --target wasm32-unknown-unknown` fails in
   `tree-sitter`'s build script. The mechanism is measured — 170 `cfg(not(feature = "portable"))`
   sites across 23 files on branch `c118cb06` — but that branch is 16 commits behind `main` and
   merging it would revert them, so it lands as an implementation: gates rebased onto `main`'s
   `PinnedInputs` carrier (the apparatus put the pins in `BuildEnvelopeInput`, which this record's
   §2 refuses), a CI job that builds and tests the configuration, and `validate-portable-inventory.sh
   --gates` comparing this manifest with the code in both directions.
2. **Phase C** wires `EngineGate` to the real envelope, conformance flips to `gate: "cli"`, and
   goldens are regenerated from the same-commit CLI. After that, the two "no, yet" rows above become
   reproducible on `main` and this record's evidence table has no branch-bound entries.

## Validation

- `.pi/scripts/validate-portable-inventory.sh` — the manifest exists, names real things, and is not
  restated here.
- `cargo test -p rigorix-engine --test unit pinned` — the witness, plus its sensitivity seeds.
- `wasm/ci/run-wasm-gates.sh` — ten gates, including the module smoke (packaged artifact, stripped)
  and the size gate (presence assertion before the measurement).
- `.pi/scripts/validate-docs.sh` — every backticked identifier in the report and this record exists.

## References

- `docs/spike-portable-slice-2026-11.md` — the measurement this record decides on
- `engine/.pi/portable-inventory.tsv` — the exclusion list, cited and not restated
- ADR-012 (failure semantics on wasm32), ADR-013 (who verifies) — the two earlier decisions this
  slice depends on
- Issues #1351 (spike), #1353/#1354 (the size subject and the withdrawn verdict), #1355 (the
  carrier), #1357/#1359 (the witnesses)

---

*Decision date: 2026-11-04*
*Decision makers: architecture-coordinator, on measured evidence from the Phase -1 spike*

## Derivation table (every ratio above, with its operands)

| Ratio | Operands | Value |
|---|---|---|
| engine-linked, as shipped vs ceiling | 1 334 325 / 1 500 000 | **88.95 %** of the 1 500 000 B ceiling |
| engine-linked, as emitted vs ceiling | 1 766 960 / 1 500 000 | **117.80 %** of the 1 500 000 B ceiling |
| `name` section share of the emitted module | 432 635 / 1 766 960 | **24.48 %** of the 1 766 960 B emitted engine-linked module |
| `data` segment share of the shipped module | 416 784 / 1 334 325 | **31.24 %** of the 1 334 325 B shipped engine-linked module |
| `data` segment share of the emitted module | 416 784 / 1 766 960 | **23.59 %** of the 1 766 960 B emitted engine-linked module |
| default module vs its budget | 169 250 × 1.25 = 211 562 | **25 % over the measured 169 250 B** |
