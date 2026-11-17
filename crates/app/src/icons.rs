//! Trailboard's own icons: line drawings on a 24-unit grid, rasterised at
//! the exact pixel size they are shown at, so their strokes stay crisp at any
//! UI scale. Each is white with coverage in alpha, and is tinted when drawn.
//!
//! An icon is a few strokes (lines, arcs, curves) and dots. A pixel's
//! coverage comes from its distance to the nearest stroke, which gives
//! round caps and joins and smooth edges with no path filling.

use compositor::{Layer, Quad, Renderer, Shape};
use scene::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Plus,
    Minus,
    Close,
    Search,
    Globe,
    Back,
    Forward,
    Reload,
    Note,
    Sticky,
    Tidy,
    Stack,
    Fit,
    Sidebar,
    More,
    Settings,
    History,
    Download,
    Shield,
    ChevronRight,
    ChevronDown,
    Lock,
    Expand,
    Keyboard,
    Trail,
    Undo,
    Moon,
    Help,
    Check,
    Board,
    Grip,
    Find,
    Picture,
}

enum Prim {
    /// A stroked segment.
    Line([f32; 2], [f32; 2]),
    /// A stroked arc of a circle, from `from` to `to` radians (clockwise on screen, `to` > `from`).
    Arc { c: [f32; 2], r: f32, from: f32, to: f32 },
    /// A filled circle.
    Dot { c: [f32; 2], r: f32 },
}

/// Collects an icon's strokes.
#[derive(Default)]
struct Pen(Vec<Prim>);

