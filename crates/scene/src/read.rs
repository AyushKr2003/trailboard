//! Solo: one card fills the window and its page lays out at the window's
//! size. The board (camera, card rects) is left as it was, so leaving Solo
//! shows the board exactly as before.

use crate::{NodeId, Rect, Scene, Vec2};

/// Screen layout of Solo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReadLayout {
    /// Title bar across the top of the window (screen px).
    pub bar: Rect,
    /// Where the page is drawn (screen px).
    pub content: Rect,
    /// Page size in CSS px.
    pub css: (i32, i32),
}

/// Layout for a `viewport` (screen px) at window `scale`, with a `bar_px` tall title bar.
pub fn layout(viewport: Vec2, scale: f32, bar_px: f32) -> ReadLayout {
    let scale = scale.max(0.1);
    let bar_h = bar_px.clamp(0.0, viewport.y);
    let bar = Rect::new(0.0, 0.0, viewport.x, bar_h);
    let content = Rect::new(0.0, bar_h, viewport.x, (viewport.y - bar_h).max(1.0));
    let css = ((content.w / scale).round().max(1.0) as i32, (content.h / scale).round().max(1.0) as i32);
    ReadLayout { bar, content, css }
}

/// Narrowest Solo width (CSS px) a card's page is laid out at; below
/// it (no window yet) a card lays out at its own size.
const MIN_READ_W: i32 = 320;

/// The CSS size a card's page lays out at on the board: as wide as Read
/// Mode's page (`read_w`, CSS px), so the card shows the page exactly as the
/// full window does, only scaled down, and never zoomed in or out by the
/// card's size; as tall as the card's shape gives at that width. `card` is
/// the card's content size (world units).
pub fn card_css(card: Vec2, read_w: i32) -> Vec2 {
    if read_w < MIN_READ_W || card.x < 1.0 || card.y < 1.0 {
        return Vec2::new(card.x.round().max(1.0), card.y.round().max(1.0));
    }
    let w = read_w as f32;
    Vec2::new(w, (card.y * w / card.x).round().max(1.0))
}

/// How long the card takes to grow into Solo, or shrink back (ms).
pub const MORPH_MS: u64 = 200;

/// One frame of the move between a card on the board and Solo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Morph {
    /// The title bar (screen px): the card's header → the read bar.
    pub bar: Rect,
    /// The page (screen px): the card's content → the read content.
    pub content: Rect,
    /// The CSS size `content` shows, so the page's pixels are drawn at their
    /// own scale (never stretched) while they catch up with the new size.
    pub css: Vec2,
}

/// The card's `header` and `content` (screen px, the content showing
/// `card_css` CSS px) at `t` = 0, Solo's `layout` at `t` = 1. Screen
/// px per CSS px moves evenly between the two, so the page grows at a
/// steady rate rather than jumping in scale at either end.
pub fn morph(header: Rect, content: Rect, card_css: Vec2, layout: &ReadLayout, t: f32) -> Morph {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    let rect = |a: Rect, b: Rect| Rect::new(lerp(a.x, b.x), lerp(a.y, b.y), lerp(a.w, b.w), lerp(a.h, b.h));
    let k0 = content.w / card_css.x.max(1.0);
    let k1 = layout.content.w / (layout.css.0 as f32).max(1.0);
    let k = lerp(k0, k1).max(1e-4);
    let c = rect(content, layout.content);
    Morph { bar: rect(header, layout.bar), content: c, css: Vec2::new(c.w / k, c.h / k) }
}

/// How many widths and heights of a page painted at `painted` CSS px a rect
/// showing `shown` CSS px spans (`compositor::Shape::Page`'s extent).
pub fn extent(shown: Vec2, painted: (f32, f32)) -> [f32; 2] {
    if painted.0 < 1.0 || painted.1 < 1.0 {
        return [1.0, 1.0];
    }
    [shown.x / painted.0, shown.y / painted.1]
}

/// The card to read when Solo is asked for: the focused card, else the
/// topmost card under the middle of the view, else the card nearest to it.
pub fn pick(scene: &Scene, focused: Option<NodeId>, view_center: Vec2) -> Option<NodeId> {
    if let Some(id) = focused.filter(|id| scene.node(*id).is_some()) {
        return Some(id);
    }
    if let Some(id) = scene.topmost_at(view_center) {
        return Some(id);
    }
    scene
        .nodes()
        .map(|n| (n.id, distance(n.frame(), view_center)))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(id, _)| id)
}

