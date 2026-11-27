//! The state a page shares with its CEF handlers: size, scale, frame, popup.

use std::cell::RefCell;

use cef::*;

use crate::popup::{self, IRect};
use crate::{cosmetic, inspector, javascript_on, media, note_activity, page_zoom, PageKind, PaintFrame, send_dev_tools};

pub(crate) struct Shared {
    pub(crate) key: u64,
    /// View size in CSS pixels.
    pub(crate) size: RefCell<(i32, i32)>,
    /// Raster scale: pixels per CSS pixel. CEF 156 OSR ignores the device scale
    /// factor we report, so we get it with page zoom instead: the view is
    /// `size × scale` pixels and the page zoom is `scale`, which keeps the CSS
    /// layout at `size` while painting `scale`× sharper. See [`zoom_level_for`].
    pub(crate) scale: RefCell<f32>,
    /// The last few (size, scale) pairs, newest first, to tell which one a
    /// paint belongs to ([`paint_css`]).
    pub(crate) recent: RefCell<Vec<((i32, i32), f32)>>,
    /// What the app shows: the page with any open popup drawn on top.
    pub(crate) frame: RefCell<PaintFrame>,
    /// While a popup is open, the page's own pixels (so the popup can move or close cleanly).
    pub(crate) view: RefCell<Option<PaintFrame>>,
    pub(crate) popup: RefCell<PopupState>,
    /// Text currently selected in the page (Chromium reports every change).
    pub(crate) selection: RefCell<String>,
    /// Bounds of the IME composition (or the caret), view px.
    pub(crate) ime_caret: RefCell<Option<IRect>>,
    /// URL the page was created with; the first navigation to it is never redirected beside.
    pub(crate) initial_url: String,
    /// The main frame's address, for its user zoom ([`set_page_zoom`]).
    pub(crate) url: RefCell<String>,
    /// The icon last asked for, so a page that repeats it isn't fetched again.
    pub(crate) icon_url: RefCell<String>,
    pub(crate) kind: PageKind,
    /// The main frame has committed a page (a link that is a file never does).
    pub(crate) committed: std::cell::Cell<bool>,
    /// The frames hiding elements by the filter lists' cosmetic rules.
    pub(crate) hide: cosmetic::Frames,
    /// The main frame's scriptlets, registered to run before its page's scripts.
    pub(crate) early: cosmetic::Early,
    /// A frame clicked a cookie banner's reject button ([`consent`]) since
    /// the main frame last started loading.
    pub(crate) banner_rejected: std::cell::Cell<bool>,
    /// What each frame says about its media ([`media`]).
    pub(crate) media: RefCell<media::Frames>,
    /// The page is in full screen (`on_fullscreen_mode_change`).
    pub(crate) fullscreen: std::cell::Cell<bool>,
    /// The page keeps its focus whatever the app asks ([`Page::hold_focus`]).
    pub(crate) hold_focus: std::cell::Cell<bool>,
    /// JavaScript is off in this page (last sent with [`Shared::apply_javascript`]).
    pub(crate) script_off: std::cell::Cell<bool>,
    /// An inspector page's line to the page it inspects.
    pub(crate) bridge: RefCell<Option<inspector::Bridge>>,
}

#[derive(Default)]
pub(crate) struct PopupState {
    pub(crate) shown: bool,
    /// Where Chromium placed the popup, in view pixels.
    pub(crate) rect: IRect,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

/// `EVENTFLAG_PRECISION_SCROLLING_DELTA` (cef_types.h): a wheel event with
/// exact pixel deltas, as from a touchpad.
pub(crate) const PRECISE_SCROLL: u32 = 1 << 14;

/// Chromium zoom level (factor = 1.2^level) that renders at `scale`.
pub fn zoom_level_for(scale: f32) -> f64 {
    (scale.max(0.01) as f64).ln() / 1.2f64.ln()
}

impl Shared {
    /// View size in pixels (what CEF lays out and paints).
    pub(crate) fn view_px(&self) -> (i32, i32) {
        let (w, h) = *self.size.borrow();
        let s = *self.scale.borrow();
        (((w as f32) * s).round().max(1.0) as i32, ((h as f32) * s).round().max(1.0) as i32)
    }

