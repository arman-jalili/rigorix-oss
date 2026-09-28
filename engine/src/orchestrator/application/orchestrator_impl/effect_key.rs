//! R8 / ADR-014 effect-key derivation (extracted from `orchestrator_impl`,
//! #916). The coordinator calls `effect_key::run_effect_key`.

/// R8 / ADR-014: the run's domain-supplied effect key, recovered from the
/// planning parameters (`step_N` → params JSON) — the first step that
/// carries the reserved `/effect_key`. Entity resolution happens outside
/// rigorix; recording the key on the envelope lets effect-keyed history
/// rules compare it across runs.
pub(super) fn run_effect_key(
    parameters: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let mut steps: Vec<(&String, &String)> = parameters
        .iter()
        .filter(|(k, _)| k.starts_with("step_"))
        .collect();
    // step_10 must sort after step_9 — sort by the numeric suffix.
    steps.sort_by_key(|(k, _)| {
        k.trim_start_matches("step_")
            .parse::<usize>()
            .unwrap_or(usize::MAX)
    });
    steps.into_iter().find_map(|(_, raw)| {
        let value: serde_json::Value = serde_json::from_str(raw).ok()?;
        crate::sequence_policy::domain::effect_key_of(&value).map(str::to_string)
    })
}
