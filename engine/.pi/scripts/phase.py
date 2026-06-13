#!/usr/bin/env python3
"""phase.py — Query the Phase Manifest (.pi/architecture/PHASE_MANIFEST.toml)

Usage:
  python3 .pi/scripts/phase.py list <phase>   — List modules in a phase
  python3 .pi/scripts/phase.py module <name>  — Show which phase a module belongs to
  python3 .pi/scripts/phase.py status         — Show implementation status
  python3 .pi/scripts/phase.py help           — Show this help
"""

import sys
import os
import re
from pathlib import Path

# ── Helpers ──

def find_project_root():
    """Walk up from script dir to find engine/.pi/"""
    script_dir = Path(__file__).resolve().parent
    for parent in [script_dir, script_dir.parent] + list(script_dir.parents):
        manifest = parent / ".pi" / "architecture" / "PHASE_MANIFEST.toml"
        if manifest.exists():
            return manifest
        # also check if we're inside engine/
        manifest2 = parent / "architecture" / "PHASE_MANIFEST.toml"
        if manifest2.exists():
            return manifest2
    return None


def parse_toml_simple(path):
    """Simple TOML parser — enough for our manifest structure."""
    with open(path) as f:
        content = f.read()

    result = {}
    current_section = None
    current_module = None
    collecting = []

    for line in content.splitlines():
        # Section header: [phase0] or [milestones.m0]
        m = re.match(r'^\[([a-z0-9_.]+)\]$', line.strip())
        if m:
            if current_module:
                if current_section not in result:
                    result[current_section] = []
                result[current_section].append(dict(current_module))
                current_module = None

            # Check if this is a sub-array entry [[phaseN.modules]]
            current_section = m.group(1)
            continue

        # Sub-array entry: [[phase0.modules]]
        m = re.match(r'^\[\[([a-z0-9_.]+)\]\]$', line.strip())
        if m:
            if current_module:
                if current_section not in result:
                    result[current_section] = []
                result[current_section].append(dict(current_module))

            current_section = m.group(1)
            current_module = {}
            continue

        # Key = "value" or Key = 123 (integer) or Key = [list]
        m = re.match(r'^(\w+)\s*=\s*"([^"]*)"', line)
        if m and current_module is not None:
            current_module[m.group(1)] = m.group(2)
            continue

        m = re.match(r'^(\w+)\s*=\s*(\d+)', line)
        if m and current_module is not None:
            current_module[m.group(1)] = m.group(2)
            continue

        m = re.match(r'^(\w+)\s*=\s*\[([^\]]*)\]', line)
        if m and current_module is not None:
            val = m.group(2)
            items = [v.strip().strip('"') for v in val.split(",") if v.strip()]
            current_module[m.group(1)] = items
            continue

        m = re.match(r'^(\w+)\s*=\s*"([^"]*)"', line)
        if m:
            result[m.group(1)] = m.group(2)
            continue

        m = re.match(r'^(\w+)\s*=\s*(\d+)', line)
        if m:
            result[m.group(1)] = m.group(2)
            continue

    # Flush last module
    if current_module and current_section:
        if current_section not in result:
            result[current_section] = []
        result[current_section].append(dict(current_module))

    return result


PHASE_NAMES = {
    "phase0": "Foundation",
    "phase1": "Infrastructure",
    "phase2": "Domain Logic",
    "phase3": "Orchestration",
    "phase4": "Observability",
}

# ── Commands ──

