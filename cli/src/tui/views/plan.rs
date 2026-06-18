//! Plan preview view — shows intent, template, metrics, and the DAG tree.
//!
//! When nodes are populated (from a prior GenerateTemplate or PlanOnly action),
//! this view renders the full DAG tree so the user can review the plan before
//! running it.

use crate::tui::view_model::{ExecutionPhase, NodeStatus, NodeViewModel, TuiViewModel};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let has_nodes = !vm.nodes.is_empty();
    let is_planning = vm.phase == ExecutionPhase::Planning;

    let chunks = if has_nodes {
        Layout::vertical([
            Constraint::Length(5), // summary
            Constraint::Min(4),    // DAG tree
            Constraint::Length(2), // action hints
        ])
        .split(area)
    } else {
        Layout::vertical([
            Constraint::Min(1),    // status / hints (fill)
            Constraint::Length(2), // action hints
        ])
        .split(area)
    };

    // ── Summary block ────────────────────────────────────────────────
    if has_nodes {
        let intent = vm.intent.as_deref().unwrap_or("(no intent)");
        let template = vm.template_id.as_deref().unwrap_or("(pending)");

        // Get confidence from the first node's risk_level (best proxy available)
        let confidence_pct = vm
            .nodes
            .values()
            .find_map(|n| n.risk_level.as_ref())
            .and_then(|r| r.parse::<f64>().ok())
            .map(|c| format!(" ({:.0}%)", c * 100.0))
            .unwrap_or_default();

        // Build parameters string from node names that look like params
        let params_line = if vm.template_id.is_some() {
            // Show first node's dep info as a dependency summary
            let roots = vm
                .nodes
                .values()
                .filter(|n| n.dependencies.is_empty())
                .count();
            let leaves = vm
                .nodes
                .values()
                .filter(|n| n.dependents.is_empty() && !n.dependencies.is_empty())
                .count();
            format!("{} roots, {} leaves", roots, leaves)
        } else {
            String::new()
        };

        let summary_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" Intent: ", Style::default().bold()),
                Span::raw(intent),
            ]),
            Line::from(vec![
                Span::styled(" Template: ", Style::default().bold()),
                Span::raw(format!("{}{}", template, confidence_pct)),
                Span::raw("    "),
                Span::styled("Nodes: ", Style::default().bold()),
                Span::raw(vm.nodes.len().to_string()),
                Span::raw("    "),
                Span::raw(params_line),
            ]),
            Line::from(vec![
                Span::styled(" LLM: ", Style::default().bold()),
                Span::raw(format!("{} calls", vm.metrics.llm_calls)),
                Span::raw("    "),
                Span::styled("Tokens: ", Style::default().bold()),
                Span::raw(vm.metrics.tokens.to_string()),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(summary_lines)
                .block(
                    Block::default()
                        .title(" Plan Preview ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: false }),
            chunks[0],
        );

        // ── DAG tree ──────────────────────────────────────────────
        render_dag_nodes(frame, chunks[1], vm);
    } else if let Some(err) = &vm.error {
        // Error state
        let lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("  Error: {}", err),
                Style::default().fg(Color::Red),
            )]),
            Line::from(""),
            Line::from(vec![Span::raw(
                "  Press [Esc] to return to command bar, or [Tab] for other views.",
            )]),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title(" Plan Preview ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red)),
                )
                .wrap(Wrap { trim: false }),
            chunks[0],
        );
    } else if is_planning {
        // Generating
        let intent = vm.intent.as_deref().unwrap_or("(no intent)");
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" Intent: ", Style::default().bold()),
                Span::raw(intent),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Generating plan via LLM...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::raw(
                "  The DAG will appear here once generation completes.",
            )]),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title(" Plan Preview ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: false }),
            chunks[0],
        );
    } else {
        // No nodes, not planning — prompt user
        let intent = vm.intent.as_deref().unwrap_or("(no intent)");
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" Intent: ", Style::default().bold()),
                Span::raw(intent),
            ]),
            Line::from(""),
            Line::from(vec![Span::raw("  No plan generated yet.")]),
            Line::from(vec![Span::styled(
                "  Press [g] to generate a plan from this intent.",
                Style::default().fg(Color::Blue),
            )]),
            Line::from(""),
            Line::from(vec![Span::raw(
                "  The plan DAG will appear here after generation.",
            )]),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title(" Plan Preview ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: false }),
            chunks[0],
        );
    }

    // ── Action hints (always at bottom) ──────────────────────────────
    let hint_chunk = if has_nodes { chunks[2] } else { chunks[1] };
    let hints = if has_nodes {
        vec![
            Span::styled(" [r] Run", Style::default().fg(Color::Green)),
            Span::raw("    "),
            Span::styled("[g] Regenerate", Style::default().fg(Color::Blue)),
            Span::raw("    "),
            Span::styled("[d] Node Detail", Style::default().fg(Color::Cyan)),
            Span::raw("    "),
            Span::styled("[Esc] Cancel", Style::default().fg(Color::Red)),
        ]
    } else if is_planning {
        vec![Span::styled(
            " [Esc] Cancel / Back",
            Style::default().fg(Color::Red),
        )]
    } else {
        vec![
            Span::styled(" [g] Generate", Style::default().fg(Color::Blue)),
            Span::raw("    "),
            Span::styled("[Esc] Cancel", Style::default().fg(Color::Red)),
        ]
    };
    frame.render_widget(
        Paragraph::new(Line::from(hints)).wrap(Wrap { trim: false }),
        hint_chunk,
    );
}

/// Render DAG nodes as a list — same visual style as the dag_tree widget
/// but with a title matching the plan preview context.
fn render_dag_nodes(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let nodes: Vec<&NodeViewModel> = vm.nodes.values().collect();
    let items: Vec<ListItem> = nodes
        .iter()
        .map(|n| {
            let s = match n.status {
                NodeStatus::Completed => Style::default().fg(Color::Green),
                NodeStatus::InProgress => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                NodeStatus::Failed => Style::default().fg(Color::Red),
                NodeStatus::Retrying => Style::default().fg(Color::Yellow),
                NodeStatus::Pending | NodeStatus::Skipped => Style::default().fg(Color::DarkGray),
            };
            let icon = match n.status {
                NodeStatus::Completed => "\u{2713}",
                NodeStatus::InProgress => "\u{25b6}",
                NodeStatus::Failed => "\u{2717}",
                NodeStatus::Retrying => "\u{21bb}",
                NodeStatus::Pending => "\u{00b7}",
                NodeStatus::Skipped => "\u{2013}",
            };
            let timing = n
                .timing_ms
                .map(|ms| format!(" {}ms", ms))
                .unwrap_or_default();

            // Show dependency count as extra context in plan preview
            let dep_hint = if !n.dependencies.is_empty() {
                format!(" (depends on: {})", n.dependencies.join(", "))
            } else {
                String::new()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", icon), s),
                Span::raw(format!(" {} ", n.name)),
                Span::styled(n.tool_name.clone(), Style::default().fg(Color::Blue)),
                Span::raw(timing),
                Span::styled(dep_hint, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let title = format!(" Plan DAG ({} nodes) ", nodes.len());
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        area,
    );
}
