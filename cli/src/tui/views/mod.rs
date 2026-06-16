//! View implementations — full-screen renderable views.
//!
//! @canonical .pi/architecture/modules/tui.md#views
//! Implements: Contract Freeze — Views component
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Each view is a full-screen renderable that draws to a ratatui `Frame`.
//! Views are selected by `ActiveView` and drawn by the render loop.

/// Dashboard — DAG tree + selected node details + metrics + progress.
pub mod dashboard;

/// Plan preview — plan with confirm actions.
pub mod plan;

/// History — past execution browser.
pub mod history;

/// Events — filterable event timeline.
pub mod events;

/// Nodes — full node list table with detail panel.
pub mod nodes;

/// Settings — configuration panel.
pub mod settings;

/// Templates — template list/show.
pub mod templates;

/// Clarification — LLM clarification requests.
pub mod clarification;

/// Diff — plan comparison side-by-side.
pub mod diff;

use crate::tui::view_model::{ActiveView, TuiViewModel};
use crate::tui::widgets::WidgetContext;

/// Trait for a full-screen renderable view.
pub trait View: Send + Sync {
    /// Draw the view into the given ratatui frame.
    fn draw(&self, frame: &mut ratatui::Frame<'_>, vm: &TuiViewModel, ctx: &WidgetContext);

    /// Handle a key event for this view.
    fn handle_key(&mut self, key: ratatui::crossterm::event::KeyEvent) -> bool;

    /// Get the name of this view.
    fn name(&self) -> &'static str;

    /// Get the `ActiveView` variant this view corresponds to.
    fn view_type(&self) -> ActiveView;
}
