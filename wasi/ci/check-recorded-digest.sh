#!/usr/bin/env bash
# check-recorded-digest.sh — a digest in the docs is the release page's digest, quoted.
#
#   bash wasi/ci/check-recorded-digest.sh v1.14.3
#   bash wasi/ci/check-recorded-digest.sh v1.14.3 --digest <sha256>
#   bash wasi/ci/check-recorded-digest.sh v1.14.3 --docs-root <dir>
#   bash wasi/ci/check-recorded-digest.sh --self-test
#
# # Why this exists
#
# `v1.14.3` carries `ab77bb82…` (1,899,431 B). The CHANGELOG and the runbook recorded
# `b0ed0ee6…` — a number I measured from the tree *before* the version bump, so it denoted an
# artifact that never shipped, and a vendor following the runbook would have pinned a digest
# their download does not have (#1509).
#
# Three numbers for one tag is the same defect as a version number that denotes two builds. The
# release job already proves the page matches the artifact (`check-release-asset.sh`); nothing
# read the docs. This does, and it fails naming both numbers.
#
# Rules:
#
#   1. The docs record a digest for this tag — a release whose address is documented nowhere is
#      the state #1503 existed to end;
#   2. every digest recorded for this tag equals the page's. Digests recorded for **other**
#      releases are historical facts, not claims about this tag, and are ignored — the rule is
#      per line: a line that names the tag may carry only that tag's digest, and two releases'
#      digests on one line are refused as ambiguous rather than guessed at;
#   3. a mismatch names both numbers and where the stale one lives;
#   4. an **abbreviated** digest (`b0ed0ee6…`) is refused. The docs recorded exactly that, and it
#      is unfalsifiable by construction: no check can compare a prefix to a page, so a reader
#      cannot verify it and a vendor cannot pin it. A digest in a document is 64 hex characters
#      or it is a gesture.
#
# Usage notes: the page's digest is read from the release (`gh release download`), so the
# comparison is against what a download produces — not against a number in a workflow.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
TAG=""
DIGEST=""
DOCS_ROOT="$ROOT"
MODE="check"

while [ $# -gt 0 ]; do
  case "$1" in
    --digest) [ $# -ge 2 ] || { echo "check-recorded-digest: --digest needs a value" >&2; exit 2; }; DIGEST="$2"; shift 2 ;;
    --docs-root) [ $# -ge 2 ] || { echo "check-recorded-digest: --docs-root needs a directory" >&2; exit 2; }; DOCS_ROOT="$2"; shift 2 ;;
    --self-test) MODE="self-test"; shift ;;
    -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "check-recorded-digest: unknown argument '$1'" >&2; exit 2 ;;
    *) TAG="$1"; shift ;;
  esac
done

# The digests a document records for a tag: the CHANGELOG's version section, and every runbook
# line that names the tag. A digest elsewhere in either file belongs to another release.
scan() {
  python3 - "$1" "$2" "$3" <<'PY'
import re
import sys

path, tag, version = sys.argv[1], sys.argv[2], sys.argv[2].lstrip("v")
try:
    text = open(path).read()
except FileNotFoundError:
    print(f"{path}: missing")  # a doc that does not exist records nothing; named, not skipped
    sys.exit(0)

HEX = re.compile(r"\b[0-9a-f]{64}\b")
# A prefix followed by an ellipsis: the shape the docs used, and the shape that cannot be checked.
ABBREV = re.compile(r"\b[0-9a-f]{7,63}(?:…|\.\.\.)")
found = []
short = []

if path.endswith("CHANGELOG.md"):
    # Only the section for this version: [1.14.3] up to the next `## [`.
    m = re.search(rf"^## \[{re.escape(version)}\][^\n]*$", text, re.M)
    if m:
        rest = text[m.end():]
        end = re.search(r"^## \[", rest, re.M)
        section = rest[: end.start()] if end else rest
        for h in HEX.finditer(section):
            line = section.count("\n", 0, h.start()) + text.count("\n", 0, m.end()) + 2
            found.append((line, h.group(0)))
        for h in ABBREV.finditer(section):
            line = section.count("\n", 0, h.start()) + text.count("\n", 0, m.end()) + 2
            short.append((line, h.group(0)))
else:
    # Every line that names the tag or the version, so a sentence about `v1.14.3` is checked and
    # a sentence about `v1.14.2` is not.
    for i, line in enumerate(text.split("\n"), 1):
        if tag in line or f"`{version}`" in line:
            for h in HEX.finditer(line):
                found.append((i, h.group(0)))
            for h in ABBREV.finditer(line):
                short.append((i, h.group(0)))

for line, digest in found:
    print(f"{path}:{line}:{digest}")
for line, digest in short:
    print(f"{path}:{line}:ABBREV:{digest}")
PY
}

