# Public text may name only published repositories

The issue asked for an allowlist so that a private repository's name fails **by absence** — a denylist cannot live in a public repository, because writing the private names down is the leak it prevents. The complement can.

## The premise moved while the issue was open, and I measured both states

The issue's example was `rigorix-sdk`, which this tree called "same-owner private". Measured 2026-11-11:

| repository | `gh repo view --json visibility` | anonymous GET |
|---|---|---|
| `rigorix-oss` | PUBLIC | 200 |
| `rigorix-sdk` | **PUBLIC**, pushed 2026-11-11T16:33Z | **200** |
| the private enterprise repository | **PRIVATE** | **404** |

and `rigorix-verifier` 0.10.0 is anonymously installable (crates.io 200 with a UA, PyPI 200, npm 200).

So concealing the `ci.yml` literal would have spent a repository variable and an "unset" failure mode to hide a dependency anyone can `git clone`, and would have renamed `RIGORIX_SDK_SCHEMAS` — a documented name in the SDK's own `rust/RELEASING.md` §3 — for cross-repo drift and no gain. The real instance was in a different file: `anchor-e2e.yml` reached the **private** repository through a literal owner/name with a PAT in the clone URL.

## The check

`.pi/scripts/validate-public-references.sh` — every repository-shaped token in governed text must be registered in `.pi/public-repositories.tsv` with a kind and evidence.

- **The registry is the subject list**, not a pattern compiled into the script. Deleting the `rigorix-sdk` row turns **112** mentions into findings — the seed that proves the registry decides rather than the shape.
- A `repository` row must carry the `gh repo view`/`gh repo list` result **and** the word public; a row typed from memory is refused (exit 2). That is the issue's criterion "derived from the org's public repositories, not typed from memory", enforced rather than stated.
- Kinds separate the nouns that keep getting conflated: `repository`, `artifact` (a published crate is not a repository), `workspace-package`, `proposed-crate`, `rejected`, `identifier` (a fixture, env prefix, hook name or script name owns nothing).
- Governed = `.md`, `.tsv`, `.yml`, `.yaml`, **workflows included** — no `.github/**` exemption, because the one instance that matters lives there. Code is out, and that hole is named in the script: `engine/src/**` is full of temp-directory names shaped like the organisation's (`std::env::temp_dir()` fixtures), and a fixture is not a claim about whose repository something is.
- Two exit codes: malformed registry (2) and violation (1), because the remedies differ.

## The wiring

- **`anchor-e2e.yml`**: `vars.ENTERPRISE_REPOSITORY`, clone to `$RUNNER_TEMP/enterprise`, `ENTERPRISE_DIR` likewise, plus a second skip branch that names the missing setting instead of failing on `arman-jalili/`. Still `workflow_dispatch` + weekly, still skips cleanly.
- **`ci.yml`**: `rigorix-sdk` is public, so the conformance stage clones it with the workflow's own `contents: read`; `secrets.RIGORIX_SDK_TOKEN` is deleted. One fewer long-lived PAT — and the stage can now run on a **fork PR**, where a secret does not exist and the job previously read as an infrastructure error. `repository:`, `path:`, `ref: main`, `RIGORIX_SDK_SCHEMAS` and `--manifest-path rigorix-sdk/rust/…` stay.
- **`ci-affected.tsv`**: the `rigorix-sdk/**` row is deleted — no diff in this repository can contain that path, so it could never select a job, while `.github/**` (ALL) and `schemas/**` already select `conformance`.
- **`demo/anchor-e2e/{run.sh,docker-compose.yml}`** default to `../enterprise`, matching what the workflow produces.

## The sweep

33 files, 47 mentions: the private repository's name — and the two proposed crates that carried it as a prefix — now read as prose. Every rewrite keeps the sentence's subject; no decision, number or measurement moved.

## Acceptance

| # | Criterion | Command | Result |
|---|---|---|---|
| 1 | the tree passes | `validate-public-references.sh` | ✅ 49 registered names, 1654 governed files |
| 2 | an unregistered name fails | plant it in a workflow copy | ✅ exit 1, naming `file:line: token` |
| 3 | the same shape, registered, passes | plant `rigorix-sdk` | ✅ exit 0 |
| 4 | crate names are not repositories | `rigorix-verifier` in prose | ✅ passes (`artifact`) |
| 5 | an unpublished `*-demo` fails | plant one | ✅ exit 1 |
| 6 | the registry is evidence | a `repository` row without a measured result | ✅ exit 2, and says to run the command |
| 7 | the registry decides, not the shape | delete the `rigorix-sdk` row | ✅ 112 findings |
| 8 | registered gating | `.pi/guards.tsv` | ✅ `gating`, `run_here=yes` |
| 9 | no workflow exemption | governed set | ✅ includes `.github/workflows/**` |
| 10 | conformance green without the PAT | the `📐 SDK-schema conformance` job on this PR | see CI |
| 11 | `anchor-e2e.yml` parses | `validate-workflows.sh` | ✅ (it does not execute on a PR, by design) |

## What this does not do

- It does not touch git history, where the name predates this branch. Removing it there is a rewrite decision, not a docs sweep.
- It does not rename `ENTERPRISE_REPO_TOKEN`: the workflow still needs it, and a secret's value cannot be read back to re-create it under another name.
- `demo/anchor-e2e/run.sh` still spells the enterprise binary by the name that repository's own crate builds. It is a path in someone else's build output, and `.sh` is outside the governed set — the same limit stated for source code.
- Source code is out of the governed set, deliberately and stated in the script.

Closes #1513.
