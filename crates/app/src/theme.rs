//! The Omarchy look: the current theme's palette (light or dark), Hyprland's
//! window rounding and the desktop's text size, turned into the colours and
//! sizes Trailboard draws with.
//!
//! The palette comes from `~/.local/state/omarchy/current/theme/colors.toml`.
//! Without Omarchy it falls back to a dark palette with Omarchy's yellow
//! accent, as Omarchy's own apps do. `desktop` watches the files and Hyprland
//! and hands new values to `Theme`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use compositor::Rgba;

pub type Rgb = [u8; 3];

/// Without Omarchy: a dark surface and Omarchy's fallback yellow.
/// Where an entered card's keys go, as its ring shows it (`Theme::keys_ring`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keys {
    Page,
    Insert,
    Passthrough,
    /// Caret or Visual.
    Caret,
}

/// How far apart (Oklab distance) two mode rings must be to tell apart.
const RING_APART: f32 = 0.12;

const FALLBACK: &[(&str, Rgb)] = &[
    ("accent", [0xFF, 0xD6, 0x0A]),
    ("background", [0x0e, 0x0e, 0x10]),
    ("foreground", [0xe6, 0xe6, 0xe9]),
    ("selection", [0x3a, 0x3a, 0x40]),
    ("muted", [0x5c, 0x5c, 0x66]),
    ("red", [0xf7, 0x76, 0x8e]),
    ("green", [0x9e, 0xce, 0x6a]),
    ("yellow", [0xe0, 0xaf, 0x68]),
    ("orange", [0xff, 0x9e, 0x64]),
    ("blue", [0x7a, 0xa2, 0xf7]),
    ("magenta", [0xbb, 0x9a, 0xf7]),
    ("cyan", [0x7d, 0xcf, 0xff]),
];

/// Older and hand-made themes name some colours differently; each key takes
/// the first of its other names the theme has.
const ALIASES: &[&[&str]] = &[
    &["background", "bg", "color0"],
    &["foreground", "fg", "color7"],
    &["red", "color1"],
    &["green", "color2"],
    &["yellow", "color3"],
    &["blue", "color4"],
    &["magenta", "color5", "purple"],
    &["cyan", "color6"],
    &["muted", "color8", "dark_foreground"],
    &["selection", "selection_background", "color8", "background"],
    &["orange", "yellow"],
];

/// The colours a theme file defines, and whether the theme is light.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Palette {
    pub colors: HashMap<String, Rgb>,
    pub light: bool,
}

fn parse_hex(s: &str) -> Option<Rgb> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Every `key = "#rrggbb"` in a colors.toml (quoted or bare, with or without a
/// trailing comment), aliases filled in, and whether the theme is light: its
/// `mode`, a `light.mode` file beside it (`light_mode_file`), or failing both
/// a light background.
pub fn parse_palette(text: &str, light_mode_file: bool) -> Palette {
    let mut colors = HashMap::new();
    let mut modes = HashMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once('=') else { continue };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            continue;
        }
        let value = value.trim();
        // A trailing comment starts with `#` after the value; a bare colour starts with `#` too.
        let value = match value.chars().next() {
            Some(q @ ('"' | '\'')) => match value[1..].split_once(q) {
                Some((inner, rest)) if rest.trim().is_empty() || rest.trim().starts_with('#') => inner,
                _ => continue,
            },
            _ => {
                let mut parts = value.split_whitespace();
                let v = parts.next().unwrap_or("");
                match parts.next() {
                    None => v,
                    Some(c) if c.starts_with('#') => v,
                    _ => continue,
                }
            }
        };
        if let Some(rgb) = parse_hex(value) {
            colors.insert(key.to_string(), rgb);
        } else if value == "light" || value == "dark" {
            modes.insert(key.to_string(), value.to_string());
        }
    }
    for names in ALIASES {
        if colors.contains_key(names[0]) {
            continue;
        }
        if let Some(c) = names[1..].iter().find_map(|n| colors.get(*n).copied()) {
            colors.insert(names[0].to_string(), c);
        }
    }
    let mut mode = modes.get("mode").or_else(|| modes.get("theme_type")).cloned().unwrap_or_default();
    if mode.is_empty() && light_mode_file {
        mode = "light".into();
    }
    if mode.is_empty()
        && let Some([r, g, b]) = colors.get("background")
    {
        mode = if *r as u32 + *g as u32 + *b as u32 > 382 { "light" } else { "dark" }.into();
    }
    Palette { light: mode == "light" && !colors.is_empty(), colors }
}

/// Reads a theme's colors.toml; an unreadable file is an empty palette (the fallback).
pub fn read_palette(theme_dir: &Path) -> Palette {
    match std::fs::read_to_string(theme_dir.join("colors.toml")) {
        Ok(text) => parse_palette(&text, theme_dir.join("light.mode").exists()),
        Err(_) => Palette::default(),
    }
}

