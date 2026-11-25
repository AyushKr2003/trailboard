//! Note cards: Markdown text kept in the board instead of a web page.
//!
//! The text is the note; its title is derived from it. "Add to note" quotes
//! selected page text with a link back to where it came from.

use crate::{Edge, Node, NodeId, Rect, Scene, Vec2, placement};

/// The `url` every note card carries (its page is built by the app).
pub const NOTE_URL: &str = "trailboard:note";

/// The `url` of the welcome page card (the app builds its page).
pub const WELCOME_URL: &str = "trailboard:welcome";
/// The welcome page card's title (its page never reports one).
pub const WELCOME_TITLE: &str = "Welcome to Trailboard";

/// The `url` of a start page card: a search box until something is typed
/// in it, then an ordinary page (the app builds its page).
pub const START_URL: &str = "trailboard:start";
/// The start page card's title.
pub const START_TITLE: &str = "New page";

/// The `url` of the search engine guide card (the app builds its page).
pub const ENGINES_URL: &str = "trailboard:engines";
/// The search engine guide's title.
pub const ENGINES_TITLE: &str = "Search engines";

/// The `url` of the settings page card (the app builds its page).
pub const SETTINGS_URL: &str = "trailboard:settings";
/// The settings page's title.
pub const SETTINGS_TITLE: &str = "Settings";

/// The `url` of a page inspector card: Chromium's inspector for the card it
/// is joined to. It lives only while both run, and is never saved.
pub const INSPECTOR_URL: &str = "trailboard:inspector";
/// An inspector card's title.
pub const INSPECTOR_TITLE: &str = "Inspector";

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

/// Colours a sticky note takes in turn with C, by desktop-theme name; new
/// ones start on the first.
pub const STICKY_COLOURS: [&str; 6] = ["yellow", "green", "cyan", "blue", "magenta", "red"];

/// The sticky colour after `colour`, wrapping round; the first for one not in
/// [`STICKY_COLOURS`] (a hand-edited file).
pub fn next_sticky_colour(colour: &str) -> &'static str {
    let i = STICKY_COLOURS.iter().position(|c| *c == colour).map_or(0, |i| i + 1);
    STICKY_COLOURS[i % STICKY_COLOURS.len()]
}

impl Node {
    pub fn is_note(&self) -> bool {
        self.note.is_some()
    }

    /// A note shown as a sticky note: no title bar, the paper in its colour.
    pub fn is_sticky(&self) -> bool {
        self.is_note() && self.sticky.is_some()
    }