impl Pen {
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) -> &mut Self {
        self.0.push(Prim::Line([x0, y0], [x1, y1]));
        self
    }

    fn poly(&mut self, pts: &[(f32, f32)]) -> &mut Self {
        for w in pts.windows(2) {
            self.line(w[0].0, w[0].1, w[1].0, w[1].1);
        }
        self
    }

    /// An arc from `from` to `to` degrees: 0 points right, 90 down.
    fn arc(&mut self, cx: f32, cy: f32, r: f32, from: f32, to: f32) -> &mut Self {
        self.0.push(Prim::Arc { c: [cx, cy], r, from: from.to_radians(), to: to.to_radians() });
        self
    }

    fn circle(&mut self, cx: f32, cy: f32, r: f32) -> &mut Self {
        self.arc(cx, cy, r, 0.0, 360.0)
    }

    fn dot(&mut self, cx: f32, cy: f32, r: f32) -> &mut Self {
        self.0.push(Prim::Dot { c: [cx, cy], r });
        self
    }

    /// The outline of a rectangle with rounded corners.
    fn rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) -> &mut Self {
        let (x1, y1) = (x + w, y + h);
        self.line(x + r, y, x1 - r, y).line(x1, y + r, x1, y1 - r).line(x1 - r, y1, x + r, y1).line(x, y1 - r, x, y + r);
        self.arc(x1 - r, y + r, r, 270.0, 360.0).arc(x1 - r, y1 - r, r, 0.0, 90.0);
        self.arc(x + r, y1 - r, r, 90.0, 180.0).arc(x + r, y + r, r, 180.0, 270.0)
    }

    /// An ellipse, as short segments.
    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) -> &mut Self {
        let pts: Vec<(f32, f32)> = (0..=32)
            .map(|i| {
                let a = i as f32 / 32.0 * std::f32::consts::TAU;
                (cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect();
        self.poly(&pts)
    }

    /// A cubic Bézier curve, as short segments.
    fn curve(&mut self, p: [(f32, f32); 4]) -> &mut Self {
        let pts: Vec<(f32, f32)> = (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0;
                let u = 1.0 - t;
                let b = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                ((0..4).map(|k| b[k] * p[k].0).sum(), (0..4).map(|k| b[k] * p[k].1).sum())
            })
            .collect();
        self.poly(&pts)
    }
}

fn strokes(icon: Icon) -> Vec<Prim> {
    let mut p = Pen::default();
    match icon {
        Icon::Plus => {
            p.line(12.0, 5.0, 12.0, 19.0).line(5.0, 12.0, 19.0, 12.0);
        }
        Icon::Minus => {
            p.line(5.0, 12.0, 19.0, 12.0);
        }
        Icon::Close => {
            p.line(6.5, 6.5, 17.5, 17.5).line(17.5, 6.5, 6.5, 17.5);
        }
        Icon::Search => {
            p.circle(11.0, 11.0, 6.5).line(15.8, 15.8, 20.0, 20.0);
        }
        Icon::Find => {
            // A magnifier over lines of text.
            p.circle(14.5, 14.0, 4.5).line(17.8, 17.3, 20.5, 20.0);
            p.line(4.0, 6.0, 16.0, 6.0).line(4.0, 11.0, 7.5, 11.0).line(4.0, 16.0, 7.5, 16.0);
        }
        Icon::Globe => {
            p.circle(12.0, 12.0, 8.5).line(3.5, 12.0, 20.5, 12.0).ellipse(12.0, 12.0, 3.7, 8.5);
        }
        Icon::Back => {
            p.line(19.0, 12.0, 5.5, 12.0).poly(&[(11.5, 6.0), (5.5, 12.0), (11.5, 18.0)]);
        }
        Icon::Forward => {
            p.line(5.0, 12.0, 18.5, 12.0).poly(&[(12.5, 6.0), (18.5, 12.0), (12.5, 18.0)]);
        }
        Icon::Reload => {
            // Most of a circle, clockwise, ending in an arrowhead at the top right.
            p.arc(12.0, 12.0, 7.5, 0.0, 305.0).poly(&[(19.5, 4.0), (19.5, 9.0), (14.5, 9.0)]);
        }
        Icon::Note => {
            p.rrect(5.0, 3.0, 14.0, 18.0, 2.0);
            p.line(8.5, 8.0, 15.5, 8.0).line(8.5, 12.0, 15.5, 12.0).line(8.5, 16.0, 12.5, 16.0);
        }
        Icon::Sticky => {
            // A square of paper with its corner turned up.
            p.poly(&[(14.0, 20.0), (5.0, 20.0), (4.0, 19.0), (4.0, 5.0), (5.0, 4.0), (19.0, 4.0), (20.0, 5.0), (20.0, 14.0), (14.0, 20.0)]);
            p.poly(&[(14.0, 20.0), (14.0, 14.0), (20.0, 14.0)]);
        }
        Icon::Tidy => {
            // A page and the two it led to, in columns.
            p.rrect(3.0, 9.0, 6.0, 6.0, 1.5).rrect(15.0, 3.5, 6.0, 6.0, 1.5).rrect(15.0, 14.5, 6.0, 6.0, 1.5);
            p.line(9.0, 12.0, 12.0, 12.0).poly(&[(15.0, 6.5), (12.0, 6.5), (12.0, 17.5), (15.0, 17.5)]);
        }
        Icon::Stack => {
            // A card with the edges of another behind it.
            p.rrect(4.0, 8.5, 13.0, 11.5, 2.0);
            p.poly(&[(7.5, 8.5), (7.5, 6.0), (8.5, 5.0), (19.0, 5.0), (20.0, 6.0), (20.0, 15.5), (19.0, 16.5), (17.0, 16.5)]);
        }
        Icon::Fit => {
            p.poly(&[(4.0, 9.0), (4.0, 4.0), (9.0, 4.0)]).poly(&[(15.0, 4.0), (20.0, 4.0), (20.0, 9.0)]);
            p.poly(&[(20.0, 15.0), (20.0, 20.0), (15.0, 20.0)]).poly(&[(9.0, 20.0), (4.0, 20.0), (4.0, 15.0)]);
        }
        Icon::Sidebar => {
            p.rrect(3.0, 4.0, 18.0, 16.0, 2.5).line(9.0, 4.0, 9.0, 20.0);
        }
        Icon::More => {
            p.dot(6.0, 12.0, 1.5).dot(12.0, 12.0, 1.5).dot(18.0, 12.0, 1.5);
        }
        Icon::Settings => {
            // Two sliders.
            p.line(4.0, 7.5, 12.5, 7.5).circle(15.5, 7.5, 2.4).line(18.5, 7.5, 20.0, 7.5);
            p.line(4.0, 16.5, 5.5, 16.5).circle(8.5, 16.5, 2.4).line(11.5, 16.5, 20.0, 16.5);
        }
        Icon::History => {
            p.circle(12.0, 12.0, 8.5).poly(&[(12.0, 7.0), (12.0, 12.0), (15.5, 14.0)]);
        }
        Icon::Download => {
            p.line(12.0, 4.0, 12.0, 15.0).poly(&[(7.0, 10.5), (12.0, 15.5), (17.0, 10.5)]).line(5.0, 20.0, 19.0, 20.0);
        }
        Icon::Shield => {
            p.poly(&[(12.0, 3.0), (19.0, 5.8), (19.0, 11.5), (17.4, 16.2), (12.0, 21.0), (6.6, 16.2), (5.0, 11.5), (5.0, 5.8), (12.0, 3.0)]);
        }
        Icon::ChevronRight => {
            p.poly(&[(9.0, 6.0), (15.0, 12.0), (9.0, 18.0)]);
        }
        Icon::ChevronDown => {
            p.poly(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]);
        }
        Icon::Lock => {
            p.rrect(5.0, 11.0, 14.0, 9.5, 2.0).arc(12.0, 8.0, 4.0, 180.0, 360.0).line(8.0, 8.0, 8.0, 11.0).line(16.0, 8.0, 16.0, 11.0);
        }
        Icon::Expand => {
            p.poly(&[(14.0, 4.0), (20.0, 4.0), (20.0, 10.0)]).line(20.0, 4.0, 13.5, 10.5);
            p.poly(&[(10.0, 20.0), (4.0, 20.0), (4.0, 14.0)]).line(4.0, 20.0, 10.5, 13.5);
        }
        Icon::Keyboard => {
            p.rrect(3.5, 6.0, 17.0, 12.0, 2.5).dot(7.5, 10.3, 1.0).dot(12.0, 10.3, 1.0).dot(16.5, 10.3, 1.0).line(8.5, 14.3, 15.5, 14.3);
        }
        Icon::Trail => {
            // Two pages joined by the line a link leaves.
            p.circle(6.0, 6.5, 2.5).circle(18.0, 17.5, 2.5);
            p.curve([(8.5, 6.5), (14.0, 6.5), (10.0, 17.5), (15.5, 17.5)]);
        }
        Icon::Undo => {
            p.poly(&[(8.5, 5.0), (4.0, 9.5), (8.5, 14.0)]).line(4.0, 9.5, 14.5, 9.5);
            p.arc(14.5, 14.25, 4.75, 270.0, 450.0).line(14.5, 19.0, 10.0, 19.0);
        }
        Icon::Moon => {
            // A disc with a bite taken out of its upper right.
            let (c1, r1, c2, r2) = ((11.5_f32, 12.5_f32), 8.0_f32, (16.5_f32, 7.5_f32), 6.6_f32);
            let on = |c: (f32, f32), r: f32, a: f32| (c.0 + r * a.cos(), c.1 + r * a.sin());
            let far = |p: (f32, f32), c: (f32, f32)| ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt();
            let n = 96;
            for (c, r, other, limit, outside) in [(c1, r1, c2, r2, true), (c2, r2, c1, r1, false)] {
                for i in 0..n {
                    let (a, b) = (on(c, r, i as f32 / n as f32 * std::f32::consts::TAU), on(c, r, (i + 1) as f32 / n as f32 * std::f32::consts::TAU));
                    let keep = |q: (f32, f32)| (far(q, other) >= limit) == outside;
                    if keep(a) && keep(b) {
                        p.line(a.0, a.1, b.0, b.1);
                    }
                }
            }
        }
        Icon::Help => {
            p.circle(12.0, 12.0, 8.5).arc(12.0, 9.8, 2.7, 185.0, 420.0).line(13.35, 12.14, 12.0, 13.9).dot(12.0, 16.9, 1.15);
        }
        Icon::Check => {
            p.poly(&[(5.0, 12.5), (10.0, 17.5), (19.0, 7.0)]);
        }
        Icon::Board => {
            p.rrect(3.0, 4.0, 18.0, 16.0, 2.5).rrect(6.5, 8.0, 4.5, 4.0, 1.0).rrect(13.5, 11.5, 4.5, 4.5, 1.0);
        }
        Icon::Grip => {
            p.line(20.0, 9.0, 9.0, 20.0).line(20.0, 15.0, 15.0, 20.0);
        }
        Icon::Picture => {
            // A framed picture: a sun over two hills.
            p.rrect(3.5, 4.5, 17.0, 15.0, 2.5).circle(15.5, 9.5, 1.8);
            p.poly(&[(4.0, 17.5), (9.5, 11.5), (14.0, 16.0), (16.0, 14.0), (20.0, 18.0)]);
        }
    }
    p.0
}

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

