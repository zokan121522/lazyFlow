use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::state::EditFocus;
use crate::util::{centered, priority_color, wrap_text, calculate_visual_cursor_pos};

pub fn render_edit_modal(f: &mut Frame, app: &App) {
    let Some(edit) = &app.edit_state else {
        return;
    };

    let area = centered(70, 85, f.area());
    f.render_widget(Clear, area);

    let modal_title: String = if edit.is_new {
        " New Card ".to_string()
    } else {
        format!(" Edit Card — {} ", edit.card_id)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(modal_title)
        .border_style(Style::default().fg(Color::Cyan));
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // 0: title
            Constraint::Length(3), // 1: project
            Constraint::Length(3), // 2: priority
            Constraint::Length(3), // 3: assignee
            Constraint::Min(1),   // 4: description
            Constraint::Length(1), // 5: help
        ])
        .split(inner_area);

    let title_style = if edit.focus == EditFocus::Title {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    f.render_widget(
        Paragraph::new(edit.title.clone())
            .block(Block::default().title("Title").borders(Borders::ALL).border_style(title_style)),
        chunks[0],
    );

    // Project field
    let project_style = if edit.focus == EditFocus::Project {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    f.render_widget(
        Paragraph::new(edit.project.clone())
            .block(
                Block::default()
                    .title("Project")
                    .borders(Borders::ALL)
                    .border_style(project_style),
            ),
        chunks[1],
    );

    // Priority selector
    let prio_style = if edit.focus == EditFocus::Priority {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let prio_spans = vec![
        Span::raw(" "),
        Span::styled(
            edit.priority.label(),
            Style::default()
                .fg(priority_color(edit.priority))
                .add_modifier(Modifier::BOLD),
        ),
        if edit.focus == EditFocus::Priority {
            Span::styled("  ←/→ to change", Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("")
        },
    ];
    f.render_widget(
        Paragraph::new(Line::from(prio_spans))
            .block(
                Block::default()
                    .title("Priority")
                    .borders(Borders::ALL)
                    .border_style(prio_style),
            ),
        chunks[2],
    );

    // Assignee field
    let assignee_style = if edit.focus == EditFocus::Assignee {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    f.render_widget(
        Paragraph::new(edit.assignee.clone())
            .block(
                Block::default()
                    .title("Assignee")
                    .borders(Borders::ALL)
                    .border_style(assignee_style),
            ),
        chunks[3],
    );

    let desc_style = if edit.focus == EditFocus::Description {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let inner_width = chunks[4].width.saturating_sub(2) as usize;
    let wrapped_desc = wrap_text(&edit.description, inner_width);
    let total_lines = wrapped_desc.len();
    let visible_height = (chunks[5].height.saturating_sub(2)) as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll_y = (edit.scroll_y as usize).min(max_scroll) as u16;
    let desc_title = if max_scroll > 0 {
        format!("Description  (↑/↓ line, PgUp/PgDn scroll, {} hidden)", total_lines.saturating_sub(visible_height + scroll_y as usize))
    } else {
        "Description".to_string()
    };

    f.render_widget(
        Paragraph::new(wrapped_desc.join("\n"))
            .scroll((scroll_y, 0))
            .block(
                Block::default()
                    .title(desc_title)
                    .borders(Borders::ALL)
                    .border_style(desc_style),
            ),
        chunks[4],
    );

    let help_text = if edit.focus == EditFocus::Description && max_scroll > 0 {
        "Tab: switch field  \u{2191}/\u{2193}: line  PgUp/PgDn: scroll  Ctrl+K: new line  Enter: save  Esc: cancel"
    } else {
        "Tab: switch field  \u{2190}/\u{2192}: priority  Ctrl+K: new line  Enter: save  Esc: cancel"
    };
    f.render_widget(
        Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Center),
        chunks[5],
    );

    // Position cursor
    match edit.focus {
        EditFocus::Title => {
            f.set_cursor_position((chunks[0].x + 1 + edit.cursor_pos as u16, chunks[0].y + 1));
        }
        EditFocus::Project => {
            f.set_cursor_position((chunks[1].x + 1 + edit.cursor_pos as u16, chunks[1].y + 1));
        }
        EditFocus::Assignee => {
            f.set_cursor_position((chunks[3].x + 1 + edit.cursor_pos as u16, chunks[3].y + 1));
        }
        EditFocus::Description => {
            let (x, y) =
                calculate_visual_cursor_pos(&edit.description, edit.cursor_pos, inner_width);
            let display_y = y.saturating_sub(scroll_y as usize);
            f.set_cursor_position((
                chunks[4].x + 1 + x as u16,
                chunks[4].y + 1 + display_y as u16,
            ));
        }
        EditFocus::Priority => {
            // No text cursor for priority field
        }
    }
}