    /// A page inspector card ([`INSPECTOR_URL`]).
    pub fn is_inspector(&self) -> bool {
        !self.is_note() && self.url == INSPECTOR_URL
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

    /// The first note (folded away or not) whose text is exactly `text`: an
    /// untouched copy of a note the app wrote, such as the welcome note.
    /// The welcome page card, if the board has one (folded away or not).
    pub fn welcome_card(&self) -> Option<NodeId> {
        self.all_nodes().find(|n| n.note.is_none() && n.url == WELCOME_URL).map(|n| n.id)
    }

    /// Adds the welcome page as a card at `rect`.
    pub fn add_welcome(&mut self, rect: Rect) -> NodeId {
        let id = self.add_page(rect, WELCOME_URL);
        self.nodes.get_mut(&id).expect("just added").title = WELCOME_TITLE.to_string();
        id
    }

    /// The search engine guide card, if the board has one.
    pub fn engines_card(&self) -> Option<NodeId> {
        self.all_nodes().find(|n| n.note.is_none() && n.url == ENGINES_URL).map(|n| n.id)
    }

    /// The settings page card, if the board has one.
    pub fn settings_card(&self) -> Option<NodeId> {
        self.all_nodes().find(|n| n.note.is_none() && n.url == SETTINGS_URL).map(|n| n.id)
    }

    /// Adds the settings page as a card at `rect`.
    pub fn add_settings(&mut self, rect: Rect) -> NodeId {
        let id = self.add_page(rect, SETTINGS_URL);
        self.nodes.get_mut(&id).expect("just added").title = SETTINGS_TITLE.to_string();
        id
    }

    /// Adds an inspector card beside card `from`, joined to it, sized `size`.
    pub fn add_inspector(&mut self, from: NodeId, size: Vec2) -> Option<NodeId> {
        let id = self.spawn_beside_sized(from, INSPECTOR_URL, size)?;
        self.nodes.get_mut(&id).expect("just added").title = INSPECTOR_TITLE.to_string();
        Some(id)
    }

    /// The board as it is saved: without inspector cards, which can't be restored.
    pub fn without_inspectors(&self) -> Scene {
        let mut out = self.clone();
        let gone: Vec<NodeId> = out.all_nodes().filter(|n| n.is_inspector()).map(|n| n.id).collect();
        for id in gone {
            out.remove(id);
        }
        out
    }

    /// Adds a start page as a card at `rect`.
    pub fn add_start(&mut self, rect: Rect) -> NodeId {
        let id = self.add_page(rect, START_URL);
        self.nodes.get_mut(&id).expect("just added").title = START_TITLE.to_string();
        id
    }

    pub fn note_with_text(&self, text: &str) -> Option<NodeId> {
        self.all_nodes().find(|n| n.note.as_deref() == Some(text)).map(|n| n.id)
    }

    /// Adds a note beside card `from`, joined to it by a trail edge, sized `w`×`h`.
    pub fn note_beside(&mut self, from: NodeId, text: impl Into<String>, w: f32, h: f32) -> Option<NodeId> {
        let src = self.nodes.get(&from)?.clone();
        let rect = placement::beside_sized(self, &src, w, h);
        let id = self.add_note(rect, text);
        self.nodes.get_mut(&id).expect("just added").trail = src.trail;
        self.edges.push(Edge { from, to: id });
        self.colour_new_trail(id);
        Some(id)
    }

    /// Adds a sticky note holding `text` at `rect`, in the first sticky colour.
    pub fn add_sticky(&mut self, rect: Rect, text: impl Into<String>) -> NodeId {
        let id = self.add_note(rect, text);
        self.nodes.get_mut(&id).expect("just added").sticky = Some(STICKY_COLOURS[0].to_string());
        id
    }

    /// Makes note `id` a sticky note in `colour`, or a note card again with
    /// `None`. Returns the old value (to undo), or `None` if `id` is not a
    /// note or nothing changed. The content rect stays put; only the strip
    /// above it changes height.
    pub fn set_sticky(&mut self, id: NodeId, colour: Option<String>) -> Option<Option<String>> {
        let n = self.nodes.get_mut(&id).filter(|n| n.is_note() && n.sticky != colour)?;
        Some(std::mem::replace(&mut n.sticky, colour))
    }

    /// Gives sticky note `id` its next colour. Returns the old one, as
    /// [`Scene::set_sticky`]; `None` for a card that is not a sticky note.
    pub fn cycle_sticky_colour(&mut self, id: NodeId) -> Option<Option<String>> {
        let now = self.nodes.get(&id).filter(|n| n.is_sticky())?.sticky.clone()?;
        self.set_sticky(id, Some(next_sticky_colour(&now).to_string()))
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
    use crate::{HEADER_H, STICKY_GRIP_H};

    #[test]
    fn inspectors_are_never_saved() {
        let mut s = Scene::new();
        let page = s.add_page(Rect::new(0.0, 0.0, 10.0, 10.0), "https://a/");
        let insp = s.add_inspector(page, Vec2::new(10.0, 10.0)).unwrap();
        let n = s.node(insp).unwrap();
        assert!(n.is_inspector());
        assert_eq!(n.title, INSPECTOR_TITLE);
        assert!(s.edges().iter().any(|e| e.from == page && e.to == insp));
        let saved = s.without_inspectors();
        assert!(saved.node(insp).is_none());
        assert!(saved.node(page).is_some());
        assert!(saved.edges().is_empty());
        // The live board keeps it.
        assert!(s.node(insp).is_some());
    }

    #[test]
    fn note_with_text_finds_only_untouched_copies() {
        let mut s = Scene::new();
        s.add_page(Rect::new(0.0, 0.0, 10.0, 10.0), "https://a/");
        let edited = s.add_note(Rect::new(100.0, 0.0, 10.0, 10.0), "# Hi\n\nmine");
        assert_eq!(s.note_with_text("# Hi\n"), None);
        let copy = s.add_note(Rect::new(200.0, 0.0, 10.0, 10.0), "# Hi\n");
        s.add_note(Rect::new(300.0, 0.0, 10.0, 10.0), "# Hi\n");
        assert_eq!(s.note_with_text("# Hi\n"), Some(copy), "the first copy");
        assert_ne!(Some(edited), s.note_with_text("# Hi\n"));
    }

    #[test]
    fn sticky_notes_have_a_strip_not_a_title_bar() {
        let mut s = Scene::new();
        let id = s.add_sticky(Rect::new(0.0, 100.0, 240.0, 200.0), "buy milk");
        let n = s.node(id).unwrap();
        assert!(n.is_note() && n.is_sticky());
        assert_eq!(n.sticky.as_deref(), Some("yellow"));
        assert_eq!(n.title, "buy milk");
        assert_eq!(n.frame(), Rect::new(0.0, 100.0 - STICKY_GRIP_H, 240.0, 200.0 + STICKY_GRIP_H));
        // Above the strip is board, where a title bar would have been.
        assert_eq!(s.topmost_at(Vec2::new(10.0, 100.0 - STICKY_GRIP_H - 1.0)), None);
        assert_eq!(s.topmost_at(Vec2::new(10.0, 100.0 - 1.0)), Some(id));
    }

    #[test]
    fn notes_turn_sticky_and_back_and_pages_never_do() {
        let mut s = Scene::new();
        let note = s.add_note(Rect::new(0.0, 0.0, 100.0, 100.0), "");
        let page = s.add_page(Rect::new(300.0, 0.0, 100.0, 100.0), "https://a");
        assert_eq!(s.set_sticky(page, Some("green".into())), None);
        assert!(!s.node(page).unwrap().is_sticky());
        assert_eq!(s.set_sticky(note, Some("green".into())), Some(None));
        assert_eq!(s.node(note).unwrap().header_h(), STICKY_GRIP_H);
        assert_eq!(s.set_sticky(note, Some("green".into())), None, "no change");
        assert_eq!(s.set_sticky(note, None), Some(Some("green".into())));
        assert_eq!(s.node(note).unwrap().header_h(), HEADER_H);
        assert_eq!(s.cycle_sticky_colour(note), None, "a plain note has no sticky colour");
    }

    #[test]
    fn sticky_colours_cycle_and_wrap() {
        let mut s = Scene::new();
        let id = s.add_sticky(Rect::default(), "");
        let mut seen = vec![s.node(id).unwrap().sticky.clone().unwrap()];
        for _ in 0..STICKY_COLOURS.len() {
            assert!(s.cycle_sticky_colour(id).is_some());
            seen.push(s.node(id).unwrap().sticky.clone().unwrap());
        }
        assert_eq!(seen[..STICKY_COLOURS.len()], STICKY_COLOURS.map(String::from));
        assert_eq!(seen.last().unwrap(), STICKY_COLOURS[0], "wraps to the first");
        assert_eq!(next_sticky_colour("plaid"), STICKY_COLOURS[0]);
    }

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
    fn the_welcome_page_is_a_card_found_by_its_address() {
        let mut s = Scene::new();
        assert_eq!(s.welcome_card(), None);
        s.add_page(Rect::new(0.0, 0.0, 800.0, 600.0), "https://a");
        let w = s.add_welcome(Rect::new(0.0, 1000.0, crate::CARD_W, crate::CARD_H));
        assert_eq!(s.welcome_card(), Some(w));
        let n = s.node(w).unwrap();
        assert!(!n.is_note());
        assert_eq!((n.url.as_str(), n.title.as_str()), (WELCOME_URL, WELCOME_TITLE));
    }

    #[test]
    fn the_settings_page_is_a_card_found_by_its_address() {
        let mut s = Scene::new();
        assert_eq!(s.settings_card(), None);
        let id = s.add_settings(Rect::new(0.0, 0.0, crate::CARD_W, crate::CARD_H));
        assert_eq!(s.settings_card(), Some(id));
        let n = s.node(id).unwrap();
        assert!(!n.is_note());
        assert_eq!((n.url.as_str(), n.title.as_str()), (SETTINGS_URL, SETTINGS_TITLE));
    }

    #[test]
    fn a_start_page_is_a_page_card_with_its_own_address() {
        let mut s = Scene::new();
        let id = s.add_start(Rect::new(0.0, 0.0, 800.0, 600.0));
        let n = s.node(id).unwrap();
        assert!(!n.is_note());
        assert_eq!((n.url.as_str(), n.title.as_str()), (START_URL, START_TITLE));
        assert_eq!(s.welcome_card(), None);
        assert_eq!(s.engines_card(), None);
        let g = s.spawn_beside(id, ENGINES_URL).unwrap();
        assert_eq!(s.engines_card(), Some(g));
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