/// `[font] base-size` from a shell.toml, if it sets one.
pub fn parse_base_size(text: &str) -> Option<f32> {
    let mut in_font = false;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_font = line == "[font]";
        } else if in_font
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == "base-size"
        {
            return v.trim().trim_matches('"').parse::<f32>().ok().filter(|s| (6.0..=48.0).contains(s));
        }
    }
    None
}

/// Hyprland's `decoration:rounding` from `hyprctl -j getoption` output.
pub fn parse_rounding(json: &str) -> Option<f32> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("int")?.as_i64().filter(|r| (0..=200).contains(r)).map(|r| r as f32)
}

/// Where Omarchy keeps the current theme (`TRAILBOARD_OMARCHY_STATE` overrides it for tests).
pub fn state_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("TRAILBOARD_OMARCHY_STATE") {
        return PathBuf::from(p);
    }
    home().join(".local/state/omarchy/current")
}

/// The user's shell.toml, which `omarchy display text size` writes.
pub fn user_shell_toml() -> PathBuf {
    if let Some(p) = std::env::var_os("TRAILBOARD_OMARCHY_SHELL") {
        return PathBuf::from(p);
    }
    home().join(".config/omarchy/shell.toml")
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Text size: the user's shell.toml, then the theme's, then Omarchy's 12 px.
pub fn read_base_size(user_shell: &Path, state: &Path) -> f32 {
    [user_shell.to_path_buf(), state.join("theme/shell.toml")]
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .find_map(|t| parse_base_size(&t))
        .unwrap_or(12.0)
}

/// Mixes `a` toward `b` by `t` (0 = a, 1 = b).
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Black or white, whichever reads on `c`.
pub fn text_on(c: Rgb) -> Rgb {
    if luminance(c) > 0.179 { [0, 0, 0] } else { [255, 255, 255] }
}

pub fn rgb(c: Rgb, alpha: f32) -> Rgba {
    compositor::rgba(c[0], c[1], c[2], alpha)
}

fn linear(v: u8) -> f32 {
    let v = v as f32 / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn luminance(c: Rgb) -> f32 {
    0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
}

/// WCAG contrast ratio between two colours (1 to 21).
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// A colour in OKLab: lightness (0 black to 1 white) and two colour axes.
/// Steps of lightness here look equally big to the eye whatever the hue,
/// which a mix in sRGB does not give.
fn to_oklab(c: Rgb) -> [f32; 3] {
    let (r, g, b) = (linear(c[0]), linear(c[1]), linear(c[2]));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn from_oklab(c: [f32; 3]) -> Rgb {
    let l = (c[0] + 0.396_337_78 * c[1] + 0.215_803_76 * c[2]).powi(3);
    let m = (c[0] - 0.105_561_346 * c[1] - 0.063_854_17 * c[2]).powi(3);
    let s = (c[0] - 0.089_484_18 * c[1] - 1.291_485_5 * c[2]).powi(3);
    let srgb = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let v = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    [
        srgb(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        srgb(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        srgb(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
    ]
}

/// The least OKLab lightness of a sticky note's paper on a light theme.
const STICKY_LIGHT: f32 = 0.88;

/// `c` made lighter (`d` > 0) or darker by `d` of OKLab lightness, keeping its hue.
pub fn shade(c: Rgb, d: f32) -> Rgb {
    let [l, a, b] = to_oklab(c);
    let l2 = (l + d).clamp(0.0, 1.0);
    // Near black and white there is no room for colour: fade it out with the lightness.
    let keep = if l > 1e-3 { (l2.min(1.0 - l2) / l.min(1.0 - l).max(1e-3)).min(1.0) } else { 0.0 };
    from_oklab([l2, a * keep, b * keep])
}

/// `a` mixed toward `b` by `t` in OKLab (even steps of lightness).
pub fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let (x, y) = (to_oklab(a), to_oklab(b));
    from_oklab([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t])
}

/// `ink` moved as far toward `on` as it can go while still reading on `on`
/// at contrast `target`: how secondary text is made, so "dim" is a measured
/// amount and not a guess that fails on some themes. An `ink` that reads
/// worse than `target` already is pushed away from `on` instead (toward
/// black or white) until it does.
pub fn quiet(ink: Rgb, on: Rgb, target: f32) -> Rgb {
    if contrast(ink, on) < target {
        return loud(ink, on, target);
    }
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if contrast(blend(ink, on, mid), on) >= target { lo = mid } else { hi = mid }
    }
    blend(ink, on, lo)
}

/// `ink` moved toward black or white (whichever `on` is further from) just
/// until it reads on `on` at contrast `target`; unchanged if it already does.
pub fn loud(ink: Rgb, on: Rgb, target: f32) -> Rgb {
    if contrast(ink, on) >= target {
        return ink;
    }
    let end = text_on(on);
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if contrast(blend(ink, end, mid), on) >= target { hi = mid } else { lo = mid }
    }
    blend(ink, end, hi)
}

/// Everything Trailboard draws with, derived from the Omarchy theme.
///
/// The theme gives a background, a foreground, an accent and named colours.
/// From those come four levels, each a measured step of lightness from the
/// one below (lighter on dark themes; on light ones the steps go darker
/// where there is no room left above):
///
/// | level | what | token |
/// |---|---|---|
/// | 0 | the board | [`Theme::board`] |
/// | 1 | cards | [`Theme::card`] |
/// | 2 | floating chrome: dock, sidebar, menus, lists | [`Theme::panel`] |
/// | 3 | what is under the pointer or chosen on a panel | [`Theme::hover`], [`Theme::active`] |
///
/// Text comes in three strengths by contrast on a panel: [`Theme::text`]
/// (at least 7:1), [`Theme::text2`] (4.5:1, secondary) and [`Theme::text3`]
/// (3:1, placeholders and decoration only). The accent is kept for the
/// current card, the chosen row and the one main action.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    /// The corner radius in use, logical px: from the *Corners* setting
    /// (`scene::settings::Settings::corner_radius`), set by the app.
    pub rounding: f32,
    /// Hyprland's window rounding in logical px (0, sharp, without Hyprland:
    /// Omarchy's default), for *Corners: Follow Hyprland*.
    pub hyprland: f32,
    /// The desktop's text size over Omarchy's default 12 px.
    pub text_scale: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme { palette: Palette::default(), rounding: 0.0, hyprland: 0.0, text_scale: 1.0 }
    }
}

impl Theme {
    /// A theme colour by name (`accent`, `red`, …), or the fallback palette's.
    pub fn color(&self, key: &str) -> Rgb {
        self.palette.colors.get(key).copied().unwrap_or_else(|| {
            let fb = |k: &str| FALLBACK.iter().find(|(n, _)| *n == k).map(|(_, c)| *c);
            fb(key).or_else(|| fb("foreground")).expect("fallback has foreground")
        })
    }

    pub fn light(&self) -> bool {
        self.palette.light
    }

    pub fn accent(&self) -> Rgb {
        self.color("accent")
    }

    pub fn background(&self) -> Rgb {
        self.color("background")
    }

    pub fn foreground(&self) -> Rgb {
        self.color("foreground")
    }

    /// Black or white, whichever reads on the accent (WCAG relative luminance).
    pub fn accent_foreground(&self) -> Rgb {
        text_on(self.accent())
    }

    /// The accent as text or an icon on a panel: the accent itself where it
    /// reads there, else moved toward the text colour until it does.
    pub fn accent_text(&self) -> Rgb {
        loud(self.accent(), self.panel(), 4.5)
    }

    /// A panel tinted with the accent: the chosen row, the pressed tool.
    pub fn accent_soft(&self) -> Rgb {
        // As much accent as leaves the text on it readable (4.5:1).
        let mut t: f32 = if self.light() { 0.16 } else { 0.20 };
        loop {
            let c = blend(self.panel(), self.accent(), t);
            if t <= 0.04 || contrast(self.text(), c) >= 4.5 {
                return c;
            }
            t -= 0.04;
        }
    }

    /// A trail's colour (a theme colour name). A trail without one is drawn
    /// in a quiet neutral: colour on the board means something was marked.
    pub fn trail(&self, colour: Option<&str>) -> Rgb {
        colour.map_or_else(|| self.line_strong(), |c| self.color(c))
    }

    /// A sticky note's paper: theme colour `colour` itself, a little softened
    /// toward the background so it doesn't glare, like paper on the board.
    pub fn sticky_paper(&self, colour: &str) -> Rgb {
        let paper = mix(self.color(colour), self.background(), 0.08);
        if !self.light() {
            return paper;
        }
        // A light theme's named colours are dark enough to be text (a
        // brown "yellow"); paper on a light board is the same hue, light.
        let [l, a, b] = to_oklab(paper);
        if l >= STICKY_LIGHT {
            return paper;
        }
        // Bright hues don't fit in sRGB that light: less colour until they do.
        let mut k = 0.8;
        loop {
            let c = from_oklab([STICKY_LIGHT, a * k, b * k]);
            if k < 0.05 || (to_oklab(c)[0] - STICKY_LIGHT).abs() < 0.008 {
                return c;
            }
            k *= 0.85;
        }
    }

    /// Text on a sticky note: black or white for its paper, softened.
    pub fn sticky_ink(&self, colour: &str) -> Rgb {
        let paper = self.sticky_paper(colour);
        mix(text_on(paper), paper, 0.12)
    }

    /// A sticky note's grab strip: its paper, a shade deeper (more when focused).
    pub fn sticky_strip(&self, colour: &str, focused: bool) -> Rgb {
        let paper = self.sticky_paper(colour);
        mix(paper, text_on(paper), if focused { 0.22 } else { 0.12 })
    }

    /// `c` raised `d` of lightness toward the viewer: lighter on a dark
    /// theme. On a light one surfaces go lighter too while there is room,
    /// and darker once they would pass white.
    fn raise(&self, c: Rgb, d: f32) -> Rgb {
        if !self.light() {
            return shade(c, d);
        }
        let l = to_oklab(c)[0];
        let room = (l + d * 0.6).min(1.0) - l;
        if room >= 0.02 { shade(c, room) } else { shade(c, -d * 0.55) }
    }

    /// `c` a step away from its own level, for what sits *on* a surface
    /// (lines, hover fills): lighter on dark themes, darker on light ones.
    fn step(&self, c: Rgb, d: f32) -> Rgb {
        shade(c, if self.light() { -d } else { d })
    }

    /// Level 2: panels over the board (dock, sidebar, menus, lists, labels).
    pub fn panel(&self) -> Rgb {
        self.raise(self.background(), 0.065)
    }

    /// Level 1: a card's frame and title bar; `focused` stands out a little more.
    pub fn card(&self, focused: bool) -> Rgb {
        self.raise(self.background(), if focused { 0.065 } else { 0.04 })
    }

    /// A recess in a panel: text fields, and a card with nothing to show yet.
    pub fn well(&self) -> Rgb {
        shade(self.panel(), if self.light() { -0.03 } else { -0.035 })
    }

    /// Content of a card with nothing to show yet.
    pub fn placeholder(&self) -> Rgb {
        shade(self.background(), -0.02)
    }

    /// Level 3: the row or button under the pointer.
    pub fn hover(&self) -> Rgb {
        self.step(self.panel(), 0.045)
    }

    /// Level 3, stronger: the chosen row, a pressed button, a keycap.
    pub fn active(&self) -> Rgb {
        self.step(self.panel(), 0.08)
    }

    /// The hairline around panels and cards, and between their parts.
    pub fn line(&self) -> Rgb {
        self.step(self.panel(), 0.075)
    }

    /// A line meant to be seen: outlines of stacked cards, unmarked trails, scroll thumbs.
    pub fn line_strong(&self) -> Rgb {
        self.step(self.panel(), 0.2)
    }

    /// Board dots: just enough to give the board a surface.
    pub fn dots(&self) -> Rgb {
        self.step(self.background(), if self.light() { 0.10 } else { 0.11 })
    }

    pub fn divider(&self) -> Rgb {
        self.line()
    }

    /// Primary text: the theme's foreground, strengthened if a theme's
    /// foreground reads poorly on panels (7:1 at least).
    pub fn text(&self) -> Rgb {
        loud(self.foreground(), self.panel(), 7.0)
    }

    /// Secondary text (hosts, counts, descriptions): 4.5:1 on a panel.
    pub fn text2(&self) -> Rgb {
        quiet(self.text(), self.panel(), 4.6)
    }

    /// Placeholders, disabled items and decoration: 3:1 on a panel.
    pub fn text3(&self) -> Rgb {
        quiet(self.text(), self.panel(), 3.0)
    }

    /// Ring and band of multi-selected cards (kept apart from the accent focus ring).
    pub fn marked(&self) -> Rgb {
        self.color("yellow")
    }

    pub fn danger(&self) -> Rgb {
        self.color("red")
    }

    /// The entered card's ring for where its keys go, so the mode shows
    /// without anything over the page: Page the accent, Insert green,
    /// Passthrough red, Caret / Visual magenta. A colour too near one
    /// already taken (an accent that is green, say) gives way to the next
    /// of its kind, so the four always tell apart.
    pub fn keys_ring(&self, keys: Keys) -> Rgb {
        let page = self.accent();
        let insert = self.unlike(&["green", "cyan", "blue"], &[page]);
        let pass = self.unlike(&["red", "orange", "yellow"], &[page, insert]);
        match keys {
            Keys::Page => page,
            Keys::Insert => insert,
            Keys::Passthrough => pass,
            Keys::Caret => self.unlike(&["magenta", "blue", "cyan", "orange"], &[page, insert, pass]),
        }
    }

    /// The colour of a card's awake mark: *Always active* the theme's green;
    /// *Keep awake* its cyan, or blue or magenta where the cyan is too near
    /// the green (as in themes with a mint cyan). *Auto* has none (it is
    /// drawn dim, like the other buttons).
    pub fn awake_keep(&self) -> Rgb {
        self.unlike(&["cyan", "blue", "magenta"], &[self.awake_active()])
    }

    pub fn awake_active(&self) -> Rgb {
        self.color("green")
    }

    /// The first of the colours `names` not too near any of `taken`, else the first.
    fn unlike(&self, names: &[&str], taken: &[Rgb]) -> Rgb {
        let near = |a: Rgb, b: Rgb| {
            let (a, b) = (to_oklab(a), to_oklab(b));
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() < RING_APART
        };
        names.iter().map(|n| self.color(n)).find(|&c| !taken.iter().any(|&t| near(c, t))).unwrap_or_else(|| self.color(names[0]))
    }

    /// A shadow of `strength`: the board's own hue, much darker, never a
    /// grey film. Softer on light themes, where a black shadow looks dirty.
    pub fn shadow(&self, strength: f32) -> Rgba {
        let [l, a, b] = to_oklab(self.background());
        let c = from_oklab([l * if self.light() { 0.35 } else { 0.18 }, a, b]);
        rgb(c, strength * if self.light() { 0.42 } else { 1.0 })
    }

    /// Corner radius of a floating panel (sidebar, menus, palette, toasts),
    /// in screen px: a little rounder than a card, so sheets read softer
    /// than pages; sharp when the corners are.
    pub fn radius(&self, scale: f32) -> f32 {
        if self.rounding > 0.0 { (self.rounding + 2.0) * scale } else { 0.0 }
    }

    /// Corner radius of a card (a page on the board) at zoom 1, in screen px.
    pub fn card_radius(&self, scale: f32) -> f32 {
        self.rounding * scale
    }

    /// Corner radius of a card `w`×`h` screen px on screen at `k` screen px
    /// per board unit: the card's radius scaled with the zoom, but kept
    /// visible when zoomed out (at least 4 px or the full radius, whichever
    /// is less) and never more than a sixth of the card's short side.
    pub fn card_corner(&self, k: f32, scale: f32, w: f32, h: f32) -> f32 {
        let full = self.card_radius(scale);
        (self.rounding * k).max(full.min(4.0 * scale)).min(full).min(w.min(h) / 6.0).max(0.0)
    }

    /// Corner radius of small elements (rows, buttons, keycaps): at most 6 px.
    pub fn small_radius(&self, scale: f32) -> f32 {
        self.rounding.min(6.0) * scale
    }

    /// The board colour of a space with this tint (a theme colour name, or none).
    pub fn board(&self, tint: Option<&str>) -> Rgb {
        match tint {
            Some(name) => mix(self.background(), self.color(name), if self.light() { 0.10 } else { 0.07 }),
            None => self.background(),
        }
    }

    /// A space's colour mood (*Board background: Colour mood*): the board
    /// at its top left and at its bottom right. The tint comes in stronger
    /// at the top left, a neighbouring hue deepens the far corner, and the
    /// two sit a little either side of the plain board's lightness, so the
    /// board has depth without losing the theme's mood. With no tint, the
    /// theme's background lifted and lowered.
    pub fn mood(&self, tint: Option<&str>) -> (Rgb, Rgb) {
        let bg = self.background();
        let light = self.light();
        let (lift, sink) = if light { (0.012, -0.035) } else { (0.03, -0.022) };
        let Some(name) = tint else {
            return (shade(bg, lift), shade(bg, sink));
        };
        // The far corner leans to a neighbouring hue: the mood has two colours.
        let other = match name {
            "blue" => "magenta",
            "magenta" => "blue",
            "green" => "cyan",
            "cyan" => "blue",
            "orange" => "red",
            "red" => "magenta",
            "yellow" => "orange",
            _ => name,
        };
        // Hue from the tint, lightness from the board: every tint sits at
        // the same lightness, so none reads brighter than another.
        let tinted = |c: Rgb, t: f32, dl: f32| {
            let (b, c) = (to_oklab(bg), to_oklab(c));
            from_oklab([(b[0] + dl).clamp(0.0, 1.0), b[1] + (c[1] - b[1]) * t, b[2] + (c[2] - b[2]) * t])
        };
        (tinted(self.color(name), 0.30, lift), tinted(self.color(other), 0.20, sink))
    }

    /// The colour dot that stands for a space's tint.
    pub fn swatch(&self, tint: Option<&str>) -> Rgb {
        match tint {
            Some(name) => self.color(name),
            None => self.text3(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKYO: &str = r##"mode = "dark"

accent = "#7aa2f7"
selection = "#292e42"
background = "#1a1b26"
foreground = "#a9b1d6"
red = "#f7768e"
"##;

    #[test]
    fn reads_quoted_bare_and_commented_colours() {
        let p = parse_palette(
            "accent = \"#112233\"\nbackground = #ffffff # paper\nforeground='#000000' # ink\nbogus = \"#12345\"\nname = \"x\"\n",
            false,
        );
        assert_eq!(p.colors["accent"], [0x11, 0x22, 0x33]);
        assert_eq!(p.colors["background"], [255, 255, 255]);
        assert_eq!(p.colors["foreground"], [0, 0, 0]);
        assert!(!p.colors.contains_key("bogus"));
        assert!(p.light, "light background decides when nothing else does");
    }

    #[test]
    fn mode_wins_over_background_and_light_mode_file_counts() {
        let p = parse_palette("mode = \"dark\"\nbackground = \"#ffffff\"\n", false);
        assert!(!p.light);
        let p = parse_palette("background = \"#101010\"\n", true);
        assert!(p.light);
        let p = parse_palette(TOKYO, false);
        assert!(!p.light);
    }

    #[test]
    fn aliases_fill_missing_keys() {
        let p = parse_palette("color0 = \"#010101\"\ncolor7 = \"#070707\"\npurple = \"#550055\"\ncolor8 = \"#080808\"\n", false);
        assert_eq!(p.colors["background"], [1, 1, 1]);
        assert_eq!(p.colors["foreground"], [7, 7, 7]);
        assert_eq!(p.colors["magenta"], [0x55, 0, 0x55]);
        assert_eq!(p.colors["selection"], [8, 8, 8]);
        // Orange falls back to yellow only if the theme has one.
        assert!(!p.colors.contains_key("orange"));
    }

    #[test]
    fn empty_palette_falls_back_to_omarchy_yellow_on_dark() {
        let t = Theme::default();
        assert!(!t.light());
        assert_eq!(t.accent(), [0xFF, 0xD6, 0x0A]);
        assert_eq!(t.accent_foreground(), [0, 0, 0]);
        assert_eq!(t.color("no-such-colour"), t.foreground());
    }

    #[test]
    fn theme_colours_come_from_the_palette() {
        let t = Theme { palette: parse_palette(TOKYO, false), ..Theme::default() };
        assert_eq!(t.accent(), [0x7a, 0xa2, 0xf7]);
        assert_eq!(t.board(None), [0x1a, 0x1b, 0x26]);
        // A tint moves the board a little toward the colour, never all the way.
        let tinted = t.board(Some("red"));
        assert_ne!(tinted, t.board(None));
        assert!(tinted[0] > 0x1a && tinted[0] < 0x60);
        assert_eq!(t.swatch(Some("red")), [0xf7, 0x76, 0x8e]);
        // Surfaces sit between background and foreground.
        let panel = t.panel();
        assert!(panel[2] > 0x26 && panel[2] < 0xd6);
    }

    #[test]
    fn accent_foreground_reads_on_the_accent() {
        let dark_accent = Theme { palette: parse_palette("accent = \"#1e3a8a\"\n", false), ..Theme::default() };
        assert_eq!(dark_accent.accent_foreground(), [255, 255, 255]);
        let light_accent = Theme { palette: parse_palette("accent = \"#ffe680\"\n", false), ..Theme::default() };
        assert_eq!(light_accent.accent_foreground(), [0, 0, 0]);
    }

    #[test]
    fn moods_keep_labels_readable() {
        let light = parse_palette("background = \"#fafafa\"\nforeground = \"#222222\"\naccent = \"#3366cc\"\n", false);
        let themes = [Theme { palette: parse_palette(TOKYO, false), ..Theme::default() }, Theme { palette: light, ..Theme::default() }];
        for th in &themes {
            for tint in [None, Some("blue"), Some("green"), Some("magenta"), Some("orange"), Some("cyan"), Some("red")] {
                let (near, far) = th.mood(tint);
                assert_ne!(near, far, "{tint:?}: a gradient, not flat");
                for c in [near, far] {
                    // Card labels are drawn on the board in text2.
                    assert!(contrast(th.text2(), c) >= 4.0, "{tint:?} {c:?}: {}", contrast(th.text2(), c));
                    // Still the theme's mood: near its background.
                    assert!((to_oklab(c)[0] - to_oklab(th.background())[0]).abs() < 0.08, "{tint:?} {c:?}");
                }
            }
        }
    }

    #[test]
    fn card_corners_stay_visible_when_zoomed_out() {
        let t = Theme { rounding: 10.0, ..Theme::default() };
        assert_eq!(t.card_corner(1.0, 1.0, 640.0, 420.0), 10.0);
        assert_eq!(t.card_corner(2.0, 1.0, 640.0, 420.0), 10.0, "no rounder than the setting");
        assert_eq!(t.card_corner(0.25, 1.0, 160.0, 105.0), 4.0, "kept visible");
        assert_eq!(t.card_corner(0.05, 1.0, 18.0, 12.0), 2.0, "a sixth of a tiny card");
        assert_eq!(t.card_corner(0.25, 2.0, 320.0, 210.0), 8.0, "HiDPI");
        let sharp = Theme { rounding: 0.0, ..Theme::default() };
        assert_eq!(sharp.card_corner(0.25, 1.0, 160.0, 105.0), 0.0);
        let small = Theme { rounding: 2.0, ..Theme::default() };
        assert_eq!(small.card_corner(0.25, 1.0, 160.0, 105.0), 2.0);
    }

    #[test]
    fn rounding_caps_small_elements() {
        let t = Theme { rounding: 10.0, ..Theme::default() };
        assert_eq!(t.radius(2.0), 24.0);
        assert_eq!(t.card_radius(2.0), 20.0);
        assert_eq!(t.small_radius(2.0), 12.0);
        assert_eq!(Theme::default().radius(2.0), 0.0);
    }

    #[test]
    fn base_size_only_from_the_font_section() {
        assert_eq!(parse_base_size("[bar]\nbase-size = 20\n[font]\nbase-size = 14 # bigger\n"), Some(14.0));
        assert_eq!(parse_base_size("[font]\nfamily = \"x\"\n"), None);
        assert_eq!(parse_base_size("[font]\nbase-size = 400\n"), None);
    }

    #[test]
    fn base_size_prefers_the_users_shell_toml() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user.toml");
        let state = dir.path().join("state");
        std::fs::create_dir_all(state.join("theme")).unwrap();
        assert_eq!(read_base_size(&user, &state), 12.0);
        std::fs::write(state.join("theme/shell.toml"), "[font]\nbase-size = 13\n").unwrap();
        assert_eq!(read_base_size(&user, &state), 13.0);
        std::fs::write(&user, "[font]\nbase-size = 15\n").unwrap();
        assert_eq!(read_base_size(&user, &state), 15.0);
    }

    #[test]
    fn rounding_from_hyprctl_json() {
        assert_eq!(parse_rounding(r#"{"option": "decoration:rounding", "int": 8, "set": true }"#), Some(8.0));
        assert_eq!(parse_rounding(r#"{"int": -1}"#), None);
        assert_eq!(parse_rounding("not json"), None);
    }

    fn themes() -> Vec<Theme> {
        let light = Theme { palette: parse_palette("background = \"#f5f5f5\"\nforeground = \"#202020\"\naccent = \"#ffd60a\"\n", true), ..Theme::default() };
        let white = Theme { palette: parse_palette("background = \"#ffffff\"\nforeground = \"#3c3c3c\"\n", true), ..Theme::default() };
        let faint = Theme { palette: parse_palette("background = \"#1a1b26\"\nforeground = \"#565f89\"\n", false), ..Theme::default() };
        vec![Theme::default(), Theme { palette: parse_palette(TOKYO, false), ..Theme::default() }, light, white, faint]
    }

    #[test]
    fn oklab_round_trips_and_shades_by_lightness() {
        for c in [[0x1a, 0x1b, 0x26], [0xa9, 0xb1, 0xd6], [0xff, 0xd6, 0x0a], [0, 0, 0], [255, 255, 255]] {
            let back = from_oklab(to_oklab(c));
            for i in 0..3 {
                assert!((back[i] as i32 - c[i] as i32).abs() <= 1, "{c:?} -> {back:?}");
            }
        }
        let base = [0x1a, 0x1b, 0x26];
        assert!(luminance(shade(base, 0.05)) > luminance(base));
        assert!(luminance(shade(base, -0.05)) < luminance(base));
        assert_eq!(shade([255, 255, 255], 0.2), [255, 255, 255]);
    }

    #[test]
    fn text_strengths_meet_their_contrast_on_every_theme() {
        for th in themes() {
            let panel = th.panel();
            assert!(contrast(th.text(), panel) >= 6.9, "text {:?}", th.palette.colors.get("background"));
            let (c2, c3) = (contrast(th.text2(), panel), contrast(th.text3(), panel));
            assert!((4.5..5.2).contains(&c2), "text2 {c2}");
            assert!((2.95..3.5).contains(&c3), "text3 {c3}");
            // Secondary text still reads on a card and on the row under the pointer.
            assert!(contrast(th.text2(), th.card(false)) >= 4.0);
            assert!(contrast(th.text2(), th.hover()) >= 3.6);
            assert!(contrast(th.accent_text(), panel) >= 4.45);
            assert!(contrast(th.text(), th.accent_soft()) >= 4.5);
        }
    }

    #[test]
    fn levels_step_apart_and_lines_show() {
        for th in themes() {
            let (board, card, panel) = (th.board(None), th.card(false), th.panel());
            if !th.light() {
                assert!(luminance(card) > luminance(board) && luminance(panel) > luminance(card));
                assert!(luminance(th.hover()) > luminance(panel) && luminance(th.active()) > luminance(th.hover()));
            } else {
                assert!(luminance(th.hover()) < luminance(panel) && luminance(th.active()) < luminance(th.hover()));
            }
            assert!(contrast(th.line(), panel) >= 1.12, "line {}", contrast(th.line(), panel));
            assert!(contrast(th.line_strong(), panel) >= 1.5);
            assert!(contrast(th.dots(), board) < 1.6, "dots stay faint");
        }
    }

    #[test]
    fn sticky_paper_is_light_on_a_light_theme() {
        let day = "mode = \"light\"\nbackground = \"#e8e9ee\"\nforeground = \"#343b58\"\nyellow = \"#8c6c3e\"\ngreen = \"#587539\"\nred = \"#c64343\"\n";
        let th = Theme { palette: parse_palette(day, true), ..Theme::default() };
        for c in scene::note::STICKY_COLOURS {
            let (paper, ink) = (th.sticky_paper(c), th.sticky_ink(c));
            assert!(to_oklab(paper)[0] >= STICKY_LIGHT - 0.01, "{c}: {paper:?}");
            assert!(contrast(paper, ink) >= 4.5);
        }
        // The hue stays: yellow paper is warmer than blue paper.
        let (y, b) = (th.sticky_paper("yellow"), th.sticky_paper("blue"));
        assert!(y[0] > y[2] && b[2] > b[0], "{y:?} {b:?}");
    }

    #[test]
    fn sticky_ink_reads_on_every_sticky_colour() {
        let light = Theme { palette: parse_palette("background = \"#f5f5f5\"\nforeground = \"#202020\"\n", true), ..Theme::default() };
        for th in [Theme::default(), Theme { palette: parse_palette(TOKYO, false), ..Theme::default() }, light] {
            for c in scene::note::STICKY_COLOURS {
                let (paper, ink) = (th.sticky_paper(c), th.sticky_ink(c));
                assert!(contrast(paper, ink) >= 4.5, "{c}: {paper:?} / {ink:?}");
                assert_ne!(th.sticky_strip(c, false), paper);
            }
        }
    }

    #[test]
    fn read_palette_handles_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_palette(dir.path()), Palette::default());
        std::fs::write(dir.path().join("colors.toml"), "background = \"#202020\"\n").unwrap();
        std::fs::write(dir.path().join("light.mode"), "").unwrap();
        assert!(read_palette(dir.path()).light);
    }

    #[test]
    fn awake_marks_tell_apart() {
        let far = |a: Rgb, b: Rgb| {
            let (a, b) = (to_oklab(a), to_oklab(b));
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() >= RING_APART
        };
        let fallback = Theme::default();
        assert_eq!(fallback.awake_keep(), fallback.color("cyan"));
        assert_eq!(fallback.awake_active(), fallback.color("green"));
        // A mint cyan next to the green: Keep awake steps aside to blue.
        let mint = Theme { palette: parse_palette("cyan = \"#94e2d5\"\ngreen = \"#a6e3a1\"\nblue = \"#89b4fa\"\n", false), ..Theme::default() };
        assert_eq!(mint.awake_active(), mint.color("green"));
        assert_eq!(mint.awake_keep(), mint.color("blue"));
        assert!(far(mint.awake_keep(), mint.awake_active()));
    }

    #[test]
    fn mode_rings_tell_apart() {
        let far = |a: Rgb, b: Rgb| {
            let (a, b) = (to_oklab(a), to_oklab(b));
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() >= RING_APART
        };
        let all = [Keys::Page, Keys::Insert, Keys::Passthrough, Keys::Caret];
        let fallback = Theme::default();
        assert_eq!(fallback.keys_ring(Keys::Page), fallback.accent());
        assert_eq!(fallback.keys_ring(Keys::Insert), fallback.color("green"));
        assert_eq!(fallback.keys_ring(Keys::Passthrough), fallback.color("red"));
        assert_eq!(fallback.keys_ring(Keys::Caret), fallback.color("magenta"));
        // An accent that is the theme's green or red: those step aside.
        let green = Theme { palette: parse_palette("accent = \"#9ece6a\"\n", false), ..Theme::default() };
        assert_eq!(green.keys_ring(Keys::Insert), green.color("cyan"));
        let red = Theme { palette: parse_palette("accent = \"#f7768e\"\n", false), ..Theme::default() };
        assert_eq!(red.keys_ring(Keys::Passthrough), red.color("orange"));
        for t in [fallback, green, red, Theme { palette: parse_palette(TOKYO, false), ..Theme::default() }] {
            for (i, &a) in all.iter().enumerate() {
                for &b in &all[i + 1..] {
                    assert!(far(t.keys_ring(a), t.keys_ring(b)), "{a:?} and {b:?} too alike");
                }
            }
        }
    }

}
