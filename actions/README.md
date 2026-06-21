# rigorix-actions

**GitHub Actions adapter for Rigorix — PR governance and automated code generation in CI/CD.**

A thin adapter over `rigorix-engine` that wraps the engine as a GitHub Action. All business logic lives in the engine; this crate adds only GitHub-specific I/O:

---

## Operation Modes

### Mode A — Reactive Governance

Analyze PR diffs against configurable policy rules. Blocks or flags changes based on `.rigorix/policy.toml` (loaded from the **base branch** to prevent tampering).

```yaml
# .github/workflows/rigorix-governance.yml
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  governance:
    runs-on: ubuntu-latest
    steps:
      - uses: rigorix/rigorix-action@v1
        with:
          mode: validate
          github_token: ${{ secrets.GITHUB_TOKEN }}
```

### Mode B — Active Execution

Plan and execute DAG-based code generation with a self-correcting validation loop.

```yaml
# .github/workflows/rigorix-execute.yml
on:
  workflow_dispatch:
    inputs:
      intent:
        description: 'What to do'
        required: true

jobs:
  execute:
    runs-on: ubuntu-latest
    steps:
      - uses: rigorix/rigorix-action@v1
        with:
          mode: run
          intent: ${{ github.event.inputs.intent }}
          github_token: ${{ secrets.GITHUB_TOKEN }}
```

---

## Module Structure

```
actions/src/
├── main.rs                          # Binary entry point
├── lib.rs                           # Library root
│
├── shared/                          # Shared infrastructure
│   ├── mod.rs
│   └── github_client.rs             # GitHub REST API client (Octocrab)
│
├── action_input/                    # Phase 1: Input parsing
│   ├── domain/                      # ActionInputs, ActionConfig, CommentCommand, CiEnvironment
│   ├── application/                 # InputParsingService, CommentParsingService, CiDetectionService, ConfigLoadingService
│   └── infrastructure/              # InputRepository, ConfigRepository, EventRepository
│
├── security_config/                 # Phase 0: Pre-flight security
│   ├── domain/                      # SecurityContext, HmacKey, SecurityPolicy
│   ├── application/                 # SecurityValidationService, ForkDetectionService, TokenValidationService, HmacSigningService
│   └── infrastructure/              # ForkRepository, TokenRepository, HmacKeyRepository
│
├── diff_analyzer/                   # Phase 3: PR diff analysis
│   ├── domain/                      # PrDiff, ChangedFile, DiffHunk, FileRisk, AiSignal
│   ├── application/                 # DiffParsingService, PathValidationService, RiskClassificationService, AiSignalDetectionService
│   └── infrastructure/              # DiffRepository
│
├── policy_evaluator/                # Phase 3: Policy enforcement
│   ├── domain/                      # PolicyDocument, DenyRule, ReviewRule, FlagRule, PolicyResult
│   ├── application/                 # PolicyLoadingService, PolicyEvaluationService, OrgPolicyMergingService
│   └── infrastructure/              # PolicyRepository, OrgPolicyRepository
│
├── action_output/                   # Phase 3-4: GitHub-native output
│   ├── domain/                      # FormattedOutput, WorkflowAnnotation, StepSummary, OutputVariable, PrComment
│   ├── application/                 # OutputFormattingService, AnnotationWritingService, StepSummaryWritingService
│   └── infrastructure/              # OutputRepository, SummaryRepository, GitHubClient
│
├── ci_integration/                  # Phase 4: CI primitives
│   ├── domain/                      # StatusCheckState, PrComment
│   ├── application/                 # StatusCheckService, PrCommentService
│   └── infrastructure/              # StatusCheckRepository, PrCommentRepository
│
├── audit_posting/                   # Phase 4: Audit records
│   ├── domain/                      # SignedAuditRecord
│   ├── application/                 # AuditPostingService, AuditRecordQueue
│   └── infrastructure/              # AuditBackend (filesystem + HTTP)
│
└── action_entrypoint/               # Phase 5: Event routing
    ├── domain/                      # ActionContext, ActionMode, ActionOutput, GitHubEvent
    ├── application/                 # ActionRouter, ModeResolver
    └── infrastructure/              # ContextRepository
```

---

## GitHub Action Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `mode` | Execution mode: `auto`, `run`, `plan`, `validate`, `status` | `auto` |
| `intent` | Natural-language intent for planning/execution | — |
| `github_token` | GitHub token for API calls (PR comments, status checks) | — |
| `max_iterations` | Maximum validation loop iterations | `3` |
| `max_llm_calls` | Maximum LLM API calls per execution | — |
| `max_llm_tokens` | Maximum LLM tokens per execution | — |
| `profile` | Configuration profile | — |
| `permission_mode` | Permission mode for tool execution | — |

---

## Phase 0 — Security Configuration

Runs before any operation begins:

- **Fork Detection** — Detects PRs from forked repos (prevents secret exposure)
- **Secret Masking** — Masks sensitive values from logs
- **Token Validation** — Validates GitHub token permissions
- **HMAC Signing** — Signs audit records for integrity verification
- **URL Allowlist** — Restricts outbound HTTP to approved endpoints

---

## Phase 3 — Policy-Based Governance

The `.rigorix/policy.toml` file (loaded from the **base branch**) defines rules:

```toml
[policy]
version = "1.0.0"

[policy.deny]
paths = ["**/secret*", "**/*.pem"]

[policy.review]
paths = ["src/**"]
max_additions = 500
require_approval = ["src/core/**"]

[policy.flag]
paths = ["*.generated.*"]
ai_generated_threshold = 0.8
```

Three violation categories:
| Level | Action | Use Case |
|-------|--------|----------|
| `deny` | Blocks the PR | Secrets, build artifacts |
| `require_review` | Flags for human review | Large diffs, core module changes |
| `flag` | Warns without blocking | Generated code, formatting issues |

---

## Testing

```bash
# Unit + integration tests
cargo test -p rigorix-actions

# With mock HTTP server (wiremock)
cargo test -p rigorix-actions --features mock-server
```

---

## Development Status

| Phase | Module | Status |
|-------|--------|--------|
| 0 | security_config | ✅ Contract frozen, interface-only |
| 1 | action_input | ✅ Contract frozen, interface-only |
| 3 | diff_analyzer | ✅ Contract frozen, interface-only |
| 3 | policy_evaluator | ✅ Contract frozen, interface-only |
| 3-4 | action_output | ✅ Contract frozen, interface-only |
| 4 | ci_integration | ✅ Contract frozen, interface-only |
| 4 | audit_posting | ✅ Contract frozen, interface-only |
| 5 | action_entrypoint | ✅ Contract frozen, interface-only |

---

## License

MIT OR Apache-2.0
