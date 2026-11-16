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
use scene::download::{self, Downloads, State};
use scene::pagetext;
use scene::palette::{self, Candidate};
use scene::settings::SearchEngine;
use scene::{Node, NodeId, Rect, Scene, Vec2};
use storage::pagetext::Visit;
use storage::spaces::{SpaceId, SpaceIndex};

use crate::chrome::{self, type_};
use crate::icons::{self, Icon};
use crate::omnibox;
use crate::theme::{rgb, Theme};

/// Board actions the palette can run, each with the keys that also do it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    NewCard,
    StartPage,
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
    ClearCookies,
    Downloads,
    Settings,
    Shortcuts,
    Welcome,
    ToggleBlocking,
    AllowedSites,
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
    c(Command::StartPage, "New start page", "", "search box engine home blank empty card"),
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
    c(Command::ClearCookies, "Clear cookies", "", "delete erase site data sign out logins privacy"),
    c(Command::Downloads, "Downloads", "Ctrl+J", "files saved folder download list"),
    c(Command::Settings, "Settings", "Ctrl+,", "preferences options configure search engine history colours"),
    c(Command::Shortcuts, "Keyboard shortcuts", "?", "help keys hints sheet"),
    c(Command::Welcome, "Welcome page", "", "guide help getting started tour introduction first note"),
    c(Command::ToggleBlocking, "Allow or block ads on this site", "", "adblock trackers privacy whitelist exception protections"),
    c(Command::AllowedSites, "Sites with ads allowed", "", "adblock trackers privacy whitelist allowlist exceptions protections blocking list"),
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
    /// A row of the downloads view (`scene::download::Downloads` key): Enter
    /// opens the file, or tries again if it didn't finish.
    Download(u64),
    /// A row of the allowed-sites list: Enter, a click or Shift+Delete
    /// blocks ads on the site again.
    AllowedSite(String),
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
pub fn host(url: &str) -> &str {
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
        if n.url == scene::note::WELCOME_URL {
            return ("Trailboard".to_string(), "guide help".to_string());
        }
        if n.url == scene::note::ENGINES_URL {
            return ("Trailboard".to_string(), "search engines guide compare".to_string());
        }
        if n.url == scene::note::START_URL {
            return ("Trailboard".to_string(), String::new());
        }
        if n.url == scene::note::SETTINGS_URL {
            return (
                "Trailboard".to_string(),
                "settings preferences options".to_string(),
            );
        }
        let alt = n
            .url
            .split_once("://")
            .map_or(n.url.as_str(), |(_, r)| r)
            .to_string();
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

/// The downloads view's rows (Ctrl+J), newest first: the file's name, then
/// its progress or how it ended, its site and when it started. A query
/// filters by name (or, letters close together, by address). `missing` says
/// whether a finished file is gone from where it was saved.
pub fn download_rows(
    query: &str,
    list: &Downloads,
    missing: impl Fn(&str) -> bool,
    now: i64,
    local: impl Fn(i64) -> i64,
) -> Vec<Row> {
    let items: Vec<Row> = list
        .iter()
        .map(|d| {
            let gone = d.state == State::Done && missing(&d.path);
            let mut detail = download::status(d, gone);
            let site = host(&d.url);
            if !site.is_empty() && !d.url.starts_with("data:") && !d.url.starts_with("blob:") {
                detail = format!("{detail} · {site}");
            }
            if d.state != State::Running {
                detail = format!("{detail} · {}", scene::when::label(local(d.started_ms), now));
            }
            Row { item: Item::Download(d.key), label: d.name().to_string(), detail, positions: Vec::new(), dot: None, history: None }
        })
        .collect();
    if query.trim().is_empty() {
        return items;
    }
    let urls: Vec<&str> = list.iter().map(|d| d.url.as_str()).collect();
    let cands: Vec<Candidate> = items.iter().zip(&urls).map(|(r, url)| Candidate { text: &r.label, alt: url }).collect();
    palette::rank(query, &cands).into_iter().map(|r| Row { positions: r.positions, ..items[r.index].clone() }).collect()
}

/// The allowed-sites list's rows: each site where ads are allowed
/// (`sites`, sorted, each with whether it lasts only until quitting), saying
/// how many cards on the board show it. A query filters by site.
pub fn allowed_rows(query: &str, sites: &[(String, bool)], scene: &Scene) -> Vec<Row> {
    let items: Vec<Row> = sites
        .iter()
        .map(|(site, unsaved)| {
            let cards = scene.all_nodes().filter(|n| !n.is_note() && scene::privacy::covers(site, &n.url)).count();
            let mut parts = Vec::new();
            if *unsaved {
                parts.push("Until you quit".to_string());
            }
            match cards {
                0 => {}
                1 => parts.push("1 card".to_string()),
                n => parts.push(format!("{n} cards")),
            }
            Row { item: Item::AllowedSite(site.clone()), label: site.clone(), detail: parts.join(" · "), positions: Vec::new(), dot: None, history: None }
        })
        .collect();
    if query.trim().is_empty() {
        return items;
    }
    let cands: Vec<Candidate> = items.iter().map(|r| Candidate { text: &r.label, alt: "" }).collect();
    palette::rank(query, &cands).into_iter().map(|r| Row { positions: r.positions, ..items[r.index].clone() }).collect()
}

/// The icon that stands for a command in the list.
pub fn command_icon(c: Command) -> Icon {
    use Command::*;
    match c {
        NewCard | StartPage => Icon::Plus,
        NewNote => Icon::Note,
        NewSticky => Icon::Sticky,
        Tidy => Icon::Tidy,
        Fit | FillView => Icon::Fit,
        Fold | Outline => Icon::Stack,
        NameTrail | ColourTrail => Icon::Trail,
        Sidebar => Icon::Sidebar,
        Read => Icon::Expand,
        LinkHints | Shortcuts => Icon::Keyboard,
        Arrange => Icon::Grip,
        SaveMarkdown | Downloads => Icon::Download,
        Undo | Reopen => Icon::Undo,
        Redo => Icon::Forward,
        CloseCard | Quit | DeleteSpace => Icon::Close,
        Reload => Icon::Reload,
        FindInPage => Icon::Find,
        MarkCard | SelectAll => Icon::Check,
        PageMenu => Icon::More,
        ZoomIn => Icon::Plus,
        ZoomOut => Icon::Minus,
        ZoomReset => Icon::Search,
        NewSpace | NextSpace | PreviousSpace | RenameSpace | NextTint => Icon::Board,
        PrivateSpace | NewPrivateSpace => Icon::Lock,
        History | ClearHistory => Icon::History,
        ClearCookies | ToggleBlocking | AllowedSites => Icon::Shield,
        Settings => Icon::Settings,
        Welcome => Icon::Help,
    }
}

/// The icon a row shows when it has no site icon of its own.
fn row_icon(row: &Row) -> Icon {
    match &row.item {
        Item::Card(_) if row.detail.starts_with("Note") => Icon::Note,
        Item::Card(_) | Item::Closed(_) if row.detail.ends_with("Trailboard") => Icon::Board,
        Item::Card(_) => Icon::Globe,
        Item::Space(_) => Icon::Board,
        Item::Command(c) => command_icon(*c),
        Item::Open(_) if row.history.is_some() || row.detail.starts_with("History") => Icon::History,
        Item::Open(_) if row.label.starts_with("Search ") => Icon::Search,
        Item::Open(_) => Icon::Globe,
        Item::Closed(_) => Icon::Undo,
        Item::Download(_) => Icon::Download,
        Item::AllowedSite(_) => Icon::Shield,
    }
}

/// Sizes in logical px; multiplied by the UI scale.
pub const ROW_H: f32 = 36.0;
pub const MAX_ROWS: usize = 9;
/// Height of the field, and of the line of keys under a list.
pub const FIELD_H: f32 = 48.0;
pub const FOOT_H: f32 = 32.0;
const WIDTH: f32 = 680.0;
const PAD: f32 = 6.0;
/// From the sheet's edge to an icon, and from there to the text.
const INSET: f32 = 16.0;
const ICON: f32 = 16.0;
const LEAD: f32 = INSET + ICON + 12.0;
/// Width of the page preview beside the list, and the least worth showing.
pub const PREVIEW_W: f32 = 320.0;
const PREVIEW_MIN: f32 = 160.0;
const PREVIEW_GAP: f32 = 12.0;

/// The text field's rect (shared with the address bar).
pub fn field_rect(viewport: Vec2, ui: f32) -> Rect {
    field_rect_from(viewport, ui, 0.0)
}

/// The text field's rect, centred in the part of the window right of
/// `left` (an open sidebar's edge), so a box without a list doesn't cover it.
/// It sits a seventh of the way down the window, where the eye already is,
/// with the board still in view around it.
pub fn field_rect_from(viewport: Vec2, ui: f32, left: f32) -> Rect {
    let room = viewport.x - left;
    let w = (WIDTH * ui).min(room - 32.0 * ui);
    let y = (viewport.y * 0.14).clamp(16.0 * ui, 120.0 * ui);
    Rect::new((left + (room - w) / 2.0).round(), y.round(), w.round(), FIELD_H * ui)
}

/// Where the result list is, in screen px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub field: Rect,
    /// The list's background (sized to the rows shown).
    pub rect: Rect,
    /// The line of keys under the list.
    pub foot: Rect,
    pub row_h: f32,
    /// Rows shown at once.
    pub shown: usize,
}

