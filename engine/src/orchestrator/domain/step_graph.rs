//! Step dependencies: names resolved to a graph, and the four ways that can fail.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#plan-time-gate
//! Implements: ADR-024 §4 `steps[].depends_on`, ADR-002 (the template artifact)
//!
//! Issue: #1526
//!
//! `OrchestratorService::run_from_template` executes a caller-supplied plan, and until this module
//! existed its steps could not depend on one another: `build_graph_from_steps` chained them in list
//! order, so every branching plan was flattened into a line and a host could not express a
//! concurrency frontier or the difference between "b failed, d was never attempted" and "d failed
//! after c".
//!
//! The shape is not new. `TemplateNode { depends_on }` and `DagNode` already express it; this
//! resolves *names* to the node ids a fresh graph allocates, which is the one thing neither of them
//! does, because a caller's plan names its steps and the engine names its nodes by `Uuid`.
//!
//! # Two modes, and why the default is the old behaviour
//!
//! A plan that declares no dependencies at all keeps the frozen sequential contract: step *n+1*
//! depends on step *n*. A byte-comparison test pins that, because a hundred existing callers rely
//! on it and none of them asked for a graph. Only when at least one step names a dependency does
//! the plan become dependency-driven, and then a step with no dependencies has none — it may run
//! first. A mixed plan cannot be half-sequential, so it is not.

use std::collections::{BTreeSet, HashMap};

/// Which step named which dependency, and why it is not a graph.
///
/// One variant per way a dependency set can be wrong. The variants exist so a caller can branch on
/// the refusal instead of parsing a sentence, and each message names the offending step, because a
/// refusal that says "invalid plan" teaches the next author nothing (#1526).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepDependencyError {
    /// A step names a dependency that is not a step in the same plan.
    UnknownDependency {
        /// The step whose `depends_on` carries the name.
        step: String,
        /// The name it names.
        dependency: String,
        /// The names that do exist, so the message can suggest the intended one.
        known: Vec<String>,
    },
    /// A step names itself.
    SelfEdge {
        /// The step.
        step: String,
    },
    /// Two steps carry the same name, so a dependency on that name means two nodes.
    DuplicateStepName {
        /// The name that appears twice.
        name: String,
    },
    /// The dependencies are satisfiable in no order.
    Cycle {
        /// The steps that remain when no step is ready — the cycle plus anything behind it.
        steps: Vec<String>,
    },
}

impl StepDependencyError {
    /// A stable kind, for a caller that branches without matching on fields.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::UnknownDependency { .. } => "unknown_dependency",
            Self::SelfEdge { .. } => "self_edge",
            Self::DuplicateStepName { .. } => "duplicate_step_name",
            Self::Cycle { .. } => "cycle",
        }
    }
}

impl std::fmt::Display for StepDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDependency {
                step,
                dependency,
                known,
            } => {
                let known = if known.is_empty() {
                    "the plan has no steps".to_string()
                } else {
                    format!("known steps: {}", known.join(", "))
                };
                write!(
                    f,
                    "step '{step}' depends on '{dependency}', which is not a step in this plan \
                     ({known})"
                )
            }
            Self::SelfEdge { step } => write!(f, "step '{step}' depends on itself"),
            Self::DuplicateStepName { name } => write!(
                f,
                "two steps are named '{name}', so a dependency on that name does not identify one \
                 node"
            ),
            Self::Cycle { steps } => write!(
                f,
                "the dependencies form a cycle: no step among {} can run first",
                steps.join(", ")
            ),
        }
    }
}

impl std::error::Error for StepDependencyError {}

/// A resolved plan: the order the steps are added, and the index of each step's dependencies.
///
/// Indices rather than ids, because the ids are allocated by the graph builder and this module's
/// job is to decide *which* edges exist, not what they are called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStepGraph {
    /// Step indices in the order the caller wrote them — the graph's node order.
    pub order: Vec<usize>,
    /// For each index in `order`, the indices of its dependencies, in the caller's order.
    pub dependencies: Vec<Vec<usize>>,
    /// Whether the plan declared any dependency at all. `false` means the sequential contract.
    pub explicit: bool,
}

/// Resolve step names into a dependency graph, or refuse it.
///
/// # Errors
/// One variant per way a dependency set can be wrong: a name that is not a step, a step that names
/// itself, two steps with one name, or a set of edges with no first step.
pub fn resolve_step_dependencies(
    steps: &[(String, Vec<String>)],
) -> Result<ResolvedStepGraph, StepDependencyError> {
    let mut index_of: HashMap<&str, usize> = HashMap::with_capacity(steps.len());
    for (index, (name, _)) in steps.iter().enumerate() {
        if index_of.insert(name.as_str(), index).is_some() {
            return Err(StepDependencyError::DuplicateStepName { name: name.clone() });
        }
    }

    let explicit = steps.iter().any(|(_, deps)| !deps.is_empty());
    let known: Vec<String> = steps.iter().map(|(name, _)| name.clone()).collect();

    let mut dependencies: Vec<Vec<usize>> = Vec::with_capacity(steps.len());
    for (index, (name, deps)) in steps.iter().enumerate() {
        if !explicit {
            // The frozen contract: each step depends on its predecessor.
            let deps = if index == 0 { vec![] } else { vec![index - 1] };
            dependencies.push(deps);
            continue;
        }
        let mut resolved = Vec::with_capacity(deps.len());
        for dependency in deps {
            if *dependency == *name {
                return Err(StepDependencyError::SelfEdge { step: name.clone() });
            }
            match index_of.get(dependency.as_str()) {
                Some(&target) => resolved.push(target),
                None => {
                    return Err(StepDependencyError::UnknownDependency {
                        step: name.clone(),
                        dependency: dependency.clone(),
                        known: known.clone(),
                    });
                }
            }
        }
        dependencies.push(resolved);
    }

    // Kahn's algorithm, with the caller's list position as the tie-break so the order is stable
    // run to run: a plan-time gate that iterates the steps must see the same order every time.
    let mut in_degree = vec![0usize; steps.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); steps.len()];
    for (index, deps) in dependencies.iter().enumerate() {
        in_degree[index] = deps.len();
        for &dependency in deps {
            dependents[dependency].push(index);
        }
    }
    let mut ready: BTreeSet<usize> = (0..steps.len()).filter(|i| in_degree[*i] == 0).collect();
    let mut order = Vec::with_capacity(steps.len());
    while let Some(&next) = ready.iter().next() {
        ready.remove(&next);
        order.push(next);
        for &dependent in &dependents[next] {
            in_degree[dependent] -= 1;
            if in_degree[dependent] == 0 {
                ready.insert(dependent);
            }
        }
    }
    if order.len() != steps.len() {
        let stuck: Vec<String> = (0..steps.len())
            .filter(|i| !order.contains(i))
            .map(|i| steps[i].0.clone())
            .collect();
        return Err(StepDependencyError::Cycle { steps: stuck });
    }

    Ok(ResolvedStepGraph {
        order,
        dependencies,
        explicit,
    })
}
