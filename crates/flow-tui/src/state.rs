use flow_core::model::{Card, Priority};

use crate::util::{calculate_visual_cursor_pos, find_closest_in_visual_line, wrap_text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditFocus {
    Title,
    Description,
    Priority,
    Assignee,
    Project,
}

impl EditFocus {
    pub fn next(self) -> Self {
        match self {
            EditFocus::Title => EditFocus::Project,
            EditFocus::Project => EditFocus::Priority,
            EditFocus::Priority => EditFocus::Assignee,
            EditFocus::Assignee => EditFocus::Description,
            EditFocus::Description => EditFocus::Title,
        }
    }
}

pub struct EditState {
    pub card_id: String,
    pub col_id: String,
    pub is_new: bool,
    pub title: String,
    pub description: String,
    pub priority: Priority,
    pub assignee: String,
    pub project: String,
    pub cursor_pos: usize,
    pub focus: EditFocus,
    /// Vertical scroll offset for the description field.
    pub scroll_y: u16,
}

impl EditState {
    pub fn current_text(&self) -> &str {
        match self.focus {
            EditFocus::Title => &self.title,
            EditFocus::Description => &self.description,
            EditFocus::Assignee => &self.assignee,
            EditFocus::Project => &self.project,
            EditFocus::Priority => "",
        }
    }

    pub fn current_text_mut(&mut self) -> &mut String {
        match self.focus {
            EditFocus::Title => &mut self.title,
            EditFocus::Description => &mut self.description,
            EditFocus::Assignee => &mut self.assignee,
            EditFocus::Project => &mut self.project,
            EditFocus::Priority => &mut self.title,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if matches!(self.focus, EditFocus::Priority) {
            return;
        }
        let pos = self.cursor_pos;
        let text = self.current_text_mut();
        if pos >= text.len() {
            text.push(c);
        } else {
            text.insert(pos, c);
        }
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_prev(&mut self) {
        if matches!(self.focus, EditFocus::Priority) {
            return;
        }
        if self.cursor_pos > 0 {
            let pos = self.cursor_pos;
            let text = self.current_text_mut();
            if let Some((idx, _)) = text.char_indices().filter(|(i, _)| *i < pos).last() {
                text.remove(idx);
                self.cursor_pos = idx;
            }
        }
    }

    pub fn delete_curr(&mut self) {
        if matches!(self.focus, EditFocus::Priority) {
            return;
        }
        let pos = self.cursor_pos;
        let text = self.current_text_mut();
        if pos < text.len() {
            text.remove(pos);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if matches!(self.focus, EditFocus::Priority) {
            self.priority = self.priority.prev();
            return;
        }
        if self.cursor_pos > 0 {
            let text = self.current_text();
            let pos = self.cursor_pos;
            if let Some((idx, _)) = text.char_indices().filter(|(i, _)| *i < pos).last() {
                self.cursor_pos = idx;
            }
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.focus == EditFocus::Priority {
            self.priority = self.priority.next();
            return;
        }
        let text = self.current_text();
        let pos = self.cursor_pos;
        if let Some((idx, _)) = text.char_indices().filter(|(i, _)| *i > pos).next() {
            self.cursor_pos = idx;
        } else if pos < text.len() {
            self.cursor_pos = text.len();
        }
    }

    /// Move cursor up one visual line in the Description field.
    /// `width` is the inner width of the description widget.
    pub fn move_cursor_up(&mut self, width: usize) {
        if self.focus != EditFocus::Description || width == 0 || self.description.is_empty() {
            return;
        }
        let (cx, cy) = calculate_visual_cursor_pos(&self.description, self.cursor_pos, width);
        if cy == 0 {
            return;
        }
        self.cursor_pos = find_closest_in_visual_line(&self.description, cx, cy - 1, width);
    }

    /// Move cursor down one visual line in the Description field.
    /// `width` is the inner width of the description widget.
    pub fn move_cursor_down(&mut self, width: usize) {
        if self.focus != EditFocus::Description || width == 0 || self.description.is_empty() {
            return;
        }
        let (cx, cy) = calculate_visual_cursor_pos(&self.description, self.cursor_pos, width);
        let total = wrap_text(&self.description, width).len();
        if cy >= total.saturating_sub(1) {
            return;
        }
        self.cursor_pos = find_closest_in_visual_line(&self.description, cx, cy + 1, width);
    }

    /// Real scroll: always keep the cursor near the top of the viewport (line 2).
    /// Every ↑/↓ movement causes the viewport to scroll smoothly so the cursor
    /// stays at a consistent visual position — just like a real text editor.
    pub fn ensure_cursor_visible(&mut self, width: usize, visible_height: usize) {
        if self.focus != EditFocus::Description || width == 0 || visible_height == 0 {
            return;
        }
        let (_, cy) = calculate_visual_cursor_pos(&self.description, self.cursor_pos, width);
        let total = wrap_text(&self.description, width).len();
        // Keep cursor at visual line 2 of the viewport (offset by 1 from top)
        let ideal = if cy >= 2 { cy - 2 } else { 0 };
        let max_scroll = total.saturating_sub(visible_height);
        self.scroll_y = ideal.min(max_scroll) as u16;
    }

    /// Move cursor up by `n` visual lines in the Description field.
    pub fn move_cursor_up_n(&mut self, n: usize, width: usize) {
        for _ in 0..n {
            if self.focus != EditFocus::Description || width == 0 || self.description.is_empty() {
                break;
            }
            let (cx, cy) = calculate_visual_cursor_pos(&self.description, self.cursor_pos, width);
            if cy == 0 {
                break;
            }
            let target_y = cy.saturating_sub(1);
            self.cursor_pos = find_closest_in_visual_line(&self.description, cx, target_y, width);
        }
    }

    /// Move cursor down by `n` visual lines in the Description field.
    pub fn move_cursor_down_n(&mut self, n: usize, width: usize) {
        for _ in 0..n {
            if self.focus != EditFocus::Description || width == 0 || self.description.is_empty() {
                break;
            }
            let (cx, cy) = calculate_visual_cursor_pos(&self.description, self.cursor_pos, width);
            let total = wrap_text(&self.description, width).len();
            if cy >= total.saturating_sub(1) {
                break;
            }
            self.cursor_pos = find_closest_in_visual_line(&self.description, cx, cy + 1, width);
        }
    }
}

pub struct SearchState {
    pub query: String,
    pub cursor_pos: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            cursor_pos: 0,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let pos = self.cursor_pos;
        if pos >= self.query.len() {
            self.query.push(c);
        } else {
            self.query.insert(pos, c);
        }
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_prev(&mut self) {
        if self.cursor_pos > 0 {
            let pos = self.cursor_pos;
            if let Some((idx, _)) = self.query.char_indices().filter(|(i, _)| *i < pos).last() {
                self.query.remove(idx);
                self.cursor_pos = idx;
            }
        }
    }

    pub fn matches_card(card: &Card, query: &str) -> bool {
        if query.is_empty() {
            return false;
        }
        let q = query.to_lowercase();
        card.title.to_lowercase().contains(&q) || card.description.to_lowercase().contains(&q)
    }
}


