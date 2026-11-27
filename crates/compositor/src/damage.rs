//! Pure pixel-rect maths for partial frames: damage on screen, the page
//! hiding the most of an area, the strips round it, and the key telling two
//! frames apart.

use std::collections::HashMap;

use scene::Rect;

use crate::{Background, COVER_MIN, Layer, Shape};

/// `r` (x, y, w, h) cut to a `width`×`height` image.
pub(crate) fn clamp_px(r: [u32; 4], width: u32, height: u32) -> [u32; 4] {
    let (x, y) = (r[0].min(width), r[1].min(height));
    [x, y, r[2].min(width - x), r[3].min(height - y)]
}

/// The smallest rect holding both (x, y, w, h); an empty one adds nothing.
pub(crate) fn union_px(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    if a[2] == 0 || a[3] == 0 {
        return b;
    }
    if b[2] == 0 || b[3] == 0 {
        return a;
    }
    let (x0, y0) = (a[0].min(b[0]), a[1].min(b[1]));
    let (x1, y1) = ((a[0] + a[2]).max(b[0] + b[2]), (a[1] + a[3]).max(b[1] + b[3]));
    [x0, y0, x1 - x0, y1 - y0]
}

/// Where on screen the changed page pixels show: the union over every quad
/// drawing a damaged page, cut to the viewport (empty: none on screen), or
/// `None` when a whole page changed and the frame should be drawn whole.
/// `damage` holds texture px per page key; `size` gives a page's texture size.
pub(crate) fn screen_damage(
    layers: &[Layer],
    damage: &HashMap<u64, Option<[u32; 4]>>,
    size: impl Fn(u64) -> Option<(u32, u32)>,
    viewport: (f32, f32),
) -> Option<Rect> {
    let mut out: Option<Rect> = None;
    for q in layers.iter().flat_map(|l| &l.quads) {
        let Some(k) = q.texture else { continue };
        let Some(&changed) = damage.get(&k) else { continue };
        let (rect, extent) = match q.shape {
            Shape::Page { rect, extent, .. } => (rect, extent),
            Shape::Rect { rect, .. } | Shape::RoundBottom { rect, .. } => (rect, [1.0, 1.0]),
            _ => continue,
        };
        let r = page_damage_on_screen(rect, extent, size(k)?, changed?);
        out = Some(out.map_or(r, |o| o.union(&r)));
    }
    let view = Rect::new(0.0, 0.0, viewport.0, viewport.1);
    let Some(r) = out else { return Some(Rect::new(0.0, 0.0, 0.0, 0.0)) };
    let (x0, y0) = (r.x.max(0.0), r.y.max(0.0));
    let (x1, y1) = (r.right().min(view.w), r.bottom().min(view.h));
    Some(Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)))
}

/// The screen rect where texture px `dirty` (x, y, w, h) of a `tex`-sized
/// page show, drawn into `rect` spanning `extent` textures (see
/// [`Shape::Page`]), with a margin for filtering and anti-aliasing.
pub(crate) fn page_damage_on_screen(rect: Rect, extent: [f32; 2], tex: (u32, u32), dirty: [u32; 4]) -> Rect {
    // Past the texture the shader repeats its left column: a change there
    // shows across the rest of the rect.
    if (extent[0] > 1.0 || extent[1] > 1.0) && dirty[0] == 0 {
        return rect.inflate(2.0);
    }
    let sx = rect.w / extent[0].max(1e-3) / tex.0.max(1) as f32;
    let sy = rect.h / extent[1].max(1e-3) / tex.1.max(1) as f32;
    let r = Rect::new(rect.x + dirty[0] as f32 * sx, rect.y + dirty[1] as f32 * sy, dirty[2] as f32 * sx, dirty[3] as f32 * sy);
    // A texel blends into its neighbours when scaled.
    r.inflate(2.0 + sx.max(sy))
}

/// A screen rect as a scissor (whole pixels, inside the target).
pub(crate) fn scissor_px(r: Rect, width: u32, height: u32) -> [u32; 4] {
    let x0 = (r.x.floor().max(0.0) as u32).min(width);
    let y0 = (r.y.floor().max(0.0) as u32).min(height);
    let x1 = (r.right().ceil().max(0.0) as u32).clamp(x0, width);
    let y1 = (r.bottom().ceil().max(0.0) as u32).clamp(y0, height);
    [x0, y0, x1 - x0, y1 - y0]
}

