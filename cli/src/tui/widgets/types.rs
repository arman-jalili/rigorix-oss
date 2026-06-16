use ratatui::layout::Rect;

/// Terminal size mode for adaptive layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Compact,
    Standard,
    Full,
}

impl LayoutMode {
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

/// Context passed to widgets during rendering.
#[derive(Debug, Clone)]
pub struct WidgetContext {
    pub area: Rect,
    pub layout_mode: LayoutMode,
    pub color_enabled: bool,
    pub detailed: bool,
}
