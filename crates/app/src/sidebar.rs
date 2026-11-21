//! The trails sidebar's panel (Ctrl+B, or the pointer resting on the window's left
//! edge): layout, hit testing and drawing of the rows from `scene::sidebar`.
//!
//! Our own design (M11 step 4, 2026-11-15; redone as a browser's side panel
//! 2026-11-17). The panel is a sheet floating a little in from the window's
//! left edge, over the board, faintly tinted with the space's colour. From
//! the top: the current card's back, forward and reload with a button that
//! keeps the sheet open or hides it; an address pill with the current card's
//! site (a press edits its address); the space's name and counts; a "New
//! card" row; then the list. Groups are set apart by space, not lines; the
//! pinned trails and the rest get small section labels when both are there.
//! Cards show their site's icon (a letter tile without one) and a close
//! button under the pointer. The icons are stops on the trail itself: a
//! line runs down from each page and curves into the pages it opened, as
//! the board's trails do (2026-11-21). The way from the trail's first page
//! to the current card is in the accent, so the sheet shows how you got
//! here; other trails are neutral, or their own colour once marked. The
//! current card is a raised tab with its icon ringed, and the tab glides
//! to the new current card instead of jumping. When the sheet opens its
//! rows come in one after another from the left edge (none of it when the
//! desktop asks for less motion). The foot holds the settings, a dot per
//! space and "+". Corners follow the theme's rounding, like every surface.

use std::time::{Duration, Instant};

use compositor::{Layer, Quad, Renderer, Shape, Text};
use scene::sidebar::{Group, Row};
use scene::{Node, NodeId, Rect, Scene, Vec2};

use crate::chrome::{self, Lift, Press, type_};
use crate::icons::{self, Icon};
use crate::theme::{Rgb, Theme, blend, rgb, shade};

/// Sizes in logical px; multiplied by the UI scale.
pub const WIDTH: f32 = 288.0;
/// Gap between the sheet and the window's edges.
pub const MARGIN: f32 = 8.0;
/// Everything above the list: controls, address, space, "New card".
pub const HEAD_H: f32 = 160.0;
/// The settings, space dots and "+" below the list.
pub const FOOT_H: f32 = 48.0;
pub const PAD: f32 = 10.0;
/// Rows and the controls in the head.
pub const ROW_H: f32 = 32.0;
pub const HEADER_ROW_H: f32 = 32.0;
/// Space above every group but the first.
pub const GROUP_GAP: f32 = 12.0;
/// A section label ("Pinned", "Trails") above a group.
pub const LABEL_H: f32 = 24.0;
pub const INDENT: f32 = 16.0;
pub const FONT: f32 = type_::BODY;
pub const ICON: f32 = 16.0;
/// A small square button (back, forward, a space's dot, a row's close).
pub const BUTTON: f32 = 28.0;
/// A space's cell in the foot and the room between cells.
pub const CHIP: f32 = 24.0;
pub const CHIP_GAP: f32 = 2.0;
/// The dot inside a space's cell.
pub const DOT: f32 = 8.0;
/// The panel never takes more than this share of the window's width.
pub const MAX_SHARE: f32 = 0.45;

// Where the head's parts sit, from the sheet's top.
const NAV_Y: f32 = 8.0;
const ADDRESS_Y: f32 = 44.0;
const ADDRESS_H: f32 = 36.0;
const SPACE_Y: f32 = 88.0;
const NEW_Y: f32 = 122.0;

/// Section labels, shown only when the list has both parts.
pub const PINNED: &str = "PINNED";
pub const TRAILS: &str = "TRAILS";
pub const NEW_CARD: &str = "New card";
pub const NO_ADDRESS: &str = "Search or enter address";

/// Something on the sheet that can be pressed, other than a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    Back,
    Forward,
    Reload,
    /// Keeps a peeking sheet open; hides a kept one.
    Keep,
    /// The address pill: edit the current card's address.
    Address,
    /// The space's name: the spaces menu.
    SpaceName,
    NewCard,
    /// The close button of card row `i` (shown under the pointer).
    Close(usize),
    Settings,
    /// Space `i`'s dot.
    Space(usize),
    NewSpace,
}

impl Spot {
    /// What a tooltip calls it.
    pub fn name(self) -> &'static str {
        match self {
            Spot::Back => "Back",
            Spot::Forward => "Forward",
            Spot::Reload => "Reload",
            Spot::Keep => "Trails sidebar",
            Spot::Address => "Address",
            Spot::SpaceName => "Spaces",
            Spot::NewCard => NEW_CARD,
            Spot::Close(_) => "Close card",
            Spot::Settings => "Settings",
            Spot::Space(_) => "",
            Spot::NewSpace => "New space",
        }
    }
}

/// The room a row takes in the list: space and a label above it, then the row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub gap: f32,
    pub label: Option<&'static str>,
    pub h: f32,
}

impl Slot {
    fn total(&self) -> f32 {
        self.gap + self.h
    }
}

/// The slots of `rows`, in logical px: a gap before each group but the first,
/// section labels before the first pinned and the first other trail when the
/// list has pinned trails.
pub fn slots(rows: &[Row]) -> Vec<Slot> {
    let pinned = |r: &Row| matches!(r, Row::Header { mark, .. } if mark.pinned);
    let any_pinned = rows.iter().any(pinned);
    let first_pinned = rows.iter().position(pinned);
    let first_trail = rows.iter().position(|r| matches!(r, Row::Header { group: Group::Trail(_), mark, .. } if !mark.pinned));
    rows.iter()
        .enumerate()
        .map(|(i, r)| match r {
            Row::Header { .. } => {
                let label = match () {
                    _ if !any_pinned => None,
                    _ if first_pinned == Some(i) => Some(PINNED),
                    _ if first_trail == Some(i) => Some(TRAILS),
                    _ => None,
                };
                let gap = if i == 0 { 0.0 } else { GROUP_GAP };
                Slot { gap: gap + if label.is_some() { LABEL_H } else { 0.0 }, label, h: HEADER_ROW_H }
            }
            Row::Card { .. } => Slot { gap: 0.0, label: None, h: ROW_H },
        })
        .collect()
}

/// Where the panel and its rows are, in screen px.
#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    /// The sheet.
    pub rect: Rect,
    /// The part the rows scroll in.
    pub list: Rect,
    /// The settings and space dots' strip.
    pub foot: Rect,
    ui: f32,
    /// Each row's slot, in screen px.
    slots: Vec<Slot>,
}

impl Panel {
    /// The sheet alone, for hit tests that don't need the rows.
    pub fn sheet(viewport: Vec2, ui: f32) -> Rect {
        let m = (MARGIN * ui).round();
        let w = (WIDTH * ui).min(viewport.x * MAX_SHARE).round();
        Rect::new(m, m, w, (viewport.y - 2.0 * m).max(1.0))
    }

