//! The look shared by everything drawn over the board.
//!
//! One model of depth: a surface is its level's colour (`theme`), a hairline
//! just inside its edge, and a two-part shadow whose size says how far it
//! floats. Nothing glows, shines or fades to a gradient. Sizes sit on a 4 px
//! grid and text on one scale ([`type_`]); corners follow the theme's
//! rounding (sharp on Omarchy's default).

use std::time::{Duration, Instant};

use compositor::{Layer, Quad, Renderer, Shape, Text};
use scene::{Rect, Vec2};

use crate::icons::{self, Icon};
use crate::theme::{Rgb, Theme, rgb};

/// The type scale, in logical px (multiply by the UI scale). Every text in
/// the chrome is one of these.
pub mod type_ {
    /// Keycaps, counts, section labels.
    pub const CAPTION: f32 = 11.0;
    /// Secondary lines: hosts, hints, tooltips.
    pub const SMALL: f32 = 12.0;
    /// Rows, menus, buttons, titles of cards.
    pub const BODY: f32 = 13.0;
    /// What you type into.
    pub const INPUT: f32 = 15.0;
    /// A sheet's heading.
    pub const TITLE: f32 = 18.0;
}

/// Line height the renderer gives a text of `size`.
pub fn line_h(size: f32) -> f32 {
    size * 1.3
}

/// The y to draw a text of `size` at so it is centred on `mid_y`.
pub fn text_y(mid_y: f32, size: f32) -> f32 {
    mid_y - line_h(size) / 2.0
}

/// How far a surface floats above the board: the size of its shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lift {
    /// Labels and chips that sit on the board.
    Low,
    /// The dock, menus, the sidebar, toasts.
    Mid,
    /// The palette, the shortcut sheet: modal surfaces.
    High,
}

/// A two-part shadow under `rect`, lit from above: a tight one where the
/// surface meets what is under it and a wide, soft one around it. Offset and
/// blur grow together with the lift, as real shadows do.
pub fn lift(l: &mut Layer, rect: Rect, radius: f32, th: &Theme, ui: f32, lift: Lift) {
    let (key, ambient) = match lift {
        // (offset y, blur, strength)
        Lift::Low => ((1.0, 2.0, 0.22), (3.0, 8.0, 0.16)),
        Lift::Mid => ((1.0, 3.0, 0.26), (8.0, 24.0, 0.28)),
        Lift::High => ((2.0, 6.0, 0.28), (24.0, 60.0, 0.42)),
    };
    for (dy, blur, a) in [ambient, key] {
        l.quads.push(Quad {
            shape: Shape::Shadow { rect: rect.translate(Vec2::new(0.0, dy * ui)), radius, blur: blur * ui },
            color: th.shadow(a),
            texture: None,
        });
    }
}

/// The hairline around a panel.
pub fn edge(th: &Theme) -> Rgb {
    th.line()
}

/// One device pixel at least: the width of every hairline.
pub fn hair(ui: f32) -> f32 {
    ui.round().max(1.0)
}

/// A filled rounded rectangle.
pub fn fill(l: &mut Layer, rect: Rect, radius: f32, colour: [f32; 4]) {
    l.quads.push(Quad { shape: Shape::Rect { rect, radius }, color: colour, texture: None });
}

/// An outline `width` px wide just inside `rect`.
pub fn outline(l: &mut Layer, rect: Rect, radius: f32, width: f32, colour: [f32; 4]) {
    l.quads.push(Quad { shape: Shape::Ring { rect, radius, width }, color: colour, texture: None });
}

/// A surface over the board: its shadow, `fill`, and a hairline inside its edge.
pub fn panel(l: &mut Layer, rect: Rect, radius: f32, th: &Theme, ui: f32, level: Lift, fill: Rgb) {
    panel_edged(l, rect, radius, th, ui, level, fill, edge(th), hair(ui));
}

/// [`panel`] with an edge of colour `rim`, `bw` px wide (a warning's red, say).
#[allow(clippy::too_many_arguments)]
pub fn panel_edged(l: &mut Layer, rect: Rect, radius: f32, th: &Theme, ui: f32, level: Lift, fill: Rgb, rim: Rgb, bw: f32) {
    lift(l, rect, radius, th, ui, level);
    self::fill(l, rect, radius, rgb(fill, 1.0));
    outline(l, rect, radius, bw, rgb(rim, 1.0));
}

/// A dim veil over the whole window, behind a modal surface.
pub fn scrim(screen: Rect, th: &Theme, strength: f32) -> Layer {
    let a = if th.light() { 0.18 } else { 0.42 } * strength;
    let mut c = th.shadow(1.0);
    c[3] = a;
    Layer { quads: vec![Quad { shape: Shape::Rect { rect: screen, radius: 0.0 }, color: c, texture: None }], texts: vec![] }
}

