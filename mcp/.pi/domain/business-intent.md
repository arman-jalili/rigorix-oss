# Business Intent: Rigorix MCP Gateway

## Domain Statement

A universal gateway that bridges AI coding assistants to Rigorix's deterministic execution, audit, and policy enforcement capabilities via the Model Context Protocol (MCP). This enables any MCP-compatible AI tool (Claude, Cline, Cursor, Copilot Codex, Aider, Continue.dev) to delegate execution actions to Rigorix — gaining audit trails, policy compliance, and deterministic replay without leaving their preferred interface.

## Business Problem

AI coding assistants are becoming the primary interface for software engineering. Developers spend most of their time inside chat sessions with Claude Code, Cline, or Cursor. These tools excel at conversational planning, exploration, and rapid iteration — but they lack:

1. **Deterministic execution**: Chat sessions are ephemeral. There is no guarantee the same prompt produces the same result. Actions taken by AI cannot be replayed or verified.

2. **Audit trails**: No record of what the AI did, when, or why. Compliance teams (SOC2, SOX, financial services) cannot audit AI-driven code changes.

3. **Policy enforcement**: No way for organizations to enforce rules (blocked tools, budget limits, risk thresholds) across AI tool usage. Each AI tool has its own permission model, none integrated with enterprise policy.

4. **Cross-tool governance**: An organization using Claude, Cline, and Cursor across different teams has no unified view of AI activity. Three separate tools, three separate contexts, zero cross-traceability.

The existing solutions require developers to choose: conversational flexibility OR enterprise compliance. Rigorix MCP Gateway eliminates this tradeoff.

## Target Users

### Primary: Individual Developers
- Use any AI coding tool (Claude Code, Cline, Cursor) for daily development
- Want audit trails for debugging their own work
- Want to replay failed executions
- Want deterministic guarantees when running critical operations (refactoring, migrations)
- Want to save and reuse execution templates

### Secondary: Engineering Teams
- Standardize on execution templates across team members
- Share audit trails for code review
- Enforce team-level policies (tool allowlists, budget caps)
- Track AI token usage across the team

### Tertiary: Enterprise Organizations
- Cross-tool audit trail for compliance (SOC2, SOX, Financial)
- Centralized policy management across all AI tools
- Approval workflows for high-risk operations
- Usage analytics and billing chargebacks per team
- Role-based access control for audit data
- Integration with existing SIEM and logging infrastructure

## Value Proposition

Rigorix MCP Gateway lets any AI coding tool execute actions through Rigorix, gaining deterministic execution, full audit trails, and policy enforcement — without changing the developer's workflow.

```
The AI plans conversationally. Rigorix executes deterministically. Enterprise audits everything.
```

## Core Business Capabilities

### 1. Deterministic Plan Execution
The system can execute a structured plan (a sequence of steps with tools, parameters, and constraints) and produce the same result given the same plan and context. Execution is tracked step-by-step, each step recorded in an immutable audit log.

### 2. Pre-Flight Plan Validation
Before executing a plan, the system can validate it against current policies, budget limits, tool allowlists, and risk thresholds. This catches violations before they happen and gives the AI assistant feedback to adjust the plan.

### 3. Unified Audit Trail
Every execution produces an audit record with: who requested it, which AI tool initiated it, what steps were executed, what changed in the codebase, how long it took, how many tokens were consumed. Records are queryable by execution ID, time range, template name, and status.

### 4. Template Management
Plans can be saved as reusable templates. Templates capture proven execution patterns (refactor auth module, add test suite, run migration) that can be discovered and invoked from any AI tool. Templates include constraints (budget limits, allowed tools, risk thresholds).

### 5. Enforcement Status
The system exposes current enforcement state: remaining budget, active limits, circuit breaker status, available policies. AI assistants can check this proactively before planning to avoid enforcement rejections.

### 6. Enterprise Integration (Gated)
When connected to Rigorix Enterprise, additional capabilities become available:
- Cross-team audit queries across all AI tools and all developers
- Approval workflows for high-risk execution plans
- Centralized policy management (update policies from chat)
- Usage and compliance reports aggregated by team
- Team metadata and status

### 7. MCP Protocol Compliance
The system implements the Model Context Protocol (MCP) specification:
- Transport: stdio (CLI tools) and SSE (IDE plugins, long-running servers)
- Tools: Execute, validate, audit, template operations
- Resources: Read-only URIs for audit data, policy, budget, templates
- Prompts: Pre-crafted prompt templates for AI assistants
- Session lifecycle: Initialize, capability negotiation, shutdown

## Integration Points

### AI Coding Tools (consumers)
- Claude Desktop / Claude Code — MCP native
- Cline (VS Code extension) — MCP marketplace
- Cursor IDE — MCP agent mode
- Continue.dev — MCP registry
- Aider — MCP client support
- GitHub Copilot Codex — MCP (emerging)
- Windsurf — MCP (planned)

### Rigorix Engine (execution substrate)
- Orchestration and plan execution
- Policy enforcement and budget tracking
- Audit envelope creation and storage
- Configuration and template management

### Rigorix Enterprise (value-add)
- Multi-team audit aggregation
- Approval workflow engine
- Centralized policy management
- Usage metering and billing
- Compliance reporting

## Business Rules

1. **Free-tier completeness**: The OSS MCP server must be genuinely useful standalone. Individual developers get full execution, audit, and template capabilities without any enterprise configuration.

2. **Zero enterprise code in OSS**: The OSS binary contains no enterprise tool names, no enterprise schemas, no enterprise business logic. Enterprise capabilities are proxied to the enterprise server at runtime when configured.

3. **Transparent upgrade path**: Adding enterprise capability changes no configuration beyond setting an API URL and key. Same binary, same install, same MCP server address.

4. **Audit immutability**: Once recorded, audit trails cannot be modified or deleted through MCP tools. Audit is read-only.

5. **Fail-safe enterprise proxy**: If the enterprise server is unreachable, enterprise tools return clear diagnostic errors. OSS tools continue working unaffected.

6. **Template portability**: Templates created in one AI tool are accessible from all others via the MCP server. A template created in Claude Code can be executed from Cline.

## Out of Scope (Phase 0)

- The system does NOT implement AI chat functionality. Chat/planning is handled by the connected AI tool.
- The system does NOT implement enterprise server functionality. Enterprise logic lives in rigorix-enterprise.
- The system does NOT implement RBAC or multi-tenancy. Those are enterprise server concerns.
- The system does NOT implement its own authentication beyond inheriting OS process identity (stdio) or bind-address restriction (SSE).
