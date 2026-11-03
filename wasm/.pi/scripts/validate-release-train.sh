#!/usr/bin/env bash
# validate-release-train.sh — every publishable workspace member must be on the train.
#
# Why this exists (rigorix-gate-wasm, 2026-11; and rigorix-actions before it):
#
#   release.yml:  for crate in engine cli mcp actions
#   Cargo.toml:   members = ["engine", "cli", "actions", "mcp", "server", "wasm"]
#
# `rigorix-actions` sat on 1.4.0 for seven weeks because it was outside that loop:
# `publish-crate.sh` skipped it as "already published" on every release and nothing
# compared its version to the tag. Adding the missing name fixes that release and
# nothing else — the *literal* cannot notice the next member. So this guard derives
# the train from `cargo metadata` and requires the literal to match:
#
#   1. the `for crate in …` loop names every publishable member's directory;
#   2. every publishable member has a `publish-crate.sh <pkg> <manifest>` invocation;
#   3. all publishable members carry one version (the tag compares against them all);
#   4. the release notes name each publishable crate, so the page cannot advertise a
#      train with a missing car;
#   5. a member with a host-facing npm package has an npm step — and the coupling
#      between readiness and wiring is enforced, not remembered:
#         state: shipped             -> the npm job MUST be in `github-release.needs`
#         state: packaged            -> an artifact a host can load exists (the size ceiling is
#                                       in force), but no version has been published: the job
#                                       MUST exist, MUST NOT be in `needs`, and MUST be gated
#         state: awaiting-packaging  -> no artifact yet: the job MUST exist and MUST NOT be in `needs`
#         anything else              -> fail closed
#
#      `packaged` was added by #1349: an artifact can be callable and measured without
#      having been published, and `shipped` would have claimed the publication.
#      A package that does not exist yet must not look published; a package that does
#      exist must not be quietly excluded from the release.
#
# `server` is a member at 0.1.0 with `publish = false`, so it is correctly outside the
# train. The registry of truth is the manifest, not a list in this file.
#
#   .pi/scripts/validate-release-train.sh                 # check the repo
#   .pi/scripts/validate-release-train.sh --self-test     # prove each check bites
#   .pi/scripts/validate-release-train.sh --root <dir>    # check another tree
#
# An empty scan is a failure: a guard that checks nothing passes everything.

