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
