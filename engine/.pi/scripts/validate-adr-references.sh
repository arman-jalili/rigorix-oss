#!/usr/bin/env bash
# ============================================================================
# validate-adr-references.sh — an ADR's citations point at the document they name
# ============================================================================
# ADR-2 (#1505). ADR-022's Related block labelled four ADRs as four other subjects —
# "ADR-012 (failure semantics)" pointed at Identity Attestation, whose failure semantics
# live in ADR-009 — and one body citation named a section ("ADR-012 tier 3") that exists in
# no document. Both are the same defect in the register that exists to prevent it: a claim
# whose source cannot be checked.
#
# A reader follows a citation to learn something. This guard is what makes the follow
# land: the label has to describe the document it is attached to.
#
# Rules:
#
#   1. The target exists. `ADR-nnn` must be a file in decisions/.
#   2. The label matches the target's **subject**. A subject is read from three places,
#      because the tree has three shapes: the `# ` title, a `## Title` section (ADR-014,
#      ADR-015, ADR-016 are template stubs whose real subject is there), and the filename
#      slug. A label matches when one of its words appears in the subject.
#   3. A label that names the target's *interaction* rather than its title is allowed —
#      and has to be **recorded** in `.pi/architecture/adr-citation-labels.tsv` with the
#      reason. A descriptive label is a judgement; a judgement with no record is the
#      silence this rule removes.
#   4. A section citation resolves: `ADR-nnn §N` must have a `## N.`/`### N.` heading in
#      that ADR. Bare `§N` is checked against the document it appears in.
#   5. A filename citation resolves: `ADR-nnn-<slug>.md` inside the register must be a file
#      in it. `wasm/.pi/architecture/decisions/` is the Guardian framework's own ADR set —
#      different documents with the same numbers — so a bare number means one thing inside
#      this directory and another elsewhere, which is why rule 5 is scoped to this directory.
#
# Usage:
#   bash engine/.pi/scripts/validate-adr-references.sh             # check
#   bash engine/.pi/scripts/validate-adr-references.sh --report    # citations + labels
#   bash engine/.pi/scripts/validate-adr-references.sh --self-test # prove the rules bite
#   bash engine/.pi/scripts/validate-adr-references.sh --adr-dir <dir> --labels <file>
#
# Exit codes: 0 = every citation resolves; 1 = a violation; 2 = malformed input
# ============================================================================
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TREE_ROOT="$(cd "${SELF_DIR}/../.." && pwd)"          # <repo>/engine
ADR_DIR="$TREE_ROOT/.pi/architecture/decisions"
LABELS="$TREE_ROOT/.pi/architecture/adr-citation-labels.tsv"

MODE="check"
ADR_DIR_OVERRIDE=""
LABELS_OVERRIDE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --report)    MODE="report"; shift ;;
    --self-test) MODE="self-test"; shift ;;
    --adr-dir)   [ $# -ge 2 ] || { echo "validate-adr-references: --adr-dir needs a directory" >&2; exit 2; }
                 ADR_DIR_OVERRIDE="$2"; shift 2 ;;
    --labels)    [ $# -ge 2 ] || { echo "validate-adr-references: --labels needs a file" >&2; exit 2; }
                 LABELS_OVERRIDE="$2"; shift 2 ;;
    -h|--help)   sed -n '2,44p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)           echo "validate-adr-references: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

[ -n "$ADR_DIR_OVERRIDE" ] && ADR_DIR="$ADR_DIR_OVERRIDE"
[ -n "$LABELS_OVERRIDE" ] && LABELS="$LABELS_OVERRIDE"

[ -d "$ADR_DIR" ] || { echo "validate-adr-references: $ADR_DIR does not exist" >&2; exit 2; }

