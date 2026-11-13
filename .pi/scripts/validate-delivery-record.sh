#!/usr/bin/env bash
# validate-delivery-record.sh — every number in a delivery record resolves to the thing it names.
#
#   bash .pi/scripts/validate-delivery-record.sh <path>
#   bash .pi/scripts/validate-delivery-record.sh -                # stdin: ... | bash …/validate-delivery-record.sh -
#   bash .pi/scripts/validate-delivery-record.sh --pr <n>         # gh pr view <n> --json body -q .body
#   bash .pi/scripts/validate-delivery-record.sh --comment <url>  # gh api …/issues/comments/<id> --jq .body
#   bash .pi/scripts/validate-delivery-record.sh --self-test      # fixture repo, six cases, no history needed
#
# Options: --allow-empty (a text with no claims is not a record; only pass this for non-records),
#          --repo <path> (the repository to resolve against; default: this one), --report (list claims only).
#
# Exit: 0 every claim resolved · 1 a claim does not resolve · 2 usage or input unavailable.
#
# # Why this exists
#
# #1549 counted three unresolvable numbers in one delivery record: a sha that existed in the object
# database but was never on `main` (it was the branch tip; the squash `9c22c52d` was what landed), a
# guard count (`17/17`) that was one guard's internal assertion count presented as the suite's, and
# "all gating guards passed" for a subject the local runner does not contain at all (#1546). None of
# the three is an arithmetic error. Each is a number carried from where it was true into a sentence
# that asserts it is true here.
#
# The remedy is not more care, it is that each figure is printed by a command whose output the
# record quotes — and, failing that, that a check refuses the record.
#
# # The three states of a sha (never one "invalid sha")
#
#   never pushed   `git cat-file -e <sha>^{commit}` fails — no such object anywhere
#   off-branch     resolves, but `git merge-base --is-ancestor <sha> main` fails: the commit exists
#                  and is not what landed. The message names the branch that contains it and, when a
#                  branch name and a `main` subject agree on a number, the squash it became.
#   on main        both succeed
#
# # Claims are positions, not tokens
#
# `[0-9a-f]{8}` appears in digest prefixes, fixture ids and RUSTSEC numbers; a check over every token
# fails on a correct record. Only a token in a claim position is an assertion: "merged at", "landed
# as", "on main at", "green at", "→" before a sha, "(commit|sha) <hex>". A token of bare digits is
# not a sha. An allow-list of positions is what makes this usable; an allow-list of files would make
# it depend on a directory the repository does not ship.
#
# # A count and a guard claim name their subject
#
# `126/0`, `18/18`, `17/17` are meaningless without the command that printed them, so a count is an
# error unless the line it is on, or the line above it, names a producer (a script, a target, a run
# id). The same rule applied to "all gating guards passed": a verdict about guards with no guard
# script or job named is not evidence — it is a sentence about a subject nobody identified, which is
# exactly how one guard's self-test count became the suite's result.
#
# # Prose is the record's assertion; a fenced block is output
#
# A record asserts in its prose and its tables. Inside a fenced block it is quoting what a command
# printed, so the block is checked for the one thing that makes the quote evidence — the invocation
# on its first line — and its contents are not re-scanned as the record's own claims. Without that,
# a record could not show this check refusing anything: the pasted refusal would read as an
# assertion. An output block with no command is a finding, so nothing is hidden by fencing; and a
# record that names a command which did not print the block is a different and visible lie.
#
# A record with nothing to verify is not a record: 0 claims is exit 1 unless --allow-empty is passed
# explicitly. A stage that is silently absent reports green; so does a check with no subject.
set -euo pipefail

MODE="check"
INPUT=""
PR=""
COMMENT=""
ALLOW_EMPTY="no"
REPO=""

usage() {
  sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

die_usage() {
  echo "validate-delivery-record: $1" >&2
  echo >&2
  usage >&2
  exit 2
}

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) MODE="self-test"; shift ;;
    --report) MODE="report"; shift ;;
    --allow-empty) ALLOW_EMPTY="yes"; shift ;;
    --pr)
      [ $# -ge 2 ] || die_usage "--pr needs a pull-request number"
      PR="$2"; shift 2 ;;
    --pr=*) PR="${1#--pr=}"; shift ;;
    --comment)
      [ $# -ge 2 ] || die_usage "--comment needs a comment URL or id"
      COMMENT="$2"; shift 2 ;;
    --comment=*) COMMENT="${1#--comment=}"; shift ;;
    --repo)
      [ $# -ge 2 ] || die_usage "--repo needs a directory"
      REPO="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    -) INPUT="-"; shift ;;
    -*) die_usage "unknown argument '$1'" ;;
    *) INPUT="$1"; shift ;;
  esac