    pub fn new(viewport: Vec2, ui: f32, slots: &[Slot]) -> Panel {
        let rect = Self::sheet(viewport, ui);
        let head = (HEAD_H * ui).min(rect.h);
        let foot_h = (FOOT_H * ui).min((rect.h - head).max(0.0));
        let list = Rect::new(rect.x, rect.y + head, rect.w, (rect.h - head - foot_h).max(1.0));
        let foot = Rect::new(rect.x, list.bottom(), rect.w, foot_h);
        let slots = slots.iter().map(|s| Slot { gap: s.gap * ui, label: s.label, h: s.h * ui }).collect();
        Panel { rect, list, foot, ui, slots }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// How many rows fit whole from row `scroll` on (at least one).
    pub fn fits(&self, scroll: usize) -> usize {
        let mut y = 0.0;
        let mut n = 0;
        for (k, s) in self.slots.iter().enumerate().skip(scroll) {
            // The first shown row keeps its label but not the group gap.
            y += if k == scroll { s.total() - self.lead_gap(k) } else { s.total() };
            if y > self.list.h {
                break;
            }
            n += 1;
        }
        n.max(1)
    }

    /// The group gap of row `k`, dropped when it is the first shown.
    fn lead_gap(&self, k: usize) -> f32 {
        let s = &self.slots[k];
        if s.gap > 0.0 { s.gap - if s.label.is_some() { LABEL_H * self.ui } else { 0.0 } } else { 0.0 }
    }

    /// The scroll that shows row `index` (if any), closest to `scroll`,
    /// never past the point where the last row is at the bottom.
    pub fn keep_visible(&self, index: Option<usize>, scroll: usize) -> usize {
        let mut s = scroll.min(self.max_scroll());
        if let Some(i) = index.filter(|&i| i < self.len()) {
            if i < s {
                s = i;
            }
            while i >= s + self.fits(s) && s < i {
                s += 1;
            }
        }
        s.min(self.max_scroll())
    }

    /// The largest useful scroll: the last row just shown.
    pub fn max_scroll(&self) -> usize {
        (0..self.len()).find(|&s| s + self.fits(s) >= self.len()).unwrap_or(0)
    }

    /// Wheel or Page keys: `scroll` moved by `step` rows, within the list.
    pub fn scroll_by(&self, scroll: usize, step: isize) -> usize {
        (scroll as isize + step).clamp(0, self.max_scroll() as isize) as usize
    }

    /// Rect of row `index` with the list scrolled to `scroll`, if it is shown whole.
    pub fn row_rect(&self, index: usize, scroll: usize) -> Option<Rect> {
        // `fits` claims one row even for an empty list, so check the end too.
        if index >= self.len() || index < scroll || index >= scroll + self.fits(scroll) {
            return None;
        }
        let mut y = self.list.y;
        for k in scroll..index {
            y += if k == scroll { self.slots[k].total() - self.lead_gap(k) } else { self.slots[k].total() };
        }
        let s = &self.slots[index];
        y += if index == scroll { s.gap - self.lead_gap(index) } else { s.gap };
        Some(Rect::new(self.rect.x, y, self.rect.w, s.h))
    }

    /// Where row `index`'s section label goes, if it has one and is shown.
    pub fn label_rect(&self, index: usize, scroll: usize) -> Option<(Rect, &'static str)> {
        let label = self.slots.get(index)?.label?;
        let row = self.row_rect(index, scroll)?;
        let h = LABEL_H * self.ui;
        Some((Rect::new(row.x, row.y - h, row.w, h), label))
    }

    /// Index of the row at `p` with the list scrolled to `scroll`.
    pub fn row_at(&self, p: Vec2, scroll: usize) -> Option<usize> {
        if !self.list.contains(p) {
            return None;
        }
        (scroll..scroll + self.fits(scroll)).find(|&i| self.row_rect(i, scroll).is_some_and(|r| r.contains(p)))
    }

    /// A row's filled part, inset from the sheet's sides.
    pub fn inner(&self, row: Rect) -> Rect {
        let inset = 6.0 * self.ui;
        Rect::new(row.x + inset, row.y + self.ui, row.w - 2.0 * inset, row.h - 2.0 * self.ui)
    }

    /// The close button at the right end of a card row.
    pub fn close(&self, row: Rect) -> Rect {
        let inner = self.inner(row);
        let b = (inner.h - 4.0 * self.ui).min(24.0 * self.ui);
        Rect::new(inner.right() - b - 2.0 * self.ui, inner.y + (inner.h - b) / 2.0, b, b)
    }

    /// The head's buttons: back, forward, reload from the left, the keep
    /// button on the right.
    pub fn nav(&self, spot: Spot) -> Rect {
        let b = BUTTON * self.ui;
        let y = self.rect.y + NAV_Y * self.ui;
        let x0 = self.rect.x + 6.0 * self.ui;
        let x = match spot {
            Spot::Back => x0,
            Spot::Forward => x0 + b + 2.0 * self.ui,
            Spot::Reload => x0 + 2.0 * (b + 2.0 * self.ui),
            _ => self.rect.right() - 6.0 * self.ui - b,
        };
        Rect::new(x, y, b, b)
    }

    /// The address pill.
    pub fn address(&self) -> Rect {
        let pad = PAD * self.ui;
        Rect::new(self.rect.x + pad, self.rect.y + ADDRESS_Y * self.ui, self.rect.w - 2.0 * pad, ADDRESS_H * self.ui)
    }

    /// The line with the space's name and counts.
    pub fn space_line(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y + SPACE_Y * self.ui, self.rect.w, 30.0 * self.ui)
    }

    /// The "New card" row.
    pub fn new_card(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y + NEW_Y * self.ui, self.rect.w, ROW_H * self.ui)
    }

    /// The settings button at the foot's left.
    pub fn settings(&self) -> Rect {
        let b = BUTTON * self.ui;
        Rect::new(self.foot.x + 6.0 * self.ui, self.foot.y + (self.foot.h - b) / 2.0, b, b)
    }

    /// The "+" for a new space at the foot's right.
    pub fn plus(&self) -> Rect {
        let b = BUTTON * self.ui;
        Rect::new(self.foot.right() - 6.0 * self.ui - b, self.foot.y + (self.foot.h - b) / 2.0, b, b)
    }

    /// The cell of space `i`'s dot, after the settings button.
    pub fn chip(&self, i: usize) -> Rect {
        let c = CHIP * self.ui;
        let x = self.settings().right() + 8.0 * self.ui + i as f32 * (c + CHIP_GAP * self.ui);
        Rect::new(x, self.foot.y + (self.foot.h - c) / 2.0, c, c)
    }

    /// How many space dots fit between the settings and "+".
    pub fn chips_shown(&self, n: usize) -> usize {
        (0..n).take_while(|&i| self.chip(i).right() <= self.plus().x - 4.0 * self.ui).count()
    }

    /// What can be pressed at `p`, other than a row: `n` spaces, the list
    /// scrolled to `scroll`, `closable` row indices have a close button.
    pub fn spot_at(&self, p: Vec2, n: usize, scroll: usize, closable: &dyn Fn(usize) -> bool) -> Option<Spot> {
        if !self.rect.contains(p) {
            return None;
        }
        if self.foot.contains(p) {
            if self.settings().contains(p) {
                return Some(Spot::Settings);
            }
            if self.plus().contains(p) {
                return Some(Spot::NewSpace);
            }
            let half = CHIP_GAP * self.ui / 2.0;
            return (0..self.chips_shown(n))
                .find(|&i| {
                    let c = self.chip(i);
                    Rect::new(c.x - half, self.foot.y, c.w + 2.0 * half, self.foot.h).contains(p)
                })
                .map(Spot::Space);
        }
        if self.list.contains(p) {
            let i = self.row_at(p, scroll)?;
            let row = self.row_rect(i, scroll)?;
            return (closable(i) && self.close(row).contains(p)).then_some(Spot::Close(i));
        }
        [Spot::Back, Spot::Forward, Spot::Reload, Spot::Keep]
            .into_iter()
            .find(|&s| self.nav(s).contains(p))
            .or_else(|| self.address().contains(p).then_some(Spot::Address))
            .or_else(|| self.inner(self.space_line()).contains(p).then_some(Spot::SpaceName))
            .or_else(|| self.inner(self.new_card()).contains(p).then_some(Spot::NewCard))
    }
}

