#!/usr/bin/env bash
# ============================================================================
# validate-sink-register.sh — the register, and the three class rules
# ============================================================================
# SINK-3 (#1481). Three external findings, three different bugs, one shape: a
# control is present and is not the control the sentence claims.
#
#   * run-command filtered, and the isolation it relies on was a source comment;
#   * file-write validated a path, then re-resolved it before mutating;
#   * the evidence path computed a result and dropped it.
#
# A reviewer finds those by reading. This guard is the mechanism instead: every
# boundary crossing in engine/src must be a row in
# engine/.pi/architecture/sink-register.tsv saying what enforces it, what it
# relies on, and — the column nobody was forced to write — what it does NOT do.
#
# Rules enforced here:
#
#   1. Every sink hit maps to a registered row. A `Command::new`, `fs::write`,
#      `TcpStream::connect` or `env::var` in a module with no row fails, naming
#      file:line and the kind.
#   2. A row is honest. `does_not` is mandatory; `mitigation` comes from a fixed
#      vocabulary; `mitigation=filtering` must name the deployment fact it
#      relies on (filtering is never recorded as isolation — the value
#      `isolation` is refused by name); an `fs-write` row mitigated by `type`
#      must name the handle it depends on.
#   3. No evidence result is dropped. A bare `let _ =` under audit/,
#      orchestrator/ or precondition/ fails unless the line carries
#      `// sink: <id>` and that id is a registered row.
#   5. Every call site names the row that governs it (`// sink: <id>` on or
#      above the line). A row is written about a call site, and one row cannot
#      describe five call sites with three different guarantees — that is how
#      four raw writes stayed unregistered under one `atomic` row (#1494).
#      Requiring it everywhere, rather than only when a module already has
#      two, is what makes a *new* sink in a registered file fail: the row it
#      would inherit was written about the site that was there.
#
# Usage:
#   bash engine/.pi/scripts/validate-sink-register.sh             # check
#   bash engine/.pi/scripts/validate-sink-register.sh --report    # coverage
#   bash engine/.pi/scripts/validate-sink-register.sh --list      # the rows
#   bash engine/.pi/scripts/validate-sink-register.sh --self-test # prove it bites
#
# Exit codes: 0 = register matches the code; 1 = a violation; 2 = malformed input
# ============================================================================
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TREE_ROOT="$(cd "${SELF_DIR}/../.." && pwd)"   # <repo>/engine

REG="${TREE_ROOT}/.pi/architecture/sink-register.tsv"
SRC="${TREE_ROOT}/src"

MODE="${1:-check}"

if [ "$MODE" = "--help" ] || [ "$MODE" = "-h" ]; then
	sed -n '2,45p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
	exit 0
fi

