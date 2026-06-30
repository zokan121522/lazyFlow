use flow_core::model::{Card, Priority};

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