def cmd_list(data, phase_key):
    phase_key = phase_key.lower()
    # Map short names
    short_map = {"0": "phase0", "1": "phase1", "2": "phase2", "3": "phase3", "4": "phase4"}
    phase_key = short_map.get(phase_key, phase_key)

    if phase_key not in PHASE_NAMES:
        print(f"Unknown phase: {phase_key}")
        print(f"Valid: {', '.join(PHASE_NAMES.keys())} or 0-4")
        sys.exit(1)

    modules = data.get(f"{phase_key}.modules", [])
    name = PHASE_NAMES[phase_key]
    gate = data.get(phase_key, {}).get("gate", "") if isinstance(data.get(phase_key), dict) else ""

    print()
    print(f"  ═══ {phase_key}: {name} ═══")
    print()

    for mod in modules:
        mark = "✓" if mod.get("status") == "implemented" else "○"
        print(f"    [{mod.get('id', '?'):>2}] {mark} {mod.get('name', '?'):30s} ({mod.get('status', '?'):12s})  → {mod.get('module', '?')}/")

    print()
    if gate:
        print(f"    Gate: {gate}")
        print()


def cmd_module(data, module_name):
    for phase_key in PHASE_NAMES:
        modules = data.get(f"{phase_key}.modules", [])
        for mod in modules:
            if mod.get("module", "").lower() == module_name.lower():
                print()
                print(f"    Phase:   {phase_key} ({PHASE_NAMES[phase_key]})")
                print(f"    ID:      {mod.get('id', '?')}")
                print(f"    Name:    {mod.get('name', '?')}")
                print(f"    Status:  {mod.get('status', '?')}")
                print(f"    Folder:  src/{mod.get('module', '?')}/")
                print(f"    Desc:    {mod.get('description', '?')}")
                deps = mod.get("depends_on", [])
                if deps:
                    print(f"    Depends: {', '.join(deps)}")
                else:
                    print("    Depends: (none)")
                print()
                return

    print(f"Module '{module_name}' not found in manifest.")
    print("Run 'phase.py list' to see all modules.")
    sys.exit(1)


def cmd_status(data):
    print()
    print("  ═══ Implementation Status ═══")
    print()

    implemented = 0
    planned = 0
    total = 0

    for phase_key in PHASE_NAMES:
        modules = data.get(f"{phase_key}.modules", [])
        name = PHASE_NAMES[phase_key]
        print(f"    {phase_key} ({name}):")

        for mod in modules:
            status = mod.get("status", "unknown")
            mark = "✓" if status == "implemented" else "○"
            print(f"      {mark} {mod.get('name', '?'):30s} ({status:12s})  → {mod.get('module', '?')}/")
            total += 1
            if status == "implemented":
                implemented += 1
            else:
                planned += 1

        print()

    coverage = f"{implemented}/{total} implemented ({implemented*100//total}%)"
    print(f"    Coverage: {coverage}")
    print()


def cmd_help():
    print("Usage: python3 .pi/scripts/phase.py <command> [args]")
    print()
    print("Commands:")
    print("  list <phase>    List modules in a phase (phase0-4, or 0-4)")
    print("  module <name>   Show which phase a module belongs to")
    print("  status          Show implementation status across all phases")
    print("  help            Show this help")
    print()
    print("Examples:")
    print("  python3 .pi/scripts/phase.py list phase0")
    print("  python3 .pi/scripts/phase.py module cancellation")
    print("  python3 .pi/scripts/phase.py status")


# ── Main ──

def main():
    manifest_path = find_project_root()
    if not manifest_path:
        print("Error: PHASE_MANIFEST.toml not found.")
        print("Ensure you're in the project root or engine/ directory.")
        sys.exit(1)

    data = parse_toml_simple(manifest_path)

    if len(sys.argv) < 2:
        cmd_help()
        return

    command = sys.argv[1]

    if command == "list":
        if len(sys.argv) < 3:
            print("Usage: phase.py list <phase>")
            sys.exit(1)
        cmd_list(data, sys.argv[2])
    elif command == "module":
        if len(sys.argv) < 3:
            print("Usage: phase.py module <name>")
            sys.exit(1)
        cmd_module(data, sys.argv[2])
    elif command == "status":
        cmd_status(data)
    else:
        cmd_help()


if __name__ == "__main__":
    main()
