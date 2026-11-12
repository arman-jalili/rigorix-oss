#!/usr/bin/env bash
# validate-public-references.sh — public text may name only repositories the organisation publishes.
#
#   bash .pi/scripts/validate-public-references.sh
#   bash .pi/scripts/validate-public-references.sh --report
#   bash .pi/scripts/validate-public-references.sh --self-test
#
# # The rule
#
# Every repository-shaped token in this tree's governed text must be on the allowlist in
# `.pi/public-repositories.tsv`. A private repository is not on the allowlist, so it fails **by
# absence** — which is why the rule can live in a public repository at all: the denylist would
# have to write the private names down, which is the leak it prevents. The complement can.
#
# The guard's subject list is the registry, not a pattern set compiled into this file. A name flips
# from finding to pass by editing one row, with its evidence; and a name that is public *today* is
# not asserted to be public forever — a `repository` row carries the command that measured it.
#
# # What it governs, and what it does not
#
# Governed: `.md`, `.tsv`, `.yml`, `.yaml` — prose and configuration, where a claim is made — and
# `.sh`, because a shell script is a runbook: `demo/anchor-e2e/run.sh` is the entry point a stranger
# follows to run the flagship demonstration, and it named a private repository in a `mktemp` prefix
# while the line two below it used a neutral one (#1524). A script's literals are read by a person
# and pasted by a person, which is exactly what prose is.
#
# Not governed: compiled source (`.rs`, `.mjs`, `.js`, `.py`). `engine/src/**` is full of temp
# directories shaped like the organisation's; a fixture in a `std::env::temp_dir()` call is not a
# claim about whose repository something is. That hole is named here rather than papered over: a private
# name in a Rust comment would not be caught, and the guard's output should not be read as "the
# tree names nothing private".
#
# The guard's own fixtures use a placeholder (`rigorix-not-published`), never a real name: an
# unregistered token in a governed file must fail and the message must name file, line and token,
# and a placeholder exercises that identically.
#
# This script is the one file it does not read. A self-test has to plant a token that is **not**
# registered — that is what makes it a plant — so scanning itself would force the plant into the
# registry, and a registered plant passes for the wrong reason. The exclusion is named here, and
# the seeds assert that a `.sh` file *is* governed, so the exclusion cannot silently grow to cover
# the taxonomy.
#
# # Exit codes
#
#   0  every governed token is registered
#   1  a token is not registered — a violation, remedy: rewrite the text or add a row
#   2  the registry cannot be read as a registry — malformed, remedy: fix the file
#
# Malformed and violating have different remedies, so they have different exits (#1505).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
REGISTRY="$ROOT/.pi/public-repositories.tsv"
MODE="check"

while [ $# -gt 0 ]; do
  case "$1" in
    --report) MODE="report"; shift ;;
    --self-test) MODE="self-test"; shift ;;
    --registry) [ $# -ge 2 ] || { echo "validate-public-references: --registry needs a path" >&2; exit 2; }; REGISTRY="$2"; shift 2 ;;
    --root) [ $# -ge 2 ] || { echo "validate-public-references: --root needs a directory" >&2; exit 2; }; ROOT="$2"; shift 2 ;;
    -h|--help) sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "validate-public-references: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

