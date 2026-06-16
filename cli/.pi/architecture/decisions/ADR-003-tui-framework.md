# ADR-003: TUI Framework Selection

**Status:** Accepted
**Date:** 2026-06-16

## Context

The CLI needs a real-time terminal UI that subscribes to the EventBus and renders node status, budget bars, and execution progress without blocking the async execution pipeline.

## Decision

**Use ratatui** (Rust TUI library, successor to tui-rs).

Key selection criteria:
1. **Async-native** — ratatui works with tokio; TUI rendering runs in a separate task that reads from an `mpsc::Receiver` fed by the EventBus subscriber
2. **Terminal resize** — built-in `Frame` handling for SIGWINCH
3. **Widget ecosystem** — `ratatui-widgets`, `tui-logger`, `tui-tree` for the DAG graph display
4. **Community** — most widely used Rust TUI library

## Implementation Pattern

```rust
// TUI runs in its own tokio task
let (tx, mut rx) = tokio::sync::mpsc::channel(256);

// Subscriber task: EventBus → channel
tokio::spawn(async move {
    let mut sub = event_bus.subscribe();
    while let Ok(event) = sub.recv().await {
        tx.send(event).await.ok();
    }
});

// Render task: channel → ratatui Terminal
tokio::spawn(async move {
    let mut terminal = ratatui::init();
    while let Some(event) = rx.recv().await {
        terminal.draw(|frame| {
            // Render node graph, budget bars, status
        }).ok();
    }
});
```

## Alternatives

| Framework | Reason Rejected |
|-----------|----------------|
| cursive | Callback-based, awkward with tokio async |
| iced | Immediate mode, heavier dependency, not terminal-first |
| termion | Low-level, no widget system |

*Affects: CLI Boundary, Event System*
