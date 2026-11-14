//! What the run wrote, observed while it ran (#1570, ADR-024 §4).
//!
//! The receipt declares, per step, the paths it may touch and their content digests. This module
//! collects the other half: the paths that actually changed, and a digest of what was there when the
//! record was sealed.
//!
//! # Two sets, because one of them is always sound
//!
//! - **The run's set** is taken around the whole execution: a snapshot before `execute_graph` and one
//!   after. It answers "what did this run write" with no attribution claim, which is what settles a
//!   dispute.
//! - **A step's set** is taken from its own window — the interval between the `Running` and terminal
//!   transitions the executor publishes — and it is exact **only** when that window intersects no
//!   other step's. On a frontier (`depends_on`, #1526) the windows overlap, and what the engine can
//!   honestly say is that the paths in the overlap belong to the steps that shared it. Those steps
//!   carry the union and name the overlap, rather than one of them being credited with another's
//!   writes.
//!
//! # The oracle is the tree's, and its limits are the ones it documents
//!
//! `GitDiffEffectOracle` reports **paths**, not content, and its baseline is per snapshot rather than
//! per commit (`approval/infrastructure/effect_scope.rs:48-67`). The content digests here are taken
//! by this module, on the paths the oracle found, at seal time.
//!
//! # Cost
//!
//! Each snapshot is three `git` invocations, and the callback runs inside the dispatch loop. A
//! sequential runbook pays two snapshots per step; a frontier pays them concurrently, which is why
//! the callback does no I/O beyond the snapshot itself.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::approval::infrastructure::effect_scope::{ChangeSnapshot, GitDiffEffectOracle};
use crate::execution_engine::application::service::ExecutionProgress;
use crate::execution_engine::domain::parallel_executor::NodeStatus;
use crate::receipt::ObservedEffect;

/// One node's observation window.
#[derive(Debug)]
struct Window {
    name: String,
    start: Instant,
    end: Option<Instant>,
    opened: Option<ChangeSnapshot>,
    paths: Vec<String>,
}

/// What the run wrote, per step and as a whole.
#[derive(Debug, Default)]
pub(super) struct Observations {
    /// The run's net changed paths, with a digest of each file's bytes at seal time.
    pub run: Vec<ObservedEffect>,
    /// Per step id: the paths attributable to it, and the steps it shared its window with.
    pub per_step: BTreeMap<String, StepObservation>,
}

/// One step's observed set, and whether it is exact.
#[derive(Debug, Default, Clone)]
pub(super) struct StepObservation {
    /// The paths attributable to the step — exact when `concurrent_with` is empty.
    pub paths: Vec<String>,
    /// The steps whose windows intersected this one's, so the set above is a union. Empty means the
    /// attribution is exact.
    pub concurrent_with: Vec<String>,
}

/// Records effect snapshots while the graph runs.
pub(super) struct EffectRecorder {
    root: PathBuf,
    execution_id: uuid::Uuid,
    oracle: GitDiffEffectOracle,
    before: ChangeSnapshot,
    windows: Mutex<Vec<Window>>,
}

impl EffectRecorder {
    /// Take the baseline snapshot and start recording `execution_id`.
    ///
    /// A repository that `git` cannot read yields an empty baseline: the run is not failed over an
    /// effect observation, but the observation then says nothing rather than inventing a clean tree.
    pub(super) fn new(root: impl Into<PathBuf>, execution_id: uuid::Uuid) -> Self {
        let root = root.into();
        let oracle = GitDiffEffectOracle;
        let before = oracle.snapshot(&root).unwrap_or_default();
        Self {
            root,
            execution_id,
            oracle,
            before,
            windows: Mutex::new(Vec::new()),
        }
    }

    /// The callback the executor publishes each node's transitions to.
    ///
    /// Progress for another execution is ignored: `on_progress` has no removal, so a long-lived host
    /// accumulates callbacks, and the cheap thing to do about that is to make each one a no-op for
    /// every run but its own.
    pub(super) fn callback(self: &Arc<Self>) -> Box<dyn Fn(ExecutionProgress) + Send + Sync> {
        let recorder = Arc::clone(self);
        Box::new(move |progress: ExecutionProgress| {
            if progress.dag_id != recorder.execution_id {
                return;
            }
            recorder.record(progress);
        })
    }

