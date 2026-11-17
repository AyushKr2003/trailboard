//! The one control that stays on screen over the board: a small panel at
//! the top left with the sidebar's button and the space's name (which opens
//! the list of spaces). Everything else is on the board's right-click menu,
//! the palette and the keys. A control shows its name and key when the
//! pointer rests on it.
//!
//! Layout and hit testing are pure (`layout`, `Hud::at`); `layer` draws.
//! `welcome` is what an empty board shows; `nudge` the line of guidance a
//! view with no card in it shows.

use std::time::Duration;

use compositor::{Layer, Renderer};
use scene::{Rect, Vec2};

use crate::chrome::{self, Lift, Press, type_};
use crate::icons::{self, Icon};
use crate::palette::Command;
use crate::theme::{Rgb, Theme, rgb};

/// Something on the board's chrome that can be pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    Sidebar,
    /// The space's name: opens the list of spaces.
    Space,
}

impl Control {
    /// What it is called, the keys that do the same, and its icon.
    pub fn info(self) -> (&'static str, &'static str, Icon) {
        match self {
            Control::Sidebar => ("Trails sidebar", "B", Icon::Sidebar),
            Control::Space => ("Spaces", "", Icon::ChevronDown),
        }
    }

    /// The palette command a press runs, for those that are one.
    pub fn command(self) -> Option<Command> {
        Some(match self {
            Control::Sidebar => Command::Sidebar,
            _ => return None,
        })
    }
}

/// What a press on the board's guidance does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Go {
    Command(Command),
    /// Ctrl+K, which is a key rather than a palette command.
    Palette,
}

/// Sizes in logical px; multiplied by the UI scale.
pub const MARGIN: f32 = 12.0;
pub const BUTTON: f32 = 32.0;
const PAD: f32 = 4.0;
const GAP: f32 = 2.0;
/// The space's name is cut beyond this.
const SPACE_MAX: f32 = 180.0;
/// How long the pointer rests on a control before its tooltip shows.
pub const TIP_DWELL: Duration = Duration::from_millis(450);

/// Where everything is, in screen px.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hud {
    /// The panels behind the groups of controls.
    pub panels: Vec<Rect>,
    pub controls: Vec<(Control, Rect)>,
    /// Hairlines between groups inside a panel.
    pub dividers: Vec<Rect>,
}

impl Hud {
    /// The control at `p`.
    pub fn at(&self, p: Vec2) -> Option<Control> {
        self.controls.iter().find(|(_, r)| r.contains(p)).map(|(c, _)| *c)
    }

    /// `p` is on a panel (between buttons too): the board under it gets no click.
    pub fn covers(&self, p: Vec2) -> bool {
        self.panels.iter().any(|r| r.contains(p))
    }

    pub fn rect(&self, c: Control) -> Option<Rect> {
        self.controls.iter().find(|(k, _)| *k == c).map(|(_, r)| *r)
    }

}

/// The chrome for a window of `viewport`, with the sidebar's right edge at
/// `left` (0 when it is closed) and the space's name `space_w` px wide.
pub fn layout(viewport: Vec2, ui: f32, left: f32, space_w: f32) -> Hud {
    let mut hud = Hud::default();
    let m = MARGIN * ui;
    let h = (BUTTON + 2.0 * PAD) * ui;
    // Top left: the sidebar's button, then the space.
    let (x, top) = (left + m, m);
    let name_w = space_w.min(SPACE_MAX * ui);
    // inset, dot, gap, name, gap, chevron, inset
    let space = (8.0 + 10.0 + 8.0) * ui + name_w.ceil() + 1.0 + (8.0 + 12.0 + 8.0) * ui;
    let panel = Rect::new(x, top, PAD * ui + BUTTON * ui + GAP * ui + space + PAD * ui, h);
    if panel.right() + m <= viewport.x {
        hud.controls.push((Control::Sidebar, Rect::new(x + PAD * ui, top + PAD * ui, BUTTON * ui, BUTTON * ui)));
        hud.controls.push((Control::Space, Rect::new(x + PAD * ui + BUTTON * ui + GAP * ui, top + PAD * ui, space, BUTTON * ui)));
        hud.panels.push(panel);
    }
    hud
}

/// What the chrome shows this frame.
pub struct View<'a> {
    pub space: &'a str,
    /// The space's tint.
    pub swatch: Rgb,
    pub private: bool,
    pub sidebar_open: bool,
    /// The control under the pointer, and whether the button is held on it.
    pub hover: Option<Control>,
    pub down: bool,
    /// The control whose menu is open.
    pub open: Option<Control>,
}