/// A space, for its dot.
pub struct SpaceChip {
    pub name: String,
    /// Its tint, else `None` for the theme's plain board.
    pub colour: Option<Rgb>,
    pub private: bool,
}

/// What the head shows of the current card.
#[derive(Debug, Clone, Default)]
pub struct Current {
    /// It is a web page: back, forward and reload work, as far as they can.
    pub web: bool,
    pub can_back: bool,
    pub can_forward: bool,
    /// The site shown in the pill (empty: the placeholder).
    pub host: String,
    pub secure: bool,
}

/// How the list is shown: scrolled to `scroll`, with the keyboard highlight,
/// the row under the pointer and the current card marked.
pub struct View<'a> {
    pub scroll: usize,
    pub sel: Option<usize>,
    pub hover: Option<usize>,
    /// The button under the pointer.
    pub spot: Option<Spot>,
    pub current: Option<NodeId>,
    pub head: Current,
    /// Opened with Ctrl+B (or kept), rather than peeking.
    pub kept: bool,
    /// While a trail's header is dragged: y of the line where it would go.
    pub drop_line: Option<f32>,
    pub space: &'a str,
    pub spaces: &'a [SpaceChip],
    pub active_space: usize,
    /// The window's size, to keep tooltips inside it.
    pub viewport: Vec2,
    /// The texture of a card's site icon, if there is one.
    pub icon: &'a dyn Fn(&Node) -> Option<u64>,
}

/// The letter for a site without an icon: the first letter or digit of its
/// host, past "www.", in capitals.
pub fn letter(url: &str) -> String {
    let host = scene::icon::site(url).unwrap_or_default();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    host.chars().find(|c| c.is_alphanumeric()).map_or_else(|| "·".into(), |c| c.to_uppercase().collect())
}

/// The site a pill shows for `url`: its host past "www.", else the address.
pub fn pill_host(url: &str) -> String {
    match scene::icon::site(url) {
        Some(h) => h.strip_prefix("www.").unwrap_or(&h).to_string(),
        None => url.to_string(),
    }
}

/// "5 cards · 2 trails".
pub fn counts(cards: usize, trails: usize) -> String {
    let s = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    match trails {
        _ if cards == 0 => "Empty".into(),
        0 => s(cards, "card", "cards"),
        t => format!("{} · {}", s(cards, "card", "cards"), s(t, "trail", "trails")),
    }
}

/// `text` cut with "…" to fit `max_w` px at `size`. The renderer does this
/// for every text at its clip; call it where layout needs the cut text first.
pub fn fit(r: &mut Renderer, text: &str, size: f32, bold: bool, max_w: f32) -> String {
    r.ellipsize(text, size, bold, max_w)
}

/// The row each card row hangs from in its trail: the nearest row above it
/// in its group one level up (`None` for headers and a trail's first page).
pub fn parents(rows: &[Row]) -> Vec<Option<usize>> {
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let Row::Card { depth, group, .. } = *row else { return None };
            if depth == 0 {
                return None;
            }
            (0..i).rev().take_while(|&j| rows[j].group() == group).find(|&j| matches!(rows[j], Row::Card { depth: d, .. } if d + 1 == depth))
        })
        .collect()
}

/// Which rows are on the way from their trail's first page to the card
/// `current` (it included).
pub fn on_path(rows: &[Row], parents: &[Option<usize>], current: Option<NodeId>) -> Vec<bool> {
    let mut on = vec![false; rows.len()];
    let mut at = rows.iter().position(|r| matches!(r, Row::Card { id, .. } if Some(*id) == current));
    while let Some(i) = at {
        on[i] = true;
        at = parents[i];
    }
    on
}

/// How long the raised tab takes to glide to a new current card.
pub const TAB_GLIDE: Duration = Duration::from_millis(200);

/// What the sheet's motion keeps between frames: when it opened (its rows
/// arrive from then) and where the raised tab is gliding.
#[derive(Debug, Default, Clone)]
pub struct Motion {
    /// The desktop asks for less motion: everything is simply in place.
    pub still: bool,
    last_draw: Option<Instant>,
    opened: Option<Instant>,
    tab_card: Option<NodeId>,
    /// Where the tab was last drawn (screen y), if it was.
    tab_y: Option<f32>,
    /// A glide: from where, since when.
    tab_glide: Option<(f32, Instant)>,
}

impl Motion {
    /// Notes a frame drawn at `now`; after a gap the sheet is opening again.
    fn frame(&mut self, now: Instant) {
        if self.last_draw.is_none_or(|t| now.saturating_duration_since(t) > Duration::from_millis(200)) {
            self.opened = Some(now);
            self.tab_y = None;
            self.tab_glide = None;
        }
        self.last_draw = Some(now);
    }

    /// How far row `k` (in the order drawn) has come in, 0 → 1.
    fn rise(&self, k: usize, now: Instant) -> f32 {
        let Some(at) = self.opened.filter(|_| !self.still) else { return 1.0 };
        let t = now.saturating_duration_since(at).as_secs_f32();
        chrome::ease_out((t - k.min(14) as f32 * 0.014) / 0.2)
    }

    /// Where to draw the tab for `card`, whose row is at `target` (screen
    /// y; `None` when it is not in view): it glides there when the current
    /// card changes, and follows at once when the list scrolls.
    pub fn tab(&mut self, card: Option<NodeId>, target: Option<f32>, now: Instant) -> Option<f32> {
        let Some(to) = target else {
            self.tab_card = card;
            self.tab_y = None;
            self.tab_glide = None;
            return None;
        };
        if card != self.tab_card {
            self.tab_glide = self.tab_y.filter(|_| !self.still && self.tab_card.is_some()).map(|from| (from, now));
            self.tab_card = card;
        }
        let y = match self.tab_glide {
            Some((from, since)) => {
                let t = now.saturating_duration_since(since).as_secs_f32() / TAB_GLIDE.as_secs_f32();
                if t >= 1.0 {
                    self.tab_glide = None;
                    to
                } else {
                    from + (to - from) * chrome::ease_out(t)
                }
            }
            None => to,
        };
        self.tab_y = Some(y);
        Some(y)
    }
}

/// Fades what was added to `l` since `q0` quads and `t0` texts, and moves it.
fn fade_tail(l: &mut Layer, q0: usize, t0: usize, alpha: f32, shift: Vec2) {
    if alpha >= 1.0 {
        return;
    }
    let mut tail = Layer { quads: l.quads.split_off(q0), texts: l.texts.split_off(t0) };
    chrome::fade(&mut tail, alpha, shift);
    l.quads.extend(tail.quads);
    l.texts.extend(tail.texts);
}