/// Distance from `p` to the nearest point of `r` (0 inside).
fn distance(r: Rect, p: Vec2) -> f32 {
    let dx = (r.x - p.x).max(p.x - r.right()).max(0.0);
    let dy = (r.y - p.y).max(p.y - r.bottom()).max(0.0);
    dx.hypot(dy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_fills_window_below_bar() {
        let l = layout(Vec2::new(2880.0, 1800.0), 2.0, 68.0);
        assert_eq!(l.bar, Rect::new(0.0, 0.0, 2880.0, 68.0));
        assert_eq!(l.content, Rect::new(0.0, 68.0, 2880.0, 1732.0));
        assert_eq!(l.css, (1440, 866));
    }

    #[test]
    fn morph_runs_from_the_card_to_read_mode() {
        let l = layout(Vec2::new(2000.0, 1068.0), 1.0, 68.0);
        let header = Rect::new(100.0, 100.0, 512.0, 20.0);
        let content = Rect::new(100.0, 120.0, 512.0, 360.0);
        let css = Vec2::new(1024.0, 720.0);
        let a = morph(header, content, css, &l, 0.0);
        assert_eq!((a.bar, a.content, a.css), (header, content, css));
        let b = morph(header, content, css, &l, 1.0);
        assert_eq!((b.bar, b.content), (l.bar, l.content));
        assert!((b.css.x - 2000.0).abs() < 0.01 && (b.css.y - 1000.0).abs() < 0.01);
        // Halfway: half the way in screen px, and the scale in between (0.5 → 1).
        let m = morph(header, content, css, &l, 0.5);
        assert_eq!(m.content.w, 1256.0);
        assert!((m.css.x - 1256.0 / 0.75).abs() < 0.01);
        // Out of range clamps.
        assert_eq!(morph(header, content, css, &l, 2.0), b);
    }

    #[test]
    fn card_lays_out_at_read_width() {
        // A 1024 × 640 card on a window whose Solo page is 1440 px wide.
        assert_eq!(card_css(Vec2::new(1024.0, 640.0), 1440), Vec2::new(1440.0, 900.0));
        // Resizing the card scales the page; its width stays the window's.
        assert_eq!(card_css(Vec2::new(512.0, 640.0), 1440), Vec2::new(1440.0, 1800.0));
        // No window yet: the card's own size.
        assert_eq!(card_css(Vec2::new(1024.0, 576.0), 1), Vec2::new(1024.0, 576.0));
    }

    #[test]
    fn extent_shows_old_pixels_at_their_own_size() {
        assert_eq!(extent(Vec2::new(1024.0, 720.0), (1024.0, 720.0)), [1.0, 1.0]);
        // The window-sized page squeezed back into a card: cropped, not squashed.
        assert_eq!(extent(Vec2::new(1024.0, 720.0), (2048.0, 1000.0)), [0.5, 0.72]);
        // The card's page shown in the window before it repaints: the rest filled.
        assert_eq!(extent(Vec2::new(2048.0, 1440.0), (1024.0, 720.0)), [2.0, 2.0]);
        assert_eq!(extent(Vec2::new(10.0, 10.0), (0.0, 0.0)), [1.0, 1.0]);
    }

    #[test]
    fn layout_survives_a_tiny_window() {
        let l = layout(Vec2::new(10.0, 20.0), 1.0, 34.0);
        assert_eq!(l.bar.h, 20.0);
        assert!(l.css.0 >= 1 && l.css.1 >= 1);
    }

    #[test]
    fn pick_prefers_focus_then_centre_then_nearest() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 100.0, 100.0), "a");
        let b = s.add_page(Rect::new(1000.0, 0.0, 100.0, 100.0), "b");
        let c = s.add_page(Rect::new(50.0, 50.0, 100.0, 100.0), "c");
        assert_eq!(pick(&s, Some(b), Vec2::new(60.0, 60.0)), Some(b));
        // Under the centre, the topmost card wins.
        assert_eq!(pick(&s, None, Vec2::new(60.0, 60.0)), Some(c));
        // Nothing under the centre: the nearest card.
        assert_eq!(pick(&s, None, Vec2::new(900.0, 50.0)), Some(b));
        assert_eq!(pick(&s, None, Vec2::new(-300.0, 20.0)), Some(a));
        // A focus that is gone is ignored.
        assert_eq!(pick(&s, Some(NodeId(99)), Vec2::new(1050.0, 50.0)), Some(b));
        assert_eq!(pick(&Scene::new(), None, Vec2::default()), None);
    }
}