impl Panel {
    pub fn new(viewport: Vec2, ui: f32, len: usize) -> Panel {
        let field = field_rect(viewport, ui);
        let top = field.bottom();
        let row_h = ROW_H * ui;
        let fit = ((viewport.y - top - 2.0 * PAD * ui - FOOT_H * ui - 16.0 * ui) / row_h).floor().max(1.0) as usize;
        let shown = MAX_ROWS.min(fit).min(len.max(1));
        let rect = Rect::new(field.x, top, field.w, shown as f32 * row_h + 2.0 * PAD * ui);
        let foot = Rect::new(field.x, rect.bottom(), field.w, FOOT_H * ui);
        Panel { field, rect, foot, row_h, shown }
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
    /// sheet at its right edge. `None` when neither has room.
    pub fn preview_rect(&self, viewport: Vec2, size: (u32, u32), ui: f32) -> Option<Rect> {
        let shape = size.0.max(1) as f32 / size.1.max(1) as f32;
        let margin = 16.0 * ui;
        // The largest rect of this shape within `w × h`, at most PREVIEW_W wide.
        let fit = |w: f32, h: f32| {
            let w = (PREVIEW_W * ui).min(w).min(h * shape);
            (w >= PREVIEW_MIN * ui).then_some((w, w / shape))
        };
        let x = self.rect.right() + PREVIEW_GAP * ui;
        if let Some((w, h)) = fit(viewport.x - x - margin, viewport.y - self.field.y - margin) {
            return Some(Rect::new(x, self.field.y, w, h));
        }
        let y = self.foot.bottom() + PREVIEW_GAP * ui;
        let (w, h) = fit(self.rect.w, viewport.y - y - margin)?;
        Some(Rect::new(self.rect.right() - w, y, w, h))
    }

    /// The field, the list or its foot contains `p`.
    pub fn contains(&self, p: Vec2) -> bool {
        self.field.contains(p) || self.rect.contains(p) || self.foot.contains(p)
    }
}

/// What a text field shows.
pub struct Field<'a> {
    pub icon: Icon,
    pub text: &'a str,
    /// The text up to the caret.
    pub before_caret: &'a str,
    pub placeholder: &'a str,
    /// All of the text is selected (typing replaces it).
    pub selected: bool,
    /// Said at the right end: find in page's "3 of 17".
    pub note: Option<String>,
    /// Keys and what they do, at the right end (a box without a list).
    pub hint: &'a [(&'a str, &'a str)],
}