# ── self-test: each rule has a seed that it rejects ────────────────────────
if [ "$MODE" = "self-test" ]; then
  scratch="$(mktemp -d)"
  trap 'find "$scratch" -type f -delete 2>/dev/null; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT

  # Two tiny ADRs: the second is the cited one, with a title and a §3 heading.
  mkdir -p "$scratch/decisions"
  printf '# ADR-101: A ledger of things\n\n**Related:** ADR-102 (thing store)\n\n## 1. Context\n' \
    > "$scratch/decisions/ADR-101-a-ledger.md"
  printf '# ADR-102: The thing store\n\n## 3. Shape\n\nA trap is not a refusal.\n' \
    > "$scratch/decisions/ADR-102-thing-store.md"
  : > "$scratch/labels.tsv"

  run_case() {  # run_case <label> <expect pass|fail> ...
    local label="$1" expect="$2"; shift 2
    local out status
    set +e
    out=$("$@" 2>&1); status=$?
    set -e
    case "$expect" in
      pass)
        [ "$status" -eq 0 ] || { echo "✗ self-test: $label passed when it must fail" >&2; printf '%s\n' "$out" >&2; return 1; } ;;
      fail)
        [ "$status" -eq 1 ] || { echo "✗ self-test: $label did not fail as a violation (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; } ;;
      malformed)
        # Exit 2 is the "I cannot read this" door: a malformed record must not read as a
        # pass, and it must not read as a violation either — the two have different remedies.
        [ "$status" -eq 2 ] || { echo "✗ self-test: $label was not refused as malformed (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; } ;;
    esac
    echo "  ✓ $label"
  }

  check_dir() { bash "$0" --adr-dir "$scratch/decisions" --labels "$scratch/labels.tsv"; }

  echo "── self-test: the citation rules have subjects ──"

  # Control: a label that matches the target's title passes.
  run_case "control: a matching label passes" pass check_dir

  # Rule 2: a label that names nothing in the target.
  cp "$scratch/decisions/ADR-101-a-ledger.md" "$scratch/keep"
  printf '# ADR-101: A ledger of things\n\n**Related:** ADR-102 (portable slice)\n' \
    > "$scratch/decisions/ADR-101-a-ledger.md"
  run_case "a label that names nothing in the target is refused" fail check_dir

  # Rule 3: the same label, recorded with a reason, passes.
  printf 'ADR-102\tportable slice\tthe slice is where this store is used, which ADR-102 does not title\n' \
    > "$scratch/labels.tsv"
  run_case "a recorded descriptive label passes" pass check_dir

  # Rule 3 again: a recorded row with no reason is malformed, not an allow.
  printf 'ADR-102\tportable slice\t\n' > "$scratch/labels.tsv"
  run_case "a recorded label with no reason is refused" malformed check_dir
  : > "$scratch/labels.tsv"

  # Rule 1: a citation to a number with no file.
  printf '# ADR-101: A ledger of things\n\n**Related:** ADR-999 (anything)\n' \
    > "$scratch/decisions/ADR-101-a-ledger.md"
  run_case "a citation to a missing ADR is refused" fail check_dir
  cp "$scratch/keep" "$scratch/decisions/ADR-101-a-ledger.md"

  # Rule 4: a section citation that resolves, and one that does not.
  printf '# ADR-101: A ledger of things\n\nSee ADR-102 §3.\n' > "$scratch/decisions/ADR-101-a-ledger.md"
  run_case "a § section that exists passes" pass check_dir
  printf '# ADR-101: A ledger of things\n\nSee ADR-102 §9.\n' > "$scratch/decisions/ADR-101-a-ledger.md"
  run_case "a § section that exists nowhere is refused" fail check_dir
  cp "$scratch/keep" "$scratch/decisions/ADR-101-a-ledger.md"

  # Rule 5: a filename citation that does not exist.
  printf '# ADR-101: A ledger of things\n\nSee ADR-102-thing-store.md.\n' > "$scratch/decisions/ADR-101-a-ledger.md"
  run_case "a cited filename that exists passes" pass check_dir
  printf '# ADR-101: A ledger of things\n\nSee ADR-102-missing-doc.md.\n' > "$scratch/decisions/ADR-101-a-ledger.md"
  run_case "a cited filename that does not exist is refused" fail check_dir
  cp "$scratch/keep" "$scratch/decisions/ADR-101-a-ledger.md"

  # An empty directory is never a pass: a check with no subject.
  mkdir -p "$scratch/empty"
  run_case "an ADR directory with no ADRs is refused" malformed \
    bash "$0" --adr-dir "$scratch/empty" --labels "$scratch/labels.tsv"

  echo "✅ every seed was rejected and the control case passed"
  exit 0
