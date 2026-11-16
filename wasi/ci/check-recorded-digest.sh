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
#   4. an **abbreviated** digest (`b0ed0ee6…`) is refused **when the sentence says it is this
#      release's artifact**. The docs recorded exactly that, and it is unfalsifiable by
#      construction: no check can compare a prefix to a page, so a reader cannot verify it and a
#      vendor cannot pin it. A digest in a document is 64 hex characters or it is a gesture.
#      A prefix quoted about something else — a test constant, a fixture, a golden — is not a pin
#      and is skipped, with the reason printed. `v1.15.0` failed verification on exactly that
#      (#1614): two prefixes in a sentence about the canonical-bytes constant, inside the release
#      section, were read as claims about the artifact.
#
#      What is compared, and what is not: **every full digest** in the section (rules 1-3) is
#      compared to the page regardless of its subject — a 64-character number in a release section
#      that is not the page's is the #1509 defect whether or not the sentence claims it. Only the
#      abbreviation refusal is subject-scoped, because only it depends on what the number is *for*.
#
#   5. `--pre-tag` asks the same question of the `[Unreleased]` section before anything is
#      published: an abbreviated artifact digest is caught on `main`, not on a tag that cannot move
#      without lying about what built the artifacts.
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
    --pre-tag) MODE="pre-tag"; shift ;;
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

path, tag = sys.argv[1], sys.argv[2]
# The version is its own argument so `--pre-tag` can ask about `[Unreleased]`, which has no tag.
version = sys.argv[3] if len(sys.argv) > 3 and sys.argv[3] else tag.lstrip("v")
try:
    text = open(path).read()
except FileNotFoundError:
    print(f"{path}: missing")  # a doc that does not exist records nothing; named, not skipped
    sys.exit(0)

HEX = re.compile(r"\b[0-9a-f]{64}\b")
# A prefix followed by an ellipsis: the shape the docs used, and the shape that cannot be checked.
ABBREV = re.compile(r"\b[0-9a-f]{7,63}(?:…|\.\.\.)")
# The words that make a sentence a claim about *this release's artifact*. A prefix in a sentence
# without one of these is about something else — a test constant, a fixture, a golden — and is not a
# pin, so refusing it reports the wrong subject (#1614).
ARTIFACT_SUBJECT = re.compile(
    r"release|artifact|the page|download|vendor|publish|carries|ships|wasm|module", re.I
)
SENTENCE_END = re.compile(r"(?<=[.!?])\s+\n?|\n\n")


def sentence_at(text, offset):
    """The sentence containing `offset` — the unit that says what a number is for."""
    start = 0
    for m in SENTENCE_END.finditer(text):
        if m.end() <= offset:
            start = m.end()
        else:
            break
    end = len(text)
    for m in SENTENCE_END.finditer(text, start):
        if m.start() >= offset:
            end = m.start()
            break
    return text[start:end]


def is_artifact_claim(sentence, tag):
    if tag and (tag in sentence or tag.lstrip("v") in sentence):
        return True
    return bool(ARTIFACT_SUBJECT.search(sentence))


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
            subject = "SUBJECT" if is_artifact_claim(sentence_at(section, h.start()), tag) else "OTHER"
            short.append((line, h.group(0), subject))
else:
    # Every line that names the tag or the version, so a sentence about `v1.14.3` is checked and
    # a sentence about `v1.14.2` is not.
    for i, line in enumerate(text.split("\n"), 1):
        if tag in line or f"`{version}`" in line:
            for h in HEX.finditer(line):
                found.append((i, h.group(0)))
            for h in ABBREV.finditer(line):
                subject = "SUBJECT" if is_artifact_claim(line, tag) else "OTHER"
                short.append((i, h.group(0), subject))

for line, digest in found:
    print(f"{path}:{line}:{digest}")
for line, digest, subject in short:
    print(f"{path}:{line}:ABBREV:{subject}:{digest}")
PY
}