done

if [ -z "$REPO" ]; then
  REPO="$(git rev-parse --show-toplevel 2>/dev/null || true)"
  [ -n "$REPO" ] || REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fi

# ---------------------------------------------------------------------------
# Self-test: a fixture repository, not this one's history (#1549)
# ---------------------------------------------------------------------------
if [ "$MODE" = "self-test" ]; then
  scratch="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "find '$scratch' -type f -delete 2>/dev/null; find '$scratch' -depth -type d -exec rmdir {} + 2>/dev/null || true" EXIT
  GIT="git -C $scratch -c user.email=t@example.com -c user.name=T -c commit.gpgsign=false \
       -c init.defaultBranch=main -c advice.detachedHead=false"
  $GIT init -q >/dev/null 2>&1
  echo one > "$scratch/f"
  $GIT add f >/dev/null 2>&1
  $GIT commit -qm "feat: the thing that landed (#1537)" >/dev/null 2>&1
  MAIN_SHA="$($GIT rev-parse HEAD)"
  $GIT checkout -q -b feature/1537-the-thing >/dev/null 2>&1
  echo two >> "$scratch/f"
  $GIT commit -qam "feat: the work, before the squash" >/dev/null 2>&1
  BRANCH_SHA="$($GIT rev-parse HEAD)"
  $GIT checkout -q main >/dev/null 2>&1

  ACCEPTED=0
  REFUSED=0
  USAGE=0
  case_() {  # case_ <label> <expect 0|1> <record text>
    local label="$1" expect="$2" text="$3" out status
    printf '%s\n' "$text" > "$scratch/record.md"
    set +e
    out=$(bash "$0" --repo "$scratch" "$scratch/record.md" 2>&1)
    status=$?
    set -e
    if [ "$status" -ne "$expect" ]; then
      echo "✗ self-test: $label — expected exit $expect, got $status" >&2
      printf '%s\n' "$out" >&2
      return 1
    fi
    LAST_OUT="$out"
    if [ "$expect" -eq 0 ]; then ACCEPTED=$((ACCEPTED + 1)); else REFUSED=$((REFUSED + 1)); fi
    echo "  ✓ $label"
  }
  expect_word() {  # expect_word <label> <word>
    case "$LAST_OUT" in
      *"$2"*) ;;
      *) echo "✗ self-test: $1 — the message does not say '$2'" >&2; printf '%s\n' "$LAST_OUT" >&2; return 1 ;;
    esac
  }

  echo "── self-test: a claim that does not resolve is a finding ──"

  case_ "a branch sha cited as merged is refused" 1 "Merged at $BRANCH_SHA." 
  expect_word "off-branch" "off-branch"

  case_ "an absent sha is refused, and differently" 1 "merged at deadbeefdeadbeef"
  expect_word "never-pushed" "never pushed"

  case_ "a prose digest prefix is not a claim" 0 "The digest is sha256:ab77bb82… and never resolves.
The run landed at $MAIN_SHA."
  case_ "a bare count with no producer is refused" 1 "self-test 17/17"
  expect_word "bare count" "names no subject"

  case_ "a count that names its producer passes" 0 "18/18 (validate-guards.sh --coverage)"
  case_ "an output block with no command is refused" 1 "The run printed:

\`\`\`
126 checks, 0 failed
\`\`\`
"
  expect_word "fence-without-command" "names no command"

  case_ "a prose digest is not a claim, and a fenced block is output" 0 "The release digest is \
sha256:ab77bb82 in prose, and the block below names its command:

\`\`\`
\$ bash .pi/scripts/validate-guards.sh
17/17
\`\`\`
"
  case_ "a record with no claims is refused" 1 "Nothing to see here."
  expect_word "no-claims" "no claims found"

  printf '%s\n' "  ✓ no-claims refusal says the text is not a record"
  # usage: no arguments is not a pass
  set +e
  out=$(bash "$0" 2>&1); status=$?
  set -e
  if [ "$status" -ne 2 ]; then
    echo "✗ self-test: no arguments — expected exit 2, got $status" >&2
    printf '%s\n' "$out" >&2
    exit 1
  fi
  case "$out" in
    *"Usage"*|*"bash .pi/scripts/validate-delivery-record.sh <path>"*) USAGE=$((USAGE + 1)) ;;
    *) echo "✗ self-test: no-argument refusal printed no usage" >&2; exit 1 ;;
  esac
  set +e
  out=$(bash "$0" --repo "$scratch" "$scratch/does-not-exist.md" 2>&1); status=$?
  set -e
  if [ "$status" -ne 2 ]; then
    echo "✗ self-test: an unreadable path — expected exit 2, got $status" >&2
    exit 1
  fi
  USAGE=$((USAGE + 1))

  printf '✅ %s acceptance case(s) passed, %s finding(s) refused, %s usage case(s) refused\n' \
    "$ACCEPTED" "$REFUSED" "$USAGE"
  exit 0
fi

# ---------------------------------------------------------------------------
# Resolve the subject: a path, stdin, a PR body, or a comment body
# ---------------------------------------------------------------------------
TMP=""
LABEL=""

cleanup_tmp() { [ -n "$TMP" ] && find "$(dirname "$TMP")" -name "$(basename "$TMP")" -delete 2>/dev/null; return 0; }
trap cleanup_tmp EXIT

if [ -n "$PR" ]; then
  command -v gh >/dev/null 2>&1 || { echo "validate-delivery-record: --pr needs gh on PATH" >&2; exit 2; }
  [ -n "$INPUT" ] && die_usage "--pr and a path are two subjects; pick one"
  TMP="$(mktemp)"
  if ! gh pr view "$PR" --json body -q .body >"$TMP" 2>/dev/null; then
    echo "validate-delivery-record: cannot read the body of PR $PR (gh failed)" >&2
    exit 2
  fi
  LABEL="PR #$PR body"
elif [ -n "$COMMENT" ]; then
  command -v gh >/dev/null 2>&1 || { echo "validate-delivery-record: --comment needs gh on PATH" >&2; exit 2; }
  [ -n "$INPUT" ] && die_usage "--comment and a path are two subjects; pick one"
  TMP="$(mktemp)"
  repo_slug="$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || true)"
  [ -n "$repo_slug" ] || { echo "validate-delivery-record: cannot resolve the repository with gh" >&2; exit 2; }
  case "$COMMENT" in
    *issuecomment-*) id="${COMMENT##*issuecomment-}" ;;
    *[!0-9]*) echo "validate-delivery-record: --comment wants an issue comment URL or a numeric id" >&2; exit 2 ;;
    *) id="$COMMENT" ;;
  esac
  if ! gh api "repos/$repo_slug/issues/comments/$id" --jq .body >"$TMP" 2>/dev/null; then
    echo "validate-delivery-record: cannot read comment $id (gh failed)" >&2
    exit 2
  fi
  LABEL="comment $id"
elif [ "$INPUT" = "-" ]; then
  TMP="$(mktemp)"
  cat >"$TMP"
  LABEL="stdin"
else
  [ -n "$INPUT" ] || die_usage "no record to check — pass a path, -, --pr <n>, --comment <url>, or --self-test"
  [ -r "$INPUT" ] || { echo "validate-delivery-record: cannot read $INPUT" >&2; exit 2; }
  LABEL="$INPUT"
  TMP="$INPUT"
fi

exec python3 - "$REPO" "$LABEL" "$MODE" "$ALLOW_EMPTY" "$TMP" <<'PY'
import os
import re
import subprocess
import sys

REPO, LABEL, MODE, ALLOW_EMPTY, RECORD = sys.argv[1:6]
GREEN, RED, DIM, RESET = "\033[0;32m", "\033[0;31m", "\033[2m", "\033[0m"

text = open(RECORD, encoding="utf-8", errors="replace").read()
lines = text.split("\n")

# A claim position, and nothing else, is an assertion about a sha. Bare digits are not a sha
# (`43446733161` is a CI run id), so a token must carry at least one hex letter.
SHA = r"[0-9a-fA-F]{7,40}"
CLAIM_PATTERNS = [
    re.compile(r"(?i)\b(?:merged|landed|squashed|pushed|released)\s+(?:at|as|in|into)\s+`?(" + SHA + ")"),
    re.compile(r"(?i)\bon\s+main\s+(?:at|as)\s+`?(" + SHA + ")"),
    re.compile(r"(?i)\bgreen\s+(?:at|on)\s+`?(" + SHA + ")"),
    re.compile(r"(?:→|->|=>)\s*`?(" + SHA + ")"),
    re.compile(r"(?i)\b(?:commit|sha|squash(?:ed)?)\s+(?:\w+\s+)?`?(" + SHA + ")"),
    # "merged, closed — `c10b1d70`": a status word followed by a sha is a claim whatever punctuates
    # the middle, which is the shape #1549's first defect had.
    re.compile(r"(?i)\b(?:merged|landed|squashed|shipped|released)\b.{0,30}?`?(" + SHA + ")"),
]
DECLARED_BRANCH = re.compile(r"(?i)\bbranch\s+`?(" + SHA + r")")