verdict() {
  local digest="$1"
  local recorded
  recorded=$(scan "$DOCS_ROOT/CHANGELOG.md" "$TAG" ""; scan "$DOCS_ROOT/docs/runbook.md" "$TAG" "")
  local abbreviated
  abbreviated=$(printf '%s\n' "$recorded" | grep -E ':ABBREV:' || true)
  recorded=$(printf '%s\n' "$recorded" | grep -E ':[0-9a-f]{64}$' || true)

  if [ -n "$abbreviated" ]; then
    while IFS= read -r line; do
      echo "✗ ${line%%:ABBREV:*}:${line##*:ABBREV:} is abbreviated" >&2
    done <<< "$abbreviated"
    echo "  an abbreviated digest cannot be compared to a page, so it cannot be verified by a" >&2
    echo "  reader or pinned by a vendor: quote the 64 characters the release carries" >&2
    return 1
  fi

  if [ -z "$recorded" ]; then
    echo "✗ no document records a digest for $TAG — the release has an address and the docs do" >&2
    echo "  not say what it is; a number nothing states cannot be checked or pinned" >&2
    return 1
  fi

  local stale=0
  while IFS= read -r line; do
    local where theirs
    where="${line%:*}"
    theirs="${line##*:}"
    if [ "$theirs" != "$digest" ]; then
      echo "✗ $where records $theirs, and $TAG carries $digest" >&2
      stale=$((stale + 1))
    fi
  done <<< "$recorded"

  if [ "$stale" -gt 0 ]; then
    echo "  a digest in the docs is the release page's digest, quoted — a number whose provenance" >&2
    echo "  cannot be restated (another tree, another platform, another release) is a wish" >&2
    return 1
  fi

  local count
  count=$(printf '%s\n' "$recorded" | wc -l | tr -d ' ')
  echo "✅ the docs record $TAG's digest ($digest) at $count place(s), and it is the page's"
  return 0
}

page_digest() {
  command -v gh >/dev/null 2>&1 || { echo "✗ gh is not installed, so the page cannot be read" >&2; return 1; }
  local dir
  dir=$(mktemp -d)
  # shellcheck disable=SC2064
  trap "find '$dir' -type f -delete 2>/dev/null; rmdir '$dir' 2>/dev/null || true" RETURN
  gh release download "$TAG" --dir "$dir" --pattern '*.wasm.sha256' >/dev/null 2>&1 || {
    echo "✗ $TAG carries no .wasm.sha256 asset, so there is no page digest to compare" >&2
    return 1
  }
  local f
  f=$(find "$dir" -name '*.wasm.sha256' -type f | head -1)
  cut -d' ' -f1 "$f"
}

# ── self-test ──────────────────────────────────────────────────────────────
if [ "$MODE" = "self-test" ]; then
  scratch="$(mktemp -d)"
  trap 'find "$scratch" -type f -delete 2>/dev/null; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT
  mkdir -p "$scratch/docs"
  page="ab77bb821bcff564dd2302a43cf3a73930cfd5ec94925b78328770a8023b7928"
  other="b0ed0ee658615436af1c2e576d147759c5b473e60cc45067c039cdd347347383"

  case_() {  # case_ <label> <expect pass|fail> <changelog-body> <runbook-body>
    local label="$1" expect="$2" cl="$3" rb="$4" out status
    printf '# Changelog\n\n## [1.14.3] - 2026-11-09\n\n%s\n\n## [1.14.2] - 2026-11-07\n\n%s\n' \
      "$cl" "$other" > "$scratch/CHANGELOG.md"
    printf '# Runbook\n\n%s\n' "$rb" > "$scratch/docs/runbook.md"
    set +e
    out=$(TAG=v1.14.3 DIGEST="$page" bash "$0" v1.14.3 --digest "$page" --docs-root "$scratch" 2>&1)
    status=$?
    set -e
    case "$expect" in
      pass) [ "$status" -eq 0 ] || { echo "✗ self-test: $label failed" >&2; printf '%s\n' "$out" >&2; return 1; } ;;
      fail) [ "$status" -eq 1 ] || { echo "✗ self-test: $label did not fail (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; } ;;
    esac
    echo "  ✓ $label"
  }

  echo "── self-test: the recorded-digest check has subjects ──"
  case_ "control: the docs quote the page's digest" pass \
    "- Released as \`$page\` (1,899,431 bytes, ubuntu-latest)." \
    "The release digest for v1.14.3 is \`$page\`."
  case_ "the changelog records a digest from another tree" fail \
    "- \`$other\` was my pre-bump measurement." \
    "The release digest for v1.14.3 is \`$page\`."
  case_ "the runbook records a digest the page does not carry" fail \
    "- Released as \`$page\`." \
    "The release digest for v1.14.3 is \`$other\`."
  case_ "a digest for another release is history, not a claim" pass \
    "- Released as \`$page\`." \
    "The release digest for v1.14.3 is \`$page\`.
v1.14.2's build recorded \`$other\`."
  case_ "two releases' digests on the tag's own line is ambiguous" fail \
    "- Released as \`$page\`." \
    "v1.14.3 is \`$page\`, where v1.14.2 was \`$other\`."
  case_ "an abbreviated digest is refused" fail \
    "- Released as \`ab77bb82…\` (1,899,431 bytes)." \
    "The release digest for v1.14.3 is \`ab77bb82…\`."
  case_ "no document records the tag's digest" fail \
    "- Released, digest pending." \
    "See the release page."

  # The mismatch must name both numbers.
  printf '# Changelog\n\n## [1.14.3] - 2026-11-09\n\n- \`%s\`\n' "$other" > "$scratch/CHANGELOG.md"
  printf '# Runbook\n' > "$scratch/docs/runbook.md"
  out=$(bash "$0" v1.14.3 --digest "$page" --docs-root "$scratch" 2>&1 || true)
  case "$out" in
    *"$other"*"$page"*|*"$page"*"$other"*)
      echo "  ✓ a mismatch names both numbers" ;;
    *) echo "✗ self-test: a mismatch did not name both numbers: $out" >&2; exit 1 ;;
  esac
  echo "✅ every seed behaved as stated"
  exit 0
fi

[ -n "$TAG" ] || { echo "check-recorded-digest: a tag is required" >&2; exit 2; }
if [ -z "$DIGEST" ]; then
  DIGEST=$(page_digest) || exit 1
fi
[ -n "$DIGEST" ] || { echo "check-recorded-digest: the page digest was empty" >&2; exit 1; }
verdict "$DIGEST"