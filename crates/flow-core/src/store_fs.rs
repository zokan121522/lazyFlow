use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::model::{Board, Card, Column, Priority};

pub fn load_board(root: &Path) -> io::Result<Board> {
    let txt = fs::read_to_string(root.join("board.txt"))?;
    let mut cols = Vec::new();

    for line in txt.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some(rest) = line.strip_prefix("col ") else {
            continue;
        };
        let (id, title) = parse_col(rest)?;
        let cards = load_cards(root, &id)?;
        cols.push(Column { id, title, cards });
    }

    let mut board = Board { columns: cols };
    board.sort_cards();
    Ok(board)
}

fn parse_col(rest: &str) -> io::Result<(String, String)> {
    let mut it = rest.splitn(2, ' ');
    let Some(id) = it.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing column id",
        ));
    };
    let title = it.next().unwrap_or(id).trim().trim_matches('"');
    Ok((id.to_string(), title.to_string()))
}

fn load_cards(root: &Path, col_id: &str) -> io::Result<Vec<Card>> {
    let dir = root.join("cols").join(col_id);
    let order_path = dir.join("order.txt");
    if !order_path.exists() {
        return Ok(vec![]);
    }

    let order = fs::read_to_string(order_path)?;
    let mut cards = Vec::new();

    for id in order.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let md_path = dir.join(format!("{id}.md"));
        if !md_path.exists() {
            // Orphaned card in order.txt — skip silently rather than fail
            continue;
        }
        let raw = fs::read_to_string(&md_path)?;
        let (title, desc, priority, assignee, project, updated_at) = parse_md(&raw, id);
        cards.push(Card {
            id: id.to_string(),
            title,
            description: desc,
            priority,
            assignee,
            project,
            updated_at,
        });
    }

    Ok(cards)
}

pub fn read_card_content(path: &Path) -> io::Result<(String, String, Priority, String, String, Option<i64>)> {
    let raw = fs::read_to_string(path)?;
    Ok(parse_md(&raw, ""))
}

pub fn write_card_content(path: &Path, title: &str, body: &str, priority: Priority, assignee: &str, project: &str) -> io::Result<()> {
    // Preserve any unknown frontmatter fields from the existing file
    let extra_fields = if path.exists() {
        match fs::read_to_string(path) {
            Ok(raw) => parse_extra_frontmatter(&raw),
            Err(_) => vec![],
        }
    } else {
        vec![]
    };

    let mut content = String::from("---\n");
    content.push_str(&format!("priority: {}\n", priority.label()));
    if !assignee.is_empty() {
        content.push_str(&format!("assignee: {assignee}\n"));
    }
    if !project.is_empty() {
        content.push_str(&format!("project: {project}\n"));
    }
    // Re-write any extra fields that are not the known ones
    for (key, val) in &extra_fields {
        match key.as_str() {
            "priority" | "assignee" | "project" | "updated_at" => continue,
            _ => content.push_str(&format!("{key}: {val}\n")),
        }
    }
    content.push_str(&format!("updated_at: {}\n", now_millis()));
    content.push_str(&format!("---\n# {title}\n"));
    if !body.is_empty() {
        content.push('\n');
        content.push_str(body);
        if !body.ends_with('\n') {
            content.push('\n');
        }
    }
    fs::write(path, content)
}

/// Extract all key-value pairs from the frontmatter of a markdown file.
/// Used to preserve unknown/custom fields when rewriting a card.
fn parse_extra_frontmatter(raw: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if !raw.starts_with("---\n") && !raw.starts_with("---\r\n") {
        return fields;
    }
    let after_open = if raw.starts_with("---\r\n") { 5 } else { 4 };
    // Support both LF and CRLF for the closing delimiter
    let close_pos = raw[after_open..]
        .find("\n---")
        .or_else(|| raw[after_open..].find("\r\n---"));
    let Some(close_pos) = close_pos else {
        return fields;
    };
    let frontmatter = &raw[after_open..after_open + close_pos];
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_string();
            let val = line[pos + 1..].trim().to_string();
            if !key.is_empty() {
                fields.push((key, val));
            }
        }
    }
    fields
}

