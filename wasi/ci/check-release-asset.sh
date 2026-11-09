#!/usr/bin/env bash
# check-release-asset.sh — the artifact has an address, and the address is the artifact.
#
#   bash wasi/ci/check-release-asset.sh <tag>
#   bash wasi/ci/check-release-asset.sh <tag> --expect <sha256>
#   bash wasi/ci/check-release-asset.sh --from-dir <dir> [--expect <sha256>]
#   bash wasi/ci/check-release-asset.sh --self-test
#
# # Why this is not `check-release-wiring.sh`
#
# The wiring guard reads the workflow and asks "does the job attach the artifact?" — five
# strings that must exist in YAML. That guard was green for the whole life of `package-wasi`
# while the job had never executed and `v1.14.2` carried **no assets at all** (#1503). A job
# read from YAML is not a job that runs, and a string in a workflow is not something a
# downstream project can download.
#
# So this guard asks the other question, and it can only be answered by the release itself:
#
#   1. the release exists and can be read — a `gh` failure is *not* "no asset", and the two
#      have different remedies, so they are different messages;
#   2. the release carries a `.wasm` **and** its `.wasm.sha256` — named, not counted, because
#      "assets: 1" tells nobody which one;
#   3. the digest file describes *that* module — `sha256sum -c`, so a page whose digest was
#      copied from another build fails here;
#   4. with `--expect <sha256>`, the published digest equals the expected one — that is the
#      "the page is the build, not a claim about it" check, and it is how a rebuild of the tag
#      is compared against what the world downloads.
#
# The verdict is printed with the digest, because the digest is what the downstream copy
# records (rigorix-sdk vendors this artifact and pins it by sha256).
set -euo pipefail

TAG=""
FROM_DIR=""
EXPECT=""
MODE="check"

while [ $# -gt 0 ]; do
  case "$1" in
    --tag) [ $# -ge 2 ] || { echo "check-release-asset: --tag needs a value" >&2; exit 2; }; TAG="$2"; shift 2 ;;
    --from-dir) [ $# -ge 2 ] || { echo "check-release-asset: --from-dir needs a directory" >&2; exit 2; }; FROM_DIR="$2"; shift 2 ;;
    --expect) [ $# -ge 2 ] || { echo "check-release-asset: --expect needs a sha256" >&2; exit 2; }; EXPECT="$2"; shift 2 ;;
    --self-test) MODE="self-test"; shift ;;
    -h|--help) sed -n '2,35p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "check-release-asset: unknown argument '$1'" >&2; exit 2 ;;
    *) TAG="$1"; shift ;;
  esac
done

fail() { echo "✗ $*" >&2; return 1; }

# The check body, over a directory that holds the release's assets. Separate from the
# fetching so the self-test can exercise the *check*, not a network.
check_dir() {
  local dir="$1" tag="$2"
  local wasm sha
  wasm=$(find "$dir" -maxdepth 1 -name '*.wasm' -type f | head -5)
  sha=$(find "$dir" -maxdepth 1 -name '*.wasm.sha256' -type f | head -5)

  if [ -z "$wasm" ]; then
    local listing
    listing=$(ls -A "$dir" 2>/dev/null | tr '\n' ' ')
    fail "$tag carries no .wasm asset — the artifact has no address, so every consumer has to build it themselves${listing:+ (assets present: $listing)}"
    return 1
  fi
  if [ -z "$sha" ]; then
    fail "$tag carries $(basename "$wasm") but no .wasm.sha256 beside it — a download with no recorded digest can only be trusted by rebuilding it"
    return 1
  fi
  local wasm_count sha_count
  wasm_count=$(printf '%s\n' "$wasm" | wc -l | tr -d ' ')
  sha_count=$(printf '%s\n' "$sha" | wc -l | tr -d ' ')
  if [ "$wasm_count" != "1" ] || [ "$sha_count" != "1" ]; then
    fail "$tag carries $wasm_count .wasm and $sha_count .wasm.sha256 asset(s): the page has to name one artifact, not a choice"
    return 1
  fi

  # 3. the digest file describes this module.
  ( cd "$(dirname "$sha")" && sha256sum -c "$(basename "$sha")" >/dev/null 2>&1 ) \
    || { fail "$(basename "$sha") does not describe $(basename "$wasm") — the page's digest came from another build"; return 1; }

  local published actual
  published=$(cut -d' ' -f1 "$sha")
  actual=$(sha256sum "$wasm" | cut -d' ' -f1)

  # 4. the published digest equals the expected one.
  if [ -n "$EXPECT" ]; then
    [ "$published" = "$EXPECT" ] \
      || { fail "$tag publishes $published, the expected build is $EXPECT — the release was built from a different commit than the one being compared, and a downstream copy pinned to $EXPECT will refuse it"; return 1; }
  fi

  echo "✅ $tag carries $(basename "$wasm") and its digest"
  echo "   sha256: $published"
  echo "   bytes:  $(wc -c < "$wasm" | tr -d ' ')"
  return 0
}