verdict() {
  local digest="$1"
  local recorded
  recorded=$(scan "$DOCS_ROOT/CHANGELOG.md" "$TAG" ""; scan "$DOCS_ROOT/docs/runbook.md" "$TAG" "")
  local abbreviated
  abbreviated=$(printf '%s\n' "$recorded" | grep -E ':ABBREV:' || true)
  recorded=$(printf '%s\n' "$recorded" | grep -E ':[0-9a-f]{64}$' || true)

  local refused skipped
  refused=$(printf '%s\n' "$abbreviated" | grep -F ':ABBREV:SUBJECT:' || true)
  skipped=$(printf '%s\n' "$abbreviated" | grep -F ':ABBREV:OTHER:' || true)
  if [ -n "$skipped" ]; then
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      echo "⏭  ${line%%:ABBREV:*} — ${line##*:ABBREV:OTHER:} is a prefix, and its sentence does not" >&2
      echo "   claim to be $TAG's artifact: not compared, and not required to be full" >&2
    done <<< "$skipped"
  fi
  if [ -n "$refused" ]; then
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      echo "✗ ${line%%:ABBREV:*} — ${line##*:ABBREV:SUBJECT:} is abbreviated, and its sentence says it" >&2
      echo "   is $TAG's artifact" >&2
    done <<< "$refused"
    echo "  an abbreviated digest cannot be compared to a page, so it cannot be verified by a" >&2
    echo "  reader or pinned by a vendor: quote the 64 characters the release carries." >&2
    echo "  (A prefix about a constant, a fixture or a golden is skipped instead: it is not a pin.)" >&2
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

# ── pre-tag: the same question, asked of [Unreleased], before a tag exists ──
# On a tag this check can only ever fail *after* publication, and a tag cannot move without lying
# about what built the artifacts (#1614). The half of the rule that needs no page — "this release's
# artifact digest may not be a prefix" — is therefore asked of `[Unreleased]` on `main` as well.
if [ "$MODE" = "pre-tag" ]; then
  out=$(scan "$DOCS_ROOT/CHANGELOG.md" "" "Unreleased" 2>/dev/null || true)
  abbreviated=$(printf '%s\n' "$out" | grep -F ':ABBREV:SUBJECT:' || true)
  skipped=$(printf '%s\n' "$out" | grep -F ':ABBREV:OTHER:' || true)
  if [ -n "$skipped" ]; then
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      echo "⏭  ${line%%:ABBREV:*} — ${line##*:ABBREV:OTHER:} is a prefix about something that is not"
      echo "   this release's artifact: not compared, and not required to be full"
    done <<< "$skipped"
  fi
  if [ -n "$abbreviated" ]; then
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      echo "✗ ${line%%:ABBREV:*} — ${line##*:ABBREV:SUBJECT:} is abbreviated, and its sentence says it"
      echo "   is this release's artifact: quote the 64 characters the page will carry"
    done <<< "$abbreviated"
    echo "  caught before publication, which is the only time this rule is cheap to fix" >&2
    exit 1
  fi
  echo "✅ [Unreleased] quotes no abbreviated artifact digest"
  exit 0
fi

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
  case_ "an abbreviated digest is refused when its sentence claims the artifact" fail \
    "- Released as \`ab77bb82…\` (1,899,431 bytes)." \
    "The release digest for v1.14.3 is \`ab77bb82…\`."
  # The #1614 shape, verbatim in miniature: two prefixes inside the release section, in a sentence
  # about a constant. The check used to refuse them and reddened a fully published tag.
  case_ "an abbreviated digest about a test constant is skipped, not refused" pass \
    "- The canonical-bytes constant moved from \`266d95c7…\` to \`57857e69…\`, and the entry keeps its claim." \
    "The release digest for v1.14.3 is \`$page\`."
  case_ "a prefix about another tag's build is skipped, not refused" pass \
    "- Released as \`$page\`." \
    "The release digest for v1.14.3 is \`$page\`.
v1.14.2's build was \`ab77bb82…\`, per its own page."
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
  # The skip names both prefixes it declined to check, and says why — a silent skip is a hole.
  printf '# Changelog\n\n## [1.14.3] - 2026-11-09\n\n- The constant moved from \`266d95c7…\` to \`57857e69…\`.\n' > "$scratch/CHANGELOG.md"
  printf '# Runbook\n\nThe release digest for v1.14.3 is \`%s\`.\n' "$page" > "$scratch/docs/runbook.md"
  out=$(bash "$0" v1.14.3 --digest "$page" --docs-root "$scratch" 2>&1 || true)
  case "$out" in
    *"not compared, and not required to be full"*) echo "  ✓ a skipped prefix says why it was skipped" ;;
    *) echo "✗ self-test: the skip was silent: $out" >&2; exit 1 ;;
  esac

  # ── the pre-tag question (#1614): asked of [Unreleased], before anything is published ──
  pre_tag_case() {  # pre_tag_case <label> <expect pass|fail> <unreleased body>
    local label="$1" expect="$2" body="$3" out status
    printf '# Changelog\n\n## [Unreleased]\n\n%s\n\n## [1.14.3] - 2026-11-09\n\n%s\n' \
      "$body" "$other" > "$scratch/CHANGELOG.md"
    printf '# Runbook\n' > "$scratch/docs/runbook.md"
    set +e
    out=$(bash "$0" --pre-tag --docs-root "$scratch" 2>&1)
    status=$?
    set -e
    case "$expect" in
      pass) [ "$status" -eq 0 ] || { echo "✗ self-test: $label failed" >&2; printf '%s\n' "$out" >&2; return 1; } ;;
      fail) [ "$status" -eq 1 ] || { echo "✗ self-test: $label did not fail (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; } ;;
    esac
    echo "  ✓ $label"
  }
  pre_tag_case "pre-tag: the unreleased section is clean" pass "- Nothing yet."
  pre_tag_case "pre-tag: an abbreviated artifact digest in [Unreleased] is caught on main" fail \
    "- This release carries \`ab77bb82…\` (1,899,431 bytes)."
  pre_tag_case "pre-tag: a prefix about a constant in [Unreleased] is skipped" pass \
    "- The constant moved from \`266d95c7…\` to \`57857e69…\`."

  echo "✅ every seed behaved as stated"
  exit 0
fi

[ -n "$TAG" ] || { echo "check-recorded-digest: a tag is required" >&2; exit 2; }
if [ -z "$DIGEST" ]; then
  DIGEST=$(page_digest) || exit 1
fi
[ -n "$DIGEST" ] || { echo "check-recorded-digest: the page digest was empty" >&2; exit 1; }
verdict "$DIGEST"