fn parse_md(raw: &str, fallback: &str) -> (String, String, Priority, String, String, Option<i64>) {
    let mut priority = Priority::Medium;
    let mut assignee = String::new();
    let mut project = String::new();
    let mut updated_at: Option<i64> = None;
    let content;

    // Check for frontmatter
    if raw.starts_with("---\n") || raw.starts_with("---\r\n") {
        // Find closing --- (support both LF and CRLF)
        let after_open = if raw.starts_with("---\r\n") { 5 } else { 4 };
        let close_pos = raw[after_open..]
            .find("\n---")
            .or_else(|| raw[after_open..].find("\r\n---"));
        if let Some(close_pos) = close_pos {
            // Determine if this was a CRLF close
            let is_crlf_close = raw[after_open + close_pos..].starts_with("\r\n---");
            let close_len = if is_crlf_close { 5 } else { 4 }; // "\r\n---" or "\n---"
            let frontmatter = &raw[after_open..after_open + close_pos];
            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("priority:") {
                    priority = Priority::from_str(val);
                } else if let Some(val) = line.strip_prefix("assignee:") {
                    assignee = val.trim().to_string();
                } else if let Some(val) = line.strip_prefix("project:") {
                    project = val.trim().to_string();
                } else if let Some(val) = line.strip_prefix("updated_at:") {
                    updated_at = val.trim().parse::<i64>().ok();
                }
            }
            // Content starts after closing --- and its newline
            let body_start = after_open + close_pos + close_len;
            content = if body_start < raw.len() {
                // Skip the newline after ---
                let rest = &raw[body_start..];
                if rest.starts_with('\n') {
                    &rest[1..]
                } else if rest.starts_with("\r\n") {
                    &rest[2..]
                } else {
                    rest
                }
            } else {
                ""
            };
        } else {
            content = raw;
        }
    } else {
        content = raw;
    }

    let mut lines = content.lines();
    let first = lines.next().unwrap_or("");
    let title = first.strip_prefix("# ").unwrap_or(first).trim();
    let title = if title.is_empty() { fallback } else { title };

    let rest = content[first.len()..].trim().to_string();
    (title.to_string(), rest, priority, assignee, project, updated_at)
}

pub fn move_card(root: &Path, card_id: &str, to_col_id: &str) -> io::Result<()> {
    let col_ids = list_columns(root)?;
    let src = find_card_column(root, &col_ids, card_id)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "card not found"))?;

    if src == to_col_id {
        return Ok(());
    }

    let src_dir = root.join("cols").join(&src);
    let dst_dir = root.join("cols").join(to_col_id);
    fs::create_dir_all(&dst_dir)?;

    fs::rename(
        src_dir.join(format!("{card_id}.md")),
        dst_dir.join(format!("{card_id}.md")),
    )?;

    order_remove(&src_dir.join("order.txt"), card_id)?;
    order_append(&dst_dir.join("order.txt"), card_id)?;

    Ok(())
}

pub fn create_card(root: &Path, to_col_id: &str, project: &str) -> io::Result<String> {
    let prefix = if project.is_empty() { "CARD".to_string() } else { project.to_uppercase() };
    let id = format!("{}-{}", prefix, now_millis());
    let dir = root.join("cols").join(to_col_id);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{id}.md"));
    write_card_content(&path, "New card", "", Priority::Medium, "", project)?;
    order_append(&dir.join("order.txt"), &id)?;
    Ok(id)
}

pub fn delete_card(root: &Path, card_id: &str) -> io::Result<()> {
    let col_ids = list_columns(root)?;
    let col_id = find_card_column(root, &col_ids, card_id)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "card not found"))?;

    let dir = root.join("cols").join(&col_id);
    fs::remove_file(dir.join(format!("{card_id}.md")))?;
    order_remove(&dir.join("order.txt"), card_id)?;

    Ok(())
}

pub fn card_path(root: &Path, card_id: &str) -> io::Result<PathBuf> {
    let col_ids = list_columns(root)?;
    let src = find_card_column(root, &col_ids, card_id)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "card not found"))?;
    Ok(root.join("cols").join(src).join(format!("{card_id}.md")))
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn list_columns(root: &Path) -> io::Result<Vec<String>> {
    let txt = fs::read_to_string(root.join("board.txt"))?;
    Ok(txt
        .lines()
        .filter_map(|l| l.trim().strip_prefix("col "))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(|s| s.to_string())
        .collect())
}

fn find_card_column(root: &Path, cols: &[String], card_id: &str) -> io::Result<Option<String>> {
    for c in cols {
        if root
            .join("cols")
            .join(c)
            .join(format!("{card_id}.md"))
            .exists()
        {
            return Ok(Some(c.clone()));
        }
    }
    Ok(None)
}

fn order_remove(path: &Path, id: &str) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let cur = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for l in cur.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if l != id {
            out.push(l);
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    fs::write(path, s)
}

