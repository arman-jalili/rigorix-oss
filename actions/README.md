# rigorix-actions

**The GitHub Action.** It is an adapter, not a second engine: the action maps
workflow events onto `rigorix-engine` services, formats the results as
GitHub-native output, and enforces policy against the diff under review.

```yaml
- uses: arman-jalili/rigorix-oss/actions@v1.4.0
  with:
    mode: governance
```

> **Versioning note.** `rigorix-actions` follows its **own cadence** and is **not**
> part of the `engine` / `mcp` / `cli` release train. Do not infer its version from
> a release-train tag — it is currently **1.4.0**.

---

## Two modes

**Mode A — reactive governance.** Rigorix evaluates a pull request that already
exists: the diff is analyzed, `policy.toml` rules are applied, and the result is a
status check plus annotations and a step summary. This is the CI-shaped use: *did
this change break the rules?*

**Mode B — active execution.** Rigorix runs the agent inside the workflow: intent →
plan → policy → execution → validation, with the same enforcement engine the CLI
uses. This is the autonomy-shaped use: *what is the agent allowed to do here, and
what did it do?*

Both modes end in the same place — a signed audit record.

---

## Modules

9 bounded-context modules, each with an architecture document:

| Module | Responsibility |
|---|---|
| `action_entrypoint` | Maps workflow events to engine orchestrator calls; the `main` entry |
| `action_input` | Parses Action inputs and environment context into engine types |
| `action_output` | GitHub-native output: annotations, step summaries, output variables |
| `policy_evaluator` | **Mode A** enforcement — deny / review / flag rules against the diff |
| `diff_analyzer` | Path validation, size limits, AI-generated signal detection |
| `ci_integration` | Status checks, PR comments, issue labelling, workflow orchestration |
| `audit_posting` | The audit trail, behind an `AuditBackend` seam |
| `security_config` | Hardening: fork detection, key masking, path validation, policy integrity |
| `shared` | Cross-module types and helpers |

### The audit trail is the open-core seam

OSS ships a `FilesystemAuditBackend`. The enterprise product plugs in a
`RigorixApiBackend` behind the same `AuditBackend` trait. The boundary is a trait,
not a fork of the code — so the OSS action produces a complete, self-contained
audit trail without an enterprise dependency.

### Security hardening is a module, not an afterthought

`security_config` exists because a GitHub Action runs on hostile input: pull
requests from forks, secrets in the environment, paths outside the workspace, and
a policy file the diff under review could try to modify. It handles fork
detection, API-key masking in output, path validation, and policy integrity
checking. Secret masking is guarded in CI — a value that is secret-shaped must not
reach a log line.

---

## Policy

Enforcement is driven by `policy.toml` in the repository:

```toml
[policy]
version = "1.0.0"          # the POLICY DOCUMENT version — not the crate version

[policy.deny]
paths = ["**/secret*", "**/*.pem", ".github/workflows/**"]

[policy.review]
paths = ["src/auth/**", "**/payments/**"]

[policy.flag]
paths = ["**/*.sql"]
```

The `version` field is the schema version of the policy document. It is checked
against the versions the evaluator supports, so an unsupported policy version is a
typed error rather than a silently ignored rule.

---

## Architecture

```
actions/src
├── action_entrypoint/  workflow event → orchestrator
├── action_input/       Action inputs → engine types
├── action_output/      engine results → GitHub annotations/summaries
├── policy_evaluator/   Mode A: policy vs diff
├── diff_analyzer/      path/size/AI-signal analysis
├── ci_integration/     status checks, comments, labels
├── audit_posting/      AuditBackend seam (filesystem | API)
├── security_config/    fork detection, masking, integrity
└── shared/
```

Each module is a Clean Architecture bounded context with its own
`domain` / `application` / `infrastructure` layers and frozen contracts.

---

## Development

```bash
cargo test -p rigorix-actions
cargo clippy -p rigorix-actions -- -D warnings
bash .pi/scripts/validate-architecture.sh
cd actions && bash .pi/scripts/validate-tests.sh
```

Architecture: [`.pi/architecture/modules/`](.pi/architecture/modules/).

---

## License

Licensed under either of MIT ([LICENSE-MIT](../LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](../LICENSE-APACHE)), at your option.
