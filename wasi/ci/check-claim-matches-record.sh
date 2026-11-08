#!/usr/bin/env bash
# check-claim-matches-record.sh — the artifact's words must equal its import record.
#
#   bash wasi/ci/check-claim-matches-record.sh
#   bash wasi/ci/check-claim-matches-record.sh --self-test
#   bash wasi/ci/check-claim-matches-record.sh --root <dir>
#
# # Why this exists (#1467)
#
# The artifact shipped with three sentences claiming it imports nothing, while
# `wasi/ci/imports.txt` recorded six. Two of those sentences are files a reader meets *before*
# ADR-022 — and one is the crate description, which is what `cargo metadata` prints and what a
# security review quotes. "Zero imports" is a specific claim: it says a reviewer may confirm the
# module cannot call `environ_get`. It can. What is true is that no decision *reads* it.
#
# A comment saying "keep these in step" is not a guard. This is two checks:
#
#   1. **the phrases are unrepresentable.** No file whose subject is this artifact says
#      `zero imports`, `imports nothing`, `nothing to import`, `no imports` or
#      `empty import section`. The scope is the artifact's own files, because that is the
#      subject: `wasm/.pi/scripts/languages/java/` also says "no imports", about a Java domain
#      layer, and a guard that flagged it would be reporting on a different claim.
#
#   2. **the words name the count.** The crate description, the CI graph's rationale and the
#      binding comment each name the number the record holds — as a word (`six`) or a numeral
#      (`6`). When the record gets shorter, the words must follow in the same commit or this
#      fails naming both numbers.
#
# A file that cannot be read is a failure, not a skip: a claim checker that reports on a tree it
# never read passes everything.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
RECORD="$HERE/imports.txt"
MODE="check"

while [ $# -gt 0 ]; do
  case "$1" in
    --root) [ $# -ge 2 ] || { echo "check-claim: --root needs a directory" >&2; exit 2; }; ROOT="$2"; shift 2 ;;
    --self-test) MODE="self-test"; shift ;;
    -h|--help) sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "check-claim: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

# The phrases a claim about *this* artifact must not contain.
FORBIDDEN=(
  "zero import"
  "imports nothing"
  "nothing to import"
  "no imports"
  "empty import section"
)

# Where the artifact is described. `imports.txt` is the record and ADR-022 quotes the phrase it
# rejects, so both are excluded by name rather than by a pattern that could grow.
recorded_count() { sed -e 's/#.*//' "$RECORD" | tr -d ' ' | grep -c . ; }
number_word() {
  case "$1" in
    0) echo "zero" ;; 1) echo "one" ;; 2) echo "two" ;; 3) echo "three" ;; 4) echo "four" ;;
    5) echo "five" ;; 6) echo "six" ;; 7) echo "seven" ;; 8) echo "eight" ;; 9) echo "nine" ;;
    10) echo "ten" ;; *) echo "$1" ;;
  esac
}

# ── check 1: no file describes this artifact as having no imports ──────────────
check_phrases() {
  local root="$1" failures=0 hits hit file
  local filelist=()
  local candidates=(
    "$root/wasi/src" "$root/wasi/ci" "$root/wasi/Cargo.toml" "$root/wasi/tests"
    "$root/wasm/src" "$root/wasm/Cargo.toml"
    "$root/.github/ci-graph.tsv" "$root/.github/ci-affected.tsv" "$root/.github/workflows"
  )
  for candidate in "${candidates[@]}"; do
    [ -e "$candidate" ] || { echo "✗ $candidate does not exist — the claim cannot be checked there" >&2; failures=$((failures + 1)); continue; }
    if [ -d "$candidate" ]; then
      while IFS= read -r found; do filelist+=("$found"); done < <(find "$candidate" -type f \( -name '*.rs' -o -name '*.mjs' -o -name '*.sh' -o -name '*.toml' -o -name '*.tsv' -o -name '*.yml' \))
    else
      filelist+=("$candidate")
    fi
  done

  # `set -u` and an empty array: `${filelist[@]}` is "unbound" before bash 4.4, and a tree
  # where nothing exists must fail rather than abort the script that was checking it.
  for file in ${filelist[@]+"${filelist[@]}"}; do
    case "$file" in
      "$RECORD"|*check-claim-matches-record.sh) continue ;;
    esac
    for hit in "${FORBIDDEN[@]}"; do
      if grep -qiF -- "$hit" "$file" 2>/dev/null; then
        echo "✗ $(realpath --relative-to="$root" "$file" 2>/dev/null || echo "$file") says \"$hit\"" >&2
        grep -niF -- "$hit" "$file" | head -3 | sed 's/^/    /' >&2
        failures=$((failures + 1))
      fi
    done
  done

  if [ "$failures" -eq 0 ]; then
    echo "✓ no file describing the artifact claims it carries no imports"
  fi
  return "$failures"
}