if [ "$MODE" = "self-test" ]; then
  scratch="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "find '$scratch' -type f -delete 2>/dev/null; find '$scratch' -depth -type d -exec rmdir {} + 2>/dev/null || true" EXIT
  mkdir -p "$scratch/workflows" "$scratch/docs"

  cp "$ROOT/.pi/public-repositories.tsv" "$scratch/registry.tsv"

  # Each case writes the whole tree it wants read: one workflow, one document.
  plant() {  # plant <workflow-body> <doc-body>
    printf 'name: probe\non: workflow_dispatch\n%s\n' "$1" > "$scratch/workflows/probe.yml"
    printf '# probe\n\n%s\n' "$2" > "$scratch/docs/probe.md"
  }
  run() { bash "$0" --root "$scratch" --registry "$scratch/registry.tsv"; }

  ACCEPTED=0
  REFUSED=0
  MALFORMED=0
  case_() {  # case_ <label> <expect pass|fail|malformed> <workflow-body> <doc-body>
    local label="$1" expect="$2" out status
    plant "$3" "$4"
    set +e
    out=$(run 2>&1)
    status=$?
    set -e
    case "$expect" in
      pass) [ "$status" -eq 0 ] || { echo "✗ self-test: $label passed when it must fail" >&2; printf '%s\n' "$out" >&2; return 1; }; ACCEPTED=$((ACCEPTED + 1)) ;;
      fail) [ "$status" -eq 1 ] || { echo "✗ self-test: $label did not fail as a violation (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; }; REFUSED=$((REFUSED + 1)) ;;
      malformed) [ "$status" -eq 2 ] || { echo "✗ self-test: $label was not refused as malformed (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; }; MALFORMED=$((MALFORMED + 1)) ;;
    esac
    echo "  ✓ $label"
  }

  echo "── self-test: the registry decides, and the plants are the two cases that matter ──"

  # Plant 1, required: an unregistered name, in a workflow. It must fail, and it must name where.
  # The placeholder is deliberate — a refusal fixture does not have to be a real repository, and
  # using one made this script the last place the name lived (#1524).
  case_ "an unregistered repository's name in a workflow is refused" fail \
    'jobs:
  probe:
    steps:
      - run: git clone https://github.com/arman-jalili/rigorix-not-published.git' ''
  out=$(run 2>&1 || true)
  case "$out" in
    *"workflows/probe.yml:"*"rigorix-not-published"*) echo "  ✓ the refusal names file, line and token" ;;
    *) echo "✗ self-test: the refusal did not name file:line:token: $out" >&2; exit 1 ;;
  esac

  # Plant 1b: the same name in a shell script, which is governed for the same reason a document is.
  printf 'name: probe\non: workflow_dispatch\n' > "$scratch/workflows/probe.yml"
  printf '# probe\n' > "$scratch/docs/probe.md"
  printf '#!/usr/bin/env bash\nlog="$(mktemp "/tmp/rigorix-not-published.XXXXXX")"\n' > "$scratch/docs/probe.sh"
  set +e
  out=$(run 2>&1)
  status=$?
  set -e
  if [ "$status" -eq 1 ] && printf '%s' "$out" | grep -q 'docs/probe.sh:2: rigorix-not-published'; then
    echo "  ✓ a shell script is governed, and the refusal names file, line and token"
    REFUSED=$((REFUSED + 1))
  else
    echo "✗ self-test: a shell script was not governed (status $status)" >&2
    printf '%s\n' "$out" >&2
    exit 1
  fi
  rm -f "$scratch/docs/probe.sh"

  # Plant 2, required: a *public* name in the same position. If this failed, the check would be
  # rejecting a shape rather than consulting the registry — and would pass for the wrong reason.
  case_ "the same shape naming a published repository passes" pass \
    'jobs:
  probe:
    steps:
      - run: git clone https://github.com/arman-jalili/rigorix-sdk.git' ''

  # A crate is not a repository: the names differ by kind, not by trust.
  case_ "a published artifact's name passes" pass '' 'The engine crate `rigorix-engine` and the SDK `rigorix-sdk` agree.'
  # A glob row covers a fixture family, so a new fixture in that family does not need a row.
  case_ "a token matching a registered glob passes" pass '' 'The hook `rigorix-hook-validate-path` runs first.'
  # The shape is the organisation's, so a *published-looking* unknown name still fails.
  case_ "a published demo repository ('conference-demo') passes" pass '' 'See `conference-demo` for the sequence policy.'
  case_ "an unknown demo-shaped name is refused" fail '' 'Measured on `nobody-demo` after `reset-demo.sh`.'
  case_ "an unknown rigorix-shaped name is refused" fail '' 'The `rigorix-someone-elses-repo` helper does nothing.'

  # Malformed registry: a repository row with no measured evidence is a row typed from memory.
  printf 'rigorix-oss\trepository\tit is public, I checked\n' > "$scratch/registry.tsv"
  plant '' ''
  set +e
  out=$(run 2>&1)
  status=$?
  set -e
  if [ "$status" -eq 2 ] && printf '%s' "$out" | grep -q PUBLIC; then
    echo "  ✓ a repository row without a measured result is malformed, and says why"
    MALFORMED=$((MALFORMED + 1))
  else
    echo "✗ self-test: an unmeasured repository row was not refused as malformed (status $status)" >&2
    printf '%s\n' "$out" >&2
    exit 1
  fi

  # An empty registry is never a pass: a check with no subject.
  : > "$scratch/registry.tsv"
  set +e
  out=$(run 2>&1)
  status=$?
  set -e
  if [ "$status" -eq 2 ]; then
    echo "  ✓ an empty registry is refused"
    MALFORMED=$((MALFORMED + 1))
  else
    echo "✗ self-test: an empty registry was not refused (status $status)" >&2
    exit 1
  fi

  # Derived, never typed: the summary cannot describe a run it did not have (#1508).
  printf '✅ %s acceptance case(s) passed, %s rejection case(s) refused, %s malformed-input ' \
    "$ACCEPTED" "$REFUSED" "$MALFORMED"
  printf 'case(s) refused as such\n'
  exit 0
