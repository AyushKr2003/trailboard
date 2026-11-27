//! A page's painted pixels (`PaintFrame`) and what changed in them.


use crate::popup::{self, IRect};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect { x, y, w, h }
    }

    fn frame() -> PaintFrame {
        PaintFrame { width: 100, height: 50, ..Default::default() }
    }

    #[test]
    fn changed_since_unions_the_paints_after_a_serial() {
        let mut f = frame();
        f.bump(Some(r(10, 10, 2, 16))); // 1: a caret
        f.bump(Some(r(40, 5, 10, 10))); // 2
        f.bump(Some(r(0, 30, 5, 5))); // 3
        assert_eq!(f.changed_since(3), Some(IRect::default()), "nothing new");
        assert_eq!(f.changed_since(2), Some(r(0, 30, 5, 5)));
        assert_eq!(f.changed_since(1), Some(r(0, 5, 50, 30)));
        assert_eq!(f.changed_since(0), Some(r(0, 5, 50, 30)));
    }

    #[test]
    fn changed_since_is_clamped_to_the_frame() {
        let mut f = frame();
        f.bump(Some(r(-5, 40, 200, 30)));
        assert_eq!(f.changed_since(0), Some(r(0, 40, 100, 10)));
    }

    #[test]
    fn changed_since_doesnt_know_after_a_whole_frame_change_or_when_too_far_behind() {
        let mut f = frame();
        f.bump(Some(r(1, 1, 1, 1)));
        f.bump(None); // a popup, a resize
        f.bump(Some(r(1, 1, 1, 1)));
        assert_eq!(f.changed_since(0), None);
        assert_eq!(f.changed_since(1), None);
        assert_eq!(f.changed_since(2), Some(r(1, 1, 1, 1)));
        for _ in 0..DAMAGE_KEPT {
            f.bump(Some(r(1, 1, 1, 1)));
        }
        assert_eq!(f.changed_since(2), None, "serial 3 is forgotten");
        assert_eq!(f.changed_since(3), Some(r(1, 1, 1, 1)));
    }

    #[test]
    fn a_paint_is_matched_to_the_view_size_it_was_laid_out_at() {
        // Card 1024×720 at 0.5, then read at 1440×866 at 2.
        let recent = [((1440, 866), 2.0), ((1024, 720), 0.5)];
        assert_eq!(paint_css((2880, 1732), &recent), (1440.0, 866.0));
        // The old paint, still showing after the resize.
        assert_eq!(paint_css((512, 360), &recent), (1024.0, 720.0));
        // Rounding is tolerated.
        assert_eq!(paint_css((513, 360), &recent), (1024.0, 720.0));
        // Unknown: the pixels over the newest scale.
        assert_eq!(paint_css((800, 600), &recent), (400.0, 300.0));
        assert_eq!(paint_css((10, 10), &[]), (10.0, 10.0));
    }
}