fn arc_dist(p: [f32; 2], c: [f32; 2], r: f32, from: f32, to: f32) -> f32 {
    let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
    let tau = std::f32::consts::TAU;
    if to - from >= tau - 1e-3 {
        return ((dx * dx + dy * dy).sqrt() - r).abs();
    }
    // The point's angle, taken to the turn that starts at `from`.
    let a = from + (dy.atan2(dx) - from).rem_euclid(tau);
    if a <= to {
        return ((dx * dx + dy * dy).sqrt() - r).abs();
    }
    let end = |t: f32| ((p[0] - c[0] - r * t.cos()).powi(2) + (p[1] - c[1] - r * t.sin()).powi(2)).sqrt();
    end(from).min(end(to))
}

/// Stroke width in px for an icon drawn `px` tall: 1.5 px at 16, never under 1.2.
fn stroke_px(px: u32) -> f32 {
    (px as f32 * 0.094).max(1.2)
}

/// `icon` as `px` × `px` BGRA pixels: white, with coverage in alpha.
pub fn raster(icon: Icon, px: u32) -> Vec<u8> {
    let prims = strokes(icon);
    let unit = px as f32 / 24.0;
    let half = stroke_px(px) / 2.0;
    let mut out = vec![255u8; (px * px * 4) as usize];
    for y in 0..px {
        for x in 0..px {
            let g = [(x as f32 + 0.5) / unit, (y as f32 + 0.5) / unit];
            let mut cover = 0.0_f32;
            for prim in &prims {
                // Distance past the shape's edge, in px.
                let d = match *prim {
                    Prim::Line(a, b) => seg_dist(g, a, b) * unit - half,
                    Prim::Arc { c, r, from, to } => arc_dist(g, c, r, from, to) * unit - half,
                    Prim::Dot { c, r } => (((g[0] - c[0]).powi(2) + (g[1] - c[1]).powi(2)).sqrt() - r) * unit,
                };
                cover = cover.max((0.5 - d).clamp(0.0, 1.0));
            }
            out[((y * px + x) * 4 + 3) as usize] = (cover * 255.0).round() as u8;
        }
    }
    out
}