# A producer is what makes a number evidence: the command, the target, or the run.
PRODUCER = re.compile(
    r"(?i)(?:\.sh\b|\.py\b|\.rs\b|\.tsv\b|\.json\b|\.yml\b|\.yaml\b|\.toml\b"
    r"|\bcargo\b|\bnextest\b|\bgh\s+[a-z]+|\brun\s+\d{6,}\b|\bvalidate-[a-z0-9-]+"
    r"|\blocal-ci\b|\bci\.yml\b|\bworkflow\b|\bmake\s+[a-z-]+|\bpytest\b|\bvitest\b|\bjest\b"
    r"|--self-test|--coverage)"
)
COUNT = re.compile(r"(?i)\b\d+\s*/\s*\d+\b|\b\d+\s+(?:[a-z]+\s+){0,2}(?:cases?|checks?|tests?|rows?|steps?)\b")
GUARD_WORD = re.compile(r"(?i)\bguards?\b|\bvalidators?\b")
VERDICT = re.compile(r"(?i)\b(?:pass(?:es|ed|ing)?|green|clean|covered|fails?|failing|red)\b")


def git(*args):
    """Return (ok, stdout). Never raise: a missing git or a malformed repo is an input problem,
    and the caller decides whether that is exit 2 or a finding."""
    try:
        proc = subprocess.run(["git", "-C", REPO, *args], capture_output=True, text=True)
    except FileNotFoundError:
        return False, ""
    return proc.returncode == 0, proc.stdout


def has_commit(sha):
    return git("cat-file", "-e", f"{sha}^{{commit}}")[0]


def is_on_main(sha):
    return git("merge-base", "--is-ancestor", sha, "main")[0]


def branches_containing(sha):
    names = []
    for args in (("branch", "--contains", sha), ("branch", "-r", "--contains", sha)):
        ok, out = git(*args)
        if not ok:
            continue
        for raw in out.split("\n"):
            name = raw.strip().lstrip("*").strip()
            if not name or name.endswith("/HEAD") or "->" in name:
                continue
            if name not in names:
                names.append(name)
    return names


def squash_for(branches):
    """The squash a branch became, discovered from local data only: a branch name carries an issue
    or PR number, and `main` carries a commit whose subject ends `(#<that number>)`."""
    ok, log = git("log", "main", "--oneline", "-n", "400")
    if not ok:
        return None
    for branch in branches:
        for number in re.findall(r"\d{2,}", branch):
            for entry in log.split("\n"):
                if f"(#{number})" in entry:
                    return entry.strip()
    return None


def quoted(line, limit=90):
    flat = " ".join(line.split())
    return flat if len(flat) <= limit else flat[: limit - 1] + "…"


claims = 0
resolved = 0
findings = []

if not has_commit("HEAD") or not git("rev-parse", "--verify", "main")[0]:
    print(f"{RED}❌ the repository cannot be read{RESET}", file=sys.stderr)
    print(f"   {REPO}: not a git repository with a `main` branch, so no claim can be resolved",
          file=sys.stderr)
    print(f"{DIM}   remedy: pass --repo <path> pointing at the repository the record describes; a"
          f" check with no subject is not a pass{RESET}", file=sys.stderr)
    sys.exit(2)

# A claim is the record's own assertion, and the record makes assertions in its prose. Inside a
# fenced block it is quoting what a command printed, and the thing to check is that the block names
# the command — otherwise pasting a refusal would read as a claim, and a record could never show the
# check failing. An output block with no producer is a finding, so nothing can be hidden by fencing.
in_fence = [False] * len(lines)
blocks = []
fence_start = None
for i, line in enumerate(lines):
    if line.lstrip().startswith("```"):
        in_fence[i] = True
        if fence_start is None:
            fence_start = i
        else:
            blocks.append((fence_start, i))
            fence_start = None
        continue
    in_fence[i] = fence_start is not None
if fence_start is not None:
    blocks.append((fence_start, len(lines)))

for start, end in blocks:
    claims += 1
    if any(PRODUCER.search(b) for b in lines[start + 1 : end]):
        resolved += 1
    else:
        excerpt = lines[start + 1] if start + 1 < len(lines) else ""
        findings.append((start + 1, excerpt,
                         "output block names no command: a pasted result without the invocation that "
                         "printed it is not evidence — put the command on the block's first line"))