/// One line of text centred on `mid_y`, starting at `x`.
#[allow(clippy::too_many_arguments)]
pub fn text(l: &mut Layer, s: impl Into<String>, x: f32, mid_y: f32, size: f32, colour: [f32; 4], clip: Rect, bold: bool) {
    l.texts.push(Text { text: s.into(), pos: [x, text_y(mid_y, size)], size, color: colour, clip, bold });
}

/// How a button looks for what the pointer is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Idle,
    Hover,
    /// Held down, or switched on (an open sidebar's button).
    Down,
}

/// A square icon button: nothing behind it at rest, a fill under the
/// pointer, a stronger one while pressed or on. `on` tints the icon with the
/// accent (a mode that is switched on); `enabled: false` greys it out.
#[allow(clippy::too_many_arguments)]
pub fn icon_button(l: &mut Layer, r: &mut Renderer, rect: Rect, icon: Icon, press: Press, on: bool, enabled: bool, th: &Theme, ui: f32) {
    let radius = th.small_radius(ui / th.text_scale);
    match press {
        Press::Hover if enabled => fill(l, rect, radius, rgb(th.hover(), 1.0)),
        Press::Down if enabled => fill(l, rect, radius, rgb(th.active(), 1.0)),
        _ => {}
    }
    let ink = match (enabled, on, press) {
        (false, ..) => th.text3(),
        (true, true, _) => th.accent_text(),
        (true, false, Press::Idle) => th.text2(),
        (true, false, _) => th.text(),
    };
    let c = rect.center();
    icons::draw(l, r, icon, c.x, c.y, 16.0 * ui, rgb(ink, 1.0));
}

