---
guardian_issue:
  id: "ISSUE-SPIKE-M2-FEATURE-SHIPS-IN-PUBLISHED-CRATE"
  title: "`rigorix-cli` 1.12.1 publishes a feature named `spike-m2` and a hidden `pinned-refusal` command, and no test proves a default build excludes them"
  epic: "portable-slice"
  component: "CLI"
  module: "wasm-seam"
  status: planned
  priority: medium
  tracking_issue: 1369
  dependencies:
    - "ADR-014 (§2 — the pinned-input carrier is the decision's subject, not a spike's)"
    - "the next rigorix-cli publication"

  intent: |
    #1355 needed a measurement surface: pinned clock and ids for one refusal. It shipped as

    ```toml
    # cli/Cargo.toml:18
    spike-m2 = []
    ```

    ```rust
    // cli/src/cli_boundary/cli.rs:287-288
    #[cfg(feature = "spike-m2")]
    #[command(name = "pinned-refusal", hide = true)]
    ```

    `rigorix-cli` is published, on the release train, at 1.12.1. So a crate a user depends on now
    advertises a feature named after an internal spike, and the name is the only thing wrong with it:
    ADR-014 decided that `PinnedInputs` + `with_pinned_inputs` are *the* determinism path for the gate
    seam, which makes the surface a capability, not a measurement leftover.

    Nothing proves the `#[cfg]` is load-bearing. Delete the attribute and every test still passes,
    the default build grows a command nobody asked for, and the first symptom is a help page. The
    guard that would catch it is a negative assertion, and it does not exist:

    ```
    $ grep -rn 'spike-m2|pinned-refusal' cli/src/cli_boundary/tests.rs cli/tests/ → nothing
    ```

  in_scope:
    - "The feature is renamed to `pinned-inputs` and documented as the ADR-014 carrier's CLI surface: non-default, absent from a default build, and named for what it does"
    - "A test asserts a default build's `--help` contains no `pinned-refusal`, and that the same binary built with `--features pinned-inputs` does contain it"
    - "CHANGELOG records the feature and the rename"

  out_of_scope:
    - "Promoting the surface to a default feature. ADR-014 keeps it opt-in; the pin is a determinism tool, not a user-facing flag"
    - "The engine's `PinnedInputs` API, which is additive and already tested"
    - "Wiring `EngineGate` — Phase C"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "cli/Cargo.toml — the feature name"
    interfaces:
      - "cli/src/cli_boundary/cli.rs — the hidden command"

  citations:
    - file: "cli/Cargo.toml"
      note: ":13-18 `[features]`, `default = []`, `spike-m2 = []`"
    - file: "cli/src/cli_boundary/cli.rs"
      note: ":287-288 `#[cfg(feature = \"spike-m2\")]` on `#[command(name = \"pinned-refusal\", hide = true)]`"
    - file: "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"
      note: "§2 — `PinnedInputs` through an additive method is the decision"

  verification:
    - "`cargo run -q -p rigorix-cli -- --help | grep -c pinned-refusal` returns 0 on a default build"
    - "`cargo run -q -p rigorix-cli --features pinned-inputs -- --help | grep -c pinned-refusal` returns 1"
    - "Seed: remove the `#[cfg(feature = \"pinned-inputs\")]` attribute → the default-help test fails naming the leaked command; restored, it passes"
    - "`grep -rc 'spike-m2' cli/ CHANGELOG.md` returns 0"
---

# A capability wearing a spike's name

The measurement surface is legitimate — it is how the one-site claim was proven and how the golden
digest is produced. What is not legitimate is shipping it in a published crate under an internal
name, with no test that the feature is what keeps it out of the default build.

Naming it `pinned-inputs` also fixes a documentation problem: today a reader who finds `spike-m2` in
a published crate has to guess whether it is supported.