fetch_and_check() {
  local tag="$1"
  command -v gh >/dev/null 2>&1 || { fail "gh is not installed, so the release cannot be read — this is not a missing asset"; return 1; }

  # 1. the release exists and can be read: a failure here is a *different* fact than an empty
  # asset list, and the two remedies differ (cut the release / attach the artifact).
  local meta
  meta=$(gh release view "$tag" --json tagName,assets 2>&1) \
    || { fail "the release $tag could not be read: $meta"; return 1; }

  # The asset list is read, not assumed: `gh release view` succeeds for a release with no
  # assets, which is exactly the state `v1.14.2` was in.
  printf '%s' "$meta" | python3 -c 'import json,sys; json.load(sys.stdin)["assets"]' >/dev/null \
    || { fail "the release $tag returned no asset list: $meta"; return 1; }

  local dir
  dir=$(mktemp -d)
  # shellcheck disable=SC2064
  trap "find '$dir' -type f -delete 2>/dev/null; rmdir '$dir' 2>/dev/null || true" RETURN

  # `--pattern` rather than "download everything": the point is to assert the two files a
  # consumer needs, and a download that silently fetches nothing must fail the check below,
  # not pass it.
  gh release download "$tag" --dir "$dir" --pattern '*.wasm' --pattern '*.wasm.sha256' >/dev/null 2>&1 || true
  check_dir "$dir" "$tag"
}

seed() {
  # A temp directory holding one asset shape, so the check's three failures have a subject.
  local label="$1" want="$2"
  shift 2
  local dir
  dir=$(mktemp -d)
  "$@" "$dir"
  local out status
  set +e
  out=$(check_dir "$dir" "seed-$label" 2>&1)
  status=$?
  set -e
  find "$dir" -type f -delete
  rmdir "$dir" 2>/dev/null || true
  local first
  first=$(printf '%s\n' "$out" | head -1)
  case "$want" in
    fail)
      [ "$status" -ne 0 ] || { echo "✗ seed $label: the check passed where it must fail" >&2; return 1; }
      echo "   ✓ rejected ($label): ${first#✗ }" ;;
    pass)
      [ "$status" -eq 0 ] || { echo "✗ seed $label: the control case failed: $out" >&2; return 1; }
      echo "   ✓ accepted (control): ${first#✅ }" ;;
  esac
  return 0
}

self_test() {
  echo "── self-test: the asset check has subjects ──"
  # No release at all: nothing to read, and that is not "an empty page".
  local empty
  empty=$(mktemp -d)
  if check_dir "$empty" "seed-empty" >/dev/null 2>&1; then
    echo "✗ seed empty: an empty release passed the check" >&2; exit 1
  fi
  rmdir "$empty"
  echo "   ✓ empty release is refused"

  seed "no-digest" fail write_module_only
  seed "wrong-digest" fail write_module_and_lying_digest
  seed "good" pass write_module_and_digest

  # The expectation check is a *value*, so seed it by comparing against the right and a wrong one.
  local dir
  dir=$(mktemp -d)
  write_module_and_digest "$dir"
  local digest
  digest=$(cut -d' ' -f1 "$dir/rigorix_gate_wasi.wasm.sha256")
  EXPECT="$digest" check_dir "$dir" "seed-match" >/dev/null || { echo "✗ the right expectation was refused" >&2; exit 1; }
  if EXPECT="0000000000000000000000000000000000000000000000000000000000000000" check_dir "$dir" "seed-mismatch" >/dev/null 2>&1; then
    echo "✗ a digest that does not match the published one passed the check" >&2; exit 1
  fi
  echo "   ✓ a published digest that is not the expected build is refused"
  find "$dir" -type f -delete; rmdir "$dir"
  echo "✅ every seed was rejected and the control case passed"
}

write_module_only() {
  printf 'not a real module, but the right shape\n' > "$1/rigorix_gate_wasi.wasm"
}

write_module_and_digest() {
  local dir="$1"
  printf 'the module bytes\n' > "$dir/rigorix_gate_wasi.wasm"
  ( cd "$dir" && sha256sum rigorix_gate_wasi.wasm > rigorix_gate_wasi.wasm.sha256 )
}

write_module_and_lying_digest() {
  local dir="$1"
  write_module_and_digest "$dir"
  # A digest copied from another build: the right shape, the wrong fact.
  printf '%s  rigorix_gate_wasi.wasm\n' \
    "0000000000000000000000000000000000000000000000000000000000000000" \
    > "$dir/rigorix_gate_wasi.wasm.sha256"
}

if [ "$MODE" = "self-test" ]; then
  self_test
  exit 0
fi

if [ -n "$FROM_DIR" ]; then
  [ -d "$FROM_DIR" ] || { echo "check-release-asset: $FROM_DIR is not a directory" >&2; exit 2; }
  check_dir "$FROM_DIR" "${TAG:-$FROM_DIR}"
  exit $?
fi

[ -n "$TAG" ] || { echo "check-release-asset: a tag is required (or --from-dir)" >&2; exit 2; }
fetch_and_check "$TAG"