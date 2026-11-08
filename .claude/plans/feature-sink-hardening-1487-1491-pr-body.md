# Batch 2: finish what the register could only describe

Batch 1 (#1478–#1481) made the sink register honest. It recorded three rows saying
`mitigation: none`, one saying "NOT yet enforced at this site", and stopped #1480's
third criterion at the engine boundary. This batch closes those four records and
carries the evidence verdict to both surfaces.

Order is dependency order: the reachable dispatcher first (it also touches both host
composition roots), then the two `run_command` sinks in execution order, then the
tools the register named, then the surface.

| Issue | ID | Priority | In one sentence |
|---|---|---|---|
| #1487 | SINK-4 | critical | the reachable step dispatcher required no declaration, and its only gate was optional |
| #1489 | SINK-5 | high | a caller-supplied `PATH` chose the executable the allowlist approved |
| #1488 | SINK-6 | medium | `cwd` was caller-supplied, so an allowlisted word ran another directory's config |
| #1490 | SINK-7 | medium | edit/patch re-resolved between check and write; a missing API key became a placeholder |
| #1491 | EVID-2 | medium | the engine computed the evidence verdict and neither host read it |

---

## #1487 — the reachable dispatcher declares its isolation

`RunCommandTool` was hardened in #1478 and **no shipping host constructs it**. The path
that runs a plan step is `execution_engine`'s dispatcher: a shell by design, with the
permission enforcer as its only gate — and that gate was `Option`, read as "skip the
check" when `None`. Three separate ways the control was not the control:

1. `exec_run_command` now requires the same `SandboxDeclaration` as the tool. Unset,
   unrecognized or `none` ⇒ `sandbox_undeclared`, before anything is spawned.
2. An absent enforcer is a refusal (`permission_enforcer_unavailable`), defined once so
   the two dispatch paths cannot describe it differently. The hosts stop swallowing the
   construction error: it is a startup failure in `cli`, `mcp` and the Action.
3. `PermissionMode::parse_host_value` is the one parser for all three hosts; an
   unrecognized value (`read-only`, `readOnly`, a typo) is an error naming the accepted
   set instead of being absorbed by `_ => WorkspaceWrite`.

The declared regime is recorded in the envelope's metadata (`isolation.regime`,
`isolation.requires`) — ADR-016 — so two runs under different boundaries are no longer
indistinguishable in the record.

**Breaking change:** a deployment that runs `run_command` steps must set
`RIGORIX_SANDBOX`. Recorded in `CHANGELOG.md` and `docs/deployment.md`.

| Acceptance | Proof |
|---|---|
| unset ⇒ no execution | `a_run_command_step_without_a_declaration_is_refused_before_any_spawn` — asserts the marker file is absent, not the failure type |
| declared ⇒ runs | `a_declared_regime_lets_the_step_run` |
| enforcer not optional | `PermissionEnforcerAbsent` (one definition, two paths) + the startup propagation in three hosts |
| a typo is not permissive | `an_unrecognized_permission_mode_is_an_error_naming_the_accepted_values` (cli), `…is_refused_with_the_accepted_values` (mcp), `an_unrecognized_permission_mode_fails_the_step` (action), plus engine-level parser tests |
| the register says so | `bash engine/.pi/scripts/validate-sink-register.sh` |

## #1489 — the environment cannot choose the executable

`apply_child_env` applied the caller's map verbatim, and `Command::new("echo")` resolves
a bare name through the *child's* `PATH`. Measured before the fix: the fake binary's
marker file existed and stdout was empty — the allowlist compared a word, the kernel ran
a file.

1. Reserved keys refused **by name, with the reason**: `PATH`, the loader families
   (`LD_*`, `DYLD_*`, `GCONV_PATH`), the interpreter startup hooks (`BASH_ENV`, `ENV`,
   `PYTHONSTARTUP`, `NODE_OPTIONS`, `PERL5OPT`, `RUBYOPT`), and `IFS`.
2. Every other key must be in the set the tool declared (`with_declared_env`, empty by
   default); an undeclared key is `InvalidInput` naming the declared set.
3. The program resolves in the engine's `PATH` **before** any override, and the resolved
   path is recorded (`ToolResult::resolved_executable`).

| Acceptance | Proof |
|---|---|
| PATH is not a lever | `a_caller_supplied_path_cannot_choose_the_executable` — asserts the marker, not the status |
| the loader is not a lever | `the_loader_families_are_refused_by_name` |
| unknown keys are errors | `an_undeclared_key_is_refused_and_the_declared_set_is_named` |
| the record names the binary | `a_declared_key_passes_and_the_result_names_the_executable` + `a_bare_name_resolves_in_the_engine_path_not_the_child_environment` |

## #1488 — `cwd` cannot leave the workspace

