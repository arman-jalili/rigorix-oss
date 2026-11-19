#!/usr/bin/env bash
# ============================================================================
# check-packaged-surface.sh — the crate a consumer downloads exposes the
#                             surface the release claims (#1591)
# ============================================================================
# The defect this closes: the workspace can reach a module through a path a consumer cannot. `pub mod
# receipt;` is `#[cfg(not(feature = "portable"))]` (`engine/src/lib.rs`), so a consumer that pins
# `rigorix-engine` from the registry with default features gets the module — and a consumer with
# `--features portable` does not, silently, with "unresolved import". Nothing in this repository
# asserted either half: `publish-crate.sh` runs `cargo publish -p rigorix-engine`, and `cargo publish`
# verifies that the crate *compiles*, not that what it compiles is reachable.
#
# The subject is the **packaged tarball**, not the workspace:
#
#   1. `cargo package` builds `target/package/rigorix-engine-<version>.crate` from the tree;
#   2. the tarball is unpacked into a temporary directory;
#   3. a consumer crate is written with `rigorix-engine = "=<version>"` — a registry requirement, not a
#      path — and `[patch.crates-io]` pointing at the unpacked tarball, which is how the packaged
#      source is the one that resolves while the *requirement* stays the version a host would write;
#   4. `cargo check` runs in the consumer, naming `rigorix_engine::receipt::{Receipt, verify,
#      Attestation}` and reading `Attestation::receipt_digest()`, plus — since #1619 — calling
#      `receipt::run_receipt` with a `RunReceiptConfig`, reading `RunReceiptOutput` and matching
#      `RunReceiptError`; and — since #1625 — calling `gate::infrastructure::assemble` with a
#      `GovernedAssemblyConfig`, reading `GovernedAssembly` and `AssemblyError`. A packaged surface
#      that lost either door fails here, before publish.
#
# Two assertions, because the feature boundary is the interesting half:
#
#   * default features → the three paths resolve;
#   * `features = ["portable"]` → the check **fails**, and the failure is the missing module. That is
#     the documented boundary, asserted rather than hoped for. The receipt module is deliberately not
#     in the portable slice (it needs `quality_gates`, which the slice removes); the assertion pins
#     the consequence so a host learns it from a build, not from a run.
#
# Usage:
#   .github/scripts/check-packaged-surface.sh             # the check (gating, before publish)
#   .github/scripts/check-packaged-surface.sh --self-test # plant the two defects and require a refusal
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT" || exit 1

CRATE="${CRATE:-rigorix-engine}"
MANIFEST="${MANIFEST:-engine/Cargo.toml}"
SELF_TEST=0
[[ "${1:-}" == "--self-test" ]] && SELF_TEST=1

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

# The call shape the fixture compiles against, stated as a *value* so the failure prints it (#1643).
#
# 1.16.0 was tagged and published nothing: `#1629` removed `run_receipt`'s `steps` parameter, the
# fixture below kept the old argument list, and the only failure was `E0061` from a script that ran
# nowhere until the tag existed. A reader who has the expected shape, the artifact's name and the
# version finds that in ten seconds; a reader who has `E0061` starts an investigation.
EXPECTED_SHAPE="$(cat <<'SHAPE'
    receipt::run_receipt(
        workspace: &Path,
        receipt:   &Receipt,
        config:    RunReceiptConfig { state_dir, audit_key, permission_mode },
    ) -> Result<RunReceiptOutput, RunReceiptError>

    gate::infrastructure::assemble(
        config: GovernedAssemblyConfig,
    ) -> Result<GovernedAssembly, AssemblyError>

    receipt::Receipt::from_document(&str) -> Result<Receipt, Vec<ReceiptRefusal>>
    receipt::verify(&[u8], &[u8]) -> Result<VerifiedAttestation, String>
    Attestation::receipt_digest(&self) -> &str
SHAPE
)"

