#!/usr/bin/env bash
# validate-spec-vocabulary.sh — the specification's closed vocabularies equal their owners in code.
#
#   bash engine/.pi/scripts/validate-spec-vocabulary.sh
#   bash engine/.pi/scripts/validate-spec-vocabulary.sh --report
#   bash engine/.pi/scripts/validate-spec-vocabulary.sh --self-test
#
# # Why this exists
#
# `docs/spec/execution-governance/03-requirements.md` enumerates two closed vocabularies
# normatively: the gate identifiers (§L, the table's "Identifier in the record" column) and the
# refusal wire words (RS-111's table). Their owners in the tree are exhaustive: `GateId::as_str`
# and `RefusalKind::as_str`. Both directions are already enforced *inside* the code — a variant
# without an `as_str` arm is a compile error, and serde's spelling is asserted equal to the
# accessor. Nothing enforced the third direction: **the document and the code agreeing**.
#
# That asymmetry has produced defects here before: a sixteenth kind compiles, serializes, and is
# accepted by every verifier while a reader of the specification sees fifteen. RS-108 says a kind
# MUST NOT be added without a specification version that names it; a requirement with no check is
# the aspiration `11-traceability.md` warns about (#1512).
#
# # Scope, stated because silence is not a scope
#
# RS-108 names three vocabularies; the current document enumerates **two**. The eight
# `EvidenceKind` variants are not listed anywhere under `docs/spec/` after the v2 document was
# restructured into the requirement sections, so there is nothing to compare them to — checked and
# reported by `--report`, and tracked in its own issue rather than invented here.
#
# # Exit codes
#
#   0  both vocabularies agree
#   1  a divergence — the document and the code disagree
#   2  a table or an owner could not be parsed — malformed, remedy: fix the input
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
SPEC="$ROOT/docs/spec/execution-governance/03-requirements.md"
GATES="$ROOT/engine/src/gate/domain/finding.rs"
REFUSALS="$ROOT/wasm/src/domain/outcome/refusal.rs"
MODE="check"

while [ $# -gt 0 ]; do
  case "$1" in
    --report) MODE="report"; shift ;;
    --self-test) MODE="self-test"; shift ;;
    --spec) [ $# -ge 2 ] || { echo "validate-spec-vocabulary: --spec needs a file" >&2; exit 2; }; SPEC="$2"; shift 2 ;;
    --gates) [ $# -ge 2 ] || { echo "validate-spec-vocabulary: --gates needs a file" >&2; exit 2; }; GATES="$2"; shift 2 ;;
    --refusals) [ $# -ge 2 ] || { echo "validate-spec-vocabulary: --refusals needs a file" >&2; exit 2; }; REFUSALS="$2"; shift 2 ;;
    -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "validate-spec-vocabulary: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

if [ "$MODE" = "self-test" ]; then
  scratch="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "find '$scratch' -type f -delete 2>/dev/null; find '$scratch' -depth -type d -exec rmdir {} + 2>/dev/null || true" EXIT

  cp "$SPEC" "$scratch/spec.md"
  cp "$GATES" "$scratch/gates.rs"
  cp "$REFUSALS" "$scratch/refusals.rs"
  run() { bash "$0" --spec "$scratch/spec.md" --gates "$scratch/gates.rs" --refusals "$scratch/refusals.rs"; }

  ACCEPTED=0
  REFUSED=0
  MALFORMED=0
  case_() {  # case_ <label> <expect pass|fail|malformed>
    local label="$1" expect="$2" out status
    set +e
    out=$(run 2>&1)
    status=$?
    set -e
    case "$expect" in
      pass) [ "$status" -eq 0 ] || { echo "✗ self-test: $label failed" >&2; printf '%s\n' "$out" >&2; return 1; }; ACCEPTED=$((ACCEPTED + 1)) ;;
      fail) [ "$status" -eq 1 ] || { echo "✗ self-test: $label did not fail as a divergence (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; }; REFUSED=$((REFUSED + 1)) ;;
      malformed) [ "$status" -eq 2 ] || { echo "✗ self-test: $label was not refused as malformed (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; }; MALFORMED=$((MALFORMED + 1)) ;;
    esac
    echo "  ✓ $label"
  }

  echo "── self-test: the spec and the code must agree, in both directions ──"
  case_ "control: the committed documents agree" pass

  # 1. a word the spec lists and the code does not have
  cp "$SPEC" "$scratch/spec.md"
  python3 - "$scratch/spec.md" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
