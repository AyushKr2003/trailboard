//! The trails sidebar's panel (B): layout, hit testing and drawing of the rows
//! from `scene::sidebar`. The panel covers the left edge of the board, full
//! height, under the address bar, menus, toasts and the shortcut sheet.

use compositor::{Layer, Quad, Renderer, Shape, Text};
use scene::sidebar::{Group, Row};
use scene::{NodeId, Rect, Scene, Vec2};

use crate::theme::{rgb, Theme};

/// Sizes in logical px; multiplied by the UI scale.
pub const WIDTH: f32 = 300.0;
pub const TITLE_H: f32 = 44.0;
pub const ROW_H: f32 = 26.0;
pub const PAD: f32 = 10.0;
pub const INDENT: f32 = 14.0;
pub const FONT: f32 = 13.0;
/// The panel never takes more than this share of the window's width.
pub const MAX_SHARE: f32 = 0.45;

/// Where the panel and its rows are, in screen px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub rect: Rect,
    pub rows_top: f32,
    pub row_h: f32,
    /// Number of whole rows that fit.
    pub shown: usize,
}

impl Panel {
    pub fn new(viewport: Vec2, ui: f32) -> Panel {
        let w = (WIDTH * ui).min(viewport.x * MAX_SHARE).round();
        let rows_top = TITLE_H * ui;
        let row_h = ROW_H * ui;
        let shown = ((viewport.y - rows_top - PAD * ui) / row_h).floor().max(1.0) as usize;
        Panel { rect: Rect::new(0.0, 0.0, w, viewport.y), rows_top, row_h, shown }
    }

    /// Rect of row `index` with the list scrolled to `scroll`, if it is shown.
    pub fn row_rect(&self, index: usize, scroll: usize) -> Option<Rect> {
        let at = index.checked_sub(scroll).filter(|&i| i < self.shown)?;
        Some(Rect::new(self.rect.x, self.rows_top + at as f32 * self.row_h, self.rect.w, self.row_h))
    }

    /// Index of the row at `p` among `len` rows scrolled to `scroll`.
    pub fn row_at(&self, p: Vec2, scroll: usize, len: usize) -> Option<usize> {
        if !self.rect.contains(p) || p.y < self.rows_top {
            return None;
        }
        let i = scroll + ((p.y - self.rows_top) / self.row_h) as usize;
        (i < len && i < scroll + self.shown).then_some(i)
    }
}

/// How the list is shown: scrolled to `scroll`, with the keyboard highlight,
/// the row under the pointer and the current card marked.
pub struct View {
    pub scroll: usize,
    pub sel: Option<usize>,
    pub hover: Option<usize>,
    pub current: Option<NodeId>,
}

