//! Capacity ledger demo — one hard limit, two executions, one process.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md
//! ADR-018: distributed capacity ledger
//! Implements: the runnable demo referenced by README §"See it run" (#1018)
//!
//! Run it:
//!
//! ```text
//! cargo run -q -p rigorix-engine --example capacity_ledger_demo
//! ```
//!
//! Docker-free and offline: a temp directory, one operator declaration, and the
//! real engine types. No mocks, no recorded fixtures — every number printed here
//! is read back from the ledger after the operation that changed it.
//!
//! # What it shows
//!
//! **Act 1 — the contract.** `reserve` / `commit` / `release` against one
//! resource with `hard_limit = 5`. The interesting cases are the boring ones: a
//! retry is not a second reservation, a refusal reserves nothing, and
//! `committed` and `outstanding` are never the same number (#1053).
//!
//! **Act 2 — the seam.** The same declaration, armed through
//! `CapacitySetup::from_env`, driving the *real* executor
//! (`ParallelExecutionFactoryImpl` → `execute_graph`). Two executions contend for
//! a limit of 4 with 3-unit steps: exactly one dispatches, the other is refused
//! with `capacity_denied` **before its tool runs** — the marker file the refused
//! step would have created never appears. That absence is the proof; a counter
//! that runs the tool and reports afterwards cannot produce it.

use std::path::PathBuf;
use std::sync::Arc;

use rigorix_engine::capacity_ledger::CapacitySetup;
use rigorix_engine::capacity_ledger::application::dto::{
    CommitCapacityInput, ExposureQuery, ReleaseCapacityInput, ReserveCapacityInput,
};
use rigorix_engine::capacity_ledger::application::service::CapacityLedgerService;
use rigorix_engine::capacity_ledger::domain::{Exposure, IdempotencyKey, ResourceId};
use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::event_system::application::service::EventBusService;
use rigorix_engine::execution_engine::application::dto::ExecuteGraphInput;
use rigorix_engine::execution_engine::application::factory::{
    ParallelExecutionFactory, ParallelExecutionFactoryConfig,
};
use rigorix_engine::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::execution_engine::domain::ParallelExecutorConfig;
use uuid::Uuid;

const RESOURCE: &str = "payments-budget";

fn write_declaration(
    tag: &str,
    hard_limit: u64,
    amount: u64,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "rigorix-demo-capacity-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".rigorix"))?;
    std::fs::write(
        dir.join(".rigorix/capacity.toml"),
        format!(
            r#"
[[resources]]
id = "{RESOURCE}"
hard_limit = {hard_limit}
window_seconds = 3600

[[grants]]
step = "pay"
tool = "run_command"
resource = "{RESOURCE}"
amount = {amount}
lease_seconds = 60
"#
        ),
    )?;
    Ok(dir)
}

/// committed █ | outstanding ▒ | free ·
fn bar(exposure: &Exposure) -> String {
    let committed = exposure.committed;
    let outstanding = exposure.outstanding;
    let limit = exposure.hard_limit;
    format!(
        "{}{}{}   committed {}, outstanding {}, available {} of {}",
        "█".repeat(committed as usize),
        "▒".repeat(outstanding as usize),
        "·".repeat(exposure.available() as usize),
        committed,
        outstanding,
        exposure.available(),
        limit
    )
}

async fn exposure_of(
    service: &dyn CapacityLedgerService,
) -> Result<Exposure, Box<dyn std::error::Error>> {
    Ok(service
        .exposure(ExposureQuery {
            resource: ResourceId::new(RESOURCE),
        })
        .await?
        .exposure)
}

