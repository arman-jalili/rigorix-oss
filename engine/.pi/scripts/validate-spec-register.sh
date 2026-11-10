#!/usr/bin/env bash
# validate-spec-register.sh — one register, and everything else derived from it.
#
# `docs/spec/specs.json` is the only place a version is written. The specification index in
# `docs/spec/README.md` and the site's navigation are generated from it, because a version
# typed into a table, a nav list and a page header is three versions that drift — the defect
# this project keeps finding in its own records (#1508, #1509).
#
# Rules:
#   1. every path the register names exists, and every `.md` under docs/spec is named by it
#   2. a gate has either a `spec` (its own document) or a `documented_in` (a section of
#      another document) — never neither, never both
#   3. `documented_in.section` names a heading that exists in that file
#   4. one `current` version per document id; a `superseded` row names the version that
#      replaced it, which must exist and be `current`
#   5. the gate set is the dispatcher's: preconditions, requirements, capacity, effects,
#      approval — a gate the code asks and the register does not list is a gap
#   6. the index table in docs/spec/README.md equals what the register says
#
# Usage:
#   bash engine/.pi/scripts/validate-spec-register.sh            # check
#   bash engine/.pi/scripts/validate-spec-register.sh --write    # regenerate the README index
#   bash engine/.pi/scripts/validate-spec-register.sh --self-test
#
# Exit: 0 = the register and the tree agree; 1 = they do not; 2 = malformed input.

set -uo pipefail
cd "$(dirname "$0")/../../.." || exit 2

exec python3 - "$@" <<'PY'
import json
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path.cwd()
SPEC = ROOT / "docs/spec"
REGISTER = SPEC / "specs.json"
README = SPEC / "README.md"
GATE_IDS = ["preconditions", "requirements", "capacity", "effects", "approval"]

failures: list[str] = []


def load(register: pathlib.Path) -> dict:
    try:
        return json.loads(register.read_text())
    except Exception as exc:  # noqa: BLE001
        print(f"validate-spec-register: {register}: {exc}", file=sys.stderr)
        raise SystemExit(2)


def gate_paths(reg: dict) -> list[pathlib.Path]:
    out = []
    for gate in reg.get("gates", []):
        spec = gate.get("spec")
        if spec:
            out.append(SPEC / spec["path"])
        documented = gate.get("documented_in")
        if documented:
            out.append(SPEC / documented["path"])
    for doc in reg.get("documents", []):
        out.append(SPEC / doc["path"])
    return out


