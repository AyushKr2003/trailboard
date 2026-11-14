//! The Ctrl+K palette: one box that finds a card on the board, another space,
//! or a command, by fuzzy search (`scene::palette`). It is the address bar's
//! text field (`omnibox::Target::Palette`) with a list of results under it.
//!
//! With nothing typed it lists the cards, most recently used first, then the
//! other spaces and the commands. Typing ranks them all together; a last row
//! opens what was typed as an address or a web search in a new card (first,
//! when it looks like an address).

use std::collections::HashMap;

use compositor::{Layer, Quad, Renderer, Shape, Text};
use scene::palette::{self, Candidate};
use scene::{NodeId, Rect, Scene, Vec2};
use storage::spaces::{SpaceId, SpaceIndex};

use crate::omnibox;
use crate::theme::{rgb, Theme};

/// Board actions the palette can run, each with the keys that also do it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    NewCard,
    NewNote,
    NewSticky,
    Tidy,
    Fit,
    Fold,
    Outline,
    NameTrail,
    ColourTrail,
    Sidebar,
    Read,
    LinkHints,
    Arrange,
    SaveMarkdown,
    Undo,
    Redo,
    Reopen,
    CloseCard,
    Reload,
    FindInPage,
    MarkCard,
    FillView,
    PageMenu,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    SelectAll,
    NewSpace,
    NextSpace,
    PreviousSpace,
    RenameSpace,
    NextTint,
    DeleteSpace,
    Shortcuts,
    Quit,
}

pub struct CommandInfo {
    pub cmd: Command,
    pub label: &'static str,
    pub keys: &'static str,
    /// Other words it is found by.
    pub words: &'static str,
}

const fn c(cmd: Command, label: &'static str, keys: &'static str, words: &'static str) -> CommandInfo {
    CommandInfo { cmd, label, keys, words }
}

pub const COMMANDS: &[CommandInfo] = &[
    c(Command::NewCard, "New card", "Ctrl+T", "open address tab page url"),
    c(Command::NewNote, "New note", "N", "markdown write text"),
    c(Command::NewSticky, "New sticky note", "Shift+N", "post-it memo"),
    c(Command::Tidy, "Tidy trails", "T", "arrange layout clean up"),
    c(Command::Fit, "Fit everything", "Shift+F", "zoom overview show all"),
    c(Command::Fold, "Fold or unfold trail", "S", "stack collapse"),
    c(Command::Outline, "List a stack's pages", "O", "outline stack"),
    c(Command::NameTrail, "Name trail", "R", "rename label"),
    c(Command::ColourTrail, "Next trail colour", "C", "color sticky"),
    c(Command::Sidebar, "Trails sidebar", "B", "list tree panel"),
    c(Command::Read, "Read mode", "Tab", "fullscreen focus fill window"),
    c(Command::LinkHints, "Follow a link", "f", "link hints keyboard"),
    c(Command::Arrange, "Move or resize card", "M", "arrange keyboard"),
    c(Command::SaveMarkdown, "Save as Markdown", "Ctrl+S", "export file"),
    c(Command::Undo, "Undo", "Ctrl+Z", ""),
    c(Command::Redo, "Redo", "Ctrl+Shift+Z", ""),
    c(Command::Reopen, "Reopen closed card", "Ctrl+Shift+T", "restore undo close"),
    c(Command::CloseCard, "Close card", "Delete", "remove"),
    c(Command::Reload, "Reload page", "Ctrl+R", "refresh"),
    c(Command::FindInPage, "Find in page", "Ctrl+F", "search text words"),
    c(Command::MarkCard, "Select or deselect card", "v", "mark pick choose several"),
    c(Command::FillView, "Fill the view with card", "z", "zoom to card focus"),
    c(Command::PageMenu, "Page menu", "Menu", "context right-click"),
    c(Command::ZoomIn, "Zoom page in", "Ctrl++", "bigger larger text"),
    c(Command::ZoomOut, "Zoom page out", "Ctrl+-", "smaller text"),
    c(Command::ZoomReset, "Reset page zoom", "Ctrl+0", "actual size 100%"),
    c(Command::SelectAll, "Select all cards", "Ctrl+A", ""),
    c(Command::NewSpace, "New space", "Ctrl+Shift+N", "board workspace"),
    c(Command::NextSpace, "Next space", "Ctrl+PgDn", "board workspace"),
    c(Command::PreviousSpace, "Previous space", "Ctrl+PgUp", "board workspace"),
    c(Command::RenameSpace, "Rename space", "F2", "board workspace name"),
    c(Command::NextTint, "Next space tint", "Ctrl+Shift+B", "background colour color"),
    c(Command::DeleteSpace, "Delete space", "Ctrl+Shift+W", "board workspace remove"),
    c(Command::Shortcuts, "Keyboard shortcuts", "?", "help keys hints sheet"),
    c(Command::Quit, "Quit Trailboard", "Q", "exit close"),
];

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Card(NodeId),
    Space(SpaceId),
    Command(Command),
    /// Open this URL in a new card.
    Open(String),
}