fn order_append(path: &Path, id: &str) -> io::Result<()> {
    let mut lines = if path.exists() {
        fs::read_to_string(path)?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    if !lines.iter().any(|x| x == id) {
        lines.push(id.to_string());
    }

    let mut s = lines.join("\n");
    s.push('\n');
    let parent = path.parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    fs::create_dir_all(parent)?;
    fs::write(path, s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn tmp_root() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("flow-test-{n}"))
    }

    fn write(p: &Path, s: &str) -> io::Result<()> {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(p, s)
    }

    #[test]
    fn load_and_move_persists() -> TestResult {
        let root = tmp_root();
        fs::create_dir_all(root.join("cols"))?;

        write(
            &root.join("board.txt"),
            "col todo \"TO DO\"\ncol done \"DONE\"\n",
        )?;
        write(&root.join("cols/todo/order.txt"), "A-1\n")?;
        write(&root.join("cols/todo/A-1.md"), "# Title\n\nBody\n")?;
        write(&root.join("cols/done/order.txt"), "")?;

        let b = load_board(&root)?;
        assert_eq!(b.columns[0].cards.len(), 1);
        assert_eq!(b.columns[0].cards[0].priority, Priority::Medium);

        move_card(&root, "A-1", "done")?;

        let b2 = load_board(&root)?;
        assert_eq!(b2.columns[1].cards.len(), 1);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn create_card_persists_file_and_order() -> TestResult {
        let root = tmp_root();
        write(&root.join("board.txt"), "col todo\n")?;

        let id = create_card(&root, "todo", "TestProj")?;
        assert!(id.starts_with("TESTPROJ-"));
        assert!(
            root.join("cols")
                .join("todo")
                .join(format!("{id}.md"))
                .exists()
        );

        let order = fs::read_to_string(root.join("cols/todo/order.txt"))?;
        assert!(order.lines().any(|l| l == id));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn delete_card_removes_file_and_order() -> TestResult {
        let root = tmp_root();
        write(&root.join("board.txt"), "col todo\n")?;
        let id = create_card(&root, "todo", "TestProj")?;

        delete_card(&root, &id)?;

        assert!(
            !root.join("cols")
                .join("todo")
                .join(format!("{id}.md"))
                .exists()
        );

        let order = fs::read_to_string(root.join("cols/todo/order.txt"))?;
        assert!(!order.lines().any(|l| l == id));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn write_and_read_card_content_roundtrips() -> TestResult {
        let root = tmp_root();
        fs::create_dir_all(&root)?;
        let path = root.join("CARD.md");

        write_card_content(&path, "My Title", "Body text", Priority::High, "user@test.com", "ProjectX")?;

        let (title, body, priority, assignee, project, updated_at) = read_card_content(&path)?;
        assert_eq!(title, "My Title");
        assert_eq!(body, "Body text");
        assert_eq!(priority, Priority::High);
        assert_eq!(assignee, "user@test.com");
        assert_eq!(project, "ProjectX");
        assert!(updated_at.is_some(), "updated_at should be set");

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn write_card_content_empty_body() -> TestResult {
        let root = tmp_root();
        fs::create_dir_all(&root)?;
        let path = root.join("CARD.md");

        write_card_content(&path, "Title Only", "", Priority::Low, "", "")?;

        let (title, body, priority, assignee, project, _) = read_card_content(&path)?;
        assert_eq!(title, "Title Only");
        assert!(body.is_empty());
        assert_eq!(priority, Priority::Low);
        assert!(assignee.is_empty());
        assert!(project.is_empty());

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn write_card_content_preserves_multiline_body() -> TestResult {
        let root = tmp_root();
        fs::create_dir_all(&root)?;
        let path = root.join("CARD.md");

        write_card_content(&path, "Title", "Line 1\nLine 2\nLine 3", Priority::Bug, "", "")?;

        let (title, body, priority, _, _, _) = read_card_content(&path)?;
        assert_eq!(title, "Title");
        assert!(body.contains("Line 1"));
        assert!(body.contains("Line 3"));
        assert_eq!(priority, Priority::Bug);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn parse_md_without_frontmatter_defaults_to_medium() {
        let (title, body, priority, assignee, project, updated_at) =
            parse_md("# Hello\n\nWorld", "fallback");
        assert_eq!(title, "Hello");
        assert_eq!(body, "World");
        assert_eq!(priority, Priority::Medium);
        assert!(assignee.is_empty());
        assert!(project.is_empty());
        assert!(updated_at.is_none());
    }

    #[test]
    fn parse_md_with_frontmatter() {
        let raw = "---\npriority: HIGH\nassignee: dev@test.com\nproject: MyProject\n---\n# My Card\n\nDescription";
        let (title, body, priority, assignee, project, updated_at) = parse_md(raw, "fallback");
        assert_eq!(title, "My Card");
        assert_eq!(body, "Description");
        assert_eq!(priority, Priority::High);
        assert_eq!(assignee, "dev@test.com");
        assert_eq!(project, "MyProject");
        assert!(updated_at.is_none());
    }

    #[test]
    fn load_board_sorts_cards_by_priority_then_title() -> TestResult {
        let root = tmp_root();
        fs::create_dir_all(root.join("cols"))?;

        write(
            &root.join("board.txt"),
            "col todo \"TO DO\"\n",
        )?;
        write(&root.join("cols/todo/order.txt"), "A\nB\nC\nD\nE\n")?;
        write_card_content(&root.join("cols/todo/A.md"), "Zebra", "", Priority::Low, "", "")?;
        write_card_content(&root.join("cols/todo/B.md"), "Alpha", "", Priority::High, "", "")?;
        write_card_content(&root.join("cols/todo/C.md"), "Beta", "", Priority::High, "", "")?;
        write_card_content(&root.join("cols/todo/D.md"), "Crash", "", Priority::Bug, "", "")?;
        write_card_content(&root.join("cols/todo/E.md"), "Nice to have", "", Priority::Wishlist, "", "")?;

        let b = load_board(&root)?;
        let titles: Vec<&str> = b.columns[0].cards.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["Crash", "Alpha", "Beta", "Zebra", "Nice to have"]);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn write_card_content_preserves_extra_frontmatter_fields() -> TestResult {
        let root = tmp_root();
        fs::create_dir_all(&root)?;
        let path = root.join("CARD.md");

        // Create initial file with extra fields
        fs::write(
            &path,
            "---\npriority: MEDIUM\ntags: rust,tui\ncustom_field: value42\ndue_date: 2026-07-15\n---\n# Original\n\nBody",
        )?;

        // Rewrite with standard fields
        write_card_content(&path, "Updated", "New body", Priority::High, "dev@test.com", "Flow")?;

        let raw = fs::read_to_string(&path)?;
        // Extra fields should still be present
        assert!(raw.contains("tags: rust,tui"), "tags should be preserved");
        assert!(raw.contains("custom_field: value42"), "custom_field should be preserved");
        assert!(raw.contains("due_date: 2026-07-15"), "due_date should be preserved");
        // Standard fields should be there
        assert!(raw.contains("priority: HIGH"));
        assert!(raw.contains("project: Flow"));
        assert!(raw.contains("assignee: dev@test.com"));
        assert!(raw.contains("updated_at:"));
        assert!(raw.contains("# Updated"));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn parse_md_crlf_frontmatter() {
        // Windows-style line endings in frontmatter
        let raw = "---\r\npriority: HIGH\r\nproject: Test\r\n---\r\n# CRLF Card\r\n\r\nBody text";
        let (title, body, priority, _assignee, project, _updated_at) = parse_md(raw, "fallback");
        assert_eq!(title, "CRLF Card");
        assert_eq!(body, "Body text");
        assert_eq!(priority, Priority::High);
        assert_eq!(project, "Test");
    }

    #[test]
    fn write_card_content_does_not_lose_extra_fields_when_file_absent() -> TestResult {
        // Verifies that write_card_content works on new (non-existent) files
        let root = tmp_root();
        fs::create_dir_all(&root)?;
        let path = root.join("fresh.md");

        write_card_content(&path, "Fresh", "Content", Priority::Low, "", "")?;

        let (title, body, priority, _, _, _) = read_card_content(&path)?;
        assert_eq!(title, "Fresh");
        assert_eq!(body, "Content");
        assert_eq!(priority, Priority::Low);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn frontmatter_roundtrip_all_priorities() -> TestResult {
        let root = tmp_root();
        fs::create_dir_all(&root)?;
        let path = root.join("CARD.md");

        for p in [Priority::Low, Priority::Medium, Priority::High, Priority::Bug, Priority::Wishlist] {
            write_card_content(&path, "Test", "Body", p, "", "")?;
            let (_, _, got, _, _, _) = read_card_content(&path)?;
            assert_eq!(got, p, "roundtrip failed for {:?}", p);
        }

        fs::remove_dir_all(root)?;
        Ok(())
    }
}
