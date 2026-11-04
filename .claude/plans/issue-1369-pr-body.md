A capability wearing a spike's name, and no test that the `#[cfg]` was load-bearing.

## The rename

`spike-m2` → `pinned-inputs`, six sites: `cli/Cargo.toml`, the CLI's `CliCommand` variant, three dispatch sites, the engine example's doc comment, and the two documents that name the command (`docs/spike-portable-slice-2026-11.md` §2, ADR-014 §Evidence/Consequences). CHANGELOG records it under `Unreleased → Changed`.

The measurement surface was legitimate — it is how the one-site claim was proven and how the golden digest is produced. What was not legitimate is shipping it in a published crate under an internal name: ADR-014 makes the pinned input *the* determinism path for the gate seam, so this is a capability whose name should say what it does.

Still non-default, still `hide = true`. A default build's commands, flags and behaviour are unchanged.

## The test, and the version of it that couldn't fail

`cli/tests/pinned_inputs_surface.rs`, one half per feature configuration:

| build | assertion |
|---|---|
| default (`cargo test -p rigorix-cli`) | the binary must **refuse** `pinned-refusal` — non-zero exit, no `sha256=` |
| `--features pinned-inputs` | the command runs and prints `sha256=`, and is still absent from `--help` |

**The first version asserted `--help` contains no `pinned-refusal`, and it passed while the command was fully present.** I seeded the leak by deleting both `#[cfg]` attributes:

```
pinned_clock=false timestamp=… bytes=860 sha256=6212177f…     ← the command ran
$ rigorix-cli --help | grep -c pinned-refusal
0                                                              ← and help said nothing
```

`hide = true` means a leaked command is invisible in help. The assertion was measuring the wrong surface — so it now measures the surface a leak actually shows on.

## What the seed found one layer down

Removing **only** the variant's `#[cfg]` does not compile:

```
error[E0004]: non-exhaustive patterns: `CliCommand::PinnedRefusal { .. }` not covered
   --> cli/src/cli_boundary/dispatch.rs:1239:11
```

The dispatch arm is gated too, so a partial revert is a build error rather than a silent leak. The test covers the case both attributes go, or the feature becomes default (`default = ["pinned-inputs"]`).

## CI

`cargo test --workspace` builds the CLI **without** features, so only the absence half would ever run. The integration job now runs both halves:

```yaml
cargo test -p rigorix-cli --test pinned_inputs_surface
cargo test -p rigorix-cli --features pinned-inputs --test pinned_inputs_surface
```

## Witness unchanged across the rename

```
$ rigorix pinned-refusal --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001
pinned_clock=true … bytes=853 sha256=c47b27f9…
$ cargo run -q -p rigorix-engine --example pinned_refusal -- (same pins)
pinned_clock=true … bytes=853 sha256=c47b27f9…
```

## Validation

```
cargo fmt --all --check                       clean
cargo clippy --workspace --all-targets       0 errors, 0 warnings
cargo test -p rigorix-cli --test pinned_inputs_surface                1 passed (default)
cargo test -p rigorix-cli --features pinned-inputs --test …           2 passed
docs / guards / ci / canonical / tests / architecture / script-drift  all exit 0
```

The full `cargo test --workspace` was not re-run after the rename: the target directory was cleaned mid-session and a cold rebuild of 46 engine integration binaries plus a second wasm target outlasted the budget. The per-package numbers from before the cold start were engine 61 s / 2916 passed, and only the CLI feature name changed since.

Closes #1369
