//! Widget implementations — reusable ratatui UI components.
//!
//! @canonical .pi/architecture/modules/tui.md#widgets
//! Implements: Contract Freeze — Renderer component
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Widgets are reusable UI components drawn by views. They encapsulate
//! a rendering pattern (DAG tree, progress bar, modal, etc.) and are
//! composed by views to build the full UI.

use ratatui::layout::Rect;

use crate::tui::view_model::TuiViewModel;

/// Context passed to widgets and views during rendering.
#[derive(Debug, Clone)]
pub struct WidgetContext {
    /// The area available for the widget to draw in.
    pub area: Rect,
    /// Current terminal size mode.
    pub layout_mode: LayoutMode,
    /// Whether to use color/highlighting.
    pub color_enabled: bool,
    /// Whether to show detailed information.
    pub detailed: bool,
}

/// Terminal size mode for adaptive layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// 80×24 minimum — single column.
    Compact,
    /// 120×30 — two columns.
    Standard,
    /// 160×40 — three columns.
    Full,
}

impl LayoutMode {
    /// Determine layout mode from terminal dimensions.
    pub fn from_size(width: u16, height: u16) -> Self {
        if width >= 160 && height >= 40 {
            LayoutMode::Full
        } else if width >= 120 && height >= 30 {
            LayoutMode::Standard
        } else {
            LayoutMode::Compact
        }
    }
}

/// Trait for a renderable widget.
pub trait Widget: Send + Sync {
    /// Render the widget into the given frame area.
    fn draw(&self, frame: &mut ratatui::Frame<'_>, vm: &TuiViewModel, ctx: &WidgetContext);

    /// The minimum size required for this widget to render properly.
    fn min_size(&self) -> (u16, u16);

    /// The name of this widget.
    fn name(&self) -> &'static str;
}