print_expected_shape() {
    printf '\n%s  the consumer compiles against this call shape — a mismatch is a fixture that did not\n' "$YELLOW"
    printf '  move with the crate, and the fix is the argument list below, not the crate:%s\n' "$NC" >&2
    printf '%s\n' "$EXPECTED_SHAPE" | sed 's/^/      /' >&2
    printf '  crate: %s   version: %s   subject: the packaged tarball (target/package/%s-%s.crate)\n' \
        "$CRATE" "$VERSION" "$CRATE" "$VERSION" >&2
}

version_of() { # $1 manifest → the version the crate claims
    sed -n 's/^version[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$1" | head -1
}

# ── the consumer, as a string so a mutation can be applied to it ─────────────
consumer_main() {
    cat <<'RUST'
//! A consumer of the published surface, written the way a host writes it (#1591).

use rigorix_engine::receipt::{
    Attestation, Receipt, RunReceiptConfig, RunReceiptError, RunReceiptOutput, verify,
};

fn main() {
    // The door a host's document meets.
    let _parsed: Result<Receipt, Vec<_>> = Receipt::from_document("{}");
    // The gate a host calls before dispatching anything.
    let _verifier = verify;
    // The field a host reads the receipt's pinned digest from.
    fn pinned(attestation: &Attestation) -> &str {
        attestation.receipt_digest()
    }
    let _reads_the_field = pinned as fn(&Attestation) -> &str;

    // #1619: the surface a host that *has* a receipt calls. Named, called and read from, because a
    // type that only exists is not a door — this is the shape the harness compiles against.
    async fn run(workspace: &std::path::Path, receipt: &Receipt) -> Result<RunReceiptOutput, RunReceiptError> {
        let config = RunReceiptConfig {
            state_dir: std::path::PathBuf::from("/var/lib/consumer/.rigorix/state"),
            audit_key: std::env::var("RIGORIX_HMAC_KEY").ok(),
            permission_mode: rigorix_engine::permission::domain::PermissionMode::WorkspaceWrite,
        };
        rigorix_engine::receipt::run_receipt(workspace, receipt, config).await
    }
    let _door = run;

    fn answers(output: &RunReceiptOutput) -> bool {
        output.attestation.receipt_digest() == output.receipt_digest
    }
    let _reads_the_answer = answers as fn(&RunReceiptOutput) -> bool;

    // #1625: the governed assembly is the crate's door for a host's engine — one function that
    // attaches the operator's gates. Named and called, so a release that lost it fails here.
    fn assembled(
        config: rigorix_engine::gate::infrastructure::assembly::GovernedAssemblyConfig,
    ) -> impl std::future::Future<
        Output = Result<
            rigorix_engine::gate::infrastructure::assembly::GovernedAssembly,
            rigorix_engine::gate::infrastructure::assembly::AssemblyError,
        >,
    > {
        rigorix_engine::gate::infrastructure::assembly::assemble(config)
    }
    let _governed_door = assembled;

    // And the refusal a host matches on, so the type is not merely present.
    fn refused(error: &RunReceiptError) -> bool {
        matches!(error, RunReceiptError::Refused(_))
    }
    let _matches_the_refusal = refused as fn(&RunReceiptError) -> bool;
}
RUST
}

consumer_manifest() { # $1 unpacked package dir, $2 version, $3 comma list of features (may be empty)
    local unpacked="$1" version="$2" features="$3"
    # A quoted heredoc, because a comment that mentions `[patch.crates-io]` in an unquoted one is a
    # command substitution: measured, and it took a run to see.
    cat <<'EOF'
[package]
name = "receipt-surface-consumer"
version = "0.0.0"
edition = "2021"

EOF
    if [[ -n "$features" ]]; then
        printf '# The requirement a host writes: exactly the version this release publishes, with the\n'
        printf '# feature set being asserted.\n[dependencies]\n%s = { version = "=%s", features = [%s] }\n\n' \
            "$CRATE" "$version" "$features"
    else
        printf '# The requirement a host writes: exactly the version this release publishes.\n'
        printf '[dependencies]\n%s = "=%s"\n\n' "$CRATE" "$version"
    fi
    printf '# The packaged tarball, not the workspace: `[patch.crates-io]` makes the unpacked crate the\n'
    printf '# source of that version, so the check resolves the artifact while the requirement stays one a\n'
    printf '# host would write.\n[patch.crates-io]\n%s = { path = "%s" }\n' "$CRATE" "$unpacked"
}