/// The sheet's colour: the panel, faintly tinted with the space's colour.
pub fn surface(th: &Theme, tint: Option<Rgb>) -> Rgb {
    match tint {
        Some(t) => blend(th.panel(), t, if th.light() { 0.06 } else { 0.08 }),
        None => th.panel(),
    }
}

/// The current card's raised tab: a step above the sheet (white-ish on light
/// themes), so it reads as lifted, not pressed in.
fn tab(th: &Theme, surface: Rgb) -> Rgb {
    if th.light() { shade(surface, 0.05) } else { shade(surface, 0.075) }
}

/// The panel with `rows`, as `v` says.
#[allow(clippy::too_many_arguments)]
pub fn layer(r: &mut Renderer, panel: &Panel, rows: &[Row], v: &View, scene: &Scene, th: &Theme, ui: f32, motion: &mut Motion) -> Layer {
    let now = Instant::now();
    motion.frame(now);
    let scroll = v.scroll;
    let mut l = Layer::default();
    let p = panel.rect;
    let line = chrome::hair(ui);
    let radius = th.radius(ui / th.text_scale);
    let small = th.small_radius(ui / th.text_scale);
    let tint = v.spaces.get(v.active_space).and_then(|s| s.colour);
    let surface = surface(th, tint);
    chrome::panel(&mut l, p, radius, th, ui, Lift::Mid, surface);

    let fg = th.text();
    let dim = th.text2();
    let faint = th.text3();
    let soft = blend(fg, dim, 0.4);
    let size = FONT * ui;
    let pad = PAD * ui;
    let icon = ICON * ui;
    let press = |s: Spot| if v.spot == Some(s) { Press::Hover } else { Press::Idle };

    // Head, first line: the current card's back, forward, reload; on the
    // right the button that keeps a peek open (on while kept).
    let h = &v.head;
    chrome::icon_button(&mut l, r, panel.nav(Spot::Back), Icon::Back, press(Spot::Back), false, h.web && h.can_back, th, ui);
    chrome::icon_button(&mut l, r, panel.nav(Spot::Forward), Icon::Forward, press(Spot::Forward), false, h.web && h.can_forward, th, ui);
    chrome::icon_button(&mut l, r, panel.nav(Spot::Reload), Icon::Reload, press(Spot::Reload), false, h.web, th, ui);
    let keep = if v.kept && v.spot != Some(Spot::Keep) { Press::Down } else { press(Spot::Keep) };
    chrome::icon_button(&mut l, r, panel.nav(Spot::Keep), Icon::Sidebar, keep, v.kept, true, th, ui);
    let ks = type_::CAPTION * ui;

    // The address pill: a recess with the site's lock or globe and its host.
    let pill = panel.address();
    let pill_hot = v.spot == Some(Spot::Address);
    chrome::fill(&mut l, pill, small, rgb(if pill_hot { th.hover() } else { th.well() }, 1.0));
    chrome::outline(&mut l, pill, small, line, if pill_hot { rgb(th.line_strong(), 0.8) } else { rgb(th.line(), 0.6) });
    let glyph = if h.host.is_empty() { Icon::Search } else if h.secure { Icon::Lock } else { Icon::Globe };
    let gx = pill.x + 10.0 * ui + 7.0 * ui;
    icons::draw(&mut l, r, glyph, gx, pill.center().y, 14.0 * ui, rgb(if h.host.is_empty() { faint } else { dim }, 1.0));
    let tx = gx + 7.0 * ui + 8.0 * ui;
    let room = (pill.right() - 10.0 * ui - tx).max(1.0);
    let (text, ink) = if h.host.is_empty() { (NO_ADDRESS.to_string(), faint) } else { (fit(r, &h.host, size, false, room), fg) };
    chrome::text(&mut l, text, tx, pill.center().y, size, rgb(ink, 1.0), Rect::new(tx, pill.y, room, pill.h), false);

    // The space: its dot and name, the board's counts on the right; a press
    // opens the spaces menu, which a chevron says under the pointer.
    let sl = panel.space_line();
    let mid = sl.center().y;
    let space_hot = v.spot == Some(Spot::SpaceName);
    if space_hot {
        chrome::fill(&mut l, panel.inner(sl), small, rgb(th.hover(), 1.0));
    }
    let d = DOT * ui;
    let dot = Rect::new(p.x + pad + 4.0 * ui, mid - d / 2.0, d, d);
    let swatch = tint.unwrap_or(faint);
    if v.spaces.get(v.active_space).is_some_and(|s| s.private) {
        chrome::fill(&mut l, dot, small.min(d / 2.0), rgb(swatch, 1.0));
        chrome::fill(&mut l, dot.inflate(-1.5 * ui), small.min(d / 2.0), rgb(surface, 1.0));
    } else {
        chrome::fill(&mut l, dot, small.min(d / 2.0), rgb(swatch, 1.0));
    }
    let trails = scene::sidebar::groups(scene).iter().filter(|(g, _)| matches!(g, Group::Trail(_))).count();
    let n = counts(scene.all_nodes().count(), trails);
    let cs = type_::SMALL * ui;
    let nw = r.measure(&n, cs, false);
    let right = p.right() - pad - 4.0 * ui;
    chrome::text(&mut l, n, right - nw, mid, cs, rgb(dim, 1.0), sl, false);
    let nx = dot.right() + 10.0 * ui;
    let name_room = (right - nw - 12.0 * ui - nx).max(1.0);
    let name = fit(r, v.space, size, true, name_room - 18.0 * ui);
    let name_w = r.measure(&name, size, true);
    chrome::text(&mut l, name, nx, mid, size, rgb(fg, 1.0), Rect::new(nx, sl.y, name_room, sl.h), true);
    if space_hot {
        icons::draw(&mut l, r, Icon::ChevronDown, nx + name_w + 10.0 * ui, mid, 12.0 * ui, rgb(dim, 1.0));
    }

    // "New card": a row like the others, its key shown under the pointer.
    let base_x = p.x + pad + 4.0 * ui;
    let nr = panel.inner(panel.new_card());
    let hot = v.spot == Some(Spot::NewCard);
    if hot {
        chrome::fill(&mut l, nr, small, rgb(th.hover(), 1.0));
        let widths = [r.measure("Ctrl", ks, false), r.measure("T", ks, false)];
        chrome::draw_keycaps(&mut l, &["Ctrl", "T"], &widths, nr.right() - 8.0 * ui, nr.center().y, ks, th, ui, nr);
    }
    icons::draw(&mut l, r, Icon::Plus, base_x + icon / 2.0, nr.center().y, icon, rgb(if hot { fg } else { dim }, 1.0));
    let tx = base_x + icon + 10.0 * ui;
    chrome::text(&mut l, NEW_CARD, tx, nr.center().y, size, rgb(if hot { fg } else { dim }, 1.0), nr, false);

    // A hairline between the head and the list while rows are scrolled under it.
    if scroll > 0 {
        chrome::fill(&mut l, Rect::new(p.x + line, panel.list.y - line, p.w - 2.0 * line, line), 0.0, rgb(chrome::edge(th), 1.0));
    }

    if rows.is_empty() {
        // An empty board: say so.
        let mid = panel.list.y + panel.list.h * 0.32;
        let t = "No cards yet";
        let tw = r.measure(t, size, true);
        chrome::text(&mut l, t, p.x + (p.w - tw) / 2.0, mid, size, rgb(soft, 1.0), panel.list, true);
        let hint = "Pages you open gather here";
        let hw = r.measure(hint, cs, false);
        chrome::text(&mut l, hint, p.x + (p.w - hw) / 2.0, mid + 20.0 * ui, cs, rgb(dim, 1.0), panel.list, false);
    }

    let tab_fill = tab(th, surface);

    // The trails, under the rows: from each page down and into the pages it
    // opened, ending at their icons. The way to the current card goes on top,
    // in the accent; the rest is neutral, or the trail's colour once marked.
    let ups = parents(rows);
    let path = on_path(rows, &ups, v.current);
    let indent = INDENT * ui;
    let lw = (1.5 * ui).max(1.0);
    let list = panel.list;
    let shown = scroll..scroll + panel.fits(scroll);
    let marked = |g: Group| {
        rows.iter().find_map(|row| match row {
            Row::Header { group, mark, .. } if *group == g => mark.colour.as_deref().map(|c| th.color(c)),
            _ => None,
        })
    };
    let lines_in = if v.drop_line.is_none() { motion.rise(0, now) } else { 1.0 };
    let mut lines = Layer::default();
    for hot in [false, true] {
        for (i, up) in ups.iter().enumerate() {
            let (Some(pi), Row::Card { depth, group, .. }) = (*up, &rows[i]) else { continue };
            if path[i] != hot || (pi < shown.start && i < shown.start) || (pi >= shown.end && i >= shown.end) {
                continue;
            }
            let px = (base_x + indent * (*depth - 1) as f32 + icon / 2.0).round();
            let top = panel.row_rect(pi, scroll).map_or(list.y, |pr| pr.center().y + icon / 2.0 + 3.0 * ui);
            let colour = rgb(if hot { th.accent() } else { marked(*group).unwrap_or(th.line_strong()) }, if hot { 1.0 } else { 0.85 });
            let seg = |l: &mut Layer, a: [f32; 2], b: [f32; 2]| {
                // Kept to the list, so nothing is drawn over the head or foot.
                let (y0, y1) = (a[1].max(list.y), b[1].min(list.bottom()));
                if a[1] <= b[1] && y0 <= y1 {
                    let lerp = |y: f32| if (b[1] - a[1]).abs() < 0.01 { a[0] } else { a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) };
                    l.quads.push(Quad { shape: Shape::Segment { from: [lerp(y0), y0], to: [lerp(y1), y1], width: lw }, color: colour, texture: None });
                }
            };
            match panel.row_rect(i, scroll) {
                Some(cr) => {
                    // Down, then a short curve into the icon's left side.
                    let cy = cr.center().y;
                    let end = base_x + indent * *depth as f32 - 3.0 * ui;
                    let rr = (end - px).clamp(2.0 * ui, 6.0 * ui);
                    seg(&mut lines, [px, top], [px, cy - rr]);
                    let mut prev = [px, cy - rr];
                    for k in 1..=5 {
                        let a = k as f32 / 5.0 * std::f32::consts::FRAC_PI_2;
                        let pt = [px + rr * (1.0 - a.cos()), cy - rr * (1.0 - a.sin())];
                        seg(&mut lines, prev, pt);
                        prev = pt;
                    }
                    seg(&mut lines, prev, [end, cy]);
                }
                None => seg(&mut lines, [px, top], [px, list.bottom()]),
            }
        }
    }
    fade_tail(&mut lines, 0, 0, lines_in, Vec2::new(0.0, 0.0));
    l.quads.extend(lines.quads);

    // The current card's raised tab, gliding when the current card changes.
    let cur_row = rows.iter().position(|row| matches!(row, Row::Card { id, .. } if v.current == Some(*id)));
    let cur_rect = cur_row.and_then(|i| panel.row_rect(i, scroll));
    if let (Some(y), Some(cr)) = (motion.tab(v.current, cur_rect.map(|cr| cr.y), now), cur_rect) {
        let tab_r = panel.inner(Rect::new(cr.x, y, cr.w, cr.h));
        let clip_top = tab_r.y.max(list.y);
        let tab_r = Rect::new(tab_r.x, clip_top, tab_r.w, (tab_r.bottom().min(list.bottom()) - clip_top).max(0.0));
        if tab_r.h > 1.0 {
            chrome::lift(&mut l, tab_r, small, th, ui, Lift::Low);
            chrome::fill(&mut l, tab_r, small, rgb(tab_fill, 1.0));
            chrome::outline(&mut l, tab_r, small, line, rgb(th.line(), 0.8));
        }
    } else {
        motion.tab(v.current, None, now);
    }

    let mut k = 0usize; // rows drawn, for the stagger
    for (i, row) in rows.iter().enumerate() {
        let Some(rect) = panel.row_rect(i, scroll) else { continue };
        let (q0, t0) = (l.quads.len(), l.texts.len());
        let came = motion.rise(k, now);
        k += 1;
        if let Some((lr, label)) = panel.label_rect(i, scroll) {
            let ls = type_::CAPTION * ui;
            l.texts.push(Text {
                text: label.into(),
                pos: [base_x, lr.bottom() - chrome::line_h(ls) - 4.0 * ui],
                size: ls,
                color: rgb(faint, 1.0),
                clip: lr,
                bold: true,
            });
        }
        let inner = panel.inner(rect);
        let hovered = v.hover == Some(i);
        let selected = v.sel == Some(i);
        let current = matches!(row, Row::Card { id, .. } if v.current == Some(*id));
        if selected && current {
            // The highlight on the current card keeps the tab and rings it.
            chrome::outline(&mut l, inner, small, (1.5 * ui).max(1.0), rgb(th.accent(), 0.7));
        } else if selected {
            chrome::fill(&mut l, inner, small, rgb(th.accent_soft(), 1.0));
        } else if hovered && !current {
            chrome::fill(&mut l, inner, small, rgb(th.hover(), 1.0));
        }
        let right = inner.right() - 8.0 * ui;
        match row {
            Row::Header { group, label, mark, count, collapsed } => {
                // The trail's colour, as on its tag on the board, in the icon
                // column, so every label starts where the card titles do; the
                // cards on no trail get a hollow mark. Pointed at or
                // collapsed, a chevron says it folds.
                let cy = rect.center().y;
                if *collapsed || hovered || selected {
                    let chevron = if *collapsed { Icon::ChevronRight } else { Icon::ChevronDown };
                    icons::draw(&mut l, r, chevron, base_x + icon / 2.0, cy, 14.0 * ui, rgb(dim, 1.0));
                } else {
                    let d = 8.0 * ui;
                    let mark_rect = Rect::new(base_x + icon / 2.0 - d / 2.0, cy - d / 2.0, d, d);
                    let mr = small.min(d / 2.0);
                    match group {
                        Group::Trail(_) => chrome::fill(&mut l, mark_rect, mr, rgb(mark.colour.as_deref().map_or(th.text2(), |c| th.color(c)), 1.0)),
                        Group::Others => {
                            chrome::fill(&mut l, mark_rect, mr, rgb(dim, 1.0));
                            chrome::fill(&mut l, mark_rect.inflate(-1.5 * ui), mr, rgb(surface, 1.0));
                        }
                    }
                }
                let x = base_x + icon + 10.0 * ui;
                let n = count.to_string();
                let cs = type_::CAPTION * ui;
                let nw = r.measure(&n, cs, false);
                chrome::text(&mut l, n, right - nw, cy, cs, rgb(dim, 1.0), inner, false);
                let room = (right - nw - 8.0 * ui - x).max(1.0);
                let ls = type_::SMALL * ui;
                chrome::text(&mut l, fit(r, label, ls, true, room), x, cy, ls, rgb(soft, 1.0), Rect::new(x, rect.y, room, rect.h), true);
            }
            Row::Card { id, depth, folded, .. } => {
                let Some(n) = scene.node(*id) else { continue };
                let x = base_x + indent * *depth as f32;
                let iy = rect.y + (rect.h - icon) / 2.0;
                let tile = Rect::new(x, iy, icon, icon);
                if current {
                    // The current card's stop: ringed in the accent, as the card is on the board.
                    let ring = tile.inflate(3.0 * ui);
                    chrome::outline(&mut l, ring, small.min(ring.w / 2.0).max(3.0 * ui), lw, rgb(th.accent(), 0.9));
                }
                let fade = if *folded { 0.55 } else { 1.0 };
                if let Some(key) = (v.icon)(n) {
                    l.quads.push(Quad { shape: Shape::Rect { rect: tile, radius: 0.0 }, color: [1.0, 1.0, 1.0, fade], texture: Some(key) });
                } else if let Some(c) = n.sticky.as_deref().filter(|_| n.is_sticky()) {
                    // A sticky note: a small square of its paper.
                    chrome::fill(&mut l, tile.inflate(-2.0 * ui), small.min(2.0 * ui), rgb(th.sticky_paper(c), fade));
                } else if n.is_note() {
                    icons::draw(&mut l, r, Icon::Note, tile.center().x, tile.center().y, icon, rgb(dim, fade));
                } else {
                    // A site without an icon: a letter tile.
                    let text = letter(&n.url);
                    chrome::fill(&mut l, tile, small.min(3.0 * ui), rgb(th.active(), fade));
                    let ls = 10.0 * ui;
                    let lw = r.measure(&text, ls, true);
                    chrome::text(&mut l, text, tile.x + (icon - lw) / 2.0, tile.center().y, ls, rgb(soft, fade), tile, true);
                }
                // Under the pointer a card row offers to close it.
                let closing = hovered && v.drop_line.is_none();
                let end = if closing {
                    let c = panel.close(rect);
                    let on = v.spot == Some(Spot::Close(i));
                    if on {
                        chrome::fill(&mut l, c, small, rgb(blend(tab_fill, th.danger(), 0.3), 1.0));
                    }
                    icons::draw(&mut l, r, Icon::Close, c.center().x, c.center().y, 14.0 * ui, rgb(if on { fg } else { dim }, 1.0));
                    c.x - 4.0 * ui
                } else {
                    right
                };
                let tx = x + icon + 10.0 * ui;
                let title = if n.title.trim().is_empty() { n.url.as_str() } else { n.title.trim() };
                let title = fit(r, title, size, current, (end - tx).max(1.0));
                let colour = match (current || selected, *folded) {
                    (true, _) => rgb(fg, 1.0),
                    (false, true) => rgb(dim, 1.0),
                    (false, false) => rgb(soft, 1.0),
                };
                chrome::text(&mut l, title, tx, rect.center().y, size, colour, Rect::new(tx, rect.y, (end - tx).max(1.0), rect.h), current);
            }
        }
        // It comes in from the sheet's left edge.
        fade_tail(&mut l, q0, t0, came, Vec2::new(-10.0 * ui * (1.0 - came), 0.0));
    }

    if let Some(y) = v.drop_line {
        let h = (2.0 * ui).max(1.0);
        chrome::fill(&mut l, Rect::new(p.x + 6.0 * ui, y - h / 2.0, p.w - 12.0 * ui, h), 0.0, rgb(th.accent(), 1.0));
    }

    // A thin scroll bar when the list is longer than the sheet.
    let max = panel.max_scroll();
    if max > 0 {
        let track = panel.list.inflate(-4.0 * ui);
        let shown = panel.fits(scroll) as f32 / rows.len().max(1) as f32;
        let h = (track.h * shown).max(20.0 * ui);
        let y = track.y + (track.h - h) * scroll as f32 / max as f32;
        let w = (2.0 * ui).max(1.0);
        chrome::fill(&mut l, Rect::new(p.right() - w - 3.0 * ui, y, w, h), w / 2.0, rgb(th.line_strong(), 0.8));
    }
    // A hairline over the foot while rows go on below it.
    if scroll < max {
        chrome::fill(&mut l, Rect::new(p.x + line, panel.foot.y, p.w - 2.0 * line, line), 0.0, rgb(chrome::edge(th), 1.0));
    }

    // Foot: settings on the left, a dot per space (the current one larger
    // and ringed), "+" on the right; what the pointer is on is named above it.
    chrome::icon_button(&mut l, r, panel.settings(), Icon::Settings, press(Spot::Settings), false, true, th, ui);
    chrome::icon_button(&mut l, r, panel.plus(), Icon::Plus, press(Spot::NewSpace), false, true, th, ui);
    for (i, s) in v.spaces.iter().enumerate().take(panel.chips_shown(v.spaces.len())) {
        let cell = panel.chip(i);
        let on = i == v.active_space;
        if v.spot == Some(Spot::Space(i)) {
            chrome::fill(&mut l, cell, small, rgb(th.hover(), 1.0));
        }
        let colour = s.colour.unwrap_or(faint);
        let d = if on { DOT + 2.0 } else { DOT } * ui;
        let c = cell.center();
        let dot = Rect::new(c.x - d / 2.0, c.y - d / 2.0, d, d);
        let dr = small.min(d / 2.0);
        if on {
            let ring = dot.inflate(3.0 * ui);
            chrome::outline(&mut l, ring, small.min(ring.w / 2.0), (1.5 * ui).max(1.0), rgb(dim, 0.9));
        }
        chrome::fill(&mut l, dot, dr, rgb(colour, if on || v.spot == Some(Spot::Space(i)) { 1.0 } else { 0.75 }));
        if s.private {
            // A private space is hollow.
            chrome::fill(&mut l, dot.inflate(-(1.5 * ui).max(1.0)), dr, rgb(surface, 1.0));
        }
    }
    let tip = match v.spot {
        Some(Spot::Space(i)) => v.spaces.get(i).map(|s| (s.name.clone(), "", panel.chip(i))),
        Some(s @ (Spot::Settings | Spot::NewSpace)) => {
            let at = if s == Spot::Settings { panel.settings() } else { panel.plus() };
            Some((s.name().to_string(), if s == Spot::Settings { "Ctrl+," } else { "Ctrl+Shift+N" }, at))
        }
        _ => None,
    };
    // The keep button names itself and its key, below it.
    let tip = tip.map(|t| (t, true)).or_else(|| (v.spot == Some(Spot::Keep)).then(|| ((Spot::Keep.name().to_string(), "Ctrl+B", panel.nav(Spot::Keep)), false)));
    if let Some(((name, keys, at), above)) = tip {
        let t = chrome::tooltip(r, &name, keys, at, above, v.viewport, th, ui);
        l.quads.extend(t.quads);
        l.texts.extend(t.texts);
    }
    l
}

