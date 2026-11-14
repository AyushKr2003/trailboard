//! Note cards: Markdown text kept in the board instead of a web page.
//!
//! The text is the note; its title is derived from it. "Add to note" quotes
//! selected page text with a link back to where it came from.

use crate::{Edge, Node, NodeId, Rect, Scene, placement};

/// The `url` every note card carries (its page is built by the app).
pub const NOTE_URL: &str = "trailboard:note";

/// Title shown for a note without any text.
pub const UNTITLED: &str = "Note";

/// Longest title taken from a note's first line, in characters.
const TITLE_CHARS: usize = 80;

/// The note's title: its first line with text, without Markdown markers
/// (heading `#`s, quote `>`, list bullets and task boxes, emphasis), cut at
/// [`TITLE_CHARS`]. [`UNTITLED`] when nothing is left.
pub fn title(text: &str) -> String {
    let mut in_fence = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let t = strip_markers(t);
        if t.is_empty() || t.chars().all(|c| matches!(c, '-' | '*' | '_' | '=' | ' ')) {
            continue;
        }
        return cut(&strip_inline(t), TITLE_CHARS);
    }
    UNTITLED.to_string()
}

/// Leading block markers: `#`, `>`, `-`/`*`/`+`/`1.` bullets, `[ ]`/`[x]`.
fn strip_markers(mut t: &str) -> &str {
    loop {
        let before = t;
        t = t.trim_start();
        if let Some(rest) = t.strip_prefix('>') {
            t = rest;
        } else if t.starts_with('#') {
            let rest = t.trim_start_matches('#');
            if rest.is_empty() || rest.starts_with(' ') {
                t = rest;
            }
        } else if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) {
            t = rest;
        } else if let Some(rest) = ordered_bullet(t) {
            t = rest;
        } else if let Some(rest) = ["[ ] ", "[x] ", "[X] "].iter().find_map(|b| t.strip_prefix(b)) {
            t = rest;
        }
        if t == before {
            return t.trim();
        }
    }
}

fn ordered_bullet(t: &str) -> Option<&str> {
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    t[digits..].strip_prefix(". ").or_else(|| t[digits..].strip_prefix(") "))
}