set -euo pipefail

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
MODE=""
ROOT=""

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) MODE="--self-test"; shift ;;
    --root)
      [ $# -ge 2 ] || { echo "validate-release-train: --root requires a directory" >&2; exit 2; }
      ROOT="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,38p' "$SELF" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *)
      echo "validate-release-train: unknown argument '$1'" >&2
      exit 2 ;;
  esac
done

# The guard is mirrored into `wasm/.pi/scripts`, so `../..` is the crate there, not the
# repo. Walk up until the release workflow is found; never guess a path and then report on
# a tree that was never read.
if [ -z "$ROOT" ]; then
  probe="$(dirname "$SELF")"
  while [ "$probe" != "/" ]; do
    if [ -f "$probe/.github/workflows/release.yml" ]; then ROOT="$probe"; break; fi
    probe="$(dirname "$probe")"
  done
fi
if [ -z "$ROOT" ] || [ ! -f "$ROOT/.github/workflows/release.yml" ]; then
  echo "❌ could not locate the repo root (no .github/workflows/release.yml above $(dirname "$SELF"))" >&2
  echo "   a guard that cannot find what it checks does not pass" >&2
  exit 1
fi

CORE=$(cat <<'PY'
import json, os, re, subprocess, sys

root = os.path.abspath(sys.argv[1])
release = os.path.join(root, ".github", "workflows", "release.yml")
findings = []          # (ok: bool, message: str)
scanned = {"members": 0, "publishable": 0, "publishes": 0}

def ok(msg):
    findings.append((True, msg))

def bad(msg):
    findings.append((False, msg))

def rel(path):
    try:
        return os.path.relpath(path, root)
    except ValueError:
        return path

# ── the train, from the manifests ────────────────────────────────────────────
proc = subprocess.run(
    ["cargo", "metadata", "--no-deps", "--format-version", "1",
     "--manifest-path", os.path.join(root, "Cargo.toml")],
    capture_output=True, text=True,
)
if proc.returncode != 0:
    print("  ✗ cargo metadata failed — the member list cannot be read:", file=sys.stderr)
    print("    " + proc.stderr.strip().splitlines()[0] if proc.stderr.strip() else "", file=sys.stderr)
    sys.exit(2)

meta = json.loads(proc.stdout)
members = []
for pkg in meta["packages"]:
    scanned["members"] += 1
    # cargo exposes `publish = false` as []; absent/None means publishable.
    if pkg.get("publish") == []:
        continue
    directory = os.path.dirname(os.path.abspath(pkg["manifest_path"]))
    members.append({
        "dir": os.path.basename(directory) if directory != root else ".",
        "name": pkg["name"],
        "version": pkg["version"],
        "manifest": rel(pkg["manifest_path"]),
    })
scanned["publishable"] = len(members)

if not members:
    print("  ✗ no publishable workspace member found — the scan is broken, not the tree")
    sys.exit(1)

# ── the release workflow ─────────────────────────────────────────────────────
text = open(release, encoding="utf-8").read()
lines = text.splitlines()

loops = []
for m in re.finditer(r"for\s+crate\s+in\s+([^;\n]+?)\s*;\s*do", text):
    loops.append([t for t in m.group(1).split() if t])
if not loops:
    bad("release.yml has no `for crate in …; do` version loop — nothing compares a version to the tag")
    loop_tokens = set()
else:
    loop_tokens = set(loops[0])

publishes = {crate: manifest for crate, manifest in re.findall(
    r"publish-crate\.sh\s+([A-Za-z0-9_.-]+)\s+([A-Za-z0-9_./-]+)", text)}
scanned["publishes"] = len(publishes)

# `github-release`'s `needs:` list — the set of jobs that must succeed before a
# release page is published.
def job_block(name):
    start = None
    for i, line in enumerate(lines):
        if re.match(rf"^  {re.escape(name)}:\s*$", line):
            start = i
            break
    if start is None:
        return None
    end = len(lines)
    for j in range(start + 1, len(lines)):
        if re.match(r"^  [A-Za-z0-9_.-]+:\s*$", lines[j]):
            end = j
            break
    return lines[start:end]

release_block = job_block("github-release")
needs = set()
if release_block is None:
    bad("release.yml has no `github-release` job — the train has no terminus")
else:
    for line in release_block:
        m = re.match(r"^\s+needs:\s*\[(.*)\]\s*$", line)
        if m:
            # YAML flow sequences allow commas, spaces, or both between items:
            # `[a, b]` and `[a b]` are the same list. Splitting on commas alone reads
            # `[a b]` as one job named "a b", so the check silently passes.
            needs = {t for t in re.split(r"[,\s]+", m.group(1)) if t}
            break

# ── 1. every publishable member is named in the loop ─────────────────────────
if loops:
    missing = [m["dir"] for m in members if m["dir"] not in loop_tokens]
    extra = sorted(t for t in loop_tokens if t not in {m["dir"] for m in members})
    if missing:
        for d in missing:
            pkg = next(m for m in members if m["dir"] == d)
            bad(f"`{pkg['name']}` ({d}/) is publishable but release.yml's version loop never names it "
                f"— nothing compares its version to the tag")
    else:
        ok(f"version loop names all {len(members)} publishable member(s): {' '.join(m['dir'] for m in members)}")
    if extra:
        bad(f"version loop names {extra}, which are not publishable workspace members "
            f"— a check against a crate that cannot ship is a false claim")

# ── 2. every publishable member has a publish step ───────────────────────────
missing_pub = [m for m in members if m["name"] not in publishes]
for m in missing_pub:
    bad(f"`{m['name']}` has no `publish-crate.sh {m['name']} {m['manifest']}` step "
        f"— its version can never reach crates.io")
if not missing_pub:
    ok(f"all {len(members)} publishable member(s) have a publish step")

wrong_manifest = [(m, publishes[m["name"]]) for m in members
                  if m["name"] in publishes and os.path.normpath(publishes[m["name"]]) != os.path.normpath(m["manifest"])]
for m, got in wrong_manifest:
    bad(f"`{m['name']}` publishes from {got}, but its manifest is {m['manifest']}")

# ── 3. one version across the train ──────────────────────────────────────────
versions = {m["version"] for m in members}
if len(versions) == 1:
    ok(f"train version is uniform at {versions.pop()}")
else:
    spread = ", ".join(f"{m['name']}={m['version']}" for m in sorted(members, key=lambda x: x["name"]))
    bad(f"train versions differ ({spread}) — a tag can only equal one of them, so the rest "
        f"publish at a version nobody asked for")

# ── 4. the release notes name every crate on the train ───────────────────────
unnamed = [m["name"] for m in members if f"crates.io/crates/{m['name']}" not in text]
for name in unnamed:
    bad(f"the release notes never link `{name}` — the page would advertise a train with a missing car")
if not unnamed:
    ok(f"release notes name all {len(members)} crate(s)")

# ── 5. the host-facing npm package, and its readiness coupling ───────────────
GATE_DIR = "wasm"
NPM_NAME = "rigorix-gate"
BUDGET = os.path.join(root, GATE_DIR, "ci", "bundle-budget.txt")
NPM_JOB = "publish-wasm-npm"
NPM_SCRIPT = ".github/scripts/publish-npm-package.sh"

gate = next((m for m in members if m["dir"] == GATE_DIR), None)
if gate is None:
    ok(f"no host-facing package to register ({GATE_DIR} is not a publishable member)")
else:
    state = None
    if not os.path.isfile(BUDGET):
        bad(f"`{rel(BUDGET)}` is missing, so packaging readiness is unknown — "
            f"the npm wiring cannot be checked and does not pass")
    else:
        for line in open(BUDGET, encoding="utf-8"):
            m = re.match(r"^state:\s*(\S+)\s*$", line)
            if m:
                state = m.group(1)
                break
        if state is None:
            bad(f"`{rel(BUDGET)}` has no `state:` line — readiness must be stated, not inferred")

    invocations = re.findall(rf"publish-npm-package\.sh\s+([A-Za-z0-9_@/.-]+)", text)
    if NPM_NAME not in invocations:
        bad(f"release.yml never runs `publish-npm-package.sh {NPM_NAME} …` — the host-facing "
            f"package {NPM_NAME} has no publish step anywhere in this repository")
    else:
        ok(f"release.yml runs the npm publish step for `{NPM_NAME}`")

    if not os.path.isfile(os.path.join(root, NPM_SCRIPT)):
        bad(f"`{NPM_SCRIPT}` does not exist, so the npm step names a script that cannot run")

    npm_block = job_block(NPM_JOB)
    if npm_block is None:
        bad(f"release.yml has no `{NPM_JOB}` job — the npm step is not a job, so nothing "
            f"can report whether it ran")
    elif state == "shipped":
        # Shippable: the release page must not be published before the package is.
        if NPM_JOB not in needs:
            bad(f"bundle state is `shipped` but `{NPM_JOB}` is not in `github-release.needs` — "
                f"a release page would announce a version that npm does not have")
        else:
            ok(f"bundle state is `shipped` and `{NPM_JOB}` gates the release page")
        if any(re.match(r"^\s+if:\s*", l) for l in npm_block):
            bad(f"bundle state is `shipped` but `{NPM_JOB}` is still conditional on readiness "
                f"— the condition now hides a failure instead of preventing a false one")
    elif state in ("awaiting-packaging", "packaged"):
        if NPM_JOB in needs:
            bad(f"bundle state is `{state}` but `{NPM_JOB}` is in "
                f"`github-release.needs` — no version has been published, so the whole release "
                f"would fail on a package that has not shipped yet")
        else:
            ok(f"bundle state is `{state}` and `{NPM_JOB}` is correctly outside `needs`")
        ready_if = [l for l in npm_block if re.match(r"^\s+if:\s*", l)]
        if not ready_if:
            bad(f"bundle state is `{state}` but `{NPM_JOB}` is unconditional — it would "
                f"run against a package no version has been published from, and fail the tag")
        elif not any("npm_ready" in l for l in ready_if):
            bad(f"`{NPM_JOB}` is conditional, but not on `needs.verify.outputs.npm_ready` — the "
                f"condition must read the committed readiness state, not guess at an artifact")
        else:
            ok(f"`{NPM_JOB}` is gated on `npm_ready` (registered, visibly not published until packaging)")
    elif state is not None:
        bad(f"bundle state `{state}` is not one this guard understands "
            f"(shipped | packaged | awaiting-packaging) "
            f"— an unknown state is not a reason to assume the npm wiring is correct")

# ── verdict ─────────────────────────────────────────────────────────────────
for good, msg in findings:
    print(("  ✅ " if good else "  ✗ ") + msg)

print(f"  scanned {scanned['members']} member(s), {scanned['publishable']} publishable, "
      f"{scanned['publishes']} publish step(s)")
failed = [m for good, m in findings if not good]
if failed:
    print(f"❌ release train: {len(failed)} finding(s)")
    sys.exit(1)
print("✅ release train: every publishable member is named, published, versioned and announced")
PY
)

run_check() { # run_check <root>
  python3 -c "$CORE" "$1"
}

if [ "$MODE" = "--self-test" ]; then
  echo "── validate-release-train --self-test ──"
  TMP=$(mktemp -d)
  fails=0
  passes=0

  # make_tree <dir> <loop> <publish-wasm:yes|no> <state> <needs> <npm-if:yes|no>
  make_tree() {
    local d="$1" loop="$2" pubwasm="$3" state="$4" need="$5" npmif="$6"
    mkdir -p "$d/engine/src" "$d/wasm/src" "$d/server/src" "$d/wasm/ci" "$d/.github/scripts" "$d/.github/workflows"
    printf '[workspace]\nmembers = ["engine", "server", "wasm"]\nresolver = "2"\n' > "$d/Cargo.toml"
    printf '[package]\nname = "rigorix-engine"\nversion = "1.0.0"\nedition = "2021"\n' > "$d/engine/Cargo.toml"
    printf '[package]\nname = "rigorix-gate-wasm"\nversion = "1.0.0"\nedition = "2021"\n' > "$d/wasm/Cargo.toml"
    printf '[package]\nname = "rigorix-server"\nversion = "0.1.0"\nedition = "2021"\npublish = false\n' > "$d/server/Cargo.toml"
    printf '' > "$d/engine/src/lib.rs"; printf '' > "$d/wasm/src/lib.rs"; printf '' > "$d/server/src/lib.rs"
    printf 'artifact: pkg/rigorix_gate_wasm_bg.wasm\nmeasured_bytes: 0\nbudget_bytes: 0\nstate: %s\n' "$state" > "$d/wasm/ci/bundle-budget.txt"
    printf '# npm publisher\n' > "$d/.github/scripts/publish-npm-package.sh"

    {
      echo 'jobs:'
      echo '  verify:'
      echo '    steps:'
      echo '      - run: |'
      echo "          for crate in $loop; do"
      echo '            echo "$crate"'
      echo '          done'
      echo '  publish-engine:'
      echo '    needs: [verify]'
      echo '        run: bash .github/scripts/publish-crate.sh rigorix-engine engine/Cargo.toml'
      if [ "$pubwasm" = "yes" ]; then
        echo '  publish-wasm:'
        echo '    needs: [publish-engine]'
        echo '        run: bash .github/scripts/publish-crate.sh rigorix-gate-wasm wasm/Cargo.toml'
      fi
      echo '  publish-wasm-npm:'
      echo '    needs: [publish-engine]'
      [ "$npmif" = "yes" ] && echo "    if: needs.verify.outputs.npm_ready == 'true'"
      echo '        run: bash .github/scripts/publish-npm-package.sh rigorix-gate wasm/Cargo.toml wasm/pkg'
      echo '  github-release:'
      echo "    needs: [$(printf '%s' "$need" | sed 's/ /, /g')]"
      echo '        run: |'
      echo '          echo "https://crates.io/crates/rigorix-engine"'
      echo '          echo "https://crates.io/crates/rigorix-gate-wasm"'
    } > "$d/.github/workflows/release.yml"
  }

  expect() { # expect <0|1> <label> <dir>
    local want="$1" label="$2" dir="$3" got
    set +e
    run_check "$dir" >"$TMP/out.$$" 2>&1
    got=$?
    set -e
    if [ "$got" -eq "$want" ]; then
      echo "  ✅ $label"
      passes=$((passes + 1))
    else
      echo "  ❌ $label (exit $got, wanted $want)"
      sed 's/^/     /' "$TMP/out.$$"
      fails=$((fails + 1))
    fi
    rm -f "$TMP/out.$$"
  }

  make_tree "$TMP/clean" "engine wasm" yes awaiting-packaging "publish-engine publish-wasm" yes
  expect 0 "registered crate, packaging pending, npm outside needs: passes" "$TMP/clean"

  make_tree "$TMP/noloop" "engine" yes awaiting-packaging "publish-engine publish-wasm" yes
  expect 1 "publishable member missing from the version loop: fails" "$TMP/noloop"

  make_tree "$TMP/nopub" "engine wasm" no awaiting-packaging "publish-engine" yes
  expect 1 "publishable member without a publish step: fails" "$TMP/nopub"

  make_tree "$TMP/skew" "engine wasm" yes awaiting-packaging "publish-engine publish-wasm" yes
  printf '[package]\nname = "rigorix-gate-wasm"\nversion = "0.9.0"\nedition = "2021"\n' > "$TMP/skew/wasm/Cargo.toml"
  expect 1 "train versions differ: fails" "$TMP/skew"

  make_tree "$TMP/notinnotes" "engine wasm" yes awaiting-packaging "publish-engine publish-wasm" yes
  sed -i.bak '/crates.io\/crates\/rigorix-gate-wasm/d' "$TMP/notinnotes/.github/workflows/release.yml"
  expect 1 "release notes omit a crate on the train: fails" "$TMP/notinnotes"

  make_tree "$TMP/shipped-out" "engine wasm" yes shipped "publish-engine publish-wasm" yes
  expect 1 "shipped but npm job outside needs: fails" "$TMP/shipped-out"

  make_tree "$TMP/shipped-in" "engine wasm" yes shipped "publish-engine publish-wasm publish-wasm-npm" no
  expect 0 "shipped and npm job gates the release page: passes" "$TMP/shipped-in"

  make_tree "$TMP/shipped-cond" "engine wasm" yes shipped "publish-engine publish-wasm publish-wasm-npm" yes
  expect 1 "shipped but npm job still conditional: fails" "$TMP/shipped-cond"

  make_tree "$TMP/await-in" "engine wasm" yes awaiting-packaging "publish-engine publish-wasm publish-wasm-npm" yes
  expect 1 "awaiting packaging but npm job in needs: fails" "$TMP/await-in"

  make_tree "$TMP/unknown" "engine wasm" yes measuring "publish-engine publish-wasm" yes
  expect 1 "unknown packaging state: fails closed" "$TMP/unknown"

  make_tree "$TMP/nonpm" "engine wasm" yes awaiting-packaging "publish-engine publish-wasm" yes
  sed -i.bak 's/publish-npm-package\.sh rigorix-gate/publish-npm-package.sh other-name/' "$TMP/nonpm/.github/workflows/release.yml"
  expect 1 "npm step for the wrong package name: fails" "$TMP/nonpm"

  find "$TMP" -type f -delete 2>/dev/null || true
  find "$TMP" -type d -depth -exec rmdir {} + 2>/dev/null || true

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ validate-release-train self-test"
  exit 0
fi

run_check "$ROOT"
