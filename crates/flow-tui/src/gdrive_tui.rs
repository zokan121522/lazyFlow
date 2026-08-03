// ── Google Drive TUI popup ────────────────────────────────────────────
//
// Renders the GDrive connection/sync popup and a compact status indicator
// for the filter bar. The actual GDriveClient lives in main.rs — this
// module only handles the visual layer.
//
// ──────────────────────────────────────────────────────────────────────

use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use flow_core::gdrive::GDriveStatus;

use crate::util::centered;

/// Render the GDrive settings popup in the center of the screen.
///
/// Call this from `ui::render()` when `app.gdrive_popup_open` is true.
/// `client_id_input` is the current editing buffer (if any).
pub fn render_popup(
    f: &mut Frame,
    status: &GDriveStatus,
    last_sync: Option<&str>,
    has_client_id: bool,
    client_id_input: Option<&str>,
) {
    let area = centered(60, 44, f.area());
    f.render_widget(Clear, area);

    let mut lines: Vec<Line> = Vec::new();

    // ── Status ────────────────────────────────────────────────────────
    let (status_label, status_color) = match status {
        GDriveStatus::Disconnected => ("Disconnected", Color::DarkGray),
        GDriveStatus::Authorizing { .. } => ("Authorizing...", Color::Yellow),
        GDriveStatus::Connected { .. } => ("Connected", Color::Green),
        GDriveStatus::Error(_) => ("Error", Color::Red),
    };

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("Status: "),
        Span::styled(
            status_label,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    match status {
        GDriveStatus::Connected { account } => {
            if !account.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("Account: "),
                    Span::styled(account.clone(), Style::default().fg(Color::Cyan)),
                ]));
            }
            if let Some(sync) = last_sync {
                lines.push(Line::from(vec![
                    Span::raw("Last sync: "),
                    Span::styled(sync.to_string(), Style::default().fg(Color::DarkGray)),
                ]));
            }
        }
        GDriveStatus::Authorizing {
            verification_url,
            user_code,
        } => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "1. Open this URL:",
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                verification_url.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("2. Enter code: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    user_code.clone(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        GDriveStatus::Error(msg) => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                msg.clone(),
                Style::default().fg(Color::Red),
            )));
        }
        GDriveStatus::Disconnected => {
            if !has_client_id && client_id_input.is_none() {
                // Show step-by-step instructions to get a Client ID
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "No Google OAuth Client ID configured.",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "1. Go to console.cloud.google.com",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(Span::styled(
                    "2. Create a project or select existing one",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(Span::styled(
                    "3. Enable 'Google Drive API'",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(Span::styled(
                    "4. Credentials → Create → OAuth Client ID",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(Span::styled(
                    "5. Type: Desktop application",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(Span::styled(
                    "6. Copy the Client ID and paste it here",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Press E to paste your Client ID",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
            }
            if let Some(buf) = client_id_input {
                lines.push(Line::from(Span::styled(
                    "Client ID:",
                    Style::default().fg(Color::Yellow),
                )));
                let display = if buf.is_empty() {
                    "<type/paste your Client ID here...>"
                } else {
                    buf
                };
                lines.push(Line::from(Span::styled(
                    display,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
            }
        }
    }

    // ── Hint when disconnected but has client_id ──────────────────────
    lines.push(Line::from(""));
    if let GDriveStatus::Disconnected = status {
        if has_client_id && client_id_input.is_none() {
            lines.push(Line::from(Span::styled(
                "Press C to start OAuth authorization.",
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // ── Action hints ──────────────────────────────────────────────────
    lines.push(Line::from(""));

    // Determine which hint to show based on mode
    let hint = if client_id_input.is_some() {
        " [Enter] Save  [Esc] Cancel "
    } else {
        match status {
            GDriveStatus::Disconnected if has_client_id => {
                " [C] Connect  [Esc] Close "
            }
            GDriveStatus::Disconnected => {
                " [E] Edit Client ID  [Esc] Close "
            }
            GDriveStatus::Authorizing { .. } => " [Esc] Cancel ",
            GDriveStatus::Connected { .. } => {
                " [D] Disconnect  [S] Sync now  [Esc] Close "
            }
            GDriveStatus::Error(_) => " [C] Retry  [Esc] Close ",
        }
    };
    lines.push(Line::from(Span::styled(
        hint,
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(
        Paragraph::new(lines)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .title(" Google Drive Sync ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
        area,
    );
}

/// Short status label for the filter bar (e.g. " G:ON ", " G:OFF ").
pub fn status_indicator(status: &GDriveStatus) -> &'static str {
    match status {
        GDriveStatus::Disconnected => " G:OFF ",
        GDriveStatus::Authorizing { .. } => " G:\u{2026} ",
        GDriveStatus::Connected { .. } => " G:ON ",
        GDriveStatus::Error(_) => " G:ERR ",
    }
}