check() { # $1 register, $2 src dir, $3 mode
	python3 - "$1" "$2" "$3" <<'PY'
import os, re, sys

reg_path, src_dir, mode = sys.argv[1:4]

RED, GREEN, YELLOW, DIM, NC = "\033[0;31m", "\033[0;32m", "\033[1;33m", "\033[2m", "\033[0m"

# --- the sink patterns, and the kind each one is registered under -------------
PATTERNS = [
    ("process", re.compile(r"Command::new\(")),
    ("network", re.compile(r"TcpStream::connect\(|TcpListener::bind\(|reqwest::Client::new\(")),
    ("fs-write", re.compile(r"fs::(?:write|rename|create_dir_all|remove_file|remove_dir_all)\(")),
    ("env-read", re.compile(r"env::var\(")),
]

MITIGATIONS = {
    "filtering",       # a matching decision (allowlist, operator refusal)
    "type",            # the type system refuses the wrong use
    "handle",          # an open handle pins what is mutated
    "atomic",          # temp + fsync + rename
    "retry+queue",     # redelivery through a queue
    "env-clear",       # the child's environment is constructed, not inherited
    "trust-boundary",  # a verified principal/authority gate
    "operator-declared",  # the deployment states the fact
    "none",            # deliberately unmitigated, with a reason in does_not
}

# Paths whose sinks are not production boundaries: test code is excluded, and so
# is everything after the first `#[cfg(test)]` in a file (the convention here is
# that the test module is last).
TEST_PATH = re.compile(r"(^|/)(tests?)/|_tests?\.rs$")

TEST_CFG = re.compile(r"^#\[cfg\([^]]*\btest\b[^]]*\)\]")

# Rule 4: an unmitigated row cites the issue that owns the fix.
ISSUE_REF = re.compile(r"#\d{3,}")

def test_boundary(lines):
    """Index of the first `#[cfg(... test ...)]` line: the test code starts there.

    Both `#[cfg(test)] mod tests` and `#[cfg(all(test, unix))] mod ...` occur
    here; a test-only harness is not a production sink, and registering one would
    be a row nobody can act on.
    """
    for i, line in enumerate(lines):
        if TEST_CFG.match(line.strip()):
            return i
    return len(lines)

def load_register(path):
    rows, errors = [], []
    if not os.path.exists(path):
        return rows, [f"missing register: {path}"]
    for lineno, line in enumerate(open(path, encoding="utf-8"), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        parts = line.rstrip("\n").split("\t")
        if len(parts) != 6:
            errors.append(f"{path}:{lineno}: need 6 tab-separated columns "
                          f"(sink, module, kind, mitigation, requires, does_not), got {len(parts)}")
            continue
        sink, module, kind, mitigation, requires, does_not = parts
        row = dict(sink=sink.strip(), module=module.strip(), kind=kind.strip(),
                   mitigation=mitigation.strip(), requires=requires.strip(),
                   does_not=does_not.strip(), lineno=lineno)
        rows.append(row)
    return rows, errors

def check_row(row, path, src_dir=None):
    """Rule 2: the row has to be honest."""
    errors = []
    where = f"{path}:{row['lineno']}"
    for field in ("sink", "module", "kind", "mitigation"):
        if not row[field]:
            errors.append(f"{where}: empty {field}")
    if len(row["does_not"]) < 12:
        errors.append(f"{where} ({row['sink']}): `does_not` is mandatory — the sentence that would "
                      f"have prevented all three findings is the one nobody was forced to write")
    if row["mitigation"] not in MITIGATIONS:
        if row["mitigation"] == "isolation":
            errors.append(f"{where} ({row['sink']}): `isolation` is not a mitigation — isolation is a "
                          f"deployment property. Record the mechanism in `mitigation` and the fact "
                          f"it relies on in `requires` (rule 1: filtering is never recorded as isolation)")
        else:
            errors.append(f"{where} ({row['sink']}): mitigation {row['mitigation']!r} is not in the "
                          f"vocabulary {sorted(MITIGATIONS)}")
    if row["mitigation"] == "filtering" and not row["requires"]:
        errors.append(f"{where} ({row['sink']}): a filtering row must name a `requires` — the "
                      f"deployment fact it depends on")
    if src_dir and row["module"] and not os.path.exists(os.path.join(src_dir, row["module"])):
        errors.append(f"{where} ({row['sink']}): the named module does not exist — a row that "
                      f"points at nothing records nothing")
    if row["kind"] == "fs-write" and row["mitigation"] == "type" and "handle" not in row["requires"]:
        errors.append(f"{where} ({row['sink']}): an fs-write row mitigated by a type must name the "
                      f"handle in `requires` (validation and mutation share one handle, #1479)")
    if row["mitigation"] == "none":
        # Rule 4 (#1490): a description is not a ticket.
        #
        # `mitigation=none` is a finding, and a finding with no owner becomes
        # permanent: the register did its job by naming these three, and nothing was
        # tracking the fix until the issue numbers were required.
        if not ISSUE_REF.search(row["requires"]):
            errors.append(f"{where} ({row['sink']}): mitigation `none` must name the open issue "
                          f"that owns the fix in `requires` (e.g. `open issue #1490`) — a row "
                          f"without an owner is how a gap becomes permanent")
    return errors

def matches(module, path):
    """`module` is a file or directory prefix of `path`, at a component boundary."""
    if path == module:
        return True
    base = module.rstrip("/")
    return path.startswith(base + "/")

def scan(src_dir):
    hits = []
    for dirpath, dirnames, files in os.walk(src_dir):
        dirnames.sort()
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, src_dir).replace(os.sep, "/")
            if TEST_PATH.search(rel):
                continue
            try:
                lines = open(full, encoding="utf-8", errors="replace").read().splitlines()
            except OSError:
                continue
            cut = test_boundary(lines)
            for i, line in enumerate(lines[:cut], 1):
                stripped = line.strip()
                if stripped.startswith("//"):
                    continue
                for kind, pattern in PATTERNS:
                    if pattern.search(line):
                        # The anchor may be on the same line or the two above it: the
                        # comment explains the line, so it is written next to it.
                        claim = None
                        for probe in (i, i - 1, i - 2):
                            if probe < 1:
                                continue
                            m = CLAIM.search(lines[probe - 1])
                            if m:
                                claim = m.group(1)
                                break
                        hits.append(
                            dict(file=rel, line=i, kind=kind, text=stripped[:120],
                                 claim=claim,
                                 function=enclosing_function(lines[:cut], i))
                        )
                        break
    return hits

FUNCTION = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)")
CLAIM = re.compile(r"//\s*sink:\s*([\w:.-]+)")