fi

exec python3 - "$ROOT" "$REGISTRY" "$MODE" <<'PY'
import fnmatch
import os
import re
import subprocess
import sys

ROOT, REGISTRY, MODE = sys.argv[1], sys.argv[2], sys.argv[3]
GREEN, RED, DIM, RESET = "\033[0;32m", "\033[0;31m", "\033[2m", "\033[0m"
KINDS = {"repository", "artifact", "workspace-package", "proposed-crate", "rejected", "identifier"}
GOVERNED = (".md", ".tsv", ".yml", ".yaml", ".sh")
SKIP_DIRS = ("node_modules/", "target/", ".git/")

# The organisation's name shape: hyphen-separated lowercase segments, no trailing hyphen. The
# second alternative is the demo-repository shape the issue names. Everything extracted is decided
# by the registry; nothing is decided here.
SHAPE = re.compile(r"\brigorix-[a-z0-9]+(?:-[a-z0-9]+)*\b|\b[a-z0-9]+(?:-[a-z0-9]+)*-demo\b")


def fail_malformed(msg):
    print(f"{RED}❌ the registry is not readable as a registry{RESET}", file=sys.stderr)
    print(f"   {msg}", file=sys.stderr)
    print(f"{DIM}   remedy: fix {os.path.relpath(REGISTRY, ROOT)} — a malformed record and a "
          f"violation have different fixes{RESET}", file=sys.stderr)
    sys.exit(2)