/// How long the pointer rests on the window's left edge before the sidebar peeks.
pub const PEEK_DWELL: Duration = Duration::from_millis(120);
/// How long the pointer is away from a peeking sidebar before it goes.
pub const PEEK_GRACE: Duration = Duration::from_millis(400);

/// What the edge peek asks for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeekAct {
    Open,
    Close,
}

/// The pointer resting on the window's left edge shows the sidebar without
/// the keys; once the pointer has been away from it a moment, it goes.
#[derive(Debug, Default, Clone)]
pub struct Peek {
    edge_since: Option<Instant>,
    away_since: Option<Instant>,
    /// The pointer has been seen off the edge: it peeks by moving onto the
    /// edge, not by resting there when the window opens under it.
    armed: bool,
}

impl Peek {
    /// `inside`: the pointer is in the window; `at_edge`: on its left edge
    /// and free to peek; `near`: over the sheet or close to it; `open`: the
    /// sidebar is showing, `peeking` as a peek.
    pub fn update(&mut self, now: Instant, inside: bool, at_edge: bool, near: bool, open: bool, peeking: bool) -> Option<PeekAct> {
        if !open {
            self.away_since = None;
            if !at_edge {
                self.edge_since = None;
                self.armed |= inside;
                return None;
            }
            if !self.armed {
                return None;
            }
            let since = *self.edge_since.get_or_insert(now);
            if now.duration_since(since) >= PEEK_DWELL {
                self.edge_since = None;
                self.armed = false;
                return Some(PeekAct::Open);
            }
            return None;
        }
        self.edge_since = None;
        if !near && inside {
            self.armed = true;
        }
        if !peeking || near {
            self.away_since = None;
            return None;
        }
        let since = *self.away_since.get_or_insert(now);
        if now.duration_since(since) >= PEEK_GRACE {
            self.away_since = None;
            return Some(PeekAct::Close);
        }
        None
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn cards_hang_from_the_row_one_level_up() {
        let g = Group::Trail(NodeId(1));
        let card = |id, depth| Row::Card { id: NodeId(id), group: g, depth, folded: false };
        let rows = vec![
            Row::Header { group: g, label: "t".into(), mark: TrailMark::default(), count: 5, collapsed: false },
            card(1, 0),
            card(2, 1),
            card(3, 2),
            card(4, 1),
            card(5, 2),
            Row::Card { id: NodeId(6), group: Group::Others, depth: 0, folded: false },
        ];
        let ups = parents(&rows);
        assert_eq!(ups, [None, None, Some(1), Some(2), Some(1), Some(4), None]);
        // The way to card 5: 1 → 4 → 5, not its cousins.
        let on = on_path(&rows, &ups, Some(NodeId(5)));
        assert_eq!(on, [false, true, false, false, true, true, false]);
        assert!(on_path(&rows, &ups, None).iter().all(|b| !b));
    }

    #[test]
    fn the_tab_glides_to_a_new_card_and_follows_a_scroll() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut m = Motion::default();
        assert_eq!(m.tab(Some(NodeId(1)), Some(100.0), t0), Some(100.0), "first shown: in place");
        assert_eq!(m.tab(Some(NodeId(1)), Some(132.0), ms(10)), Some(132.0), "a scroll: follows at once");
        assert_eq!(m.tab(Some(NodeId(2)), Some(232.0), ms(20)), Some(132.0), "a new card: starts from the old place");
        let mid = m.tab(Some(NodeId(2)), Some(232.0), ms(120)).unwrap();
        assert!(mid > 132.0 && mid < 232.0, "{mid}");
        assert_eq!(m.tab(Some(NodeId(2)), Some(232.0), ms(400)), Some(232.0));
        let mut still = Motion { still: true, ..Motion::default() };
        still.tab(Some(NodeId(1)), Some(10.0), t0);
        assert_eq!(still.tab(Some(NodeId(2)), Some(90.0), ms(1)), Some(90.0), "less motion: jumps");
    }

