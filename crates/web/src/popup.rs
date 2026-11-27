//! Popup widgets (`<select>` lists, date pickers, autofill) in off-screen mode.
//! CEF paints them into a separate buffer (`PET_POPUP`) with a rect in view
//! coordinates; we draw that buffer over the page frame. Pure helpers here.

/// Integer rectangle in view coordinates (CSS px unless stated).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub fn scaled(&self, s: f32) -> IRect {
        let f = |v: i32| (v as f32 * s).round() as i32;
        IRect { x: f(self.x), y: f(self.y), w: f(self.w), h: f(self.h) }
    }
}

/// Move a popup rect so it lies inside a `view_w`×`view_h` view where it can
/// (Chromium places it as if the page had a whole screen below it; our card is
/// smaller). A popup larger than the view keeps its top-left corner in view.
/// Smallest rect holding every rect in `rects` (`None` if empty).
pub fn bounding(rects: &[IRect]) -> Option<IRect> {
    let first = rects.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x + first.w, first.y + first.h);
    for r in &rects[1..] {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w);
        y1 = y1.max(r.y + r.h);
    }
    Some(IRect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
}

pub fn clamp_into_view(r: IRect, view_w: i32, view_h: i32) -> IRect {
    let mut out = r;
    if out.x + out.w > view_w {
        out.x = view_w - out.w;
    }
    if out.y + out.h > view_h {
        out.y = view_h - out.h;
    }
    out.x = out.x.max(0);
    out.y = out.y.max(0);
    out
}

/// Copy a 4-byte-per-pixel image of `src_size` (w, h) onto one of `dst_size`
/// with its top-left at `at`, clipping to the destination.
pub fn blit(dst: &mut [u8], dst_size: (u32, u32), src: &[u8], src_size: (u32, u32), at: (i32, i32)) {
    let (dw, dh, sw, sh) = (dst_size.0 as i64, dst_size.1 as i64, src_size.0 as i64, src_size.1 as i64);
    let (x, y) = (at.0 as i64, at.1 as i64);
    let x0 = x.max(0);
    let x1 = (x + sw).min(dw);
    if x0 >= x1 {
        return;
    }
    for row in y.max(0)..(y + sh).min(dh) {
        let sy = row - y;
        let s = ((sy * sw + (x0 - x)) * 4) as usize;
        let d = ((row * dw + x0) * 4) as usize;
        let n = ((x1 - x0) * 4) as usize;
        if s + n <= src.len() && d + n <= dst.len() {
            dst[d..d + n].copy_from_slice(&src[s..s + n]);
        }
    }
}