/// A result row.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub item: Item,
    pub label: String,
    /// Shown dim on the right: a page's host, "Note", "Space" or a command's keys.
    pub detail: String,
    /// Matched letters of `label` (char indices).
    pub positions: Vec<usize>,
    /// A colour dot before the label: a trail's colour, or a sticky note's.
    pub dot: Option<Dot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dot {
    Trail(String),
    Sticky(String),
}

/// `https://www.example.com/a` → `example.com`; a local file → its name.
fn host(url: &str) -> &str {
    if let Some(path) = url.strip_prefix("file://") {
        return path.rsplit('/').next().unwrap_or(path);
    }
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.strip_prefix("www.").unwrap_or(host)
}

/// The rows for `query`: every card, folded ones too (by `used`, then stacking order, most recent
/// first), the other spaces and the commands, ranked together; then the row
/// that opens the query as an address or a search.
pub fn rows(query: &str, scene: &Scene, used: &HashMap<NodeId, u64>, spaces: &SpaceIndex) -> Vec<Row> {
    let mut cards: Vec<_> = scene.all_nodes().collect();
    cards.sort_by_key(|n| std::cmp::Reverse((used.get(&n.id).copied().unwrap_or(0), n.z)));

    let mut items: Vec<Row> = Vec::new();
    let mut alts: Vec<String> = Vec::new();
    for n in cards {
        let label = if n.title.trim().is_empty() { "Untitled".to_string() } else { n.title.clone() };
        let (detail, alt) = if n.is_note() {
            let kind = if n.is_sticky() { "Sticky note" } else { "Note" };
            (kind.to_string(), n.note.clone().unwrap_or_default())
        } else {
            let alt = n.url.split_once("://").map_or(n.url.as_str(), |(_, r)| r).to_string();
            (host(&n.url).to_string(), alt)
        };
        // Folded away in a stack: still found, and going to it shows it in front.
        let detail = match (scene.is_hidden(n.id), detail.is_empty()) {
            (false, _) => detail,
            (true, true) => "In a stack".to_string(),
            (true, false) => format!("{detail} · in a stack"),
        };
        let dot = match (&n.sticky, &n.trail.colour) {
            (Some(s), _) => Some(Dot::Sticky(s.clone())),
            (None, Some(c)) => Some(Dot::Trail(c.clone())),
            _ => None,
        };
        items.push(Row { item: Item::Card(n.id), label, detail, positions: Vec::new(), dot });
        alts.push(alt);
    }
    for s in spaces.spaces.iter().filter(|s| s.id != spaces.active) {
        items.push(Row { item: Item::Space(s.id), label: s.name.clone(), detail: "Space".into(), positions: Vec::new(), dot: None });
        alts.push("go to switch space".into());
    }
    let many = spaces.spaces.len() > 1;
    for info in COMMANDS {
        if !many && matches!(info.cmd, Command::NextSpace | Command::PreviousSpace) {
            continue;
        }
        items.push(Row {
            item: Item::Command(info.cmd),
            label: info.label.into(),
            detail: info.keys.into(),
            positions: Vec::new(),
            dot: None,
        });
        alts.push(info.words.into());
    }

    let cands: Vec<Candidate> = items.iter().zip(&alts).map(|(r, alt)| Candidate { text: &r.label, alt }).collect();
    let mut out: Vec<Row> = palette::rank(query, &cands)
        .into_iter()
        .map(|r| Row { positions: r.positions, ..items[r.index].clone() })
        .collect();

    if let Some(url) = omnibox::resolve(query) {
        let q = query.trim();
        let search = url == omnibox::search_url(q);
        let label = if search { format!("Search the web for “{q}”") } else { format!("Open {q}") };
        let row = Row { item: Item::Open(url), label, detail: "New card".into(), positions: Vec::new(), dot: None };
        if search { out.push(row) } else { out.insert(0, row) }
    }
    out
}

