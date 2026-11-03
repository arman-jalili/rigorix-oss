---
guardian_issue:
  id: "ISSUE-M2-CLI-HALF-PINNED-INPUTS"
  title: "M2's CLI half: one precondition refusal reaching a byte-identical envelope from the CLI and from the crate, carried without adding a field to a public DTO"
  epic: "portable-slice"
  component: "Engine"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1355
  dependencies:
    - "#1351 M2 (the measurement this completes)"
    - "the spike report's corrected verdict (M2 is the only outstanding measurement)"

  intent: |
    The spike measured the operation twice and got the same digest twice *in the same runtime*. The
    CLI half was never reached, so the property the whole wasm crate exists to guarantee — the engine
    and its wasm slice decide identically on identical input — is unproven.

    Two facts from the spike make this small rather than large:

    1. **The clock was the only unpinned value in the operation.** One threaded site
       (`pinned_timestamp`) sufficed, not 151. The 94 `Utc::now()` and 57 `Uuid::new_v4()` sites in
       the gate modules are in callers that the operation does not traverse.
    2. **Digest equality is not payload equality.** Unpinned 884 B vs pinned 877 B — a whole-second
       instant serialises shorter than a nanosecond one. A comparison that normalises either side
       hides exactly the difference this measurement exists to find, so nothing may normalise.

    The carrier is the part that needs care. The spike added
    `BuildEnvelopeInput::pinned_timestamp`, which is a **public field on a public, literally
    constructible DTO**, and two other crates in this workspace construct that DTO literally:
    `server/tests/audit_scope_check_verify.rs:20` and `mcp/src/host/mod.rs:681`. Adding a field to a
    non-`#[non_exhaustive]` struct is a breaking change under Rust's semver rules, and it forced 11
    test edits. Making the struct `#[non_exhaustive]` would break those literals harder.

    So the pinned inputs travel in their **own type** through an **additive** method. No existing
    public item gains a field, no existing signature changes, and the four published crates detect
    nothing — which is the same M1b standard the `portable` feature had to meet.

  in_scope:
    - "A new value carrying the pinned instant and the pinned id draws, owned by the engine, additive to the public surface"
    - "An additive factory method that accepts it; `build_and_send` and `BuildEnvelopeInput` keep today's shapes exactly, and `pinned_timestamp` is removed from the DTO"
    - "CLI flags supplying pinned clock and ids, behind a non-default cargo feature, so a default build's behaviour and surface are unchanged"
    - "The envelope factory stamps the pinned instant when one is supplied and reads the wall clock otherwise, so `None` remains today's behaviour"
    - "The M2 measurement, run end to end: one precondition refusal, CLI and in-crate, digests compared with no normalisation"

  out_of_scope:
    - "Threading the seam through the remaining sites (Phase B). The measurement needs the one operation, and the count it recorded is 1"
    - "The wasm side: this issue produces the CLI-side envelope and the comparison; wiring `EngineGate` to it is Phase C"
    - "Conformance goldens, `gate: \"cli\"`, or deleting `normalised_comparison` — untouched here"
    - "A `Clock`/`IdSource` trait pair across the engine. The evidence does not support the abstraction yet: one operation, one site, and the ids in this operation are already supplied through the fixture ports"

  affected_layers:
    domain:
      - "None — no domain type changes; the pinned value is an application input"
    application:
      - "engine/src/audit/application/factory (the additive method) and the envelope factory's timestamp selection"
      - "engine/src/audit/application/dto/mod.rs — `pinned_timestamp` removed from `BuildEnvelopeInput`"
    infrastructure:
      - "cli: the pinned-input flags behind a non-default feature, read into the new value"
    interfaces:
      - "cli: one new flag pair; no existing flag changes"

  citations:
    - file: "engine/src/audit/application/dto/mod.rs"
      note: ":124 `pub pinned_timestamp` — the field this issue removes; the struct is public and not `#[non_exhaustive]`"
    - file: "engine/src/audit/application/envelope_factory_impl.rs"
      note: ":182 `timestamp: input.pinned_timestamp.unwrap_or_else(chrono::Utc::now)` — the single stamping site; it must read the new carrier"
    - file: "server/tests/audit_scope_check_verify.rs"
      note: ":20 a literal `BuildEnvelopeInput { … }` from another crate — the reason a field cannot be added"
    - file: "mcp/src/host/mod.rs"
      note: ":681 the second literal construction, in a published crate"
    - file: "cli/src/cli_boundary/orchestrator.rs"
      note: ":198, :274, :281, :309 `create_default()` — where the pinned inputs reach the factory"

  verification:
    - "`cargo build -p rigorix-engine -p rigorix-mcp -p rigorix-server` succeeds with zero edits to `server/tests/audit_scope_check_verify.rs` and `mcp/src/host/mod.rs`; `git diff --stat` on those two files is empty"
    - "`cargo test -p rigorix-engine -p rigorix-mcp -p rigorix-server` passes unmodified — the same M1b standard"
    - "The CLI, given the pinned fixture, produces an envelope whose sha256 equals the in-crate run's sha256 for the same refusal; both digests and both commands appear in the M2 report"
    - "The comparison normalises nothing: a seeded mismatch of one second in the pinned instant changes the digest (the measurement must fail on it, not absorb it)"
    - "An unpinned CLI run and a pinned CLI run of the same refusal differ in size (884 B vs 877 B today) — recorded as evidence that the pin is load-bearing, not a value that happens to be equal"
    - "Default-feature `cargo run -p rigorix-cli -- run …` behaves exactly as 1.12.1: the flags are behind the feature and absent otherwise"
---

# M2, the CLI half

## Why this is the last measurement standing

| | status |
|---|---|
| M1 dependency surface | holds — 74 crates, no forbidden dependency |
| M1b four published crates | holds once the DTO field is removed (the 11 edits were its plumbing) |
| M2 in-crate | holds — 32211b9d… twice |
| M2 CLI | **not reached** |
| M3 shipped bundle | holds — 1 170 635 B against 1 500 000 B |
| M4 cross-process determinism | holds once the clock is pinned |

One row remains. It is also the row that makes the crate's existence meaningful: without it, "the
browser decides the same as the engine" is a design intention with a digest that only ever compared
the crate to itself.

## The carrier is a semver decision, not a style one

```
$ grep -rn 'BuildEnvelopeInput {' --include=*.rs server/ mcp/
server/tests/audit_scope_check_verify.rs:20:    BuildEnvelopeInput {
mcp/src/host/mod.rs:681:            .build_and_send(BuildEnvelopeInput {
```

Two other crates construct that DTO literally, so a new public field is a breaking change — the
kind that only shows up for a downstream user who never appears in this workspace's test run. A new
type plus an additive method costs nothing by comparison and keeps the four published crates at
1.12.1's surface.

## What the measurement must not do

- **No normalised comparison.** If the two envelopes differ in timestamp width, that is a finding,
  not noise to strip.
- **No re-running until the digests agree.** A flip is a defect; the seed in the criteria exists to
  prove the comparison can fail at all.
- **No widening to the full run.** One refusal, one envelope, two runtimes. The five use cases and
  the demo are downstream of this and stay untouched.
