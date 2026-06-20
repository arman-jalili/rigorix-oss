#!/usr/bin/env python3
"""Validate planning packet structure and core gating rules.

Deterministic validator for epic planning packets produced by the
Architecture Coordinator. Checks required sections, valid values, and
structural consistency before the packet is handed off for issue generation.

Usage:
    python scripts/ci/check_planning_packet.py --input=planning_packet.md
    python scripts/ci/check_planning_packet.py --input=planning_packet.md --json

Exit codes:
    0 - Packet is valid
    1 - Validation errors found
    2 - Script error
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


REQUIRED_SECTIONS = [
    "## Stream classification",
    "## Scope summary",
    "## In scope",
    "## Out of scope",
    "## Impacted layers",
    "## Risk classification",
    "## Dependency graph",
    "## Mandatory validators",
    "## Mandatory CI gates",
    "## Forbidden shortcuts",
    "## First implementation issue recommendation",
    "## Open questions / escalation items",
]

ALLOWED_STREAMS = {"feature", "hardening", "migration", "control"}
ALLOWED_RISKS = {"low", "medium", "high"}
ALLOWED_CI_GATES = {
    "docs_policy", "architecture_conformance", "lint", "static_analysis",
    "unit", "integration", "security", "migration_verify", "release_readiness",
}
REQUIRED_VALIDATORS = {"architecture-validator"}


def extract_section(content: str, heading: str) -> str:
    pattern = re.compile(rf"^{re.escape(heading)}\s*$", re.MULTILINE)
    match = pattern.search(content)
    if not match:
        return ""
    start = match.end()
    next_heading = re.search(r"^##\s+", content[start:], re.MULTILINE)
    end = start + next_heading.start() if next_heading else len(content)
    return content[start:end].strip()


def contains_list_item(section: str) -> bool:
    return bool(re.search(r"^[-*]\s+\S|^\d+\.\s+\S", section, re.MULTILINE))


def normalize_lower(section: str) -> str:
    return re.sub(r"\s+", " ", section).strip().lower()


def parse_dash_items(section: str) -> list[str]:
    items: list[str] = []
    for line in section.splitlines():
        stripped = line.strip()
        if stripped.startswith("- "):
            items.append(stripped[2:].strip())
    return items


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate planning packet structure")
    parser.add_argument("--input", required=True, help="Path to planning packet markdown")
    parser.add_argument("--json", action="store_true", help="Emit JSON output")
    args = parser.parse_args()

    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Error: Input file not found: {args.input}", file=sys.stderr)
        return 2

    content = input_path.read_text(encoding="utf-8")
    checks: list[dict] = []
    errors: list[str] = []

    # 1. Required sections present
    for section in REQUIRED_SECTIONS:
        if section in content:
            checks.append({"check": f"section:{section}", "status": "pass"})
        else:
            checks.append({"check": f"section:{section}", "status": "fail", "message": f"Missing required section"})
            errors.append(f"Missing required section: {section}")

    # 2. Stream classification is valid
    stream_section = extract_section(content, "## Stream classification")
    if stream_section:
        stream_lower = normalize_lower(stream_section)
        found_streams = [s for s in ALLOWED_STREAMS if s.lower() in stream_lower]
        if found_streams:
            checks.append({"check": "stream_classification", "status": "pass", "value": found_streams[0]})
        else:
            checks.append({"check": "stream_classification", "status": "fail", "message": f"Stream must be one of: {', '.join(sorted(ALLOWED_STREAMS))}"})
            errors.append("Stream classification must be explicit and valid")

    # 3. Risk classification is valid
    risk_section = extract_section(content, "## Risk classification")
    if risk_section:
        risk_lower = normalize_lower(risk_section)
        found_risks = [r for r in ALLOWED_RISKS if r in risk_lower]
        if found_risks:
            checks.append({"check": "risk_classification", "status": "pass", "value": found_risks[0]})
        else:
            checks.append({"check": "risk_classification", "status": "fail", "message": f"Risk must be one of: {', '.join(sorted(ALLOWED_RISKS))}"})
            errors.append("Risk classification must be explicit and valid")

    # 4. In scope has items
    in_scope = extract_section(content, "## In scope")
    if in_scope:
        if contains_list_item(in_scope):
            checks.append({"check": "in_scope_items", "status": "pass"})
        else:
            checks.append({"check": "in_scope_items", "status": "fail", "message": "In scope section must contain bullet items"})
            errors.append("In scope section must have at least one bullet item")

    # 5. Out of scope has items
    out_scope = extract_section(content, "## Out of scope")
    if out_scope:
        if contains_list_item(out_scope):
            checks.append({"check": "out_of_scope_items", "status": "pass"})
        else:
            checks.append({"check": "out_of_scope_items", "status": "fail", "message": "Out of scope section must contain bullet items"})
            errors.append("Out of scope section must have at least one bullet item")

    # 6. Impacted layers identified
    layers_section = extract_section(content, "## Impacted layers")
    if layers_section and contains_list_item(layers_section):
        checks.append({"check": "impacted_layers", "status": "pass"})
    else:
        checks.append({"check": "impacted_layers", "status": "fail", "message": "Impacted layers section must contain bullet items"})
        errors.append("Impacted layers must be listed")

    # 7. Dependency graph has items
    dep_section = extract_section(content, "## Dependency graph")
    if dep_section and contains_list_item(dep_section):
        checks.append({"check": "dependency_graph", "status": "pass"})
    else:
        checks.append({"check": "dependency_graph", "status": "fail", "message": "Dependency graph section must contain ordered items"})
        errors.append("Dependency graph must be specified")

    # 8. Mandatory validators include architecture-validator
    validators_section = extract_section(content, "## Mandatory validators")
    if validators_section:
        items = parse_dash_items(validators_section)
        items_lower = [i.lower() for i in items]
        if any("architecture" in i for i in items_lower):
            checks.append({"check": "mandatory_validators", "status": "pass"})
        else:
            checks.append({"check": "mandatory_validators", "status": "fail", "message": f"Architecture validator is mandatory"})
            errors.append("Architecture Validator must be in mandatory validators")
    else:
        checks.append({"check": "mandatory_validators", "status": "fail", "message": "Section not found"})

    # 9. CI gates are valid
    ci_section = extract_section(content, "## Mandatory CI gates")
    if ci_section:
        items = parse_dash_items(ci_section)
        invalid = [i for i in items if i not in ALLOWED_CI_GATES]
        if invalid:
            checks.append({"check": "ci_gates", "status": "fail", "message": f"Unknown CI gates: {', '.join(invalid)}"})
            errors.append(f"Unknown CI gates: {', '.join(invalid)}")
        elif items:
            checks.append({"check": "ci_gates", "status": "pass", "value": items})
        else:
            checks.append({"check": "ci_gates", "status": "fail", "message": "No CI gates specified"})
            errors.append("At least one CI gate must be specified")
    else:
        checks.append({"check": "ci_gates", "status": "fail", "message": "Section not found"})

    # 10. First issue recommendation exists
    first_issue = extract_section(content, "## First implementation issue recommendation")
    if first_issue and len(first_issue) > 10:
        checks.append({"check": "first_issue_recommendation", "status": "pass"})
    else:
        checks.append({"check": "first_issue_recommendation", "status": "fail", "message": "First implementation issue must be recommended"})
        errors.append("First implementation issue must be recommended")

    # Summary
    passed = sum(1 for c in checks if c["status"] == "pass")
    failed = sum(1 for c in checks if c["status"] == "fail")

    if args.json:
        output = {
            "input": str(input_path),
            "timestamp": __import__("datetime").datetime.utcnow().isoformat() + "Z",
            "summary": {"total": len(checks), "passed": passed, "failed": failed},
            "checks": checks,
            "errors": errors,
            "status": "pass" if failed == 0 else "fail",
        }
        print(json.dumps(output, indent=2))
    else:
        print(f"\nPlanning Packet Validation: {args.input}")
        print(f"{'='*50}")
        for check in checks:
            icon = "PASS" if check["status"] == "pass" else "FAIL"
            print(f"  [{icon}] {check['check']}")
            if check["status"] == "fail" and "message" in check:
                print(f"         {check['message']}")
        print(f"\nSummary: {passed} passed, {failed} failed, {len(checks)} total")
        if errors:
            print(f"\nErrors ({len(errors)}):")
            for e in errors:
                print(f"  - {e}")
        print(f"\nResult: {'VALID' if failed == 0 else 'INVALID'}")

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