/// A small label that names a control and its key, floating `gap` px from
/// `anchor`: above it when `above`, else below; kept inside the window.
#[allow(clippy::too_many_arguments)]
pub fn tooltip(r: &mut Renderer, label: &str, keys: &str, anchor: Rect, above: bool, viewport: Vec2, th: &Theme, ui: f32) -> Layer {
    let mut l = Layer::default();
    let size = type_::SMALL * ui;
    let ksize = type_::CAPTION * ui;
    let (pad, gap, h) = (8.0 * ui, 8.0 * ui, 28.0 * ui);
    let lw = r.measure(label, size, false);
    let caps = keycaps(keys);
    let widths: Vec<f32> = caps.iter().flatten().map(|k| r.measure(k, ksize, false)).collect();
    let kw = if widths.is_empty() { 0.0 } else { keycaps_width(&widths, ksize) + 8.0 * ui };
    let w = pad + lw + kw + if widths.is_empty() { pad } else { 5.0 * ui };
    let x = (anchor.center().x - w / 2.0).clamp(6.0 * ui, (viewport.x - w - 6.0 * ui).max(6.0 * ui));
    let y = if above { anchor.y - gap - h } else { anchor.bottom() + gap };
    let rect = Rect::new(x.round(), y.round(), w.round(), h);
    // A step above a panel, with a firmer edge: it floats over anything.
    let (face, ink) = (th.active(), th.text());
    let radius = th.small_radius(ui / th.text_scale);
    lift(&mut l, rect, radius, th, ui, Lift::Low);
    fill(&mut l, rect, radius, rgb(face, 1.0));
    outline(&mut l, rect, radius, hair(ui), rgb(th.line_strong(), 0.7));
    text(&mut l, label, rect.x + pad, rect.center().y, size, rgb(ink, 1.0), rect, false);
    if let Some(keys) = caps {
        let mut kx = rect.right() - 5.0 * ui - keycaps_width(&widths, ksize);
        for (k, kwid) in keys.iter().zip(&widths) {
            let cw = cap_w(*kwid, ksize);
            let cap = Rect::new(kx.round(), (rect.center().y - ksize * 0.85).round(), cw.round(), (ksize * 1.7).round());
            fill(&mut l, cap, radius.min(4.0 * ui), rgb(th.panel(), 1.0));
            text(&mut l, *k, cap.x + (cap.w - kwid) / 2.0, cap.center().y, ksize, rgb(th.text2(), 1.0), cap, true);
            kx += cw + ksize * 0.3;
        }
    }
    l
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

/// Width of one keycap around a key `w` px wide at text `size`.
fn cap_w(w: f32, size: f32) -> f32 {
    (w + 2.0 * size * 0.5).max(size * 1.7)
}

/// Size of a row of keycaps for `keys` at text `size`, given each key's text width.
pub fn keycaps_width(widths: &[f32], size: f32) -> f32 {
    widths.iter().map(|w| cap_w(*w, size)).sum::<f32>() + size * 0.3 * widths.len().saturating_sub(1) as f32
}

/// Draws `keys` (with text widths `widths`) as keycaps ending at `right`,
/// centred on `mid_y`; returns the left edge. A keycap is flat: a raised
/// fill with a hairline, the key in secondary text.
#[allow(clippy::too_many_arguments)]
pub fn draw_keycaps(l: &mut Layer, keys: &[&str], widths: &[f32], right: f32, mid_y: f32, size: f32, th: &Theme, ui: f32, clip: Rect) -> f32 {
    let gap = size * 0.3;
    let h = size * 1.7;
    let radius = th.small_radius(ui / th.text_scale).min(4.0 * ui);
    let total = keycaps_width(widths, size);
    let mut x = right - total;
    let left = x;
    for (k, w) in keys.iter().zip(widths) {
        let cw = cap_w(*w, size);
        let cap = Rect::new(x.round(), (mid_y - h / 2.0).round(), cw.round(), h.round());
        fill(l, cap, radius, rgb(th.hover(), 1.0));
        outline(l, cap, radius, hair(ui), rgb(th.line_strong(), 0.55));
        l.texts.push(Text {
            text: (*k).to_string(),
            pos: [cap.x + (cap.w - w) / 2.0, text_y(cap.center().y, size)],
            size,
            color: rgb(th.text2(), 1.0),
            clip,
            bold: true,
        });
        x += cw + gap;
    }
    left
}

/// How long overlays take to appear.
pub const APPEAR: Duration = Duration::from_millis(160);

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

/// How long a card's label takes to fade in or out.
pub const LABEL_FADE: Duration = Duration::from_millis(120);

/// Per-item fades (cards' labels): each moves toward shown or hidden at a
/// steady speed (all the way in [`LABEL_FADE`]), turning back from wherever
/// it is when the wish changes.
#[derive(Debug, Default)]
pub struct Fades {
    /// Item → (wanted shown, alpha when that was decided, when).
    items: std::collections::HashMap<u64, (bool, f32, Instant)>,
}

impl Fades {
    /// The alpha of `id` now, given whether it should be shown. An item
    /// seen for the first time starts where it should be, without fading.
    pub fn alpha(&mut self, id: u64, shown: bool, now: Instant) -> f32 {
        let at = |(want, from, t): (bool, f32, Instant)| {
            let p = now.saturating_duration_since(t).as_secs_f32() / LABEL_FADE.as_secs_f32();
            if want { (from + p).min(1.0) } else { (from - p).max(0.0) }
        };
        match self.items.get(&id).copied() {
            None => {
                self.items.insert(id, (shown, if shown { 1.0 } else { 0.0 }, now));
                if shown { 1.0 } else { 0.0 }
            }
            Some(e) if e.0 == shown => at(e),
            Some(e) => {
                let a = at(e);
                self.items.insert(id, (shown, a, now));
                a
            }
        }
    }

    /// Forgets items no longer drawn.
    pub fn retain(&mut self, keep: impl Fn(u64) -> bool) {
        self.items.retain(|&id, _| keep(id));
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
            Shape::Page { rect, radius, round_bottom, extent } => Shape::Page { rect: rect.translate(d), radius, round_bottom, extent },
            Shape::Shadow { rect, radius, blur } => Shape::Shadow { rect: rect.translate(d), radius, blur },
            Shape::Ring { rect, radius, width } => Shape::Ring { rect: rect.translate(d), radius, width },
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
    fn fades_turn_back_midway() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut f = Fades::default();
        assert_eq!(f.alpha(1, false, t0), 0.0, "first seen: no fade");
        assert_eq!(f.alpha(2, true, t0), 1.0);
        assert_eq!(f.alpha(1, true, t0), 0.0);
        assert!((f.alpha(1, true, ms(60)) - 0.5).abs() < 0.01);
        // Hidden again halfway: back down from 0.5, at the same speed.
        assert!((f.alpha(1, false, ms(60)) - 0.5).abs() < 0.01);
        assert!((f.alpha(1, false, ms(90)) - 0.25).abs() < 0.01);
        assert!((f.alpha(1, false, ms(120)) - 0.0).abs() < 0.01);
        assert_eq!(f.alpha(1, false, ms(500)), 0.0);
        f.retain(|id| id == 2);
        assert_eq!(f.alpha(1, true, ms(500)), 1.0, "forgotten: starts shown");
    }

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
        assert_eq!(one, 17.0);
        let two = keycaps_width(&[5.0, 30.0], 10.0);
        assert_eq!(two, 17.0 + 40.0 + 3.0);
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
