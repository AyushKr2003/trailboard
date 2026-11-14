//! Where new cards go. Our own rule (Marble's `tools::placement` was not
//! decompiled): a card opened from a link goes to the right of its source,
//! below any siblings already there, sliding down until it overlaps nothing.

use crate::{Node, Rect, Scene, HEADER_H};

/// Horizontal and vertical gap between cards, in world units.
pub const GAP: f32 = 60.0;

/// Content rect for a new card opened from `src`.
pub fn beside(scene: &Scene, src: &Node) -> Rect {
    beside_sized(scene, src, src.rect.w, src.rect.h)
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
