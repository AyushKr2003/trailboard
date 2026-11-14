//! The look shared by everything drawn over the board: panels with a layered
//! shadow, a hairline edge and a faint top sheen; the dimmed backdrop behind
//! modal lists; keycap chips; and the short ease-in that overlays appear with.
//! Corners still follow the theme's rounding (sharp on Omarchy's default).

use std::time::{Duration, Instant};

use compositor::{Layer, Quad, Shape, Text};
use scene::{Rect, Vec2};

use crate::theme::{Rgb, Theme, mix, rgb};

/// How far a surface floats above the board: the size of its shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lift {
    /// Labels and chips that sit on the board.
    Low,
    /// Menus, the sidebar, toasts.
    Mid,
    /// The palette, the shortcut sheet: modal surfaces.
    High,
}

/// A two-part shadow under `rect`: a tight, darker contact shadow and a wide,
/// soft ambient one, which together read as depth rather than a smudge.
pub fn lift(l: &mut Layer, rect: Rect, radius: f32, th: &Theme, ui: f32, lift: Lift) {
    let (key, ambient) = match lift {
        // (offset y, blur, strength)
        Lift::Low => ((1.0, 3.0, 0.30), (4.0, 12.0, 0.22)),
        Lift::Mid => ((2.0, 5.0, 0.32), (10.0, 28.0, 0.34)),
        Lift::High => ((3.0, 8.0, 0.36), (20.0, 52.0, 0.46)),
    };
    for (dy, blur, a) in [ambient, key] {
        l.quads.push(Quad {
            shape: Shape::Shadow { rect: rect.translate(Vec2::new(0.0, dy * ui)), radius, blur: blur * ui },
            color: th.shadow(a),
            texture: None,
        });
    }
}

/// The hairline around a panel: a step lighter than the panel on dark
/// themes, a step darker on light ones.
pub fn edge(th: &Theme) -> Rgb {
    if th.light() { mix(th.panel(), [0, 0, 0], 0.13) } else { mix(th.panel(), th.foreground(), 0.13) }
}

/// A surface over the board: shadow, hairline edge, `fill`, and a faint
/// highlight along its top edge.
pub fn panel(l: &mut Layer, rect: Rect, radius: f32, th: &Theme, ui: f32, level: Lift, fill: Rgb) {
    panel_edged(l, rect, radius, th, ui, level, fill, edge(th), ui.max(1.0));
}

/// [`panel`] with an edge of colour `rim`, `bw` px wide (a warning's red, say).
#[allow(clippy::too_many_arguments)]
pub fn panel_edged(l: &mut Layer, rect: Rect, radius: f32, th: &Theme, ui: f32, level: Lift, fill: Rgb, rim: Rgb, bw: f32) {
    lift(l, rect, radius, th, ui, level);
    let outer = if radius > 0.0 { radius + bw } else { 0.0 };
    l.quads.push(Quad { shape: Shape::Rect { rect: rect.inflate(bw), radius: outer }, color: rgb(rim, 1.0), texture: None });
    l.quads.push(Quad { shape: Shape::Rect { rect, radius }, color: rgb(fill, 1.0), texture: None });
    sheen(l, rect, radius, th, ui);
}

/// The 1 px highlight along the top of a surface (kept inside rounded corners).
pub fn sheen(l: &mut Layer, rect: Rect, radius: f32, th: &Theme, ui: f32) {
    let bw = ui.max(1.0);
    let colour = if th.light() { rgb([255, 255, 255], 0.7) } else { rgb(th.foreground(), 0.06) };
    l.quads.push(Quad {
        shape: Shape::Rect { rect: Rect::new(rect.x + radius, rect.y, (rect.w - 2.0 * radius).max(0.0), bw), radius: 0.0 },
        color: colour,
        texture: None,
    });
}

/// A dim veil over the whole window, behind a modal surface.
pub fn scrim(screen: Rect, th: &Theme, strength: f32) -> Layer {
    let a = if th.light() { 0.16 } else { 0.34 } * strength;
    Layer { quads: vec![Quad { shape: Shape::Rect { rect: screen, radius: 0.0 }, color: [0.0, 0.0, 0.0, a], texture: None }], texts: vec![] }
}

