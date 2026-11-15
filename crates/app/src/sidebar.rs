//! The trails sidebar's panel (B, or the pointer resting on the window's left
//! edge): layout, hit testing and drawing of the rows from `scene::sidebar`.
//!
//! Our own design (M11 step 4, 2026-11-15). The panel is a sheet floating a
//! little in from the window's left edge, over the board, under the address
//! bar, menus, toasts and the shortcut sheet. Groups are set apart by space,
//! not lines; the pinned trails and the rest get small section labels when
//! both are there. Cards show their site's icon (a letter tile without one)
//! and hang from their trail with faint guide lines. The space's name heads
//! the sheet and the spaces sit as chips along its foot. Controls stay quiet
//! (a group's chevron shows only when collapsed or pointed at). Corners
//! follow the theme's rounding, like every other surface.

use std::time::{Duration, Instant};

use compositor::{Layer, Quad, Renderer, Shape, Text};
use scene::sidebar::{Group, Row};
use scene::{Node, NodeId, Rect, Scene, Vec2};

use crate::theme::{Rgb, Theme, mix, rgb};

/// Sizes in logical px; multiplied by the UI scale.
pub const WIDTH: f32 = 280.0;
/// Gap between the sheet and the window's edges.
pub const MARGIN: f32 = 8.0;
/// The space's name and the counts above the list.
pub const HEAD_H: f32 = 64.0;
/// The space chips below the list.
pub const FOOT_H: f32 = 46.0;
pub const PAD: f32 = 12.0;
pub const ROW_H: f32 = 28.0;
pub const HEADER_ROW_H: f32 = 30.0;
/// Space above every group but the first.
pub const GROUP_GAP: f32 = 10.0;
/// A section label ("Pinned", "Trails") above a group.
pub const LABEL_H: f32 = 22.0;
pub const INDENT: f32 = 16.0;
pub const FONT: f32 = 12.5;
pub const ICON: f32 = 14.0;
/// A space chip and the room between chips.
pub const CHIP: f32 = 12.0;
pub const CHIP_GAP: f32 = 12.0;
/// The panel never takes more than this share of the window's width.
pub const MAX_SHARE: f32 = 0.45;

/// Section labels, shown only when the list has both parts.
pub const PINNED: &str = "PINNED";
pub const TRAILS: &str = "TRAILS";

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
    /// The space chips' strip.
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
        let head = HEAD_H * ui;
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

    /// The chip of space `i` of `n` (and `n` is the "new space" chip).
    pub fn chip(&self, i: usize) -> Rect {
        let c = CHIP * self.ui;
        let x = self.foot.x + PAD * self.ui + i as f32 * (c + CHIP_GAP * self.ui);
        Rect::new(x, self.foot.y + (self.foot.h - c) / 2.0, c, c)
    }

    /// The chip under `p` among `n` spaces and the "new space" chip (index `n`).
    pub fn chip_at(&self, p: Vec2, n: usize) -> Option<usize> {
        if !self.foot.contains(p) {
            return None;
        }
        let half = CHIP_GAP * self.ui / 2.0;
        (0..=n).find(|&i| {
            let c = self.chip(i);
            Rect::new(c.x - half, self.foot.y, c.w + 2.0 * half, self.foot.h).contains(p)
        })
    }
}

/// A space, for its chip.
pub struct SpaceChip {
    pub name: String,
    /// Its tint, else `None` for the theme's plain board.
    pub colour: Option<Rgb>,
    pub private: bool,
}

/// How the list is shown: scrolled to `scroll`, with the keyboard highlight,
/// the row under the pointer and the current card marked.
pub struct View<'a> {
    pub scroll: usize,
    pub sel: Option<usize>,
    pub hover: Option<usize>,
    pub current: Option<NodeId>,
    /// While a trail's header is dragged: y of the line where it would go.
    pub drop_line: Option<f32>,
    pub space: &'a str,
    pub spaces: &'a [SpaceChip],
    pub active_space: usize,
    /// The chip under the pointer (`spaces.len()` for "new space").
    pub hover_chip: Option<usize>,
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

