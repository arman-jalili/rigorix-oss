#!/usr/bin/env bash
# check-release-wiring.sh — the artifact is on the release page, and the page is not a claim.
#
#   bash wasi/ci/check-release-wiring.sh [--root <dir>]
#   bash wasi/ci/check-release-wiring.sh --self-test
#
# # Why this is not `validate-release-train.sh`
#
# That guard derives the *crates.io train* from `cargo metadata`, and `wasi/Cargo.toml` is
# `publish = false` on purpose: this ships as an asset, not as a crate. So the train guard is
# right to ignore it — and that silence is exactly how an artifact ends up with a build job
# and no attachment, or an attachment built by a different command than the one that was
# gated. Five checks, each one naming a string that must exist:
#
#   1. a `package-wasi` job exists;
#   2. `github-release` needs it (a job nobody waits for is a job that can fail silently);
#   3. the job builds through `wasi/ci/build.sh` — the *identity* of the artifact, not a copy
#      of its target and flags;
#   4. the job gates what it built (imports, size, both conformance modes) before staging;
#   5. the page attaches both the module and its digest, and verifies the digest first.
#
# A missing string is a failure. A check that passes because it could not read the file is
# not a check, so an unreadable or empty workflow fails closed.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
MODE="check"

while [ $# -gt 0 ]; do
  case "$1" in
    --root) [ $# -ge 2 ] || { echo "check-release-wiring: --root needs a directory" >&2; exit 2; }; ROOT="$2"; shift 2 ;;
    --self-test) MODE="self-test"; shift ;;
    -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "check-release-wiring: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

# Every check is a question with a name, so a failure says which promise broke.
check() {
  local file="$1" want="$2" why="$3"
  if grep -qF -- "$want" "$file"; then
    echo "✓ $why"
    return 0
  fi
  echo "✗ $why" >&2
  echo "    release.yml never mentions: $want" >&2
  return 1
}

run_checks() {
  local workflow="$1"
  local failures=0
  if [ ! -s "$workflow" ]; then
    echo "✗ $workflow is missing or empty — the wiring cannot be read, so it does not pass" >&2
    return 1
  fi
  check "$workflow" "  package-wasi:" "a package-wasi job exists" || failures=$((failures + 1))
  check "$workflow" "verify-published, package-wasi]" "github-release waits for package-wasi" || failures=$((failures + 1))
  check "$workflow" "bash wasi/ci/build.sh" "the job builds through wasi/ci/build.sh" || failures=$((failures + 1))
  check "$workflow" "bash wasi/ci/check-imports.sh" "the job gates the import section it built" || failures=$((failures + 1))
  check "$workflow" "bash wasi/ci/check-size.sh" "the job measures the artifact against its ceiling" || failures=$((failures + 1))
  check "$workflow" "--truncate-request" "the job proves a truncated request is a refusal" || failures=$((failures + 1))
  check "$workflow" "sha256sum -c rigorix_gate_wasi.wasm.sha256" "the digest is verified, not written and trusted" || failures=$((failures + 1))
  check "$workflow" "wasi-asset/rigorix_gate_wasi.wasm" "the release page attaches the module" || failures=$((failures + 1))
  check "$workflow" "wasi-asset/rigorix_gate_wasi.wasm.sha256" "the release page attaches the digest" || failures=$((failures + 1))
  return "$failures"
}

if [ "$MODE" = "check" ]; then
  run_checks "$ROOT/.github/workflows/release.yml"
  exit 0
fi

# ── self-test: every check must be able to fail ───────────────────────────────────
scratch="$(mktemp -d)"
trap 'find "$scratch" -type f -delete; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT
source_workflow="$ROOT/.github/workflows/release.yml"
[ -s "$source_workflow" ] || { echo "self-test: $source_workflow is missing or empty" >&2; exit 1; }

# 1. the real workflow passes
mkdir -p "$scratch/ok/.github/workflows"
cp "$source_workflow" "$scratch/ok/.github/workflows/release.yml"
if ! run_checks "$scratch/ok/.github/workflows/release.yml" >/dev/null; then
  echo "self-test: the real release.yml does not pass its own wiring check" >&2
  exit 1
fi
echo "✓ self-test: the real workflow passes"

# 2. one check at a time, each removed: the check that names it must fail, and only then.
assert_bites() {
  local label="$1" needle="$2" why="$3"
  mkdir -p "$scratch/case/.github/workflows"
  grep -vF -- "$needle" "$source_workflow" > "$scratch/case/.github/workflows/release.yml"
  if run_checks "$scratch/case/.github/workflows/release.yml" >/dev/null 2>&1; then
    echo "self-test: removing '$label' still passed — that check has no subject" >&2
    exit 1
  fi
  echo "✓ self-test: without '$label', the wiring fails ($why)"
}

assert_bites "the build script" "bash wasi/ci/build.sh" "a build that is not the gated one"
assert_bites "the import gate" "bash wasi/ci/check-imports.sh" "an artifact nobody inspected"
assert_bites "the size ceiling" "bash wasi/ci/check-size.sh" "a size nobody measured"
assert_bites "the digest verification" "sha256sum -c rigorix_gate_wasi.wasm.sha256" "a digest nobody checked"
assert_bites "the attached module" "wasi-asset/rigorix_gate_wasi.wasm" "a page without the artifact"
assert_bites "the attached digest" "wasi-asset/rigorix_gate_wasi.wasm.sha256" "a page without the digest"

# 3. a missing workflow fails closed rather than reporting nothing to check.
mkdir -p "$scratch/empty/.github/workflows"
if run_checks "$scratch/empty/.github/workflows/release.yml" >/dev/null 2>&1; then
  echo "self-test: a missing workflow passed — the check reports on a tree it never read" >&2
  exit 1
fi
echo "✓ self-test: a missing workflow fails closed"