fi

REPO_ROOT="$(cd "$TREE_ROOT/.." && pwd)"
exec python3 - "$ADR_DIR" "$LABELS" "$MODE" "$REPO_ROOT" "$TREE_ROOT" <<'PY'
import glob
import os
import re
import sys

adr_dir, labels_path, mode, repo_root, tree_root = sys.argv[1:6]
RED, GREEN, DIM, RESET = "\033[0;31m", "\033[0;32m", "\033[2m", "\033[0m"


def die(msg):
    print(f"{RED}❌ {msg}{RESET}", file=sys.stderr)
    sys.exit(2)


files = {}
for path in sorted(glob.glob(os.path.join(adr_dir, "ADR-*.md"))):
    m = re.match(r"ADR-(\d{3})", os.path.basename(path))
    if not m:
        continue  # ADR-template.md and anything not numbered
    files[m.group(1)] = path
if not files:
    die(f"{adr_dir} holds no ADR-nnn*.md file — a check with no subject passes for the wrong reason")

STOP = {"the", "and", "with", "for", "not", "that", "this", "into", "from", "one", "its", "are",
        "record", "decision", "architecture", "adr"}


def words(text):
    return {w for w in re.findall(r"[a-z][a-z-]{3,}", text.lower()) if w not in STOP}


def subject(num):
    """The target's subject: title, `## Title` section, and the filename slug (#1505)."""
    path = files.get(num)
    if not path:
        return None
    text = open(path).read()
    parts = []
    first = text.split("\n", 1)[0].lstrip("# ").strip()
    parts.append(first)
    m = re.search(r"^##\s+Title\s*\n+(.+)$", text, re.M)
    if m:
        parts.append(m.group(1).strip())
    slug = os.path.basename(path)[len("ADR-000-"):-len(".md")]
    parts.append(slug.replace("-", " "))
    return " / ".join(parts), words(" ".join(parts))


# ── recorded descriptive labels ────────────────────────────────────────────
recorded = {}
if os.path.exists(labels_path):
    with open(labels_path) as fh:
        for lineno, line in enumerate(fh, 1):
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            cols = line.rstrip("\n").split("\t")
            if len(cols) < 3:
                die(f"{labels_path}:{lineno}: expected 3 tab-separated columns "
                    f"(ADR-nnn / label / reason), got {len(cols)}")
            num, label, reason = cols[0].strip(), " ".join(cols[1].split()), cols[2].strip()
            if not reason:
                die(f"{labels_path}:{lineno}: {num} ({label}) is recorded with no reason — "
                    "a descriptive label is a judgement, and the record is the review")
            if not re.fullmatch(r"ADR-\d{3}", num):
                die(f"{labels_path}:{lineno}: {num!r} is not an ADR-nnn identifier")
            recorded[(num, label.lower())] = reason




violations = []
citations = []
descriptive = []

