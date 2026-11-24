#!/usr/bin/env bash
# ============================================================================
# validate-spawn-sites.sh — every child process is spawned with a declared env
# ============================================================================
# #1661. Measured before this guard: 68 `Command::new` sites across the five
# crates, and two policies among them. `run_command` cleared the environment and
# re-added an allowlist (#1653); the precondition runner inherited everything
# until it was routed through the same helper; and every fixed-argv `git` spawn
# inherited the whole parent environment — including `RIGORIX_HMAC_KEY`, which is
# what an audit envelope is signed with.
#
# The exposure is not hypothetical: `git commit` runs the repository's hooks, and
# a hook is a program from the repository. `RIGORIX_HMAC_KEY` reaching one is a
# repository reading the key that signs the trail recording what it did.
#
# Rules:
#
#   1. In production code (`src/`, excluding the test modules this tree names
#      `*_test.rs`, `*_tests.rs`, `tests.rs` or puts under `tests/`), a
#      `Command::new(` must be within `--window` lines of a declared environment
#      decision: `apply_child_env(`, `apply_git_child_env(` or `.env_clear()`.
#   2. A site that must not declare one — because the spawn *is* the helper, or
#      because a third party builds the command — is a row in
#      `.pi/spawn-sites.tsv` with a reason. A row without a reason is refused: an
#      exemption list without reasons is a list of names, and the next reader
#      cannot tell a decision from an oversight.
#   3. A row that matches no site is refused, because a stale exemption says the
#      opposite of the truth about the file it names.
#
# Scope: `engine/src`, `mcp/src`, `actions/src`, `cli/src`, `server/src`.

set -euo pipefail

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
HERE="$(dirname "$SELF")"
ROOT="$(cd "$HERE/../.." && pwd)"
WINDOW=15
SELF_TEST=0

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) SELF_TEST=1; shift ;;
    --root)
      [ $# -ge 2 ] || { echo "validate-spawn-sites: --root requires a path" >&2; exit 2; }
      ROOT="$2"; shift 2 ;;
    -h|--help) sed -n '2,34p' "$SELF" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "validate-spawn-sites: unknown argument '$1'" >&2; exit 2 ;;
    *) ROOT="$1"; shift ;;
  esac
done
ALLOWLIST="$ROOT/.pi/spawn-sites.tsv"

# One line per finding: UNCLAIMED, MALFORMED or STALE, then the detail.
scan() { # scan <root>
  python3 - "$1" "$WINDOW" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
window = int(sys.argv[2])
crates = ["engine", "mcp", "actions", "cli", "server"]
allowlist = root / ".pi" / "spawn-sites.tsv"


def is_test_path(relative):
    name = relative.name
    parts = set(relative.parts)
    return (
        name.endswith("_test.rs")
        or name.endswith("_tests.rs")
        or name == "tests.rs"
        or "tests" in parts
        or name == "test_utils.rs"
        or name == "mocks.rs"
        or name.endswith("_mocks.rs")
    )


allowed = {}
if allowlist.is_file():
    for raw in allowlist.read_text().splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        cells = raw.split("\t")
        if len(cells) < 2 or not cells[1].strip():
            print("MALFORMED\t" + raw)
            continue
        allowed[cells[0].strip()] = cells[1].strip()

findings = []
used = set()
for crate in crates:
    crate_root = root / crate / "src"
    if not crate_root.is_dir():
        continue
    for path in sorted(crate_root.rglob("*.rs")):
        relative = path.relative_to(root).as_posix()
        inner = path.relative_to(root / crate)
        if is_test_path(inner):
            continue
        lines = path.read_text(errors="replace").splitlines()
        # An inline `#[cfg(test)]` module ends the production region. Approximate on purpose: a test
        # module can declare its own `#[cfg(test)]` inner module, and the worst case is that the guard
        # looks at produced code only.
        production_end = len(lines)
        for index, line in enumerate(lines):
            if line.strip().startswith("#[cfg(test)]"):
                production_end = index
                break
        for index, line in enumerate(lines[:production_end]):
            if "Command::new" not in line:
                continue
            stripped = line.strip()
            if stripped.startswith("//") or stripped.startswith("*") or stripped.startswith("/*"):
                continue
            # A scan that names the pattern in a string is not a spawn — this guard has matched its own
            # subject's text twice in this repository's history, so the check is explicit.
            if line.count('"') % 2 == 1:
                continue
            ahead = "\n".join(lines[max(0, index - window): index + window + 1])
            if any(
                marker in ahead
                for marker in (
                    "apply_child_env(",
                    "apply_git_child_env(",
                    "apply_git_child_env_with(",
                    ".env_clear()",
                    "git_std_command(",
                    "git_tokio_command(",
                )
            ):
                continue
            if relative in allowed:
                used.add(relative)
                continue
            findings.append("UNCLAIMED\t{}:{}\t{}".format(relative, index + 1, line.strip()[:90]))

for row in allowed:
    if row not in used:
        findings.append("STALE\t" + row + "\t" + allowed[row][:80])

for finding in findings:
    print(finding)
PY
}