def enclosing_function(lines, lineno):
    """The nearest `fn` above a hit: the branch a call site belongs to (#1494)."""
    for i in range(min(lineno, len(lines)) - 1, -1, -1):
        m = FUNCTION.match(lines[i])
        if m:
            return m.group(1)
    return "<file scope>"

def scan_dropped_evidence(src_dir):
    """Rule 3: `let _ =` under the evidence paths, unless it names a sink."""
    roots = ("audit/", "orchestrator/", "precondition/")
    findings = []
    for dirpath, dirnames, files in os.walk(src_dir):
        dirnames.sort()
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, src_dir).replace(os.sep, "/")
            if not rel.startswith(roots) or TEST_PATH.search(rel):
                continue
            try:
                lines = open(full, encoding="utf-8", errors="replace").read().splitlines()
            except OSError:
                continue
            cut = test_boundary(lines)
            for i, line in enumerate(lines[:cut], 1):
                stripped = line.strip()
                if not re.match(r"let _ =", stripped):
                    continue
                context = "\n".join(lines[max(0, i - 3):i + 1])
                if re.search(r"//\s*sink:\s*[\w:.-]+", context):
                    continue
                findings.append((rel, i, stripped[:120]))
    return findings

rows, errors = load_register(reg_path)
reg_errors = list(errors)
for row in rows:
    reg_errors.extend(check_row(row, reg_path, src_dir))

sink_ids = {row["sink"] for row in rows}

hits = scan(src_dir)
unregistered, registered, unclaimed = [], [], []

# Rule 5 (#1494): a module with more than one sink of a kind cannot be described by
# one row, so each of its call sites names the row that governs it.
by_module = {}
for hit in hits:
    by_module.setdefault((hit["file"], hit["kind"]), []).append(hit)

for (rel, kind), group in by_module.items():
    candidates = [r for r in rows if r["kind"] == kind and matches(r["module"], rel)]
    if not candidates:
        for hit in group:
            unregistered.append((hit["file"], hit["line"], kind, hit["text"]))
        continue
    for hit in group:
        claim = hit["claim"]
        row = next((r for r in rows if r["sink"] == claim), None) if claim else None
        if row is None or row["kind"] != kind or not matches(row["module"], rel):
            unclaimed.append(hit)
            continue
        registered.append((rel, hit["line"], kind))

dropped = scan_dropped_evidence(src_dir)