for num, path in sorted(files.items()):
    name = os.path.basename(path)
    body = open(path).read()
    # Citations are searched over the whole text with `\s+`, not line by line: a label that
    # wraps ("ADR-014 (portable slice and\npinned inputs)") is still a label, and a scanner
    # that skipped it would be the vacuous check this guard exists to replace (#1505).
    for m in re.finditer(r"ADR-(\d{3})\s*\(([^)]{2,70})\)", body):
        lineno = body.count("\n", 0, m.start()) + 1
        target, label = m.group(1), " ".join(m.group(2).split())
        if True:
            if target not in files:
                violations.append(f"{name}:{lineno}: cites ADR-{target}, which does not exist")
                continue
            subj, subj_words = subject(target)
            citations.append((name, lineno, target, label))
            if words(label) & subj_words:
                continue
            if (f"ADR-{target}", label.lower()) in recorded:
                descriptive.append((name, lineno, target, label))
                continue
            violations.append(
                f"{name}:{lineno}: ADR-{target} ({label}) — the label names nothing in that "
                f"document's subject: {subj}"
            )

    for lineno, line in enumerate(body.split("\n"), 1):
        # Rule 4: section citations.
        for m in re.finditer(r"ADR-(\d{3})\s*§\s*(\d+)", line):
            target, section = m.group(1), m.group(2)
            if target not in files:
                continue  # already reported by rule 1 through the label form, if present
            body = open(files[target]).read()
            if not re.search(rf"^#{{2,3}}\s+{section}\.", body, re.M):
                violations.append(
                    f"{name}:{lineno}: cites ADR-{target} §{section}, and ADR-{target} has no "
                    f"§{section} heading"
                )
        # Rule 5: a filename citation must name a file. A path-qualified citation
        # (`mcp/.pi/architecture/decisions/ADR-005-….md`) resolves in the tree it names —
        # this repository carries several ADR sets, and a bare number means one document
        # inside `decisions/` and another inside `wasm/.pi/architecture/decisions/`.
        for m in re.finditer(r"((?:[A-Za-z0-9_.-]+/)*)(ADR-\d{3}-[a-z0-9-]+\.md)", line):
            prefix, cited = m.group(1), m.group(2)
            # Three roots, because the register is addressed three ways in practice: from the
            # repository root, from this crate's root (`.pi/architecture/decisions/…` is how
            # engine docs name it), and by bare filename inside `decisions/`.
            candidates = [
                os.path.join(repo_root, prefix, cited) if prefix else os.path.join(adr_dir, cited),
                os.path.join(tree_root, prefix, cited) if prefix else None,
            ]
            if not any(c and os.path.exists(c) for c in candidates):
                violations.append(
                    f"{name}:{lineno}: cites {prefix}{cited}, which does not exist anywhere it "
                    f"could mean (repo root, {os.path.basename(tree_root)}/, or "
                    f"{os.path.basename(adr_dir)}/)"
                )
        for m in re.finditer(r"(?<!ADR-\d{3} )§\s*(\d+)", line):
            section = m.group(1)
            body = open(path).read()
            if not re.search(rf"^#{{2,3}}\s+{section}\.", body, re.M):
                violations.append(
                    f"{name}:{lineno}: cites §{section}, and this ADR has no §{section} heading"
                )

if mode == "report":
    print(f"{len(files)} ADR(s), {len(citations)} labelled citation(s), "
          f"{len(descriptive)} recorded descriptive label(s)")
    for name, lineno, target, label in citations:
        tag = "subject" if words(label) & subject(target)[1] else "recorded"
        print(f"  {name}:{lineno}  ADR-{target}  [{tag}]  {label}")
    sys.exit(0)

if violations:
    print(f"{RED}❌ {len(violations)} ADR citation(s) point at a document that does not say "
          f"that{ RESET}", file=sys.stderr)
    for v in violations:
        print(f"   {v}", file=sys.stderr)
    print(f"{DIM}   a citation that leads nowhere is a claim: cite the paragraph, or record the "
          f"descriptive label in {os.path.relpath(labels_path)}{RESET}", file=sys.stderr)
    sys.exit(1)

print(f"{GREEN}✅ ADR references: {len(citations)} labelled citation(s) resolve to the subject they "
      f"name, {len(descriptive)} descriptive label(s) are recorded with a reason{RESET}")
sys.exit(0)
PY