/// Inline Markdown reduced to its text: emphasis and code marks dropped,
/// `[text](url)` kept as `text`.
fn strip_inline(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut rest = t;
    while let Some(c) = rest.chars().next() {
        if c == '[' {
            // [text](url) → text
            if let Some(close) = rest.find("](")
                && let Some(end) = rest[close + 2..].find(')')
            {
                out.push_str(&strip_inline(&rest[1..close]));
                rest = &rest[close + 2 + end + 1..];
                continue;
            }
        }
        if !matches!(c, '*' | '_' | '`' | '~') {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let c: String = s.chars().take(max - 1).collect();
    format!("{}…", c.trim_end())
}

/// Selected page text as a Markdown quote with a link back to its page:
///
/// ```text
/// > first line
/// > second line
///
/// — [Page title](https://…)
/// ```
///
/// Blank lines inside the selection stay quoted (`>`), so it is one quote.
/// Returns `None` when the selection has no text.
pub fn quote(selection: &str, source_title: &str, source_url: &str) -> Option<String> {
    let lines: Vec<&str> = selection.lines().map(str::trim_end).collect();
    let first = lines.iter().position(|l| !l.trim().is_empty())?;
    let last = lines.iter().rposition(|l| !l.trim().is_empty())?;
    let mut out = String::new();
    for l in &lines[first..=last] {
        if l.trim().is_empty() {
            out.push_str(">\n");
        } else {
            out.push_str("> ");
            out.push_str(l.trim_start());
            out.push('\n');
        }
    }
    let name = source_title.trim();
    let name = if name.is_empty() { source_url } else { name };
    // Brackets in the title would end the link text early.
    let name: String = name.chars().map(|c| if matches!(c, '[' | ']') { ' ' } else { c }).collect();
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if source_url.is_empty() {
        out.push_str(&format!("\n— {name}\n"));
    } else {
        out.push_str(&format!("\n— [{name}](<{source_url}>)\n"));
    }
    Some(out)
}

/// `text` with `block` added at the end, a blank line between them.
pub fn append(text: &str, block: &str) -> String {
    let head = text.trim_end();
    if head.is_empty() {
        return block.to_string();
    }
    format!("{head}\n\n{block}")
}

impl Node {
    pub fn is_note(&self) -> bool {
        self.note.is_some()
    }
}

impl Scene {
    /// Adds a note card holding `text` at `rect`.
    pub fn add_note(&mut self, rect: Rect, text: impl Into<String>) -> NodeId {
        let text = text.into();
        let id = self.add_page(rect, NOTE_URL);
        let n = self.nodes.get_mut(&id).expect("just added");
        n.title = title(&text);
        n.note = Some(text);
        id
    }

    /// Adds a note beside card `from`, joined to it by a trail edge, sized `w`×`h`.
    pub fn note_beside(&mut self, from: NodeId, text: impl Into<String>, w: f32, h: f32) -> Option<NodeId> {
        let src = self.nodes.get(&from)?.clone();
        let rect = placement::beside_sized(self, &src, w, h);
        let id = self.add_note(rect, text);
        self.nodes.get_mut(&id).expect("just added").trail = src.trail;
        self.edges.push(Edge { from, to: id });
        Some(id)
    }

    /// Replaces a note's text (and so its title). False if `id` is not a note.
    pub fn set_note(&mut self, id: NodeId, text: impl Into<String>) -> bool {
        let Some(n) = self.nodes.get_mut(&id).filter(|n| n.is_note()) else { return false };
        let text = text.into();
        n.title = title(&text);
        n.note = Some(text);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_takes_the_first_line_with_text() {
        assert_eq!(title(""), UNTITLED);
        assert_eq!(title("\n  \n---\n"), UNTITLED);
        assert_eq!(title("# Trip *ideas*\nmore"), "Trip ideas");
        assert_eq!(title("\n\n> - [x] **done** thing"), "done thing");
        assert_eq!(title("12. step [one](https://x.example) `code`"), "step one code");
        assert_eq!(title("#hashtag first"), "#hashtag first");
        assert_eq!(title("```\nlet x = 1;\n```\nAfter code"), "After code");
        let long = "word ".repeat(40);
        let t = title(&long);
        assert_eq!(t.chars().count(), TITLE_CHARS);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn quote_marks_every_line_and_links_back() {
        let q = quote("\n  first line  \n\nsecond\n\n", "A [nice] page", "https://a.example/x").unwrap();
        assert_eq!(q, "> first line\n>\n> second\n\n— [A nice page](<https://a.example/x>)\n");
        assert_eq!(quote(" \n\t", "t", "u"), None);
        let untitled = quote("x", "  ", "https://u.example").unwrap();
        assert!(untitled.ends_with("— [https://u.example](<https://u.example>)\n"));
    }

    #[test]
    fn append_puts_a_blank_line_between() {
        assert_eq!(append("", "> q\n"), "> q\n");
        assert_eq!(append("  \n", "> q\n"), "> q\n");
        assert_eq!(append("# Notes\n\n\n", "> q\n"), "# Notes\n\n> q\n");
    }

    #[test]
    fn notes_are_cards_with_text() {
        let mut s = Scene::new();
        let page = s.add_page(Rect::new(0.0, 0.0, 800.0, 600.0), "https://a");
        let n = s.add_note(Rect::new(0.0, 1000.0, 400.0, 300.0), "# Hello");
        assert!(s.node(n).unwrap().is_note());
        assert!(!s.node(page).unwrap().is_note());
        assert_eq!(s.node(n).unwrap().url, NOTE_URL);
        assert_eq!(s.node(n).unwrap().title, "Hello");
        assert!(s.set_note(n, "Renamed"));
        assert_eq!(s.node(n).unwrap().title, "Renamed");
        assert!(!s.set_note(page, "nope"));
        assert_eq!(s.node(page).unwrap().note, None);

        let b = s.note_beside(page, "> quote", 400.0, 300.0).unwrap();
        assert!(s.edges().contains(&Edge { from: page, to: b }));
        let (fp, fb) = (s.node(page).unwrap().frame(), s.node(b).unwrap().frame());
        assert!(fb.x > fp.right() && !fb.intersects(&fp));
        assert_eq!((s.node(b).unwrap().rect.w, s.node(b).unwrap().rect.h), (400.0, 300.0));
    }
}
