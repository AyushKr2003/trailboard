//! The Ctrl+K palette: one box that finds a card on the board, another space,
//! or a command, by fuzzy search (`scene::palette`). It is the address bar's
//! text field (`omnibox::Target::Palette`) with a list of results under it.
//!
//! With nothing typed it lists the cards, most recently used first, then the
//! other spaces and the commands. Typing ranks them all together; a last row
//! opens what was typed as an address or a web search in a new card (first,
//! when it looks like an address). Pages from history (`storage::pagetext`)
//! that no card shows come just before that row. Cards closed this session
//! (in this space) are found too, and come back where they were.

use std::collections::{HashMap, HashSet};

use compositor::{Layer, Quad, Renderer, Shape, Text};
use scene::pagetext;
use scene::settings::{Key, SearchEngine, Settings};
use scene::palette::{self, Candidate};
use scene::{Node, NodeId, Rect, Scene, Vec2};
use storage::pagetext::Visit;
use storage::spaces::{SpaceId, SpaceIndex};

use crate::omnibox;
use crate::theme::{rgb, Theme};

/// Board actions the palette can run, each with the keys that also do it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    PrivateSpace,
    NewPrivateSpace,
    DeleteSpace,
    History,
    ClearHistory,
    Settings,
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
    c(Command::PrivateSpace, "Private space on or off", "Ctrl+Shift+P", "incognito history cookies unrecorded privacy"),
    c(Command::NewPrivateSpace, "New private space", "", "incognito board workspace unrecorded privacy"),
    c(Command::DeleteSpace, "Delete space", "Ctrl+Shift+W", "board workspace remove"),
    c(Command::History, "History", "Ctrl+H", "visited pages browse past recent"),
    c(Command::ClearHistory, "Clear history", "", "forget delete erase visited pages privacy"),
    c(Command::Settings, "Settings", "Ctrl+,", "preferences options configure search engine history colours"),
    c(Command::Shortcuts, "Keyboard shortcuts", "?", "help keys hints sheet"),
    c(Command::Quit, "Quit Trailboard", "Q", "exit close"),
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    Card(NodeId),
    Space(SpaceId),
    Command(Command),
    /// Open this URL in a new card.
    Open(String),
    /// Bring back this closed card where it was.
    Closed(NodeId),
    /// A row of the settings panel: Enter or a click steps its value.
    Setting(Key),
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
    /// The page in history this row came from: Shift+Delete forgets it.
    pub history: Option<String>,
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

/// History rows shown at most (after the board's own results).
pub const MAX_HISTORY: usize = 6;
/// Closed cards searched at most, newest first.
pub const MAX_CLOSED: usize = 10;
/// Closed cards listed when nothing is typed.
pub const MAX_CLOSED_EMPTY: usize = 3;

/// A label for a card: its title, or "Untitled".
fn card_label(n: &Node) -> String {
    if n.title.trim().is_empty() { "Untitled".to_string() } else { n.title.clone() }
}

/// The colour dot a card shows in the list: a sticky note's, or its trail's.
fn card_dot(n: &Node) -> Option<Dot> {
    match (&n.sticky, &n.trail.colour) {
        (Some(s), _) => Some(Dot::Sticky(s.clone())),
        (None, Some(c)) => Some(Dot::Trail(c.clone())),
        _ => None,
    }
}

/// What a card says on the right ("example.com", "Note") and the second text
/// it is found by (its address without the scheme, a note's body).
fn card_detail(n: &Node) -> (String, String) {
    if n.is_note() {
        let kind = if n.is_sticky() { "Sticky note" } else { "Note" };
        (kind.to_string(), n.note.clone().unwrap_or_default())
    } else {
        let alt = n.url.split_once("://").map_or(n.url.as_str(), |(_, r)| r).to_string();
        (host(&n.url).to_string(), alt)
    }
}

