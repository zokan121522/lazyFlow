use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use crossterm::event::KeyCode;

use crate::app::{Action, App};
use crate::help::{help_text, render_help};
use crate::edit::render_edit_modal;
use crate::state::SearchState;
use crate::util::{centered, priority_color, selected_card_id};

pub fn action_from_key(code: KeyCode) -> Option<Action> {
    Some(match code {
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
        KeyCode::Char('p') => Action::ProjectFilter,

        _ => return None,
    })
}

pub fn render(f: &mut Frame, app: &App, render_area: Option<Rect>) {
    let chunks = if app.banner.is_some() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(2),
            ])
            .split(render_area.unwrap_or_else(|| f.area()))
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(render_area.unwrap_or_else(|| f.area()))
    };

    let (banner_area, main, help) = if app.banner.is_some() {
        (Some(chunks[0]), chunks[1], chunks[2])
    } else {
        (None, chunks[0], chunks[1])
    };

    if let (Some(a), Some(text)) = (banner_area, app.banner.as_deref()) {
        f.render_widget(
            Paragraph::new(Span::styled(text, Style::default().fg(Color::Yellow))),
            a,
        );
    }

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

        let area = centered(70, 45, render_area.unwrap_or_else(|| f.area()));
        f.render_widget(Clear, area);

        let mut lines = Vec::new();
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
                Span::styled(&card.project, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
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

        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                Block::default()
                    .title("Detail")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            area,
        );
    }

    if app.confirm_delete {
        let area = centered(40, 20, render_area.unwrap_or_else(|| f.area()));
        f.render_widget(Clear, area);

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
            area,
        );
    }

    // Edit modal
    render_edit_modal(f, app);

    // Help overlay — drawn last so it appears on top of everything
    render_help(f, app, render_area);

    // Project filter modal
    if let Some(pf) = &app.project_filter_state {
        let area = centered(50, 50, f.area());
        f.render_widget(Clear, area);

        let mut items: Vec<ListItem> = Vec::new();
        for (i, proj_name) in pf.projects.iter().enumerate() {
            let check = if pf.selected[i] { "[x]" } else { "[ ]" };
            let label = if proj_name.is_empty() {
                "(sin proyecto)"
            } else {
                proj_name.as_str()
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("{check} "), Style::default().fg(Color::Cyan)),
                Span::raw(label),
            ])));
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .title("Project Filter")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = ListState::default();
        state.select(Some(pf.cursor));
        f.render_stateful_widget(list, area, &mut state);
    }
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
                Style::default()
            };
            let proj_style = if dimmed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Magenta)
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