/// The opaque page hiding the most of `area` (px: x, y, w, h), as its layer,
/// its instance (quads are numbered in drawing order, as `render` lays them
/// out) and the part of `area` it hides, clear of its rounded corners and
/// anti-aliased edge. `None` when no page hides at least [`COVER_MIN`] of it.
/// Pages are opaque (CEF paints over an opaque background); one faded with
/// the colour's alpha doesn't count.
pub(crate) fn cover(layers: &[Layer], area: [u32; 4]) -> Option<(usize, u32, [u32; 4])> {
    let mut best: Option<(usize, u32, [u32; 4])> = None;
    let mut inst = 0u32;
    for (i, layer) in layers.iter().enumerate() {
        for q in &layer.quads {
            if let Shape::Page { rect, radius, .. } = q.shape
                && q.color[3] >= 1.0
            {
                // A rounded corner's arc passes (1 - 1/√2)·r in from both
                // edges at its middle; the edge's last half pixel blends.
                let r = radius.min(rect.w / 2.0).min(rect.h / 2.0).max(0.0);
                let m = r * (1.0 - std::f32::consts::FRAC_1_SQRT_2) + 1.0;
                let x0 = (rect.x + m).ceil().max(0.0) as u32;
                let y0 = (rect.y + m).ceil().max(0.0) as u32;
                let x1 = (rect.right() - m).floor().max(0.0) as u32;
                let y1 = (rect.bottom() - m).floor().max(0.0) as u32;
                let hole = intersect_px([x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)], area);
                let size = hole[2] * hole[3];
                if size >= COVER_MIN && best.is_none_or(|b| size > b.2[2] * b.2[3]) {
                    best = Some((i, inst, hole));
                }
            }
            inst += 1;
        }
    }
    best
}

/// Where in its texture to copy `hole` (screen px: x, y, w, h) from, when a
/// page quad there would show its texels 1:1 and unchanged: its colour
/// white and opaque, its rect at whole pixels, one texel a pixel, and the
/// hole inside the texture's part of the rect (past it the shader repeats
/// the left column). The shader samples each pixel's centre at a texel's
/// centre then, so a copy gives the same pixels for a fraction of the cost
/// on a software renderer. `None`: shade it.
pub(crate) fn page_copy_from(shape: &Shape, color: [f32; 4], tex: (u32, u32), hole: [u32; 4]) -> Option<[u32; 2]> {
    const EPS: f32 = 1e-3;
    let Shape::Page { rect, extent, .. } = *shape else { return None };
    let whole = |v: f32| (v - v.round()).abs() < EPS;
    let one = |span: f32, ext: f32, texels: u32| ext > 0.0 && (span / ext - texels as f32).abs() < EPS * texels.max(1) as f32;
    if color != [1.0; 4] || !whole(rect.x) || !whole(rect.y) || !one(rect.w, extent[0], tex.0) || !one(rect.h, extent[1], tex.1) {
        return None;
    }
    let (x0, y0) = (rect.x.round() as i64, rect.y.round() as i64);
    let (sx, sy) = (hole[0] as i64 - x0, hole[1] as i64 - y0);
    let fits = sx >= 0 && sy >= 0 && sx + hole[2] as i64 <= tex.0 as i64 && sy + hole[3] as i64 <= tex.1 as i64;
    (fits && hole[2] > 0 && hole[3] > 0).then_some([sx as u32, sy as u32])
}

/// Whether the inside of this page quad (clear of its corners, as
/// [`cover`] gives it) can be drawn by `fs_page`: its texels unchanged and
/// unblended. White and opaque, and spanning no more than its texture (past
/// it `fs_quad` repeats the left column).
pub(crate) fn page_plain(shape: &Shape, color: [f32; 4]) -> bool {
    matches!(*shape, Shape::Page { extent, .. } if extent[0] <= 1.0 + 1e-3 && extent[1] <= 1.0 + 1e-3) && color == [1.0; 4]
}