/// "5 cards · 2 trails".
pub fn counts(cards: usize, trails: usize) -> String {
    let s = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    match trails {
        _ if cards == 0 => "Empty".into(),
        0 => s(cards, "card", "cards"),
        t => format!("{} · {}", s(cards, "card", "cards"), s(t, "trail", "trails")),
    }
}

/// `text` cut with "…" to fit `max_w` px at `size` (the renderer wraps
/// what doesn't fit, so every single-line label goes through this).
pub fn fit(r: &mut Renderer, text: &str, size: f32, bold: bool, max_w: f32) -> String {
    if r.measure(text, size, bold) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let t: String = chars[..mid].iter().collect::<String>().trim_end().to_string() + "…";
        if r.measure(&t, size, bold) <= max_w { lo = mid } else { hi = mid - 1 }
    }
    let cut: String = chars[..lo].iter().collect();
    format!("{}…", cut.trim_end())
}

/// A one-colour rect.
fn fill(l: &mut Layer, rect: Rect, radius: f32, colour: [f32; 4]) {
    l.quads.push(Quad { shape: Shape::Rect { rect, radius }, color: colour, texture: None });
}

/// The panel with `rows`, as `v` says.
pub fn layer(r: &mut Renderer, panel: &Panel, rows: &[Row], v: &View, scene: &Scene, th: &Theme, ui: f32) -> Layer {
    let scroll = v.scroll;
    let mut l = Layer::default();
    let p = panel.rect;
    let line = ui.max(1.0);
    let radius = th.radius(ui / th.text_scale);
    let small = th.small_radius(ui / th.text_scale);
    let surface = th.panel();
    crate::chrome::panel(&mut l, p, radius, th, ui, crate::chrome::Lift::Mid, surface);

    let fg = th.foreground();
    let dim = th.dim_text();
    let soft = mix(fg, dim, 0.3);
    let size = FONT * ui;
    let pad = PAD * ui;
    let text_h = |s: f32| s * 1.3;

    // Head: the space's name, the counts under it, the key on the right.
    let name_size = 15.0 * ui;
    let sub_size = 11.5 * ui;
    let head = Rect::new(p.x, p.y, p.w, panel.list.y - p.y);
    let block = text_h(name_size) + 2.0 * ui + text_h(sub_size);
    let top = head.y + (head.h - block) / 2.0;
    let name_clip = Rect::new(p.x + pad, head.y, p.w - 2.0 * pad - 40.0 * ui, head.h);
    let name = fit(r, v.space, name_size, true, name_clip.w);
    l.texts.push(Text { text: name, pos: [p.x + pad, top], size: name_size, color: rgb(fg, 1.0), clip: name_clip, bold: true });
    let trails = scene::sidebar::groups(scene).iter().filter(|(g, _)| matches!(g, Group::Trail(_))).count();
    l.texts.push(Text {
        text: counts(scene.all_nodes().count(), trails),
        pos: [p.x + pad, top + text_h(name_size) + 2.0 * ui],
        size: sub_size,
        color: rgb(dim, 1.0),
        clip: name_clip,
        bold: false,
    });
    let key_size = 11.0 * ui;
    let kw = r.measure("B", key_size, false);
    crate::chrome::draw_keycaps(&mut l, &["B"], &[kw], p.right() - pad, top + text_h(name_size) / 2.0, key_size, th, ui, head);
    // A hairline under the head only while rows are scrolled under it.
    if scroll > 0 {
        fill(&mut l, Rect::new(p.x, panel.list.y - line, p.w, line), 0.0, rgb(crate::chrome::edge(th), 1.0));
    }

    if rows.is_empty() {
        // An empty board: say so, and how to start.
        let mid = panel.list.y + panel.list.h * 0.38;
        let t = "No cards yet";
        let tw = r.measure(t, size, true);
        l.texts.push(Text { text: t.into(), pos: [p.x + (p.w - tw) / 2.0, mid - text_h(size)], size, color: rgb(soft, 1.0), clip: panel.list, bold: true });
        let hint = "opens a page";
        let ks = 10.5 * ui;
        let widths = [r.measure("Ctrl", ks, false), r.measure("T", ks, false)];
        let hw = r.measure(hint, sub_size, false);
        let total = crate::chrome::keycaps_width(&widths, ks) + 6.0 * ui + hw;
        let x0 = p.x + (p.w - total) / 2.0;
        let y = mid + 14.0 * ui;
        crate::chrome::draw_keycaps(&mut l, &["Ctrl", "T"], &widths, x0 + crate::chrome::keycaps_width(&widths, ks), y, ks, th, ui, panel.list);
        l.texts.push(Text {
            text: hint.into(),
            pos: [x0 + total - hw, y - text_h(sub_size) / 2.0],
            size: sub_size,
            color: rgb(dim, 1.0),
            clip: panel.list,
            bold: false,
        });
    }

    let hover_fill = mix(surface, fg, if th.light() { 0.05 } else { 0.06 });
    let current_fill = mix(surface, fg, if th.light() { 0.08 } else { 0.10 });
    let inset = 6.0 * ui;
    let icon = ICON * ui;
    // Where a card's icon starts at depth 0: under its trail's colour mark.
    let base_x = p.x + pad + 12.0 * ui;
    let text_y = |row: Rect, s: f32| row.y + (row.h - text_h(s)) / 2.0;

    for (i, row) in rows.iter().enumerate() {
        let Some(rect) = panel.row_rect(i, scroll) else { continue };
        if let Some((lr, label)) = panel.label_rect(i, scroll) {
            let ls = 10.0 * ui;
            l.texts.push(Text {
                text: label.into(),
                pos: [p.x + pad, lr.bottom() - text_h(ls) - 3.0 * ui],
                size: ls,
                color: rgb(dim, 0.85),
                clip: lr,
                bold: true,
            });
        }
        let inner = Rect::new(rect.x + inset, rect.y + 1.0 * ui, rect.w - 2.0 * inset, rect.h - 2.0 * ui);
        let hovered = v.hover == Some(i);
        let selected = v.sel == Some(i);
        let current = matches!(row, Row::Card { id, .. } if v.current == Some(*id));
        if selected {
            fill(&mut l, inner, small, rgb(th.selection(), 0.9));
        } else if current {
            fill(&mut l, inner, small, rgb(current_fill, 1.0));
        } else if hovered {
            fill(&mut l, inner, small, rgb(hover_fill, 1.0));
        }
        if current {
            // The current card: a short accent bar on the sheet's inner edge.
            let bar = Rect::new(p.x + 2.0 * ui, rect.y + rect.h * 0.22, (2.0 * ui).max(1.0), rect.h * 0.56);
            fill(&mut l, bar, 0.0, rgb(th.accent(), 1.0));
        }
        let right = inner.right() - 6.0 * ui;
        match row {
            Row::Header { group, label, mark, count, collapsed } => {
                // A chevron only when it says something: collapsed, or pointed at.
                if *collapsed || hovered || selected {
                    let cs = 10.0 * ui;
                    let chevron = if *collapsed { "▸" } else { "▾" };
                    l.texts.push(Text { text: chevron.into(), pos: [p.x + pad - 2.0 * ui, text_y(rect, cs)], size: cs, color: rgb(dim, 1.0), clip: inner, bold: false });
                }
                // The trail's colour, as on its tag on the board, in the icon
                // column, so every label starts where the card titles do; the
                // cards on no trail get a hollow mark.
                let d = 8.0 * ui;
                let mark_rect = Rect::new(base_x + icon / 2.0 - d / 2.0, rect.y + (rect.h - d) / 2.0, d, d);
                let mr = small.min(d / 2.0);
                match group {
                    Group::Trail(_) => fill(&mut l, mark_rect, mr, rgb(th.trail(mark.colour.as_deref()), 1.0)),
                    Group::Others => {
                        fill(&mut l, mark_rect, mr, rgb(dim, 1.0));
                        fill(&mut l, mark_rect.inflate(-line), mr, rgb(surface, 1.0));
                    }
                }
                let x = base_x + icon + 8.0 * ui;
                let n = count.to_string();
                let cs = 11.0 * ui;
                let nw = r.measure(&n, cs, false);
                l.texts.push(Text { text: n, pos: [right - nw, text_y(rect, cs)], size: cs, color: rgb(dim, 1.0), clip: inner, bold: false });
                let room = (right - nw - 8.0 * ui - x).max(1.0);
                l.texts.push(Text {
                    text: fit(r, label, size, true, room),
                    pos: [x, text_y(rect, size)],
                    size,
                    color: rgb(if selected { fg } else { mix(fg, soft, 0.2) }, 1.0),
                    clip: Rect::new(x, rect.y, (right - nw - 8.0 * ui - x).max(1.0), rect.h),
                    bold: true,
                });
            }
            Row::Card { id, depth, folded, group } => {
                let Some(n) = scene.node(*id) else { continue };
                let indent = INDENT * ui;
                let x = base_x + indent * *depth as f32;
                // Faint guides from each level of the trail down past this card.
                if matches!(group, Group::Trail(_)) {
                    for level in 0..*depth {
                        let gx = (base_x + indent * level as f32 + icon / 2.0).round();
                        fill(&mut l, Rect::new(gx, rect.y, line, rect.h), 0.0, rgb(fg, if th.light() { 0.12 } else { 0.10 }));
                    }
                }
                let iy = rect.y + (rect.h - icon) / 2.0;
                let tile = Rect::new(x, iy, icon, icon);
                let fade = if *folded { 0.55 } else { 1.0 };
                if let Some(key) = (v.icon)(n) {
                    l.quads.push(Quad { shape: Shape::Rect { rect: tile, radius: 0.0 }, color: [1.0, 1.0, 1.0, fade], texture: Some(key) });
                } else if let Some(c) = n.sticky.as_deref().filter(|_| n.is_sticky()) {
                    // A sticky note: a small square of its paper.
                    fill(&mut l, tile.inflate(-1.5 * ui), small.min(2.0 * ui), rgb(th.sticky_paper(c), fade));
                } else {
                    // A note, or a site without an icon: a letter tile.
                    let (text, face) = if n.is_note() { ("¶".to_string(), mix(surface, th.accent(), 0.22)) } else { (letter(&n.url), mix(surface, fg, 0.14)) };
                    fill(&mut l, tile, small.min(3.0 * ui), rgb(face, fade));
                    let ls = 9.5 * ui;
                    let lw = r.measure(&text, ls, true);
                    l.texts.push(Text { text, pos: [tile.x + (icon - lw) / 2.0, tile.y + (icon - text_h(ls)) / 2.0], size: ls, color: rgb(soft, fade), clip: tile, bold: true });
                }
                let tx = x + icon + 8.0 * ui;
                let title = if n.title.trim().is_empty() { n.url.as_str() } else { n.title.trim() };
                let title = fit(r, title, size, current, (right - tx).max(1.0));
                let colour = match (current || selected, *folded) {
                    (true, _) => rgb(fg, 1.0),
                    (false, true) => rgb(dim, 1.0),
                    (false, false) => rgb(soft, 1.0),
                };
                l.texts.push(Text {
                    text: title,
                    pos: [tx, text_y(rect, size)],
                    size,
                    color: colour,
                    clip: Rect::new(tx, rect.y, (right - tx).max(1.0), rect.h),
                    bold: current,
                });
            }
        }
    }

    if let Some(y) = v.drop_line {
        let h = (2.0 * ui).max(1.0);
        fill(&mut l, Rect::new(p.x + inset, y - h / 2.0, p.w - 2.0 * inset, h), 0.0, rgb(th.accent(), 1.0));
    }

    // A thin scroll bar when the list is longer than the sheet.
    let max = panel.max_scroll();
    if max > 0 {
        let track = panel.list.inflate(-4.0 * ui);
        let shown = panel.fits(scroll) as f32 / rows.len().max(1) as f32;
        let h = (track.h * shown).max(20.0 * ui);
        let y = track.y + (track.h - h) * scroll as f32 / max as f32;
        let w = (2.0 * ui).max(1.0);
        fill(&mut l, Rect::new(p.right() - w - 3.0 * ui, y, w, h), w / 2.0, rgb(dim, 0.45));
    }
    // A hairline over the foot while rows go on below it.
    if scroll < max {
        fill(&mut l, Rect::new(p.x, panel.foot.y, p.w, line), 0.0, rgb(crate::chrome::edge(th), 1.0));
    }

    // Foot: one chip per space, the current one ringed, then "+" for a new one;
    // the name of the chip under the pointer beside them.
    let n = v.spaces.len();
    for (i, s) in v.spaces.iter().enumerate() {
        let c = panel.chip(i);
        let colour = s.colour.unwrap_or_else(|| mix(surface, fg, 0.35));
        if i == v.active_space {
            let ring = c.inflate(3.0 * ui);
            fill(&mut l, ring.inflate(line), small, rgb(dim, 0.9));
            fill(&mut l, ring, small, rgb(surface, 1.0));
        }
        if s.private {
            // A private space is hollow.
            fill(&mut l, c, small.min(3.0 * ui), rgb(colour, 1.0));
            let w = (1.5 * ui).max(1.0);
            fill(&mut l, c.inflate(-w), small.min(3.0 * ui), rgb(surface, 1.0));
        } else {
            fill(&mut l, c, small.min(3.0 * ui), rgb(colour, if v.hover_chip == Some(i) || i == v.active_space { 1.0 } else { 0.8 }));
        }
    }
    let plus = panel.chip(n);
    let ps = 15.0 * ui;
    let pw = r.measure("+", ps, false);
    l.texts.push(Text {
        text: "+".into(),
        pos: [plus.x + (plus.w - pw) / 2.0, plus.y + (plus.h - text_h(ps)) / 2.0],
        size: ps,
        color: rgb(if v.hover_chip == Some(n) { fg } else { dim }, 1.0),
        clip: panel.foot,
        bold: false,
    });
    let tip = match v.hover_chip {
        Some(i) if i < n => Some(v.spaces[i].name.clone()),
        Some(_) => Some("New space".to_string()),
        None => None,
    };
    if let Some(tip) = tip {
        let ts = 11.5 * ui;
        let x = plus.right() + 12.0 * ui;
        let tip = fit(r, &tip, ts, false, (p.right() - pad - x).max(1.0));
        l.texts.push(Text {
            text: tip,
            pos: [x, panel.foot.y + (panel.foot.h - text_h(ts)) / 2.0],
            size: ts,
            color: rgb(soft, 1.0),
            clip: Rect::new(x, panel.foot.y, (p.right() - pad - x).max(1.0), panel.foot.h),
            bold: false,
        });
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
    fn chips_line_up_in_the_foot_and_hit_with_room_around() {
        let p = panel(&[header(1, false)], 300.0);
        let a = p.chip(0);
        let b = p.chip(1);
        assert_eq!(b.x - a.x, CHIP + CHIP_GAP);
        assert!(p.foot.contains(a.center()));
        assert_eq!(p.chip_at(a.center(), 2), Some(0));
        assert_eq!(p.chip_at(Vec2::new(b.x - 3.0, b.center().y), 2), Some(1));
        assert_eq!(p.chip_at(p.chip(2).center(), 2), Some(2), "the new-space chip");
        assert_eq!(p.chip_at(p.chip(3).center(), 2), None);
        assert_eq!(p.chip_at(Vec2::new(a.center().x, p.list.y + 5.0), 2), None);
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
        // Opened with B, it never goes by itself.
        assert_eq!(p.update(ms(2000), true, false, false, true, false), None);
        assert_eq!(p.update(ms(9000), true, false, false, true, false), None);
    }
}