`fs_at::resolve_dir_for_child(root, cwd)` opens the root and walks each component with
`O_NOFOLLOW`, refuses `..` outright (accepting it means re-resolving to answer), refuses
an absolute path outside the root by name, and returns a `ValidatedChildDir` owning the
pinned descriptor. The child enters it with `fchdir` in `pre_exec` —
`Command::current_dir(path)` would hand the path to the child to resolve again, which is
the check/use split SINK-2 removed.

| Acceptance | Proof |
|---|---|
| absolute outside | `an_absolute_cwd_outside_the_workspace_is_denied_before_any_spawn` |
| `..` | `a_parent_component_in_cwd_is_denied` (even when it points back inside) |
| symlinked cwd | `a_symlinked_cwd_cannot_be_used_to_escape` |
| the legitimate case | `a_cwd_inside_the_workspace_still_works`, `the_default_cwd_is_the_workspace_root` |

## #1490 — edit, patch and the stores mutate through a handle

- `file_edit_tool` and `file_patch_tool` resolve once through `fs_at`, read through the
  same handle (`fs_at::read_file`, new) and write with `write_atomic`; both keep a source
  scan of the production section for `fs::write`/`fs::read_to_string`.
- `TemplateGenerationFactoryImpl` returns `GeneratorError::MissingApiKey { variable }`
  instead of substituting `"placeholder-key"`.
- **New guard rule:** a `mitigation: none` row must cite the open issue that owns the
  fix (`--self-test` seeds a row without one and requires rejection). Writing it surfaced
  five more `none` rows than the issue listed — the dispatcher's `file_write` step, the
  hooks config store, the approval store directory, and the policy and symbol-cache
  stores. All five now write atomically through `fs_at::resolve_absolute_for_write`, which
  creates the parent through the handle chain and propagates its failure instead of
  letting it surface as a confusing write error.
- Result: **the register has no `none` rows**, which was the acceptance criterion.

| Acceptance | Proof |
|---|---|
| edit/patch coupled | `a_symlinked_component_cannot_redirect_the_edit` / `…_the_patch` (+ `the_tool_does_not_write_paths_itself`) |
| no raw mutation | the source scans above |
| the key fails closed | `an_absent_api_key_is_a_named_error_not_a_placeholder` |
| no `none` without a ticket | `validate-sink-register.sh --self-test` seed G |

## #1491 — the verdict reaches both surfaces

- CLI: the status bar prints `evidence: complete` / `evidence: degraded — <reason>
  (<n> undelivered)` beside the run's phase, red and bold when degraded.
- MCP: `rigorix_execute` returns `evidence: { status, reason?, undelivered? }`.
- `not_requested` is always stated, never omitted: an absent field is indistinguishable
  from a client that does not know the field — which is how the defect stayed invisible.

Tests assert the **rendered string** and the **serialised response**: four MCP tests
drive the real `ExecuteHandlerImpl` through a facade returning each variant and parse the
JSON body; three CLI tests call the status bar's label function.

---

## What this does not do

- It does not implement a sandbox, and it does not verify that `RIGORIX_SANDBOX=container`
  is true. It makes the deployment state the fact and refuses the step when it does not.
- The dispatcher's `sh -c` sink still executes a shell string; `exec_run_command`'s
  register row says so. The boundary is the declaration, not an allowlist — and the shell
  resolves its own argv, so that path cannot record the resolved binary the way the tool
  does.
- `cwd` resolution does not stop a process with write access inside the root from
  swapping a component *before* the resolution runs; the control is over the window
  between validation and spawn, which is the window that existed.
- The reserved env list is not exhaustive by construction: rule 2 (declared keys only) is
  what closes the class, and the reserved list is there to give the specific reason.
- `resolve_absolute_for_write` canonicalizes the parent, so engine-owned stores follow
  symlinks in their configured path. That is deliberate — the path comes from the
  operator's configuration, not from a caller — and the register row says so.

## Validation

```
cargo build --workspace                                        PASS
cargo test --workspace --all-targets --no-fail-fast            PASS (98 test binaries)
cargo clippy --workspace --all-targets -- -D warnings          PASS
cargo fmt --check                                              PASS
cargo audit                                                    no advisories against the lockfile
cargo check -p rigorix-engine --features portable              PASS
validate-portable-inventory.sh --gates                         PASS
.pi/scripts/validate-guards.sh (+ --self-test)                  PASS (gating + 7 seeds)
.pi/scripts/validate-unsafe-sites.sh / validate-panic-sites.sh   PASS (18 / 37 sites, all baselined)
.pi/scripts/validate-docs.sh                                     PASS
engine/.pi/scripts/validate-{canonical,architecture,architecture-readiness,ci,tests,security,operations,integration}.sh   PASS
```

Closes #1487, #1488, #1489, #1490, #1491.