/// The rows for `query`: every card, folded ones too (by `used`, then stacking order, most recent
/// first), cards closed this session (`closed`, oldest first as the undo
/// stack keeps them; newest are offered first), the other spaces and the
/// commands, ranked together; then pages from history (`visits`, best first)
/// that no card shows; then the row that opens the query as an address or a
/// search.
#[allow(clippy::too_many_arguments)]
pub fn rows(
    query: &str,
    scene: &Scene,
    used: &HashMap<NodeId, u64>,
    closed: &[&Node],
    spaces: &SpaceIndex,
    visits: &[Visit],
    engine: SearchEngine,
) -> Vec<Row> {
    let mut cards: Vec<_> = scene.all_nodes().collect();
    cards.sort_by_key(|n| std::cmp::Reverse((used.get(&n.id).copied().unwrap_or(0), n.z)));

    let mut items: Vec<Row> = Vec::new();
    let mut alts: Vec<String> = Vec::new();
    for n in cards {
        let (detail, alt) = card_detail(n);
        // Folded away in a stack: still found, and going to it shows it in front.
        let detail = match (scene.is_hidden(n.id), detail.is_empty()) {
            (false, _) => detail,
            (true, true) => "In a stack".to_string(),
            (true, false) => format!("{detail} · in a stack"),
        };
        items.push(Row { item: Item::Card(n.id), label: card_label(n), detail, positions: Vec::new(), dot: card_dot(n), history: None });
        alts.push(alt);
    }
    // Closed cards, newest first, once each, and not those back on the board (undo).
    let cap = if query.trim().is_empty() { MAX_CLOSED_EMPTY } else { MAX_CLOSED };
    let mut gone: HashSet<NodeId> = HashSet::new();
    for n in closed.iter().rev().filter(|n| scene.node(n.id).is_none() && gone.insert(n.id)).take(cap) {
        let (detail, alt) = card_detail(n);
        let detail = if detail.is_empty() { "Closed".to_string() } else { format!("Closed · {detail}") };
        items.push(Row { item: Item::Closed(n.id), label: card_label(n), detail, positions: Vec::new(), dot: card_dot(n), history: None });
        alts.push(alt);
    }
    for s in spaces.spaces.iter().filter(|s| s.id != spaces.active) {
        items.push(Row { item: Item::Space(s.id), label: s.name.clone(), detail: "Space".into(), positions: Vec::new(), dot: None, history: None });
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
            history: None,
        });
        alts.push(info.words.into());
    }

    let cands: Vec<Candidate> = items.iter().zip(&alts).map(|(r, alt)| Candidate { text: &r.label, alt }).collect();
    let mut out: Vec<Row> = palette::rank(query, &cands)
        .into_iter()
        .map(|r| Row { positions: r.positions, ..items[r.index].clone() })
        .collect();

    if !query.trim().is_empty() {
        // A page a card shows is that card's row (if not already found), or
        // else a closed card's that showed it; other pages open in a new card.
        let mut cards: HashMap<&str, usize> = HashMap::new();
        for (i, r) in items.iter().enumerate() {
            let node = match r.item {
                Item::Card(id) => scene.node(id),
                Item::Closed(id) => closed.iter().rev().copied().find(|n| n.id == id),
                _ => None,
            };
            if let Some(n) = node.filter(|n| !n.is_note()) {
                cards.entry(n.url.as_str()).or_insert(i);
            }
        }
        let mut seen: HashSet<Item> = out.iter().map(|r| r.item.clone()).collect();
        let mut added = 0;
        for v in visits {
            if added == MAX_HISTORY {
                break;
            }
            let row = match cards.get(v.url.as_str()) {
                Some(&i) => items[i].clone(),
                None => {
                    let label = if v.title.trim().is_empty() { v.url.clone() } else { v.title.clone() };
                    let detail = format!("History · {}", host(&v.url));
                    Row { item: Item::Open(v.url.clone()), label, detail, positions: Vec::new(), dot: None, history: None }
                }
            };
            if seen.insert(row.item.clone()) {
                let positions = pagetext::highlight(query, &row.label);
                out.push(Row { positions, history: Some(v.url.clone()), ..row });
                added += 1;
            }
        }
    }

    if let Some(url) = omnibox::resolve(query, engine) {
        let q = query.trim();
        let search = url == omnibox::search_url(q, engine);
        let label = if search { format!("Search {} for “{q}”", engine.name()) } else { format!("Open {q}") };
        let row = Row { item: Item::Open(url), label, detail: "New card".into(), positions: Vec::new(), dot: None, history: None };
        if search { out.push(row) } else { out.insert(0, row) }
    }
    out
}

