//! Benchmarks for DAG engine operations.
//! @canonical .pi/architecture/modules/execution-engine.md#enforcement-path-cost-baseline-1208
//!
//!
//! Measures topological sort, graph seal, and ready queue performance
//! across varying graph sizes.

use std::sync::Arc;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use uuid::Uuid;

use rigorix_engine::dag_engine::domain::graph::{TaskGraph, TaskNode};
use rigorix_engine::effect_commitment::DispatchSeam;
use rigorix_engine::effect_commitment::application::EffectCommitmentSetup;
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::execution_engine::application::dto::ExecuteGraphInput;
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use rigorix_engine::execution_engine::domain::ParallelExecutorConfig;

fn build_graph(node_count: usize, edge_factor: usize) -> TaskGraph {
    let mut graph = TaskGraph::new();
    let mut prev_nodes: Vec<Uuid> = Vec::new();

    for i in 0..node_count {
        let id = Uuid::new_v4();
        let deps: Vec<Uuid> = if edge_factor > 0 && !prev_nodes.is_empty() {
            prev_nodes.iter().rev().take(edge_factor).copied().collect()
        } else {
            vec![]
        };

        let node = TaskNode::new(id, format!("n{}", i), "echo", deps, "bench");
        let _ = graph.add_unchecked(node);
        prev_nodes.push(id);
    }

    graph
}

fn bench_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("topological_sort");
    for &size in &[10, 50, 100] {
        let graph = build_graph(size, 2);
        group.bench_with_input(format!("{}_nodes", size), &graph, |b, g| {
            b.iter(|| {
                let mut g = g.clone();
                let _ = black_box(g.seal());
            });
        });
    }
    group.finish();
}

fn bench_seal_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("seal_graph");
    for &size in &[10, 50, 100] {
        let graph = build_graph(size, 0);
        group.bench_with_input(format!("{}_nodes", size), &graph, |b, g| {
            b.iter(|| {
                let mut g = g.clone();
                let _ = black_box(g.seal());
            });
        });
    }
    group.finish();
}

/// A graph whose nodes do almost no work, so the measurement is the gate
/// sequence plus dispatch, not the tool. `file_read` is the cheapest shipped
/// tool; the file lives in the repo so the path is stable from any cwd.
fn build_execution_graph(node_count: usize) -> TaskGraph {
    let manifest = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let intent = format!(r#"{{"path": "{manifest}"}}"#);
    let mut graph = TaskGraph::new();
    for i in 0..node_count {
        let node = TaskNode::new(
            Uuid::new_v4(),
            format!("n{i}"),
            "file_read",
            vec![],
            &intent,
        );
        graph.add_unchecked(node).expect("add unchecked");
    }
    graph.seal().expect("seal");
    graph
}

fn executor(seam: Option<Arc<dyn DispatchSeam>>) -> ParallelExecutionServiceImpl {
    // One node at a time: the measurement is the marginal gate cost, not how
    // well tokio schedules parallel file reads on the benchmark machine.
    let config = ParallelExecutorConfig {
        max_concurrent_executions: 1,
        ..ParallelExecutorConfig::default()
    };
    let executor = ParallelExecutionServiceImpl::new(
        config,
        Box::new(RetryEvaluationServiceImpl::new()),
        Arc::new(EventBusServiceImpl::default()),
    );
    match seam {
        Some(seam) => executor.with_effect_commitment_seam(seam),
        None => executor,
    }
}

fn in_memory_seam(runtime: &tokio::runtime::Runtime) -> Arc<dyn DispatchSeam> {
    let dir = std::env::temp_dir().join(format!("rigorix-bench-effect-{}", std::process::id()));
    std::fs::create_dir_all(dir.join(".rigorix")).expect("create bench declaration dir");
    std::fs::write(
        dir.join(".rigorix/effect-commitment.toml"),
        "[backend]\nkind = \"in_memory\"\n",
    )
    .expect("write bench declaration");
    runtime
        .block_on(EffectCommitmentSetup::from_env(&dir))
        .expect("valid bench declaration")
        .expect("armed seam")
        .dispatch_seam()
}

/// #1208: the marginal cost of the gate sequence on `execute_graph`.
///
/// The baseline is the default path (no ADR-019 seam); `seam_in_memory` adds
/// the write-ahead record + settle per node. A fresh `dag_id` per iteration
/// keeps the in-memory ledger keyed per execution, so the measurement does not
/// hit the retry fence after the first iteration.
fn bench_enforcement_path(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let seam = in_memory_seam(&runtime);
    let executors = [
        ("seam_absent", executor(None)),
        ("seam_in_memory", executor(Some(seam))),
    ];

    let mut group = c.benchmark_group("enforcement_path");
    for &size in &[1, 10, 100] {
        for (label, executor) in &executors {
            let graph = build_execution_graph(size);
            group.bench_with_input(format!("{label}/{}_nodes", size), &size, |b, _| {
                b.iter(|| {
                    let input = ExecuteGraphInput {
                        dag_id: Uuid::new_v4(),
                        graph: Some(graph.clone()),
                        config_override: None,
                    };
                    let output = runtime
                        .block_on(executor.execute_graph(input))
                        .expect("execute graph");
                    black_box(output.result.completed_count)
                });
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_topological_sort,
    bench_seal_graph,
    bench_enforcement_path
);
criterion_main!(benches);