    use super::*;
    use scene::TrailMark;

    fn header(id: u64, pinned: bool) -> Row {
        Row::Header { group: Group::Trail(NodeId(id)), label: "t".into(), mark: TrailMark { pinned, ..Default::default() }, count: 2, collapsed: false }
    }
    fn card(id: u64) -> Row {
        Row::Card { id: NodeId(id), group: Group::Trail(NodeId(1)), depth: 0, folded: false }
    }

    #[test]
    fn groups_are_spaced_and_sections_labelled_only_with_pins() {
        let rows = [header(1, false), card(1), card(2), header(3, false), card(3)];
        let s = slots(&rows);
        assert_eq!(s[0].gap, 0.0);
        assert_eq!(s[1].gap, 0.0);
        assert_eq!(s[3].gap, GROUP_GAP);
        assert!(s.iter().all(|s| s.label.is_none()), "no labels without pins");
        let rows = [header(1, true), card(1), header(3, false), card(3), header(5, false)];
        let s = slots(&rows);
        assert_eq!(s[0].label, Some(PINNED));
        assert_eq!(s[0].gap, LABEL_H);
        assert_eq!(s[2].label, Some(TRAILS));
        assert_eq!(s[2].gap, GROUP_GAP + LABEL_H);
        assert_eq!(s[4].label, None);
    }

