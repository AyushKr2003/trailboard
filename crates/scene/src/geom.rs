use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

/// Axis-aligned rectangle: top-left corner plus size.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }

    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }

    pub fn translate(&self, d: Vec2) -> Rect {
        Rect::new(self.x + d.x, self.y + d.y, self.w, self.h)
    }

    pub fn inflate(&self, m: f32) -> Rect {
        Rect::new(self.x - m, self.y - m, self.w + 2.0 * m, self.h + 2.0 * m)
    }
}

/// Maps world coordinates (card units, ≈ CSS px) to screen pixels.
///
/// `scale` is the window's device scale factor, so a card at `zoom == 1.0`
/// is drawn at its natural CSS size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    /// World point shown at the centre of the viewport.
    pub center: Vec2,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { center: Vec2::default(), zoom: 1.0 }
    }
}

pub const MIN_ZOOM: f32 = 0.05;
pub const MAX_ZOOM: f32 = 4.0;

impl Camera {
    /// Pixels per world unit.
    pub fn pixels_per_unit(&self, scale: f32) -> f32 {
        self.zoom * scale
    }

    pub fn world_to_screen(&self, p: Vec2, viewport: Vec2, scale: f32) -> Vec2 {
        (p - self.center) * self.pixels_per_unit(scale) + viewport * 0.5
    }

    pub fn screen_to_world(&self, p: Vec2, viewport: Vec2, scale: f32) -> Vec2 {
        (p - viewport * 0.5) * (1.0 / self.pixels_per_unit(scale)) + self.center
    }

    pub fn rect_to_screen(&self, r: Rect, viewport: Vec2, scale: f32) -> Rect {
        let tl = self.world_to_screen(Vec2::new(r.x, r.y), viewport, scale);
        let k = self.pixels_per_unit(scale);
        Rect::new(tl.x, tl.y, r.w * k, r.h * k)
    }

    /// Pan by a screen-space delta (pixels).
    pub fn pan_pixels(&mut self, d: Vec2, scale: f32) {
        self.center = self.center - d * (1.0 / self.pixels_per_unit(scale));
    }

    /// Zoom by `factor`, keeping the world point under `anchor` (screen px) fixed.
    pub fn zoom_about(&mut self, factor: f32, anchor: Vec2, viewport: Vec2, scale: f32) {
        let before = self.screen_to_world(anchor, viewport, scale);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.screen_to_world(anchor, viewport, scale);
        self.center = self.center + (before - after);
    }

    /// Camera that fits `r` inside the viewport with a margin.
    pub fn fit(r: Rect, viewport: Vec2, scale: f32, margin_px: f32) -> Camera {
        let avail = Vec2::new((viewport.x - 2.0 * margin_px).max(1.0), (viewport.y - 2.0 * margin_px).max(1.0));
        let zoom = (avail.x / (r.w * scale)).min(avail.y / (r.h * scale)).clamp(MIN_ZOOM, MAX_ZOOM);
        Camera { center: r.center(), zoom }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_world_round_trip() {
        let cam = Camera { center: Vec2::new(100.0, -50.0), zoom: 0.5 };
        let vp = Vec2::new(1920.0, 1080.0);
        let p = Vec2::new(321.0, 654.0);
        let back = cam.world_to_screen(cam.screen_to_world(p, vp, 2.0), vp, 2.0);
        assert!((back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3);
    }

    #[test]
    fn zoom_keeps_anchor_fixed() {
        let mut cam = Camera::default();
        let vp = Vec2::new(1000.0, 800.0);
        let anchor = Vec2::new(200.0, 300.0);
        let before = cam.screen_to_world(anchor, vp, 1.0);
        cam.zoom_about(1.7, anchor, vp, 1.0);
        let after = cam.screen_to_world(anchor, vp, 1.0);
        assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
    }

    #[test]
    fn fit_contains_rect() {
        let r = Rect::new(-500.0, -200.0, 3000.0, 1000.0);
        let vp = Vec2::new(1280.0, 720.0);
        let cam = Camera::fit(r, vp, 1.0, 40.0);
        let s = cam.rect_to_screen(r, vp, 1.0);
        assert!(s.x >= 39.0 && s.right() <= vp.x - 39.0);
        assert!(s.y >= 0.0 && s.bottom() <= vp.y);
    }
}