/// Pages the history view lists at most.
pub const HISTORY_LIST: usize = 500;

/// The history view's rows (Ctrl+H): `visits` as given (newest first), each
/// saying when it was seen. A page a card shows goes to that card; others
/// open in a new card. `local` turns a UTC time in ms into local ms, and
/// `now` is local ms.
pub fn history_rows(query: &str, scene: &Scene, visits: &[Visit], now: i64, local: impl Fn(i64) -> i64) -> Vec<Row> {
    let mut cards: HashMap<&str, NodeId> = HashMap::new();
    for n in scene.all_nodes().filter(|n| !n.is_note()) {
        cards.entry(n.url.as_str()).or_insert(n.id);
    }
    visits
        .iter()
        .map(|v| {
            let label = if v.title.trim().is_empty() { v.url.clone() } else { v.title.clone() };
            let when = scene::when::label(local(v.seen_ms), now);
            let (item, detail) = match cards.get(v.url.as_str()) {
                Some(&id) => (Item::Card(id), format!("On the board · {} · {when}", host(&v.url))),
                None => (Item::Open(v.url.clone()), format!("{} · {when}", host(&v.url))),
            };
            let positions = pagetext::highlight(query, &label);
            Row { item, label, detail, positions, dot: None, history: Some(v.url.clone()) }
        })
        .collect()
}

/// The settings panel's rows (Ctrl+,): each setting with its value on the
/// right, in the panel's order, or ranked by `query` (which also finds a
/// setting by its values: "google" finds *Search engine*). `notes` add a word
/// after a value (where it came from, say).
pub fn settings_rows(query: &str, settings: &Settings, notes: &HashMap<Key, String>) -> Vec<Row> {
    let items: Vec<Row> = Key::ALL
        .iter()
        .map(|&k| {
            let value = settings.value(k);
            let detail = match notes.get(&k) {
                Some(n) => format!("{value} · {n}"),
                None => value,
            };
            Row { item: Item::Setting(k), label: k.label().into(), detail, positions: Vec::new(), dot: None, history: None }
        })
        .collect();
    let alts: Vec<String> = Key::ALL.iter().map(|k| k.words()).collect();
    let cands: Vec<Candidate> = items.iter().zip(&alts).map(|(r, alt)| Candidate { text: &r.label, alt }).collect();
    palette::rank(query, &cands).into_iter().map(|r| Row { positions: r.positions, ..items[r.index].clone() }).collect()
}

/// Sizes in logical px; multiplied by the UI scale.
pub const ROW_H: f32 = 36.0;
pub const MAX_ROWS: usize = 9;
const GAP: f32 = 6.0;
const PAD: f32 = 6.0;
const FONT: f32 = 15.0;
const DETAIL: f32 = 13.0;
/// Width of the page preview beside the list, and the least worth showing.
pub const PREVIEW_W: f32 = 320.0;
const PREVIEW_MIN: f32 = 160.0;

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

    /// Where the highlighted page's thumbnail (`size` px) goes: right of the
    /// list, top-aligned, [`PREVIEW_W`] wide (less for a tall page, to fit
    /// the window's height); or, in a window too narrow for that, under the
    /// list at its right edge. `None` when neither has room.
    pub fn preview_rect(&self, viewport: Vec2, size: (u32, u32), ui: f32) -> Option<Rect> {
        let shape = size.0.max(1) as f32 / size.1.max(1) as f32;
        let margin = 16.0 * ui;
        // The largest rect of this shape within `w × h`, at most PREVIEW_W wide.
        let fit = |w: f32, h: f32| {
            let w = (PREVIEW_W * ui).min(w).min(h * shape);
            (w >= PREVIEW_MIN * ui).then_some((w, w / shape))
        };
        let x = self.rect.right() + 2.0 * GAP * ui;
        if let Some((w, h)) = fit(viewport.x - x - margin, viewport.y - self.rect.y - margin) {
            return Some(Rect::new(x, self.rect.y, w, h));
        }
        let y = self.rect.bottom() + 2.0 * GAP * ui;
        let (w, h) = fit(self.rect.w, viewport.y - y - margin)?;
        Some(Rect::new(self.rect.right() - w, y, w, h))
    }

    /// The field or the list contains `p`.
    pub fn contains(&self, p: Vec2) -> bool {
        self.field.contains(p) || self.rect.contains(p)
    }
}

