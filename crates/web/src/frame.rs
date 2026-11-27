//! A page's painted pixels (`PaintFrame`) and what changed in them.

use crate::*;

/// Latest painted frame of a page, BGRA8, tightly packed.
#[derive(Default)]
pub struct PaintFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Bumped on every paint so consumers can upload only when it changes.
    pub serial: u64,
    /// The view size in CSS px these pixels were laid out at. Right after a
    /// resize it is still the old size until Chromium paints again, so the
    /// app can show the old pixels at their own size instead of stretching
    /// them over the new one.
    pub css: (f32, f32),
    /// What the last few serials changed, oldest first: `(serial, rect in
    /// pixels)`, `None` for the whole frame. See [`PaintFrame::changed_since`].
    pub(crate) damage: Vec<(u64, Option<IRect>)>,
}

/// Serials remembered in [`PaintFrame::damage`]; a consumer further behind
/// uploads the whole frame.
pub(crate) const DAMAGE_KEPT: usize = 16;

impl PaintFrame {
    /// A new serial, which changed `rect` (`None`: everything, or unknown).
    pub(crate) fn bump(&mut self, rect: Option<IRect>) {
        self.serial += 1;
        if self.damage.len() == DAMAGE_KEPT {
            self.damage.remove(0);
        }
        self.damage.push((self.serial, rect));
    }

    /// The pixels that changed after `serial` up to now, as one rect (clamped
    /// to the frame), or `None` if that isn't known (too far behind, a resize,
    /// a popup), in which case the whole frame should be taken.
    pub fn changed_since(&self, serial: u64) -> Option<IRect> {
        if serial >= self.serial {
            return Some(IRect::default());
        }
        let first = self.damage.first()?.0;
        if first > serial + 1 {
            return None; // forgotten
        }
        let rects: Option<Vec<IRect>> = self.damage.iter().filter(|(s, _)| *s > serial).map(|(_, r)| *r).collect();
        let r = popup::bounding(&rects?)?;
        let (x0, y0) = (r.x.max(0), r.y.max(0));
        let (x1, y1) = ((r.x + r.w).min(self.width as i32), (r.y + r.h).min(self.height as i32));
        Some(IRect { x: x0, y: y0, w: (x1 - x0).max(0), h: (y1 - y0).max(0) })
    }
}

/// The CSS size a `px` (width, height) paint was laid out at: the newest of
/// the `recent` view sizes (CSS px, raster scale), newest first, whose pixel
/// size it matches (±1 px for rounding); else `px` over the newest scale.
pub fn paint_css(px: (u32, u32), recent: &[((i32, i32), f32)]) -> (f32, f32) {
    let near = |a: f32, b: u32| (a.round() - b as f32).abs() <= 1.0;
    for &((w, h), s) in recent {
        if near(w as f32 * s, px.0) && near(h as f32 * s, px.1) {
            return (w as f32, h as f32);
        }
    }
    let s = recent.first().map_or(1.0, |r| r.1).max(0.01);
    (px.0 as f32 / s, px.1 as f32 / s)
}