    /// The user's zoom for this page (1.0 for none; notes are never zoomed).
    pub(crate) fn user_zoom(&self) -> f64 {
        match self.kind {
            PageKind::Web => page_zoom(&self.url.borrow()),
            PageKind::Note | PageKind::App | PageKind::Inspector => 1.0,
        }
    }

    /// Turns JavaScript on or off in the page as [`set_javascript`] says for
    /// `url` (where it is going). Web pages only: the app's own pages need theirs.
    pub(crate) fn apply_javascript(&self, host: &BrowserHost, url: &str) {
        if self.kind != PageKind::Web {
            return;
        }
        let off = !javascript_on(url);
        if off == self.script_off.get() {
            return;
        }
        self.script_off.set(off);
        log::info!("page {}: JavaScript {}", self.key, if off { "off" } else { "on" });
        send_dev_tools(host, "Emulation.setScriptExecutionDisabled", &format!(r#"{{"value":{off}}}"#));
    }

    /// Re-assert our page zoom; Chromium keeps zoom per origin, so a navigation can reset it.
    /// It is the raster scale times the user's zoom: the view stays `size × scale`
    /// pixels, so the page lays out `size / zoom` CSS px wide. Input and popup
    /// coordinates are view px / scale (card units) either way, so they need no change.
    pub(crate) fn apply_zoom(&self, host: &BrowserHost) {
        let want = zoom_level_for((*self.scale.borrow() as f64 * self.user_zoom()) as f32);
        if (host.zoom_level() - want).abs() > 1e-3 {
            log::debug!("page {} zoom level {:.3} -> {want:.3}", self.key, host.zoom_level());
            host.set_zoom_level(want);
        }
    }

    /// Where the popup is drawn (view px): Chromium's rect moved inside the card.
    pub(crate) fn popup_drawn(&self) -> Option<(IRect, IRect)> {
        let p = self.popup.borrow();
        if !p.shown || p.rect.w <= 0 || p.rect.h <= 0 {
            return None;
        }
        let (w, h) = self.view_px();
        Some((p.rect, popup::clamp_into_view(p.rect, w, h)))
    }

    /// Map a point in the card (CSS px) to the view pixel Chromium expects. A point over
    /// the drawn popup is shifted back to where Chromium thinks the popup is.
    pub(crate) fn widget_point(&self, x: i32, y: i32) -> (i32, i32) {
        let s = *self.scale.borrow();
        let (x, y) = ((x as f32 * s).round() as i32, (y as f32 * s).round() as i32);
        match self.popup_drawn() {
            Some((orig, drawn)) if drawn.contains(x, y) => (x - (drawn.x - orig.x), y - (drawn.y - orig.y)),
            _ => (x, y),
        }
    }

    /// Rebuild `frame` from the clean view plus the popup.
    pub(crate) fn compose(&self) {
        let view = self.view.borrow();
        let Some(view) = view.as_ref() else { return };
        let mut f = self.frame.borrow_mut();
        f.width = view.width;
        f.height = view.height;
        f.css = view.css;
        f.pixels.clear();
        f.pixels.extend_from_slice(&view.pixels);
        if let Some((_, drawn)) = self.popup_drawn() {
            let p = self.popup.borrow();
            // Right after a scale change the last paint can lag the requested size;
            // map view px onto the pixels we actually have.
            let scale = view.width as f32 / self.view_px().0 as f32;
            // Clamp again in frame pixels, using the buffer's real size.
            let mut dev = drawn.scaled(scale);
            dev.w = p.width as i32;
            dev.h = p.height as i32;
            let dev = popup::clamp_into_view(dev, view.width as i32, view.height as i32);
            log::trace!(
                "page {} popup blit {:?} buffer {}x{} view {}x{} scale {scale}",
                self.key, dev, p.width, p.height, view.width, view.height
            );
            popup::blit(&mut f.pixels, (view.width, view.height), &p.pixels, (p.width, p.height), (dev.x, dev.y));
        }
        f.bump(None);
        note_activity();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_level_matches_chromium_factor() {
        for s in [0.5f32, 1.0, 1.25, 2.0] {
            let factor = 1.2f64.powf(zoom_level_for(s));
            assert!((factor - s as f64).abs() < 1e-9, "{s}: {factor}");
        }
        assert_eq!(zoom_level_for(1.0), 0.0);
    }
}