pub fn layer(r: &mut Renderer, hud: &Hud, v: &View, th: &Theme, ui: f32) -> Layer {
    let mut l = Layer::default();
    let radius = th.radius(ui / th.text_scale);
    let small = th.small_radius(ui / th.text_scale);
    for p in &hud.panels {
        chrome::panel(&mut l, *p, radius, th, ui, Lift::Mid, th.panel());
    }
    for d in &hud.dividers {
        chrome::fill(&mut l, *d, 0.0, rgb(th.line(), 1.0));
    }
    for &(c, rect) in &hud.controls {
        let press = match (v.hover == Some(c), v.down, v.open == Some(c)) {
            (_, _, true) | (true, true, _) => Press::Down,
            (true, false, _) => Press::Hover,
            _ => Press::Idle,
        };
        let mid = rect.center();
        match c {
            Control::Space => {
                if press != Press::Idle {
                    chrome::fill(&mut l, rect, small, rgb(if press == Press::Down { th.active() } else { th.hover() }, 1.0));
                }
                let d = 10.0 * ui;
                let dot = Rect::new((rect.x + 8.0 * ui).round(), (mid.y - d / 2.0).round(), d, d);
                if v.private {
                    // A private space's mark is hollow, as in the sidebar.
                    chrome::outline(&mut l, dot, d / 2.0, (2.0 * ui).max(1.5), rgb(v.swatch, 1.0));
                } else {
                    chrome::fill(&mut l, dot, d / 2.0, rgb(v.swatch, 1.0));
                }
                let size = type_::BODY * ui;
                let x = dot.right() + 8.0 * ui;
                let room = rect.right() - (8.0 + 12.0 + 8.0) * ui - x;
                let name = r.ellipsize(v.space, size, true, room);
                chrome::text(&mut l, name, x, mid.y, size, rgb(th.text(), 1.0), Rect::new(x, rect.y, room.max(1.0), rect.h), true);
                icons::draw(&mut l, r, Icon::ChevronDown, rect.right() - 14.0 * ui, mid.y, 12.0 * ui, rgb(th.text3(), 1.0));
            }
            _ => {
                let on = c == Control::Sidebar && v.sidebar_open;
                let press = if on && press == Press::Idle { Press::Down } else { press };
                chrome::icon_button(&mut l, r, rect, c.info().2, press, on, true, th, ui);
            }
        }
    }
    l
}

/// The tooltip of `c`: above a control in the lower half of the window, else below it.
pub fn tip(r: &mut Renderer, hud: &Hud, c: Control, viewport: Vec2, th: &Theme, ui: f32) -> Option<Layer> {
    let rect = hud.rect(c)?;
    let (label, keys, _) = c.info();
    Some(chrome::tooltip(r, label, keys, rect, rect.y > viewport.y / 2.0, viewport, th, ui))
}

/// A line of guidance with one button, centred at `mid_x` with its bottom at
/// `bottom`: what an empty board, or a view with no card in it, offers.
/// Returns the layer and the button's rect.
#[allow(clippy::too_many_arguments)]
pub fn nudge(r: &mut Renderer, text: &str, action: &str, keys: &str, mid_x: f32, bottom: f32, hot: bool, th: &Theme, ui: f32) -> (Layer, Rect) {
    let mut l = Layer::default();
    let size = type_::BODY * ui;
    let ksize = type_::CAPTION * ui;
    let (pad, h, bh) = (12.0 * ui, 40.0 * ui, 28.0 * ui);
    let tw = r.measure(text, size, false);
    let aw = r.measure(action, size, true);
    let caps = chrome::keycaps(keys).unwrap_or_default();
    let widths: Vec<f32> = caps.iter().map(|k| r.measure(k, ksize, false)).collect();
    let kw = if widths.is_empty() { 0.0 } else { chrome::keycaps_width(&widths, ksize) + 8.0 * ui };
    let bw = 10.0 * ui + aw + kw + if widths.is_empty() { 10.0 * ui } else { 6.0 * ui };
    let w = pad + tw + 12.0 * ui + bw + 6.0 * ui;
    let rect = Rect::new((mid_x - w / 2.0).round(), (bottom - h).round(), w.round(), h);
    chrome::panel(&mut l, rect, th.radius(ui / th.text_scale), th, ui, Lift::Mid, th.panel());
    chrome::text(&mut l, text, rect.x + pad, rect.center().y, size, rgb(th.text2(), 1.0), rect, false);
    let button = Rect::new(rect.right() - 6.0 * ui - bw, rect.y + (h - bh) / 2.0, bw, bh);
    let small = th.small_radius(ui / th.text_scale);
    chrome::fill(&mut l, button, small, rgb(if hot { th.active() } else { th.hover() }, 1.0));
    chrome::text(&mut l, action, button.x + 10.0 * ui, button.center().y, size, rgb(th.text(), 1.0), button, true);
    if !widths.is_empty() {
        chrome::draw_keycaps(&mut l, &caps, &widths, button.right() - 6.0 * ui, button.center().y, ksize, th, ui, button);
    }
    (l, button)
}

