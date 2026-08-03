use std::{
    collections::VecDeque,
    io, panic,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
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

use flow_core::gdrive::{GDriveClient, GDriveStatus, PollResult};
use flow_core::{Board, provider, model::Priority};
use flow_tui::{App, Action, EditFocus, EditState, SearchState, ui::action_from_key, ui::render};

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

    let refresh_interval = refresh_interval_ms();
    let mut last_refresh = Instant::now();

    let mut app = App::new(board);
    app.refresh_interval_ms = refresh_interval;
    app.focus_first_non_empty();

    // Per-project color overrides from colors.json (best-effort).
    app.project_colors = provider.load_project_colors();

    // ── Google Drive init ──────────────────────────────────────────────
    let mut gdrive = GDriveClient::new();
    app.gdrive_status = gdrive.status().clone();
    app.gdrive_has_client_id = gdrive.has_client_id();
    if let Some(sync) = gdrive.last_sync() {
        app.gdrive_last_sync = Some(sync.to_string());
    }

    // Pull board from Google Drive on startup (last-write-wins).
    if gdrive.is_authenticated() {
        match gdrive.download_board() {
            Ok(Some(board_json)) => {
                if let Ok(remote_board) = serde_json::from_value(board_json) {
                    app.board = remote_board;
                    app.focus_first_non_empty();
                    app.banner = Some("Loaded board from Google Drive".to_string());
                    app.last_refresh_at = Some(Instant::now());
                }
            }
            Ok(None) => {
                // First time — no board on GDrive yet.
                // Push the local board so GDrive has a copy.
                if let Ok(board_json) = serde_json::to_value(&app.board) {
                    if let Err(e) = gdrive.upload_board(&board_json) {
                        eprintln!("[gdrive] initial upload failed: {e}");
                    }
                }
            }
            Err(e) => {
                app.banner = Some(format!("GDrive pull failed: {e}"));
            }
        }
        app.gdrive_status = gdrive.status().clone();
        app.gdrive_last_sync = gdrive.last_sync().map(|s| s.to_string());
    }

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
                    gdrive_sync(&mut gdrive, &mut app);
                    update_quit_banner(&mut app, quitting, &move_queue, move_rx.is_some());
                }
                Ok(Ok(None)) => {
                    move_rx = None;
                    if let Some((card_id, dst)) = move_queue.pop_front() {
                        move_rx = Some(spawn_move(card_id, dst));
                        app.banner = Some(format!("Moving... ({} queued)", move_queue.len()));
                    } else {
                        app.banner = None;
                        // Move completed successfully — sync to GDrive
                        gdrive_sync(&mut gdrive, &mut app);
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

        // Auto-refresh: poll board from disk at configurable interval
        // Skips refresh when editing or viewing detail to avoid disrupting the user.
        if refresh_interval > 0 && last_refresh.elapsed() >= Duration::from_millis(refresh_interval) {
            try_refresh(&mut app, &mut provider, false);
            last_refresh = Instant::now();
        }

        // ── GDrive auth polling ─────────────────────────────────────────
        if matches!(gdrive.status(), GDriveStatus::Authorizing { .. }) {
            let result = gdrive.try_poll_token();
            match result {
                PollResult::Success => {
                    app.banner = Some("GDrive: Connected!".to_string());
                    // Push board to freshly-authorized Drive
                    gdrive_sync(&mut gdrive, &mut app);
                }
                PollResult::Expired => {
                    app.banner = Some("GDrive: Authorization expired. Try again.".to_string());
                }
                PollResult::Denied => {
                    app.banner = Some("GDrive: Authorization denied.".to_string());
                }
                PollResult::TransientError(msg) => {
                    eprintln!("[gdrive] poll error: {msg}");
                }
                PollResult::Waiting => {}
            }
            app.gdrive_status = gdrive.status().clone();
            app.gdrive_has_client_id = gdrive.has_client_id();
            app.gdrive_last_sync = gdrive.last_sync().map(|s| s.to_string());
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

                    // Ctrl+R force-refresh — works even in edit/detail mode
                    if k.code == crossterm::event::KeyCode::Char('r')
                        && k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                    {
                        try_refresh(&mut app, &mut provider, true);
                        last_refresh = Instant::now();
                        continue;
                    }

                    // Ctrl+G / Ctrl+T — GDrive popup toggle (direct, before action_from_key)
                    if (k.code == crossterm::event::KeyCode::Char('g')
                        || k.code == crossterm::event::KeyCode::Char('t'))
                        && k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                    {
                        app.gdrive_popup_open = !app.gdrive_popup_open;
                        if app.gdrive_popup_open {
                            app.banner =
                                Some("GDrive: press C to connect, E to edit Client ID".to_string());
                        } else {
                            app.banner = None;
                        }
                        continue;
                    }

                    // ── GDrive popup key handling ────────────────────────
                    if app.gdrive_popup_open {
                        // If editing client ID, route chars to the buffer
                        if let Some(ref mut buf) = app.gdrive_client_id_input {
                            match k.code {
                                crossterm::event::KeyCode::Enter => {
                                    let client_id = buf.trim().to_string();
                                    if !client_id.is_empty() {
                                        // Save via config
                                        let mut config = gdrive.get_config();
                                        config.client_id = client_id.clone();
                                        if let Err(e) = gdrive.save_config(&config) {
                                            app.banner =
                                                Some(format!("GDrive: failed to save config: {e}"));
                                        } else {
                                            gdrive.set_client_id(client_id);
                                            app.banner = Some("GDrive: Client ID saved".to_string());
                                        }
                                    }
                                    app.gdrive_client_id_input = None;
                                }
                                crossterm::event::KeyCode::Esc => {
                                    app.gdrive_client_id_input = None;
                                }
                                crossterm::event::KeyCode::Char(c) => {
                                    buf.push(c);
                                }
                                crossterm::event::KeyCode::Backspace => {
                                    buf.pop();
                                }
                                _ => {}
                            }
                        } else {
                            match k.code {
                                crossterm::event::KeyCode::Char('c')
                                | crossterm::event::KeyCode::Char('C') => {
                                    if let Err(e) = gdrive.start_device_auth() {
                                        app.banner = Some(format!("GDrive: {e}"));
                                    }
                                }
                                crossterm::event::KeyCode::Char('d')
                                | crossterm::event::KeyCode::Char('D') => {
                                    if let Err(e) = gdrive.disconnect() {
                                        app.banner = Some(format!("GDrive: {e}"));
                                    }
                                    app.gdrive_popup_open = false;
                                }
                                crossterm::event::KeyCode::Char('s')
                                | crossterm::event::KeyCode::Char('S') => {
                                    gdrive_sync(&mut gdrive, &mut app);
                                    app.banner = Some("GDrive: Synced".to_string());
                                }
                                crossterm::event::KeyCode::Char('e')
                                | crossterm::event::KeyCode::Char('E') => {
                                    if !gdrive.has_client_id() {
                                        let current = gdrive.get_config().client_id;
                                        app.gdrive_client_id_input = Some(current);
                                    }
                                }
                                crossterm::event::KeyCode::Esc => {
                                    if matches!(gdrive.status(), GDriveStatus::Authorizing { .. })
                                    {
                                        gdrive.cancel_auth();
                                    }
                                    app.gdrive_popup_open = false;
                                }
                                _ => {}
                            }
                        }
                        app.gdrive_status = gdrive.status().clone();
                        app.gdrive_has_client_id = gdrive.has_client_id();
                        app.gdrive_last_sync = gdrive.last_sync().map(|s| s.to_string());
                        continue;
                    }

                    // ── Project color picker key handling ───────────────
                    if app.color_picker_open {
                        match k.code {
                            crossterm::event::KeyCode::Left => {
                                app.apply(Action::ColorPickerLeft);
                            }
                            crossterm::event::KeyCode::Right => {
                                app.apply(Action::ColorPickerRight);
                            }
                            crossterm::event::KeyCode::Enter => {
                                app.apply(Action::ColorPickerConfirm);
                                // Persist the updated color map
                                if app.colors_dirty {
                                    if let Err(e) = provider.save_project_colors(&app.project_colors)
                                    {
                                        app.banner =
                                            Some(format!("Failed to save project colors: {e}"));
                                    }
                                    app.colors_dirty = false;
                                }
                            }
                            crossterm::event::KeyCode::Esc => {
                                app.color_picker_open = false;
                            }
                            crossterm::event::KeyCode::Tab => {
                                app.color_picker_open = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if let Some(edit) = app.edit_state.as_mut() {
                        // Terminal-size helper for description field (85% modal, 13 rows fixed overhead)
                        let desc_dims = || -> (usize, usize) {
                            crossterm::terminal::size()
                                .ok()
                                .map(|(tw, th)| {
                                    let iw = ((tw as usize).saturating_mul(70) / 100).saturating_sub(2);
                                    let dh = ((th as usize).saturating_mul(85) / 100).saturating_sub(15);
                                    (iw, dh.max(1))
                                })
                                .unwrap_or((40, 5))
                        };

                        match k.code {
                            crossterm::event::KeyCode::Esc => {
                                app.edit_state = None;
                            }
                            crossterm::event::KeyCode::Tab => {
                                edit.focus = edit.focus.next();
                                if edit.focus != EditFocus::Priority {
                                    edit.cursor_pos = edit.current_text().len();
                                }
                                edit.scroll_y = 0;
                                // When entering Description, snap cursor into visible area
                                if edit.focus == EditFocus::Description {
                                    let (iw, dh) = desc_dims();
                                    edit.ensure_cursor_visible(iw, dh);
                                }
                            }
                            crossterm::event::KeyCode::Char('k')
                                if k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                edit.insert_char('\n');
                                if edit.focus == EditFocus::Description {
                                    let (iw, dh) = desc_dims();
                                    edit.ensure_cursor_visible(iw, dh);
                                }
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
                                                         gdrive_sync(&mut gdrive, &mut app);
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
                                                 gdrive_sync(&mut gdrive, &mut app);
                                             }
                                             Err(e) => app.banner = Some(format!("Reload failed: {e}")),
                                        }
                                    }
                                }
                                app.edit_state = None;
                            }
                            crossterm::event::KeyCode::Char(c) => {
                                edit.insert_char(c);
                                if edit.focus == EditFocus::Description {
                                    let (iw, dh) = desc_dims();
                                    edit.ensure_cursor_visible(iw, dh);
                                }
                            }
                            crossterm::event::KeyCode::Backspace => {
                                edit.delete_prev();
                                if edit.focus == EditFocus::Description {
                                    let (iw, dh) = desc_dims();
                                    edit.ensure_cursor_visible(iw, dh);
                                }
                            }
                            crossterm::event::KeyCode::Delete => {
                                edit.delete_curr();
                                if edit.focus == EditFocus::Description {
                                    let (iw, dh) = desc_dims();
                                    edit.ensure_cursor_visible(iw, dh);
                                }
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
                                let (iw, dh) = desc_dims();
                                edit.move_cursor_up(iw);
                                edit.ensure_cursor_visible(iw, dh);
                            }
                            crossterm::event::KeyCode::Down
                                if edit.focus == EditFocus::Description =>
                            {
                                let (iw, dh) = desc_dims();
                                edit.move_cursor_down(iw);
                                edit.ensure_cursor_visible(iw, dh);
                            }
                            crossterm::event::KeyCode::PageUp
                                if edit.focus == EditFocus::Description =>
                            {
                                let (iw, dh) = desc_dims();
                                edit.move_cursor_up_n(dh, iw);
                                edit.ensure_cursor_visible(iw, dh);
                            }
                            crossterm::event::KeyCode::PageDown
                                if edit.focus == EditFocus::Description =>
                            {
                                let (iw, dh) = desc_dims();
                                edit.move_cursor_down_n(dh, iw);
                                edit.ensure_cursor_visible(iw, dh);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Detail view scroll: up/down scroll content
                    if app.detail_open {
                        let est_lines = app
                            .board
                            .columns
                            .get(app.col)
                            .and_then(|col| col.cards.get(app.row))
                            .map(|card| {
                                let lines = card.description.lines().count();
                                (7 + lines) as u16
                            })
                            .unwrap_or(0);

                        // Compute actual max_scroll using terminal size (matches render logic)
                        let max_scroll = crossterm::terminal::size()
                            .ok()
                            .map(|(_tw, th)| {
                                // main area = terminal - filter(3) - help(2) = th - 5
                                // visible_height = main - borders(2) = th - 7
                                let visible_height = (th as usize).saturating_sub(7);
                                (est_lines as usize).saturating_sub(visible_height)
                            })
                            .unwrap_or(est_lines as usize);
                        app.detail_scroll = app.detail_scroll.min(max_scroll as u16);

                        match k.code {
                            crossterm::event::KeyCode::Up => {
                                app.detail_scroll = app.detail_scroll.saturating_sub(1);
                                continue;
                            }
                            crossterm::event::KeyCode::Down => {
                                app.detail_scroll = (app.detail_scroll + 1).min(max_scroll as u16);
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
                                                 gdrive_sync(&mut gdrive, &mut app);
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

                    if let Some(a) = action_from_key(k.code, k.modifiers, app.filter_focus) {
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
                                let cur_id = selected_card_id(&app);
                                match provider.load_board() {
                                    Ok(mut b) => {
                                        b.apply_project_filter(&app.project_filter);
                                        b.sort_cards_with(app.sort_order);
                                        app.board = b;
                                        if let Some(id) = cur_id {
                                            focus_card_by_id(&mut app, &id);
                                        } else {
                                            app.focus_first_non_empty();
                                        }
                                        app.banner = None;
                                    }
                                    Err(e) => app.banner = Some(format!("Refresh failed: {e}")),
                                }
                                app.last_refresh_at = Some(Instant::now());
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

fn try_refresh(app: &mut App, provider: &mut Box<dyn flow_core::Provider>, force: bool) {
    if !force && (app.edit_state.is_some() || app.detail_open || app.filter_focus) {
        return; // Safe: don't interrupt editing, detail view, or filter
    }
    let cur_id = selected_card_id(app);
    match provider.load_board() {
        Ok(mut b) => {
            b.apply_project_filter(&app.project_filter);
            b.sort_cards_with(app.sort_order);
            app.board = b;
            if let Some(id) = cur_id {
                focus_card_by_id(app, &id);
            } else {
                app.focus_first_non_empty();
            }
            app.last_refresh_at = Some(Instant::now());
            app.banner = None;
        }
        Err(e) => {
            if force {
                app.banner = Some(format!("Refresh failed: {e}"));
            }
        }
    }
}

fn refresh_interval_ms() -> u64 {
    std::env::var("FLOW_REFRESH_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000)
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

/// Upload the current board to Google Drive (best-effort).
///
/// Only does something if the GDrive client is authenticated. Errors are
/// silently logged — they are visible in the popup status.
fn gdrive_sync(gdrive: &mut GDriveClient, app: &mut App) {
    if !gdrive.is_authenticated() {
        return;
    }
    if let Ok(board_json) = serde_json::to_value(&app.board) {
        if let Err(e) = gdrive.upload_board(&board_json) {
            eprintln!("[gdrive] sync failed: {e}");
        }
    } else {
        eprintln!("[gdrive] failed to serialize board");
    }
    app.gdrive_status = gdrive.status().clone();
    app.gdrive_last_sync = gdrive.last_sync().map(|s| s.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_interval_default_is_3000() {
        assert_eq!(refresh_interval_ms(), 3000);
    }

    #[test]
    fn try_refresh_skips_when_editing() {
        let board = Board { columns: vec![] };
        let mut app = App::new(board);
        // Simulate editing
        app.edit_state = Some(EditState {
            card_id: "x".into(),
            col_id: "todo".into(),
            is_new: false,
            title: "test".into(),
            description: "".into(),
            priority: Priority::Medium,
            assignee: "".into(),
            project: "".into(),
            cursor_pos: 0,
            focus: EditFocus::Title,
            scroll_y: 0,
        });
        // With force=false, should skip without crashing
        let mut provider = flow_core::provider::from_env();
        try_refresh(&mut app, &mut provider, false);
        // App state unchanged (still in edit mode)
        assert!(app.edit_state.is_some());
    }

    #[test]
    fn try_refresh_skips_when_detail_open() {
        let mut app = App::new(Board { columns: vec![] });
        app.detail_open = true;
        let mut provider = flow_core::provider::from_env();
        try_refresh(&mut app, &mut provider, false);
        assert!(app.detail_open);
    }

    #[test]
    fn try_refresh_skips_when_filter_focused() {
        let mut app = App::new(Board { columns: vec![] });
        app.filter_focus = true;
        let mut provider = flow_core::provider::from_env();
        try_refresh(&mut app, &mut provider, false);
        assert!(app.filter_focus);
    }

    #[test]
    fn try_refresh_force_works_even_with_edit() {
        let mut app = App::new(Board { columns: vec![] });
        app.edit_state = Some(EditState {
            card_id: "x".into(),
            col_id: "todo".into(),
            is_new: false,
            title: "test".into(),
            description: "".into(),
            priority: Priority::Medium,
            assignee: "".into(),
            project: "".into(),
            cursor_pos: 0,
            focus: EditFocus::Title,
            scroll_y: 0,
        });
        let mut provider = flow_core::provider::from_env();
        // force=true should attempt refresh even with edit open
        // This won't crash even if load_board fails (e.g. no board configured)
        try_refresh(&mut app, &mut provider, true);
        // Edit state should remain (force doesn't close edit, just refreshes board)
        assert!(app.edit_state.is_some());
    }

    #[test]
    fn selected_card_id_returns_none_for_empty_board() {
        let app = App::new(Board { columns: vec![] });
        assert_eq!(selected_card_id(&app), None);
    }
}
