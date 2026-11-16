//! The one control that stays on screen over the board: a small panel at
//! the top left with the sidebar's button and the space's name (which opens
//! the list of spaces). Everything else is on the board's right-click menu,
//! the palette and the keys. A control shows its name and key when the
//! pointer rests on it.
//!
//! Layout and hit testing are pure (`layout`, `Hud::at`); `layer` draws.
//! `nudge` is the line of guidance an empty board or an empty view shows.

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
}