if mode in ("--list", "list"):
    print(f"{len(rows)} registered sink(s):")
    for row in rows:
        print(f"  {row['sink']:<42} {row['kind']:<9} {row['mitigation']:<18} {row['module']}")
    print(f"\n{len(registered)} production hit(s) map to a row; {len(unregistered)} do not.")
    sys.exit(0)

if mode in ("--report", "report"):
    print(f"register: {reg_path}")
    print(f"source:   {src_dir}\n")
    for kind, _ in PATTERNS:
        kind_rows = [r for r in rows if r["kind"] == kind]
        kind_hits = [h for h in registered if h[2] == kind]
        print(f"  {kind:<9} {len(kind_rows):>3} row(s)  {len(kind_hits):>3} registered hit(s)")
    print()
    for rel, lineno, kind, text in unregistered:
        print(f"  UNREGISTERED  {rel}:{lineno}  [{kind}]  {text}")
    for hit in unclaimed:
        print(f"  UNCLAIMED     {hit['file']}:{hit['line']}  [{hit['kind']}]  "
              f"{hit['function']}()  claim={hit['claim']}")
    for rel, lineno, text in dropped:
        print(f"  DROPPED       {rel}:{lineno}  {text}")
    if not unregistered and not unclaimed and not dropped and not reg_errors:
        print("  register matches the code.")
        sys.exit(0)
    sys.exit(1)

failures = []
failures.extend(reg_errors)
for rel, lineno, kind, text in unregistered:
    failures.append(
        f"engine/src/{rel}:{lineno}: unregistered {kind} sink — {text}\n"
        f"    add a row to {os.path.basename(reg_path)}, or route the call through a "
        f"module that already has one"
    )
for hit in unclaimed:
    where = f"engine/src/{hit['file']}:{hit['line']}"
    if hit["claim"] is None:
        failures.append(
            f"{where}: unclaimed {hit['kind']} sink in {hit['function']}() — {hit['text']}\n"
            f"    every call site names the row that governs it (#1494): one row cannot state what\n"
            f"    several sites do, and a site that inherits a row written about another site is\n"
            f"    how an unguarded write stayed registered as `atomic`. Write `// sink: <id>` on or\n"
            f"    above the line — and split the row if the guarantees differ"
        )
    else:
        failures.append(
            f"{where}: `// sink: {hit['claim']}` does not name a {hit['kind']} row whose module\n"
            f"    contains {hit['file']} — an anchor that points at another call site's row\n"
            f"    describes nothing"
        )
for rel, lineno, text in dropped:
    failures.append(
        f"engine/src/{rel}:{lineno}: dropped result on the evidence path — {text}\n"
        f"    consume the result, or carry `// sink: <id>` naming a registered row "
        f"whose `does_not` column states the loss"
    )
# An excuse that names nothing is not an excuse.
for dirpath, _dirnames, files in os.walk(src_dir):
    for name in files:
        if not name.endswith(".rs"):
            continue
        full = os.path.join(dirpath, name)
        try:
            lines = open(full, encoding="utf-8", errors="replace").read().splitlines()
        except OSError:
            continue
        for i, line in enumerate(lines, 1):
            m = re.search(r"//\s*sink:\s*([\w:.-]+)", line)
            if m and m.group(1) not in sink_ids:
                rel = os.path.relpath(full, src_dir)
                failures.append(
                    f"engine/src/{rel}:{i}: `// sink: {m.group(1)}` names no registered row"
                )

if failures:
    print(f"{RED}✗ sink register: {len(failures)} finding(s){NC}\n", file=sys.stderr)
    for failure in failures:
        print(f"  {failure}\n", file=sys.stderr)
    sys.exit(1)

print(f"{GREEN}✅ sink register: {len(rows)} row(s), {len(registered)} production hit(s), "
      f"all registered, claimed and honest{NC}")
print(f"{DIM}   every row states what it does not do; every boundary crossing has a row{NC}")
PY
}