/// Sizes in logical px; multiplied by the UI scale.
pub const ROW_H: f32 = 36.0;
pub const MAX_ROWS: usize = 9;
const GAP: f32 = 6.0;
const PAD: f32 = 6.0;
const FONT: f32 = 15.0;
const DETAIL: f32 = 13.0;

/// The text field's rect (shared with the address bar).
pub fn field_rect(viewport: Vec2, ui: f32) -> Rect {
    let w = (720.0 * ui).min(viewport.x - 32.0 * ui);
    Rect::new((viewport.x - w) / 2.0, 18.0 * ui, w, 48.0 * ui)
}

/// Where the result list is, in screen px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub field: Rect,
    /// The list's background (sized to the rows shown).
    pub rect: Rect,
    pub row_h: f32,
    /// Rows shown at once.
    pub shown: usize,
}

impl Panel {
    pub fn new(viewport: Vec2, ui: f32, len: usize) -> Panel {
        let field = field_rect(viewport, ui);
        let top = field.bottom() + GAP * ui;
        let row_h = ROW_H * ui;
        let fit = ((viewport.y - top - 2.0 * PAD * ui - 16.0 * ui) / row_h).floor().max(1.0) as usize;
        let shown = MAX_ROWS.min(fit).min(len.max(1));
        let rect = Rect::new(field.x, top, field.w, shown as f32 * row_h + 2.0 * PAD * ui);
        Panel { field, rect, row_h, shown }
    }

    fn rows_top(&self) -> f32 {
        self.rect.y + (self.rect.h - self.shown as f32 * self.row_h) / 2.0
    }

    /// Rect of row `index` with the list scrolled to `scroll`, if it is shown.
    pub fn row_rect(&self, index: usize, scroll: usize) -> Option<Rect> {
        let at = index.checked_sub(scroll).filter(|&i| i < self.shown)?;
        Some(Rect::new(self.rect.x, self.rows_top() + at as f32 * self.row_h, self.rect.w, self.row_h))
    }

    /// Index of the row at `p` among `len` rows scrolled to `scroll`.
    pub fn row_at(&self, p: Vec2, scroll: usize, len: usize) -> Option<usize> {
        if !self.rect.contains(p) || p.y < self.rows_top() {
            return None;
        }
        let i = scroll + ((p.y - self.rows_top()) / self.row_h) as usize;
        (i < len && i < scroll + self.shown).then_some(i)
    }

    /// The field or the list contains `p`.
    pub fn contains(&self, p: Vec2) -> bool {
        self.field.contains(p) || self.rect.contains(p)
    }
}

