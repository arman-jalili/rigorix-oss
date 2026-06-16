//! Benchmarks for DAG engine operations.
//!
//! Measures topological sort, graph seal, and ready queue performance
//! across varying graph sizes.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use uuid::Uuid;

use rigorix::dag_engine::domain::graph::{TaskGraph, TaskNode};

fn build_graph(node_count: usize, edge_factor: usize) -> TaskGraph {
    let mut graph = TaskGraph::new();
    let mut prev_nodes: Vec<Uuid> = Vec::new();

    for i in 0..node_count {
        let id = Uuid::new_v4();
        let deps: Vec<Uuid> = if edge_factor > 0 && !prev_nodes.is_empty() {
            prev_nodes
                .iter()
                .rev()
                .take(edge_factor)
                .copied()
                .collect()
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

criterion_group!(benches, bench_topological_sort, bench_seal_graph);
criterion_main!(benches);