/// Draws `hints` (keys and what they do) right to left, ending at `right`;
/// returns the left edge. A hint with no label is only its keys.
#[allow(clippy::too_many_arguments)]
fn draw_hints(l: &mut Layer, r: &mut Renderer, hints: &[(&str, &str)], right: f32, mid_y: f32, clip: Rect, th: &Theme, ui: f32) -> f32 {
    let size = type_::SMALL * ui;
    let ksize = type_::CAPTION * ui;
    let mut x = right;
    for (keys, label) in hints.iter().rev() {
        if !label.is_empty() {
            let w = r.measure(label, size, false);
            x -= w;
            chrome::text(l, *label, x, mid_y, size, rgb(th.text2(), 1.0), clip, false);
            x -= 6.0 * ui;
        }
        // "Shift+Enter" is two caps; "↑" one.
        let caps: Vec<&str> = keys.split('+').collect();
        let widths: Vec<f32> = caps.iter().map(|k| r.measure(k, ksize, false)).collect();
        x = chrome::draw_keycaps(l, &caps, &widths, x, mid_y, ksize, th, ui, clip);
        x -= if label.is_empty() { 3.0 * ui } else { 14.0 * ui };
    }
    x
}

/// The field: its icon, what was typed (or what to type), the caret, and
/// its note or hint at the right end. The sheet behind it is drawn by the caller.
pub fn field_layer(r: &mut Renderer, rect: Rect, f: &Field, th: &Theme, ui: f32) -> Layer {
    let mut l = Layer::default();
    let size = type_::INPUT * ui;
    let mid = rect.center().y;
    icons::draw(&mut l, r, f.icon, rect.x + (INSET + ICON / 2.0) * ui, mid, ICON * ui, rgb(th.text2(), 1.0));
    let mut right = rect.right() - INSET * ui;
    if let Some(note) = &f.note {
        let nsize = type_::SMALL * ui;
        let w = r.measure(note, nsize, false);
        chrome::text(&mut l, note.clone(), right - w, mid, nsize, rgb(th.text2(), 1.0), rect, false);
        right -= w + 14.0 * ui;
    }
    // The hint yields to what is typed: it shows while there is room for both.
    let x0 = rect.x + LEAD * ui;
    let text_w = r.measure(f.text, size, false);
    if !f.hint.is_empty() {
        let mut probe = Layer::default();
        let left = draw_hints(&mut probe, r, f.hint, right, mid, rect, th, ui);
        if left - 24.0 * ui >= x0 + text_w.max(r.measure(f.placeholder, size, false)) {
            l.quads.append(&mut probe.quads);
            l.texts.append(&mut probe.texts);
            right = left;
        }
    }
    let clip = Rect::new(x0, rect.y, (right - x0).max(1.0), rect.h);
    // A text longer than the field scrolls to keep the caret in view.
    let caret_x = r.measure(f.before_caret, size, false);
    let shift = (caret_x - (clip.w - 8.0 * ui)).max(0.0);
    let caret_h = (size * 1.35).round();
    let caret_y = (mid - caret_h / 2.0).round();
    if f.text.is_empty() {
        chrome::text(&mut l, f.placeholder, x0, mid, size, rgb(th.text3(), 1.0), clip, false);
    } else if f.selected {
        let sel = Rect::new(x0 - 2.0 * ui, caret_y, (text_w + 4.0 * ui).min(clip.w), caret_h);
        chrome::fill(&mut l, sel, th.small_radius(ui / th.text_scale).min(3.0 * ui), rgb(th.accent_soft(), 1.0));
    }
    if !(f.selected && !f.text.is_empty()) {
        let w = (1.5 * ui).round().max(1.0);
        chrome::fill(&mut l, Rect::new((x0 + caret_x - shift).round(), caret_y, w, caret_h), 0.0, rgb(th.accent_text(), 1.0));
    }
    if !f.text.is_empty() {
        l.texts.push(Text { text: f.text.to_string(), pos: [x0 - shift, chrome::text_y(mid, size)], size, color: rgb(th.text(), 1.0), clip, bold: false });
    }
    l
}