/// The panel with `rows`, as `marks` says.
pub fn layer(r: &mut Renderer, panel: &Panel, rows: &[Row], marks: &View, scene: &Scene, th: &Theme, ui: f32) -> Layer {
    let scroll = marks.scroll;
    let mut l = Layer::default();
    let p = panel.rect;
    let line = ui.max(1.0);
    l.quads.push(Quad {
        shape: Shape::Shadow { rect: p.translate(Vec2::new(4.0 * ui, 0.0)), radius: 0.0, blur: 18.0 * ui },
        color: th.shadow(0.45),
        texture: None,
    });
    l.quads.push(Quad { shape: Shape::Rect { rect: p, radius: 0.0 }, color: rgb(th.panel(), 1.0), texture: None });
    l.quads.push(Quad {
        shape: Shape::Rect { rect: Rect::new(p.right() - line, p.y, line, p.h), radius: 0.0 },
        color: rgb(th.divider(), 1.0),
        texture: None,
    });

    let size = FONT * ui;
    let pad = PAD * ui;
    let text_y = |row: Rect| row.y + (row.h - size * 1.3) / 2.0;
    // Title: "Trails", the key that closes it on the right.
    let title = Rect::new(p.x, p.y, p.w, panel.rows_top);
    let tsize = 15.0 * ui;
    l.texts.push(Text {
        text: "Trails".into(),
        pos: [p.x + pad + 4.0 * ui, title.y + (title.h - tsize * 1.3) / 2.0],
        size: tsize,
        color: rgb(th.foreground(), 1.0),
        clip: title,
        bold: true,
    });
    let hint = "B · Esc";
    let hw = r.measure(hint, size, false);
    l.texts.push(Text {
        text: hint.into(),
        pos: [p.right() - pad - hw - 4.0 * ui, title.y + (title.h - size * 1.3) / 2.0],
        size,
        color: rgb(th.dim_text(), 1.0),
        clip: title,
        bold: false,
    });

    if rows.is_empty() {
        let row = Rect::new(p.x, panel.rows_top, p.w, panel.row_h);
        l.texts.push(Text {
            text: "No cards yet".into(),
            pos: [p.x + pad + 4.0 * ui, text_y(row)],
            size,
            color: rgb(th.dim_text(), 1.0),
            clip: row,
            bold: false,
        });
    }

    for (i, row) in rows.iter().enumerate() {
        let Some(rect) = panel.row_rect(i, scroll) else { continue };
        let inner = Rect::new(rect.x + pad * 0.5, rect.y + 1.0 * ui, rect.w - pad - line, rect.h - 2.0 * ui);
        if marks.sel == Some(i) || marks.hover == Some(i) {
            let alpha = if marks.sel == Some(i) { 1.0 } else { 0.55 };
            l.quads.push(Quad {
                shape: Shape::Rect { rect: inner, radius: th.small_radius(ui / th.text_scale) },
                color: rgb(th.selection(), alpha),
                texture: None,
            });
        }
        let right = inner.right() - pad * 0.5;
        match row {
            Row::Header { group, label, mark, count, collapsed } => {
                let chevron = if *collapsed { "▸" } else { "▾" };
                let mut x = inner.x + pad * 0.5;
                l.texts.push(Text { text: chevron.into(), pos: [x, text_y(rect)], size, color: rgb(th.dim_text(), 1.0), clip: inner, bold: false });
                x += size * 1.1;
                if let Group::Trail(_) = group {
                    // A dot in the trail's colour, as on its tag on the board.
                    let d = size * 0.7;
                    l.quads.push(Quad {
                        shape: Shape::Rect { rect: Rect::new(x, rect.y + (rect.h - d) / 2.0, d, d), radius: d / 2.0 },
                        color: rgb(th.trail(mark.colour.as_deref()), 1.0),
                        texture: None,
                    });
                    x += d + size * 0.5;
                }
                let n = count.to_string();
                let nw = r.measure(&n, size, false);
                l.texts.push(Text { text: n, pos: [right - nw, text_y(rect)], size, color: rgb(th.dim_text(), 1.0), clip: inner, bold: false });
                l.texts.push(Text {
                    text: label.clone(),
                    pos: [x, text_y(rect)],
                    size,
                    color: rgb(th.foreground(), 1.0),
                    clip: Rect::new(x, rect.y, (right - nw - size * 0.6 - x).max(1.0), rect.h),
                    bold: true,
                });
            }
            Row::Card { id, depth, folded, .. } => {
                let Some(n) = scene.node(*id) else { continue };
                let current = marks.current == Some(*id);
                if current {
                    // A short accent bar marks the current card, as in a stack's outline.
                    let bar = Rect::new(inner.x + 2.0 * ui, rect.y + rect.h * 0.25, (2.0 * ui).max(1.0), rect.h * 0.5);
                    l.quads.push(Quad { shape: Shape::Rect { rect: bar, radius: 0.0 }, color: rgb(th.accent(), 1.0), texture: None });
                }
                let mut x = inner.x + pad * 0.5 + size * 1.1 + INDENT * ui * *depth as f32;
                if let Some(c) = n.sticky.as_deref().filter(|_| n.is_sticky()) {
                    // A sticky note shows its colour as a small square of paper.
                    let d = size * 0.65;
                    l.quads.push(Quad {
                        shape: Shape::Rect { rect: Rect::new(x, rect.y + (rect.h - d) / 2.0, d, d), radius: 1.5 * ui },
                        color: rgb(th.sticky_paper(c), 1.0),
                        texture: None,
                    });
                    x += d + size * 0.5;
                }
                let title = if n.title.trim().is_empty() { n.url.as_str() } else { n.title.trim() };
                let title = crate::menu::cut(title, crate::menu::OUTLINE_CHARS);
                // Folded-away cards (behind a stack's front) are dimmed.
                let color = if *folded { th.dim_text() } else { th.foreground() };
                l.texts.push(Text {
                    text: title,
                    pos: [x, text_y(rect)],
                    size,
                    color: rgb(color, 1.0),
                    clip: Rect::new(x, rect.y, (right - x).max(1.0), rect.h),
                    bold: current,
                });
            }
        }
    }

    // A thin scroll bar when the list is longer than the panel.
    if rows.len() > panel.shown {
        let track = panel.shown as f32 * panel.row_h;
        let h = (track * panel.shown as f32 / rows.len() as f32).max(16.0 * ui);
        let y = panel.rows_top + (track - h) * scroll as f32 / (rows.len() - panel.shown) as f32;
        let w = 3.0 * ui;
        l.quads.push(Quad {
            shape: Shape::Rect { rect: Rect::new(p.right() - line - w - 2.0 * ui, y, w, h), radius: w / 2.0 },
            color: rgb(th.dim_text(), 0.6),
            texture: None,
        });
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_fit_below_the_title_and_hit_where_drawn() {
        let p = Panel::new(Vec2::new(1200.0, 44.0 + 26.0 * 10.0 + 10.0), 1.0);
        assert_eq!(p.rect.w, WIDTH);
        assert_eq!(p.shown, 10);
        assert_eq!(p.row_rect(3, 0).unwrap().y, TITLE_H + 3.0 * ROW_H);
        assert_eq!(p.row_rect(3, 2).unwrap().y, TITLE_H + ROW_H);
        assert_eq!(p.row_rect(1, 2), None, "scrolled off the top");
        assert_eq!(p.row_rect(12, 2), None, "below the last shown row");
        let mid = |i: f32| Vec2::new(50.0, TITLE_H + (i + 0.5) * ROW_H);
        assert_eq!(p.row_at(mid(0.0), 2, 20), Some(2));
        assert_eq!(p.row_at(mid(4.0), 0, 3), None, "past the last row");
        assert_eq!(p.row_at(Vec2::new(50.0, 10.0), 0, 20), None, "the title");
        assert_eq!(p.row_at(Vec2::new(WIDTH + 1.0, TITLE_H + 1.0), 0, 20), None, "the board");
    }

    #[test]
    fn a_narrow_window_keeps_most_of_the_board() {
        let p = Panel::new(Vec2::new(500.0, 400.0), 1.0);
        assert_eq!(p.rect.w, (500.0 * MAX_SHARE).round());
        assert!(Panel::new(Vec2::new(500.0, 10.0), 1.0).shown >= 1);
    }
}