/// The keys of a shortcut as written in the palette ("Ctrl+Shift+N", "Ctrl++",
/// "?") split into caps; `None` for text that isn't one shortcut (a host, "Space",
/// or a choice of keys such as "B · Esc").
pub fn keycaps(detail: &str) -> Option<Vec<&str>> {
    let d = detail.trim();
    if d.is_empty() || d.contains(' ') || d.chars().count() > 24 {
        return None;
    }
    if d == "+" {
        return Some(vec!["+"]);
    }
    let (body, plus) = match d.strip_suffix("++") {
        Some(rest) => (rest, true),
        None => (d, false),
    };
    let mut keys: Vec<&str> = body.split('+').collect();
    if keys.iter().any(|k| k.is_empty() || k.chars().count() > 6) {
        return None;
    }
    if plus {
        keys.push("+");
    }
    // Modifiers must come first; anything else reads as plain text.
    let mods = ["Ctrl", "Shift", "Alt", "Super"];
    if keys[..keys.len() - 1].iter().any(|k| !mods.contains(k)) {
        return None;
    }
    Some(keys)
}

/// Size of a row of keycaps for `keys` at text `size`, given each key's text width.
pub fn keycaps_width(widths: &[f32], size: f32) -> f32 {
    let (pad, gap) = (size * 0.45, size * 0.3);
    widths.iter().map(|w| (w + 2.0 * pad).max(size * 1.6)).sum::<f32>() + gap * widths.len().saturating_sub(1) as f32
}

/// Draws `keys` (with text widths `widths`) as keycaps ending at `right`,
/// centred on `mid_y`; returns the left edge.
#[allow(clippy::too_many_arguments)]
pub fn draw_keycaps(l: &mut Layer, keys: &[&str], widths: &[f32], right: f32, mid_y: f32, size: f32, th: &Theme, ui: f32, clip: Rect) -> f32 {
    let (pad, gap) = (size * 0.45, size * 0.3);
    let h = size * 1.7;
    let bw = ui.max(1.0);
    let radius = th.small_radius(ui);
    let face = if th.light() { mix(th.panel(), [255, 255, 255], 0.6) } else { mix(th.panel(), th.foreground(), 0.07) };
    let rim = edge(th);
    let base = if th.light() { mix(th.panel(), [0, 0, 0], 0.22) } else { mix(th.panel(), [0, 0, 0], 0.45) };
    let total = keycaps_width(widths, size);
    let mut x = right - total;
    let left = x;
    for (k, w) in keys.iter().zip(widths) {
        let cw = (w + 2.0 * pad).max(size * 1.6);
        let cap = Rect::new(x, mid_y - h / 2.0, cw, h);
        // A darker lip under the cap, then its rim and face.
        l.quads.push(Quad { shape: Shape::Rect { rect: cap.translate(Vec2::new(0.0, bw)).inflate(bw), radius: radius + bw }, color: rgb(base, 1.0), texture: None });
        l.quads.push(Quad { shape: Shape::Rect { rect: cap.inflate(bw), radius: if radius > 0.0 { radius + bw } else { 0.0 } }, color: rgb(rim, 1.0), texture: None });
        l.quads.push(Quad { shape: Shape::Rect { rect: cap, radius }, color: rgb(face, 1.0), texture: None });
        l.texts.push(Text {
            text: (*k).to_string(),
            pos: [x + (cw - w) / 2.0, mid_y - size * 1.3 / 2.0],
            size,
            color: rgb(mix(th.foreground(), th.dim_text(), 0.35), 1.0),
            clip,
            bold: false,
        });
        x += cw + gap;
    }
    left
}

/// How long overlays take to appear.
pub const APPEAR: Duration = Duration::from_millis(150);

/// Progress (0 → 1, eased) of something that appears when it is first shown,
/// or when what it shows changes (`key`).
#[derive(Debug, Default, Clone)]
pub struct Appear {
    since: Option<(Instant, u64)>,
}

impl Appear {
    /// Notes whether it is shown this frame (and as what), and returns how far
    /// it has come in, eased out; 1 once settled, 0 when hidden.
    pub fn update(&mut self, shown: Option<u64>, now: Instant) -> f32 {
        match (shown, self.since) {
            (None, _) => {
                self.since = None;
                0.0
            }
            (Some(key), Some((at, k))) if k == key => ease_out(now.saturating_duration_since(at).as_secs_f32() / APPEAR.as_secs_f32()),
            (Some(key), _) => {
                self.since = Some((now, key));
                0.0
            }
        }
    }
}