# ── the run: package, unpack, check the consumer both ways ──────────────────
run_check() { # $1 tarball-version, $2 unpacked dir, $3 mode (quiet|verbose) → 0 both assertions held
    local version="$1" unpacked="$2" mode="$3"
    local work consumer status_portable=0 out_ok out_portable

    work="$(mktemp -d)"
    consumer="$work/consumer"
    mkdir -p "$consumer/src"
    consumer_main > "$consumer/src/main.rs"
    consumer_manifest "$unpacked" "$version" "" > "$consumer/Cargo.toml"

    # The consumer is outside the workspace, so rustup would answer with the runner's `stable` rather
    # than this repository's pin — and with `RUSTFLAGS=-D warnings` inherited from CI's job, a *newer*
    # stable's deprecations read as a compile error in a crate that compiles fine here:
    # `use of deprecated method Atomic::<u32>::fetch_update` and eight more, on 2026-11-19, from a
    # tarball this check had just accepted locally. The subject is "the surface a consumer of *this*
    # release gets", so the consumer compiles with the toolchain the release is built with.
    if [[ -f "$ROOT/rust-toolchain.toml" ]]; then
        cp "$ROOT/rust-toolchain.toml" "$consumer/rust-toolchain.toml"
    elif [[ -f "$ROOT/rust-toolchain" ]]; then
        cp "$ROOT/rust-toolchain" "$consumer/rust-toolchain"
    fi

    # ── assertion 1: default features reach the three paths ──
    # `|| status` rather than `if !`: a pipeline reports the last command's status, and the negation
    # discards it, which would make this branch unreachable (measured in local-ci once already).
    out_ok="$( cd "$consumer" && cargo check --quiet 2>&1 )" || status_ok=$?
    if [[ "${status_ok:-0}" -ne 0 ]]; then
        printf '%s\n' "$out_ok" | tail -20 >&2
        echo -e "${RED}❌ the packaged $CRATE $version does not expose the surface a consumer names${NC}" >&2
        print_expected_shape
        return 1
    fi

    # ── assertion 2: `portable` does NOT, and that is the documented boundary ──
    consumer_manifest "$unpacked" "$version" '"portable"' > "$consumer/Cargo.toml"
    out_portable="$( cd "$consumer" && cargo check --quiet 2>&1 )" || status_portable=$?
    if [[ "${status_portable}" -eq 0 ]]; then
        echo -e "${RED}❌ a consumer with \`features = [\"portable\"]\` compiled — the slice is supposed to exclude the receipt, so either the exclusion moved or this check is measuring the wrong crate${NC}" >&2
        return 1
    fi
    if ! printf '%s' "$out_portable" | grep -qiE "unresolved import|no .receipt. in the root|cannot find module|E0432|E0433"; then
        # A failure for another reason (a broken dependency, a bad network) would pass this check while
        # asserting nothing: the message is the subject, so an unrecognised one is a failure to check.
        printf '%s\n' "$out_portable" | tail -20 >&2
        echo -e "${RED}❌ the portable consumer failed for a reason that is not the missing receipt module${NC}" >&2
        return 1
    fi
    find "$work" -mindepth 0 -delete 2>/dev/null || true
    [[ "$mode" == "verbose" ]] && echo "  both assertions held"
    return 0
}

# ── main ────────────────────────────────────────────────────────────────────
VERSION="$(version_of "$MANIFEST")"
[[ -n "$VERSION" ]] || { echo "❌ check-packaged-surface: no version in $MANIFEST" >&2; exit 2; }

echo "── the packaged surface of $CRATE $VERSION ──"
# `--no-verify` because the point is not to re-run this crate's tests (the pipeline does that) but to
# produce the artifact a consumer downloads.
cargo package -p "$CRATE" --no-verify --allow-dirty >/dev/null 2>&1 || {
    echo "❌ check-packaged-surface: \`cargo package -p $CRATE\` failed — there is no artifact to check" >&2
    exit 1
}
TARBALL="target/package/$CRATE-$VERSION.crate"
[[ -f "$TARBALL" ]] || { echo "❌ check-packaged-surface: $TARBALL was not produced" >&2; exit 1; }