/// Texture keys for icons: above every page, site-icon and preview key.
const KEY: u64 = 1 << 61;

/// The texture of `icon` for drawing it `size` px tall (uploaded on first
/// use), and the size to draw it at. Sizes are whole pixels, so an icon is
/// never resampled in the chrome; on the board, where sizes follow the zoom,
/// icons come in steps of 4 px and are scaled down a little.
pub fn texture(r: &mut Renderer, icon: Icon, size: f32, exact: bool) -> (u64, f32) {
    let px = if exact { size.round().clamp(8.0, 96.0) as u32 } else { (((size / 4.0).ceil() * 4.0) as u32).clamp(8, 96) };
    let key = KEY | ((icon as u64) << 12) | px as u64;
    if !r.has_page(key) {
        r.update_page(key, px, px, &raster(icon, px), 1);
    }
    (key, if exact { px as f32 } else { size })
}

/// Draws `icon` centred on (`cx`, `cy`), `size` px tall, in `colour`.
pub fn draw(l: &mut Layer, r: &mut Renderer, icon: Icon, cx: f32, cy: f32, size: f32, colour: [f32; 4]) {
    let (key, px) = texture(r, icon, size, true);
    // On whole pixels: a half-pixel offset would blur every stroke.
    let rect = Rect::new((cx - px / 2.0).round(), (cy - px / 2.0).round(), px, px);
    l.quads.push(Quad { shape: Shape::Rect { rect, radius: 0.0 }, color: colour, texture: Some(key) });
}