s = s.replace("| the cap does not admit this consumption | `capacity_exceeded` |",
              "| the cap does not admit this consumption | `capacity_exceeded` · `capacity_reserved` |", 1)
open(p, "w").write(s)
PY
  case_ "a wire word the spec lists and the code lacks fails" fail

  # 2. a word the code has and the spec does not list
  cp "$SPEC" "$scratch/spec.md"
  python3 - "$scratch/refusals.rs" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
s = s.replace('            Self::DuplicateEffect => "duplicate_effect",',
              '            Self::DuplicateEffect => "duplicate_effect",\n            Self::DuplicationDetected => "duplication_detected",', 1)
open(p, "w").write(s)
PY
  case_ "a wire word the code has and the spec lacks fails" fail

  # 3. one word spelled differently in one place
  cp "$REFUSALS" "$scratch/refusals.rs"
  python3 - "$scratch/spec.md" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
s = s.replace("`session_sealed`", "`session_closed`", 1)
open(p, "w").write(s)
PY
  case_ "a word spelled differently in one place fails" fail

  # 4. the gate table with a row removed: the code names a gate the document does not
  cp "$SPEC" "$scratch/spec.md"
  python3 - "$scratch/spec.md" <<'PY'
import sys
p = sys.argv[1]
lines = open(p).read().split("\n")
lines = [l for l in lines if "`capacity`" not in l or "Identifier" in l]
open(p, "w").write("\n".join(lines))
PY
  case_ "a gate identifier the document does not list fails" fail

  # 5. a spec with no gate table at all: unparseable is not a pass
  printf '# nothing here\n' > "$scratch/spec.md"
  case_ "a document with no table is malformed, not a pass" malformed

  cp "$SPEC" "$scratch/spec.md"
  printf '✅ %s acceptance case(s) passed, %s divergence(s) refused, %s malformed-input case(s) refused\n' \
    "$ACCEPTED" "$REFUSED" "$MALFORMED"
  exit 0
fi

exec python3 - "$SPEC" "$GATES" "$REFUSALS" "$MODE" <<'PY'
import os
import re
import sys

SPEC, GATES_SRC, REFUSALS_SRC, MODE = sys.argv[1:5]
GREEN, RED, DIM, RESET = "\033[0;32m", "\033[0;31m", "\033[2m", "\033[0m"


def malformed(msg):
    print(f"{RED}❌ the input cannot be read as a vocabulary{RESET}", file=sys.stderr)
    print(f"   {msg}", file=sys.stderr)
    print(f"{DIM}   remedy: fix the input — a malformed table and a divergence have different "
          f"fixes{RESET}", file=sys.stderr)
    sys.exit(2)


def read(path):
    try:
        return open(path, encoding="utf-8").read()
    except OSError as exc:
        malformed(f"{path}: {exc.strerror}")


spec = read(SPEC)
lines = spec.split("\n")

# ── the gate identifiers: §L's table, "Identifier in the record" column ────────────────────────
gate_words = []
in_table = False
for line in lines:
    if line.startswith("| Gate | Identifier in the record |"):
        in_table = True
        continue
    if in_table:
        if not line.startswith("|"):
            break
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 2 or set(cells[0]) <= set("-: "):
            continue  # the separator row
        ident = cells[1]
        if ident.startswith("—") or ident.startswith("-"):
            continue  # the document says this gate carries no identifier; that is a statement
        for word in re.findall(r"`([a-z_]+)`", ident):
            gate_words.append(word)
