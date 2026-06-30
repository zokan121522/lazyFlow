use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::util::centered;

pub fn help_text(app: &App) -> String {
    let filter_info = if app.project_filter.is_empty() {
        String::new()
    } else {
        format!(" [{}]", app.project_filter.join(","))
    };
    format!(
        "h/l ←/→ focus  j/k ↑/↓ select  H/L move  a/n new  e edit  d del  Enter detail  ? help  r refresh  s sort({})  / search  Tab/p filter{}  Esc/q quit",
        app.sort_order.label(),
        filter_info,
    )
}

pub fn render_help(f: &mut Frame, app: &App, render_area: Option<Rect>) {
    if !app.show_help {
        return;
    }
    let area = centered(65, 70, render_area.unwrap_or_else(|| f.area()));
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "── Navigation ──",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from("  h / ←       Focus left column"),
        Line::from("  l / →       Focus right column"),
        Line::from("  j / ↓       Select next card"),
        Line::from("  k / ↑       Select previous card"),
        Line::from(""),
        Line::from(Span::styled(
            "── Cards ──",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from("  a / n       New card"),
        Line::from("  e           Edit card"),
        Line::from("  d           Delete card"),
        Line::from("  H           Move card left"),
        Line::from("  L           Move card right"),
        Line::from("  Enter       View / close detail"),
        Line::from(""),
        Line::from(Span::styled(
            "── Edit Mode ──",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab         Switch field"),
        Line::from("  ← / →       Change priority"),
        Line::from("  Ctrl+K      Insert new line"),
        Line::from("  Enter       Save"),
        Line::from("  Esc         Cancel"),
        Line::from(""),
        Line::from(Span::styled(
            "── Project Filter ──",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab / p     Focus filter bar"),
        Line::from("  ← / →       Select project"),
        Line::from("  Enter       Confirm filter"),
        Line::from("  Esc         Unfocus filter bar"),
        Line::from(""),
        Line::from(Span::styled(
            "── General ──",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from("  r           Refresh board"),
        Line::from("  s           Toggle sort order"),
        Line::from("  /           Search cards"),
        Line::from("  ?           Show this help"),
        Line::from("  Esc         Close / go back"),
        Line::from("  q           Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  Any key to close",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    f.render_widget(
        Paragraph::new(lines)
            .alignment(ratatui::layout::Alignment::Left)
            .block(
                Block::default()
                    .title(" Keyboard Shortcuts ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
        area,
    );
}
