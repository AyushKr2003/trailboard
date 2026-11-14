//! Save as Markdown: a note as its own text, or a whole board as an outline of
//! its trails. Our own format:
//!
//! ```text
//! # Space name
//!
//! ## Trail name (or its first card's title)
//!
//! - [Page](<https://…>)
//!   - [Page opened from it](<https://…>)
//!   - Note: Title
//!
//!     The note's text, indented under its item.
//!
//! ## A note on its own
//!
//! Its text, headings one level down.
//!
//! ## Other pages
//!
//! - [Page on no trail](<https://…>)
//! ```
//!
//! Trails come in the order their first card was made; folded cards are
//! included in their place. Headings inside notes move down a level so the
//! space stays the only `#`.

use crate::{Node, Scene};
use std::collections::BTreeSet;

/// Heading of the section that lists pages on no trail.
pub const OTHER_PAGES: &str = "Other pages";

/// A note's text as a Markdown file: as written, ending in one newline.
pub fn note_markdown(text: &str) -> String {
    let body = text.trim_end();
    if body.is_empty() { String::new() } else { format!("{body}\n") }
}

/// The whole board of the space `space` as Markdown (see the module docs).
pub fn board_markdown(scene: &Scene, space: &str) -> String {
    let mut out = format!("# {}\n", one_line(space, "Board"));
    let mut seen = BTreeSet::new();
    let mut lone_pages = Vec::new();
    for n in scene.all_nodes() {
        if seen.contains(&n.id) {
            continue;
        }
        let tree = scene.trail_tree(n.id);
        seen.extend(tree.iter().map(|&(id, _)| id));
        let mark = scene.trail_mark(n.id);
        if tree.len() == 1 && mark.name.is_none() {
            if n.is_note() {
                out.push('\n');
                out.push_str(&lone_note(n));
            } else {
                lone_pages.push(n.id);
            }
            continue;
        }
        let head = tree.first().and_then(|&(id, _)| scene.node(id)).map(|n| n.title.as_str()).unwrap_or("");
        let heading = mark.name.as_deref().unwrap_or(head);
        out.push_str(&format!("\n## {}\n\n", one_line(heading, "Trail")));
        for (id, depth) in tree {
            if let Some(n) = scene.node(id) {
                out.push_str(&item(n, depth));
            }
        }
    }
    if !lone_pages.is_empty() {
        out.push_str(&format!("\n## {OTHER_PAGES}\n\n"));
        for id in lone_pages {
            if let Some(n) = scene.node(id) {
                out.push_str(&item(n, 0));
            }
        }
    }
    out
}

/// A list item for card `n` at `depth`: a link for a page, the note's text
/// indented under a `Note: Title` line for a note.
fn item(n: &Node, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let Some(text) = n.note.as_deref() else {
        return format!("{indent}- {}\n", link(&n.title, &n.url));
    };
    let mut out = format!("{indent}- Note: {}\n", escape(&one_line(&n.title, crate::note::UNTITLED)));
    let body = demote(text.trim_end());
    if !body.trim().is_empty() {
        out.push('\n');
        let pad = "  ".repeat(depth + 1);
        for line in body.lines() {
            if line.trim().is_empty() {
                out.push('\n');
            } else {
                out.push_str(&format!("{pad}{line}\n"));
            }
        }
        out.push('\n');
    }
    out
}

/// A note on no trail: its text with headings moved down, under a `##`
/// heading of its title unless it starts with a heading of its own.
fn lone_note(n: &Node) -> String {
    let text = n.note.as_deref().unwrap_or("").trim();
    let starts_with_heading = text.lines().next().is_some_and(|l| heading_level(l).is_some());
    let mut out = String::new();
    if !starts_with_heading {
        out.push_str(&format!("## {}\n\n", one_line(&n.title, crate::note::UNTITLED)));
    }
    if !text.is_empty() {
        out.push_str(&demote(text));
        out.push('\n');
    }
    out
}

/// `[title](<url>)`, the URL as text when the title is empty.
pub fn link(title: &str, url: &str) -> String {
    let text = one_line(title, url);
    let text = if text.is_empty() { "Untitled".to_string() } else { text };
    if url.is_empty() {
        return escape(&text);
    }
    // `<`, `>` and line breaks would end the bracketed destination.
    let dest: String = url
        .chars()
        .map(|c| match c {
            '<' => "%3C".to_string(),
            '>' => "%3E".to_string(),
            '\n' | '\r' => String::new(),
            c => c.to_string(),
        })
        .collect();
    format!("[{}](<{dest}>)", escape(&text))
}

