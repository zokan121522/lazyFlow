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

    let area = centered(70, 65, f.area());
    f.render_widget(Clear, area);

    let modal_title = if edit.is_new { "New Card" } else { "Edit Card" };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(modal_title)
        .border_style(Style::default().fg(Color::Cyan));
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // 0: card id
            Constraint::Length(3), // 1: title
            Constraint::Length(3), // 2: project
            Constraint::Length(3), // 3: priority
            Constraint::Length(3), // 4: assignee
            Constraint::Min(1),   // 5: description
            Constraint::Length(1), // 6: help
        ])
        .split(inner_area);

    let header_line = if edit.is_new {
        Line::from(Span::styled(
            "New card",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(vec![
            Span::raw("Editing "),
            Span::styled(
                &edit.card_id,
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ])
    };
    f.render_widget(
        Paragraph::new(header_line).alignment(ratatui::layout::Alignment::Center),
        chunks[0],
    );

    let title_style = if edit.focus == EditFocus::Title {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    f.render_widget(
        Paragraph::new(edit.title.clone())
            .block(Block::default().title("Title").borders(Borders::ALL).border_style(title_style)),
        chunks[1],
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
        chunks[2],
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
        chunks[3],
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
        chunks[4],
    );

    let desc_style = if edit.focus == EditFocus::Description {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let inner_width = chunks[5].width.saturating_sub(2) as usize;
    let wrapped_desc = wrap_text(&edit.description, inner_width);
    let total_lines = wrapped_desc.len();
    let visible_height = (chunks[5].height.saturating_sub(2)) as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll_y = (edit.scroll_y as usize).min(max_scroll) as u16;
    let desc_title = if max_scroll > 0 {
        format!("Description  (↑/↓ scroll, {} hidden)", total_lines.saturating_sub(visible_height + scroll_y as usize))
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
        chunks[5],
    );

    let help_text = if edit.focus == EditFocus::Description && max_scroll > 0 {
        "Tab: switch field  \u{2191}/\u{2193}: scroll  Ctrl+K: new line  Enter: save  Esc: cancel"
    } else {
        "Tab: switch field  \u{2190}/\u{2192}: priority  Ctrl+K: new line  Enter: save  Esc: cancel"
    };
    f.render_widget(
        Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Center),
        chunks[6],
    );

    // Position cursor
    match edit.focus {
        EditFocus::Title => {
            f.set_cursor_position((chunks[1].x + 1 + edit.cursor_pos as u16, chunks[1].y + 1));
        }
        EditFocus::Project => {
            f.set_cursor_position((chunks[2].x + 1 + edit.cursor_pos as u16, chunks[2].y + 1));
        }
        EditFocus::Assignee => {
            f.set_cursor_position((chunks[4].x + 1 + edit.cursor_pos as u16, chunks[4].y + 1));
        }
        EditFocus::Description => {
            let (x, y) =
                calculate_visual_cursor_pos(&edit.description, edit.cursor_pos, inner_width);
            let display_y = y.saturating_sub(scroll_y as usize);
            f.set_cursor_position((
                chunks[5].x + 1 + x as u16,
                chunks[5].y + 1 + display_y as u16,
            ));
        }
        EditFocus::Priority => {
            // No text cursor for priority field
        }
    }
}