/// Draws `icon` filling `rect` (a square that follows the board's zoom).
pub fn draw_scaled(l: &mut Layer, r: &mut Renderer, icon: Icon, rect: Rect, colour: [f32; 4]) {
    let (key, _) = texture(r, icon, rect.w, false);
    l.quads.push(Quad { shape: Shape::Rect { rect, radius: 0.0 }, color: colour, texture: Some(key) });
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[Icon] = &[
        Icon::Plus, Icon::Minus, Icon::Close, Icon::Search, Icon::Globe, Icon::Back, Icon::Forward, Icon::Reload,
        Icon::Note, Icon::Sticky, Icon::Tidy, Icon::Stack, Icon::Fit, Icon::Sidebar, Icon::More, Icon::Settings,
        Icon::History, Icon::Download, Icon::Shield, Icon::ChevronRight, Icon::ChevronDown, Icon::Lock, Icon::Expand,
        Icon::Keyboard, Icon::Trail, Icon::Undo, Icon::Moon, Icon::Help, Icon::Check, Icon::Board, Icon::Grip,
        Icon::Find, Icon::Picture,
    ];

    fn alpha(px: &[u8], size: u32, x: u32, y: u32) -> u8 {
        px[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn every_icon_draws_inside_its_box_at_every_size() {
        for &icon in ALL {
            for size in [12u32, 16, 20, 24, 32] {
                let px = raster(icon, size);
                assert_eq!(px.len(), (size * size * 4) as usize);
                let inked = (0..size * size).filter(|i| px[(*i * 4 + 3) as usize] > 128).count();
                assert!(inked >= 6, "{icon:?} at {size}: nearly empty");
                assert!(inked < (size * size) as usize / 2, "{icon:?} at {size}: mostly filled");
                // Nothing touches the outer edge, so a neighbour never clips it.
                for i in 0..size {
                    for (x, y) in [(i, 0), (i, size - 1), (0, i), (size - 1, i)] {
                        assert!(alpha(&px, size, x, y) < 80, "{icon:?} at {size}: ink on the edge ({x}, {y})");
                    }
                }
            }
        }
    }

    #[test]
    fn strokes_are_white_and_about_a_pixel_and_a_half_wide() {
        let px = raster(Icon::Minus, 16);
        assert!(px.chunks(4).all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255));
        // The bar's column of coverage through the middle adds up to its width.
        let width: f32 = (0..16).map(|y| alpha(&px, 16, 8, y) as f32 / 255.0).sum();
        assert!((width - 1.5).abs() < 0.1, "{width}");
        assert_eq!(stroke_px(8), 1.2);
    }

    #[test]
    fn arcs_measure_to_the_curve_and_its_ends() {
        let quarter = (0.0, std::f32::consts::FRAC_PI_2);
        // On the curve, inside the sweep.
        let on = [10.0 * (0.5_f32).cos(), 10.0 * (0.5_f32).sin()];
        assert!(arc_dist(on, [0.0, 0.0], 10.0, quarter.0, quarter.1) < 1e-4);
        // Opposite the sweep: the distance to the nearer end, not to the circle.
        assert!((arc_dist([-10.0, 0.0], [0.0, 0.0], 10.0, quarter.0, quarter.1) - 200.0_f32.sqrt()).abs() < 1e-3);
        assert!((seg_dist([5.0, 3.0], [0.0, 0.0], [10.0, 0.0]) - 3.0).abs() < 1e-5);
    }
}