def check(reg: dict) -> list[str]:
    failures.clear()

    gates = reg.get("gates", [])
    docs = reg.get("documents", [])

    # 5. the gate set is the code's, not a subset of it
    seen = [g.get("id") for g in gates]
    for missing in [g for g in GATE_IDS if g not in seen]:
        failures.append(f"register: the gate '{missing}' is asked by the dispatcher and missing here")
    for extra in [g for g in seen if g not in GATE_IDS]:
        failures.append(f"register: '{extra}' is not a gate the dispatcher asks")

    # 2. a gate is documented somewhere, and the two forms are exclusive
    for gate in gates:
        has_spec, has_doc = gate.get("spec"), gate.get("documented_in")
        if has_spec and has_doc:
            failures.append(f"gate {gate['id']}: both a `spec` and a `documented_in` — pick one")
        if not has_spec and not has_doc:
            failures.append(f"gate {gate['id']}: neither a `spec` nor a `documented_in` — the question is unanswerable from the specification")

    # 1. every named path exists, and nothing under docs/spec is unregistered
    named = set()
    for path in gate_paths(reg):
        rel = path.relative_to(SPEC).as_posix()
        named.add(rel)
        if not path.exists():
            failures.append(f"register names {rel}, which does not exist")
    # A `directory: true` entry is one document set: it presents every markdown file
    # beside it, so the set's members do not each need a row of their own.
    for doc in reg.get("documents", []):
        if doc.get("directory"):
            base = (SPEC / doc["path"]).parent
            for member in base.glob("*.md"):
                named.add(member.relative_to(SPEC).as_posix())
    for path in sorted(SPEC.rglob("*.md")):
        rel = path.relative_to(SPEC).as_posix()
        if rel == "README.md" or rel == "execution-governance/README.md":
            continue
        if rel not in named:
            failures.append(f"{rel} exists and no register entry names it — a document nothing presents")

    # 3. a `documented_in` section must exist
    for gate in gates:
        documented = gate.get("documented_in")
        if not documented:
            continue
        path = SPEC / documented["path"]
        section = documented.get("section", "")
        number = section.lstrip("§")
        if path.exists() and not re.search(rf"^##\s+{re.escape(number)}\.", path.read_text(), re.M):
            failures.append(f"gate {gate['id']}: {documented['path']} has no '## {number}.' heading")

    # 4. versions: one current per id, and a superseded row points at a current one
    by_id: dict[str, list[dict]] = {}
    for doc in [d for g in gates if (d := g.get("spec"))] + docs:
        by_id.setdefault(doc["path"].split("-v")[0], []).append(doc)
    for key, rows in by_id.items():
        current = [r for r in rows if r.get("status") == "current"]
        if len(current) > 1:
            failures.append(f"{key}: {len(current)} documents are 'current' — a presentation can show only one")
        for row in rows:
            if row.get("status") == "superseded":
                successor = row.get("superseded_by")
                if not successor:
                    failures.append(f"{row['path']}: superseded, but no `superseded_by`")
                elif not any(r["path"] == successor and r.get("status") == "current" for r in rows):
                    failures.append(f"{row['path']}: `superseded_by` {successor} is not a current version of this document")

    # 6. the README index equals the register
    expected = index_rows(reg)
    actual = readme_rows(README.read_text()) if README.exists() else []
    if expected != actual:
        failures.append(
            "docs/spec/README.md index differs from the register "
            f"({len(actual)} row(s) in the file, {len(expected)} in specs.json) — run --write"
        )

    return list(failures)


def index_rows(reg: dict) -> list[tuple[str, str, str, str]]:
    rows = []
    for gate in reg.get("gates", []):
        spec = gate.get("spec")
        documented = gate.get("documented_in")
        if spec:
            link = f"[{gate['title']}](./{spec['path']})"
            status = spec["status"]
            version, date = spec["version"], spec["date"]
        else:
            link = f"{gate['title']}"
            status = f"in `{documented['path']}` {documented['section']}"
            version, date = "—", "—"
        rows.append((link, version, date, status))
    for doc in reg.get("documents", []):
        rows.append((f"[{doc['title']}](./{doc['path']})", doc["version"], doc["date"], doc["status"]))
    return rows


def readme_rows(text: str) -> list[tuple[str, str, str, str]]:
    """Parse the generated table back. The order is part of the contract."""
    rows = []
    for line in text.splitlines():
        m = re.match(r"^\|\s*(.+?)\s*\|\s*(.+?)\s*\|\s*(.+?)\s*\|\s*(.+?)\s*\|$", line)
        if not m or m.group(1) in ("Specification", "---", "-----------"):
            continue
        if set(m.group(1)) <= {"-", " "}:
            continue
        rows.append((m.group(1), m.group(2), m.group(3), m.group(4)))
    return rows


def render_index(reg: dict) -> str:
    lines = [
        "| Specification | Version | Date | Status |",
        "|---------------|---------|------|--------|",
    ]
    for link, version, date, status in index_rows(reg):
        lines.append(f"| {link} | {version} | {date} | {status} |")
    return "\n".join(lines)


INDEX_START = "<!-- index: generated from specs.json by engine/.pi/scripts/validate-spec-register.sh — do not edit by hand -->"
INDEX_END = "<!-- /index -->"


def write_index(reg: dict) -> None:
    text = README.read_text()
    block = f"{INDEX_START}\n{render_index(reg)}\n{INDEX_END}"
    if INDEX_START in text:
        text = re.sub(re.escape(INDEX_START) + r".*?" + re.escape(INDEX_END), block, text, flags=re.S)
    else:
        text = re.sub(r"(\| Specification \| Version \| Date \| Status \|\n\|[-| ]+\|\n(?:\|.*\n)+)", block + "\n", text)
    README.write_text(text)
    print("spec register: index written")