/// The overlap of two px rects (x, y, w, h); empty (w or h 0) if none.
pub(crate) fn intersect_px(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let (x0, y0) = (a[0].max(b[0]), a[1].max(b[1]));
    let (x1, y1) = ((a[0] + a[2]).min(b[0] + b[2]), (a[1] + a[3]).min(b[1] + b[3]));
    [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
}

/// `area` without `hole` (inside it), as the strips above, below, left and
/// right of it; together with `hole` they cover `area` once.
pub(crate) fn cut_px(area: [u32; 4], hole: [u32; 4]) -> Vec<[u32; 4]> {
    let [ax, ay, aw, ah] = area;
    let [hx, hy, hw, hh] = hole;
    [
        [ax, ay, aw, hy - ay],
        [ax, hy + hh, aw, ay + ah - (hy + hh)],
        [ax, hy, hx - ax, hh],
        [hx + hw, hy, ax + aw - (hx + hw), hh],
    ]
    .into_iter()
    .filter(|r| r[2] > 0 && r[3] > 0)
    .collect()
}

/// A fingerprint of everything a frame shows: the viewport, the background,
/// every quad and text in order, and the pixels' serial of each page drawn.
pub(crate) fn frame_key(viewport: (f32, f32), bg: &Background, layers: &[Layer], serial: impl Fn(u64) -> Option<u64>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let f = |h: &mut std::collections::hash_map::DefaultHasher, xs: &[f32]| {
        for x in xs {
            x.to_bits().hash(h);
        }
    };
    f(&mut h, &[viewport.0, viewport.1, bg.grain, bg.grid_spacing, bg.dot_radius, bg.fine]);
    f(&mut h, &bg.color);
    f(&mut h, &bg.color2);
    f(&mut h, &bg.dot);
    f(&mut h, &bg.grid_origin);
    for layer in layers {
        layer.quads.len().hash(&mut h);
        for q in &layer.quads {
            match q.shape {
                Shape::Rect { rect, radius } => {
                    0u8.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius]);
                }
                Shape::RoundBottom { rect, radius } => {
                    1u8.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius]);
                }
                Shape::Shadow { rect, radius, blur } => {
                    2u8.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius, blur]);
                }
                Shape::Ring { rect, radius, width } => {
                    3u8.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius, width]);
                }
                Shape::Segment { from, to, width } => {
                    4u8.hash(&mut h);
                    f(&mut h, &[from[0], from[1], to[0], to[1], width]);
                }
                Shape::Page { rect, radius, square, extent } => {
                    5u8.hash(&mut h);
                    square.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius, extent[0], extent[1]]);
                }
            }
            f(&mut h, &q.color);
            q.texture.hash(&mut h);
            q.texture.map(&serial).hash(&mut h);
        }
        layer.texts.len().hash(&mut h);
        for t in &layer.texts {
            t.text.hash(&mut h);
            t.bold.hash(&mut h);
            f(&mut h, &[t.pos[0], t.pos[1], t.size, t.clip.x, t.clip.y, t.clip.w, t.clip.h]);
            f(&mut h, &t.color);
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Quad, Text};

    fn bg() -> Background {
        Background {
            color: [0.1, 0.1, 0.1, 1.0],
            color2: [0.1, 0.1, 0.1, 1.0],
            grain: 0.0,
            dot: [0.5, 0.5, 0.5, 1.0],
            grid_origin: [0.0, 0.0],
            grid_spacing: 24.0,
            dot_radius: 1.0,
            fine: 0.0,
        }
    }

    fn layer(x: f32, texture: Option<u64>, text: &str) -> Layer {
        let rect = Rect { x, y: 10.0, w: 100.0, h: 50.0 };
        Layer {
            quads: vec![Quad { shape: Shape::Page { rect, radius: 8.0, square: 0, extent: [1.0, 1.0] }, color: [1.0; 4], texture }],
            texts: vec![Text { text: text.into(), pos: [x, 0.0], size: 13.0, color: [1.0; 4], clip: rect, bold: false }],
        }
    }

    #[test]
    fn frame_key_changes_with_what_is_shown() {
        let serial = |s: u64| move |_k: u64| Some(s);
        let key = |x: f32, text: &str, s: u64| frame_key((800.0, 600.0), &bg(), &[layer(x, Some(7), text)], serial(s));
        let same = key(10.0, "a", 1);
        assert_eq!(same, key(10.0, "a", 1), "the same frame twice");
        assert_ne!(same, key(10.5, "a", 1), "a quad moved half a pixel");
        assert_ne!(same, key(10.0, "b", 1), "a label changed");
        assert_ne!(same, key(10.0, "a", 2), "the page painted");
        assert_ne!(same, frame_key((801.0, 600.0), &bg(), &[layer(10.0, Some(7), "a")], serial(1)), "the window resized");
        let mut dark = bg();
        dark.color = [0.0, 0.0, 0.0, 1.0];
        assert_ne!(same, frame_key((800.0, 600.0), &dark, &[layer(10.0, Some(7), "a")], serial(1)), "the theme changed");
        let two = frame_key((800.0, 600.0), &bg(), &[layer(10.0, Some(7), "a"), Layer::default()], serial(1));
        assert_ne!(same, two, "an empty layer counts");
    }

    fn page_quad(rect: Rect, extent: [f32; 2], key: u64) -> Layer {
        Layer {
            quads: vec![Quad { shape: Shape::Page { rect, radius: 8.0, square: 0, extent }, color: [1.0; 4], texture: Some(key) }],
            texts: vec![],
        }
    }

    #[test]
    fn a_page_at_one_texel_a_pixel_is_copied() {
        let page = |rect, extent| Shape::Page { rect, radius: 8.0, square: 0, extent };
        let at = page(Rect::new(100.0, 50.0, 800.0, 600.0), [1.0, 1.0]);
        // The hole inside the corners: from the texture at (10, 10).
        assert_eq!(page_copy_from(&at, [1.0; 4], (800, 600), [110, 60, 780, 580]), Some([10, 10]));
        // Dimmed (a sleeping card), half a pixel off, or scaled: shaded.
        assert_eq!(page_copy_from(&at, [0.72, 0.72, 0.74, 1.0], (800, 600), [110, 60, 780, 580]), None);
        let off = page(Rect::new(100.5, 50.0, 800.0, 600.0), [1.0, 1.0]);
        assert_eq!(page_copy_from(&off, [1.0; 4], (800, 600), [110, 60, 780, 580]), None);
        assert_eq!(page_copy_from(&at, [1.0; 4], (1600, 1200), [110, 60, 780, 580]), None, "a 2× texture");
        // Painted smaller than its card (extent 1.25 wide): copied only
        // where the texture reaches, not where its left column repeats.
        let wide = page(Rect::new(100.0, 50.0, 1000.0, 600.0), [1.25, 1.0]);
        assert_eq!(page_copy_from(&wide, [1.0; 4], (800, 600), [110, 60, 700, 580]), Some([10, 10]));
        assert_eq!(page_copy_from(&wide, [1.0; 4], (800, 600), [110, 60, 980, 580]), None);
        // Cropped (extent below 1): the part shown is still 1:1.
        let crop = page(Rect::new(0.0, 0.0, 400.0, 600.0), [0.5, 1.0]);
        assert_eq!(page_copy_from(&crop, [1.0; 4], (800, 600), [5, 5, 390, 590]), Some([5, 5]));
        assert_eq!(page_copy_from(&Shape::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), radius: 0.0 }, [1.0; 4], (10, 10), [0, 0, 10, 10]), None);
    }

    #[test]
    fn a_white_page_inside_its_texture_is_plain() {
        let page = |extent| Shape::Page { rect: Rect::new(0.5, 0.0, 300.0, 200.0), radius: 8.0, square: 0, extent };
        assert!(page_plain(&page([1.0, 1.0]), [1.0; 4]));
        assert!(page_plain(&page([0.6, 0.9]), [1.0; 4]), "scaled or cropped");
        assert!(!page_plain(&page([1.2, 1.0]), [1.0; 4]), "the left column repeats");
        assert!(!page_plain(&page([1.0, 1.0]), [0.72, 0.72, 0.74, 1.0]), "dimmed");
        assert!(!page_plain(&Shape::Rect { rect: Rect::new(0.0, 0.0, 1.0, 1.0), radius: 0.0 }, [1.0; 4]));
    }

    #[test]
    fn page_damage_maps_texels_to_the_screen() {
        // A 200×100 page drawn at half size from (10, 20).
        let r = page_damage_on_screen(Rect::new(10.0, 20.0, 100.0, 50.0), [1.0, 1.0], (200, 100), [40, 20, 2, 18]);
        // 0.5 px a texel, plus the 2.5 px filtering margin.
        let want = Rect::new(30.0, 30.0, 1.0, 9.0).inflate(2.5);
        assert_eq!((r.x, r.y, r.w, r.h), (want.x, want.y, want.w, want.h));
    }

    #[test]
    fn page_damage_in_the_repeated_column_covers_the_rect() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        let r = page_damage_on_screen(rect, [1.5, 1.0], (200, 100), [0, 5, 3, 3]);
        assert_eq!((r.x, r.y, r.w, r.h), (8.0, 18.0, 104.0, 54.0));
        let inside = page_damage_on_screen(rect, [1.5, 1.0], (200, 100), [50, 5, 3, 3]);
        assert!(inside.w < 10.0, "away from the left column it stays small");
    }

    #[test]
    fn screen_damage_unions_cuts_and_gives_up_on_whole_pages() {
        let layers = [page_quad(Rect::new(0.0, 0.0, 100.0, 100.0), [1.0, 1.0], 1), page_quad(Rect::new(500.0, 0.0, 100.0, 100.0), [1.0, 1.0], 2)];
        let size = |_| Some((100, 100));
        let vp = (300.0, 300.0);
        let none = HashMap::new();
        assert_eq!(screen_damage(&layers, &none, size, vp).map(|r| r.w), Some(0.0), "nothing changed");
        let off = HashMap::from([(2, Some([0, 0, 10, 10]))]);
        assert_eq!(screen_damage(&layers, &off, size, vp).map(|r| r.w), Some(0.0), "card 2 is off screen");
        let caret = HashMap::from([(1, Some([50, 50, 2, 10]))]);
        let r = screen_damage(&layers, &caret, size, vp).expect("known");
        assert!(r.x > 40.0 && r.right() < 60.0 && r.h < 20.0, "{r:?}");
        let whole = HashMap::from([(1, None)]);
        assert!(screen_damage(&layers, &whole, size, vp).is_none());
    }

    #[test]
    fn rects_in_pixels() {
        assert_eq!(clamp_px([90, 40, 50, 50], 100, 50), [90, 40, 10, 10]);
        assert_eq!(clamp_px([200, 0, 5, 5], 100, 50), [100, 0, 0, 5]);
        assert_eq!(union_px([0, 0, 0, 0], [5, 5, 2, 2]), [5, 5, 2, 2], "an empty rect adds nothing");
        assert_eq!(union_px([1, 1, 2, 2], [5, 5, 2, 2]), [1, 1, 6, 6]);
        assert_eq!(scissor_px(Rect::new(-3.0, 2.5, 10.2, 400.0), 50, 100), [0, 2, 8, 98]);
    }

    #[test]
    fn frames_start_at_the_page_hiding_the_most() {
        let shadow = Quad {
            shape: Shape::Shadow { rect: Rect::new(0.0, 0.0, 400.0, 300.0), radius: 8.0, blur: 20.0 },
            color: [0.0, 0.0, 0.0, 0.5],
            texture: None,
        };
        let small = page_quad(Rect::new(10.0, 10.0, 100.0, 100.0), [1.0, 1.0], 1);
        let mut big = page_quad(Rect::new(100.0, 50.0, 400.0, 300.0), [1.0, 1.0], 2);
        big.quads.insert(0, shadow);
        let area = [0, 0, 1000, 800];
        let (layer, inst, hole) = cover(&[small.clone(), big.clone()], area).expect("a cover");
        // Layer 1's page follows its shadow: instance 2 in drawing order.
        assert_eq!((layer, inst), (1, 2));
        // In from the rounded corners (8 px: 3.34 px) and the blended edge.
        assert_eq!(hole, [104, 54, 392, 292]);
        // Only the part inside the area counts.
        assert_eq!(cover(&[big.clone()], [0, 0, 200, 200]).map(|c| c.2), Some([104, 54, 96, 146]));
        // A faded page, or one too small, hides nothing.
        let mut faded = big.clone();
        faded.quads[1].color[3] = 0.5;
        assert!(cover(&[faded], area).is_none());
        assert!(cover(&[page_quad(Rect::new(0.0, 0.0, 40.0, 40.0), [1.0, 1.0], 3)], area).is_none());
    }

    #[test]
    fn cut_leaves_strips_round_the_hole() {
        let area = [0, 0, 100, 80];
        let strips = cut_px(area, [10, 20, 30, 40]);
        assert_eq!(strips, vec![[0, 0, 100, 20], [0, 60, 100, 20], [0, 20, 10, 40], [40, 20, 60, 40]]);
        let covered: u32 = strips.iter().map(|r| r[2] * r[3]).sum::<u32>() + 30 * 40;
        assert_eq!(covered, 100 * 80, "strips and hole cover the area once");
        assert!(cut_px(area, area).is_empty());
        assert_eq!(intersect_px([0, 0, 10, 10], [20, 20, 5, 5])[2..], [0, 0]);
    }
}