/// The line under a list: how many rows it has, and the keys that act on them.
pub fn foot_layer(r: &mut Renderer, foot: Rect, hints: &[(&str, &str)], len: usize, th: &Theme, ui: f32) -> Layer {
    let mut l = Layer::default();
    let line = chrome::hair(ui);
    chrome::fill(&mut l, Rect::new(foot.x + line, foot.y, foot.w - 2.0 * line, line), 0.0, rgb(th.line(), 1.0));
    let mid = foot.center().y;
    let left = draw_hints(&mut l, r, hints, foot.right() - INSET * ui, mid, foot, th, ui);
    let count = match len {
        0 => String::new(),
        1 => "1 result".to_string(),
        n => format!("{n} results"),
    };
    let size = type_::SMALL * ui;
    let x = foot.x + INSET * ui;
    if x + r.measure(&count, size, false) + 16.0 * ui <= left {
        chrome::text(&mut l, count, x, mid, size, rgb(th.text3(), 1.0), foot, false);
    }
    l
}

/// The result list's rows; the sheet behind them is drawn with the field.
/// `site_icons` has each row's site icon texture, where it has one;
/// `empty` is what an empty list says.
#[allow(clippy::too_many_arguments)]
pub fn layer(r: &mut Renderer, panel: &Panel, rows: &[Row], site_icons: &[Option<u64>], sel: usize, scroll: usize, empty: &str, th: &Theme, ui: f32) -> Layer {
    let mut l = Layer::default();
    let p = panel.rect;

    let size = type_::BODY * ui;
    let dsize = type_::SMALL * ui;
    let small = th.small_radius(ui / th.text_scale);
    if rows.is_empty() {
        let w = r.measure(empty, size, false);
        chrome::text(&mut l, empty, p.x + (p.w - w) / 2.0, p.center().y, size, rgb(th.text2(), 1.0), p, false);
        return l;
    }
    for (i, row) in rows.iter().enumerate() {
        let Some(rr) = panel.row_rect(i, scroll) else { continue };
        let selected = i == sel;
        let mid = rr.center().y;
        if selected {
            let hl = Rect::new(rr.x + PAD * ui, rr.y + 1.0 * ui, rr.w - 2.0 * PAD * ui, rr.h - 2.0 * ui);
            chrome::fill(&mut l, hl, small, rgb(th.active(), 1.0));
        }
        // What kind of thing the row is, at a glance: its site's icon, the
        // colour of its trail or sticky note, or an icon for its kind.
        let icon = ICON * ui;
        let ix = rr.x + INSET * ui;
        match (site_icons.get(i).copied().flatten(), &row.dot) {
            (_, Some(d)) => {
                let colour = match d {
                    Dot::Trail(c) => th.color(c),
                    Dot::Sticky(c) => th.sticky_paper(c),
                };
                let d = 10.0 * ui;
                chrome::fill(&mut l, Rect::new((ix + (icon - d) / 2.0).round(), (mid - d / 2.0).round(), d, d), d / 2.0, rgb(colour, 1.0));
            }
            (Some(key), None) => l.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(ix.round(), (mid - icon / 2.0).round(), icon, icon), radius: 0.0 },
                color: [1.0; 4],
                texture: Some(key),
            }),
            (None, None) => icons::draw(&mut l, r, row_icon(row), ix + icon / 2.0, mid, icon, rgb(if selected { th.text() } else { th.text2() }, 1.0)),
        }
        let mut x = rr.x + LEAD * ui;

        // A command's keys as keycaps; anything else (a host, "Space") as secondary text.
        let right = rr.right() - INSET * ui;
        let caps = matches!(row.item, Item::Command(_)).then(|| chrome::keycaps(&row.detail)).flatten();
        let detail_x = match caps {
            Some(keys) => {
                let ksize = type_::CAPTION * ui;
                let widths: Vec<f32> = keys.iter().map(|k| r.measure(k, ksize, false)).collect();
                chrome::draw_keycaps(&mut l, &keys, &widths, right, mid, ksize, th, ui, rr)
            }
            None => {
                // The detail keeps at most 45% of the row; the label has the rest.
                let detail = r.ellipsize(&row.detail, dsize, false, rr.w * 0.45);
                let dw = r.measure(&detail, dsize, false);
                chrome::text(&mut l, detail, right - dw, mid, dsize, rgb(th.text2(), 1.0), rr, false);
                right - dw
            }
        };
        let clip = Rect::new(x, rr.y, (detail_x - 16.0 * ui - x).max(0.0), rr.h);
        // The label in runs: matched letters stand out, heavier and in the accent.
        for (text, matched) in runs(&row.label, &row.positions) {
            let w = r.measure(&text, size, matched);
            let color = if matched { rgb(th.accent_text(), 1.0) } else { rgb(th.text(), 1.0) };
            chrome::text(&mut l, text, x, mid, size, color, clip, matched);
            x += w;
        }
    }
    // A thin scroll bar when not all rows fit.
    if rows.len() > panel.shown {
        let w = (3.0 * ui).round().max(2.0);
        let track = Rect::new(p.right() - w - 2.0 * ui, p.y + PAD * ui, w, p.h - 2.0 * PAD * ui);
        let h = (track.h * panel.shown as f32 / rows.len() as f32).max(16.0 * ui);
        let y = track.y + (track.h - h) * scroll as f32 / (rows.len() - panel.shown) as f32;
        chrome::fill(&mut l, Rect::new(track.x, y, track.w, h), track.w / 2.0, rgb(th.line_strong(), 0.8));
    }
    l
}

