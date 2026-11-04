//! The `pinned-inputs` feature is what keeps the measurement surface out of a default build
//! (#1369).
//!
//! @canonical .pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md
//! Canonical Reference: wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md
//! Issue: #1369
//!
//! `rigorix-cli` is published and on the release train, so a feature name and a hidden
//! command are part of its surface whether or not anyone uses them. ADR-014 makes the
//! pinned input a *capability* of the gate seam, not a spike leftover — which is why the
//! feature is called `pinned-inputs` — but a capability is still opt-in, and opt-in is only
//! true if the default binary does not have it.
//!
//! Before this file, nothing proved the `#[cfg]` attribute was load-bearing: delete it and
//! every test still passes, the default build grows a command nobody asked for, and the
//! first symptom is a help page. The pair below is that proof — one side runs in a default
//! build, the other only when the feature is on, so a leaked command fails the first and a
//! vanished one fails the second.
#![allow(clippy::disallowed_types)] // #1083: test-only spawn of the binary under test

use std::process::Command;

/// The binary under test, built with whatever features this test run was built with.
#[cfg(feature = "pinned-inputs")]
fn help() -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_rigorix-cli"))
        .arg("--help")
        .output()
        .expect("the CLI binary runs");
    assert!(
        output.status.success(),
        "`--help` must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[cfg(not(feature = "pinned-inputs"))]
#[test]
fn a_default_build_does_not_accept_the_pinned_refusal_command() {
    // Behavioural, not `--help`. The command is `hide = true`, so a leak is **invisible**
    // in the help text: the first version of this test asserted on `--help` and passed
    // while the command was fully present in the default build. What a leak actually looks
    // like is the command *running*, so that is what is asserted.
    let output = Command::new(env!("CARGO_BIN_EXE_rigorix-cli"))
        .arg("pinned-refusal")
        .output()
        .expect("the CLI binary runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success() && !combined.contains("sha256="),
        "a default build must not accept `pinned-refusal` — the command is behind the \
         non-default `pinned-inputs` feature (ADR-014, #1369). It ran, so the \
         `#[cfg(feature = \"pinned-inputs\")]` attributes are gone or the feature became \
         default:\n--- output begin ---\n{combined}\n--- output end ---"
    );
}

#[cfg(feature = "pinned-inputs")]
#[test]
fn the_pinned_inputs_feature_adds_the_command() {
    // Hidden commands do not appear in `--help`, so the honest check is `--help` for the
    // absence and a parse attempt for the presence: `pinned-refusal` with no arguments is a
    // valid invocation and prints the experiment's summary line.
    let output = Command::new(env!("CARGO_BIN_EXE_rigorix-cli"))
        .arg("pinned-refusal")
        .output()
        .expect("the CLI binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("sha256="),
        "with `--features pinned-inputs` the command must exist and run; exit {:?}, stdout: {stdout}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "pinned-inputs")]
#[test]
fn the_command_is_hidden_from_help() {
    // Hidden is deliberate: the pin is a determinism tool for comparing runtimes, not a
    // user-facing flag (ADR-014). If it ever becomes visible, that is a decision, not a
    // default.
    let help = help();
    assert!(
        !help.contains("pinned-refusal"),
        "`pinned-refusal` is `hide = true` by design; a visible entry is a product decision:\n{help}"
    );
}
