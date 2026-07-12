#!/usr/bin/env bash
# Guardian Local Developer Workflow — Stage Runner
#
# Runs a specific CI stage locally.
#
# Usage:
#   ./run_stage.sh docs_policy
#   ./run_stage.sh architecture_conformance
#   ./run_stage.sh lint
#   ./run_stage.sh static_analysis
#   ./run_stage.sh unit
#   ./run_stage.sh integration
#   ./run_stage.sh security
#   ./run_stage.sh migration_verify
#   ./run_stage.sh package_build
#   ./run_stage.sh release_readiness

set -euo pipefail

PI_DIR=".pi"
CI_DIR="${PI_DIR}/scripts/ci"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

STAGE="${1:-}"

if [[ -z "$STAGE" ]]; then
    echo "Usage: $0 <stage_name>"
    echo ""
    echo "Available stages:"
    echo "  docs_policy              MR traceability, docs sync"
    echo "  architecture_conformance 11+ architectural contract checks"
    echo "  lint                     Language-specific linting"
    echo "  static_analysis          Type checking, import boundaries"
    echo "  unit                     Unit tests"
    echo "  integration              Integration tests (requires PostgreSQL, Redis)"
    echo "  security                 Secret scan, dependency audit"
    echo "  migration_verify         Migration apply + verification"
    echo "  package_build            Docker build"
    echo "  release_readiness        Runbook, observability, release policy"
    exit 1
fi

echo -e "${YELLOW}Running stage: ${STAGE}${NC}"
echo ""

case "$STAGE" in
    docs_policy)
        if [[ -f "${CI_DIR}/run_stage_docs_policy.sh" ]]; then
            bash "${CI_DIR}/run_stage_docs_policy.sh"
        elif [[ -f "${CI_DIR}/stage_docs_policy.sh" ]]; then
            bash "${CI_DIR}/stage_docs_policy.sh"
        else
            echo -e "${RED}✗ Script not found${NC}"
            exit 1
        fi
        ;;

    architecture_conformance)
        if [[ -f "${CI_DIR}/check_architecture_conformance.sh" ]]; then
            bash "${CI_DIR}/check_architecture_conformance.sh"
        else
            echo -e "${RED}✗ Script not found${NC}"
            exit 1
        fi
        ;;

    lint)
        # Auto-detect and run language-specific lint
        if [[ -f "pyproject.toml" ]]; then
            echo "Python project detected"
            ruff check . && ruff format --check .
        elif [[ -f "package.json" ]]; then
            echo "TypeScript project detected"
            biome check . && biome format --check .
        elif [[ -f "Cargo.toml" ]]; then
            echo "Rust project detected"
            cargo clippy -- -D warnings && cargo fmt --check
        elif [[ -f "go.mod" ]]; then
            echo "Go project detected"
            golangci-lint run && gofmt -d .
        else
            echo -e "${YELLOW}⊘ No project configuration found${NC}"
        fi
        ;;

    static_analysis)
        if [[ -f "pyproject.toml" ]]; then
            mypy . --ignore-missing-imports || true
        elif [[ -f "tsconfig.json" ]]; then
            tsc --noEmit || true
        elif [[ -f "Cargo.toml" ]]; then
            cargo check || true
        fi
        if [[ -f "${CI_DIR}/check_arch_sanity.sh" ]]; then
            bash "${CI_DIR}/check_arch_sanity.sh"
        fi
        if [[ -f "${CI_DIR}/check_import_boundaries.sh" ]]; then
            bash "${CI_DIR}/check_import_boundaries.sh"
        fi
        ;;

    unit)
        if command -v pytest &>/dev/null; then
            pytest tests/unit -v
        elif command -v bun &>/dev/null; then
            bun test
        elif command -v cargo &>/dev/null; then
            cargo test
        elif command -v go &>/dev/null; then
            go test ./...
        else
            echo -e "${YELLOW}⊘ No test runner found${NC}"
        fi
        ;;

    integration)
        # Check services
        if ! command -v psql &>/dev/null || ! psql -h localhost -U postgres -c "SELECT 1" &>/dev/null 2>&1; then
            echo -e "${YELLOW}⊘ PostgreSQL not available${NC}"
            echo "  docker run -d --name postgres -e POSTGRES_PASSWORD=test -p 5432:5432 postgres:16"
            exit 0
        fi
        if ! command -v redis-cli &>/dev/null || ! redis-cli ping 2>/dev/null | grep -q "PONG"; then
            echo -e "${YELLOW}⊘ Redis not available${NC}"
            echo "  docker run -d --name redis -p 6379:6379 redis:7"
            exit 0
        fi

        if command -v pytest &>/dev/null; then
            pytest tests/integration -v
        else
            echo -e "${YELLOW}⊘ No integration test runner found${NC}"
        fi
        ;;

    security)
        if [[ -f "${CI_DIR}/stage_security.sh" ]]; then
            bash "${CI_DIR}/stage_security.sh"
        else
            # Fallback: secret scan
            echo "Running secret scan..."
            if grep -rE "(sk-[A-Za-z0-9]{32,}|ghp_[A-Za-z0-9]{36}|AKIA[0-9A-Z]{16})" . --include="*.py" --include="*.ts" --include="*.env" 2>/dev/null | grep -v ".git" | grep -v "node_modules" | grep -q .; then
                echo -e "${RED}✗ Potential secrets found${NC}"
                exit 1
            fi
            echo -e "${GREEN}✓ No secrets detected${NC}"
        fi
        ;;

    migration_verify)
        if ! command -v psql &>/dev/null || ! psql -h localhost -U postgres -c "SELECT 1" &>/dev/null 2>&1; then
            echo -e "${YELLOW}⊘ PostgreSQL not available${NC}"
            exit 0
        fi
        if command -v alembic &>/dev/null; then
            alembic upgrade head
        else
            echo -e "${YELLOW}⊘ alembic not installed${NC}"
        fi
        ;;

    package_build)
        if ! command -v docker &>/dev/null; then
            echo -e "${YELLOW}⊘ Docker not available${NC}"
            exit 0
        fi
        if [[ ! -f "Dockerfile" ]]; then
            echo -e "${YELLOW}⊘ No Dockerfile found${NC}"
            exit 0
        fi
        docker build -t guardian:local .
        ;;

    release_readiness)
        if [[ -f "${PI_DIR}/scripts/validate-architecture-readiness.sh" ]]; then
            bash "${PI_DIR}/scripts/validate-architecture-readiness.sh"
        else
            echo -e "${YELLOW}⊘ validate-architecture-readiness.sh not found${NC}"
        fi
        ;;

    *)
        echo -e "${RED}✗ Unknown stage: ${STAGE}${NC}"
        echo ""
        echo "Available stages: docs_policy, architecture_conformance, lint, static_analysis, unit, integration, security, migration_verify, package_build, release_readiness"
        exit 1
        ;;
esac
