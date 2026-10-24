# Changelog

All notable changes to Rigorix are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is the in-repo home for release history — the README is a product page
and links here. Each entry lists the pull requests merged and the issues closed in
that version's window, derived from git and the issue tracker, so the entries and
the tags cannot disagree. Backfilled 2026-10-15.

The release train is `rigorix-engine` / `rigorix-mcp` / `rigorix-cli` /
`rigorix-actions`, which version together. `rigorix-actions` is the adapter crate and
the Docker image behind `action.yml`; its version is the version of the engine it
adapts. It was outside the train until 1.11.0, which is why its numbers jump from
1.4.0 to 1.11.0 — for seven weeks it was skipped as "already published".

## [Unreleased]

### Fixed

- `rigorix-actions` joins the release train: it had been excluded from the version gate, so nothing bumped it and `publish-crate.sh` skipped it as already published for four releases (1.4.0, uploaded 2026-09-06, while `actions/` gained 307 lines). Its stale `rigorix-engine` pin is now guarded by `validate-workspace-pins.sh`, and 1.11.0 is published. The generated release notes, README, CONTRIBUTING, RELEASING, and the action README now all say the same thing: four crates, one version (#1216) ([#1217](https://github.com/arman-jalili/rigorix-oss/pull/1217))

- The `receipt_reader` config example names the settlement axis (`none` without a reader) separately from durability, and the consequence-gating spec states `1.9.0 and later` instead of stopping at 1.10.0 (#1214) ([#1215](https://github.com/arman-jalili/rigorix-oss/pull/1215))

## [1.11.0] - 2026-10-24

[1.11.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.11.0

### Added

- **Capacity ledger bounded context (ADR-018):** frozen contract, in-process and HTTP adapters, intent-derived keys, service-owned `ReservationGuard`, a rolling exposure window, and the ambiguous lapsed-lease state (#1020–#1034, #1049–#1054) ([#1035](https://github.com/arman-jalili/rigorix-oss/pull/1035), [#1065](https://github.com/arman-jalili/rigorix-oss/pull/1065))
- **Effect commitment bounded context (ADR-019):** write-ahead record, four-state machine with non-terminal `Ambiguous`, resolver authority, dependent and retry fences, L1/L2/L3 adapters, armed in all three composition roots, with a runbook and DR plan (#1112–#1135) ([#1136](https://github.com/arman-jalili/rigorix-oss/pull/1136), [#1162](https://github.com/arman-jalili/rigorix-oss/pull/1162))
- Enforcement budgets are evaluated before every tool call on both dispatch paths; recovery recipes are opt-in (#1108) ([#1111](https://github.com/arman-jalili/rigorix-oss/pull/1111))
- `rigorix doctor`, an `init` seam inventory, and `rigorix key` that emits the HMAC key the engine reads (#1101–#1103) ([#1105](https://github.com/arman-jalili/rigorix-oss/pull/1105)–[#1107](https://github.com/arman-jalili/rigorix-oss/pull/1107))
- Process-spawn guard: clippy denies spawning outside the approved sinks, one mechanical guard per escaped failure class, and stable baselines (#1082, #1083, #1089) ([#1084](https://github.com/arman-jalili/rigorix-oss/pull/1084)–[#1088](https://github.com/arman-jalili/rigorix-oss/pull/1088))
- Plan creation declares the effect scope of file writes, patches, and stages, so the R5 oracle can report an out-of-scope effect (#1175) ([#1180](https://github.com/arman-jalili/rigorix-oss/pull/1180))
- `rigorix doctor` reports the effect-commitment seam and its derived ceiling (#1176) ([#1180](https://github.com/arman-jalili/rigorix-oss/pull/1180))
- `doctor` reports durability and settlement as separate axes, with the restart cost (#1182) ([#1184](https://github.com/arman-jalili/rigorix-oss/pull/1184))
- The approve surface accepts an optional approver-declared scope, merged with the planner's and covered by the intent hash; opt-in strict mode refuses an empty scope (#1190) ([#1194](https://github.com/arman-jalili/rigorix-oss/pull/1194))

### Fixed

- Effect-commitment runtime gaps: capacity holds reuse the intent key, orphaned `Dispatched` records are swept, the intent key reaches process providers, and an unreachable `http` backend is refused at startup (#1163–#1166) ([#1167](https://github.com/arman-jalili/rigorix-oss/pull/1167))
- The `Dispatched` sweep is gated on ledger ownership, so an L2 client cannot judge another process's clock (#1173) ([#1180](https://github.com/arman-jalili/rigorix-oss/pull/1180))
- The provider contract states the implemented process channel and marks `Idempotency-Key` reserved (#1174) ([#1180](https://github.com/arman-jalili/rigorix-oss/pull/1180))
- Recovery reports `Unsupported` instead of `Recovered` for repairs it cannot perform (#1157) ([#1168](https://github.com/arman-jalili/rigorix-oss/pull/1168))
- `--enforcement` is refused with an actionable message, and the refusal renders without gaps (#1156, #1171) ([#1169](https://github.com/arman-jalili/rigorix-oss/pull/1169), [#1172](https://github.com/arman-jalili/rigorix-oss/pull/1172))
- Process-spawn allow comments match the code; a hook that closes stdin before reading is a normal early exit (#1159) ([#1170](https://github.com/arman-jalili/rigorix-oss/pull/1170))
- Run-command rejects compound shell strings and clears the child environment (#1067, #1068); approval binding refuses without an HMAC key (#1070); the scope oracle is wired in all roots and sees staged changes (#1072, #1099); the anchor label requires a verified head (#1071) ([#1084](https://github.com/arman-jalili/rigorix-oss/pull/1084)–[#1106](https://github.com/arman-jalili/rigorix-oss/pull/1106))
- TLS trust path takes the two 2026-10 RUSTSEC fixes (#1110) ([#1110](https://github.com/arman-jalili/rigorix-oss/pull/1110))
- The wiring claim for the `Dispatched` sweep states both branches instead of "every root sweeps" (#1183) ([#1184](https://github.com/arman-jalili/rigorix-oss/pull/1184))
- The cli and server event bridges classify `EffectScopeChecked`, and a workspace compile gate catches the next unhandled public-enum variant (#1195) ([#1197](https://github.com/arman-jalili/rigorix-oss/pull/1197))
- Dependency audit is green again: `ctutils` is 0.4.4, and `ratatui` 0.30.2 removes `lru` 0.12 and `paste`; the remaining unmaintained advisories are reviewed ignores in `.cargo/audit.toml` with tracking issues and an expiry (#1198) ([#1201](https://github.com/arman-jalili/rigorix-oss/pull/1201))
- `rigorix-actions` parses YAML with `serde-saphyr` instead of the unmaintained `serde_yaml`/`unsafe-libyaml` (#1199); `rigorix-mcp` uses `keyring-core` with the Linux kernel keyutils store instead of the unmaintained `zbus`/`zvariant` chain (#1200) ([#1204](https://github.com/arman-jalili/rigorix-oss/pull/1204))
- `local-ci --stage security` runs the same `cargo audit --deny warnings` as Hardening (#1202); `validate-decision-expiry.sh` fails a lapsed advisory ignore or an invalid `docs/issues` status, with a self-test (#1203) ([#1204](https://github.com/arman-jalili/rigorix-oss/pull/1204))
- `action.yml` YAML 1.1 scalar resolution is documented and pinned by tests: `yes/no/on/off/y/n` are booleans, `010` is a number, duplicate keys fail loudly (#1205); `local-ci` rejects unknown arguments and stage names instead of running the whole suite (#1206) ([#1207](https://github.com/arman-jalili/rigorix-oss/pull/1207))
- `tokio` 1.52.3 → 1.54.1 closes RUSTSEC-2026-0386 (`AsyncFd` I/O safety); the strict audit gate caught it locally (#1207) ([#1207](https://github.com/arman-jalili/rigorix-oss/pull/1207))
- Server HTTP/SSE is now exercised end-to-end over a real socket, including a `Last-Event-ID` reconnect, and the action binary is driven with a fixture workflow environment and asserted through its outputs and exit code (#1209) ([#1210](https://github.com/arman-jalili/rigorix-oss/pull/1210))
- The dispatch path has a criterion baseline: `enforcement_path` measures `execute_graph` with the effect seam absent and present at 1/10/100 nodes, with the numbers recorded in the execution-engine module doc (#1208) ([#1211](https://github.com/arman-jalili/rigorix-oss/pull/1211))
- The dispatch benchmark's module doc now records the method and its noise floor instead of a non-reproducing table: the seam delta was below the harness's resolution at N ≤ 100, and criterion defaults (not 50 samples) are what the bench runs (#1212) ([#1213](https://github.com/arman-jalili/rigorix-oss/pull/1213))
- An armed commitment seam no longer disables retries: a `NotCommitted` attempt is retryable as a new attempt record under the same intent key, while `Committed` and `Ambiguous` stay refused (#1196) ([#1197](https://github.com/arman-jalili/rigorix-oss/pull/1197))
- `execute_node` runs the effect write-ahead and settle through the shared gate path (#1185) ([#1191](https://github.com/arman-jalili/rigorix-oss/pull/1191))
- The intent key is derived once and used by the record, provider, and receipt reader; the placeholder is gone (#1187) ([#1191](https://github.com/arman-jalili/rigorix-oss/pull/1191))
- The signed envelope records whether the scope check ran (`checked` / `not_declared` / `unavailable`), not only whether it found violations (#1189) ([#1194](https://github.com/arman-jalili/rigorix-oss/pull/1194))

### Internal

- The effect-commitment and capacity-ledger contract proofs gate through `.pi/guards.tsv` and `.pi/scripts/validate-guards.sh`, with seeded-failure self-tests (#1186) ([#1192](https://github.com/arman-jalili/rigorix-oss/pull/1192))
- Nine effect-commitment issue packets are `done` with a `closed_reason`, and `engine/.pi/issues/README.md` states the closing convention (#1188) ([#1193](https://github.com/arman-jalili/rigorix-oss/pull/1193))

<details><summary>Issues closed in this release (112)</summary>

- #1212 — the recorded enforcement-path baseline does not reproduce: the 26% seam cost re-measures as 3% inside overlapping CIs, and the doc claims 50 samples the bench never sets
- #1208 — no benchmark covers the enforcement path: criterion measures topological_sort and seal_graph, not the five gates every dispatch passes
- #1209 — the server HTTP/SSE surface and the actions entrypoint have no end-to-end test: 6 of 190 test files spawn a process, 1 binds a socket
- #1206 — local-ci silently ignores unknown arguments, so a typo runs the whole suite instead of one stage
- #1205 — the YAML parser swap silently changes scalar resolution: yes/no/on/off become booleans, 010 becomes 10.0, duplicate keys now hard-error
- #1203 — decision expiry is a comment and issue front matter is unvalidated: nothing reads `.cargo/audit.toml` dates or `docs/issues/**` status
- #1202 — the strict audit gate has no local stand-in: local-ci runs plain `cargo audit`, Hardening denies warnings
- #1200 — mcp: review the Linux keyring chain (zbus/zvariant unmaintained; RUSTSEC-2026-0370/71/72)
- #1199 — actions: migrate off serde_yaml 0.9 (unmaintained; RUSTSEC-2024-0449)
- #1198 — main is red for dependency advisories, not code: ctutils 0.4.2 (patch available) fails Audit, and Hardening --deny warnings fails on five unmaintained/unsound crates
- #1196 — with effect-commitment armed, a node cannot retry after a definite failure: the fence allows NotCommitted, begin_effect refuses every terminal record
- #1195 — main is red: ExecutionEvent::EffectScopeChecked was added without updating the two event bridges, failing 9 CI and Hardening jobs
- #1190 — the approve surface cannot declare a scope, so run_command steps have no way to be verified — the runbook says the approve surface may set one
- #1189 — the signed envelope records scope violations but not whether the check ran — "no violations" is indistinguishable from "never checked"
- #1188 — nine engine issue packets for effect-commitment still say status:planned after the epic merged, and closed_reason was not applied
- #1187 — the provider and the receipt reader use the placeholder {dag_id}:{node_id}:pending, not the intent-derived key — a changed payload collides
- #1186 — the 150-check contract proof never runs in CI: root-only script, crate-scoped loops, and a root loop that is continue-on-error + || true
- #1185 — execute_node runs the shared gate sequence but not the effect-commitment write-ahead — the #1074 divergence, one layer down
- #1183 — W17 says every composition root sweeps orphaned Dispatched records; at L2 none does, and the guard only checks that the evidence path exists
- #1182 — the ceiling is one scalar for two orthogonal axes: doctor prints L3 for a deployment with no durability, and derive's comment states a rule the code does not implement
- #1178 — the CHANGELOG's Unreleased section is empty although a whole context and three fix batches landed since 1.10.0
- #1177 — the OSS wiring-claims manifest has no effect-commitment row, and three shipped modules still say Status: Planned
- #1176 — doctor reports every seam except effect-commitment, so the newest enforcement boundary and its claim ceiling are invisible
- #1175 — the effect-scope oracle is wired in every root and nothing produces a declared_scope — the scope control cannot fire, and the docs call it by design
- #1174 — the provider contract says HTTP providers receive Idempotency-Key, but no execution path sends HTTP — the only sender is a test
- #1173 — the Dispatched sweep runs in every client root against the local clock; at L2 the service must own it, or roots fence each other live work
- #1171 — the --enforcement refusal renders with 18-space gaps, and the comment beside the working path still says the flag selects a preset
- #1166 — backend.kind = "http" points at a ledger service that does not exist in OSS, and no deployment ships a config — every run is L1 and the L2 path is unreachable
- #1165 — the intent key is never sent to the provider, so the receipt reader idempotency-key lookup can only miss — L3 is unreachable by construction
- #1164 — nothing reconciles Dispatched records after a crash: may_retry only blocks Ambiguous, so a resumed run re-dispatches an effect that may already have happened
- #1163 — capacity holds use a per-attempt UUID while the effect record uses an intent key: a crash after reserve leaves an unrecoverable hold, and a resumed dispatch holds twice
- #1160 — .pi/INDEX.md and the architecture-coordinator agent reference four files that exist under no path — decide delete or instantiate
- #1159 — git_read_tool: the comment justifying its process-spawn allow says sh -c, but the code is argv-only — the review note contradicts the thing it certifies
- #1157 — recovery: execute_step returns Ok for five repairs without performing them — the seam is wired, reports success, and repairs nothing
- #1156 — cli: --enforcement is a documented override that is threaded and never read — passing it changes nothing
- #1135 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #1134 — Proofing: Validation scripts + CI integration
- #1133 — Implement CompositionRoots — effect-commitment
- #1132 — Implement NodeStateVocabulary — effect-commitment
- #1131 — Implement RetryFence — effect-commitment
- #1130 — Implement DependentFence — effect-commitment
- #1129 — Implement DispatchSeam — effect-commitment
- #1128 — Implement EffectCommitmentLedger — effect-commitment
- #1127 — Implement DefaultEffectCommitmentFactory — effect-commitment
- #1126 — Implement EffectCommitmentConfig — effect-commitment
- #1125 — Implement ProviderReceiptReader — effect-commitment
- #1124 — Implement HttpEffectLedger — effect-commitment
- #1123 — Implement InMemoryEffectLedger — effect-commitment
- #1122 — Implement ReconciliationPolicy — effect-commitment
- #1121 — Implement IntentKeyDerivation — effect-commitment
- #1120 — Implement EffectCommitmentService — effect-commitment
- #1119 — Implement CommitmentError — effect-commitment
- #1118 — Implement ResolverIdentity — effect-commitment
- #1117 — Implement ObservedOutcome — effect-commitment
- #1116 — Implement EffectRecord — effect-commitment
- #1115 — Implement EffectState — effect-commitment
- #1114 — Implement EffectIntent — effect-commitment
- #1113 — Contract Freeze: Define interfaces and contracts
- #1112 — Epic: effect-commitment
- #1108 — enforcement/recovery: wire the two modules that no entry point constructed (budgets pre-dispatch, recovery opt-in)
- #1104 — integration: the MCP gateway is the only adapter surface — record whether that is the ceiling or a gap
- #1103 — cli: no `rigorix doctor` — nothing can answer 'is enforcement actually armed in this repo?'
- #1102 — cli: `rigorix key` prints a random UUID that nothing consumes — 26 readers of RIGORIX_HMAC_KEY, zero of this value
- #1101 — cli: `rigorix init` scaffolds no enforcement configuration and names none of the env vars that arm it
- #1100 — audit: verify every module doc and ADR claim of end-to-end wiring against the call graph, then fix or delete what does not hold
- #1099 — approval: the effect-scope oracle still ships disabled in all three entry points — #1072 closed COMPLETED with the wiring half deferred
- #1096 — .pi/context/domain-workflow.md points at two .pi/ paths that live under engine/ — one also names the wrong file
- #1094 — demo/capacity-ledger/README.md points at two paths that do not exist — both are missing the engine/ prefix
- #1090 — the fix for retry-replaces-guard (found by the capacity demo) has no test — a future edit to the guard registry silently reopens it
- #1089 — the panic baseline is still keyed by file:line, so any edit above a site reads as a new panic — the unsafe baseline was re-keyed, this one was not
- #1083 — clippy.toml: ban process-spawn types and methods outside the approved module, so a new spawn site cannot bypass the shell policy
- #1082 — spike: catalogue the failure classes that escaped gates and reviews, and build one guard per class
- #1081 — tests: three precondition runner tests spawn /bin/sh on a wall-clock budget and time out under full-suite parallelism
- #1074 — execution-engine: the public execute_node trait method runs a node without the approval, precondition, sequence or capacity gates
- #1073 — audit: verify_authority_fact_bundle proves the projection matches the supplied history, not that the history came from the anchor
- #1072 — approval: the git effect-scope oracle is disabled in every production entry point, and its diff misses staged changes and uses one baseline per loop
- #1071 — audit: an envelope is labelled HistoryIntegrity::Anchored whenever the anchor is in anchored mode, even when no verified head exists
- #1070 — approval: RIGORIX_APPROVAL_BINDING=1 with no HMAC key logs a warning and silently falls back to unbound approval
- #1069 — sequence_policy: the R3 companion obligation is satisfied by a step existing anywhere in the plan — not by ordering, dependency, or success
- #1068 — run_command: the child process inherits the engine environment, including RIGORIX_HMAC_KEY and anchor credentials
- #1067 — run_command: the allowlist checks a prefix of a compound shell string, and check_bash is fed the JSON intent instead of the command
- #1066 — enforcement: the BudgetScope docs instruct operators to use shared_contested, which validation always refuses
- #1061 — enforcement: BudgetScope::SharedContested defers the limit and nothing takes over — no binding to a capacity-ledger resource
- #1060 — docs: ADR-018 and the gap ledger still claim the deleted persistence ports exist
- #1054 — capacity_ledger: ReservationGuard drop performs a fire-and-forget async release and swallows the failure
- #1053 — capacity_ledger: 'single-execution atomicity' understates the in-process guarantee, and the cross-session case is untested
- #1052 — capacity_ledger: reusing an idempotency key with a different amount silently returns the original reservation
- #1051 — capacity_ledger: ResourceSpec.window is accepted and echoed but never enforced — committed exposure never ages out
- #1050 — capacity_ledger: a lapsed lease is treated as proof the effect did not happen (no ambiguous-outcome state)
- #1049 — capacity_ledger: ReserveCapacityOutput::guard is always None — the DTO advertises an RAII handle that does not exist
- #1034 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #1033 — Proofing: Validation scripts + CI integration
- #1032 — Implement CapacityLedger — capacity-ledger
- #1031 — Implement HttpLedger — capacity-ledger
- #1030 — Implement InProcessLedger — capacity-ledger
- #1029 — Implement ReservationGuard — capacity-ledger
- #1028 — Implement CapacityLedgerService — capacity-ledger
- #1027 — Implement LedgerError — capacity-ledger
- #1026 — Implement IdempotencyKey — capacity-ledger
- #1025 — Implement Exposure — capacity-ledger
- #1024 — Implement Grant — capacity-ledger
- #1023 — Implement Reservation — capacity-ledger
- #1022 — Implement Resource — capacity-ledger
- #1021 — Contract Freeze: Define interfaces and contracts
- #1020 — Epic: capacity_ledger
- #1019 — enforcement: resolve the 'reserved for a future implementation' per-tool call counter
- #1018 — [deferred] README: restore a runnable demo once the capacity-ledger demo exists
- #1017 — enforcement: ResourceBudget is a counter, not a reservation — supersede it via the capacity ledger
- #1016 — persistence ports: LlmBudgetRepository and CapacityLedgerRepository are both declared and neither has a caller
- #1015 — design: distributed capacity ledger — cross-session aggregate enforcement (ADR-018)
- #1013 — Competitive positioning: replace the category-error comparison table, and give release notes a durable home
- #1012 — Documentation hygiene: the README must convert, and every doc must be verified against code

</details>


## [1.10.0] - 2026-10-14

[1.10.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.10.0

### Added

- Production panic surface + self-owned guard gating (#997, #998) ([#999](https://github.com/arman-jalili/rigorix-oss/pull/999))
- Consequence-gating batch — spec, providers, fact projection (#1000–#1002) ([#1003](https://github.com/arman-jalili/rigorix-oss/pull/1003))
- Surface the authority-fact projection; make it re-derivable (#1001, #1006) ([#1008](https://github.com/arman-jalili/rigorix-oss/pull/1008))
- Serve the frozen authority-fact method (#1010, #1009) ([#1011](https://github.com/arman-jalili/rigorix-oss/pull/1011))

### Fixed

- Validator single source of truth + a deterministic staleness test (#1004, #1005) ([#1007](https://github.com/arman-jalili/rigorix-oss/pull/1007))

<details><summary>Issues closed in this release (10)</summary>

- #997 — Production panic surface: 41 baselined sites; poisoned-lock panics in server are the reachable class
- #998 — Self-owned vulnerability checks: make the guards that exist actually gate (and grow the set)
- #1000 — Precondition providers: an operator-configured external determination service as a precondition, never an inherited verdict
- #1001 — Authority-fact projection: a third-party-verifiable export of signed history as attributable present-state facts
- #1002 — Publish Consequence Gating as a versioned, citable specification with a falsifiable validation protocol and explicit claim ceilings
- #1004 — The mirrored .pi/scripts validators have no single source of truth — five trees drifted into 2–4 variants each
- #1005 — Flaky test: mcp test_schema_cache_is_stale asserts on a zero-duration window, so it races the clock
- #1006 — Authority-fact projection surfaces: CLI command + MCP tool (the MCP tool is a frozen-catalog change across three repos)
- #1009 — Server: add the frozen rigorix.audit.authorityFacts method to the catalog and expose its MCP tool
- #1010 — MCP: implement the frozen rigorix_audit_authority_facts tool (descriptor + host routing)

</details>

## [1.9.5] - 2026-10-11

[1.9.5]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.9.5

### Added

- Structured attribution reason on findings ([#995](https://github.com/arman-jalili/rigorix-oss/pull/995))
- Optional MCP Streamable HTTP adapter ([#996](https://github.com/arman-jalili/rigorix-oss/pull/996))

<details><summary>Issues closed in this release (2)</summary>

- #915 — OSS-MCP-STREAMABLE-HTTP: optional MCP Streamable HTTP transport (trigger-gated, ADR-0001 D4)
- #994 — Attribution absence has three causes and one empty state — add a structured reason code (contract change)

</details>

## [1.9.4] - 2026-10-08

[1.9.4]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.9.4

## [1.9.3] - 2026-10-08

[1.9.3]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.9.3

## [1.9.2] - 2026-10-08

[1.9.2]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.9.2

### Added

- Bind the trust boundary to the check artifact, not argv[0] ([#993](https://github.com/arman-jalili/rigorix-oss/pull/993))

### Changed

- 984: document the authority boundary as a mode → guarantee matrix ([#988](https://github.com/arman-jalili/rigorix-oss/pull/988))
- 985: flip the --isolated boundary row to available ([#989](https://github.com/arman-jalili/rigorix-oss/pull/989))
- 986: refuse a check writable by the engine's euid (require_immutable_check) ([#990](https://github.com/arman-jalili/rigorix-oss/pull/990))
- 987: bind check + authority digests into signed precondition evidence ([#991](https://github.com/arman-jalili/rigorix-oss/pull/991))

<details><summary>Issues closed in this release (5)</summary>

- #984 — Document the authority boundary as a per-mode strength matrix (not a snapshot claim)
- #985 — Ship an isolation recipe: setup-authority.sh --isolated (root-owned, non-writable)
- #986 — Engine: detect a check writable by the agent's UID and refuse/flag it
- #987 — Signed attestation: bind the check identity + authority digest into the envelope (OSS + SDK)
- #992 — Interpreter-based checks bypass the trust boundary and mis-bind attribution (command[0] only)

</details>

## [1.9.1] - 2026-10-06

[1.9.1]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.9.1

## [1.9.0] - 2026-10-06

[1.9.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.9.0

### Added

- Present a bearer credential to the anchor (RIGORIX_ANCHOR_TOKEN) — fixes OSS↔enterprise wire ([#924](https://github.com/arman-jalili/rigorix-oss/pull/924))
- #912 OSS-ANCHOR-WIRE-E2E — live anchored-path E2E + demo ([#925](https://github.com/arman-jalili/rigorix-oss/pull/925))
- ACT-1 PreconditionSetup + factory wiring (ISSUE-PF-ACT-1) ([#974](https://github.com/arman-jalili/rigorix-oss/pull/974))
- ACT-2 wire PreconditionSetup into composition roots ([#975](https://github.com/arman-jalili/rigorix-oss/pull/975))

### Changed

- [Issue #938] Contract Freeze: consequence-gating ([#952](https://github.com/arman-jalili/rigorix-oss/pull/952))
- [Issue #939] Implement Precondition — consequence-gating ([#953](https://github.com/arman-jalili/rigorix-oss/pull/953))
- [Issue #940] Implement PreconditionRepository — consequence-gating ([#954](https://github.com/arman-jalili/rigorix-oss/pull/954))
- [Issue #941] Implement PreconditionRunner — consequence-gating ([#955](https://github.com/arman-jalili/rigorix-oss/pull/955))
- [Issue #942] Implement PreconditionService — consequence-gating ([#956](https://github.com/arman-jalili/rigorix-oss/pull/956))
- [Issue #943] Implement DispatchGate — consequence-gating ([#957](https://github.com/arman-jalili/rigorix-oss/pull/957))
- [Issue #944] Implement PreconditionFinding — consequence-gating ([#958](https://github.com/arman-jalili/rigorix-oss/pull/958))
- [Issue #945] Implement GatingMode — consequence-gating ([#959](https://github.com/arman-jalili/rigorix-oss/pull/959))
- [Issue #946] Implement CompanionStepObligation — consequence-gating ([#960](https://github.com/arman-jalili/rigorix-oss/pull/960))
- [Issue #947] Implement PreconditionError — consequence-gating ([#961](https://github.com/arman-jalili/rigorix-oss/pull/961))
- [Issue #948] Implement PreconditionSurfaces — consequence-gating ([#962](https://github.com/arman-jalili/rigorix-oss/pull/962))
- [Issue #949] Implement HardeningConfig — consequence-gating ([#963](https://github.com/arman-jalili/rigorix-oss/pull/963))
- [Issue #950] Proofing & CI Enforcement — consequence-gating ([#964](https://github.com/arman-jalili/rigorix-oss/pull/964))
- [Issue #951] Architecture Readiness — consequence-gating ([#965](https://github.com/arman-jalili/rigorix-oss/pull/965))

### Fixed

- #913 OSS-SERVER-SSE-AUTH — gate GET /events ([#919](https://github.com/arman-jalili/rigorix-oss/pull/919))
- Make architecture + canonical workspace- and cwd-aware ([#927](https://github.com/arman-jalili/rigorix-oss/pull/927))
- Re-home R2/R3 contract checks after REL relocations ([#981](https://github.com/arman-jalili/rigorix-oss/pull/981))
- Preserve finding arrays on approval-resume re-emission ([#983](https://github.com/arman-jalili/rigorix-oss/pull/983))

### Docs

- REN-1 rename module doc to precondition.md + refs ([#977](https://github.com/arman-jalili/rigorix-oss/pull/977))

### Internal

- Bump rigorix-verifier to 0.2 (SDK anchored verification) ([#918](https://github.com/arman-jalili/rigorix-oss/pull/918))
- #917 OSS-REFACTOR-TESTS — externalize the 3 in-module test files ([#920](https://github.com/arman-jalili/rigorix-oss/pull/920))
- #916 OSS-REFACTOR-ORCHESTRATOR — split orchestrator_impl.rs ([#921](https://github.com/arman-jalili/rigorix-oss/pull/921))
- Split execution_engine/service_impl.rs (Tier 1 #2) ([#922](https://github.com/arman-jalili/rigorix-oss/pull/922))
- Extract generator.rs scanning helpers (Tier 2 #4) ([#923](https://github.com/arman-jalili/rigorix-oss/pull/923))
- ACT-3 end-to-end composition reachability + evidence ([#976](https://github.com/arman-jalili/rigorix-oss/pull/976))
- REN-2 rename source refs, tests dir, and CI scripts ([#978](https://github.com/arman-jalili/rigorix-oss/pull/978))
- REL-1 relocate R2 GatingMode out of precondition ([#979](https://github.com/arman-jalili/rigorix-oss/pull/979))
- REL-2 relocate R3 companion obligation into requirements ([#980](https://github.com/arman-jalili/rigorix-oss/pull/980))

<details><summary>Issues closed in this release (39)</summary>

- #912 — OSS-ANCHOR-WIRE-E2E: live anchored-path E2E — rigorix-server anchor client ↔ enterprise Execution API (+ demo)
- #913 — OSS-SERVER-SSE-AUTH: gate `GET /events` (fail-closed) or enforce localhost-only
- #914 — OSS-REFACTOR-PROGRAM: 19 files >1000 LOC — validated inventory + risk-ordered sequencing
- #916 — OSS-REFACTOR-ORCHESTRATOR: split orchestrator_impl.rs (4166 LOC)
- #917 — OSS-REFACTOR-TESTS: externalize the 3 in-module test files >1000 LOC (pure move)
- #926 — OSS-VALIDATORS-WORKSPACE: architecture + canonical validators are dead gates (flat-layer / cwd assumption)
- #928 — Epic: consequence-gating
- #929 — Contract Freeze: Define interfaces and contracts
- #930 — Implement Domain Layer (`domain/`) — template
- #931 — Implement Application Layer (`application/`) — template
- #932 — Implement Infrastructure Layer (`infrastructure/`) — template
- #933 — Implement [Component Name] — template
- #934 — Implement [ComponentName] — template
- #935 — Proofing: Validation scripts + CI integration
- #936 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #937 — Epic: consequence-gating
- #938 — Contract Freeze: Define interfaces and contracts
- #939 — Implement Precondition — consequence-gating
- #940 — Implement PreconditionRepository — consequence-gating
- #941 — Implement PreconditionRunner — consequence-gating
- #942 — Implement PreconditionService — consequence-gating
- #943 — Implement DispatchGate — consequence-gating
- #944 — Implement PreconditionFinding — consequence-gating
- #945 — Implement GatingMode — consequence-gating
- #946 — Implement CompanionStepObligation — consequence-gating
- #947 — Implement PreconditionError — consequence-gating
- #948 — Implement PreconditionSurfaces — consequence-gating
- #949 — Implement HardeningConfig — consequence-gating
- #950 — Proofing: Validation scripts + CI integration
- #951 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #966 — Epic: precondition activation + module rename (ADR-017 follow-ups)
- #967 — ACT: PreconditionSetup + ParallelExecutionFactoryConfig wiring
- #968 — ACT: wire precondition setup into composition roots (MCP/CLI/actions/server)
- #969 — ACT: end-to-end reachability test (gate armed from config, refusal + evidence)
- #970 — REN: rename module doc to precondition.md + architecture references
- #971 — REN: update canonical comments, tests dir, and CI script names
- #972 — REL (optional): relocate R2 GatingMode to execution_engine/dag_engine
- #973 — REL (optional): relocate R3 companion-step obligation into sequence_policy
- #982 — Evidence preservation: approval-resume re-emission drops ALL finding arrays (ADR-013/015/017)

</details>

## [1.8.0] - 2026-09-26

[1.8.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.8.0

### Added

- #899 OSS-AUDIT-C — ADR-016 Phase C anchored mode ([#908](https://github.com/arman-jalili/rigorix-oss/pull/908))

### Docs

- Close out audit-integrity Phase A–C in the docs ([#910](https://github.com/arman-jalili/rigorix-oss/pull/910))

### Internal

- Catalog parity is a subset, not equality ([#906](https://github.com/arman-jalili/rigorix-oss/pull/906))
- #907 OSS-CATALOG-SUBSET-VERIFIER — adopt the shared subset guard ([#909](https://github.com/arman-jalili/rigorix-oss/pull/909))
- 1.8.0 — ADR-016 Phase C + Phase B ledger ([#911](https://github.com/arman-jalili/rigorix-oss/pull/911))

<details><summary>Issues closed in this release (3)</summary>

- #899 — OSS-AUDIT-C: Audit integrity Phase C — anchored mode (anchor client + AnchoredHistoryAdapter + fail-closed)
- #905 — OSS-CATALOG-SUBSET: the OSS server catalog is a SUBSET of the frozen `rigorix.*` catalog
- #907 — OSS-CATALOG-SUBSET-VERIFIER: adopt `rigorix-verifier::verify_catalog_subset` (drop the inline re-check)

</details>

## [1.7.0] - 2026-09-24

[1.7.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.7.0

### Added

- OSS reference execution host skeleton (#888 PR B) ([#892](https://github.com/arman-jalili/rigorix-oss/pull/892))
- GET /events SSE push channel (#888 PR E) ([#894](https://github.com/arman-jalili/rigorix-oss/pull/894))
- ADR-016 Phase A — verify-on-read + local chain + unanchored mode ([#902](https://github.com/arman-jalili/rigorix-oss/pull/902))
- Wire engine event bus → SSE EventHub ([#903](https://github.com/arman-jalili/rigorix-oss/pull/903))

### Fixed

- Repair red main — Docker workspace copy + env-lock test race (+ image build on PRs) ([#896](https://github.com/arman-jalili/rigorix-oss/pull/896))
- Enforce ADR-014 retention coupling at composition ([#901](https://github.com/arman-jalili/rigorix-oss/pull/901))

### Docs

- Audit integrity model — ledger / projection / cache + trust anchor ([#897](https://github.com/arman-jalili/rigorix-oss/pull/897))

### Internal

- Extract host composition into the library (#888 PR A) ([#891](https://github.com/arman-jalili/rigorix-oss/pull/891))
- Method-surface coverage for execution/audit/template (#888 PR C+D) ([#893](https://github.com/arman-jalili/rigorix-oss/pull/893))
- 1.7.0 — audit integrity Phase A, live server events, chain re-emission fix ([#904](https://github.com/arman-jalili/rigorix-oss/pull/904))

<details><summary>Issues closed in this release (4)</summary>

- #888 — OSS-C2: rigorix-server — OSS reference execution host (axum JSON-RPC + SSE)
- #895 — GAP-A-29: ADR-014 retention coupling is unenforced — validate_retention is dead code (no retention config, no pruning caller)
- #898 — OSS-AUDIT-A: Audit integrity Phase A (OSS) — verify-on-read + local chain + local_unanchored mode
- #900 — OSS-SERVER-EVENTS: Wire the engine event bus → server EventHub (live GET /events)

</details>

## [1.6.0] - 2026-09-22

[1.6.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.6.0

### Added

- Implement #884 - operator-controlled step requirements (ADR-015) ([#886](https://github.com/arman-jalili/rigorix-oss/pull/886))
- Implement #889 - enterprise policy bundle ingestion + conformance round-trip ([#890](https://github.com/arman-jalili/rigorix-oss/pull/890))

### Docs

- Operator-controlled step requirements ([#885](https://github.com/arman-jalili/rigorix-oss/pull/885))

### Internal

- Cover the R9 step-requirement shapes (ADR-015 AC) ([#887](https://github.com/arman-jalili/rigorix-oss/pull/887))

<details><summary>Issues closed in this release (2)</summary>

- #884 — F-20260921-02 (GAP-A-28): Operator-controlled step requirements (ADR-015)
- #889 — OSS-C5: Engine consumes an enterprise-exported policy bundle — conformance round-trip

</details>

## [1.5.0] - 2026-09-21

[1.5.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.5.0

### Added

- Real LLM planning, SSE removal, session handshake, e2e hygiene (#724 #727 #739 #742) ([#762](https://github.com/arman-jalili/rigorix-oss/pull/762))
- Wire RiskClassifier into gate path + fix layering violations (#735 #738) ([#763](https://github.com/arman-jalili/rigorix-oss/pull/763))
- Backend selection in EvaluateInput ([#765](https://github.com/arman-jalili/rigorix-oss/pull/765))
- Governance Mode A — diff analysis + policy evaluation ([#766](https://github.com/arman-jalili/rigorix-oss/pull/766))
- Implement indexer stack — IndexerService + 3 repositories ([#767](https://github.com/arman-jalili/rigorix-oss/pull/767))
- Implement 9 unbuilt repository traits across modules ([#768](https://github.com/arman-jalili/rigorix-oss/pull/768))
- Wire FailureClassifierService into the retry loop ([#769](https://github.com/arman-jalili/rigorix-oss/pull/769))
- Filesystem TemplateRepository in the engine ([#770](https://github.com/arman-jalili/rigorix-oss/pull/770))
- Gap-ledger remainder — A-02/A-20/A-22 + M-09 ADRs + tooling ([#778](https://github.com/arman-jalili/rigorix-oss/pull/778))
- ADR-011 dispatch wiring — ApprovalService at the choke point (opt-in) ([#809](https://github.com/arman-jalili/rigorix-oss/pull/809))
- Approval evidence events + envelope schema (ADR-011) ([#811](https://github.com/arman-jalili/rigorix-oss/pull/811))
- R5 effect-scope oracle — git-diff changed-path oracle + engine hook ([#812](https://github.com/arman-jalili/rigorix-oss/pull/812))
- Production approval binding flip — factory attach + env-driven (ADR-011) ([#813](https://github.com/arman-jalili/rigorix-oss/pull/813))
- Envelope population + boundary identity passthrough (ADR-011) ([#814](https://github.com/arman-jalili/rigorix-oss/pull/814))
- Rigorix_approve_execution surfaces approver identity (ADR-011 R3) ([#815](https://github.com/arman-jalili/rigorix-oss/pull/815))
- Cross-run conflicting-action rules over the signed audit trail ([#872](https://github.com/arman-jalili/rigorix-oss/pull/872))
- Contract-sync conformance — engine output vs rigorix-sdk schemas (F-20260907-01/02) ([#879](https://github.com/arman-jalili/rigorix-oss/pull/879))
- ADR-014 effect-identity matching (value-identity predicate + effect-keyed history) ([#881](https://github.com/arman-jalili/rigorix-oss/pull/881))
- Record the run's /effect_key on the signed envelope (completes ADR-014) ([#882](https://github.com/arman-jalili/rigorix-oss/pull/882))
- Retention validator + catalog drift test + effect-key fixture ([#883](https://github.com/arman-jalili/rigorix-oss/pull/883))

### Changed

- [Issue #700] Contract Freeze: identity — interfaces, DTOs, schemas ([#709](https://github.com/arman-jalili/rigorix-oss/pull/709))
- [Issue #701] Implement IdentityClaim — is_valid, redacted_summary, envelope identity block ([#710](https://github.com/arman-jalili/rigorix-oss/pull/710))
- [Issue #702] Implement IdentitySource — attestation degradation to Unverified (explicit, no error) ([#711](https://github.com/arman-jalili/rigorix-oss/pull/711))
- [Issue #703] Implement IdentityAttestationService — JWKS verification (valid→Verified, tampered→Unverified) ([#712](https://github.com/arman-jalili/rigorix-oss/pull/712))
- [Issue #704] Implement TokenVerifier — complete unit coverage (NullVerifier + JwksVerifier) ([#713](https://github.com/arman-jalili/rigorix-oss/pull/713))
- [Issue #705] Implement IdentityRepository — FileSystemIdentityRepository (atomic write-rename) ([#714](https://github.com/arman-jalili/rigorix-oss/pull/714))
- [Issue #706] Implement IdentityError — recovery semantics + CoreOrchestratorError wiring ([#715](https://github.com/arman-jalili/rigorix-oss/pull/715))
- [Issue #707] Identity Proofing & CI Enforcement — contract + coverage scripts, stage 34 ([#716](https://github.com/arman-jalili/rigorix-oss/pull/716))
- [Issue #708] Architecture Readiness — identity runbook, DR plan, observability, docs sync ([#717](https://github.com/arman-jalili/rigorix-oss/pull/717))
- [Issue #786] Contract Freeze: Define interfaces and contracts — approval contracts frozen ([#798](https://github.com/arman-jalili/rigorix-oss/pull/798))
- [Issue #787] Implement ExecutionIntent — approval ([#799](https://github.com/arman-jalili/rigorix-oss/pull/799))
- [Issue #788] Implement IntentHash — approval ([#800](https://github.com/arman-jalili/rigorix-oss/pull/800))
- [Issue #789] Implement ApprovalRecord — approval ([#801](https://github.com/arman-jalili/rigorix-oss/pull/801))
- [Issue #790] Implement DecisionContext — approval ([#802](https://github.com/arman-jalili/rigorix-oss/pull/802))
- [Issue #791] Implement ScopeViolation — approval ([#803](https://github.com/arman-jalili/rigorix-oss/pull/803))
- [Issue #792] Implement ApprovalService — approval ([#804](https://github.com/arman-jalili/rigorix-oss/pull/804))
- [Issue #793] Implement ApproveInput / ApproveOutput — approval ([#805](https://github.com/arman-jalili/rigorix-oss/pull/805))
- [Issue #794] Implement ApprovalError — approval ([#806](https://github.com/arman-jalili/rigorix-oss/pull/806))
- [Issue #795] Proofing: Validation scripts + CI integration — approval ([#807](https://github.com/arman-jalili/rigorix-oss/pull/807))
- [Issue #796] Architecture Readiness: Runbook, DR, docs, CI enforcement — approval ([#808](https://github.com/arman-jalili/rigorix-oss/pull/808))
- [Issue #820] Contract Freeze: Define interfaces and contracts — auth contracts frozen ([#828](https://github.com/arman-jalili/rigorix-oss/pull/828))
- [Issue #821] Implement AuthService (Application) — ISSUE-AUTH-1 ([#829](https://github.com/arman-jalili/rigorix-oss/pull/829))
- [Issue #822] Implement IdpClient (Infrastructure) — ISSUE-AUTH-2 ([#830](https://github.com/arman-jalili/rigorix-oss/pull/830))
- [Issue #823] Implement KeychainStore (Infrastructure) — ISSUE-AUTH-3 ([#831](https://github.com/arman-jalili/rigorix-oss/pull/831))
- [Issue #824] Implement TokenProvider (Infrastructure) — ISSUE-AUTH-4 ([#832](https://github.com/arman-jalili/rigorix-oss/pull/832))
- [Issue #825] Implement AuthHandler / SSE Auth (Interfaces) — ISSUE-AUTH-5 ([#833](https://github.com/arman-jalili/rigorix-oss/pull/833))
- [Issue #826] Proofing & CI Enforcement — auth contracts ([#834](https://github.com/arman-jalili/rigorix-oss/pull/834))
- [Issue #827] Architecture Readiness — auth runbook, DR plan, docs sync ([#835](https://github.com/arman-jalili/rigorix-oss/pull/835))
- [Issue #838] Contract Freeze: Define interfaces and contracts — sequence-policy contracts frozen ([#853](https://github.com/arman-jalili/rigorix-oss/pull/853))
- [Issue #839] ISSUE-SEQUENCE-POLICY-1: Implement SequenceRule — TOML parse + round trip ([#854](https://github.com/arman-jalili/rigorix-oss/pull/854))
- [Issue #840] ISSUE-SEQUENCE-POLICY-2: Implement SequencePolicyService — evaluate_plan/evaluate_prefix ([#855](https://github.com/arman-jalili/rigorix-oss/pull/855))
- [Issue #841] ISSUE-SEQUENCE-POLICY-3: Implement SequencePolicyError — close acceptance ([#856](https://github.com/arman-jalili/rigorix-oss/pull/856))
- [Issue #842] ISSUE-SEQUENCE-POLICY-4: Implement StepPredicate matching ([#857](https://github.com/arman-jalili/rigorix-oss/pull/857))
- [Issue #843] ISSUE-SEQUENCE-POLICY-5: Implement Matcher — windowed matching + determinism ([#858](https://github.com/arman-jalili/rigorix-oss/pull/858))
- [Issue #844] ISSUE-SEQUENCE-POLICY-6: Orchestrator R2 — plan-time promote/deny gate ([#859](https://github.com/arman-jalili/rigorix-oss/pull/859))
- [Issue #845] ISSUE-SEQUENCE-POLICY-7: MCP validate_plan structured sequence findings ([#860](https://github.com/arman-jalili/rigorix-oss/pull/860))
- [Issue #846] ISSUE-SEQUENCE-POLICY-8: Execution Engine R3 run-time prefix gate ([#861](https://github.com/arman-jalili/rigorix-oss/pull/861))
- [Issue #847] ISSUE-SEQUENCE-POLICY-9: Fail-closed corrupt config refuses plan ([#862](https://github.com/arman-jalili/rigorix-oss/pull/862))
- [Issue #848] ISSUE-SEQUENCE-POLICY-10: Fail-open-absent — run executes unchanged ([#863](https://github.com/arman-jalili/rigorix-oss/pull/863))
- [Issue #849] ISSUE-SEQUENCE-POLICY-11: Audit R6 — envelope events + redacted summaries ([#864](https://github.com/arman-jalili/rigorix-oss/pull/864))
- [Issue #850] ISSUE-SEQUENCE-POLICY-12: Permission R5 — deny .rigorix/** agent writes by default ([#865](https://github.com/arman-jalili/rigorix-oss/pull/865))
- [Issue #851] Proofing & CI enforcement: sequence-policy ([#866](https://github.com/arman-jalili/rigorix-oss/pull/866))
- [Issue #852] Architecture Readiness: runbook, DR, observability, docs — sequence-policy ([#867](https://github.com/arman-jalili/rigorix-oss/pull/867))

### Fixed

- Exec-engine integrity — unknown-tool/missing-node failures, config_override, hydrate started_at (#718 #719 #720 #750) ([#759](https://github.com/arman-jalili/rigorix-oss/pull/759))
- Hooks — pipe deadlock + PreToolUse/PostToolUse in parallel dispatch (#721 #722) ([#760](https://github.com/arman-jalili/rigorix-oss/pull/760))
- Audit evidence — full-envelope HMAC, lifecycle events, degraded marker (#723 #755 #734 #752) ([#761](https://github.com/arman-jalili/rigorix-oss/pull/761))
- Make the conventions actually pass — CI red since #773 gated them ([#784](https://github.com/arman-jalili/rigorix-oss/pull/784))
- Restore guardian-manifest.json tracking ([#797](https://github.com/arman-jalili/rigorix-oss/pull/797))
- Atomic call-limit check — concurrent reserve() could over-commit ([#810](https://github.com/arman-jalili/rigorix-oss/pull/810))
- Wire the evidence producers — ApprovalRecorded + ScopeViolationRecorded now publish ([#816](https://github.com/arman-jalili/rigorix-oss/pull/816))
- Cargo fmt the merged approval-wiring lines (mcp/cli/actions) ([#817](https://github.com/arman-jalili/rigorix-oss/pull/817))
- Coverage.sh --lcov produced an empty percentage + wrong output dir ([#836](https://github.com/arman-jalili/rigorix-oss/pull/836))
- Scope EngineFacade method count to the trait impl region ([#868](https://github.com/arman-jalili/rigorix-oss/pull/868))
- Runtime wiring for sequence-policy + auth epics; workspace 1.4.0 ([#869](https://github.com/arman-jalili/rigorix-oss/pull/869))
- IdpConfig accepts loopback HTTP for local mock IdPs ([#870](https://github.com/arman-jalili/rigorix-oss/pull/870))
- Sign envelopes + template-level history + shared event bus ([#873](https://github.com/arman-jalili/rigorix-oss/pull/873))
- Per-run drain windows + ADR local-validation acceptance ([#874](https://github.com/arman-jalili/rigorix-oss/pull/874))

### Docs

- Reconcile README/HOW/AGENTS/ADR drift with the code (#743 #745 #747 #754) ([#772](https://github.com/arman-jalili/rigorix-oss/pull/772))
- Freeze SDK + server-mode strategy (2026-09-06) ([#875](https://github.com/arman-jalili/rigorix-oss/pull/875))
- Catalog design frozen (rigorix-sdk ADR-0001) ([#880](https://github.com/arman-jalili/rigorix-oss/pull/880))

### Internal

- Replace blocking std IO on the async runtime — tokio::fs + process timeouts ([#764](https://github.com/arman-jalili/rigorix-oss/pull/764))
- Test-hardening: TDD stub correction, state_persistence suite, live generator, adversarial tests (#741 #753 #748 #740) ([#771](https://github.com/arman-jalili/rigorix-oss/pull/771))
- Make proofing gate + reconcile stale claims ([#773](https://github.com/arman-jalili/rigorix-oss/pull/773))
- Repo hygiene — .gitignore repair, untrack runtime state + demo media ([#774](https://github.com/arman-jalili/rigorix-oss/pull/774))
- Typed errors instead of panics on the evidence path ([#775](https://github.com/arman-jalili/rigorix-oss/pull/775))
- Wire real coverage measurement into CI ([#779](https://github.com/arman-jalili/rigorix-oss/pull/779))
- Remove 39 heuristic *_coverage.sh scripts — superseded by real coverage ([#780](https://github.com/arman-jalili/rigorix-oss/pull/780))
- L-08 global unwrap pass — 105 production sites → typed errors ([#781](https://github.com/arman-jalili/rigorix-oss/pull/781))
- M-15 canonicalize ExecutionState node-state persistence ([#782](https://github.com/arman-jalili/rigorix-oss/pull/782))
- Release prep — bump crates to 1.3.0 ([#783](https://github.com/arman-jalili/rigorix-oss/pull/783))
- Debug build by default — --release becomes opt-in ([#818](https://github.com/arman-jalili/rigorix-oss/pull/818))

<details><summary>Issues closed in this release (94)</summary>

- #699 — Epic: identity
- #700 — Contract Freeze: Define interfaces and contracts
- #701 — Implement IdentityClaim — identity
- #702 — Implement IdentitySource — identity
- #703 — Implement IdentityAttestationService — identity
- #704 — Implement TokenVerifier — identity
- #705 — Implement IdentityRepository — identity
- #706 — Implement IdentityError — identity
- #707 — Proofing: Validation scripts + CI integration
- #708 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #718 — [GAP-A-01] unknown tool must fail, not report success
- #719 — [GAP-A-02] missing graph/node must fail, not return placeholder
- #720 — [GAP-A-03] config_override must be applied or removed
- #721 — [GAP-A-04] hook-runner subprocess pipe deadlock
- #722 — [GAP-A-05] wire PreToolUse/PostToolUse hooks into parallel dispatch
- #723 — [GAP-A-06] envelope HMAC must cover the full canonical payload
- #724 — [GAP-A-07] wire real LLM planning into MCP build_real_engine
- #725 — [GAP-A-08] implement governance action mode (Mode A)
- #726 — [GAP-A-09] budget accounting atomicity + production RAII guard
- #727 — [GAP-A-10] implement SSE transport or drop the flag
- #728 — [GAP-A-11] implement parsed-but-unread config flags
- #729 — [GAP-A-12] replace blocking std IO on the async runtime
- #730 — [GAP-A-13] add backend selection to scored_evaluation API
- #731 — [GAP-A-14] enforce time limits + prove enforcement blocks
- #732 — [GAP-A-15] implement repo_engine indexer stack (4 traits)
- #733 — [GAP-A-16] implement + wire ~10 repository traits
- #734 — [GAP-A-17] wire AuditEvent emission
- #735 — [GAP-A-18] connect RiskClassifier into risk-gating path
- #736 — [GAP-A-19] wire FailureClassifierService into retry loop
- #737 — [GAP-A-20] implement filesystem TemplateRepository in engine
- #738 — [GAP-A-21] fix 2 domain->application layering violations
- #739 — [GAP-A-22] wire session creation into MCP initialize handshake
- #740 — [GAP-A-23] adversarial tests for enforcement-critical paths
- #741 — [GAP-A-24] remove or convert 24 inert assert(false) stubs
- #742 — [GAP-A-25] mcp e2e: shared helpers, polling, tempdirs
- #743 — [GAP-A-26] fix binary name mismatch (rigorix vs rigorix-cli)
- #744 — [GAP-A-27] fix stale claims; make CI proofing actually gate
- #745 — [GAP-A-28] reconcile crate/module count drift
- #746 — [GAP-A-29] repo hygiene: .gitignore, .mov, committed state
- #747 — [GAP-A-30] fix misc doc drift (badge, SECURITY, comment, crypto versions)
- #748 — [GAP-H-03] planning: add live TemplateGenerator tests (Claude)
- #749 — [GAP-H-07] approve_node must enforce the requires_approval gate
- #750 — [GAP-H-08] hydrate_execution must preserve persisted started_at
- #751 — [GAP-L-08] reduce unwrap/expect/panic on dispatch and evidence paths
- #752 — [GAP-L-09] verify git_commit/git_branch population in audit envelope
- #753 — [GAP-M-01] state_persistence: add dedicated test file
- #754 — [GAP-M-09] docs: reconcile ADR statuses with implementation
- #755 — [GAP-M-12] envelope signing must be required for approval-bearing runs
- #756 — [GAP-M-13] wire planning_prompt_content capture (or decision_context reuse)
- #757 — [GAP-M-14] approval/evidence event-publish failures must be observable
- #758 — [GAP-M-15] consolidate ExecutionState dual node-state persistence
- #776 — [GAP-M-12] require signing for approval-bearing audit runs; add evidence: degraded marker
- #777 — [GAP-L-08] global unwrap/expect/panic elimination backlog (beyond dispatch/evidence path)
- #785 — Epic: approval
- #786 — Contract Freeze: Define interfaces and contracts
- #787 — Implement ExecutionIntent — approval
- #788 — Implement IntentHash — approval
- #789 — Implement ApprovalRecord — approval
- #790 — Implement DecisionContext — approval
- #791 — Implement ScopeViolation — approval
- #792 — Implement ApprovalService — approval
- #793 — Implement ApproveInput / ApproveOutput — approval
- #794 — Implement ApprovalError — approval
- #795 — Proofing: Validation scripts + CI integration
- #796 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #819 — Epic: auth
- #820 — Contract Freeze: Define interfaces and contracts
- #821 — AuthService (Application): Orchestrates the identity lifecycle: login (device flow), refresh, status, logou
- #822 — IdpClient (Infrastructure): OIDC device flow client for any IdP the dev/org configures
- #823 — KeychainStore (Infrastructure): Long-lived credential custody — the refresh token lives HERE, never in config fi
- #824 — TokenProvider (Infrastructure): Short-TTL access token custody in-memory — what the agent-visible surface can pr
- #825 — AuthHandler / SSE Auth (Interfaces): MCP tool handlers + optional SSE transport gate
- #826 — Proofing: Validation scripts + CI integration
- #827 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #837 — Epic: sequence-policy
- #838 — Contract Freeze: Define interfaces and contracts
- #839 — Implement SequenceRule — sequence-policy
- #840 — Implement SequencePolicyService — sequence-policy
- #841 — Implement SequencePolicyError — sequence-policy
- #842 — Implement StepPredicate — sequence-policy
- #843 — Implement Matcher — sequence-policy
- #844 — Implement Orchestrator (R2) — sequence-policy
- #845 — Implement MCP surface — sequence-policy
- #846 — Implement Execution Engine (R3) — sequence-policy
- #847 — Implement Fail-closed — sequence-policy
- #848 — Implement Fail-open-absent — sequence-policy
- #849 — Implement Audit (R6) — sequence-policy
- #850 — Implement Permission (R5) — sequence-policy
- #851 — Proofing: Validation scripts + CI integration
- #852 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #871 — R7: cross-run conflicting-action rules — audit trail as policy input (sequence-policy module)
- #876 — F-20260907-01: [rigorix-oss] Envelope-v2 conformance — engine output vs rigorix-sdk schemas + real signed fixtures
- #877 — F-20260907-02: [rigorix-oss] Policy/claims conformance — engine types vs rigorix-sdk policy.json + claims.json v1
- #878 — F-20260907-05: [rigorix-oss] Identity gate for consequential runs + principal hygiene (unauthenticated seat-mutation bypass)

</details>

## [1.2.0] - 2026-08-21

[1.2.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.2.0

## [1.1.0] - 2026-08-06

[1.1.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.1.0

### Added

- Contract freeze — all interfaces defined ([#695](https://github.com/arman-jalili/rigorix-oss/pull/695))
- Domain layer implementation ([#696](https://github.com/arman-jalili/rigorix-oss/pull/696))
- Batch — all pre-implemented components ([#697](https://github.com/arman-jalili/rigorix-oss/pull/697))
- Implement all remaining components ([#698](https://github.com/arman-jalili/rigorix-oss/pull/698))

<details><summary>Issues closed in this release (22)</summary>

- #673 — Epic: scored-evaluation
- #674 — Contract Freeze: Define interfaces and contracts
- #675 — Implement Domain Layer (`domain/`) — scored-evaluation
- #676 — Implement Application Layer (`application/`) — scored-evaluation
- #677 — Implement Infrastructure Layer (`infrastructure/`) — scored-evaluation
- #678 — Implement ScoredEvaluationNode — scored-evaluation
- #679 — Implement Rubric — scored-evaluation
- #680 — Implement ScoringResult — scored-evaluation
- #681 — Implement ScoringBackend (Trait) — scored-evaluation
- #682 — Implement ScoredEvaluationEvent — scored-evaluation
- #683 — Implement ScoredEvaluationError — scored-evaluation
- #684 — Implement ScoredEvaluationService — scored-evaluation
- #685 — Implement EvaluateInput / EvaluateOutput — scored-evaluation
- #686 — Implement MCPBackend — scored-evaluation
- #687 — Implement HTTPBackend — scored-evaluation
- #688 — Implement LocalBackend — scored-evaluation
- #689 — Implement ScoreDimension — scored-evaluation
- #690 — Implement ScoringBackend — scored-evaluation
- #691 — Implement ScoreAbove policy condition — scored-evaluation
- #692 — Implement ScoreBelow policy condition — scored-evaluation
- #693 — Proofing: Validation scripts + CI integration
- #694 — Architecture Readiness: Runbook, DR, docs, CI enforcement

</details>

## [1.0.0] - 2026-07-14

[1.0.0]: https://github.com/arman-jalili/rigorix-oss/releases/tag/v1.0.0

### Added

- Implement ConfigService with multi-source loading ([#8](https://github.com/arman-jalili/rigorix-oss/pull/8))
- Implement SecretService with env var loading ([#9](https://github.com/arman-jalili/rigorix-oss/pull/9))
- Add configuration proofing CI scripts ([#10](https://github.com/arman-jalili/rigorix-oss/pull/10))
- Architecture readiness — docs, changelog, proofing ([#11](https://github.com/arman-jalili/rigorix-oss/pull/11))
- Contract freeze for audit module ([#17](https://github.com/arman-jalili/rigorix-oss/pull/17))
- AuditEnvelope implementation ([#18](https://github.com/arman-jalili/rigorix-oss/pull/18))
- Proofing validation scripts + CI integration for audit ([#19](https://github.com/arman-jalili/rigorix-oss/pull/19))
- Architecture readiness for audit module ([#20](https://github.com/arman-jalili/rigorix-oss/pull/20))
- Contract freeze for failure-classification ([#39](https://github.com/arman-jalili/rigorix-oss/pull/39))
- Implement FailureType classification services ([#40](https://github.com/arman-jalili/rigorix-oss/pull/40))
- Implement RetryStrategy with full integration tests ([#41](https://github.com/arman-jalili/rigorix-oss/pull/41))
- Implement classify_failure() free function ([#42](https://github.com/arman-jalili/rigorix-oss/pull/42))
- Add failure-classification proofing scripts + CI integration ([#43](https://github.com/arman-jalili/rigorix-oss/pull/43))
- Architecture readiness for failure-classification module ([#44](https://github.com/arman-jalili/rigorix-oss/pull/44))
- Contract freeze enforcement module ([#62](https://github.com/arman-jalili/rigorix-oss/pull/62))
- Implement EnforcementConfig with preset builders and validation ([#63](https://github.com/arman-jalili/rigorix-oss/pull/63))
- Implement ExecutionEnforcer with runtime enforcement ([#64](https://github.com/arman-jalili/rigorix-oss/pull/64))
- Create enforcement proofing CI scripts ([#65](https://github.com/arman-jalili/rigorix-oss/pull/65))
- Architecture readiness for enforcement module ([#66](https://github.com/arman-jalili/rigorix-oss/pull/66))
- Freeze contracts for budget-tracking epic (LlmBudget, LlmBudgetReservation) ([#73](https://github.com/arman-jalili/rigorix-oss/pull/73))
- Implement LlmBudget and LlmBudgetReservation ([#74](https://github.com/arman-jalili/rigorix-oss/pull/74))
- Add proofing scripts and CI integration for budget-tracking ([#75](https://github.com/arman-jalili/rigorix-oss/pull/75))
- Architecture readiness for budget-tracking (runbook, DR plan, docs) ([#76](https://github.com/arman-jalili/rigorix-oss/pull/76))
- Contract freeze risk-gating module (#issue-contract-freeze) ([#95](https://github.com/arman-jalili/rigorix-oss/pull/95))
- Implement RiskClassifier component ([#96](https://github.com/arman-jalili/rigorix-oss/pull/96))
- Complete RiskLevel domain entity with comprehensive tests ([#97](https://github.com/arman-jalili/rigorix-oss/pull/97))
- Complete RiskConfig domain entity with comprehensive tests ([#98](https://github.com/arman-jalili/rigorix-oss/pull/98))
- Add risk-gating proofing scripts and CI stage ([#99](https://github.com/arman-jalili/rigorix-oss/pull/99))
- Architecture readiness for risk-gating epic ([#100](https://github.com/arman-jalili/rigorix-oss/pull/100))
- Contract freeze — dag-engine interfaces and DTOs ([#118](https://github.com/arman-jalili/rigorix-oss/pull/118))
- Implement TaskGraph #114 — topo sort, cycle detection, ready queue ([#119](https://github.com/arman-jalili/rigorix-oss/pull/119))
- Implement ExecutionPolicyService #115 — retry decisions, backoff, validation ([#120](https://github.com/arman-jalili/rigorix-oss/pull/120))
- Add dag-engine proofing scripts #116 — contract check, coverage, CI ([#121](https://github.com/arman-jalili/rigorix-oss/pull/121))
- Architecture readiness #117 — runbook, DR plan, doc sync ([#122](https://github.com/arman-jalili/rigorix-oss/pull/122))
- Contract freeze for tool-system ([#129](https://github.com/arman-jalili/rigorix-oss/pull/129))
- Implement Tool Trait concrete tools ([#130](https://github.com/arman-jalili/rigorix-oss/pull/130))
- Implement TemplateParser ([#131](https://github.com/arman-jalili/rigorix-oss/pull/131))
- Implement TemplateEngine ([#132](https://github.com/arman-jalili/rigorix-oss/pull/132))
- Proofing scripts ([#133](https://github.com/arman-jalili/rigorix-oss/pull/133))
- Architecture-readiness for template-system ([#134](https://github.com/arman-jalili/rigorix-oss/pull/134))
- Implement ToolRegistry ([#135](https://github.com/arman-jalili/rigorix-oss/pull/135))
- Tool-system proofing scripts and architecture-readiness (#127, #128) ([#136](https://github.com/arman-jalili/rigorix-oss/pull/136))
- Freeze template-generation contracts (TemplateGenerator, generation service, symbol validation) ([#167](https://github.com/arman-jalili/rigorix-oss/pull/167))
- Implement TemplateGenerator trait with comprehensive tests ([#168](https://github.com/arman-jalili/rigorix-oss/pull/168))
- Implement ClaudeTemplateGenerator with Anthropic Messages API ([#169](https://github.com/arman-jalili/rigorix-oss/pull/169))
- Implement Phase 3 Symbol Validation service ([#170](https://github.com/arman-jalili/rigorix-oss/pull/170))
- Verify planning-pipeline proofing scripts are complete ([#171](https://github.com/arman-jalili/rigorix-oss/pull/171))
- Verify architecture readiness for planning-pipeline ([#172](https://github.com/arman-jalili/rigorix-oss/pull/172))
- Freeze CoreOrchestratorError and ExecutionError contracts ([#191](https://github.com/arman-jalili/rigorix-oss/pull/191))
- Add error-handling proofing scripts and CI stage ([#192](https://github.com/arman-jalili/rigorix-oss/pull/192))
- Architecture readiness for error-handling module ([#193](https://github.com/arman-jalili/rigorix-oss/pull/193))
- Implement ParallelExecutor — concurrent DAG execution with retry ([#200](https://github.com/arman-jalili/rigorix-oss/pull/200))
- Implement Retry Logic — inline retry loop and factory wiring ([#201](https://github.com/arman-jalili/rigorix-oss/pull/201))
- Add execution-engine proofing scripts and CI stage ([#202](https://github.com/arman-jalili/rigorix-oss/pull/202))
- Architecture readiness for execution-engine — runbook, DR plan, docs ([#203](https://github.com/arman-jalili/rigorix-oss/pull/203))
- Extract template_generation into standalone Clean Architecture module ([#205](https://github.com/arman-jalili/rigorix-oss/pull/205))
- EPIC-002 — Architecture & Code Quality (issues #207-#210) ([#221](https://github.com/arman-jalili/rigorix-oss/pull/221))
- EPIC-001 — Observability Foundation (issues #211-#214) ([#222](https://github.com/arman-jalili/rigorix-oss/pull/222))
- EPIC-003 — Testing Hardening (issues #215-#217) ([#223](https://github.com/arman-jalili/rigorix-oss/pull/223))
- Freeze CLI boundary contracts (Issue #236) ([#240](https://github.com/arman-jalili/rigorix-oss/pull/240))
- CLI foundation scaffold — config, tracing, signals (Issue #237) ([#241](https://github.com/arman-jalili/rigorix-oss/pull/241))
- CLI proofing scripts — contracts, coverage, CI integration (Issue #238) ([#242](https://github.com/arman-jalili/rigorix-oss/pull/242))
- Architecture readiness — runbook, DR plan, docs (Issue #239) ([#243](https://github.com/arman-jalili/rigorix-oss/pull/243))
- Issue 1.2 — Configuration Loading ([#249](https://github.com/arman-jalili/rigorix-oss/pull/249))
- Issue #247 — Proofing scripts for configuration module ([#250](https://github.com/arman-jalili/rigorix-oss/pull/250))
- Issue #248 — Architecture Readiness for configuration module ([#251](https://github.com/arman-jalili/rigorix-oss/pull/251))
- Issue #255 — Observability proofing scripts ([#257](https://github.com/arman-jalili/rigorix-oss/pull/257))
- Issue #256 — Observability architecture readiness ([#258](https://github.com/arman-jalili/rigorix-oss/pull/258))
- Contract freeze for templates module (Issue #266) ([#270](https://github.com/arman-jalili/rigorix-oss/pull/270))
- Proofing scripts + CI integration for templates module (Issue #268) ([#271](https://github.com/arman-jalili/rigorix-oss/pull/271))
- Architecture readiness for templates module (Issue #269) ([#272](https://github.com/arman-jalili/rigorix-oss/pull/272))
- Contract freeze for cancellation module ([#278](https://github.com/arman-jalili/rigorix-oss/pull/278))
- Proofing scripts + CI integration for cancellation module (Issue #276) ([#279](https://github.com/arman-jalili/rigorix-oss/pull/279))
- Architecture readiness for cancellation module (Issue #277) ([#280](https://github.com/arman-jalili/rigorix-oss/pull/280))
- Contract freeze for configuration module ([#286](https://github.com/arman-jalili/rigorix-oss/pull/286))
- Architecture readiness for configuration module (Issue #285) ([#287](https://github.com/arman-jalili/rigorix-oss/pull/287))
- Contract freeze for observability module ([#293](https://github.com/arman-jalili/rigorix-oss/pull/293))
- Architecture readiness for observability module (Issue #292) ([#294](https://github.com/arman-jalili/rigorix-oss/pull/294))
- Contract freeze for cli-boundary module ([#300](https://github.com/arman-jalili/rigorix-oss/pull/300))
- Architecture readiness for cli-boundary module (Issue #299) ([#301](https://github.com/arman-jalili/rigorix-oss/pull/301))
- Contract freeze for template-generation module ([#307](https://github.com/arman-jalili/rigorix-oss/pull/307))
- Proofing + architecture readiness for template-generation module ([#308](https://github.com/arman-jalili/rigorix-oss/pull/308))
- Contract freeze for planning-pipeline module ([#314](https://github.com/arman-jalili/rigorix-oss/pull/314))
- Proofing + architecture readiness for planning-pipeline module ([#315](https://github.com/arman-jalili/rigorix-oss/pull/315))
- Contract freeze for execution-engine module ([#321](https://github.com/arman-jalili/rigorix-oss/pull/321))
- Proofing + architecture readiness for execution-engine ([#322](https://github.com/arman-jalili/rigorix-oss/pull/322))
- Contract freeze for state-persistence module ([#328](https://github.com/arman-jalili/rigorix-oss/pull/328))
- Proofing + architecture readiness for state-persistence ([#329](https://github.com/arman-jalili/rigorix-oss/pull/329))
- Contract freeze for event-system module ([#335](https://github.com/arman-jalili/rigorix-oss/pull/335))
- Proofing + architecture readiness for event-system ([#336](https://github.com/arman-jalili/rigorix-oss/pull/336))
- Contract freeze for cli-boundary module ([#362](https://github.com/arman-jalili/rigorix-oss/pull/362))
- Implement CliParser ([#363](https://github.com/arman-jalili/rigorix-oss/pull/363))
- Implement cli-boundary components (dispatcher, config, signal, output, orchestrator) ([#364](https://github.com/arman-jalili/rigorix-oss/pull/364))
- Proofing and architecture readiness for cli-boundary ([#365](https://github.com/arman-jalili/rigorix-oss/pull/365))
- Contract freeze for tui module ([#378](https://github.com/arman-jalili/rigorix-oss/pull/378))
- Implement tui components (render loop, views, input) ([#379](https://github.com/arman-jalili/rigorix-oss/pull/379))
- Real tui implementation (widgets, keymap, full views) ([#380](https://github.com/arman-jalili/rigorix-oss/pull/380))
- Full view implementations — no more empty views ([#381](https://github.com/arman-jalili/rigorix-oss/pull/381))
- Architecture readiness for tui module ([#383](https://github.com/arman-jalili/rigorix-oss/pull/383))
- Full engine wiring — all 7 sub-services connected ([#384](https://github.com/arman-jalili/rigorix-oss/pull/384))
- Llm models config — models.json support like pi agent ([#385](https://github.com/arman-jalili/rigorix-oss/pull/385))
- Wire CodeGraphService into orchestrator + add module_deps to RepoContext ([#403](https://github.com/arman-jalili/rigorix-oss/pull/403))
- Adopt compact format, targeted context, skip-when-known patterns ([#404](https://github.com/arman-jalili/rigorix-oss/pull/404))
- Wire final two connections — orchestrator builds CodeGraph, pipeline injects module_deps ([#405](https://github.com/arman-jalili/rigorix-oss/pull/405))
- Async execution engine, progress events, TUI EventBridge ([#407](https://github.com/arman-jalili/rigorix-oss/pull/407))
- Tree-sitter anchored file_patch tool ([#408](https://github.com/arman-jalili/rigorix-oss/pull/408))
- Hooks module — contract freeze, implementation, and proofing ([#418](https://github.com/arman-jalili/rigorix-oss/pull/418))
- Hooks proofing scripts and CI stage integration ([#419](https://github.com/arman-jalili/rigorix-oss/pull/419))
- Hooks architecture readiness — runbook, DR plan, docs ([#420](https://github.com/arman-jalili/rigorix-oss/pull/420))
- Hooks proofing scripts and CI stage integration ([#421](https://github.com/arman-jalili/rigorix-oss/pull/421))
- Hooks architecture readiness — runbook, DR plan, docs ([#422](https://github.com/arman-jalili/rigorix-oss/pull/422))
- Implement EditFileTool — exact-string file editing ([#433](https://github.com/arman-jalili/rigorix-oss/pull/433))
- Enhance ReadFileTool with offset/limit, binary detection, size limits ([#434](https://github.com/arman-jalili/rigorix-oss/pull/434))
- Implement SyntaxGate — post-edit tree-sitter syntax verification ([#435](https://github.com/arman-jalili/rigorix-oss/pull/435))
- Add code-generation proofing scripts and CI stage ([#436](https://github.com/arman-jalili/rigorix-oss/pull/436))
- Architecture readiness for code-generation module ([#437](https://github.com/arman-jalili/rigorix-oss/pull/437))
- Contract freeze for recovery-recipes module ([#447](https://github.com/arman-jalili/rigorix-oss/pull/447))
- Implement recovery-recipes components (#440-#446) ([#448](https://github.com/arman-jalili/rigorix-oss/pull/448))
- Contract freeze for quality-gates module (#449, #450) ([#456](https://github.com/arman-jalili/rigorix-oss/pull/456))
- Implement quality-gates components (#451-#455) ([#457](https://github.com/arman-jalili/rigorix-oss/pull/457))
- Freeze policy-engine public interfaces and contracts ([#478](https://github.com/arman-jalili/rigorix-oss/pull/478))
- Implement policy-engine core services (#471-#475) ([#479](https://github.com/arman-jalili/rigorix-oss/pull/479))
- Add policy-engine proofing scripts and architecture readiness docs (#476, #477) ([#480](https://github.com/arman-jalili/rigorix-oss/pull/480))
- Contract Freeze — llm-step interfaces and contracts ([#488](https://github.com/arman-jalili/rigorix-oss/pull/488))
- Implement LlmGenerateNode — LLM generation node for DAG execution ([#489](https://github.com/arman-jalili/rigorix-oss/pull/489))
- Implement LlmStepContext — source context and failure analysis for LLM generation ([#490](https://github.com/arman-jalili/rigorix-oss/pull/490))
- Implement LlmStepService — full integration of LLM generation pipeline ([#491](https://github.com/arman-jalili/rigorix-oss/pull/491))
- Add proofing and CI enforcement scripts for llm-step module ([#492](https://github.com/arman-jalili/rigorix-oss/pull/492))
- Architecture readiness for llm-step — runbook, DR plan, observability ([#493](https://github.com/arman-jalili/rigorix-oss/pull/493))
- Contract freeze for failure-parser epic ([#502](https://github.com/arman-jalili/rigorix-oss/pull/502))
- Implement TemplateFailure domain model and service ([#503](https://github.com/arman-jalili/rigorix-oss/pull/503))
- Implement FailureParserService with fix suggestion generation ([#504](https://github.com/arman-jalili/rigorix-oss/pull/504))
- Implement TypeScript Parser for tsc output ([#505](https://github.com/arman-jalili/rigorix-oss/pull/505))
- Implement FixSuggestionServiceImpl with symbol matching ([#506](https://github.com/arman-jalili/rigorix-oss/pull/506))
- Add CI proofing scripts for failure-parser ([#507](https://github.com/arman-jalili/rigorix-oss/pull/507))
- Architecture readiness for failure-parser epic ([#508](https://github.com/arman-jalili/rigorix-oss/pull/508))
- Freeze contracts for plan-validation module ([#517](https://github.com/arman-jalili/rigorix-oss/pull/517))
- Implement ValidationLoop Config, State, Service, and ContextAugmenter (#511 #512 #513 #514) ([#518](https://github.com/arman-jalili/rigorix-oss/pull/518))
- Proofing scripts, runbook, and DR plan for plan-validation (#515 #516) ([#519](https://github.com/arman-jalili/rigorix-oss/pull/519))
- Contract freeze for action-input epic ([#529](https://github.com/arman-jalili/rigorix-oss/pull/529))
- Implement ActionInputs parsing ([#530](https://github.com/arman-jalili/rigorix-oss/pull/530))
- Implement InputParser with integration tests ([#531](https://github.com/arman-jalili/rigorix-oss/pull/531))
- Implement CommentParser ([#532](https://github.com/arman-jalili/rigorix-oss/pull/532))
- Implement CiDetector ([#533](https://github.com/arman-jalili/rigorix-oss/pull/533))
- Implement ConfigLoader ([#534](https://github.com/arman-jalili/rigorix-oss/pull/534))
- Action-input proofing scripts ([#535](https://github.com/arman-jalili/rigorix-oss/pull/535))
- Architecture readiness for action-input ([#536](https://github.com/arman-jalili/rigorix-oss/pull/536))
- Contract freeze for security-config epic ([#546](https://github.com/arman-jalili/rigorix-oss/pull/546))
- Implement ForkDetector ([#547](https://github.com/arman-jalili/rigorix-oss/pull/547))
- Implement SecretMasker, TokenValidator, UrlAllowlist, HmacSigner (#540-#543) ([#548](https://github.com/arman-jalili/rigorix-oss/pull/548))
- Proofing & architecture readiness for security-config (#544-#545) ([#549](https://github.com/arman-jalili/rigorix-oss/pull/549))
- Architecture readiness for security-config ([#550](https://github.com/arman-jalili/rigorix-oss/pull/550))
- Contract freeze for diff-analyzer module ([#560](https://github.com/arman-jalili/rigorix-oss/pull/560))
- Implement diff-analyzer components (#553 #554 #555 #556 #557) ([#561](https://github.com/arman-jalili/rigorix-oss/pull/561))
- Proofing scripts for diff-analyzer ([#562](https://github.com/arman-jalili/rigorix-oss/pull/562))
- Architecture readiness for diff-analyzer ([#563](https://github.com/arman-jalili/rigorix-oss/pull/563))
- Freeze contracts for all components ([#573](https://github.com/arman-jalili/rigorix-oss/pull/573))
- Implement policy-evaluator components (PolicyDocument, PolicyRule, PolicyLoader, PolicyEvaluator, OrgPolicyMerger) ([#574](https://github.com/arman-jalili/rigorix-oss/pull/574))
- Proofing scripts, runbook, DR plan for policy-evaluator ([#575](https://github.com/arman-jalili/rigorix-oss/pull/575))
- Implement OutputFormatter — top-level output orchestrator ([#584](https://github.com/arman-jalili/rigorix-oss/pull/584))
- Implement AnnotationWriter — GitHub workflow annotations ([#585](https://github.com/arman-jalili/rigorix-oss/pull/585))
- Implement StepSummaryWriter — markdown step summaries ([#586](https://github.com/arman-jalili/rigorix-oss/pull/586))
- Add action-output proofing scripts and CI stage ([#587](https://github.com/arman-jalili/rigorix-oss/pull/587))
- Action-output epic — all 5 components implemented (57 tests) ([#588](https://github.com/arman-jalili/rigorix-oss/pull/588))
- Contract freeze for action-entrypoint module ([#619](https://github.com/arman-jalili/rigorix-oss/pull/619))
- Implement ActionRouter for action-entrypoint ([#620](https://github.com/arman-jalili/rigorix-oss/pull/620))
- Implement ActionContext for action-entrypoint ([#621](https://github.com/arman-jalili/rigorix-oss/pull/621))
- Implement ActionMode for action-entrypoint ([#622](https://github.com/arman-jalili/rigorix-oss/pull/622))
- Add proofing scripts for action-entrypoint module ([#623](https://github.com/arman-jalili/rigorix-oss/pull/623))
- Mcp-server contract freeze + implementation + proofing + readiness ([#630](https://github.com/arman-jalili/rigorix-oss/pull/630))
- Freeze execution-tools contracts — interfaces, DTOs, events, schemas ([#639](https://github.com/arman-jalili/rigorix-oss/pull/639))
- Implement execution-tools — EngineFacade, handlers, tests ([#640](https://github.com/arman-jalili/rigorix-oss/pull/640))
- Execution-tools proofing and architecture readiness ([#641](https://github.com/arman-jalili/rigorix-oss/pull/641))
- Implement all audit-tools components (contracts + implementations) ([#651](https://github.com/arman-jalili/rigorix-oss/pull/651))
- Implement template-tools epic — contract freeze, handlers, repository, CI proofing ([#662](https://github.com/arman-jalili/rigorix-oss/pull/662))
- Enterprise-proxy epic - contract freeze, implementation, proofing, and architecture readiness ([#671](https://github.com/arman-jalili/rigorix-oss/pull/671))
- Enterprise-proxy conditional wiring in main.rs + E2E tests ([#672](https://github.com/arman-jalili/rigorix-oss/pull/672))

### Changed

- Contract Freeze: Cancellation Module ([#27](https://github.com/arman-jalili/rigorix-oss/pull/27))
- Implementation: CancellationManager ([#28](https://github.com/arman-jalili/rigorix-oss/pull/28))
- Implementation: ShutdownSignal ([#29](https://github.com/arman-jalili/rigorix-oss/pull/29))
- Proofing & CI Enforcement: Cancellation ([#30](https://github.com/arman-jalili/rigorix-oss/pull/30))
- Architecture Readiness: Cancellation ([#31](https://github.com/arman-jalili/rigorix-oss/pull/31))
- [#46] Contract Freeze: event-system — interfaces, DTOs, events, API contracts ([#51](https://github.com/arman-jalili/rigorix-oss/pull/51))
- [#47] Implement EventBus — pub-sub with in-memory persistence ([#52](https://github.com/arman-jalili/rigorix-oss/pull/52))
- [#48] Implement ExecutionEvent — all 11 variants with helpers, constructors, serde tests ([#53](https://github.com/arman-jalili/rigorix-oss/pull/53))
- [#49] Proofing & CI Enforcement: event-system — contract checks, coverage, CI stage ([#54](https://github.com/arman-jalili/rigorix-oss/pull/54))
- [#50] Architecture Readiness: event-system — runbook, DR plan, CHANGELOG, CI ([#55](https://github.com/arman-jalili/rigorix-oss/pull/55))
- [Issue #78] Contract Freeze: state-persistence interfaces and schemas ([#83](https://github.com/arman-jalili/rigorix-oss/pull/83))
- [Issue #79] Implement ExecutionState: serializable execution snapshots ([#84](https://github.com/arman-jalili/rigorix-oss/pull/84))
- [Issue #80] Implement StateManager: graph persistence, record storage, locking ([#85](https://github.com/arman-jalili/rigorix-oss/pull/85))
- [Issue #81] Proofing: validation scripts and CI integration for state-persistence ([#86](https://github.com/arman-jalili/rigorix-oss/pull/86))
- [Issue #82] Architecture Readiness: runbook, DR plan, docs sync, CI enforcement ([#87](https://github.com/arman-jalili/rigorix-oss/pull/87))
- [issue-contract-freeze] Contract Freeze: template-system — TemplateParser & TemplateEngine interfaces ([#107](https://github.com/arman-jalili/rigorix-oss/pull/107))
- [issue-templateparser] Implement TemplateParser and TemplateEngine ([#108](https://github.com/arman-jalili/rigorix-oss/pull/108))
- [issue-proofing] Add CI proofing scripts for template-system ([#109](https://github.com/arman-jalili/rigorix-oss/pull/109))
- [issue-proofing] Add CI proofing scripts for template-system ([#110](https://github.com/arman-jalili/rigorix-oss/pull/110))
- [issue-architecture-readiness] Add runbook, DR plan, update architecture docs ([#111](https://github.com/arman-jalili/rigorix-oss/pull/111))
- [#138] Contract Freeze: repo-engine — SymbolGraph, SymbolDefinition, SymbolWorkspaceIntent ([#144](https://github.com/arman-jalili/rigorix-oss/pull/144))
- [#139] SymbolGraph: Thread-safe in-memory symbol graph with O(1) lookup ([#145](https://github.com/arman-jalili/rigorix-oss/pull/145))
- [#140] SymbolDefinition: pub struct with kind, location, signature, docs, source files ([#146](https://github.com/arman-jalili/rigorix-oss/pull/146))
- [#141] SymbolWorkspaceIntent: TaskNode workspace intent for Phase 3 validation ([#147](https://github.com/arman-jalili/rigorix-oss/pull/147))
- [#142] Proofing: CI validation scripts + hardening pipeline integration ([#148](https://github.com/arman-jalili/rigorix-oss/pull/148))
- [#143] Architecture Readiness: runbook, DR plan, changelog, observability ([#149](https://github.com/arman-jalili/rigorix-oss/pull/149))
- [Issue #151] Contract Freeze: planning-pipeline ([#156](https://github.com/arman-jalili/rigorix-oss/pull/156))
- [Issue #152] Implement PlanningPipeline component ([#157](https://github.com/arman-jalili/rigorix-oss/pull/157))
- [Issue #153] Implement Claude and OpenAI classifiers ([#158](https://github.com/arman-jalili/rigorix-oss/pull/158))
- [Issue #155] Architecture readiness: planning-pipeline ([#159](https://github.com/arman-jalili/rigorix-oss/pull/159))
- [Issue #173] Contract Freeze: audit — all interfaces, DTOs, events, and API contracts ([#181](https://github.com/arman-jalili/rigorix-oss/pull/181))
- [Issue #175] AuditEnvelope: verified frozen contract implementation ([#182](https://github.com/arman-jalili/rigorix-oss/pull/182))
- [Issue #176] AuditSender: verified frozen contract implementation ([#183](https://github.com/arman-jalili/rigorix-oss/pull/183))
- [Issue #177] AuditQueue: verified frozen contract implementation ([#184](https://github.com/arman-jalili/rigorix-oss/pull/184))
- [Issue #178] CircuitBreaker: verified frozen contract implementation ([#185](https://github.com/arman-jalili/rigorix-oss/pull/185))
- Batch: code quality fixes (L-01, L-02, L-03, L-07) ([#226](https://github.com/arman-jalili/rigorix-oss/pull/226))
- Batch: testing expansion (C-03, H-03, L-04) ([#227](https://github.com/arman-jalili/rigorix-oss/pull/227))
- Batch: tooling and CI improvements (M-03, M-04, L-06) ([#228](https://github.com/arman-jalili/rigorix-oss/pull/228))
- Batch: architecture improvements (M-07, M-08) ([#229](https://github.com/arman-jalili/rigorix-oss/pull/229))
- Batch: testing deep part 1 (M-01) ([#230](https://github.com/arman-jalili/rigorix-oss/pull/230))
- Batch: testing deep part 2 (M-02) ([#231](https://github.com/arman-jalili/rigorix-oss/pull/231))
- Batch: failure-injection tests (M-11) ([#232](https://github.com/arman-jalili/rigorix-oss/pull/232))
- Batch: final five gaps — C-03, M-09, M-10, L-05, H-03 ([#233](https://github.com/arman-jalili/rigorix-oss/pull/233))
- Batch: fix remaining 4 gaps (M-09, M-01, H-03, L-05) ([#234](https://github.com/arman-jalili/rigorix-oss/pull/234))
- [Issue #338] Contract freeze: orchestrator — all interfaces, DTOs, events, and API contracts defined ([#344](https://github.com/arman-jalili/rigorix-oss/pull/344))
- [Issue #339] OrchestratorService: full lifecycle implementation ([#345](https://github.com/arman-jalili/rigorix-oss/pull/345))
- [Issue #340] OrchestratorBuilder: dependency injection wiring ([#346](https://github.com/arman-jalili/rigorix-oss/pull/346))
- [Issue #341] ExecutionRecord: constructors, helpers, and tests ([#347](https://github.com/arman-jalili/rigorix-oss/pull/347))
- [Issue #342] Proofing: orchestrator validation scripts + CI integration ([#348](https://github.com/arman-jalili/rigorix-oss/pull/348))
- [Issue #343] Architecture readiness: orchestrator — runbook, DR plan, docs sync ([#349](https://github.com/arman-jalili/rigorix-oss/pull/349))
- [Issue #387] Contract Freeze: Define interfaces and contracts for code-graph ([#397](https://github.com/arman-jalili/rigorix-oss/pull/397))
- [Issue #388] CodeGraph: Implement core directed multigraph ([#398](https://github.com/arman-jalili/rigorix-oss/pull/398))
- [Issue #392] PersistedCodeGraph: On-disk filesystem persistence ([#399](https://github.com/arman-jalili/rigorix-oss/pull/399))
- [Issue #391] CodeGraphBuilder: Workspace scanning and import parsing ([#400](https://github.com/arman-jalili/rigorix-oss/pull/400))
- [Issue #395] Proofing: Validation scripts + CI integration ([#401](https://github.com/arman-jalili/rigorix-oss/pull/401))
- [Issue #396] Architecture Readiness: Runbook, DR plan, docs ([#402](https://github.com/arman-jalili/rigorix-oss/pull/402))
- Contract Freeze: permission-enforcer module ([#467](https://github.com/arman-jalili/rigorix-oss/pull/467))
- Implement PermissionEnforcer — concrete service and factory ([#468](https://github.com/arman-jalili/rigorix-oss/pull/468))
- [Issue #590] Contract Freeze: ci-integration - define StatusCheckManager & PrCommentManager interfaces ([#583](https://github.com/arman-jalili/rigorix-oss/pull/583))
- [Issue #591] Implement StatusCheckManager - GitHub commit status checks ([#595](https://github.com/arman-jalili/rigorix-oss/pull/595))
- [Issue #592] Implement PrCommentManager - structured PR review comments ([#596](https://github.com/arman-jalili/rigorix-oss/pull/596))
- [Issue #593] Proofing & CI Enforcement for ci-integration ([#597](https://github.com/arman-jalili/rigorix-oss/pull/597))
- [Issue #594] Architecture Readiness: ci-integration epic closeout ([#598](https://github.com/arman-jalili/rigorix-oss/pull/598))
- Issue-contract-freeze: Contract Freeze for audit-posting module ([#608](https://github.com/arman-jalili/rigorix-oss/pull/608))
- Issue-auditbackend: Implement AuditBackend trait (open-core boundary) ([#609](https://github.com/arman-jalili/rigorix-oss/pull/609))
- Issue-signedauditrecord: Implement HMAC-SHA256 signing for audit records ([#610](https://github.com/arman-jalili/rigorix-oss/pull/610))
- Issue-auditposter: Implement AuditPoster service and AuditRecordQueue ([#611](https://github.com/arman-jalili/rigorix-oss/pull/611))

### Fixed

- Correct template-generation architecture doc paths to match actual code ([#204](https://github.com/arman-jalili/rigorix-oss/pull/204))
- Complete is_retriable() coverage for all domain errors (H-06) ([#224](https://github.com/arman-jalili/rigorix-oss/pull/224))
- Move classify.rs to application layer (M-05) ([#225](https://github.com/arman-jalili/rigorix-oss/pull/225))
- Load template parameters before extraction, fix graph generation ([#406](https://github.com/arman-jalili/rigorix-oss/pull/406))
- Wire composition root — route all 10 OSS tools in main.rs ([#663](https://github.com/arman-jalili/rigorix-oss/pull/663))
- Complete acceptance criterion #12 — E2E execute → audit test, wire audit storage ([#664](https://github.com/arman-jalili/rigorix-oss/pull/664))

### Internal

- Comprehensive .gitignore + project scaffold and contract freeze ([#7](https://github.com/arman-jalili/rigorix-oss/pull/7))
- Fully decouple template_generation from planning module ([#206](https://github.com/arman-jalili/rigorix-oss/pull/206))
- Restructure CLI to per-module DDD architecture ([#259](https://github.com/arman-jalili/rigorix-oss/pull/259))
- Split views and widgets into separate files ([#382](https://github.com/arman-jalili/rigorix-oss/pull/382))

<details><summary>Issues closed in this release (418)</summary>

- #1 — Epic: "configuration"
- #2 — Contract Freeze: Define interfaces and contracts
- #3 — Config: Top-level configuration with multi-source loading
- #4 — Secret: API key wrapper that redacts in all output
- #5 — Proofing: Validation scripts + CI integration
- #6 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #12 — Epic: "error-handling"
- #13 — Contract Freeze: Define interfaces and contracts
- #14 — AuditEnvelope: Typed envelope containing execution audit data
- #15 — Proofing: Validation scripts + CI integration
- #16 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #21 — Epic: "cancellation"
- #22 — Contract Freeze: Define interfaces and contracts
- #23 — CancellationManager: Central manager for execution cancellation with dual shutdown levels
- #24 — ShutdownSignal: pub enum ShutdownSignal {
- #25 — Proofing: Validation scripts + CI integration
- #26 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #32 — Epic: "failure-classification"
- #33 — Contract Freeze: Define interfaces and contracts
- #34 — FailureType: Classify execution failures into typed categories
- #35 — RetryStrategy: Define what to do on failure
- #36 — classify_failure(): pub fn classify_failure(error_message: &str) -> FailureType;
- #37 — Proofing: Validation scripts + CI integration
- #38 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #45 — Epic: "event-system"
- #46 — Contract Freeze: Define interfaces and contracts
- #47 — EventBus: Central pub-sub event bus with synchronous in-memory persistence
- #48 — ExecutionEvent — All 11 Variants: pub enum ExecutionEvent {
- #49 — Proofing: Validation scripts + CI integration
- #50 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #56 — Epic: "enforcement"
- #57 — Contract Freeze: Define interfaces and contracts
- #58 — EnforcementConfig: Hard cap configuration with validated safety limits
- #59 — ExecutionEnforcer: Atomic runtime enforcer that gates every action against limits
- #60 — Proofing: Validation scripts + CI integration
- #61 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #67 — Epic: "budget-tracking"
- #68 — Contract Freeze: Define interfaces and contracts
- #69 — LlmBudget: Track and enforce LLM usage with RAII reservation
- #70 — LlmBudgetReservation: RAII guard that holds budget capacity
- #71 — Proofing: Validation scripts + CI integration
- #72 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #77 — Epic: "state-persistence"
- #78 — Contract Freeze: Define interfaces and contracts
- #79 — ExecutionState: Serializable snapshot of an entire execution
- #80 — StateManager: Atomic persistence with cross-process locking
- #81 — Proofing: Validation scripts + CI integration
- #82 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #88 — Epic: "risk-gating"
- #89 — Contract Freeze: Define interfaces and contracts
- #90 — RiskClassifier: Determines risk level of a tool based on its name and parameters
- #91 — RiskLevel: **Definition:** `rigorix/src/core.rs`
- #92 — RiskConfig: **Definition:** `rigorix/src/config.rs`
- #93 — Proofing: Validation scripts + CI integration
- #94 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #101 — Epic: "template-system"
- #102 — Contract Freeze: Define interfaces and contracts
- #103 — TemplateParser: Parse TOML template files into validated Template structs
- #104 — TemplateEngine: Runtime registry that holds registered templates and instantiates them into exec
- #105 — Proofing: Validation scripts + CI integration
- #106 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #112 — Epic: "dag-engine"
- #113 — Contract Freeze: Define interfaces and contracts
- #114 — TaskGraph: Core DAG data structure supporting two-phase construction
- #115 — ExecutionPolicy: Per-node execution and retry configuration
- #116 — Proofing: Validation scripts + CI integration
- #117 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #123 — Epic: "tool-system"
- #124 — Contract Freeze: Define interfaces and contracts
- #125 — Tool Trait: Core abstraction for all tool implementations
- #126 — ToolRegistry: Registry of all available tools by name with lookup
- #127 — Proofing: Validation scripts + CI integration
- #128 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #137 — Epic: "repo-engine"
- #138 — Contract Freeze: Define interfaces and contracts
- #139 — SymbolGraph: Thread-safe in-memory graph of code symbols with O(1) lookup
- #140 — SymbolDefinition: pub struct SymbolDefinition {
- #141 — SymbolWorkspaceIntent: Each TaskNode carries a `SymbolWorkspaceIntent` that describes how it interacts
- #142 — Proofing: Validation scripts + CI integration
- #143 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #150 — Epic: "planning-pipeline"
- #151 — Contract Freeze: Define interfaces and contracts
- #152 — PlanningPipeline: Orchestrate the 6-phase planning flow
- #153 — Classifier Trait: Abstract LLM-based intent classification
- #154 — Proofing: Validation scripts + CI integration
- #155 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #160 — Epic: "template-generation"
- #161 — Contract Freeze: Define interfaces and contracts
- #162 — TemplateGenerator Trait: Abstract interface for LLM-based template generation
- #163 — ClaudeTemplateGenerator: Production generator using Anthropic Messages API with structured prompt enginee
- #164 — Phase 3: Symbol Validation: Validates generated template against indexed symbol graph before registration:
- #165 — Proofing: Validation scripts + CI integration
- #166 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #173 — Epic: "error-handling"
- #174 — Contract Freeze: Define interfaces and contracts
- #175 — AuditEnvelope: Typed envelope containing execution audit data
- #176 — AuditSender: Deliver envelopes via HTTP with retry logic
- #177 — AuditQueue: Queue for failed deliveries
- #178 — CircuitBreaker: Circuit breaker for HTTP resilience
- #179 — Proofing: Validation scripts + CI integration
- #180 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #186 — Epic: "error-handling"
- #187 — Contract Freeze: Define interfaces and contracts
- #188 — CoreOrchestratorError: Root error type with `#[from]` for all sub-errors
- #189 — Proofing: Validation scripts + CI integration
- #190 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #194 — Epic: "execution-engine"
- #195 — Contract Freeze: Define interfaces and contracts
- #196 — ParallelExecutor: Schedule and run all DAG nodes concurrently respecting dependencies
- #197 — Retry Logic: The executor implements an **inline retry loop** per node (not using `execute_wi
- #198 — Proofing: Validation scripts + CI integration
- #199 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #207 — [EPIC-002] Move classifiers out of domain layer (H-04)
- #208 — [EPIC-002] Merge execution stub into execution_engine (H-05)
- #209 — [EPIC-002] Fix 24 compiler warnings (H-01)
- #210 — [EPIC-002] Per-module is_retriable() delegation (H-06)
- #211 — [EPIC-001] Add tracing crate + instrument all services (C-01)
- #212 — [EPIC-001] Centralized HealthService (C-02)
- #213 — [EPIC-001] Prometheus /metrics endpoints (C-02)
- #214 — [EPIC-001] Health endpoints for remaining 13 modules (C-02)
- #215 — [EPIC-003] Concurrent-safety tests (C-03)
- #216 — [EPIC-003] Cross-module integration test suite (H-02)
- #217 — [EPIC-003] Live LLM API integration tests (H-03)
- #218 — Tracking: EPIC-002 — Architecture & Code Quality
- #219 — Tracking: EPIC-001 — Observability Foundation
- #220 — Tracking: EPIC-003 — Testing Hardening
- #235 — Epic: "cli-boundary"
- #236 — Contract Freeze: Define interfaces and contracts
- #237 — Phase 1: CLI Foundation Scaffold — config, tracing, signals
- #238 — Proofing: Validation scripts + CI integration
- #239 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #244 — Epic: "configuration"
- #245 — Contract Freeze: Define interfaces and contracts
- #246 — [Component Name]: [What this component does]
- #247 — Proofing: Validation scripts + CI integration
- #248 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #252 — Epic: "observability"
- #253 — Contract Freeze: Define interfaces and contracts
- #254 — [Component Name]: [What this component does]
- #255 — Proofing: Validation scripts + CI integration
- #256 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #260 — Epic: "templates"
- #261 — Contract Freeze: Define interfaces and contracts
- #262 — [Component Name]: [What this component does]
- #263 — Proofing: Validation scripts + CI integration
- #264 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #265 — Epic: "templates"
- #266 — Contract Freeze: Define interfaces and contracts
- #267 — [Component Name]: [What this component does]
- #268 — Proofing: Validation scripts + CI integration
- #269 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #273 — Epic: "cancellation"
- #274 — Contract Freeze: Define interfaces and contracts
- #275 — [Component Name]: [What this component does]
- #276 — Proofing: Validation scripts + CI integration
- #277 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #281 — Epic: "configuration"
- #282 — Contract Freeze: Define interfaces and contracts
- #283 — [Component Name]: [What this component does]
- #284 — Proofing: Validation scripts + CI integration
- #285 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #288 — Epic: "observability"
- #289 — Contract Freeze: Define interfaces and contracts
- #290 — [Component Name]: [What this component does]
- #291 — Proofing: Validation scripts + CI integration
- #292 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #295 — Epic: "cli-boundary"
- #296 — Contract Freeze: Define interfaces and contracts
- #297 — [Component Name]: [What this component does]
- #298 — Proofing: Validation scripts + CI integration
- #299 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #302 — Epic: "template-generation"
- #303 — Contract Freeze: Define interfaces and contracts
- #304 — [Component Name]: [What this component does]
- #305 — Proofing: Validation scripts + CI integration
- #306 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #309 — Epic: "planning-pipeline"
- #310 — Contract Freeze: Define interfaces and contracts
- #311 — [Component Name]: [What this component does]
- #312 — Proofing: Validation scripts + CI integration
- #313 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #316 — Epic: "execution-engine"
- #317 — Contract Freeze: Define interfaces and contracts
- #318 — [Component Name]: [What this component does]
- #319 — Proofing: Validation scripts + CI integration
- #320 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #323 — Epic: "state-persistence"
- #324 — Contract Freeze: Define interfaces and contracts
- #325 — [Component Name]: [What this component does]
- #326 — Proofing: Validation scripts + CI integration
- #327 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #330 — Epic: "event-system"
- #331 — Contract Freeze: Define interfaces and contracts
- #332 — [Component Name]: [What this component does]
- #333 — Proofing: Validation scripts + CI integration
- #334 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #337 — Epic: "orchestrator"
- #338 — Contract Freeze: Define interfaces and contracts
- #339 — OrchestratorService: Single entry point for executing a Rigorix run from intent to result.
- #340 — OrchestratorBuilder: Constructs an Orchestrator from a Config, wiring all internal dependencies.
- #341 — ExecutionRecord: Complete output of a run, containing everything needed for audit, TUI, and persi
- #342 — Proofing: Validation scripts + CI integration
- #343 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #350 — Epic: "cli-boundary"
- #351 — Contract Freeze: Define interfaces and contracts
- #352 — CliParser: Clap argument parser defining 14 CLI commands, global flags (--format, -v), and
- #353 — Dispatcher: Match parsed `CliCommand` to the appropriate handler — orchestrator (run/plan/ca
- #354 — OrchestratorBuilder: Build and wire `OrchestratorService` with config + repo_root. Passes shared `Can
- #355 — ConfigLoader: Merge `rigorix.toml` + environment variables (`RIGORIX_*`) + CLI flags with prio
- #356 — OutputFormatter: `LogFormatter` trait with four implementations — Pretty (human-readable with Uni
- #357 — SignalHandler: Ctrl+C detection with two-level cancellation (single = graceful finish in-flight
- #358 — TracingInit: Initialize `tracing-subscriber` with `RIGORIX_LOG` env filter. Installed at star
- #359 — CliError: Error type mapping engine errors to CLI exit codes (0 success, 1 general error,
- #360 — Proofing: Validation scripts + CI integration
- #361 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #366 — Epic: "tui"
- #367 — Contract Freeze: Define interfaces and contracts
- #368 — CommandBar: Persistent text input at screen bottom for intents and `/commands`. Supports aut
- #369 — PlanReview: Plan preview view showing template, confidence, DAG nodes, parameters, and actio
- #370 — EventBridge: Async subscriber converting engine `ExecutionEvent`s into `ViewModelMutation`s v
- #371 — ViewModel: Root state model with double-buffering — `TuiViewModel` (execution phase, DAG tr
- #372 — Renderer: ratatui widget rendering: DAG Tree, Progress Bar, Status Bar, Event Log, Modal,
- #373 — Views: Full-screen view implementations: Dashboard (DAG tree + details + metrics), Node
- #374 — InputHandler: Keyboard event dispatch with global key bindings (Esc, Tab, q, F1, Ctrl+C), comm
- #375 — OrchestratorSpawner: Spawns engine orchestrator in a `tokio::spawn`'d background task. Builds same `O
- #376 — Proofing: Validation scripts + CI integration
- #377 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #386 — Epic: "code-graph"
- #387 — Contract Freeze: Define interfaces and contracts
- #388 — CodeGraph: Core directed multigraph of module-level dependencies. Nodes are source files; e
- #389 — ModuleNode: pub struct ModuleNode {
- #390 — ModuleEdge: pub struct ModuleEdge {
- #391 — CodeGraphBuilder: Scans the workspace, parses import statements via tree-sitter, constructs the Co
- #392 — PersistedCodeGraph: On-disk serialization format stored at `.rigorix/code-graph.bin`.
- #393 — CodeGraphService: Service interface for graph queries. Used by the Planning Pipeline and template
- #394 — CodeGraphFormatter: Formats the module dependency graph into a compact string for the LLM system pro
- #395 — Proofing: Validation scripts + CI integration
- #396 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #409 — Epic: "hooks"
- #410 — Contract Freeze: Define interfaces and contracts
- #411 — HookEvent: Identifies which lifecycle point a hook runs at
- #412 — Hook Protocol: JSON contract between the engine and hook scripts
- #413 — HookRunResult: Aggregated result from running all hook commands for an event
- #414 — HookRunner: Executes all registered hook commands for a given event and aggregates results
- #415 — HookConfig: Declarative hook command registration per event, loaded from config
- #416 — Proofing: Validation scripts + CI integration
- #417 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #423 — Epic: "code-generatio"
- #424 — Contract Freeze: Define interfaces and contracts
- #425 — EditFileTool: The primary code insertion mechanism. Replaces an exact text string in a file wi
- #426 — EditFileInput: The structured input the LLM provides to target an edit
- #427 — EditFileResult: Structured feedback returned to the LLM after every edit — enables self-verifica
- #428 — StructuredPatchHunk: Individual diff hunk for structured diff output
- #429 — ReadFileTool: Read files with offset/limit paging and binary detection
- #430 — SyntaxGate: Post-edit tree-sitter syntax verification — uses Rigorix's existing `code_graph`
- #431 — Proofing: Validation scripts + CI integration
- #432 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #438 — Epic: "recovery-recipes"
- #439 — Contract Freeze: Define interfaces and contracts
- #440 — FailureScenario: Typed enumeration of all known recoverable failure scenarios
- #441 — RecoveryStep: Individual executable recovery actions
- #442 — RecoveryRecipe: Binds a scenario to its recovery steps and escalation policy
- #443 — RecoveryContext: Tracks per-scenario attempt counts and recovery events within an execution sessi
- #444 — RecoveryResult: pub enum RecoveryResult {
- #445 — Proofing: Validation scripts + CI integration
- #446 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #449 — Epic: "quality-gate"
- #450 — Contract Freeze: Define interfaces and contracts
- #451 — QualityLevel: Four-tier escalation of test scope
- #452 — GreenContract: Declares required quality level and evaluates observed level
- #453 — QualityGateOutcome: pub enum QualityGateOutcome {
- #454 — Proofing: Validation scripts + CI integration
- #455 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #458 — Epic: "permission-enforcer"
- #459 — Contract Freeze: Define interfaces and contracts
- #460 — PermissionMode: Three-tier hierarchy controlling tool execution scope
- #461 — PermissionPolicy: Authorization decisions based on active mode and tool requirements
- #462 — PermissionEnforcer: Three specific checks: general tool gating, file write boundaries, bash command
- #463 — BashClassifier: Classifies bash commands into intent categories for mode-aware gating
- #464 — EnforcementResult: Structured outcome of permission enforcement
- #465 — Proofing: Validation scripts + CI integration
- #466 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #469 — Epic: "policy-engine"
- #470 — Contract Freeze: Define interfaces and contracts
- #471 — PolicyRule: A single policy rule with priority
- #472 — PolicyCondition: Composable conditions evaluated against LaneContext
- #473 — PolicyAction: What the engine does when a rule matches
- #474 — LaneContext: Typed snapshot of execution state evaluated by conditions
- #475 — PolicyEngine: Evaluates rules against context, returns flat action list
- #476 — Proofing: Validation scripts + CI integration
- #477 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #481 — Epic: "llm-step"
- #482 — Contract Freeze: Define interfaces and contracts
- #483 — LlmGenerateNode: The execution engine sees this as just another node type — but internally it cal
- #484 — LlmStepContext: Provides the LLM with source code context and previous failure analysis
- #485 — LlmStepService: Application service for LLM generation during execution
- #486 — Proofing: Validation scripts + CI integration
- #487 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #494 — Epic: "failure-parser"
- #495 — Contract Freeze: Define interfaces and contracts
- #496 — TemplateFailure: Typed classification of template execution failures
- #497 — FailureParserService: Parse raw output into structured failures with suggestions
- #498 — TypeScript Parser: Parse `tsc --noEmit --pretty false` output into structured failures
- #499 — Suggested Fix Generation: **How suggestions are derived:**
- #500 — Proofing: Validation scripts + CI integration
- #501 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #509 — Epic: "plan-validation"
- #510 — Contract Freeze: Define interfaces and contracts
- #511 — ValidationLoop Config: pub struct ValidationLoopConfig {
- #512 — ValidationState: pub struct ValidationState {
- #513 — ValidationLoopService: pub trait ValidationLoopService: Send + Sync {
- #514 — ContextAugmenter: /// Transforms failure analysis into augmented planning context.
- #515 — Proofing: Validation scripts + CI integration
- #516 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #520 — Epic: "action-input"
- #521 — Contract Freeze: Define interfaces and contracts
- #522 — ActionInputs: Typed container of all GitHub Action inputs
- #523 — InputParser: Parse all GitHub Action inputs from environment variables
- #524 — CommentParser: Parse `/rigorix` slash commands from issue and PR comments
- #525 — CiDetector: Detects the CI environment and adjusts engine configuration
- #526 — ConfigLoader: Loads `action.yml` defaults and merges with environment overrides
- #527 — Proofing: Validation scripts + CI integration
- #528 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #537 — Epic: "security-config"
- #538 — Contract Freeze: Define interfaces and contracts
- #539 — ForkDetector: /// Detects whether a PR originates from a forked repository.
- #540 — SecretMasker: /// Masks sensitive values from GitHub Actions workflow logs.
- #541 — TokenValidator: /// Validates that the GitHub token has the required permissions.
- #542 — UrlAllowlist: /// Validates backend URLs against a configured allowlist.
- #543 — HmacSigner: /// HMAC-SHA256 signing for audit record integrity.
- #544 — Proofing: Validation scripts + CI integration
- #545 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #551 — Epic: "diff-analyzer"
- #552 — Contract Freeze: Define interfaces and contracts
- #553 — PrDiff: /// Structured representation of a Pull Request diff.
- #554 — PathValidator: /// Validates file paths for security and correctness.
- #555 — LimitEnforcer: /// Enforces resource limits on PR diffs.
- #556 — RiskClassifier: /// Classifies changed files by risk level based on path patterns.
- #557 — AiSignalDetector: /// Detects AI-generated code signals in PR diffs.
- #558 — Proofing: Validation scripts + CI integration
- #559 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #564 — Epic: "policy-evaluator"
- #565 — Contract Freeze: Define interfaces and contracts
- #566 — PolicyDocument: /// Complete policy document loaded from `.rigorix/policy.toml`.
- #567 — PolicyRule Types: /// A deny rule: if a changed file matches this pattern, the PR is blocked.
- #568 — PolicyLoader: /// Loads policy from the repository's base branch.
- #569 — PolicyEvaluator: /// Evaluates PR diff against policy rules.
- #570 — OrgPolicyMerger: /// Merges organization-level policy with repository policy.
- #571 — Proofing: Validation scripts + CI integration
- #572 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #576 — Epic: "action-output"
- #577 — Contract Freeze: Define interfaces and contracts
- #578 — OutputFormatter: Top-level formatter that orchestrates all output channels
- #579 — AnnotationWriter: Emits GitHub workflow annotations for template failures
- #580 — StepSummaryWriter: Writes GitHub Actions step summaries (markdown rendered in the Actions UI)
- #581 — Proofing: Validation scripts + CI integration
- #582 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #589 — Epic: "ci-integration"
- #590 — Contract Freeze: Define interfaces and contracts
- #591 — StatusCheckManager: Creates and updates GitHub commit status checks
- #592 — PrCommentManager: Posts structured PR review comments
- #593 — Proofing: Validation scripts + CI integration
- #594 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #599 — Epic: "audit-posting"
- #600 — Contract Freeze: Define interfaces and contracts
- #601 — AuditBackend Trait (Open-Core Boundary): /// Trait for posting audit records to an external backend.
- #602 — SignedAuditRecord: /// An HMAC-signed audit record posted to the backend.
- #603 — FilesystemAuditBackend (OSS Default): /// Ships with the OSS action. Stores audit records as JSON files
- #604 — AuditPoster: /// Orchestrates audit record posting with retry and circuit breaker.
- #605 — AuditRecordQueue: /// Local queue for audit records when the backend is unavailable.
- #606 — Proofing: Validation scripts + CI integration
- #607 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #612 — Epic: "action-entrypoint"
- #613 — Contract Freeze: Define interfaces and contracts
- #614 — ActionRouter: Map GitHub Action event types to engine service calls
- #615 — ActionContext: GitHub Actions environment parsed into a typed context
- #616 — ActionMode: Determines what the engine should do
- #617 — Proofing: Validation scripts + CI integration
- #618 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #624 — Epic: mcp-server
- #625 — Contract Freeze: Define interfaces and contracts
- #626 — McpServer: The core server orchestrating transport, session, tool registry, and routing.
- #627 — ToolRegistry: Central registry of all registered tools with their schemas and handlers.
- #628 — Proofing: Validation scripts + CI integration
- #629 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #631 — Epic: execution-tools
- #632 — Contract Freeze: Define interfaces and contracts
- #633 — EngineFacade (Aggregate Root): Thin async facade over rigorix-engine that all execution/audit/template tools ca
- #634 — ExecuteHandler (Domain Service): Handles `rigorix_execute` tool calls: validates input, delegates to engine, form
- #635 — ValidatePlanHandler (Domain Service): Handles `rigorix_validate_plan` tool calls: checks plan against enforcement poli
- #636 — CheckEnforcementHandler (Domain Service): Handles `rigorix_check_enforcement` tool calls: queries engine for current budge
- #637 — Proofing: Validation scripts + CI integration
- #638 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #642 — Epic: audit-tools
- #643 — Contract Freeze: Define interfaces and contracts
- #644 — AuditQueryService (Aggregate Root): Service that queries audit records from rigorix-engine and formats them for MCP.
- #645 — ReadAuditHandler (Domain Service): Handles `rigorix_read_audit` tool calls: retrieves audit by execution ID.
- #646 — ListAuditsHandler (Domain Service): Handles `rigorix_list_audits` tool calls: lists recent executions with filtering
- #647 — AuditSummaryHandler (Domain Service): Handles `rigorix_audit_summary` tool calls: generates aggregate statistics.
- #648 — AuditFormatter (Domain Service): Formats audit data for MCP consumption: human-readable markdown or structured JS
- #649 — Proofing: Validation scripts + CI integration
- #650 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #652 — Epic: template-tools
- #653 — Contract Freeze: Define interfaces and contracts
- #654 — TemplateRepository (Aggregate Root): Filesystem-backed repository for plan templates stored as TOML files in `.rigori
- #655 — ListTemplatesHandler (Domain Service): Handles `rigorix_list_templates` tool calls: discovers templates from filesystem
- #656 — GetTemplateHandler (Domain Service): Handles `rigorix_get_template` tool calls: reads and returns a specific template
- #657 — CreateTemplateHandler (Domain Service): Handles `rigorix_create_template` tool calls: validates and saves a new template
- #658 — ValidateTemplateHandler (Domain Service): Handles `rigorix_validate_template` tool calls: validates template structure.
- #659 — TemplateConverter (Domain Service): Converts between TOML (filesystem storage) and JSON (MCP transport) template for
- #660 — Proofing: Validation scripts + CI integration
- #661 — Architecture Readiness: Runbook, DR, docs, CI enforcement
- #665 — Epic: enterprise-proxy
- #666 — Contract Freeze: Define interfaces and contracts
- #667 — EnterpriseProxy (Aggregate Root): Proxies `rigorix_enterprise_*` tool calls to the enterprise API.
- #668 — SchemaCache (Domain Service): Caches enterprise tool schemas for capability negotiation during MCP initializat
- #669 — Proofing: Validation scripts + CI integration
- #670 — Architecture Readiness: Runbook, DR, docs, CI enforcement

</details>

