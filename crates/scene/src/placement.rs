//! Where new cards go. Our own rule (Marble's `tools::placement` was not
//! decompiled): a card opened from a link goes to the right of its source,
//! below any siblings already there, sliding down until it overlaps nothing.

use crate::{Node, Rect, Scene, Vec2, HEADER_H};

/// Horizontal and vertical gap between cards, in world units.
pub const GAP: f32 = 60.0;

/// Width and height ratios a card may take from the screen: from a tall
/// portrait screen (9:21) to a very wide one (32:9). Anything outside is
/// taken as a wrong measurement.
const SHAPES: std::ops::RangeInclusive<f32> = (9.0 / 21.0)..=(32.0 / 9.0);
/// Heights a new card may have, world units.
const HEIGHTS: std::ops::RangeInclusive<f32> = 400.0..=1600.0;

/// The size of a new card: shaped like the window a full-screen app gets on
/// an empty workspace (`full_window`: the screen less bars, gaps and
/// borders), else like the screen itself, else 16:9; at the default width,
/// [`crate::CARD_W`], so pages lay out as on a desktop. Sizes that are
/// missing, empty or of an unlikely shape are skipped. The height is kept to
/// [`HEIGHTS`], and is whole.
pub fn card_size(full_window: Option<Vec2>, screen: Option<Vec2>) -> Vec2 {
    let shape = [full_window, screen]
        .into_iter()
        .flatten()
        .filter(|s| s.x > 0.0 && s.y > 0.0 && s.x.is_finite() && s.y.is_finite())
        .map(|s| s.x / s.y)
        .find(|r| SHAPES.contains(r));
    let Some(shape) = shape else { return Vec2::new(crate::CARD_W, crate::CARD_H) };
    let h = (crate::CARD_W / shape).round().clamp(*HEIGHTS.start(), *HEIGHTS.end());
    Vec2::new(crate::CARD_W, h)
}

/// Content rect for a new card opened from `src`, at the default card size
/// whatever size `src` is.
pub fn beside(scene: &Scene, src: &Node) -> Rect {
    beside_sized(scene, src, crate::CARD_W, crate::CARD_H)
}

/// Content rect of size `w`×`h` for a new card opened from `src`.
pub fn beside_sized(scene: &Scene, src: &Node, w: f32, h: f32) -> Rect {
    let size = (w, h);
    let x = src.rect.right() + GAP;
    let mut y = src.rect.y;
    // Walk down in steps until the candidate frame is free.
    for _ in 0..256 {
        let candidate = Rect::new(x, y, size.0, size.1);
        let frame = Rect::new(candidate.x, candidate.y - HEADER_H, candidate.w, candidate.h + HEADER_H);
        match scene.nodes().filter(|n| n.frame().inflate(GAP / 2.0 - 1.0).intersects(&frame)).map(|n| n.rect.bottom()).reduce(f32::max) {
            None => return candidate,
            Some(blocking_bottom) => y = blocking_bottom + GAP + HEADER_H,
        }
    }
    Rect::new(x, y, size.0, size.1)
}

/// Content rect for a brand-new card near the viewport centre, avoiding overlaps.
pub fn free_near(scene: &Scene, center: crate::Vec2, w: f32, h: f32) -> Rect {
    let mut r = Rect::new(center.x - w / 2.0, center.y - h / 2.0, w, h);
    for _ in 0..256 {
        let frame = Rect::new(r.x, r.y - HEADER_H, r.w, r.h + HEADER_H);
        match scene.nodes().find(|n| n.frame().inflate(GAP / 2.0 - 1.0).intersects(&frame)) {
            None => return r,
            Some(n) => r.x = n.rect.right() + GAP,
        }
    }
    r
}

/// Content rect of size `w`×`h` to the left of `src`, tops aligned (a guide
/// beside the first page). Falls back to [`free_near`] when that spot is taken.
pub fn left_of(scene: &Scene, src: &Node, w: f32, h: f32) -> Rect {
    let r = Rect::new(src.rect.x - GAP - w, src.rect.y, w, h);
    let frame = Rect::new(r.x, r.y - HEADER_H, r.w, r.h + HEADER_H);
    if scene.nodes().any(|n| n.frame().inflate(GAP / 2.0 - 1.0).intersects(&frame)) {
        free_near(scene, src.rect.center(), w, h)
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_take_the_shape_of_a_full_window_then_the_screen_then_16_9() {
        let full = Vec2::new(1896.0, 1030.0);
        let screen = Vec2::new(1920.0, 1080.0);
        assert_eq!(card_size(Some(full), Some(screen)), Vec2::new(1024.0, 556.0));
        assert_eq!(card_size(None, Some(screen)), Vec2::new(1024.0, 576.0));
        assert_eq!(card_size(None, Some(Vec2::new(2560.0, 1600.0))), Vec2::new(1024.0, 640.0));
        // Nothing known: the default, which is 16:9.
        assert_eq!(card_size(None, None), Vec2::new(crate::CARD_W, crate::CARD_H));
        assert!((crate::CARD_W / crate::CARD_H - 16.0 / 9.0).abs() < 1e-3);
        // A bad measurement is skipped for the next one.
        assert_eq!(card_size(Some(Vec2::new(0.0, 1030.0)), Some(screen)), Vec2::new(1024.0, 576.0));
        assert_eq!(card_size(Some(Vec2::new(5000.0, 40.0)), None), Vec2::new(crate::CARD_W, crate::CARD_H));
        // A portrait screen: taller cards, kept within limits.
        assert_eq!(card_size(None, Some(Vec2::new(1080.0, 1920.0))).y, 1600.0);
        assert_eq!(card_size(None, Some(Vec2::new(1200.0, 1600.0))).y, 1365.0);
    }

    #[test]
    fn left_of_aligns_tops_with_a_gap() {
        let mut s = Scene::new();
        let page = s.add_page(Rect::new(0.0, 0.0, 1024.0, 720.0), "https://a/");
        let src = s.node(page).unwrap().clone();
        let r = left_of(&s, &src, 480.0, 720.0);
        assert_eq!(r, Rect::new(-GAP - 480.0, 0.0, 480.0, 720.0));
    }

    #[test]
    fn left_of_avoids_a_card_already_there() {
        let mut s = Scene::new();
        let page = s.add_page(Rect::new(0.0, 0.0, 1024.0, 720.0), "https://a/");
        let other = s.add_page(Rect::new(-700.0, 100.0, 400.0, 300.0), "https://b/");
        let src = s.node(page).unwrap().clone();
        let r = left_of(&s, &src, 480.0, 720.0);
        let frame = Rect::new(r.x, r.y - HEADER_H, r.w, r.h + HEADER_H);
        for id in [page, other] {
            assert!(!s.node(id).unwrap().frame().intersects(&frame), "overlaps card {}", id.0);
        }
    }
}