    fn panel(rows: &[Row], list_h: f32) -> Panel {
        let vp = Vec2::new(1200.0, list_h + HEAD_H + FOOT_H + 2.0 * MARGIN);
        Panel::new(vp, 1.0, &slots(rows))
    }

    #[test]
    fn rows_are_laid_out_with_gaps_and_hit_where_drawn() {
        let rows = [header(1, false), card(1), header(3, false), card(3)];
        let p = panel(&rows, 500.0);
        assert_eq!(p.rect, Rect::new(MARGIN, MARGIN, WIDTH, 500.0 + HEAD_H + FOOT_H));
        let top = MARGIN + HEAD_H;
        assert_eq!(p.row_rect(0, 0).unwrap().y, top);
        assert_eq!(p.row_rect(1, 0).unwrap().y, top + HEADER_ROW_H);
        assert_eq!(p.row_rect(2, 0).unwrap().y, top + HEADER_ROW_H + ROW_H + GROUP_GAP);
        assert_eq!(p.row_at(Vec2::new(50.0, top + HEADER_ROW_H + 3.0), 0), Some(1));
        // The gap between groups is no row.
        assert_eq!(p.row_at(Vec2::new(50.0, top + HEADER_ROW_H + ROW_H + 2.0), 0), None);
        assert_eq!(p.row_at(Vec2::new(50.0, MARGIN + 10.0), 0), None, "the head");
        assert_eq!(p.row_at(Vec2::new(WIDTH + MARGIN + 1.0, top + 3.0), 0), None, "the board");
        // Scrolled to the second group: its gap is dropped at the top.
        assert_eq!(p.row_rect(2, 2).unwrap().y, top);
    }