if not gate_words:
    malformed(f"{SPEC}: no `| Gate | Identifier in the record |` table — the §L vocabulary is the "
              f"check's subject and it was not found")

# ── the refusal wire words: RS-111's table ────────────────────────────────────────────────────
refusal_words = []
in_table = False
for line in lines:
    if line.startswith("| Kind | Wire word |"):
        in_table = True
        continue
    if in_table:
        if not line.startswith("|"):
            break
        for word in re.findall(r"`([a-z_]+)`", line):
            refusal_words.append(word)
if not refusal_words:
    malformed(f"{SPEC}: no `| Kind | Wire word |` table — RS-111's vocabulary was not found")

# ── the owners: the exhaustive `as_str` arms ──────────────────────────────────────────────────
def as_str_words(path, what):
    src = read(path)
    # Only the `as_str` body: a doc comment elsewhere can contain a quoted word, and the point is
    # to read what the code *decides*, not what a comment mentions.
    m = re.search(r"fn as_str\((?:&self|self)\)[^{]*\{(.*?)\n    \}", src, re.S)
    if not m:
        malformed(f"{path}: no `fn as_str(self)` / `fn as_str(&self)` body — {what}'s owner was "
                  f"not found")
    words = re.findall(r'"[a-z_]+"', m.group(1))
    return [w.strip('"') for w in words]


gate_code = as_str_words(GATES_SRC, "GateId")
refusal_code = as_str_words(REFUSALS_SRC, "RefusalKind")

findings = []


def compare(label, spec_words, code_words, spec_ref, code_ref, hint):
    dupes = {w for w in set(spec_words) if spec_words.count(w) > 1}
    if dupes:
        findings.append(f"{spec_ref}: {', '.join(sorted(dupes))} appears more than once in the "
                        f"{label} table — a vocabulary with two spellings is not closed")
    for word in sorted(set(spec_words) - set(code_words)):
        findings.append(f"{spec_ref} lists `{word}`; {code_ref} has no such word — the document "
                        f"names a {label} the code cannot produce")
    for word in sorted(set(code_words) - set(spec_words)):
        findings.append(f"{code_ref} produces `{word}`, and {spec_ref} does not list it — {hint}")


compare("gate identifier", gate_words, gate_code,
        "§L (the gate table)", "GateId::as_str",
        "a gate identifier is added by adding a row to that table (§L, RS-114)")
compare("refusal kind", refusal_words, refusal_code,
        "RS-111", "RefusalKind::as_str",
        "a new kind needs a specification version that names it (RS-108), not a table row")

if MODE == "report":
    print(f"spec: {os.path.relpath(SPEC)}")
    print(f"  gate identifiers    {len(gate_words)} in the document, "
          f"{len(gate_code)} in GateId::as_str")
    print(f"  refusal wire words  {len(refusal_words)} in the document, "
          f"{len(refusal_code)} in RefusalKind::as_str")
    print("  evidence kinds      not compared: the document does not enumerate them after the "
          "v2 restructure (RS-108 names the vocabulary; §L does not list it)")
    for f in findings:
        print(f"  {f}")
    sys.exit(1 if findings else 0)

if findings:
    print(f"{RED}❌ {len(findings)} divergence(s) between the specification and the code{RESET}",
          file=sys.stderr)
    for f in findings:
        print(f"   {f}", file=sys.stderr)
    print(f"{DIM}   the specification is the record a reader checks the wire words against: change "
          f"the document and the code together, or neither{RESET}", file=sys.stderr)
    sys.exit(1)

print(f"{GREEN}✅ spec vocabulary: {len(gate_words)} gate identifier(s) and {len(refusal_words)} "
      f"refusal wire word(s) equal their owners exactly{RESET}")
sys.exit(0)
PY