/// Bring `dst` up to date with `src`, a `width`×`height` 4-byte-per-pixel
/// image of which only `dirty` changed since `dst` last matched it. Copies
/// each dirty rect's rows (clipped), or the whole image when `dst` isn't the
/// same size (`same_size` false or another length) or nothing says what
/// changed. Returns the bytes copied.
pub fn copy_dirty(dst: &mut Vec<u8>, src: &[u8], width: u32, height: u32, same_size: bool, dirty: &[IRect]) -> usize {
    if !same_size || dst.len() != src.len() || dirty.is_empty() {
        dst.clear();
        dst.extend_from_slice(src);
        return src.len();
    }
    let (w, h) = (width as i64, height as i64);
    let mut copied = 0;
    for r in dirty {
        let (x0, y0) = ((r.x as i64).max(0), (r.y as i64).max(0));
        let (x1, y1) = ((r.x as i64 + r.w as i64).min(w), (r.y as i64 + r.h as i64).min(h));
        if x0 >= x1 || y0 >= y1 {
            continue;
        }
        let n = ((x1 - x0) * 4) as usize;
        for row in y0..y1 {
            let o = ((row * w + x0) * 4) as usize;
            dst[o..o + n].copy_from_slice(&src[o..o + n]);
            copied += n;
        }
    }
    copied
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect { x, y, w, h }
    }

    #[test]
    fn clamp_keeps_fitting_popup() {
        assert_eq!(clamp_into_view(r(10, 20, 100, 50), 400, 300), r(10, 20, 100, 50));
    }

    #[test]
    fn clamp_moves_overflow_back_in() {
        assert_eq!(clamp_into_view(r(350, 280, 100, 50), 400, 300), r(300, 250, 100, 50));
        assert_eq!(clamp_into_view(r(-5, -8, 10, 10), 400, 300), r(0, 0, 10, 10));
    }

    #[test]
    fn clamp_oversized_keeps_top_left() {
        assert_eq!(clamp_into_view(r(50, 50, 500, 400), 400, 300), r(0, 0, 500, 400));
    }

    #[test]
    fn bounding_unions() {
        assert_eq!(bounding(&[]), None);
        assert_eq!(bounding(&[r(5, 5, 0, 10)]), Some(r(5, 5, 0, 10)));
        assert_eq!(bounding(&[r(5, 5, 2, 10), r(20, 3, 4, 10)]), Some(r(5, 3, 19, 12)));
    }

    #[test]
    fn scaled_rounds() {
        assert_eq!(r(1, 2, 3, 5).scaled(1.5), r(2, 3, 5, 8));
    }

    fn img(w: u32, h: u32, v: u8) -> Vec<u8> {
        vec![v; (w * h * 4) as usize]
    }

    fn px(buf: &[u8], w: u32, x: u32, y: u32) -> u8 {
        buf[((y * w + x) * 4) as usize]
    }

    #[test]
    fn blit_inside() {
        let mut d = img(4, 4, 0);
        blit(&mut d, (4, 4), &img(2, 2, 9), (2, 2), (1, 1));
        assert_eq!(px(&d, 4, 0, 0), 0);
        assert_eq!(px(&d, 4, 1, 1), 9);
        assert_eq!(px(&d, 4, 2, 2), 9);
        assert_eq!(px(&d, 4, 3, 3), 0);
    }

    #[test]
    fn blit_clips_every_side() {
        let mut d = img(4, 4, 0);
        blit(&mut d, (4, 4), &img(6, 6, 7), (6, 6), (-1, -1));
        assert!(d.iter().all(|&b| b == 7));
        let mut d = img(4, 4, 0);
        blit(&mut d, (4, 4), &img(2, 2, 7), (2, 2), (3, 3));
        assert_eq!(px(&d, 4, 3, 3), 7);
        assert_eq!(d.iter().filter(|&&b| b == 7).count(), 4); // one pixel
        let mut d = img(4, 4, 0);
        blit(&mut d, (4, 4), &img(2, 2, 7), (2, 2), (10, 0));
        assert!(d.iter().all(|&b| b == 0));
    }

    #[test]
    fn copy_dirty_takes_only_the_dirty_rows() {
        let src = img(4, 3, 5);
        let mut d = img(4, 3, 0);
        let n = copy_dirty(&mut d, &src, 4, 3, true, &[r(1, 1, 2, 1), r(3, 2, 5, 5)]);
        assert_eq!(n, (2 + 1) * 4, "two pixels, then one clipped to the frame");
        let fives: Vec<(u32, u32)> = (0..3).flat_map(|y| (0..4).map(move |x| (x, y))).filter(|&(x, y)| px(&d, 4, x, y) == 5).collect();
        assert_eq!(fives, vec![(1, 1), (2, 1), (3, 2)]);
        assert_eq!(copy_dirty(&mut d, &src, 4, 3, true, &[r(-9, -9, 2, 2)]), 0, "outside");
    }

    #[test]
    fn copy_dirty_takes_everything_when_it_cant_tell() {
        let src = img(4, 3, 5);
        for (mut d, same, dirty) in [
            (img(4, 3, 0), false, vec![r(0, 0, 1, 1)]),
            (Vec::new(), true, vec![r(0, 0, 1, 1)]),
            (img(4, 3, 0), true, vec![]),
        ] {
            assert_eq!(copy_dirty(&mut d, &src, 4, 3, same, &dirty), src.len());
            assert_eq!(d, src);
        }
    }
}