    fn record(&self, progress: ExecutionProgress) {
        let name = progress.state.node_name.clone();
        let Ok(mut windows) = self.windows.lock() else {
            return;
        };
        match progress.state.status {
            NodeStatus::Running => {
                // A retry re-enters Running: keep the earliest start and the latest snapshot, so the
                // window covers everything the step did rather than only its last attempt.
                if let Some(window) = windows.iter_mut().find(|w| w.name == name) {
                    window.opened = self
                        .oracle
                        .snapshot(&self.root)
                        .ok()
                        .or(window.opened.take());
                    return;
                }
                windows.push(Window {
                    name,
                    start: Instant::now(),
                    end: None,
                    opened: self.oracle.snapshot(&self.root).ok(),
                    paths: Vec::new(),
                });
            }
            // Every non-`Running` status closes the window. The set is listed rather than caught by
            // a wildcard so a new lifecycle value is a compile error here instead of a window that
            // never closes and a step with no observation.
            NodeStatus::Completed
            | NodeStatus::Failed
            | NodeStatus::Skipped
            | NodeStatus::AwaitingApproval
            | NodeStatus::IntentMismatch
            | NodeStatus::Pending
            | NodeStatus::Ready => {
                let Some(window) = windows.iter_mut().find(|w| w.name == name) else {
                    return;
                };
                window.end = Some(Instant::now());
                if let (Some(opened), Ok(after)) =
                    (window.opened.take(), self.oracle.snapshot(&self.root))
                {
                    window.paths = self.oracle.diff(&opened, &after);
                }
            }
        }
    }

    /// Close the observation: the run's set, the per-step sets, and who shared a window with whom.
    pub(super) fn finish(&self, receipt: &crate::receipt::Receipt) -> Observations {
        let after = self.oracle.snapshot(&self.root).unwrap_or_default();
        let run_paths = self.oracle.diff(&self.before, &after);

        let windows = self
            .windows
            .lock()
            .map(|w| {
                w.iter()
                    .map(|window| {
                        (
                            window.name.clone(),
                            window.start,
                            window.end.unwrap_or_else(Instant::now),
                            window.paths.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let digests = self.content_digests(&run_paths);

        let mut per_step: BTreeMap<String, StepObservation> = BTreeMap::new();
        for (index, (name, start, end, paths)) in windows.iter().enumerate() {
            let mut concurrent = Vec::new();
            let mut union: BTreeSet<String> = paths.iter().cloned().collect();
            for (other_index, (other_name, other_start, other_end, other_paths)) in
                windows.iter().enumerate()
            {
                if other_index == index {
                    continue;
                }
                if other_start <= end && start <= other_end {
                    concurrent.push(other_name.clone());
                    union.extend(other_paths.iter().cloned());
                }
            }
            concurrent.sort();
            per_step.insert(
                name.clone(),
                StepObservation {
                    paths: union.into_iter().collect(),
                    concurrent_with: concurrent,
                },
            );
        }

        let run = run_paths
            .iter()
            .map(|path| ObservedEffect {
                path: path.clone(),
                content_digest: digests.get(path).cloned().unwrap_or_default(),
            })
            .collect();

        // A receipt step whose node never ran has no window; its set is empty and the expectation is
        // reported by the caller, which is where "no result for this step" already lives.
        for step in &receipt.steps {
            per_step.entry(step.id.clone()).or_default();
        }

        Observations { run, per_step }
    }

    /// A digest per path, read through the same discipline the engine's writers use.
    ///
    /// An empty digest means the file could not be read at seal time — deleted since the snapshot, or
    /// reached through a symlinked component, which is refused rather than followed. It never means
    /// "the digest of empty bytes", because that would be a fact the engine did not observe.
    fn content_digests(&self, paths: &[String]) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for path in paths {
            if let Ok(bytes) = read_without_following_links(&self.root, path) {
                out.insert(path.clone(), crate::receipt::sha256_hex(&bytes));
            }
        }
        out
    }
}

/// Read `relative` under `root`, refusing a symlinked component.
///
/// The write path uses `fs_at`'s handle-relative walk; this is the read that answers "what is in the
/// file the run left", and it must not follow a link out of the workspace to do it.
fn read_without_following_links(root: &Path, relative: &str) -> std::io::Result<Vec<u8>> {
    let relative = Path::new(relative);
    let mut current = root.to_path_buf();
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() && index + 1 < components.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{} is a symlinked component", current.display()),
            ));
        }
    }
    std::fs::read(&current)
}