/// The result list (the field is drawn with the address bar's code).
pub fn layer(r: &mut Renderer, panel: &Panel, rows: &[Row], sel: usize, scroll: usize, th: &Theme, ui: f32) -> Layer {
    let mut l = Layer::default();
    let p = panel.rect;
    let radius = th.radius(ui);
    l.quads.push(Quad {
        shape: Shape::Shadow { rect: p.translate(Vec2::new(0.0, 8.0 * ui)), radius, blur: 28.0 * ui },
        color: th.shadow(0.6),
        texture: None,
    });
    l.quads.push(Quad { shape: Shape::Rect { rect: p, radius }, color: rgb(th.panel(), 1.0), texture: None });

    let size = FONT * ui;
    let dsize = DETAIL * ui;
    let pad = 14.0 * ui;
    if rows.is_empty() {
        let row = Rect::new(p.x, p.y, p.w, p.h);
        l.texts.push(Text {
            text: "Nothing found".into(),
            pos: [p.x + pad, row.y + (row.h - size * 1.3) / 2.0],
            size,
            color: rgb(th.dim_text(), 1.0),
            clip: row,
            bold: false,
        });
        return l;
    }
    for (i, row) in rows.iter().enumerate() {
        let Some(rr) = panel.row_rect(i, scroll) else { continue };
        let selected = i == sel;
        if selected {
            let hl = Rect::new(rr.x + PAD * ui, rr.y, rr.w - 2.0 * PAD * ui, rr.h);
            l.quads.push(Quad { shape: Shape::Rect { rect: hl, radius: th.small_radius(ui) }, color: rgb(th.selection(), 1.0), texture: None });
        }
        let mut x = rr.x + pad;
        let dot = 8.0 * ui;
        if let Some(d) = &row.dot {
            let colour = match d {
                Dot::Trail(c) => th.trail(Some(c)),
                Dot::Sticky(c) => th.sticky_paper(c),
            };
            let dr = Rect::new(x, rr.y + (rr.h - dot) / 2.0, dot, dot);
            l.quads.push(Quad { shape: Shape::Rect { rect: dr, radius: dot / 2.0 }, color: rgb(colour, 1.0), texture: None });
        }
        x += dot + 8.0 * ui;

        let dw = r.measure(&row.detail, dsize, false);
        let detail_x = rr.right() - pad - dw;
        l.texts.push(Text {
            text: row.detail.clone(),
            pos: [detail_x, rr.y + (rr.h - dsize * 1.3) / 2.0],
            size: dsize,
            color: rgb(th.dim_text(), 1.0),
            clip: rr,
            bold: false,
        });
        let clip = Rect::new(rr.x, rr.y, (detail_x - 12.0 * ui - rr.x).max(0.0), rr.h);
        let y = rr.y + (rr.h - size * 1.3) / 2.0;
        // The label in runs: matched letters in the accent, the rest in the text colour.
        for (text, matched) in runs(&row.label, &row.positions) {
            let w = r.measure(&text, size, false);
            let color = if matched { rgb(th.accent(), 1.0) } else { rgb(th.foreground(), 1.0) };
            l.texts.push(Text { text, pos: [x, y], size, color, clip, bold: false });
            x += w;
        }
    }
    // A thin scroll bar when not all rows fit.
    if rows.len() > panel.shown {
        let track = Rect::new(p.right() - 4.0 * ui, p.y + PAD * ui, 2.0 * ui, p.h - 2.0 * PAD * ui);
        let h = (track.h * panel.shown as f32 / rows.len() as f32).max(12.0 * ui);
        let y = track.y + (track.h - h) * scroll as f32 / (rows.len() - panel.shown) as f32;
        l.quads.push(Quad { shape: Shape::Rect { rect: Rect::new(track.x, y, track.w, h), radius: track.w / 2.0 }, color: rgb(th.divider(), 1.0), texture: None });
    }
    l
}