for index, line in enumerate(lines):
    if not line.strip() or in_fence[index]:
        continue
    context = line + "\n" + (lines[index - 1] if index else "")
    produced = bool(PRODUCER.search(context))

    # --- 1. shas in claim positions -------------------------------------------------
    cited = []
    for pattern in CLAIM_PATTERNS:
        for match in pattern.finditer(line):
            sha = match.group(1).lower()
            if sha in {s for s, _ in cited}:
                continue
            cited.append((sha, match.start()))
    if cited:
        states = {}
        for sha, _pos in cited:
            if not has_commit(sha) and not re.search(r"[a-f]", sha):
                # All digits: a run id and a PR number look the same, and neither is a commit.
                states[sha] = "not-a-sha"
            elif not has_commit(sha):
                states[sha] = "never"
            elif is_on_main(sha):
                states[sha] = "main"
            else:
                states[sha] = "off"
            claims += 1
        landed_here = any(state == "main" for state in states.values())
        # "branch <sha> → squash <sha>": a branch sha quoted beside the commit it became is the form
        # the rule asks for, so it resolves — the citation says which is which.
        branch_at = line.lower().find("branch")
        for sha, pos in cited:
            if states[sha] == "not-a-sha":
                findings.append((index + 1, line,
                                 f"`{sha}` is not a sha: `git cat-file -e {sha}^{{commit}}` fails and it "
                                 f"is all digits, which is how a run id or a PR number is also written "
                                 f"— quote the hex commit, or drop it from a claim position"))
                continue
            if states[sha] == "never":
                findings.append((index + 1, line,
                                 f"`{sha}` was never pushed: `git cat-file -e {sha}^{{commit}}` fails, "
                                 f"so this commit exists nowhere and cannot be what landed"))
                continue
            if states[sha] == "main":
                resolved += 1
                continue
            if branch_at >= 0 and pos > branch_at and landed_here:
                resolved += 1
                continue
            branches = branches_containing(sha)
            squash = squash_for(branches)
            named = ", ".join(f"`{b}`" for b in branches) or "no branch this checkout knows about"
            where = f"off-branch: `{sha}` resolves but `git merge-base --is-ancestor {sha} main` fails" \
                    f" — it is the tip of {named}, not what landed"
            if squash:
                where += f"; `main` carries `{squash}`, which is the squash to cite"
            findings.append((index + 1, line, where))

    # --- 2. a count names its producer ----------------------------------------------
    for match in COUNT.finditer(line):
        claims += 1
        if produced:
            resolved += 1
            continue
        findings.append((index + 1, line,
                         f'count "{match.group(0).strip()}" names no subject: a number without the '
                         f"command that printed it is not evidence — name the script or target on "
                         f"this line or the line above"))

    # --- 3. a guard claim names its subject -----------------------------------------
    if GUARD_WORD.search(line) and VERDICT.search(line) and not produced:
        claims += 1
        findings.append((index + 1, line,
                         "guard verdict names no subject: say which guard script, job or run decided "
                         "it — 'all guards passed' is a sentence about a subject nobody identified"))

if MODE == "report":
    print(f"{LABEL}: {len(lines)} line(s), {claims} claim(s), {resolved} resolved, "
          f"{len(findings)} unresolved")
    for lineno, _, message in findings:
        print(f"  {lineno}: {message}")
    sys.exit(0)

if claims == 0:
    if ALLOW_EMPTY == "yes":
        print(f"{GREEN}✅ {LABEL}: 0 claims (empty text allowed explicitly){RESET}")
        sys.exit(0)
    print(f"{RED}❌ no claims found in {LABEL}{RESET}", file=sys.stderr)
    print(f"{DIM}   a record with nothing to verify is not a record, so this is exit 1: cite the shas,"
          f" counts and verdicts the work produced, or pass --allow-empty if the text is not a"
          f" record{RESET}", file=sys.stderr)
    sys.exit(1)

for lineno, line, message in findings:
    print(f"{RED}❌ {LABEL}:{lineno}{RESET} {message}")
    print(f"{DIM}   {quoted(line)}{RESET}")

if findings:
    print(f"{RED}❌ {LABEL}: {claims} claim(s), {resolved} resolved, {len(findings)} unresolved{RESET}")
    sys.exit(1)

print(f"{GREEN}✅ {LABEL}: {claims} claim(s), {resolved} resolved, 0 unresolved{RESET}")
PY