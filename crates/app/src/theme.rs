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

fn luminance(c: Rgb) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// Everything Trailboard draws with, derived from the Omarchy theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    /// Hyprland's window rounding in logical px (0, sharp, without Hyprland: Omarchy's default).
    pub rounding: f32,
    /// The desktop's text size over Omarchy's default 12 px.
    pub text_scale: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme { palette: Palette::default(), rounding: 0.0, text_scale: 1.0 }
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

    /// A trail's colour (a theme colour name), or the accent for none.
    pub fn trail(&self, colour: Option<&str>) -> Rgb {
        colour.map_or_else(|| self.accent(), |c| self.color(c))
    }

    /// A sticky note's paper: theme colour `colour` itself, a little softened
    /// toward the background so it doesn't glare, like paper on the board.
    pub fn sticky_paper(&self, colour: &str) -> Rgb {
        mix(self.color(colour), self.background(), 0.08)
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

    /// A step from the background toward the foreground: surfaces and lines.
    fn step(&self, t: f32) -> Rgb {
        mix(self.background(), self.foreground(), t)
    }

    /// Panels over the board: address bar, menus, the shortcut sheet, labels.
    pub fn panel(&self) -> Rgb {
        self.step(if self.light() { 0.05 } else { 0.07 })
    }

    /// A card's frame and title bar; `focused` stands out a little more.
    pub fn card(&self, focused: bool) -> Rgb {
        match (self.light(), focused) {
            (false, false) => self.step(0.08),
            (false, true) => self.step(0.14),
            (true, false) => self.step(0.07),
            (true, true) => self.step(0.13),
        }
    }

    /// Content of a card with nothing to show yet.
    pub fn placeholder(&self) -> Rgb {
        self.step(0.04)
    }

    /// Board dots.
    pub fn dots(&self) -> Rgb {
        self.step(if self.light() { 0.25 } else { 0.2 })
    }

    /// Outline of the cards behind a stack's front.
    pub fn stack_edge(&self) -> Rgb {
        self.step(0.32)
    }

    pub fn divider(&self) -> Rgb {
        self.step(0.15)
    }

    /// Secondary text (descriptions, sleeping titles, placeholders).
    pub fn dim_text(&self) -> Rgb {
        mix(self.foreground(), self.background(), 0.38)
    }

    /// Highlighted rows and selected text.
    pub fn selection(&self) -> Rgb {
        self.color("selection")
    }

    /// Ring and band of multi-selected cards (kept apart from the accent focus ring).
    pub fn marked(&self) -> Rgb {
        self.color("yellow")
    }

    pub fn danger(&self) -> Rgb {
        self.color("red")
    }

    /// Shadows are softer on light themes.
    pub fn shadow(&self, strength: f32) -> Rgba {
        [0.0, 0.0, 0.0, strength * if self.light() { 0.45 } else { 1.0 }]
    }

    /// Corner radius of a panel, in screen px.
    pub fn radius(&self, scale: f32) -> f32 {
        self.rounding * scale
    }

    /// Corner radius of small elements (rows, highlights): at most 4 px.
    pub fn small_radius(&self, scale: f32) -> f32 {
        self.rounding.min(4.0) * scale
    }

    /// The board colour of a space with this tint (a theme colour name, or none).
    pub fn board(&self, tint: Option<&str>) -> Rgb {
        match tint {
            Some(name) => mix(self.background(), self.color(name), if self.light() { 0.10 } else { 0.07 }),
            None => self.background(),
        }
    }

    /// The colour dot that stands for a space's tint.
    pub fn swatch(&self, tint: Option<&str>) -> Rgb {
        match tint {
            Some(name) => self.color(name),
            None => self.dim_text(),
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
    fn rounding_caps_small_elements() {
        let t = Theme { rounding: 10.0, ..Theme::default() };
        assert_eq!(t.radius(2.0), 20.0);
        assert_eq!(t.small_radius(2.0), 8.0);
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

    #[test]
    fn sticky_ink_reads_on_every_sticky_colour() {
        let contrast = |a: Rgb, b: Rgb| {
            let (la, lb) = (luminance(a), luminance(b));
            (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
        };
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
}