/// `text` cut into runs of matched (`positions`) and unmatched letters.
pub fn runs(text: &str, positions: &[usize]) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut next = positions.iter().peekable();
    for (i, ch) in text.chars().enumerate() {
        let matched = next.peek() == Some(&&i);
        if matched {
            next.next();
        }
        match out.last_mut() {
            Some((s, m)) if *m == matched => s.push(ch),
            _ => out.push((ch.to_string(), matched)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> Scene {
        let mut s = Scene::new();
        let r = Rect::new(0.0, 0.0, 400.0, 300.0);
        let a = s.add_page(r, "https://www.rust-lang.org/learn");
        s.node_mut(a).expect("a").title = "Learn Rust".into();
        let b = s.add_page(r, "https://en.wikipedia.org/wiki/Trail");
        s.node_mut(b).expect("b").title = "Trail - Wikipedia".into();
        s
    }

    #[test]
    fn hosts() {
        assert_eq!(host("https://www.example.com/a?b"), "example.com");
        assert_eq!(host("file:///home/x.html"), "x.html");
        assert_eq!(host("about:blank"), "about:blank");
        assert_eq!(host("example.org"), "example.org");
    }

    #[test]
    fn empty_query_lists_recent_cards_then_commands() {
        let s = board();
        let ids: Vec<NodeId> = s.nodes().map(|n| n.id).collect();
        let mut used = HashMap::new();
        used.insert(ids[0], 5);
        let r = rows("", &s, &used, &SpaceIndex::default());
        assert_eq!(r[0].item, Item::Card(ids[0]));
        assert_eq!(r[0].detail, "rust-lang.org");
        assert_eq!(r[1].item, Item::Card(ids[1]));
        assert!(matches!(r[2].item, Item::Command(Command::NewCard)));
        // One space: no next/previous space, and no Open row without a query.
        assert!(!r.iter().any(|r| matches!(r.item, Item::Command(Command::NextSpace) | Item::Open(_))));
    }

    #[test]
    fn query_ranks_cards_commands_and_search() {
        let s = board();
        let r = rows("trail", &s, &HashMap::new(), &SpaceIndex::default());
        assert_eq!(r[0].label, "Trail - Wikipedia");
        assert_eq!(r[0].positions, vec![0, 1, 2, 3, 4]);
        assert!(r.iter().any(|r| r.label == "Trails sidebar"));
        let last = r.last().expect("rows");
        assert_eq!(last.item, Item::Open("https://duckduckgo.com/?q=trail".into()));
        assert_eq!(last.label, "Search the web for “trail”");
        // Found by its URL.
        let r = rows("rust-lang", &s, &HashMap::new(), &SpaceIndex::default());
        assert_eq!(r[0].label, "Learn Rust");
        assert!(r[0].positions.is_empty());
    }

    #[test]
    fn an_address_opens_first() {
        let r = rows("example.com", &board(), &HashMap::new(), &SpaceIndex::default());
        assert_eq!(r[0].item, Item::Open("https://example.com".into()));
        assert_eq!(r[0].label, "Open example.com");
    }

    #[test]
    fn other_spaces_are_listed() {
        let mut spaces = SpaceIndex::default();
        let other = spaces.add();
        spaces.rename(other, "Reading");
        let r = rows("read", &Scene::new(), &HashMap::new(), &spaces);
        assert_eq!(r[0].item, Item::Space(other));
        assert!(r.iter().any(|r| r.item == Item::Command(Command::Read)));
        let r = rows("", &Scene::new(), &HashMap::new(), &spaces);
        assert!(r.iter().any(|r| r.item == Item::Command(Command::NextSpace)));
        // The active space is not offered.
        assert!(!r.iter().any(|r| r.item == Item::Space(spaces.active)));
    }

    #[test]
    fn label_runs() {
        assert_eq!(
            runs("Tidy trails", &[0, 5, 6]),
            vec![("T".into(), true), ("idy ".into(), false), ("tr".into(), true), ("ails".into(), false)]
        );
        assert_eq!(runs("ab", &[]), vec![("ab".into(), false)]);
    }

    #[test]
    fn panel_layout() {
        let vp = Vec2::new(1000.0, 800.0);
        let p = Panel::new(vp, 1.0, 20);
        assert_eq!(p.shown, MAX_ROWS);
        assert_eq!(p.field, Rect::new(140.0, 18.0, 720.0, 48.0));
        let r0 = p.row_rect(0, 0).expect("row 0");
        assert_eq!(p.row_at(r0.center(), 0, 20), Some(0));
        assert_eq!(p.row_at(r0.center(), 3, 20), Some(3));
        assert!(p.row_rect(MAX_ROWS, 0).is_none());
        assert_eq!(p.row_at(Vec2::new(10.0, r0.center().y), 0, 20), None);
        // Few rows: the list shrinks to them; a short window fits fewer.
        assert_eq!(Panel::new(vp, 1.0, 2).shown, 2);
        assert_eq!(Panel::new(Vec2::new(1000.0, 250.0), 1.0, 20).shown, 4);
        assert!(p.contains(Vec2::new(500.0, 40.0)));
        assert!(!p.contains(Vec2::new(500.0, 790.0)));
    }
}
