use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{Action, App};
use crate::gdrive_tui;
use crate::help::{help_text, render_help};
use crate::edit::render_edit_modal;
use crate::state::SearchState;
use crate::util::{centered, priority_color, project_color, selected_card_id};

pub fn action_from_key(code: KeyCode, modifiers: KeyModifiers, filter_focus: bool) -> Option<Action> {
    // Ctrl+G toggles GDrive popup — works from any state
    if code == KeyCode::Char('g') && modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Action::GDrive);
    }
    // Ctrl+T fallback for GDrive (some terminals intercept Ctrl+G)
    if code == KeyCode::Char('t') && modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Action::GDrive);
    }

    if filter_focus {
        // When filter bar has focus, ←/→/Enter/Esc/Tab are filter-aware
        return Some(match code {
            KeyCode::Left => Action::FilterLeft,
            KeyCode::Right => Action::FilterRight,
            KeyCode::Enter => Action::FilterConfirm,
            KeyCode::Esc => Action::CloseOrQuit,
            KeyCode::Tab => Action::TabFocus,
            // 'c' opens the project color picker (only meaningful on a project tab)
            KeyCode::Char('c') => Action::ColorPickerToggle,
            _ => return None,
        });
    }

    // Plain keys — allow SHIFT (needed for uppercase H/L), exclude Ctrl/Alt
    if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT {
        return Some(match code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Esc => Action::CloseOrQuit,

            KeyCode::Char('h') | KeyCode::Left => Action::FocusLeft,
            KeyCode::Char('l') | KeyCode::Right => Action::FocusRight,

            KeyCode::Char('j') | KeyCode::Down => Action::SelectDown,
            KeyCode::Char('k') | KeyCode::Up => Action::SelectUp,

            KeyCode::Char('H') => Action::MoveLeft,
            KeyCode::Char('L') => Action::MoveRight,

            KeyCode::Enter => Action::ToggleDetail,
            KeyCode::Char('r') => Action::Refresh,
            KeyCode::Char('d') => Action::Delete,
            KeyCode::Char('a') | KeyCode::Char('n') => Action::Add,
            KeyCode::Char('e') => Action::Edit,
            KeyCode::Char('s') => Action::ToggleSort,
            KeyCode::Char('/') => Action::Search,
            KeyCode::Char('p') | KeyCode::Tab => Action::TabFocus,

            _ => return None,
        });
    }

    None
}