/// The highlighted page's thumbnail (texture `key`) at `at`, framed like the list.
pub fn preview_layer(at: Rect, key: u64, th: &Theme, ui: f32) -> Layer {
    let mut l = Layer::default();
    let radius = th.radius(ui / th.text_scale);
    chrome::lift(&mut l, at, radius, th, ui, chrome::Lift::High);
    l.quads.push(Quad { shape: Shape::Rect { rect: at, radius }, color: [1.0; 4], texture: Some(key) });
    chrome::outline(&mut l, at, radius, chrome::hair(ui), rgb(th.line(), 1.0));
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
        assert_eq!((r.x, r.y, r.w, r.h), (panel.rect.right() + 12.0, panel.field.y, PREVIEW_W, 240.0));
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
        assert_eq!((u.right(), u.y, u.w, u.h), (p.rect.right(), p.foot.bottom() + 12.0, PREVIEW_W, 240.0));
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
    fn allowed_site_rows() {
        let mut s = board();
        s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "https://de.wikipedia.org/wiki/Pfad");
        let sites = vec![("example.com".to_string(), true), ("rust-lang.org".to_string(), false), ("wikipedia.org".to_string(), false)];
        let r = allowed_rows("", &sites, &s);
        let got: Vec<(&str, &str)> = r.iter().map(|r| (r.label.as_str(), r.detail.as_str())).collect();
        assert_eq!(got, [("example.com", "Until you quit"), ("rust-lang.org", "1 card"), ("wikipedia.org", "2 cards")]);
        assert_eq!(r[1].item, Item::AllowedSite("rust-lang.org".into()));
        let r = allowed_rows("wiki", &sites, &s);
        assert_eq!(r[0].label, "wikipedia.org");
        assert!(!r[0].positions.is_empty());
        assert!(allowed_rows("zzqx", &sites, &s).is_empty());
        assert!(allowed_rows("", &[], &s).is_empty());
    }

    #[test]
    fn download_view_rows() {
        let day = 86_400_000;
        let mut l = Downloads::default();
        let a = l.start("https://files.example.org/report.pdf", "/home/u/Downloads/report.pdf", false, 9 * day);
        l.update(a, 4_200_000, 4_200_000, State::Done);
        let b = l.start("https://www.example.com/big.iso", "/home/u/Downloads/big.iso", false, 10 * day);
        l.update(b, 1_500_000, 3_000_000, State::Running);
        let c = l.start("data:text/plain,hi", "/home/u/Downloads/hi.txt", false, 10 * day);
        l.update(c, 0, 0, State::Failed);

        let r = download_rows("", &l, |_| false, 10 * day + 3_600_000, |ms| ms);
        let shown: Vec<(&str, &str)> = r.iter().map(|r| (r.label.as_str(), r.detail.as_str())).collect();
        assert_eq!(
            shown,
            [
                ("hi.txt", "Failed · Today 00:00"),
                ("big.iso", "1.5 MB of 3.0 MB · 50% · example.com"),
                ("report.pdf", "4.2 MB · files.example.org · Yesterday 00:00"),
            ]
        );
        let r = download_rows("", &l, |p| p.ends_with(".pdf"), 10 * day, |ms| ms);
        assert!(r[2].detail.starts_with("Deleted or moved"));
        let r = download_rows("rep", &l, |_| false, 10 * day, |ms| ms);
        assert_eq!(r[0].item, Item::Download(a));
        assert_eq!(r[0].positions, vec![0, 1, 2]);
        // An address finds it too when its letters lie together.
        let r = download_rows("files.ex", &l, |_| false, 10 * day, |ms| ms);
        assert_eq!(r.first().map(|r| &r.item), Some(&Item::Download(a)));
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
        assert_eq!(p.field, Rect::new(160.0, 112.0, 680.0, 48.0));
        let r0 = p.row_rect(0, 0).expect("row 0");
        assert_eq!(p.row_at(r0.center(), 0, 20), Some(0));
        assert_eq!(p.row_at(r0.center(), 3, 20), Some(3));
        assert!(p.row_rect(MAX_ROWS, 0).is_none());
        assert_eq!(p.row_at(Vec2::new(10.0, r0.center().y), 0, 20), None);
        // Few rows: the list shrinks to them; a short window fits fewer.
        assert_eq!(Panel::new(vp, 1.0, 2).shown, 2);
        assert_eq!(Panel::new(Vec2::new(1000.0, 250.0), 1.0, 20).shown, 2);
        assert!(p.contains(Vec2::new(500.0, 130.0)));
        // The line of keys under the list is part of the sheet.
        assert!(p.contains(p.foot.center()));
        assert!(!p.contains(Vec2::new(500.0, 790.0)));
    }

    #[test]
    fn a_lone_field_sits_right_of_the_sidebar() {
        let vp = Vec2::new(1000.0, 800.0);
        assert_eq!(field_rect_from(vp, 1.0, 0.0), field_rect(vp, 1.0));
        // Wide window: full width, centred in the room right of the sheet.
        let r = field_rect_from(Vec2::new(1600.0, 800.0), 1.0, 296.0);
        assert_eq!(r.w, 680.0);
        assert_eq!(r.x, 296.0 + (1304.0 - 680.0) / 2.0);
        // Narrow window: shrinks to the room, never starting under the sheet.
        let r = field_rect_from(vp, 1.0, 296.0);
        assert_eq!(r.x, 296.0 + 16.0);
        assert_eq!(r.right(), 1000.0 - 16.0);
    }
}