    #[test]
    fn scrolling_keeps_the_highlight_whole_and_stops_at_the_end() {
        let rows: Vec<Row> = (0..20).map(|i| if i % 5 == 0 { header(i, false) } else { card(i) }).collect();
        let p = panel(&rows, 6.0 * ROW_H + 5.0);
        assert!(p.fits(0) < rows.len());
        let s = p.keep_visible(Some(12), 0);
        assert!(p.row_rect(12, s).is_some(), "row 12 shown at scroll {s}");
        assert!(p.row_rect(11, s - 1).is_none() || s == 0 || p.row_rect(12, s - 1).is_none(), "no further than needed");
        assert_eq!(p.keep_visible(Some(3), s), 3);
        let max = p.max_scroll();
        assert!(p.row_rect(19, max).is_some());
        assert!(p.row_rect(19, max - 1).is_none());
        assert_eq!(p.keep_visible(None, 99), max);
        assert_eq!(p.scroll_by(max, 5), max);
        assert_eq!(p.scroll_by(2, -5), 0);
        // A short list never scrolls.
        let p = panel(&rows[..3], 500.0);
        assert_eq!(p.max_scroll(), 0);
    }

    #[test]
    fn section_labels_sit_above_their_header() {
        let rows = [header(1, true), card(1), header(3, false)];
        let p = panel(&rows, 500.0);
        let (lr, text) = p.label_rect(2, 0).unwrap();
        assert_eq!(text, TRAILS);
        assert_eq!(lr.bottom(), p.row_rect(2, 0).unwrap().y);
        assert!(p.label_rect(1, 0).is_none());
    }

    #[test]
    fn foot_and_head_buttons_hit_where_drawn() {
        let p = panel(&[header(1, false), card(1)], 300.0);
        let a = p.chip(0);
        let b = p.chip(1);
        assert_eq!(b.x - a.x, CHIP + CHIP_GAP);
        assert!(p.foot.contains(a.center()));
        let none = |_: usize| false;
        assert_eq!(p.spot_at(a.center(), 2, 0, &none), Some(Spot::Space(0)));
        assert_eq!(p.spot_at(Vec2::new(b.x + 1.0, b.center().y), 2, 0, &none), Some(Spot::Space(1)));
        assert_eq!(p.spot_at(p.chip(2).center(), 2, 0, &none), None, "past the last space");
        assert_eq!(p.spot_at(p.plus().center(), 2, 0, &none), Some(Spot::NewSpace));
        assert_eq!(p.spot_at(p.settings().center(), 2, 0, &none), Some(Spot::Settings));
        assert!(p.chips_shown(50) < 50, "dots stop before the plus");
        assert!(p.chip(p.chips_shown(50) - 1).right() <= p.plus().x);
        for s in [Spot::Back, Spot::Forward, Spot::Reload, Spot::Keep] {
            assert_eq!(p.spot_at(p.nav(s).center(), 2, 0, &none), Some(s));
        }
        assert_eq!(p.spot_at(p.address().center(), 2, 0, &none), Some(Spot::Address));
        assert_eq!(p.spot_at(p.space_line().center(), 2, 0, &none), Some(Spot::SpaceName));
        assert_eq!(p.spot_at(p.new_card().center(), 2, 0, &none), Some(Spot::NewCard));
        assert!(p.new_card().bottom() <= p.list.y, "the head fits above the list");
        assert!(p.address().right() < p.rect.right());
    }

    #[test]
    fn a_card_row_closes_from_its_button_only() {
        let p = panel(&[header(1, false), card(1)], 300.0);
        let row = p.row_rect(1, 0).unwrap();
        let x = p.close(row);
        assert!(row.contains(x.center()) && x.right() <= row.right());
        let cards = |i: usize| i == 1;
        assert_eq!(p.spot_at(x.center(), 1, 0, &cards), Some(Spot::Close(1)));
        assert_eq!(p.spot_at(row.center(), 1, 0, &cards), None, "the rest of the row picks the card");
        let header_row = p.row_rect(0, 0).unwrap();
        assert_eq!(p.spot_at(p.close(header_row).center(), 1, 0, &cards), None, "headers have no close");
    }

    #[test]
    fn a_narrow_window_keeps_most_of_the_board() {
        let r = Panel::sheet(Vec2::new(500.0, 400.0), 1.0);
        assert_eq!(r.w, (500.0 * MAX_SHARE).round());
        let p = Panel::new(Vec2::new(500.0, 60.0), 1.0, &slots(&[header(1, false)]));
        assert!(p.fits(0) >= 1);
    }

    #[test]
    fn an_empty_list_has_no_rows_to_hit() {
        let p = Panel::new(Vec2::new(1000.0, 800.0), 1.0, &[]);
        assert_eq!(p.row_rect(0, 0), None);
        assert_eq!(p.label_rect(0, 0), None);
        assert_eq!(p.row_at(Vec2::new(p.list.x + 10.0, p.list.y + 10.0), 0), None);
        assert_eq!(p.keep_visible(Some(0), 3), 0);
    }

    #[test]
    fn letters_and_counts() {
        assert_eq!(letter("https://www.wikipedia.org/wiki/X"), "W");
        assert_eq!(letter("https://123.example/"), "1");
        assert_eq!(letter("trailboard:note"), "·");
        assert_eq!(pill_host("https://www.wikipedia.org/wiki/X"), "wikipedia.org");
        assert_eq!(counts(0, 0), "Empty");
        assert_eq!(counts(1, 0), "1 card");
        assert_eq!(counts(5, 1), "5 cards · 1 trail");
        assert_eq!(counts(7, 3), "7 cards · 3 trails");
    }

    #[test]
    fn peek_waits_on_the_edge_and_lingers_after_the_pointer_leaves() {
        let mut p = Peek::default();
        let t = Instant::now();
        let ms = |n: u64| t + Duration::from_millis(n);
        // Outside the window nothing arms it.
        assert_eq!(p.update(t, false, false, false, false, false), None);
        // Resting on the edge from the start does nothing.
        assert_eq!(p.update(t, true, true, false, false, false), None);
        assert_eq!(p.update(ms(500), true, true, false, false, false), None);
        let t = ms(1000);
        let ms = |n: u64| t + Duration::from_millis(n);
        assert_eq!(p.update(t, true, false, false, false, false), None);
        assert_eq!(p.update(t, true, true, false, false, false), None);
        assert_eq!(p.update(ms(60), true, true, false, false, false), None);
        // Leaving the edge early starts the wait over.
        assert_eq!(p.update(ms(70), true, false, false, false, false), None);
        assert_eq!(p.update(ms(100), true, true, false, false, false), None);
        assert_eq!(p.update(ms(200), true, true, false, false, false), None);
        assert_eq!(p.update(ms(230), true, true, false, false, false), Some(PeekAct::Open));
        // Open as a peek: stays while near, goes after the grace once away.
        assert_eq!(p.update(ms(300), true, false, true, true, true), None);
        assert_eq!(p.update(ms(400), true, false, false, true, true), None);
        assert_eq!(p.update(ms(600), true, false, true, true, true), None, "came back");
        assert_eq!(p.update(ms(700), true, false, false, true, true), None);
        assert_eq!(p.update(ms(1099), true, false, false, true, true), None);
        assert_eq!(p.update(ms(1100), true, false, false, true, true), Some(PeekAct::Close));
        // Opened with Ctrl+B, it never goes by itself.
        assert_eq!(p.update(ms(2000), true, false, false, true, false), None);
        assert_eq!(p.update(ms(9000), true, false, false, true, false), None);
    }
}