pub fn render(f: &mut Frame, app: &App, render_area: Option<Rect>) {
    let area = render_area.unwrap_or_else(|| f.area());

    // Build constraints: [banner?] [filter:3] [main:Min(1)] [help:2]
    // Filter needs 3 lines: top border + content + bottom border (Tabs with Borders::ALL)
    let mut constraints = vec![
        Constraint::Length(3), // filter bar (visual tabs with borders)
        Constraint::Min(1),    // main area (board columns)
        Constraint::Length(2), // help bar
    ];
    if app.banner.is_some() {
        constraints.insert(0, Constraint::Length(1)); // banner
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Banner
    let mut idx = 0;
    if app.banner.is_some() {
        if let Some(text) = app.banner.as_deref() {
            f.render_widget(
                Paragraph::new(Span::styled(text, Style::default().fg(Color::Yellow))),
                chunks[idx],
            );
        }
        idx += 1;
    }

    // Filter bar
    render_filter_bar(f, app, chunks[idx]);
    idx += 1;

    // Main (board)
    let main = chunks[idx];
    idx += 1;

    // Help bar
    let help = chunks[idx];

    if app.board.columns.is_empty() {
        f.render_widget(
            Paragraph::new("No columns found. Check board.txt.")
                .block(Block::default().borders(Borders::ALL)),
            main,
        );
    } else {
        let rects = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Ratio(1, app.board.columns.len() as u32);
                app.board.columns.len()
            ])
            .split(main);

        for (i, r) in rects.iter().enumerate() {
            draw_col(f, app, i, *r);
        }
    }

    if let Some(search) = &app.search_state {
        let matches = app.search_matches();
        let match_info = if search.query.is_empty() {
            String::new()
        } else {
            format!("  ({} matches)", matches.len())
        };
        let search_line = Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(&search.query),
            Span::styled(&match_info, Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(
            Paragraph::new(search_line).block(Block::default().borders(Borders::TOP)),
            help,
        );
        f.set_cursor_position((
            help.x + 1 + search.cursor_pos as u16,
            help.y + 1,
        ));
    } else {
        f.render_widget(
            Paragraph::new(help_text(app)).block(Block::default().borders(Borders::TOP)),
            help,
        );
    }

    if app.detail_open {
        let Some(col) = app.board.columns.get(app.col) else {
            return;
        };
        let Some(card) = col.cards.get(app.row) else {
            return;
        };

        f.render_widget(Clear, main);

        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            &card.id,
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::raw("Priority: "),
            Span::styled(card.priority.label(), Style::default().fg(priority_color(card.priority))),
        ]));
        if !card.project.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("Project: "),
                Span::styled(&card.project, Style::default().fg(project_color(&card.project, &app.project_colors)).add_modifier(Modifier::BOLD)),
            ]));
        }
        if !card.assignee.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("Assignee: "),
                Span::styled(&card.assignee, Style::default().add_modifier(Modifier::BOLD)),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(card.title.clone()));
        lines.push(Line::from(""));

        if card.description.trim().is_empty() {
            lines.push(Line::from(Span::styled(
                "No description",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for l in card.description.lines() {
                lines.push(Line::from(l.to_string()));
            }
        }

        let total_lines = lines.len();
        let visible_height = (main.height.saturating_sub(2)) as usize;
        let max_scroll = total_lines.saturating_sub(visible_height);
        let scroll_y = (app.detail_scroll as usize).min(max_scroll) as u16;
        let detail_title = if max_scroll > 0 {
            format!("Detail  (↑/↓ scroll, {} hidden)", total_lines.saturating_sub(visible_height + scroll_y as usize))
        } else {
            "Detail".to_string()
        };

        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false })
                .scroll((scroll_y, 0))
                .block(
                    Block::default()
                        .title(detail_title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)),
                ),
            main,
        );
    }

    if app.confirm_delete {
        let confirm_area = centered(40, 20, area);
        f.render_widget(Clear, confirm_area);

        let card_id = selected_card_id(app).unwrap_or_else(|| "Unknown".to_string());
        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Delete card "),
                Span::styled(&card_id, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("?"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("es / "),
                Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("o"),
            ]),
        ];

        f.render_widget(
            Paragraph::new(text)
                .alignment(ratatui::layout::Alignment::Center)
                .block(
                    Block::default()
                        .title("Confirm Delete")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red)),
                ),
            confirm_area,
        );
    }

    // Edit modal
    render_edit_modal(f, app);

    // GDrive sync popup
    if app.gdrive_popup_open {
        gdrive_tui::render_popup(
            f,
            &app.gdrive_status,
            app.gdrive_last_sync.as_deref(),
            app.gdrive_has_client_id,
            app.gdrive_client_id_input.as_deref(),
        );
    }

    // Project color picker popup
    if app.color_picker_open {
        render_color_picker(f, app, area);
    }

    // Help overlay — drawn last so it appears on top of everything
    render_help(f, app, render_area);
}

/// Render the persistent project filter bar as visual tabs at the top.
fn render_filter_bar(f: &mut Frame, app: &App, rect: Rect) {
    use ratatui::widgets::Tabs;

    let projects = app.board.project_recency();
    // Build tab titles: "All" + projects (each project colored by its override/hash)
    let mut tab_titles: Vec<Line> = Vec::with_capacity(1 + projects.len());
    tab_titles.push(Line::from(" All "));
    for proj in &projects {
        let color = project_color(proj, &app.project_colors);
        let title = if app.filter_focus && selected_project(app).as_deref() == Some(proj.as_str()) {
            format!(" [{}] ", proj)
        } else {
            format!(" {} ", proj)
        };
        tab_titles.push(Line::from(Span::styled(title, Style::default().fg(color))));
    }

    // Selected index follows filter_cursor (0 = All, 1+ = project position)
    let selected = app.filter_cursor;

    // Border changes color when filter bar has focus
    let border_style = if app.filter_focus {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let gdrive_label = gdrive_tui::status_indicator(&app.gdrive_status);
    let hint = if app.filter_focus {
        if app.filter_cursor > 0 {
            format!("{gdrive_label} c: color  Tab: unfocus ")
        } else {
            format!("{gdrive_label} Tab to unfocus ")
        }
    } else {
        format!("{gdrive_label} Tab / p to filter ")
    };

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(Span::styled(hint, Style::default().fg(Color::DarkGray))),
        )
        .select(selected)
        .divider(Span::raw(" │ "))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().fg(Color::DarkGray));

    f.render_widget(tabs, rect);
}