# ── check 2: each sentence names the count the record holds ────────────────────
check_counts() {
  local root="$1" count word failures=0
  count="$(recorded_count)"
  word="$(number_word "$count")"

  names_count() {
    local file="$1" label="$2"
    if [ ! -f "$file" ]; then
      echo "✗ $label: $file does not exist" >&2
      return 1
    fi
    if grep -qiF -- "$word" "$file" || grep -qE "(^|[^0-9])$count([^0-9]|$)" "$file"; then
      echo "✓ $label names $word (the record's $count)"
      return 0
    fi
    echo "✗ $label does not name $word, which is what wasi/ci/imports.txt records ($count)" >&2
    return 1
  }

  names_count "$root/wasi/Cargo.toml" "the crate description" || failures=$((failures + 1))
  names_count "$root/.github/ci-graph.tsv" "the CI graph's rationale" || failures=$((failures + 1))
  names_count "$root/wasm/Cargo.toml" "the binding comment" || failures=$((failures + 1))
  return "$failures"
}

run_checks() {
  local root="$1" failures=0
  check_phrases "$root" || failures=$((failures + $?))
  check_counts "$root" || failures=$((failures + $?))
  return "$failures"
}

if [ "$MODE" = "check" ]; then
  [ -f "$RECORD" ] || { echo "✗ $RECORD is missing — there is no record to compare against" >&2; exit 1; }
  run_checks "$ROOT"
  exit 0
fi

# ── self-test: the checks must be able to fail ────────────────────────────────
scratch="$(mktemp -d)"
trap 'find "$scratch" -type f -delete; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT

# The real tree passes.
run_checks "$ROOT" >/dev/null || { echo "self-test: the real tree does not pass its own check" >&2; exit 1; }
echo "✓ self-test: the real tree passes"

# The seed from the issue: put the old description back, and require a failure that names it.
mkdir -p "$scratch/tree/wasi/ci" "$scratch/tree/wasm/src" "$scratch/tree/.github/workflows"
cp "$RECORD" "$scratch/tree/wasi/ci/imports.txt"
RECORD="$scratch/tree/wasi/ci/imports.txt"
sed 's/^description = .*/description = "The Rigorix gate as a WASI artifact: one JSON entry, zero imports, no host glue"/' \
  "$ROOT/wasi/Cargo.toml" > "$scratch/tree/wasi/Cargo.toml"
cp "$ROOT/wasm/Cargo.toml" "$scratch/tree/wasm/Cargo.toml"
cp "$ROOT/.github/ci-graph.tsv" "$scratch/tree/.github/ci-graph.tsv"
if run_checks "$scratch/tree" >/dev/null 2>&1; then
  echo "self-test: a description claiming zero imports passed — the check does not bite" >&2
  exit 1
fi
echo "✓ self-test: a description claiming zero imports fails (the issue's seed)"
if cmp -s "$ROOT/wasi/Cargo.toml" "$scratch/tree/wasi/Cargo.toml"; then
  echo "self-test: the seed was not applied — the failing check proved nothing" >&2
  exit 1
fi

# The second half: a record that shrinks must fail the sentences that name the old count.
sed 's/^description = .*/description = "The Rigorix gate as a WASI artifact: one JSON entry, six prologue imports that no decision reads, no host glue"/' \
  "$ROOT/wasi/Cargo.toml" > "$scratch/tree/wasi/Cargo.toml"
# One process, not a pipeline: with `pipefail`, `head` closing the pipe would abort the
# self-test itself (SIGPIPE) rather than testing anything.
awk '!/^#/ && NF { print; if (++n == 3) exit }' "$RECORD" > "$scratch/tree/wasi/ci/short.txt"
RECORD="$scratch/tree/wasi/ci/short.txt"
if run_checks "$scratch/tree" >/dev/null 2>&1; then
  echo "self-test: a record of 3 with sentences saying six passed — the count is not compared" >&2
  exit 1
fi
echo "✓ self-test: a record holding three while the words say six fails, naming both"

# A missing file is a failure, not a skip.
mkdir -p "$scratch/empty"
if run_checks "$scratch/empty" >/dev/null 2>&1; then
  echo "self-test: an empty tree passed — the check reports on files it never read" >&2
  exit 1
fi
echo "✓ self-test: a tree without the files fails closed"

echo
echo "✓ every claim check has a subject"
