use crate::tui::view_model::TuiViewModel;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Gauge},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let total = vm.metrics.nodes_total.max(1);
    let pct = (vm.metrics.nodes_completed as f64 / total as f64) * 100.0;
    frame.render_widget(
        Gauge::default()
            .block(Block::default().title(" Progress ").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Green))
            .percent(pct as u16)
            .label(format!(
                "{:.0}% ({}/{})",
                pct, vm.metrics.nodes_completed, total
            )),
        area,
    );
}