/// Project currently under the filter cursor (None when "All" is selected).
fn selected_project(app: &App) -> Option<String> {
    if app.filter_cursor == 0 {
        return None;
    }
    app.board
        .project_recency()
        .get(app.filter_cursor - 1)
        .cloned()
}

/// Render the project color picker popup over the filter bar.
fn render_color_picker(f: &mut Frame, app: &App, area: Rect) {
    use ratatui::widgets::Paragraph;

    let picker_area = centered(46, 30, area);
    f.render_widget(Clear, picker_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("Project: "),
        Span::styled(
            &app.color_picker_project,
            Style::default().fg(project_color(&app.color_picker_project, &app.project_colors))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // Two rows of palette swatches: name colored with its own color.
    let palette = crate::util::PROJECT_COLOR_PALETTE;
    let mid = palette.len() / 2;
    for row in 0..2 {
        let mut spans = Vec::new();
        for (i, (name, color)) in palette.iter().enumerate().skip(row * mid).take(mid) {
            let selected = i == app.color_picker_cursor;
            let swatch = if selected {
                format!("[{name}]")
            } else {
                format!(" {name} ")
            };
            let mut style = Style::default().fg(*color);
            if selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            spans.push(Span::styled(swatch, style));
            spans.push(Span::raw("  "));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("←/→", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" navigate   "),
        Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" assign   "),
        Span::styled("Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" cancel"),
    ]));

    f.render_widget(
        Paragraph::new(lines)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .title("Project Color")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
        picker_area,
    );
}

pub fn draw_col(f: &mut Frame, app: &App, idx: usize, rect: Rect) {
    let col = &app.board.columns[idx];
    let focused = idx == app.col;

    let border = if focused { Color::Cyan } else { Color::Gray };

    let searching = app
        .search_state
        .as_ref()
        .map_or(false, |s| !s.query.is_empty());

    let items: Vec<ListItem> = col
        .cards
        .iter()
        .map(|c| {
            let dimmed = searching
                && app
                    .search_state
                    .as_ref()
                    .map_or(false, |s| !SearchState::matches_card(c, &s.query));
            let prio_style = if dimmed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(priority_color(c.priority))
            };
            let title_style = if dimmed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(priority_color(c.priority))
            };
            let proj_style = if dimmed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(project_color(&c.project, &app.project_colors))
            };
            let mut spans = vec![
                Span::styled(format!("[{}] ", c.priority.short_label()), prio_style),
            ];
            if !c.project.is_empty() {
                spans.push(Span::styled(format!("{} ", c.project), proj_style));
            }
            spans.push(Span::styled(c.title.clone(), title_style));
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!("{} ({})", col.title, col.cards.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = ListState::default();
    if focused && !col.cards.is_empty() {
        state.select(Some(app.row.min(col.cards.len() - 1)));
    }

    f.render_stateful_widget(list, rect, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn action(code: KeyCode, mods: KeyModifiers) -> Option<Action> {
        action_from_key(code, mods, false)
    }

    #[test]
    fn uppercase_h_with_shift_moves_card_left() {
        assert_eq!(action(KeyCode::Char('H'), KeyModifiers::SHIFT), Some(Action::MoveLeft));
    }

    #[test]
    fn uppercase_l_with_shift_moves_card_right() {
        assert_eq!(action(KeyCode::Char('L'), KeyModifiers::SHIFT), Some(Action::MoveRight));
    }

    #[test]
    fn lowercase_h_focuses_left() {
        assert_eq!(action(KeyCode::Char('h'), KeyModifiers::empty()), Some(Action::FocusLeft));
    }

    #[test]
    fn lowercase_l_focuses_right() {
        assert_eq!(action(KeyCode::Char('l'), KeyModifiers::empty()), Some(Action::FocusRight));
    }

    #[test]
    fn ctrl_keys_are_not_consumed_as_plain_keys() {
        // Ctrl+G/Ctrl+T handled before the plain-key block; ensure they map to GDrive
        assert_eq!(action(KeyCode::Char('g'), KeyModifiers::CONTROL), Some(Action::GDrive));
        assert_eq!(action(KeyCode::Char('t'), KeyModifiers::CONTROL), Some(Action::GDrive));
    }
}
