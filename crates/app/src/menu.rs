//! The context menu overlay: layout, hit testing and keyboard movement.
//! Which items appear comes from `web::menu`.

use scene::{NodeId, Rect, Vec2};
use web::menu::MenuItem;

/// Sizes in logical px; multiplied by the window scale.
pub const ITEM_H: f32 = 28.0;
pub const SEP_H: f32 = 9.0;
pub const PAD: f32 = 6.0;
pub const TEXT_PAD: f32 = 14.0;
pub const FONT: f32 = 14.0;
pub const MIN_W: f32 = 180.0;

pub struct ContextMenu {
    /// The card whose page was right-clicked.
    pub page: NodeId,
    pub items: Vec<MenuItem>,
    /// Outer box, screen px.
    pub rect: Rect,
    /// One rect per item (separators included), screen px.
    pub rows: Vec<Rect>,
    pub hover: Option<usize>,
}

impl ContextMenu {
    /// Lay out `items` at `anchor` (screen px) with the given widest label width.
    pub fn new(page: NodeId, items: Vec<MenuItem>, anchor: Vec2, label_w: f32, viewport: Vec2, scale: f32) -> Self {
        let (rect, rows) = layout(&items, anchor, label_w, viewport, scale);
        ContextMenu { page, items, rect, rows, hover: None }
    }

    /// Index of the enabled item at `p`.
    pub fn item_at(&self, p: Vec2) -> Option<usize> {
        self.rows.iter().position(|r| r.contains(p)).filter(|&i| self.items[i].is_enabled())
    }

    /// Move the highlight by `step` (±1) over enabled items, wrapping around.
    pub fn step(&mut self, step: i32) {
        let n = self.items.len() as i32;
        if n == 0 {
            return;
        }
        let mut i = match self.hover {
            Some(h) => h as i32,
            None if step > 0 => -1,
            None => n,
        };
        for _ in 0..n {
            i = (i + step).rem_euclid(n);
            if self.items[i as usize].is_enabled() {
                self.hover = Some(i as usize);
                return;
            }
        }
    }
}

/// Menu box and row rects. The menu opens down and right of `anchor`, flipping
/// up or left when it would leave the viewport, and is clamped inside it.
pub fn layout(items: &[MenuItem], anchor: Vec2, label_w: f32, viewport: Vec2, scale: f32) -> (Rect, Vec<Rect>) {
    let w = (label_w + 2.0 * TEXT_PAD * scale).max(MIN_W * scale);
    let content_h: f32 =
        items.iter().map(|i| if matches!(i, MenuItem::Separator) { SEP_H } else { ITEM_H }).sum::<f32>() * scale;
    let h = content_h + 2.0 * PAD * scale;
    let mut x = anchor.x;
    let mut y = anchor.y;
    if x + w > viewport.x {
        x = anchor.x - w;
    }
    if y + h > viewport.y {
        y = anchor.y - h;
    }
    x = x.min(viewport.x - w).max(0.0);
    y = y.min(viewport.y - h).max(0.0);
    let mut rows = Vec::with_capacity(items.len());
    let mut top = y + PAD * scale;
    for i in items {
        let rh = if matches!(i, MenuItem::Separator) { SEP_H } else { ITEM_H } * scale;
        rows.push(Rect::new(x + PAD * scale, top, w - 2.0 * PAD * scale, rh));
        top += rh;
    }
    (Rect::new(x, y, w, h), rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use web::menu::{items, MenuContext};

    fn sample() -> Vec<MenuItem> {
        // Back (disabled), Forward (disabled), Reload, -, Select all
        items(&MenuContext::default())
    }

    #[test]
    fn opens_below_right_and_flips_at_edges() {
        let vp = Vec2::new(800.0, 600.0);
        let (r, rows) = layout(&sample(), Vec2::new(100.0, 100.0), 50.0, vp, 1.0);
        assert_eq!((r.x, r.y, r.w), (100.0, 100.0, MIN_W));
        assert_eq!(r.h, 4.0 * ITEM_H + SEP_H + 2.0 * PAD);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].y, 100.0 + PAD);
        assert_eq!(rows[3].h, SEP_H);
        let (r, _) = layout(&sample(), Vec2::new(790.0, 590.0), 50.0, vp, 1.0);
        assert_eq!((r.right(), r.bottom()), (790.0, 590.0));
        // Too tall to flip up: clamped to the top.
        let (r, _) = layout(&sample(), Vec2::new(10.0, 50.0), 50.0, Vec2::new(800.0, 120.0), 1.0);
        assert_eq!(r.y, 0.0);
    }

    #[test]
    fn hit_and_keyboard_skip_disabled_and_separators() {
        let vp = Vec2::new(800.0, 600.0);
        let mut m = ContextMenu::new(NodeId(1), sample(), Vec2::new(0.0, 0.0), 50.0, vp, 1.0);
        let center = |r: Rect| Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert_eq!(m.item_at(center(m.rows[0])), None, "Back is disabled");
        assert_eq!(m.item_at(center(m.rows[2])), Some(2));
        assert_eq!(m.item_at(center(m.rows[3])), None, "separator");
        assert_eq!(m.item_at(Vec2::new(500.0, 500.0)), None);
        m.step(1);
        assert_eq!(m.hover, Some(2));
        m.step(1);
        assert_eq!(m.hover, Some(4));
        m.step(1);
        assert_eq!(m.hover, Some(2), "wraps");
        m.hover = None;
        m.step(-1);
        assert_eq!(m.hover, Some(4));
    }
}
