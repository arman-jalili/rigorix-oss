# 11 — Traceability

**Status: informative.** Requirement → implementation → test → demonstration.

A requirement without a trace is an aspiration. This table is how the specification
stays honest.

| Requirement | Implementation anchor | Verification |
|---|---|---|
| RS-001 plan before execution | planning pipeline, DAG engine | integration tests; all demos |
| RS-002 sealed acyclic plan | dag-engine (two-phase, Kahn, cycle detection) | unit + integration tests |
| RS-003 no model in decision path | dispatch path; no LLM in gates | code review; ADR-007 |
| RS-004 reject unevaluable plan | fail-closed evaluation in sequence/precondition modules | negative tests |
| RS-010 approval required | approval module; `requires_approval` | migration/conference demos |
| RS-011 intent-bound, single-use, time-bound | ADR-011 canonical intent + HMAC | `approval_binding_tests` |
| RS-012 re-derive at dispatch, halt on mismatch | dispatch loop `HaltIntent` | approval-binding tests |
| RS-013 attested approver identity | ADR-012 device flow | identity epic tests |
| RS-014 fail closed on missing key | #1070 `Err(MissingKey)` | `approval_binding_env_tests` |
| RS-020 precondition at dispatch | ADR-017 `DispatchGate` | authority-demo; engine tests |
| RS-021 fail closed on ambiguity | ADR-017 failure taxonomy | failure-mode tests |
| RS-022 no caching across the gap | per-dispatch evaluation | engine tests |
| RS-023 no short-circuit between preconditions | ADR-017 R4 non-inheritance | R4 tests |
| RS-024 check and authority digests | signed findings | authority-demo output |
| RS-025 check outside agent write scope | `--isolated` mode; out-of-repo check | authority-demo boundary docs |
| RS-026 companion step must succeed | #1069 `evaluate_companion_prerequisite` | companion-prerequisite tests |
| RS-030 plan-time sequence evaluation | ADR-013 plan validation | sequence-policy tests |
| RS-031 prefix gate | dispatch loop prefix evaluation | engine tests |
| RS-032 cross-run over signed history | ADR-013 R7 | conference-demo Session A′ |
| RS-033 promote or refuse only | ADR-013 actions | config validation tests |
| RS-034 parameter cross-reference | ADR-014 JSON-pointer predicates | matcher tests |
| RS-035 canonical effect keys | ADR-014 | payouts-demo |
| RS-040 reserve-then-authorize | ADR-018 service + guard | capacity contract suite |
| RS-041 release on drop | `ReservationGuard` | guard tests |
| RS-042 refuse on unknown capacity | `LedgerError::Unavailable` | HTTP adapter tests |
| RS-043 no cross-session claim in-process | ADR-018 claim ceiling | docs review |
| RS-050 one gate for all routes | #1074 `gate.rs` | gate parity tests |
| RS-051 refuse when unevaluable | all gate modules | negative tests |
| RS-052 control posture reporting | `rigorix doctor` (#1103/#1105) | doctor tests; new seams must be added |
| RS-053 reject compound shell | #1067 | `run_command` tests |
| RS-054 no secret inheritance | #1068 cleared env | subprocess tests |
| RS-060 decision record with inputs | envelope findings | audit tests; demos |
| RS-061 degraded marking | `evidence_degraded` | audit tests |
| RS-062 chain | ADR-016 `prev_hash`/`sequence` | chain tests |
| RS-063 external anchor | Ed25519 anchor (enterprise) | anchor tests; SDK verify |
| RS-064 verifier states its level | SDK verifier surface | verifier tests |
| RS-065 offline verification | `rigorix-verifier` (5 languages) | published packages |
| RS-066 refusal reconstructable without transcript | signed finding | authority-demo |
| RS-070 all interfaces through the gate | server/MCP/CLI/actions | parity tests |
| RS-071 no direct-execute surface | catalog subset rule | catalog tests |
| RS-072 control plane not agent-writable | permission R5 | permission tests |
| RS-073 scope checked before planning | permission enforcer | permission tests |
| RS-080–082 claim discipline | this document set | review |
| RS-090 write-ahead before dispatch | ADR-019 `EffectCommitmentService::begin` | effect-commitment tests |
| RS-091 four-state machine, non-terminal Ambiguous | `EffectState` | state-machine tests |
| RS-092 dependents fenced (F1) | fence module | fence tests |
| RS-093 retry refused (F2) | reconciliation policy | retry tests |
| RS-094 intent-derived key | `IntentKeyDerivation` | key tests |
| RS-095 key exported to provider | `EFFECT_IDEMPOTENCY_ENV` | provider tests |
| RS-096 resolver authority (F3/F4) | `ResolverIdentity`, closed errors | resolution tests |
| RS-097 durability must name the store | ADR-019; doctor derived ceiling | docs review |
| RS-098 settlement requires receipt (L3) | provider receipt reader | receipt tests |
| Effect-scope evidence wired | #1106; plan declares scope (#1175) | R5 oracle tests |

## Known gaps

| Requirement | Status | Note |
|---|---|---|
| RS-052 control posture reporting | implemented, and fragile | every new control seam must be added to `rigorix doctor` or it becomes invisible to the discipline that closed RS-052 |
| RS-043 distributed claim ceiling | enforced in docs only | needs a machine-checkable mode assertion |
| RS-063 anchor as separate party | deployment property | cannot be enforced by code; must be documented per deployment |

Gaps are expected. A traceability table with no gaps is usually a table nobody
verified.
