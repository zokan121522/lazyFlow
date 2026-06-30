use std::{
    collections::VecDeque,
    io, panic,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
};

use flow_core::{Board, provider, model::Priority};
use flow_tui::{App, Action, EditState, EditFocus, SearchState, ui::render, ui::action_from_key};

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_tui(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut provider = provider::from_env();

    let board = match provider.load_board() {
        Ok(b) => b,
        Err(e) => {
            let mut app = App::new(Board { columns: vec![] });
            app.banner = Some(format!("Load failed: {e}"));
            loop {
                terminal.draw(|f| render(f, &app, None))?;
                if event::poll(Duration::from_millis(50))? {
                    if let Event::Key(k) = event::read()? {
                        if k.kind == KeyEventKind::Press
                            && matches!(k.code, crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc)
                        {
                            break;
                        }
                    }
                }
            }
            return Ok(());
        }
    };

    let mut app = App::new(board);
    app.focus_first_non_empty();
    type MoveOutcome = Result<Option<Board>, String>;
    let mut move_rx: Option<Receiver<MoveOutcome>> = None;
    let mut move_queue: VecDeque<(String, String)> = VecDeque::new();
    const MAX_QUEUE_SIZE: usize = 64;
    let mut quitting = false;

    loop {
        if let Some(rx) = move_rx.as_ref() {
            match rx.try_recv() {
                Ok(Ok(Some(mut board))) => {
                    board.sort_cards_with(app.sort_order);
                    app.board = board;
                    app.clamp();
                    app.banner = Some(
                        "Move failed: reloaded board (optimistic state corrected)".to_string(),
                    );
                    move_queue.clear();
                    move_rx = None;
                    update_quit_banner(&mut app, quitting, &move_queue, move_rx.is_some());
                }
                Ok(Ok(None)) => {
                    move_rx = None;
                    if let Some((card_id, dst)) = move_queue.pop_front() {
                        move_rx = Some(spawn_move(card_id, dst));
                        app.banner = Some(format!("Moving... ({} queued)", move_queue.len()));
                    } else {
                        app.banner = None;
                    }
                    update_quit_banner(&mut app, quitting, &move_queue, move_rx.is_some());
                }
                Ok(Err(msg)) => {
                    app.banner = Some(format!("Move failed: {msg}"));
                    move_queue.clear();
                    move_rx = None;
                    update_quit_banner(&mut app, quitting, &move_queue, move_rx.is_some());
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    app.banner = Some("Move failed: worker disconnected".to_string());
                    move_rx = None;
                    update_quit_banner(&mut app, quitting, &move_queue, move_rx.is_some());
                }
            }
        }

        if quitting && move_rx.is_none() && move_queue.is_empty() {
            return Ok(());
        }

        terminal.draw(|f| render(f, &app, None))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    if app.show_help {
                        match k.code {
                            crossterm::event::KeyCode::Char('?')
                            | crossterm::event::KeyCode::Esc
                            | crossterm::event::KeyCode::Char('q') => {
                                app.show_help = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if let Some(edit) = app.edit_state.as_mut() {
                        // Clamp scroll_y generously — actual clamping happens in render.
                        // Use generous estimate to avoid getting stuck on ↑/↓.
                        let est_lines = edit
                            .description
                            .lines()
                            .count()
                            .max(edit.description.len() / 40)
                            .saturating_add(5) as u16; // generous padding
                        edit.scroll_y = edit.scroll_y.min(est_lines);

                        match k.code {
                            crossterm::event::KeyCode::Esc => {
                                app.edit_state = None;
                            }
                            crossterm::event::KeyCode::Tab => {
                                edit.focus = edit.focus.next();
                                if edit.focus != EditFocus::Priority {
                                    edit.cursor_pos = edit.current_text().len();
                                }
                                // Reset scroll when switching fields
                                edit.scroll_y = 0;
                            }
                            crossterm::event::KeyCode::Char('k')
                                if k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                edit.insert_char('\n');
                                continue;
                            }
                            crossterm::event::KeyCode::Enter => {
                                let is_new = edit.is_new;
                                let col_id = edit.col_id.clone();
                                let title = edit.title.clone();
                                let description = edit.description.clone();
                                let priority = edit.priority;
                                let assignee = edit.assignee.clone();
                                let project = edit.project.clone();

                                if is_new {
                                    // Create card on disk with project-based ID
                                    match provider.create_card(&col_id, &project) {
                                        Ok(card_id) => {
                                            if let Err(e) = provider.update_card(&card_id, &title, &description, priority, &assignee, &project) {
                                                app.banner = Some(format!("Save failed: {e}"));
                                            } else {
                                                match provider.load_board() {
                                                    Ok(mut b) => {
                                                        b.apply_project_filter(&app.project_filter);
                                                        b.sort_cards_with(app.sort_order);
                                                        app.board = b;
                                                        focus_card_by_id(&mut app, &card_id);
                                                        app.banner = Some("Card created".to_string());
                                                    }
                                                    Err(e) => app.banner = Some(format!("Reload failed: {e}")),
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            app.banner = Some(format!("Create failed: {e}"));
                                        }
                                    }
                                } else {
                                    let card_id = edit.card_id.clone();
                                    if let Err(e) = provider.update_card(&card_id, &title, &description, priority, &assignee, &project) {
                                        app.banner = Some(format!("Save failed: {e}"));
                                    } else {
                                        match provider.load_board() {
                                            Ok(mut b) => {
                                                b.apply_project_filter(&app.project_filter);
                                                b.sort_cards_with(app.sort_order);
                                                app.board = b;
                                                focus_card_by_id(&mut app, &card_id);
                                                app.banner = Some("Card saved".to_string());
                                            }
                                            Err(e) => app.banner = Some(format!("Reload failed: {e}")),
                                        }
                                    }
                                }
                                app.edit_state = None;
                            }
                            crossterm::event::KeyCode::Char(c) => {
                                edit.insert_char(c);
                            }
                            crossterm::event::KeyCode::Backspace => {
                                edit.delete_prev();
                            }
                            crossterm::event::KeyCode::Delete => {
                                edit.delete_curr();
                            }
                            crossterm::event::KeyCode::Left => {
                                edit.move_cursor_left();
                            }
                            crossterm::event::KeyCode::Right => {
                                edit.move_cursor_right();
                            }
                            crossterm::event::KeyCode::Home => {
                                if edit.focus != EditFocus::Priority {
                                    edit.cursor_pos = 0;
                                }
                            }
                            crossterm::event::KeyCode::End => {
                                if edit.focus != EditFocus::Priority {
                                    edit.cursor_pos = edit.current_text().len();
                                }
                            }
                            crossterm::event::KeyCode::Up
                                if edit.focus == EditFocus::Description =>
                            {
                                edit.scroll_y = edit.scroll_y.saturating_sub(1);
                            }
                            crossterm::event::KeyCode::Down
                                if edit.focus == EditFocus::Description =>
                            {
                                edit.scroll_y = (edit.scroll_y + 1).min(est_lines);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Detail view scroll: up/down scroll content
                    if app.detail_open {
                        // Clamp detail_scroll generously — actual clamping happens in render.
                        let est_lines = app
                            .board
                            .columns
                            .get(app.col)
                            .and_then(|col| col.cards.get(app.row))
                            .map(|card| {
                                let lines = card.description.lines().count();
                                let extra = card.description.len() / 40;
                                (7 + lines + extra).saturating_add(5) as u16
                            })
                            .unwrap_or(0);
                        app.detail_scroll = app.detail_scroll.min(est_lines);

                        match k.code {
                            crossterm::event::KeyCode::Up => {
                                app.detail_scroll = app.detail_scroll.saturating_sub(1);
                                continue;
                            }
                            crossterm::event::KeyCode::Down => {
                                app.detail_scroll = (app.detail_scroll + 1).min(est_lines);
                                continue;
                            }
                            _ => {}
                        }
                    }

                    if app.search_state.is_some() {
                        match k.code {
                            crossterm::event::KeyCode::Esc => {
                                app.search_state = None;
                            }
                            crossterm::event::KeyCode::Enter => {
                                app.search_state = None;
                            }
                            crossterm::event::KeyCode::Char(c) => {
                                if let Some(search) = app.search_state.as_mut() {
                                    search.insert_char(c);
                                }
                                let matches = app.search_matches();
                                if !matches.is_empty() {
                                    let current = (app.col, app.row);
                                    if !matches.contains(&current) {
                                        app.col = matches[0].0;
                                        app.row = matches[0].1;
                                    }
                                }
                            }
                            crossterm::event::KeyCode::Backspace => {
                                if let Some(search) = app.search_state.as_mut() {
                                    search.delete_prev();
                                }
                                let matches = app.search_matches();
                                if !matches.is_empty() {
                                    let current = (app.col, app.row);
                                    if !matches.contains(&current) {
                                        app.col = matches[0].0;
                                        app.row = matches[0].1;
                                    }
                                }
                            }
                            crossterm::event::KeyCode::Down => {
                                app.select_next_match();
                            }
                            crossterm::event::KeyCode::Up => {
                                app.select_prev_match();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if app.confirm_delete {
                        match k.code {
                            crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => {
                                if let Some(card_id) = selected_card_id(&app) {
                                    if let Err(e) = provider.delete_card(&card_id) {
                                        app.banner = Some(format!("Delete failed: {e}"));
                                    } else {
                                        match provider.load_board() {
                                            Ok(mut b) => {
                                                b.sort_cards_with(app.sort_order);
                                                app.board = b;
                                                app.clamp();
                                                app.banner = Some(format!("Card {card_id} deleted"));
                                            }
                                            Err(e) => {
                                                app.banner = Some(format!("Reload failed: {e}"))
                                            }
                                        }
                                    }
                                }
                                app.confirm_delete = false;
                            }
                            crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Char('N') | crossterm::event::KeyCode::Esc => {
                                app.confirm_delete = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if k.code == crossterm::event::KeyCode::Char('?') {
                        app.show_help = true;
                        continue;
                    }

                    if let Some(a) = action_from_key(k.code, app.filter_focus) {
                        if quitting {
                            if matches!(a, Action::MoveLeft | Action::MoveRight) {
                                continue;
                            }
                        }

                        match a {
                            Action::ToggleSort => {
                                app.apply(a);
                            }
                            Action::Add => {
                                if quitting {
                                    continue;
                                }
                                let Some(col) = app.board.columns.get(app.col) else {
                                    app.banner = Some("Create failed: no column selected".to_string());
                                    continue;
                                };
                                app.edit_state = Some(EditState {
                                    card_id: String::new(),
                                    col_id: col.id.clone(),
                                    is_new: true,
                                    title: "New card".to_string(),
                                    description: String::new(),
                                    priority: Priority::Medium,
                                    assignee: String::new(),
                                    project: String::new(),
                                    cursor_pos: 8,
                                    focus: EditFocus::Title,
                                    scroll_y: 0,
                                });
                            }
                            Action::Delete => {
                                if !app.board.columns.is_empty() && !app.board.columns[app.col].cards.is_empty() {
                                    app.confirm_delete = true;
                                }
                            }
                            Action::Search => {
                                app.search_state = Some(SearchState::new());
                            }

                            Action::Edit => {
                                if quitting {
                                    continue;
                                }
                                let Some(col) = app.board.columns.get(app.col) else { continue; };
                                let Some(card) = col.cards.get(app.row) else {
                                    app.banner = Some("Edit failed: no card selected".to_string());
                                    continue;
                                };
                                app.edit_state = Some(EditState {
                                    card_id: card.id.clone(),
                                    col_id: col.id.clone(),
                                    is_new: false,
                                    title: card.title.clone(),
                                    description: card.description.clone(),
                                    priority: card.priority,
                                    assignee: card.assignee.clone(),
                                    project: card.project.clone(),
                                    cursor_pos: card.title.len(),
                                    focus: EditFocus::Title,
                                    scroll_y: 0,
                                });
                            }
                            Action::MoveLeft => {
                                if move_rx.is_some() {
                                    if move_queue.len() >= MAX_QUEUE_SIZE {
                                        app.banner = Some(
                                            "Move queue full — too many pending moves".to_string(),
                                        );
                                    } else if let Some((card_id, dst)) = app.optimistic_move(-1) {
                                        move_queue.push_back((card_id, dst));
                                        app.banner = Some(format!(
                                            "Moving... ({} queued)",
                                            move_queue.len()
                                        ));
                                    }
                                } else if let Some((card_id, dst)) = app.optimistic_move(-1) {
                                    move_rx = Some(spawn_move(card_id, dst));
                                    app.banner = Some("Moving...".to_string());
                                }
                            }
                            Action::MoveRight => {
                                if move_rx.is_some() {
                                    if move_queue.len() >= MAX_QUEUE_SIZE {
                                        app.banner = Some(
                                            "Move queue full — too many pending moves".to_string(),
                                        );
                                    } else if let Some((card_id, dst)) = app.optimistic_move(1) {
                                        move_queue.push_back((card_id, dst));
                                        app.banner = Some(format!(
                                            "Moving... ({} queued)",
                                            move_queue.len()
                                        ));
                                    }
                                } else if let Some((card_id, dst)) = app.optimistic_move(1) {
                                    move_rx = Some(spawn_move(card_id, dst));
                                    app.banner = Some("Moving...".to_string());
                                }
                            }
                            Action::Refresh => {
                                if quitting {
                                    continue;
                                }
                                match provider.load_board() {
                                    Ok(mut b) => {
                                        b.apply_project_filter(&app.project_filter);
                                        b.sort_cards_with(app.sort_order);
                                        app.board = b;
                                        app.focus_first_non_empty();
                                        app.banner = None;
                                    }
                                    Err(e) => app.banner = Some(format!("Refresh failed: {e}")),
                                }
                            }
                            _ => {
                                if app.apply(a) {
                                    if move_rx.is_some() || !move_queue.is_empty() {
                                        quitting = true;
                                        update_quit_banner(
                                            &mut app,
                                            quitting,
                                            &move_queue,
                                            move_rx.is_some(),
                                        );
                                    } else {
                                        break;
                                    }
                                }
                                if app.filter_dirty {
                                    app.filter_dirty = false;
                                    match provider.load_board() {
                                        Ok(mut b) => {
                                            b.apply_project_filter(&app.project_filter);
                                            b.sort_cards_with(app.sort_order);
                                            app.board = b;
                                            app.focus_first_non_empty();
                                            app.banner = if app.project_filter.is_empty() {
                                                Some("Project filter: all".to_string())
                                            } else {
                                                Some(format!("Project filter: {}", app.project_filter.join(", ")))
                                            };
                                        }
                                        Err(e) => app.banner = Some(format!("Reload failed: {e}")),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn selected_card_id(app: &App) -> Option<String> {
    app.board
        .columns
        .get(app.col)
        .and_then(|col| col.cards.get(app.row))
        .map(|card| card.id.clone())
}

fn focus_card_by_id(app: &mut App, card_id: &str) {
    for (col_idx, col) in app.board.columns.iter().enumerate() {
        if let Some(row_idx) = col.cards.iter().position(|c| c.id == card_id) {
            app.col = col_idx;
            app.row = row_idx;
            app.clamp();
            return;
        }
    }
    app.focus_first_non_empty();
}

fn update_quit_banner(
    app: &mut App,
    quitting: bool,
    move_queue: &VecDeque<(String, String)>,
    move_in_flight: bool,
) {
    if !quitting {
        return;
    }
    let pending = move_queue.len() + if move_in_flight { 1 } else { 0 };
    app.banner = if pending == 0 {
        None
    } else {
        Some(format!("Finishing {pending} pending moves before quit..."))
    };
}

fn spawn_move(card_id: String, dst: String) -> Receiver<Result<Option<Board>, String>> {
    let (tx, rx) = mpsc::channel::<Result<Option<Board>, String>>();
    thread::spawn(move || {
        let res = panic::catch_unwind(|| {
            let mut p = flow_core::provider::from_env();
            match p.move_card(&card_id, &dst) {
                Ok(()) => {
                    let _ = tx.send(Ok(None));
                }
                Err(move_err) => match p.load_board() {
                    Ok(board) => {
                        let _ = tx.send(Ok(Some(board)));
                    }
                    Err(_) => {
                        let _ = tx.send(Err(move_err.to_string()));
                    }
                },
            }
        });
        if res.is_err() {
            let _ = tx.send(Err("worker panicked".to_string()));
        }
    });
    rx
}
