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
        Priority::Medium => Color::LightBlue,
        Priority::Low => Color::DarkGray,
        Priority::Wishlist => Color::Cyan,
    }
}

/// Canonical palette shown in the project color picker.
/// Names are stable identifiers persisted in `colors.json`.
pub const PROJECT_COLOR_PALETTE: &[(&str, Color)] = &[
    ("Red", Color::Red),
    ("LightRed", Color::LightRed),
    ("Green", Color::Green),
    ("LightGreen", Color::LightGreen),
    ("Yellow", Color::Yellow),
    ("LightYellow", Color::LightYellow),
    ("Blue", Color::Blue),
    ("LightBlue", Color::LightBlue),
    ("Magenta", Color::Magenta),
    ("LightMagenta", Color::LightMagenta),
    ("Cyan", Color::Cyan),
    ("LightCyan", Color::LightCyan),
];

/// Resolve a persisted color name (from `colors.json`) to a `Color`.
pub fn color_from_name(name: &str) -> Option<Color> {
    PROJECT_COLOR_PALETTE
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, c)| *c)
}

/// Canonical name for a palette color (used when persisting).
pub fn color_name(color: Color) -> &'static str {
    PROJECT_COLOR_PALETTE
        .iter()
        .find(|(_, c)| *c == color)
        .map(|(n, _)| *n)
        .unwrap_or("LightCyan")
}

/// Color for a project name. If the project has an override in `colors`,
/// that color wins; otherwise a deterministic hash picks from the palette.
pub fn project_color(name: &str, colors: &std::collections::HashMap<String, String>) -> Color {
    if let Some(persisted) = colors.get(name) {
        if let Some(c) = color_from_name(persisted) {
            return c;
        }
    }
    let h: u64 = name
        .to_lowercase()
        .bytes()
        .fold(0xcbf29ce484222325u64, |acc, b| {
            (acc ^ b as u64).wrapping_mul(0x100000001b3)
        });
    PROJECT_COLOR_PALETTE[(h as usize) % PROJECT_COLOR_PALETTE.len()].1
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

/// Find the best byte position in `text` closest to a target visual line and column.
/// Scans all character boundaries and the end position to find the closest match.
pub fn find_closest_in_visual_line(text: &str, target_x: usize, target_y: usize, width: usize) -> usize {
    let mut best_pos = 0;
    let mut best_x_dist = usize::MAX;

    for (byte_idx, _) in text.char_indices() {
        let (x, y) = calculate_visual_cursor_pos(text, byte_idx, width);
        if y == target_y {
            let x_dist = x.abs_diff(target_x);
            if x_dist < best_x_dist {
                best_x_dist = x_dist;
                best_pos = byte_idx;
            }
        }
    }

    // Also try end-of-string position
    let (x, y) = calculate_visual_cursor_pos(text, text.len(), width);
    if y == target_y && x.abs_diff(target_x) < best_x_dist {
        best_pos = text.len();
    }

    best_pos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn color_roundtrip_palette_names() {
        for (name, color) in PROJECT_COLOR_PALETTE {
            assert_eq!(color_from_name(name), Some(*color), "parse {name}");
            assert_eq!(color_name(*color), *name, "name of {name}");
        }
    }

    #[test]
    fn color_from_name_is_case_insensitive() {
        assert_eq!(color_from_name("lightblue"), Some(Color::LightBlue));
        assert_eq!(color_from_name("LIGHTRED"), Some(Color::LightRed));
        assert_eq!(color_from_name("nope"), None);
    }

    #[test]
    fn project_color_uses_override_when_present() {
        let colors = map(&[("studyflow", "LightCyan")]);
        assert_eq!(project_color("studyflow", &colors), Color::LightCyan);
    }

    #[test]
    fn project_color_falls_back_to_hash_without_override() {
        let colors = map(&[]);
        // Deterministic: same name always yields same color.
        let a = project_color("studyflow", &colors);
        let b = project_color("studyflow", &colors);
        assert_eq!(a, b);
        // Different names generally differ (palette has 12 entries).
        let c = project_color("server", &colors);
        assert_eq!(project_color("server", &colors), c);
    }

    #[test]
    fn project_color_ignores_invalid_override() {
        let colors = map(&[("studyflow", "not-a-color")]);
        // Falls back to hash instead of panicking.
        let _ = project_color("studyflow", &colors);
    }
}