UNPACK="$(mktemp -d)"
trap 'find "${UNPACK:-/nonexistent}" -mindepth 0 -delete 2>/dev/null || true' EXIT
tar xzf "$TARBALL" -C "$UNPACK"
PACKAGED="$UNPACK/$CRATE-$VERSION"
[[ -d "$PACKAGED" ]] || { echo "❌ check-packaged-surface: the tarball did not unpack to $PACKAGED" >&2; exit 1; }

if run_check "$VERSION" "$PACKAGED" verbose; then
    echo -e "${GREEN}✅ packaged surface: $CRATE $VERSION — the tarball target/package/$CRATE-$VERSION.crate — exposes${NC}"
    printf '     %s\n' "receipt::{Receipt::from_document, verify, Attestation::receipt_digest, run_receipt, RunReceiptConfig, RunReceiptOutput, RunReceiptError}"
    printf '     %s\n' "gate::infrastructure::assemble::{GovernedAssembly, GovernedAssemblyConfig, AssemblyError}"
    echo -e "${GREEN}   with default features, and withholds receipt under \`portable\`${NC}"
else
    exit 1
fi

# ── self-test: plant the two defects a consumer check exists to catch ───────
if [[ "$SELF_TEST" -eq 1 ]]; then
    passes=0; fails=0
    echo "── check-packaged-surface --self-test ──"
    # (a) the *type* is renamed in the packaged source, which is the path a consumer names: the check
    # must fail and must say which name it could not find. The first version of this fixture edited
    # `src/receipt/mod.rs` looking for `pub mod receipt;`, which lives in `src/lib.rs` — so it planted
    # nothing and the self-test said the check passed. A mutation that does not apply is not a test.
    planted="$UNPACK/planted-a"; cp -R "$PACKAGED" "$planted"
    python3 - "$planted/src/receipt/mod.rs" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
before = s
s = s.replace("Expectation, Receipt, ReceiptStep,", "Expectation, Receipt as ReceiptX, ReceiptStep,", 1)
assert s != before, "the plant did not apply: `Receipt` is not in the re-export list"
p.write_text(s)
PY
    if out="$(run_check "$VERSION" "$planted" quiet 2>&1)"; then
        echo -e "  ${RED}❌${NC} a renamed type still passed the consumer check"
        fails=$((fails + 1))
    elif printf '%s' "$out" | grep -q "Receipt"; then
        echo -e "  ${GREEN}✅${NC} a renamed \`Receipt\` fails the check, naming the path"
        passes=$((passes + 1))
    else
        echo -e "  ${RED}❌${NC} the check failed, but nothing named \`Receipt\`: $(printf '%s' "$out" | tail -2 | tr '\n' ' ')"
        fails=$((fails + 1))
    fi

    # (b) the module is gated out of the default build: the consumer must fail, and the workspace
    # would not have noticed.
    planted="$UNPACK/planted-b"; cp -R "$PACKAGED" "$planted"
    python3 - "$planted/src/lib.rs" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
s = s.replace("#[cfg(not(feature = \"portable\"))]\npub mod receipt;",
              "#[cfg(feature = \"never_enabled\")]\npub mod receipt;", 1)
p.write_text(s)
PY
    if out="$(run_check "$VERSION" "$planted" quiet 2>&1)"; then
        echo -e "  ${RED}❌${NC} a default build without the module still passed the consumer check"
        fails=$((fails + 1))
    elif printf '%s' "$out" | grep -qi "receipt"; then
        echo -e "  ${GREEN}✅${NC} a default build without \`pub mod receipt\` fails the consumer check, naming the module"
        passes=$((passes + 1))
    else
        echo -e "  ${RED}❌${NC} the check failed, but nothing named \`receipt\`: $(printf '%s' "$out" | tail -2 | tr '\n' ' ')"
        fails=$((fails + 1))
    fi

    echo "  self-test: $passes passed, $fails failed"
    [[ "$fails" -eq 0 ]] || exit 1
fi