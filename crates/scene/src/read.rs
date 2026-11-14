//! Read Mode: one card fills the window and its page lays out at the window's
//! size. The board (camera, card rects) is left as it was, so leaving Read Mode
//! shows the board exactly as before.

use crate::{NodeId, Rect, Scene, Vec2};

/// Screen layout of Read Mode.
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

/// The card to read when Read Mode is asked for: the focused card, else the
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