def load():
    rows, seen = [], set()
    try:
        text = open(REGISTRY).read()
    except OSError as exc:
        fail_malformed(f"{REGISTRY}: {exc.strerror}")
    for lineno, line in enumerate(text.split("\n"), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            fail_malformed(f"{os.path.basename(REGISTRY)}:{lineno}: expected 3 tab-separated "
                           f"columns (token, kind, evidence), found {len(parts)}")
        token, kind, evidence = parts[0].strip(), parts[1].strip(), parts[2].strip()
        if kind not in KINDS:
            fail_malformed(f"{os.path.basename(REGISTRY)}:{lineno}: kind '{kind}' is not one of "
                           f"{', '.join(sorted(KINDS))}")
        # A reason has to be a reason. For a *package* that means a registry was asked; for an
        # identifier it means the row says what the token is instead of restating it.
        if kind in {"artifact", "workspace-package", "proposed-crate"} and not re.search(
                r"crates\.io|pypi|npm|maven|github\.com|gh repo|registr", evidence, re.I):
            fail_malformed(f"{os.path.basename(REGISTRY)}:{lineno}: '{token}' is registered as a "
                           f"{kind} without naming the registry its status was measured on")
        if kind in {"identifier", "rejected"} and len(evidence) < 20:
            fail_malformed(f"{os.path.basename(REGISTRY)}:{lineno}: '{evidence}' is not evidence "
                           f"— say what the name is, or the command that measured it")
        if kind == "repository":
            # A repository is on the allowlist because it was *measured* public, not because it
            # was believed to be. The row must carry the measurement.
            if not re.search(r"gh repo (view|list)", evidence) or not re.search(
                    r"public", evidence, re.I):
                fail_malformed(
                    f"{os.path.basename(REGISTRY)}:{lineno}: '{token}' is registered as a "
                    f"repository without the command that showed it is PUBLIC — run "
                    f"`gh repo view arman-jalili/{token} --json visibility` and record the result"
                )
        if token in seen:
            fail_malformed(f"{os.path.basename(REGISTRY)}:{lineno}: '{token}' is registered twice")
        seen.add(token)
        rows.append((token, kind, evidence))
    if not rows:
        fail_malformed("the registry is empty, so every token would be a violation and a check "
                       "with no subject is not a pass")
    return rows


rows = load()


def registered(token):
    for name, kind, _ in rows:
        if name == token or ("*" in name and fnmatch.fnmatchcase(token, name)):
            return kind
    return None


def tree_paths():
    """Every tracked path, or — outside a repository, where the self-test reads a scratch tree —
    every file on disk. One rule, two sources of the same list."""
    try:
        out = subprocess.run(["git", "-C", ROOT, "ls-files"], capture_output=True, text=True,
                             check=True).stdout
        return [p for p in out.split("\n") if p]
    except (subprocess.CalledProcessError, FileNotFoundError):
        found = []
        for base, dirs, names in os.walk(ROOT):
            dirs[:] = [d for d in dirs if d not in {"node_modules", "target", ".git"}]
            for name in names:
                found.append(os.path.relpath(os.path.join(base, name), ROOT))
        return found


def governed_files():
    for path in tree_paths():
        if path.startswith(SKIP_DIRS) or not path.endswith(GOVERNED):
            continue
        if os.path.abspath(os.path.join(ROOT, path)) == os.path.abspath(REGISTRY):
            continue  # the registry names what it registers; it is the allowlist, not a subject
        if path.endswith("/validate-public-references.sh") or path == "validate-public-references.sh":
            continue  # its fixtures must stay unregistered — see the scope note above
        yield path


violations, seen_tokens = [], {}
for path in governed_files():
    try:
        text = open(os.path.join(ROOT, path), errors="ignore").read()
    except OSError:
        continue  # a broken symlink carries no prose
    for lineno, line in enumerate(text.split("\n"), 1):
        for m in SHAPE.finditer(line):
            token = m.group(0)
            kind = registered(token)
            if kind is None:
                violations.append((path, lineno, token))
            else:
                seen_tokens[token] = seen_tokens.get(token, 0) + 1

if MODE == "report":
    kinds = {}
    for name, kind, _ in rows:
        kinds[kind] = kinds.get(kind, 0) + 1
    governed = sum(1 for _ in governed_files())
    print(f"{governed} governed file(s); registry: "
          + ", ".join(f"{n} {k}" for k, n in sorted(kinds.items())))
    for token, hits in sorted(seen_tokens.items(), key=lambda kv: -kv[1]):
        print(f"  {token:38} {hits:4} use(s)   [{registered(token)}]")
    if violations:
        print(f"  {len(violations)} token(s) NOT registered:")
        for path, lineno, token in violations:
            print(f"    {path}:{lineno}  {token}")
    sys.exit(0)

if violations:
    print(f"{RED}❌ {len(violations)} name(s) in this tree are not registered as public{RESET}",
          file=sys.stderr)
    for path, lineno, token in violations:
        print(f"   {path}:{lineno}: {token}", file=sys.stderr)
    print(f"{DIM}   a name that is not on the allowlist is not published. Either say what it is —"
          f" the artifact `…`, the enterprise deployment, a private downstream workstream — or"
          f" record it in {os.path.relpath(REGISTRY, ROOT)} with its kind and the command that"
          f" showed it is public{RESET}", file=sys.stderr)
    sys.exit(1)

print(f"{GREEN}✅ public references: every one of {len(seen_tokens)} registered name(s) in governed "
      f"text names something published or is recorded as an identifier{RESET}")
sys.exit(0)
PY