/// What an empty board offers, as (label, keys, what a press does).
pub const WELCOME_ACTIONS: [(&str, &str, Go); 3] = [
    ("Open a page", "Ctrl+T", Go::Command(Command::NewCard)),
    ("Search everything", "Ctrl+K", Go::Palette),
    ("All keys", "?", Go::Command(Command::Shortcuts)),
];

/// The welcome's big line, and the one under it (a private space says
/// what it keeps).
pub const WELCOME_DISPLAY: &str = "Where to first?";
const WELCOME_LINE: &str = "Open a page. Its links open beside it, joined by a trail.";
const WELCOME_LINE_PRIVATE: &str = "Pages here stay out of history, and their cookies go when you quit.";

/// Widths (screen px) of what the welcome shows, measured by the caller.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WelcomeSizes {
    pub label: f32,
    pub display: f32,
    pub line: f32,
    /// Each action's whole chip.
    pub chips: Vec<f32>,
}

/// Where the welcome's parts go, in screen px; mid lines for the texts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WelcomeLayout {
    /// Everything, for the sheet behind it over a photo.
    pub block: Rect,
    pub label_y: f32,
    pub display_y: f32,
    pub line_y: f32,
    pub chips: Vec<Rect>,
}

/// Height of a welcome chip, logical px.
const CHIP_H: f32 = 32.0;
const CHIP_GAP: f32 = 8.0;

/// Lays the welcome out in `area` (the board beside the sidebar): centred
/// across, its middle a little above the area's (where the eye rests).
/// The chips sit in a row, or one above the other when the row doesn't fit,
/// each as wide as the widest then.
pub fn welcome_layout(s: &WelcomeSizes, area: Rect, ui: f32) -> WelcomeLayout {
    let (label_h, display_h, line_h) = (chrome::line_h(type_::CAPTION * ui), chrome::line_h(type_::DISPLAY * ui), chrome::line_h(type_::BODY * ui));
    let (gap, h) = (CHIP_GAP * ui, CHIP_H * ui);
    let row_w = s.chips.iter().sum::<f32>() + gap * s.chips.len().saturating_sub(1) as f32;
    let room = (area.w - 32.0 * ui).max(0.0);
    let stacked = row_w > room;
    let chips_h = if stacked { s.chips.len() as f32 * (h + gap) - gap } else { h };
    let total = label_h + 10.0 * ui + display_h + 6.0 * ui + line_h + 24.0 * ui + chips_h;
    let mid_x = area.x + area.w / 2.0;
    let top = (area.y + area.h * 0.45 - total / 2.0).max(area.y + 16.0 * ui).round();
    let label_y = top + label_h / 2.0;
    let display_y = top + label_h + 10.0 * ui + display_h / 2.0;
    let line_y = display_y + display_h / 2.0 + 6.0 * ui + line_h / 2.0;
    let chips_top = line_y + line_h / 2.0 + 24.0 * ui;
    let chips = if stacked {
        let w = s.chips.iter().copied().fold(0.0, f32::max);
        (0..s.chips.len())
            .map(|i| Rect::new((mid_x - w / 2.0).round(), (chips_top + i as f32 * (h + gap)).round(), w.round(), h))
            .collect()
    } else {
        let mut x = mid_x - row_w / 2.0;
        s.chips
            .iter()
            .map(|w| {
                let r = Rect::new(x.round(), chips_top.round(), w.round(), h);
                x += w + gap;
                r
            })
            .collect()
    };
    let wide = s.label.max(s.display).max(s.line).max(if stacked { s.chips.iter().copied().fold(0.0, f32::max) } else { row_w });
    let block = Rect::new(mid_x - wide / 2.0, top, wide, total).inflate(24.0 * ui);
    WelcomeLayout { block, label_y, display_y, line_y, chips }
}

/// What the welcome shows this frame.
pub struct WelcomeView<'a> {
    pub space: &'a str,
    pub swatch: Rgb,
    pub private: bool,
    /// The board is a photo: the welcome sits on a sheet to read over it.
    pub on_photo: bool,
    /// The pointer, if it may press a chip, and whether its button is held.
    pub pointer: Option<Vec2>,
    pub down: bool,
}