if [ "$MODE" = "--self-test" ] || [ "$MODE" = "self-test" ]; then
	echo "── sink register self-tests (each seed must be rejected) ──"
	WORK="$(mktemp -d)"
	trap 'rm -rf "$WORK"' EXIT
	failures=0

	seed() { # $1 name, $2 expected-fragment, then: files are written by caller
		local name="$1" expect="$2" out
		set +e
		out="$(check "$WORK/reg.tsv" "$WORK/src" check 2>&1)"
		local rc=$?
		set -e
		if [ $rc -eq 0 ]; then
			echo "  ✗ $name — the guard accepted it"
			failures=$((failures + 1))
		elif ! grep -q "$expect" <<<"$out"; then
			echo "  ✗ $name — rejected, but the message does not name it ($expect)"
			echo "$out" | sed 's/^/      /'
			failures=$((failures + 1))
		else
			echo "  ✓ $name"
		fi
	}

	reset_fixture() {
		rm -rf "$WORK/src" "$WORK/reg.tsv"
		mkdir -p "$WORK/src/tools/application"
	}

	# --- seed A: an unregistered Command::new -------------------------------
	reset_fixture
	cat >"$WORK/src/tools/application/new_sink.rs" <<'RS'
pub fn spawn() {
    let _child = std::process::Command::new("x").arg("y");
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
# sink	module	kind	mitigation	requires	does_not
tools:run-command-spawn	tools/application/run_command_tool.rs	process	filtering	deployment isolation (RIGORIX_SANDBOX)	does not constrain what the executable does once started
TSV
	seed "an unregistered Command::new is refused" "new_sink.rs:2"

	# --- seed B: filtering recorded without the fact it depends on ---------
	reset_fixture
	cat >"$WORK/src/tools/application/run_command_tool.rs" <<'RS'
pub fn f() {
    let _child = std::process::Command::new("git");
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
tools:run-command-spawn	tools/application/run_command_tool.rs	process	filtering		does not constrain what the executable does once started
TSV
	seed "a filtering row without a requires is refused" "must name a .requires."

	# --- seed C: filtering recorded as isolation ---------------------------
	reset_fixture
	cat >"$WORK/src/tools/application/run_command_tool.rs" <<'RS'
pub fn f() {
    let _child = std::process::Command::new("git");
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
tools:run-command-spawn	tools/application/run_command_tool.rs	process	isolation		does not constrain what the executable does once started
TSV
	seed "a row claiming isolation is refused" "is not a mitigation"

	# --- seed D: a dropped evidence result ---------------------------------
	reset_fixture
	mkdir -p "$WORK/src/audit/application"
	cat >"$WORK/src/audit/application/service.rs" <<'RS'
pub async fn f(service: &Service) {
    let _ = service.build_and_send().await;
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
audit:local-chain	audit/application/service.rs	fs-write	atomic	atomic replace by the store	does not survive a full disk
TSV
	seed "a dropped evidence result is refused" "dropped result on the evidence path"

	# --- seed E: the same line WITH an excuse naming a registered row ------
	cat >"$WORK/src/audit/application/service.rs" <<'RS'
pub async fn f(service: &Service) {
    // sink: audit:local-chain — the loss is stated in the register
    let _ = service.build_and_send().await;
}
RS
	set +e
	out="$(check "$WORK/reg.tsv" "$WORK/src" check 2>&1)"
	rc=$?
	set -e
	if [ $rc -eq 0 ]; then
		echo "  ✓ a dropped result with a registered \`// sink:\` excuse is accepted"
	else
		echo "  ✗ a legitimate excuse was rejected:"
		echo "$out" | sed 's/^/      /'
		failures=$((failures + 1))
	fi

	# --- seed F: a row without does_not -----------------------------------
	reset_fixture
	cat >"$WORK/src/tools/application/run_command_tool.rs" <<'RS'
pub fn f() {
    let _child = std::process::Command::new("git");
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
tools:run-command-spawn	tools/application/run_command_tool.rs	process	filtering	deployment isolation (RIGORIX_SANDBOX)	none
TSV
	seed "a row without a real does_not is refused" "does_not. is mandatory"

	# --- seed G: an unmitigated row with no owner -------------------------
	#
	# #1490: the register named three `none` rows and nothing was tracking the
	# fix. A finding without an owner becomes permanent, so the row must cite the
	# issue that owns it.
	reset_fixture
	cat >"$WORK/src/tools/application/run_command_tool.rs" <<'RS'
pub fn f() {
    let _child = std::process::Command::new("git");
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
tools:run-command-spawn	tools/application/run_command_tool.rs	process	none	—	does not execute argv: the string still reaches the shell
TSV
	seed "an unmitigated row with no owning issue is refused" "must name the open issue"

	# --- seed H: a second call site under one row (#1494) ------------------
	#
	# The measured defect: one `atomic` row for a file with five write sites, of
	# which four were raw path writes. A row is about a call site, so a module with
	# two sinks of a kind must name each of them.
	reset_fixture
	cat >"$WORK/src/tools/application/store.rs" <<'RS'
pub fn save(path: &str, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}

pub fn append(path: &str, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
tools:store-write	tools/application/store.rs	fs-write	atomic	fs_at::write_atomic — temp file, fsync, rename	in the parent follows links above the final component
TSV
	seed "a second, unclaimed call site in one module is refused" "unclaimed fs-write sink"

	# --- seed I: the anchor points at a row that does not govern the line --
	reset_fixture
	cat >"$WORK/src/tools/application/store.rs" <<'RS'
pub fn save(path: &str, bytes: &[u8]) {
    // sink: tools:other-write
    std::fs::write(path, bytes).unwrap();
}

pub fn append(path: &str, bytes: &[u8]) {
    // sink: tools:other-write
    std::fs::write(path, bytes).unwrap();
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
tools:store-write	tools/application/store.rs	fs-write	atomic	fs_at::write_atomic — temp file, fsync, rename	in the parent follows links above the final component
tools:other-write	tools/application/other.rs	fs-write	atomic	fs_at::write_atomic — temp file, fsync, rename	in the parent follows links above the final component
TSV
	mkdir -p "$WORK/src/tools/application"
	cat >"$WORK/src/tools/application/other.rs" <<'RS'
pub fn f(path: &str, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}
RS
	seed "an anchor naming another module's row is refused" "does not name a fs-write row whose module"

	# --- the control: claiming both sites passes --------------------------
	reset_fixture
	cat >"$WORK/src/tools/application/store.rs" <<'RS'
pub fn save(path: &str, bytes: &[u8]) {
    // sink: tools:store-write
    std::fs::write(path, bytes).unwrap();
}

pub fn append(path: &str, bytes: &[u8]) {
    // sink: tools:store-write
    std::fs::write(path, bytes).unwrap();
}
RS
	cat >"$WORK/reg.tsv" <<'TSV'
tools:store-write	tools/application/store.rs	fs-write	atomic	fs_at::write_atomic — temp file, fsync, rename	in the parent follows links above the final component
TSV
	set +e
	out="$(check "$WORK/reg.tsv" "$WORK/src" check 2>&1)"
	rc=$?
	set -e
	if [ $rc -eq 0 ]; then
		echo "  ✓ two call sites claiming one row are accepted when the row governs both"
	else
		echo "  ✗ a legitimately claimed pair was rejected:"
		echo "$out" | sed 's/^/      /'
		failures=$((failures + 1))
	fi

	if [ "$failures" -ne 0 ]; then
		echo "✗ $failures self-test(s) failed"
		exit 1
	fi
	echo "✅ every seed was rejected and the control case passed"
	exit 0
fi

if [ ! -d "$SRC" ]; then
	echo "no engine/src at $SRC — cannot validate the register" >&2
	exit 2
fi

check "$REG" "$SRC" "$MODE"