/// Cubic ease-out over 0..1, clamped.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Fades `l` to `alpha` and moves it by `shift` px.
pub fn fade(l: &mut Layer, alpha: f32, shift: Vec2) {
    if alpha >= 1.0 && shift.x == 0.0 && shift.y == 0.0 {
        return;
    }
    let d = shift;
    for q in &mut l.quads {
        q.color[3] *= alpha;
        q.shape = match q.shape {
            Shape::Rect { rect, radius } => Shape::Rect { rect: rect.translate(d), radius },
            Shape::RoundBottom { rect, radius } => Shape::RoundBottom { rect: rect.translate(d), radius },
            Shape::Shadow { rect, radius, blur } => Shape::Shadow { rect: rect.translate(d), radius, blur },
            Shape::Segment { from, to, width } => Shape::Segment { from: [from[0] + d.x, from[1] + d.y], to: [to[0] + d.x, to[1] + d.y], width },
        };
    }
    for t in &mut l.texts {
        t.color[3] *= alpha;
        t.pos[0] += d.x;
        t.pos[1] += d.y;
        t.clip = t.clip.translate(d);
    }
}

/// Fades `l` in as `came` goes 0 → 1, sliding the last `px` down into place.
pub fn arrive(l: &mut Layer, came: f32, px: f32) {
    fade(l, came, Vec2::new(0.0, -px * (1.0 - came)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycaps_split_shortcuts_only() {
        assert_eq!(keycaps("Ctrl+T"), Some(vec!["Ctrl", "T"]));
        assert_eq!(keycaps("Ctrl+Shift+P"), Some(vec!["Ctrl", "Shift", "P"]));
        assert_eq!(keycaps("N"), Some(vec!["N"]));
        assert_eq!(keycaps("?"), Some(vec!["?"]));
        assert_eq!(keycaps("Ctrl++"), Some(vec!["Ctrl", "+"]));
        assert_eq!(keycaps("Ctrl+,"), Some(vec!["Ctrl", ","]));
        assert_eq!(keycaps("+"), Some(vec!["+"]));
        // Not a single shortcut: hosts, words, choices.
        assert_eq!(keycaps("en.wikipedia.org"), None);
        assert_eq!(keycaps("Space"), Some(vec!["Space"]));
        assert_eq!(keycaps("B · Esc"), None);
        assert_eq!(keycaps("Shift+F+G"), None);
        assert_eq!(keycaps(""), None);
        assert_eq!(keycaps("Ctrl+"), None);
    }

    #[test]
    fn keycap_row_width_adds_caps_and_gaps() {
        // Narrow keys keep a square-ish minimum width.
        let one = keycaps_width(&[5.0], 10.0);
        assert_eq!(one, 16.0);
        let two = keycaps_width(&[5.0, 30.0], 10.0);
        assert_eq!(two, 16.0 + 39.0 + 3.0);
    }

    #[test]
    fn appear_eases_in_and_restarts_on_change() {
        let mut a = Appear::default();
        let t0 = Instant::now();
        assert_eq!(a.update(Some(1), t0), 0.0);
        let mid = a.update(Some(1), t0 + APPEAR / 2);
        assert!(mid > 0.5 && mid < 1.0, "{mid}");
        assert_eq!(a.update(Some(1), t0 + APPEAR * 2), 1.0);
        // Shown as something else: starts again.
        assert_eq!(a.update(Some(2), t0 + APPEAR * 3), 0.0);
        assert_eq!(a.update(None, t0 + APPEAR * 4), 0.0);
        assert_eq!(a.update(Some(2), t0 + APPEAR * 5), 0.0);
    }

    #[test]
    fn fade_moves_and_dims_everything() {
        let mut l = Layer::default();
        l.quads.push(Quad { shape: Shape::Rect { rect: Rect::new(0.0, 10.0, 5.0, 5.0), radius: 0.0 }, color: [1.0; 4], texture: None });
        l.texts.push(Text { text: "a".into(), pos: [0.0, 10.0], size: 10.0, color: [1.0; 4], clip: Rect::new(0.0, 0.0, 10.0, 10.0), bold: false });
        fade(&mut l, 0.5, Vec2::new(0.0, -4.0));
        assert_eq!(l.quads[0].color[3], 0.5);
        assert!(matches!(l.quads[0].shape, Shape::Rect { rect, .. } if rect.y == 6.0));
        assert_eq!(l.texts[0].pos[1], 6.0);
        assert_eq!(l.texts[0].clip.y, -4.0);
    }
}