/// Backslashes and brackets escaped, so text can't end a link early.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `s` on one line, whitespace collapsed; `fallback` when that leaves nothing.
fn one_line(s: &str, fallback: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { fallback.to_string() } else { t }
}

/// ATX heading level of `line` (up to 3 spaces before the `#`s).
fn heading_level(line: &str) -> Option<usize> {
    let lead = line.len() - line.trim_start_matches(' ').len();
    if lead > 3 {
        return None;
    }
    let rest = &line[lead..];
    let level = rest.len() - rest.trim_start_matches('#').len();
    let after = &rest[level..];
    ((1..=6).contains(&level) && (after.is_empty() || after.starts_with([' ', '\t']))).then_some(level)
}

/// Every heading outside fenced code one level down (`######` stays).
fn demote(text: &str) -> String {
    let mut fence: Option<&str> = None;
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| t.starts_with(m));
        match (fence, marker) {
            (None, Some(m)) => fence = Some(m),
            (Some(f), Some(m)) if f == m => fence = None,
            _ => {}
        }
        let in_code = fence.is_some() || marker.is_some();
        match heading_level(line) {
            Some(l) if !in_code && l < 6 => out.push(line.replacen('#', "##", 1)),
            _ => out.push(line.to_string()),
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    #[test]
    fn a_note_saves_as_written() {
        assert_eq!(note_markdown("# Hi\n\n- a\n\n\n"), "# Hi\n\n- a\n");
        assert_eq!(note_markdown("  \n"), "");
    }

    #[test]
    fn links_escape_their_text_and_url() {
        assert_eq!(link("A [b] c\\", "https://x.example/a b"), r"[A \[b\] c\\](<https://x.example/a b>)");
        assert_eq!(link("  ", "https://u.example"), "[https://u.example](<https://u.example>)");
        assert_eq!(link("t", "https://x/<y>"), "[t](<https://x/%3Cy%3E>)");
        assert_eq!(link("", ""), "Untitled");
    }

    #[test]
    fn headings_move_down_outside_code() {
        let t = "# One\n## Two\n```\n# code\n```\n#tag\n    # indented code\n###### Six";
        assert_eq!(demote(t), "## One\n### Two\n```\n# code\n```\n#tag\n    # indented code\n###### Six");
    }

    #[test]
    fn trails_become_nested_lists() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "https://a.example");
        s.node_mut(a).unwrap().title = "Alpha".into();
        let b = s.spawn_beside(a, "https://b.example").unwrap();
        s.node_mut(b).unwrap().title = "Beta".into();
        s.note_beside(b, "# Thoughts\n\nsome *text*\n\n```\n# not a heading\n```", 400.0, 300.0).unwrap();
        s.spawn_beside(a, "https://c.example").unwrap();
        s.add_note(Rect::new(0.0, 3000.0, 400.0, 300.0), "plain first line\n\n# Later");
        s.add_note(Rect::new(0.0, 4000.0, 400.0, 300.0), "# Own heading\nbody");
        s.add_page(Rect::new(0.0, 5000.0, 400.0, 300.0), "https://lone.example");
        s.fold(b).unwrap(); // folded cards are still listed, in place
        let md = board_markdown(&s, "My  space");
        let want = "\
# My space

## Alpha

- [Alpha](<https://a.example>)
  - [Beta](<https://b.example>)
    - Note: Thoughts

      ## Thoughts

      some *text*

      ```
      # not a heading
      ```

  - [https://c.example](<https://c.example>)

## plain first line

plain first line

## Later

## Own heading
body

## Other pages

- [https://lone.example](<https://lone.example>)
";
        assert_eq!(md, want);
    }

    #[test]
    fn a_named_trail_uses_its_name_and_a_named_lone_card_gets_a_section() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "https://a.example");
        s.spawn_beside(a, "https://b.example").unwrap();
        s.name_trail(a, "Research");
        let lone = s.add_page(Rect::new(0.0, 3000.0, 400.0, 300.0), "https://l.example");
        s.name_trail(lone, "Later");
        let md = board_markdown(&s, "");
        assert!(md.starts_with("# Board\n\n## Research\n\n- [https://a.example]"), "{md}");
        assert!(md.contains("\n## Later\n\n- [https://l.example](<https://l.example>)\n"), "{md}");
        assert!(!md.contains(OTHER_PAGES));
    }

    #[test]
    fn an_empty_board_is_just_its_title() {
        assert_eq!(board_markdown(&Scene::new(), "Space 1"), "# Space 1\n");
    }
}