/// The result list (the field is drawn with the address bar's code).
/// `empty` is what an empty list says.
#[allow(clippy::too_many_arguments)]
pub fn layer(r: &mut Renderer, panel: &Panel, rows: &[Row], sel: usize, scroll: usize, empty: &str, th: &Theme, ui: f32) -> Layer {
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
            text: empty.into(),
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

/// The highlighted page's thumbnail (texture `key`) at `at`, framed like the list.
pub fn preview_layer(at: Rect, key: u64, th: &Theme, ui: f32) -> Layer {
    let mut l = Layer::default();
    let radius = th.radius(ui);
    let border = (1.0 * ui).max(1.0);
    let frame = at.inflate(border);
    l.quads.push(Quad {
        shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 8.0 * ui)), radius, blur: 28.0 * ui },
        color: th.shadow(0.6),
        texture: None,
    });
    l.quads.push(Quad { shape: Shape::Rect { rect: frame, radius: radius + border }, color: rgb(th.divider(), 1.0), texture: None });
    l.quads.push(Quad { shape: Shape::Rect { rect: at, radius }, color: [1.0; 4], texture: Some(key) });
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
    fn preview_sits_right_of_the_list_when_there_is_room() {
        let vp = Vec2::new(1600.0, 900.0);
        let panel = Panel::new(vp, 1.0, 5);
        let r = panel.preview_rect(vp, (480, 360), 1.0).unwrap();
        assert_eq!((r.x, r.y, r.w, r.h), (panel.rect.right() + 12.0, panel.rect.y, PREVIEW_W, 240.0));
        // A tall page is cut to the window's height and narrows to keep its shape.
        let tall = panel.preview_rect(vp, (120, 360), 1.0).unwrap();
        assert!(tall.bottom() <= vp.y - 16.0 + 0.01 && (tall.w / tall.h - 1.0 / 3.0).abs() < 0.01, "{tall:?}");
        // Some room: a narrower preview.
        let mid = Vec2::new(1150.0, 900.0);
        let m = Panel::new(mid, 1.0, 5).preview_rect(mid, (480, 360), 1.0).unwrap();
        assert!(m.w < PREVIEW_W && m.w >= PREVIEW_MIN && m.right() <= mid.x - 16.0 + 0.01, "{m:?}");
        // A narrow window: under the list, at its right edge.
        let small = Vec2::new(900.0, 900.0);
        let p = Panel::new(small, 1.0, 5);
        let u = p.preview_rect(small, (480, 360), 1.0).unwrap();
        assert_eq!((u.right(), u.y, u.w, u.h), (p.rect.right(), p.rect.bottom() + 12.0, PREVIEW_W, 240.0));
        // No room under a long list in a short window either: none.
        let short = Vec2::new(900.0, 420.0);
        let p = Panel::new(short, 1.0, 9);
        assert_eq!(p.preview_rect(short, (480, 360), 1.0), None);
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
        let r = rows("", &s, &used, &[], &SpaceIndex::default(), &[], SearchEngine::DuckDuckGo);
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
        let r = rows("trail", &s, &HashMap::new(), &[], &SpaceIndex::default(), &[], SearchEngine::DuckDuckGo);
        assert_eq!(r[0].label, "Trail - Wikipedia");
        assert_eq!(r[0].positions, vec![0, 1, 2, 3, 4]);
        assert!(r.iter().any(|r| r.label == "Trails sidebar"));
        let last = r.last().expect("rows");
        assert_eq!(last.item, Item::Open("https://duckduckgo.com/?q=trail".into()));
        assert_eq!(last.label, "Search DuckDuckGo for “trail”");
        // Found by its URL.
        let r = rows("rust-lang", &s, &HashMap::new(), &[], &SpaceIndex::default(), &[], SearchEngine::DuckDuckGo);
        assert_eq!(r[0].label, "Learn Rust");
        assert!(r[0].positions.is_empty());
    }

    #[test]
    fn an_address_opens_first() {
        let r = rows("example.com", &board(), &HashMap::new(), &[], &SpaceIndex::default(), &[], SearchEngine::DuckDuckGo);
        assert_eq!(r[0].item, Item::Open("https://example.com".into()));
        assert_eq!(r[0].label, "Open example.com");
    }

    #[test]
    fn other_spaces_are_listed() {
        let mut spaces = SpaceIndex::default();
        let other = spaces.add();
        spaces.rename(other, "Reading");
        let r = rows("read", &Scene::new(), &HashMap::new(), &[], &spaces, &[], SearchEngine::DuckDuckGo);
        assert_eq!(r[0].item, Item::Space(other));
        assert!(r.iter().any(|r| r.item == Item::Command(Command::Read)));
        let r = rows("", &Scene::new(), &HashMap::new(), &[], &spaces, &[], SearchEngine::DuckDuckGo);
        assert!(r.iter().any(|r| r.item == Item::Command(Command::NextSpace)));
        // The active space is not offered.
        assert!(!r.iter().any(|r| r.item == Item::Space(spaces.active)));
    }

    #[test]
    fn history_comes_before_search_without_open_cards() {
        let v = |url: &str, title: &str| Visit { url: url.into(), title: title.into(), seen_ms: 0 };
        let visits = [
            v("https://www.rust-lang.org/learn", "Learn Rust"), // a card shows it
            v("https://doc.rust-lang.org/book/", "The Rust Book"),
            v("https://example.org/x", ""),
        ];
        let r = rows("rust", &board(), &HashMap::new(), &[], &SpaceIndex::default(), &visits, SearchEngine::DuckDuckGo);
        // The card showing the first page is found by its title already, and only once.
        assert_eq!(r.iter().filter(|r| r.label == "Learn Rust").count(), 1);
        let hist: Vec<&Row> = r.iter().filter(|r| r.detail.starts_with("History")).collect();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].item, Item::Open("https://doc.rust-lang.org/book/".into()));
        assert_eq!(hist[0].detail, "History · doc.rust-lang.org");
        assert_eq!(hist[0].positions, vec![4, 5, 6, 7]);
        assert_eq!(hist[1].label, "https://example.org/x");
        assert_eq!(r[r.len() - 2].label, "https://example.org/x");
        assert!(r.last().unwrap().label.starts_with("Search DuckDuckGo"));
        // Text only the index matched finds the card that shows it.
        let r = rows("borrowing", &board(), &HashMap::new(), &[], &SpaceIndex::default(), &visits[..1], SearchEngine::DuckDuckGo);
        assert_eq!(r[0].label, "Learn Rust");
        assert!(matches!(r[0].item, Item::Card(_)));
        assert_eq!(r[0].detail, "rust-lang.org");
        // Nothing typed: no history.
        let r = rows("", &board(), &HashMap::new(), &[], &SpaceIndex::default(), &visits, SearchEngine::DuckDuckGo);
        assert!(!r.iter().any(|r| r.detail.starts_with("History")));
    }

    #[test]
    fn closed_cards_are_found_and_come_back() {
        let mut s = board();
        let r = Rect::new(0.0, 0.0, 400.0, 300.0);
        let close = |s: &mut Scene, url: &str, title: &str| {
            let id = s.add_page(r, url);
            s.node_mut(id).expect("card").title = title.into();
            s.remove(id).expect("removed").0
        };
        let a = close(&mut s, "https://www.gnu.org/", "GNU");
        let b = close(&mut s, "https://archlinux.org/", "Arch Linux");
        let c = close(&mut s, "https://kernel.org/", "The Kernel");
        let d = close(&mut s, "https://hypr.land/", "Hyprland");
        // `b` was closed twice (closed, undone, closed again): listed once.
        let closed = [&a, &b, &c, &b, &d];
        let used = HashMap::new();

        // Nothing typed: the newest few, after the open cards, before the commands.
        let rows0 = rows("", &s, &used, &closed, &SpaceIndex::default(), &[], SearchEngine::DuckDuckGo);
        let items: Vec<&Item> = rows0.iter().map(|r| &r.item).collect();
        assert!(matches!(items[0], Item::Card(_)) && matches!(items[1], Item::Card(_)));
        assert_eq!(&items[2..5], &[&Item::Closed(d.id), &Item::Closed(b.id), &Item::Closed(c.id)]);
        assert!(matches!(items[5], Item::Command(_)));
        assert_eq!(rows0[2].detail, "Closed · hypr.land");

        // Typed: found by title or address.
        let found = rows("gnu", &s, &used, &closed, &SpaceIndex::default(), &[], SearchEngine::DuckDuckGo);
        assert_eq!(found[0].item, Item::Closed(a.id));
        assert_eq!(found[0].label, "GNU");
        let found = rows("kernel.org", &s, &used, &closed, &SpaceIndex::default(), &[], SearchEngine::DuckDuckGo);
        assert_eq!(found[1].item, Item::Closed(c.id), "after the Open row");

        // A history page a closed card showed is that card's row.
        let visits = [Visit { url: "https://archlinux.org/".into(), title: "Arch Linux".into(), seen_ms: 0 }];
        let found = rows("wiki pacman", &s, &used, &closed, &SpaceIndex::default(), &visits, SearchEngine::DuckDuckGo);
        assert_eq!(found[0].item, Item::Closed(b.id));
        assert!(found[0].history.is_some());

        // A closed card that is back on the board (undo) is not offered.
        let mut s2 = s.clone();
        assert!(s2.insert(d.clone(), &[]));
        let r2 = rows("hypr", &s2, &used, &closed, &SpaceIndex::default(), &[], SearchEngine::DuckDuckGo);
        assert!(!r2.iter().any(|r| r.item == Item::Closed(d.id)));
        assert!(r2.iter().any(|r| r.item == Item::Card(d.id)));
    }

    #[test]
    fn settings_rows_find_by_name_and_value() {
        let s = Settings::default();
        let all = settings_rows("", &s, &HashMap::new());
        assert_eq!(all.iter().map(|r| r.item.clone()).collect::<Vec<_>>(), Key::ALL.map(Item::Setting).to_vec());
        assert_eq!(all[0].detail, "DuckDuckGo");
        let r = settings_rows("google", &s, &HashMap::new());
        assert_eq!(r[0].item, Item::Setting(Key::Search));
        let r = settings_rows("hist", &s, &HashMap::new());
        assert_eq!(r[0].item, Item::Setting(Key::History));
        assert_eq!(r[0].positions, vec![5, 6, 7, 8]);
        let notes = HashMap::from([(Key::LivePages, "TRAILBOARD_MAX_LIVE".to_string())]);
        let r = settings_rows("awake", &s, &notes);
        assert_eq!(r[0].detail, "6 · TRAILBOARD_MAX_LIVE");
        assert!(settings_rows("zzqx", &s, &HashMap::new()).is_empty());
    }

    #[test]
    fn history_view_rows() {
        let v = |url: &str, title: &str, seen_ms| Visit { url: url.into(), title: title.into(), seen_ms };
        let day = 86_400_000;
        let visits = [
            v("https://www.rust-lang.org/learn", "Learn Rust", 10 * day + 3_600_000), // a card shows it
            v("https://doc.rust-lang.org/book/", "The Rust Book", 9 * day),
            v("https://example.org/x", " ", 2 * day),
        ];
        let s = board();
        // Local time is UTC + 1 h here.
        let r = history_rows("rust", &s, &visits, 10 * day + 7_200_000, |ms| ms + 3_600_000);
        assert_eq!(r.len(), 3);
        assert!(matches!(r[0].item, Item::Card(_)));
        assert_eq!(r[0].detail, "On the board · rust-lang.org · Today 02:00");
        assert_eq!(r[0].positions, vec![6, 7, 8, 9]);
        assert_eq!(r[1].item, Item::Open("https://doc.rust-lang.org/book/".into()));
        assert_eq!(r[1].detail, "doc.rust-lang.org · Yesterday 01:00");
        assert_eq!(r[2].label, "https://example.org/x");
        assert_eq!(r[2].detail, "example.org · 3 Jan");
        assert!(r.iter().all(|r| r.history.is_some()));
        assert_eq!(r[1].history.as_deref(), Some("https://doc.rust-lang.org/book/"));
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
