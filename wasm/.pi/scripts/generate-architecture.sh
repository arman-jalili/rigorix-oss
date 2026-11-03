#!/usr/bin/env bash
# Architecture Generator
#
# Generates canonical architecture modules from intent or existing documents.
#
# Usage:
#   bash .pi/scripts/generate-architecture.sh --intent "Build an auth system..."
#   bash .pi/scripts/generate-architecture.sh --from "docs/prd.md,docs/design.md" --module "auth-system"

set -euo pipefail

PI_DIR=".pi"
ARCH_DIR="${PI_DIR}/architecture"
MODULES_DIR="${ARCH_DIR}/modules"

INTENT=""
FROM_DOCS=""
MODULE_NAME=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --intent) INTENT="$2"; shift 2 ;;
        --from) FROM_DOCS="$2"; shift 2 ;;
        --module) MODULE_NAME="$2"; shift 2 ;;
        *) shift ;;
    esac
done

# Validate inputs
if [[ -z "$INTENT" && -z "$FROM_DOCS" ]]; then
    echo "Error: --intent or --from is required"
    echo "Usage: $0 --intent \"description\" [--module name]"
    echo "       $0 --from \"file1.md,file2.md\" --module name"
    exit 1
fi

if [[ -z "$MODULE_NAME" ]]; then
    if [[ -n "$INTENT" ]]; then
        # Derive module name from intent (first few words, lowercase)
        MODULE_NAME=$(echo "$INTENT" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9 ]//g' | awk '{print $1"-"$2}' | head -c 40)
    else
        echo "Error: --module is required when using --from"
        exit 1
    fi
fi

mkdir -p "$MODULES_DIR"

OUTPUT_FILE="${MODULES_DIR}/${MODULE_NAME}.md"

if [[ -f "$OUTPUT_FILE" ]]; then
    echo "Warning: Module file already exists: $OUTPUT_FILE"
    read -p "Overwrite? (y/N): " confirm
    if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
        echo "Aborted."
        exit 0
    fi
fi

echo "Generating architecture module: $OUTPUT_FILE"
echo ""

if [[ -n "$FROM_DOCS" ]]; then
    # Extract content from existing documents
    IFS=',' read -ra DOCS <<< "$FROM_DOCS"
    echo "Reading documents:"
    CONTEXT=""
    for doc in "${DOCS[@]}"; do
        doc=$(echo "$doc" | xargs) # trim whitespace
        if [[ -f "$doc" ]]; then
            echo "  ✓ $doc"
            CONTEXT="${CONTEXT}
--- Content from ${doc} ---
$(cat "$doc")
"
        else
            echo "  ✗ $doc (not found)"
        fi
    done

    echo ""
    echo "=== EXTRACTED CONTEXT ==="
    echo "$CONTEXT" | head -100
    echo "..."
    echo ""
    echo "Generate the architecture module based on these documents."
    echo "Run the /architect-generate command with the agent to create the module."

elif [[ -n "$INTENT" ]]; then
    echo "Intent: $INTENT"
    echo ""

    # Generate a structured module template from intent
    cat > "$OUTPUT_FILE" << TEMPLATE
# ${MODULE_NAME//-/ }

> Generated from intent on $(date -u +%Y-%m-%dT%H:%M:%SZ)
> Intent: ${INTENT}

## Overview

> TODO: Fill in the module overview based on the intent above.

## Components

### Component 1: [Name]
status: planned
description: [TODO: Describe what this component does]
depends: [TODO: List dependencies, or "none"]

### Component 2: [Name]
status: planned
description: [TODO: Describe what this component does]
depends: [TODO: Component 1, or other dependencies]

### Component 3: [Name]
status: planned
description: [TODO: Describe what this component does]
depends: [TODO: Dependencies]

### Architecture Observability
status: planned
description: Runbook, DR plan, metrics, tracing for the ${MODULE_NAME//-/ } module.
depends: [TODO: All other components]

## Interfaces

### Inputs
- [TODO: What does this module receive?]

### Outputs
- [TODO: What does this module produce?]

## Dependencies

### Internal
- [TODO: Other Guardian modules this depends on]

### External
- [TODO: Third-party services, APIs, databases]

## Failure Modes

- [TODO: What happens when this module fails?]
- [TODO: Recovery strategies]

## Security Considerations

- [TODO: Authentication, authorization, data protection]
TEMPLATE

    echo "✓ Module template created: $OUTPUT_FILE"
    echo ""
    echo "Next steps:"
    echo "1. Edit $OUTPUT_FILE to fill in the TODO sections"
    echo "2. Run: /architect --epic \"${MODULE_NAME//-/ } v1\""
    echo "3. Guardian will discover the planned components and generate issues"
fi

echo ""
echo "Done."