fn step(n: u32, label: &str) {
    println!("\n{n}. {label}");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Rigorix capacity ledger — ADR-018");
    println!("committed █ | outstanding ▒ | free ·\n");

    // ── Act 1: the contract ────────────────────────────────────────────────
    let hard_limit = 5;
    let repo = write_declaration("act1", hard_limit, 3)?;
    let setup = CapacitySetup::from_env(&repo)
        .await?
        .ok_or("the demo's declaration must arm the seam")?;
    let ledger = setup.service.clone();

    // The declaration lives in a throwaway directory; printing the path would
    // put a machine-specific string in a transcript meant to be reproducible.
    println!("Declaration: .rigorix/capacity.toml (in a temp dir, written by this demo)");
    println!("  resource {RESOURCE}: hard_limit {hard_limit}, window 3600s");
    println!("  step \"pay\": amount 3, lease 60s");
    println!("  armed: {}", setup.is_armed());
    println!("\n── Act 1 — the contract ──");

    step(1, "execution A reserves 3");
    let key_a = IdempotencyKey::new("run-A/pay");
    let a = ledger
        .reserve(ReserveCapacityInput {
            resource: ResourceId::new(RESOURCE),
            amount: 3,
            idempotency_key: key_a.clone(),
            requested_by: Some("run-A:pay".to_string()),
        })
        .await?;
    println!("   {}", bar(&a.exposure));

    step(2, "A retries with the same idempotency key");
    let a_retry = ledger
        .reserve(ReserveCapacityInput {
            resource: ResourceId::new(RESOURCE),
            amount: 3,
            idempotency_key: key_a.clone(),
            requested_by: Some("run-A:pay".to_string()),
        })
        .await?;
    println!("   {}", bar(&a_retry.exposure));
    println!(
        "   same reservation: {} — a retry is not a second hold",
        a_retry.reservation.id == a.reservation.id
    );

    step(3, "execution B reserves 3");
    match ledger
        .reserve(ReserveCapacityInput {
            resource: ResourceId::new(RESOURCE),
            amount: 3,
            idempotency_key: IdempotencyKey::new("run-B/pay"),
            requested_by: Some("run-B:pay".to_string()),
        })
        .await
    {
        Ok(_) => println!("   UNEXPECTED: over the cap"),
        Err(error) => {
            println!("   refused: {error}");
            println!("   {}", bar(&exposure_of(ledger.as_ref()).await?));
            println!("   unchanged — a refusal reserves nothing");
        }
    }

    step(4, "B reserves what is left: 2");
    let b = ledger
        .reserve(ReserveCapacityInput {
            resource: ResourceId::new(RESOURCE),
            amount: 2,
            idempotency_key: IdempotencyKey::new("run-B/pay"),
            requested_by: Some("run-B:pay".to_string()),
        })
        .await?;
    println!("   {}", bar(&b.exposure));

    step(5, "A commits 3 — its work happened");
    let a_commit = ledger
        .commit(CommitCapacityInput {
            reservation_id: a.reservation.id,
        })
        .await?;
    println!("   {}", bar(&a_commit.exposure));
    println!(
        "   committed + outstanding = {} + {} = {} (never one number)",
        a_commit.exposure.committed,
        a_commit.exposure.outstanding,
        a_commit.exposure.committed + a_commit.exposure.outstanding
    );

    step(6, "B releases 2 — its work did not happen");
    let b_release = ledger
        .release(ReleaseCapacityInput {
            reservation_id: b.reservation.id,
            reason: Some("validation failed before dispatch".to_string()),
        })
        .await?;
    println!("   {}", bar(&b_release.exposure));

    // ── Act 2: the seam, through the real executor ─────────────────────────
    println!("\n── Act 2 — the seam, through the real executor ──");
    let limit = 4;
    let repo = write_declaration("act2", limit, 3)?;
    let setup = CapacitySetup::from_env(&repo)
        .await?
        .ok_or("the demo's declaration must arm the seam")?;
    let handle = setup.clone();
    let executor: Box<dyn ParallelExecutionService> = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            executor_config: ParallelExecutorConfig::default(),
            event_bus: Some(Arc::new(EventBusServiceImpl::default()) as Arc<dyn EventBusService>),
            capacity: Some(setup),
            ..Default::default()
        })
        .await?;

    let mut markers = Vec::new();
    let mut graphs = Vec::new();
    for tag in ["one", "two"] {
        let marker = repo.join(format!("tool-ran-{tag}"));
        markers.push(marker.clone());
        let mut graph = TaskGraph::new();
        graph.add_unchecked(TaskNode::new(
            Uuid::new_v4(),
            "pay",
            "run_command",
            vec![],
            format!(
                r#"{{"tool":"run_command","command":"touch {}"}}"#,
                marker.display()
            ),
        ))?;
        graph.seal()?;
        graphs.push(graph);
    }

    println!("  hard_limit {limit}, step amount 3, two executions launched together");
    let (first, second) = tokio::join!(
        executor.execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(graphs.remove(0)),
            config_override: None,
        }),
        executor.execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(graphs.remove(0)),
            config_override: None,
        })
    );
    let results = [first?, second?];
    let refused = results
        .iter()
        .filter(|r| r.result.failed_count == 1)
        .count();
    let ran = results
        .iter()
        .filter(|r| r.result.completed_count == 1)
        .count();
    println!("  dispatched: {ran}, refused before dispatch: {refused}");

    // Which execution wins is a race, so the transcript prints counts, not
    // identities: one marker for the one dispatched step, zero for the refused.
    println!(
        "  tool markers on disk: {} of {} (the refused step never ran its tool)",
        markers.iter().filter(|m| m.exists()).count(),
        markers.len()
    );
    let exposure = handle
        .service
        .exposure(ExposureQuery {
            resource: ResourceId::new(RESOURCE),
        })
        .await?
        .exposure;
    println!("  {}", bar(&exposure));

    let refused_result = results
        .iter()
        .find(|r| r.result.failed_count == 1)
        .ok_or("one execution must be refused")?;
    let failure = refused_result
        .result
        .node_results
        .values()
        .find(|n| !n.success)
        .ok_or("the refused execution must report a failed node")?;
    println!(
        "  refusal: {} (the step's tool was never called)",
        failure.failure_type.as_deref().unwrap_or("?")
    );

    // The invariant the whole module exists for.
    assert_eq!(
        refused, 1,
        "with a cap of {limit} and steps of 3, exactly one execution must be refused"
    );
    let committed = exposure.committed;
    let outstanding = exposure.outstanding;
    assert!(
        committed + outstanding <= limit,
        "committed + outstanding must never exceed the cap: {committed} + {outstanding} > {limit}"
    );
    assert!(
        markers.iter().filter(|m| m.exists()).count() == ran,
        "a refused step's tool must never run: {} marker(s) for {ran} dispatched step(s)",
        markers.iter().filter(|m| m.exists()).count()
    );

    println!("\nok — the cap held, and the refused step's tool never ran.");
    let _ = std::fs::remove_dir_all(&repo);
    Ok(())
}