/// The calm page an empty board shows: the space's name in small spaced
/// capitals after its colour, a large display line, a quiet line under it,
/// and a few actions with their keys, straight on the board's colour mood.
/// Returns the layer and each action's rect.
pub fn welcome(r: &mut Renderer, v: &WelcomeView, area: Rect, th: &Theme, ui: f32) -> (Layer, Vec<(Rect, Go)>) {
    let mut l = Layer::default();
    let (cap, body, display) = (type_::CAPTION * ui, type_::BODY * ui, type_::DISPLAY * ui);
    // The space's name, cut so a long one stays a label.
    let mut name: String = v.space.trim().chars().take(28).collect::<String>().to_uppercase();
    if v.space.trim().chars().count() > 28 {
        name.push('…');
    }
    if v.private {
        name.push_str(" · PRIVATE");
    }
    let dot = 8.0 * ui;
    let track = |r: &mut Renderer, s: &str| s.chars().map(|c| if c.is_whitespace() { cap * 0.3 } else { r.measure(&c.to_string(), cap, true) } + cap * 0.1).sum::<f32>();
    let label_w = dot + 8.0 * ui + track(r, &name);
    let line = if v.private { WELCOME_LINE_PRIVATE } else { WELCOME_LINE };
    let caps: Vec<(Vec<&str>, Vec<f32>)> = WELCOME_ACTIONS
        .iter()
        .map(|(_, keys, _)| {
            let k = chrome::keycaps(keys).unwrap_or_default();
            let w = k.iter().map(|k| r.measure(k, cap, false)).collect();
            (k, w)
        })
        .collect();
    let chip_w: Vec<f32> = WELCOME_ACTIONS
        .iter()
        .zip(&caps)
        .map(|((label, _, _), (_, w))| 12.0 * ui + r.measure(label, body, true) + 10.0 * ui + chrome::keycaps_width(w, cap) + 6.0 * ui)
        .collect();
    let sizes = WelcomeSizes { label: label_w, display: r.measure(WELCOME_DISPLAY, display, true), line: r.measure(line, body, false), chips: chip_w };
    let lay = welcome_layout(&sizes, area, ui);
    let radius = th.radius(ui / th.text_scale);
    if v.on_photo {
        chrome::panel(&mut l, lay.block, radius, th, ui, Lift::Mid, th.panel());
    }
    let mid_x = area.x + area.w / 2.0;
    let clip = Rect::new(area.x, lay.block.y, area.w, lay.block.h);
    // The space's colour and name.
    let x = (mid_x - label_w / 2.0).round();
    let d = Rect::new(x, (lay.label_y - dot / 2.0).round(), dot, dot);
    if v.private {
        chrome::outline(&mut l, d, dot / 2.0, (1.5 * ui).max(1.5), rgb(v.swatch, 1.0));
    } else {
        chrome::fill(&mut l, d, dot / 2.0, rgb(v.swatch, 1.0));
    }
    chrome::spaced(&mut l, r, &name, d.right() + 8.0 * ui, lay.label_y, cap, rgb(th.text2(), 1.0), clip);
    chrome::text(&mut l, WELCOME_DISPLAY, (mid_x - sizes.display / 2.0).round(), lay.display_y, display, rgb(th.text(), 1.0), clip, true);
    chrome::text(&mut l, line, (mid_x - sizes.line / 2.0).round(), lay.line_y, body, rgb(th.text2(), 1.0), clip, false);
    // The actions: quiet sheets at rest, lifted under the pointer.
    let small = th.small_radius(ui / th.text_scale);
    let mut out = Vec::new();
    for ((&rect, (label, _, go)), (keys, widths)) in lay.chips.iter().zip(WELCOME_ACTIONS.iter()).zip(&caps) {
        let hot = v.pointer.is_some_and(|p| rect.contains(p));
        if hot {
            chrome::lift(&mut l, rect, small, th, ui, Lift::Low);
            chrome::fill(&mut l, rect, small, rgb(if v.down { th.active() } else { th.panel() }, 1.0));
        } else {
            chrome::fill(&mut l, rect, small, rgb(th.panel(), if v.on_photo { 1.0 } else { 0.55 }));
        }
        chrome::outline(&mut l, rect, small, chrome::hair(ui), rgb(if hot { th.line_strong() } else { th.line() }, 1.0));
        chrome::text(&mut l, *label, rect.x + 12.0 * ui, rect.center().y, body, rgb(th.text(), 1.0), rect, true);
        chrome::draw_keycaps(&mut l, keys, widths, rect.right() - 6.0 * ui, rect.center().y, cap, th, ui, rect);
        out.push((rect, *go));
    }
    (l, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_sits_top_left_with_full_targets() {
        let vp = Vec2::new(1280.0, 800.0);
        let hud = layout(vp, 1.0, 0.0, 60.0);
        assert_eq!(hud.panels.len(), 1);
        assert_eq!((hud.panels[0].x, hud.panels[0].y), (MARGIN, MARGIN));
        // Every control is at least 32 px each way (WCAG 2.5.8 asks for 24).
        assert!(hud.controls.iter().all(|(_, r)| r.w >= 32.0 && r.h >= 32.0));
        let (a, b) = (hud.rect(Control::Sidebar).unwrap(), hud.rect(Control::Space).unwrap());
        assert!(!a.inflate(-0.5).intersects(&b.inflate(-0.5)));
        assert_eq!(hud.at(a.center()), Some(Control::Sidebar));
        assert_eq!(hud.at(b.center()), Some(Control::Space));
        assert_eq!(hud.at(Vec2::new(640.0, 300.0)), None);
        assert!(hud.covers(Vec2::new(MARGIN + 1.0, MARGIN + 1.0)));
    }

    #[test]
    fn an_open_sidebar_pushes_the_panel_right_and_long_names_are_cut() {
        let vp = Vec2::new(1280.0, 800.0);
        let (closed, open) = (layout(vp, 1.0, 0.0, 60.0), layout(vp, 1.0, 288.0, 60.0));
        assert_eq!(open.rect(Control::Sidebar).unwrap().x - closed.rect(Control::Sidebar).unwrap().x, 288.0);
        let long = layout(vp, 1.0, 0.0, 4000.0);
        assert!(long.rect(Control::Space).unwrap().w <= SPACE_MAX + 60.0);
        // No room beside a sidebar in a very narrow window: no panel.
        assert!(layout(Vec2::new(360.0, 800.0), 1.0, 288.0, 60.0).panels.is_empty());
    }

    #[test]
    fn controls_name_themselves() {
        assert_eq!(Control::Sidebar.command(), Some(Command::Sidebar));
        assert_eq!(Control::Space.command(), None);
        assert!(chrome::keycaps(Control::Sidebar.info().1).is_some());
    }

    fn sizes() -> WelcomeSizes {
        WelcomeSizes { label: 90.0, display: 220.0, line: 340.0, chips: vec![150.0, 180.0, 100.0] }
    }

    #[test]
    fn the_welcome_is_centred_with_its_chips_in_a_row() {
        let area = Rect::new(0.0, 0.0, 1280.0, 800.0);
        let lay = welcome_layout(&sizes(), area, 1.0);
        assert_eq!(lay.chips.len(), 3);
        let (first, last) = (lay.chips[0], lay.chips[2]);
        assert_eq!(first.y, last.y, "one row");
        assert!(((first.x + last.right()) / 2.0 - 640.0).abs() <= 1.0, "centred across");
        assert!(lay.chips.windows(2).all(|w| w[0].right() < w[1].x), "no overlap");
        assert!(lay.chips.iter().all(|c| c.h >= 24.0), "targets of at least 24 px");
        assert!(lay.label_y < lay.display_y && lay.display_y < lay.line_y && lay.line_y < first.y);
        // A little above the middle, and the block holds everything.
        assert!(lay.block.center().y < 400.0);
        assert!(lay.chips.iter().all(|c| lay.block.contains(Vec2::new(c.x, c.y)) && lay.block.contains(Vec2::new(c.right(), c.bottom()))));
    }

    #[test]
    fn a_narrow_board_stacks_the_chips_and_a_sidebar_moves_the_middle() {
        let lay = welcome_layout(&sizes(), Rect::new(0.0, 0.0, 420.0, 800.0), 1.0);
        assert!(lay.chips.windows(2).all(|w| w[0].bottom() < w[1].y && w[0].x == w[1].x && w[0].w == w[1].w));
        let beside = welcome_layout(&sizes(), Rect::new(288.0, 0.0, 992.0, 800.0), 1.0);
        assert!((beside.block.center().x - (288.0 + 496.0)).abs() <= 1.0);
        // A short window never pushes it above the top.
        let short = welcome_layout(&sizes(), Rect::new(0.0, 0.0, 1280.0, 120.0), 1.0);
        assert!(short.label_y > 0.0);
    }
}