cleanup() { python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$1"; }

if [ "$SELF_TEST" -eq 1 ]; then
  echo "── validate-spawn-sites --self-test ──"
  TMP="$(mktemp -d)"
  trap 'cleanup "$TMP"' EXIT
  passes=0
  fails=0
  mkdir -p "$TMP/engine/src" "$TMP/.pi"

  expect_has() { # expect_has <label> <token> <output>
    if printf '%s' "$3" | grep -q "$2"; then
      echo "  ✅ $1"
      passes=$((passes + 1))
    else
      echo "  ❌ $1 — output was:"
      printf '%s\n' "$3" | sed 's/^/       /'
      fails=$((fails + 1))
    fi
  }
  expect_clean() { # expect_clean <label> <output>
    if [ -z "$2" ]; then
      echo "  ✅ $1"
      passes=$((passes + 1))
    else
      echo "  ❌ $1 — output was:"
      printf '%s\n' "$2" | sed 's/^/       /'
      fails=$((fails + 1))
    fi
  }

  cat >"$TMP/engine/src/a.rs" <<'RS'
pub async fn spawn_it() {
    let out = tokio::process::Command::new("git").output().await;
}
RS

  : >"$TMP/.pi/spawn-sites.tsv"
  expect_has "a production spawn with no declared environment is refused" "UNCLAIMED" "$(scan "$TMP")"
  expect_has "…and the finding names the file and line" "a.rs:2" "$(scan "$TMP")"

  cat >"$TMP/engine/src/a.rs" <<'RS'
pub async fn spawn_it() {
    let mut cmd = tokio::process::Command::new("git");
    apply_git_child_env(&mut cmd);
    let out = cmd.output().await;
}
RS
  expect_clean "a spawn that declares its environment passes" "$(scan "$TMP")"

  cat >"$TMP/engine/src/a.rs" <<'RS'
pub async fn spawn_it() {
    let out = tokio::process::Command::new("git").output().await;
}
RS
  printf 'engine/src/a.rs\t\n' >"$TMP/.pi/spawn-sites.tsv"
  expect_has "an exemption without a reason is refused" "MALFORMED" "$(scan "$TMP")"

  printf 'engine/src/gone.rs\tthe helper itself, which cannot call itself\n' >"$TMP/.pi/spawn-sites.tsv"
  expect_has "an exemption that matches no site is stale" "STALE" "$(scan "$TMP")"

  printf 'engine/src/a.rs\tthe helper itself, which cannot call itself\n' >"$TMP/.pi/spawn-sites.tsv"
  expect_clean "a claimed site passes" "$(scan "$TMP")"

  mkdir -p "$TMP/engine/src/tests"
  cat >"$TMP/engine/src/tests/mod.rs" <<'RS'
fn fixture() {
    let out = std::process::Command::new("git").output().unwrap();
}
RS
  expect_clean "a test module is not the guard's subject" "$(scan "$TMP")"

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ validate-spawn-sites self-test"
  exit 0
fi

[ -d "$ROOT/engine/src" ] || { echo "❌ validate-spawn-sites: $ROOT has no engine/src" >&2; exit 1; }

OUTPUT="$(scan "$ROOT")"
MALFORMED="$(printf '%s\n' "$OUTPUT" | grep -c '^MALFORMED' || true)"
STALE="$(printf '%s\n' "$OUTPUT" | grep -c '^STALE' || true)"
UNCLAIMED="$(printf '%s\n' "$OUTPUT" | grep -c '^UNCLAIMED' || true)"

if [ "$MALFORMED" -gt 0 ]; then
  echo "❌ spawn sites: $MALFORMED exemption(s) without a reason — a reasonless exemption is a name, not a decision"
  printf '%s\n' "$OUTPUT" | grep '^MALFORMED' | sed 's/^/  /'
  exit 1
fi
if [ "$STALE" -gt 0 ]; then
  echo "❌ spawn sites: $STALE exemption(s) match no site — a stale exemption says the opposite of the truth"
  printf '%s\n' "$OUTPUT" | grep '^STALE' | sed 's/^/  /'
  exit 1
fi
if [ "$UNCLAIMED" -gt 0 ]; then
  echo "❌ spawn sites: $UNCLAIMED production spawn(s) with no declared child environment (#1661)"
  printf '%s\n' "$OUTPUT" | grep '^UNCLAIMED' | sed 's/^/  /'
  echo "  Add \`apply_child_env\` / \`apply_git_child_env\` beside the spawn, or a row in"
  echo "  .pi/spawn-sites.tsv with a reason. The allowlist exists for a spawn that must inherit;"
  echo "  it is not a place for a site whose marker was inconvenient."
  exit 1
fi
echo "✅ spawn sites: every production spawn declares its child environment"