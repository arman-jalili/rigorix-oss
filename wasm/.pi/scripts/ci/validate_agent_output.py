#!/usr/bin/env python3
"""Validate agent output schema compliance.

Validates that agent-generated outputs conform to expected schemas per agent role.
Checks required sections, contradiction patterns, and structural consistency.

Usage:
    python scripts/ci/validate_agent_output.py --input=output.md
    python scripts/ci/validate_agent_output.py --input=output.md --schema=architecture-validator
    python scripts/ci/validate_agent_output.py --input=output.md --json

Exit codes:
    0 - Output is valid
    1 - Validation errors found
    2 - Script error
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


SCHEMAS: dict[str, dict] = {
    "architecture-validator": {
        "required_sections": [
            "## Decision",
            "## Blocking findings",
            "## Non-blocking recommendations",
            "## Section reference verification",
            "## Acceptance criteria verification",
        ],
        "contradiction_checks": [
            {
                "pattern1": r"## Decision\s*\n\s*-\s*`?pass`?",
                "pattern2": r"## Non-blocking recommendations\s*\n\s*-",
                "message": "Decision is 'pass' but recommendations exist (should be 'pass_with_recommendations')",
            },
        ],
    },
    "epic-plan": {
        "required_sections": [
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
        ],
        "contradiction_checks": [],
    },
    "issue-draft": {
        "required_sections": [
            "## Why",
            "## Scope",
            "## In scope",
            "## Out of scope",
            "## Dependencies",
            "## Acceptance criteria",
            "## Verification",
            "## Canonical references",
        ],
        "contradiction_checks": [],
    },
    "implementation-report": {
        "required_sections": [
            "## Readiness check",
            "## Acceptance criteria trace map",
            "## Files changed",
            "## Tests and CI impacts",
            "## Toolchain validation",
            "## Done / not done against acceptance criteria",
        ],
        "contradiction_checks": [],
    },
    "generic": {
        "required_sections": ["## Summary"],
        "contradiction_checks": [],
    },
}


def extract_section(content: str, heading: str) -> str:
    pattern = re.compile(rf"^{re.escape(heading)}\s*$", re.MULTILINE)
    match = pattern.search(content)
    if not match:
        return ""
    start = match.end()
    next_heading = re.search(r"^##\s+", content[start:], re.MULTILINE)
    end = start + next_heading.start() if next_heading else len(content)
    return content[start:end].strip()


def check_contradictions(content: str, checks: list[dict]) -> list[dict]:
    results: list[dict] = []
    for check in checks:
        has_p1 = bool(re.search(check["pattern1"], content, re.MULTILINE))
        has_p2 = bool(re.search(check["pattern2"], content, re.MULTILINE))
        if has_p1 and has_p2:
            results.append({
                "check": "contradiction",
                "status": "fail",
                "message": check["message"],
            })
        else:
            results.append({
                "check": f"contradiction: {check['message'][:40]}",
                "status": "pass",
            })
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate agent output schema compliance")
    parser.add_argument("--input", required=True, help="Path to agent output markdown")
    parser.add_argument("--schema", default="generic", help="Schema to validate against")
    parser.add_argument("--json", action="store_true", help="Emit JSON output")
    args = parser.parse_args()

    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Error: Input file not found: {args.input}", file=sys.stderr)
        return 2

    content = input_path.read_text(encoding="utf-8")
    schema = SCHEMAS.get(args.schema, SCHEMAS["generic"])
    all_checks: list[dict] = []

    # Check required sections
    for section in schema["required_sections"]:
        if section in content:
            all_checks.append({"check": f"section:{section}", "status": "pass"})
        else:
            all_checks.append({
                "check": f"section:{section}",
                "status": "fail",
                "message": f"Missing required section: {section}",
            })

    # Check contradictions
    contradiction_results = check_contradictions(content, schema["contradiction_checks"])
    all_checks.extend(contradiction_results)

    # Summary
    passed = sum(1 for c in all_checks if c["status"] == "pass")
    failed = sum(1 for c in all_checks if c["status"] == "fail")

    if args.json:
        output = {
            "input": str(input_path),
            "schema": args.schema,
            "timestamp": __import__("datetime").datetime.utcnow().isoformat() + "Z",
            "summary": {"total": len(all_checks), "passed": passed, "failed": failed},
            "checks": all_checks,
            "status": "pass" if failed == 0 else "fail",
        }
        print(json.dumps(output, indent=2))
    else:
        print(f"\nAgent Output Validation ({args.schema}): {args.input}")
        print(f"{'='*50}")
        for check in all_checks:
            icon = "PASS" if check["status"] == "pass" else "FAIL"
            print(f"  [{icon}] {check['check']}")
            if check["status"] == "fail" and "message" in check:
                print(f"         {check['message']}")
        print(f"\nSummary: {passed} passed, {failed} failed, {len(all_checks)} total")
        print(f"Result: {'VALID' if failed == 0 else 'INVALID'}")

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
