use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
};

use flow_core::model::Priority;

use crate::app::App;

pub fn priority_color(p: Priority) -> Color {
    match p {
        Priority::Bug => Color::Red,
        Priority::High => Color::Yellow,
        Priority::Medium => Color::White,
        Priority::Low => Color::DarkGray,
        Priority::Wishlist => Color::Cyan,
    }
}

pub fn centered(px: u16, py: u16, r: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - py) / 2),
            Constraint::Percentage(py),
            Constraint::Percentage((100 - py) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - px) / 2),
            Constraint::Percentage(px),
            Constraint::Percentage((100 - px) / 2),
        ])
        .split(v[1])[1]
}

pub fn selected_card_id(app: &App) -> Option<String> {
    app.board
        .columns
        .get(app.col)
        .and_then(|col| col.cards.get(app.row))
        .map(|card| card.id.clone())
}

pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for line in text.lines().chain(if text.ends_with('\n') { Some("") } else { None }) {
        if line.is_empty() {
            lines.push("".to_string());
            continue;
        }
        let mut current_line = String::new();
        for word in line.split_inclusive(' ') {
            if current_line.len() + word.len() > width && !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }
            // If the word itself is too long, we must break it
            let mut remaining_word = word;
            while remaining_word.len() > width {
                let (part, rest) = remaining_word.split_at(width);
                lines.push(part.to_string());
                remaining_word = rest;
            }
            current_line.push_str(remaining_word);
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    if lines.is_empty() {
        lines.push("".to_string());
    }
    lines
}

pub fn calculate_visual_cursor_pos(text: &str, cursor_pos: usize, width: usize) -> (usize, usize) {
    if width == 0 {
        return (0, 0);
    }

    let mut current_offset = 0;
    let mut y = 0;

    for line in text.split_inclusive('\n') {
        let line_len = line.len();

        if cursor_pos >= current_offset && cursor_pos <= current_offset + line_len {
            // Found the hard line where the cursor is.
            let mut x = 0;
            let mut line_y = y;
            let mut current_pos_in_line = 0;
            let target_pos_in_line = cursor_pos - current_offset;

            for word in line.split_inclusive(' ') {
                let word_len = word.len();

                if x + word_len > width && x > 0 {
                    line_y += 1;
                    x = 0;
                }

                if target_pos_in_line >= current_pos_in_line
                    && target_pos_in_line <= current_pos_in_line + word_len
                {
                    // Cursor is in this word
                    let delta = target_pos_in_line - current_pos_in_line;

                    let mut remaining_delta = delta;
                    let mut temp_x = x;
                    let mut temp_y = line_y;

                    while remaining_delta > width - temp_x && width > 0 {
                        let can_fit = width - temp_x;
                        remaining_delta -= can_fit;
                        temp_y += 1;
                        temp_x = 0;
                    }
                    return (temp_x + remaining_delta, temp_y);
                }

                let mut remaining_word_len = word_len;
                while remaining_word_len > width - x && width > 0 {
                    let can_fit = width - x;
                    remaining_word_len -= can_fit;
                    line_y += 1;
                    x = 0;
                }
                x += remaining_word_len;
                current_pos_in_line += word_len;
            }

            return (x, line_y);
        }

        // Count how many soft lines this hard line takes
        let wrapped = wrap_text(line.trim_end_matches('\n'), width);
        y += wrapped.len().max(1);
        current_offset += line_len;
    }

    (0, y)
}