# ── self-test ────────────────────────────────────────────────────────────────
def self_test() -> int:
    cases = []

    def case(label, mutate, expect):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp) / "docs/spec"
            (root / "execution-governance").mkdir(parents=True)
            (root / "execution-governance/03-requirements.md").write_text(
                "## B. Approval\n\n## D. Requirements\n\n## E. Capacity\n\n## J. Commitment\n"
            )
            (root / "consequence-gating-v1.md").write_text("# Consequence Gating v1.0\n")
            (root / "terminology.md").write_text("# Terminology\n")
            (root / "validation-protocol-v1.md").write_text("# Protocol\n")
            (root / "execution-governance/README.md").write_text("# Execution Governance\n")

            reg = json.loads(REGISTER.read_text())
            readme = root / "README.md"
            # The index is written from the *pristine* register, so a mutation that changes
            # a version shows up as drift instead of being copied into the file it drifts from.
            readme.write_text(f"# Spec\n\n{INDEX_START}\n{render_index(reg)}\n{INDEX_END}\n")

            global SPEC, README
            saved = (SPEC, README)
            SPEC, README = root, readme
            try:
                mutate(reg)
                (root / "specs.json").write_text(json.dumps(reg))
                problems = check(json.loads((root / "specs.json").read_text()))
            finally:
                SPEC, README = saved
            cases.append((label, bool(problems) == expect, expect, problems))

    def drop_gate(reg):
        reg["gates"] = [g for g in reg["gates"] if g["id"] != "capacity"]

    def two_current(reg):
        reg["documents"].append(
            {
                "id": "terminology",
                "title": "Terminology",
                "identifier": "rigorix-consequence-gating-terminology",
                "path": "terminology-v2.md",
                "version": "2.0",
                "date": "2026-11-10",
                "status": "current",
            }
        )
        (SPEC / "terminology-v2.md").write_text("# Terminology v2.0\n")

    def superseded_without_successor(reg):
        reg["documents"][0]["status"] = "superseded"

    def index_drift(reg):
        reg["documents"][0]["version"] = "9.9"

    def missing_section(reg):
        reg["gates"][1]["documented_in"]["section"] = "§Z"

    case("a clean register passes", lambda reg: None, False)
    case("a gate the dispatcher asks, missing from the register", drop_gate, True)
    case("two current versions of one document", two_current, True)
    case("superseded with no successor", superseded_without_successor, True)
    case("the README index contradicts the register", index_drift, True)
    case("documented_in points at a heading that is not there", missing_section, True)

    rejected = sum(1 for _l, ok, expect, _p in cases if ok and expect)
    controls = sum(1 for _l, ok, expect, _p in cases if ok and not expect)
    failed = [(l, p) for l, ok, _e, p in cases if not ok]
    for label, _ok, _expect, problems in cases:
        if (label, problems) not in failed:
            print(f"  ✓ {label}")
        else:
            print(f"  ✗ {label}")
            for problem in problems:
                print(f"      {problem}")
    for label, problems in failed:
        if not problems:
            print(f"      {label}: the case passed for the wrong reason — it produced no finding", file=sys.stderr)

    expected_rejected = sum(1 for _l, _ok, expect, _p in cases if expect)
    if rejected != expected_rejected or controls != 1 or failed:
        print(f"self-test: {rejected}/{expected_rejected} planted contradictions rejected, "
              f"{controls} control accepted, {len(failed)} case(s) wrong", file=sys.stderr)
        return 1
    print(f"✅ spec-register self-test: {rejected} planted contradiction(s) rejected, {controls} control accepted")
    return 0


args = sys.argv[1:]
if "--self-test" in args:
    raise SystemExit(self_test())

register = load(REGISTER)
if "--write" in args:
    write_index(register)

problems = check(register)
if problems:
    print(f"❌ spec register: {len(problems)} contradiction(s):", file=sys.stderr)
    for problem in problems:
        print(f"  - {problem}", file=sys.stderr)
    raise SystemExit(1)

gates = register.get("gates", [])
with_spec = sum(1 for g in gates if g.get("spec"))
print(f"✅ spec register: {len(gates)} gates ({with_spec} with their own document), "
      f"{len(register.get('documents', []))} document(s), index generated from one file")
PY
