//! Chromium (CEF) in off-screen-rendering mode. Each card owns one windowless
//! browser whose frames arrive as BGRA pixel buffers; the app uploads them to
//! GPU textures. All CEF callbacks run on the main thread because we drive CEF
//! with `external_message_pump` + [`pump`], so plain `Rc`/`RefCell` are enough.
//! The one exception is request blocking ([`shield`]), asked on Chromium's IO thread.

pub mod consent;
pub mod cosmetic;
pub mod download;
pub mod keys;
pub mod links;
pub mod menu;
pub mod popup;
pub mod sandbox;
pub mod shield;
pub mod text;

use cef::{args::Args, *};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc as StdRc;

use popup::IRect;

/// Something a page asked the app to do. Drained once per frame with [`drain_events`].
#[derive(Debug, Clone)]
pub enum WebEvent {
    /// A link (or `window.open`) on page `source` should open as a new card beside it.
    OpenBeside { source: u64, url: String },
    TitleChanged { page: u64, title: String },
    UrlChanged { page: u64, url: String },
    LoadingChanged { page: u64, loading: bool },
    /// The mouse cursor the page wants while the pointer is over it.
    CursorChanged { page: u64, cursor: Cursor },
    /// The user right-clicked at (`x`, `y`) CSS px in the page; the app shows its own menu.
    ContextMenu { page: u64, x: f32, y: f32, context: menu::MenuContext },
    /// The text of note page `page` changed (sent by the note editor, see [`PageKind::Note`]).
    NoteText { page: u64, text: String },
    /// The links visible in page `page`, answering [`Page::request_links`]:
    /// the page's layout width in CSS px (to scale their rects to the view's)
    /// and the links. Any page can send this, so only expect it after asking.
    Links { page: u64, width: f32, links: Vec<links::Link> },
    /// Find in page ([`Page::find`]): `count` matches, the `active` one (1-based,
    /// 0 for none); `done` once Chromium has finished counting.
    FindResult { page: u64, count: i32, active: i32, done: bool },
    /// The icon of page `page`, which was showing `page_url` when it said which
    /// icon it has: BGRA8 with straight (not premultiplied) alpha, at most [`ICON_SIZE`] px a side.
    Favicon { page: u64, page_url: String, width: u32, height: u32, pixels: Vec<u8> },
    /// The text of web page `page` at `url`, answering [`Page::request_text`].
    /// `url` is the page's address as the browser knows it (a page can't claim
    /// another); the message is dropped if the page has moved on since.
    PageText { page: u64, url: String, title: String, text: String },
    /// Trailboard was started again (from a launcher, or another app opening a
    /// link) while this instance runs on the same profile. Chromium's process
    /// singleton handed us its command line: the plain `args` (URLs or paths,
    /// switches left out) and the directory it was started in. The other
    /// process exits with [`Startup::Handed`].
    Relaunch { args: Vec<String>, cwd: String },
    /// Page `page` started download `id` of `url`, which Chromium would call
    /// `suggested`. Answer with [`download::save_to`]. `blank`: the page never
    /// showed anything (a card opened for a link that turned out to be a file).
    DownloadAsked { page: u64, id: u32, url: String, suggested: String, blank: bool },
    /// Progress of a download answered with [`download::save_to`].
    DownloadUpdated { id: u32, received: i64, total: i64, state: download::Progress },
    /// A [`shield::fetch`] tagged `tag` ended: `ok` with the whole body of an
    /// HTTP 200 answer, else failed (`body` empty).
    Fetched { tag: String, ok: bool, body: Vec<u8> },
}

/// Largest favicon side asked of Chromium, px (title bars draw 16 px icons, sharp at 2×).
pub const ICON_SIZE: u32 = 32;

/// What a page is: a web page, or one of the app's own note editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageKind {
    #[default]
    Web,
    /// The app's note editor. It never navigates away (links open beside), its
    /// title and address are not reported, and it sends its text as a console
    /// message starting with [`NOTE_MESSAGE`], which becomes [`WebEvent::NoteText`].
    Note,
}

/// Prefix of the console message a note editor sends with its whole text.
pub const NOTE_MESSAGE: &str = "\u{1}trailboard-note\u{1}";

/// Prefix of the console message a note editor sends with a link that was
/// clicked: it opens beside the note. (A `data:` page may not open `file:`
/// links itself, and this needs no popup permission.)
pub const NOTE_OPEN: &str = "\u{1}trailboard-open\u{1}";

/// Editing commands run on the page's focused frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

/// Mouse cursor shapes a page can ask for (CSS `cursor`), independent of any windowing crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    VerticalText,
    Wait,
    Progress,
    Help,
    Crosshair,
    Cell,
    Move,
    Grab,
    Grabbing,
    NotAllowed,
    NoDrop,
    Copy,
    Alias,
    ContextMenu,
    ZoomIn,
    ZoomOut,
    AllScroll,
    ResizeE,
    ResizeN,
    ResizeNE,
    ResizeNW,
    ResizeS,
    ResizeSE,
    ResizeSW,
    ResizeW,
    ResizeNS,
    ResizeEW,
    ResizeNESW,
    ResizeNWSE,
    ResizeCol,
    ResizeRow,
    /// `cursor: none`.
    Hidden,
}

impl Cursor {
    fn from_cef(t: CursorType) -> Cursor {
        match t {
            CursorType::HAND => Cursor::Pointer,
            CursorType::IBEAM => Cursor::Text,
            CursorType::VERTICALTEXT => Cursor::VerticalText,
            CursorType::WAIT => Cursor::Wait,
            CursorType::PROGRESS => Cursor::Progress,
            CursorType::HELP => Cursor::Help,
            CursorType::CROSS => Cursor::Crosshair,
            CursorType::CELL => Cursor::Cell,
            CursorType::MOVE | CursorType::DND_MOVE => Cursor::Move,
            CursorType::GRAB => Cursor::Grab,
            CursorType::GRABBING => Cursor::Grabbing,
            CursorType::NOTALLOWED => Cursor::NotAllowed,
            CursorType::NODROP | CursorType::DND_NONE => Cursor::NoDrop,
            CursorType::COPY | CursorType::DND_COPY => Cursor::Copy,
            CursorType::ALIAS | CursorType::DND_LINK => Cursor::Alias,
            CursorType::CONTEXTMENU => Cursor::ContextMenu,
            CursorType::ZOOMIN => Cursor::ZoomIn,
            CursorType::ZOOMOUT => Cursor::ZoomOut,
            CursorType::MIDDLEPANNING
            | CursorType::MIDDLE_PANNING_VERTICAL
            | CursorType::MIDDLE_PANNING_HORIZONTAL
            | CursorType::EASTPANNING
            | CursorType::NORTHPANNING
            | CursorType::NORTHEASTPANNING
            | CursorType::NORTHWESTPANNING
            | CursorType::SOUTHPANNING
            | CursorType::SOUTHEASTPANNING
            | CursorType::SOUTHWESTPANNING
            | CursorType::WESTPANNING => Cursor::AllScroll,
            CursorType::EASTRESIZE => Cursor::ResizeE,
            CursorType::NORTHRESIZE => Cursor::ResizeN,
            CursorType::NORTHEASTRESIZE => Cursor::ResizeNE,
            CursorType::NORTHWESTRESIZE => Cursor::ResizeNW,
            CursorType::SOUTHRESIZE => Cursor::ResizeS,
            CursorType::SOUTHEASTRESIZE => Cursor::ResizeSE,
            CursorType::SOUTHWESTRESIZE => Cursor::ResizeSW,
            CursorType::WESTRESIZE => Cursor::ResizeW,
            CursorType::NORTHSOUTHRESIZE => Cursor::ResizeNS,
            CursorType::EASTWESTRESIZE => Cursor::ResizeEW,
            CursorType::NORTHEASTSOUTHWESTRESIZE => Cursor::ResizeNESW,
            CursorType::NORTHWESTSOUTHEASTRESIZE => Cursor::ResizeNWSE,
            CursorType::COLUMNRESIZE => Cursor::ResizeCol,
            CursorType::ROWRESIZE => Cursor::ResizeRow,
            CursorType::NONE => Cursor::Hidden,
            // POINTER, CUSTOM (bitmap cursors aren't supported yet) and anything new.
            _ => Cursor::Default,
        }
    }
}

thread_local! {
    static EVENTS: RefCell<Vec<WebEvent>> = const { RefCell::new(Vec::new()) };
}

fn emit(e: WebEvent) {
    EVENTS.with_borrow_mut(|q| q.push(e));
}

pub fn drain_events() -> Vec<WebEvent> {
    EVENTS.with_borrow_mut(std::mem::take)
}

thread_local! {
    /// Colour scheme pages should report (`None`: leave Chromium's own choice).
    static DARK: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static DEVTOOLS_ID: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

/// Makes pages report a light or dark `prefers-color-scheme`, following the
/// desktop theme. New pages take it when created; call `Page::apply_color_scheme`
/// on live pages after changing it. Windowless (Alloy-style) browsers ignore
/// the request context's Chrome colour scheme, so this goes through DevTools
/// media emulation per page, which also survives navigation.
pub fn set_color_scheme(dark: bool) {
    DARK.set(Some(dark));
}

type ZoomLookup = StdRc<dyn Fn(&str) -> f64>;

thread_local! {
    /// The user's page zoom for a URL (see [`set_page_zoom`]).
    static PAGE_ZOOM: RefCell<Option<ZoomLookup>> = const { RefCell::new(None) };
}

/// Sets how web pages find their user zoom (Ctrl +/−) from their URL: a factor,
/// 1.0 for none. Pages ask on every navigation; after changing what it answers,
/// call [`Page::refresh_zoom`] on live pages. Notes are never zoomed.
pub fn set_page_zoom(lookup: impl Fn(&str) -> f64 + 'static) {
    PAGE_ZOOM.with_borrow_mut(|z| *z = Some(StdRc::new(lookup)));
}

fn page_zoom(url: &str) -> f64 {
    let f = PAGE_ZOOM.with_borrow(|z| z.clone());
    f.map_or(1.0, |f| f(url)).clamp(0.05, 20.0)
}

// ---------------------------------------------------------------------------
// Process bootstrap
// ---------------------------------------------------------------------------

wrap_app! {
    struct TrailApp;

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefStringUtf16>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(cl) = command_line else { return };
            cl.append_switch(Some(&"no-startup-window".into()));
            cl.append_switch(Some(&"noerrdialogs".into()));
            cl.append_switch(Some(&"hide-crash-restore-bubble".into()));
            cl.append_switch(Some(&"use-mock-keychain".into()));
            cl.append_switch(Some(&"password-store=basic".into()));
            // Thin overlay scrollbars that show while a page scrolls (or the
            // pointer is near them) and fade when idle, instead of the classic
            // grey bar with arrow buttons on every card. They take no width
            // from the page. Pages that style their own scrollbars keep them.
            cl.append_switch_with_value(Some(&"enable-features".into()), Some(&"OverlayScrollbar".into()));
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(TrailProcess::new())
        }
    }
}

wrap_browser_process_handler! {
    struct TrailProcess;

    impl BrowserProcessHandler {
        fn on_already_running_app_relaunch(
            &self,
            command_line: Option<&mut CommandLine>,
            current_directory: Option<&CefString>,
        ) -> ::std::os::raw::c_int {
            let Some(cl) = command_line else { return 0 };
            let mut list = CefStringList::new();
            cl.arguments(Some(&mut list));
            let cwd = current_directory.map(|d| d.to_string()).unwrap_or_default();
            emit(WebEvent::Relaunch { args: list.into_iter().collect(), cwd });
            // Handled: we open the URLs ourselves instead of a Chrome window.
            1
        }
    }
}

/// Outcome of [`start`].
pub enum Startup {
    /// This process is a CEF helper (renderer, GPU, …) that has finished; exit with this code.
    Subprocess(i32),
    /// This is the browser process and CEF is initialized.
    Browser(Engine),
    /// Another Trailboard already runs on this profile; Chromium handed it our
    /// command line (it sees [`WebEvent::Relaunch`]). Exit successfully.
    Handed,
}

/// The CEF build this was compiled against, e.g.
/// `156.0.26+g8ea0c73+chromium-156.0.8078.88`. Needs no running CEF.
pub fn engine_version() -> &'static str {
    std::ffi::CStr::from_bytes_with_nul(sys::CEF_VERSION).ok().and_then(|c| c.to_str().ok()).unwrap_or("unknown")
}

/// Must run first in `main`. Handles CEF helper subprocesses (which re-execute
/// this same binary) and initializes CEF in the browser process.
pub fn start(cef_dir: &Path, cache_dir: &Path) -> Result<Startup, String> {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = Args::new();
    let cmd = args.as_cmd_line().ok_or("no command line")?;
    let type_switch = CefString::from("type");
    let is_browser = cmd.has_switch(Some(&type_switch)) != 1;

    let mut app = TrailApp::new();
    let code = execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut());
    if !is_browser {
        return Ok(Startup::Subprocess(code.max(0)));
    }
    if code != -1 {
        return Err(format!("execute_process returned {code} in the browser process"));
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(cache_dir).map_err(|e| e.to_string())?;
    // Chromium looks for `chrome-sandbox` beside the real executable (a
    // `/usr/bin` symlink resolves to `/usr/lib/trailboard`).
    let sandbox = sandbox::detect(exe.parent().unwrap_or(Path::new("/")));
    let settings = Settings {
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        no_sandbox: (!sandbox.is_on()) as i32,
        browser_subprocess_path: CefString::from(exe.to_string_lossy().as_ref()),
        resources_dir_path: CefString::from(cef_dir.to_string_lossy().as_ref()),
        locales_dir_path: CefString::from(cef_dir.join("locales").to_string_lossy().as_ref()),
        root_cache_path: CefString::from(cache_dir.to_string_lossy().as_ref()),
        cache_path: CefString::from(cache_dir.join("Default").to_string_lossy().as_ref()),
        persist_session_cookies: 1,
        log_severity: LogSeverity::WARNING,
        ..Default::default()
    };
    if initialize(Some(args.as_main_args()), Some(&settings), Some(&mut app), std::ptr::null_mut()) != 1 {
        if get_exit_code() == sys::cef_resultcode_t::CEF_RESULT_CODE_NORMAL_EXIT_PROCESS_NOTIFIED as i32 {
            return Ok(Startup::Handed);
        }
        return Err(format!("CEF initialize failed (exit code {})", get_exit_code()));
    }
    Ok(Startup::Browser(Engine { sandbox }))
}

/// Handle to an initialized CEF. Call [`Engine::shutdown`] after every page is dropped.
pub struct Engine {
    sandbox: sandbox::Sandbox,
}

impl Engine {
    /// How Chromium's child processes are confined.
    pub fn sandbox(&self) -> &sandbox::Sandbox {
        &self.sandbox
    }

    /// Run pending CEF work. Call often (every loop iteration).
    pub fn pump(&self) {
        do_message_loop_work();
    }

    pub fn shutdown(self) {
        // Let close requests settle before tearing down.
        for _ in 0..20 {
            do_message_loop_work();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        cef::shutdown();
    }
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

/// Latest painted frame of a page, BGRA8, tightly packed.
#[derive(Default)]
pub struct PaintFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Bumped on every paint so consumers can upload only when it changes.
    pub serial: u64,
    /// The view size in CSS px these pixels were laid out at. Right after a
    /// resize it is still the old size until Chromium paints again, so the
    /// app can show the old pixels at their own size instead of stretching
    /// them over the new one.
    pub css: (f32, f32),
}

/// The CSS size a `px` (width, height) paint was laid out at: the newest of
/// the `recent` view sizes (CSS px, raster scale), newest first, whose pixel
/// size it matches (±1 px for rounding); else `px` over the newest scale.
pub fn paint_css(px: (u32, u32), recent: &[((i32, i32), f32)]) -> (f32, f32) {
    let near = |a: f32, b: u32| (a.round() - b as f32).abs() <= 1.0;
    for &((w, h), s) in recent {
        if near(w as f32 * s, px.0) && near(h as f32 * s, px.1) {
            return (w as f32, h as f32);
        }
    }
    let s = recent.first().map_or(1.0, |r| r.1).max(0.01);
    (px.0 as f32 / s, px.1 as f32 / s)
}

struct Shared {
    key: u64,
    /// View size in CSS pixels.
    size: RefCell<(i32, i32)>,
    /// Raster scale: pixels per CSS pixel. CEF 156 OSR ignores the device scale
    /// factor we report, so we get it with page zoom instead: the view is
    /// `size × scale` pixels and the page zoom is `scale`, which keeps the CSS
    /// layout at `size` while painting `scale`× sharper. See [`zoom_level_for`].
    scale: RefCell<f32>,
    /// The last few (size, scale) pairs, newest first, to tell which one a
    /// paint belongs to ([`paint_css`]).
    recent: RefCell<Vec<((i32, i32), f32)>>,
    /// What the app shows: the page with any open popup drawn on top.
    frame: RefCell<PaintFrame>,
    /// While a popup is open, the page's own pixels (so the popup can move or close cleanly).
    view: RefCell<Option<PaintFrame>>,
    popup: RefCell<PopupState>,
    /// Text currently selected in the page (Chromium reports every change).
    selection: RefCell<String>,
    /// Bounds of the IME composition (or the caret), view px.
    ime_caret: RefCell<Option<IRect>>,
    /// URL the page was created with; the first navigation to it is never redirected beside.
    initial_url: String,
    /// The main frame's address, for its user zoom ([`set_page_zoom`]).
    url: RefCell<String>,
    /// The icon last asked for, so a page that repeats it isn't fetched again.
    icon_url: RefCell<String>,
    kind: PageKind,
    /// The main frame has committed a page (a link that is a file never does).
    committed: std::cell::Cell<bool>,
    /// The frames hiding elements by the filter lists' cosmetic rules.
    hide: cosmetic::Frames,
    /// A frame clicked a cookie banner's reject button ([`consent`]) since
    /// the main frame last started loading.
    banner_rejected: std::cell::Cell<bool>,
}

#[derive(Default)]
struct PopupState {
    shown: bool,
    /// Where Chromium placed the popup, in view pixels.
    rect: IRect,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// `EVENTFLAG_PRECISION_SCROLLING_DELTA` (cef_types.h): a wheel event with
/// exact pixel deltas, as from a touchpad.
const PRECISE_SCROLL: u32 = 1 << 14;

/// Chromium zoom level (factor = 1.2^level) that renders at `scale`.
pub fn zoom_level_for(scale: f32) -> f64 {
    (scale.max(0.01) as f64).ln() / 1.2f64.ln()
}

impl Shared {
    /// View size in pixels (what CEF lays out and paints).
    fn view_px(&self) -> (i32, i32) {
        let (w, h) = *self.size.borrow();
        let s = *self.scale.borrow();
        (((w as f32) * s).round().max(1.0) as i32, ((h as f32) * s).round().max(1.0) as i32)
    }

    /// The user's zoom for this page (1.0 for none; notes are never zoomed).
    fn user_zoom(&self) -> f64 {
        match self.kind {
            PageKind::Web => page_zoom(&self.url.borrow()),
            PageKind::Note => 1.0,
        }
    }

    /// Re-assert our page zoom; Chromium keeps zoom per origin, so a navigation can reset it.
    /// It is the raster scale times the user's zoom: the view stays `size × scale`
    /// pixels, so the page lays out `size / zoom` CSS px wide. Input and popup
    /// coordinates are view px / scale (card units) either way, so they need no change.
    fn apply_zoom(&self, host: &BrowserHost) {
        let want = zoom_level_for((*self.scale.borrow() as f64 * self.user_zoom()) as f32);
        if (host.zoom_level() - want).abs() > 1e-3 {
            log::debug!("page {} zoom level {:.3} -> {want:.3}", self.key, host.zoom_level());
            host.set_zoom_level(want);
        }
    }

    /// Where the popup is drawn (view px): Chromium's rect moved inside the card.
    fn popup_drawn(&self) -> Option<(IRect, IRect)> {
        let p = self.popup.borrow();
        if !p.shown || p.rect.w <= 0 || p.rect.h <= 0 {
            return None;
        }
        let (w, h) = self.view_px();
        Some((p.rect, popup::clamp_into_view(p.rect, w, h)))
    }

    /// Map a point in the card (CSS px) to the view pixel Chromium expects. A point over
    /// the drawn popup is shifted back to where Chromium thinks the popup is.
    fn widget_point(&self, x: i32, y: i32) -> (i32, i32) {
        let s = *self.scale.borrow();
        let (x, y) = ((x as f32 * s).round() as i32, (y as f32 * s).round() as i32);
        match self.popup_drawn() {
            Some((orig, drawn)) if drawn.contains(x, y) => (x - (drawn.x - orig.x), y - (drawn.y - orig.y)),
            _ => (x, y),
        }
    }

    /// Rebuild `frame` from the clean view plus the popup.
    fn compose(&self) {
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
        f.serial += 1;
    }
}

wrap_render_handler! {
    struct PageRender {
        shared: StdRc<Shared>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let (w, h) = self.shared.view_px();
                rect.x = 0;
                rect.y = 0;
                rect.width = w.max(1);
                rect.height = h.max(1);
            }
        }

        fn screen_info(&self, _browser: Option<&mut Browser>, info: Option<&mut ScreenInfo>) -> ::std::os::raw::c_int {
            match info {
                Some(info) => {
                    // Scale comes from page zoom (see `Shared::scale`), so the "screen" is 1×.
                    info.device_scale_factor = 1.0;
                    let (w, h) = self.shared.view_px();
                    let r = Rect { x: 0, y: 0, width: w, height: h };
                    info.rect = r.clone();
                    info.available_rect = r;
                    1
                }
                None => 0,
            }
        }

        fn on_text_selection_changed(
            &self,
            _browser: Option<&mut Browser>,
            selected_text: Option<&CefString>,
            _selected_range: Option<&Range>,
        ) {
            let text = selected_text.map(|t| t.to_string()).unwrap_or_default();
            log::trace!("page {} selection {} chars", self.shared.key, text.chars().count());
            *self.shared.selection.borrow_mut() = text;
        }

        fn on_ime_composition_range_changed(
            &self,
            _browser: Option<&mut Browser>,
            _selected_range: Option<&Range>,
            character_bounds: Option<&[Rect]>,
        ) {
            let rects: Vec<IRect> = character_bounds
                .unwrap_or_default()
                .iter()
                .map(|r| IRect { x: r.x, y: r.y, w: r.width, h: r.height })
                .collect();
            *self.shared.ime_caret.borrow_mut() = popup::bounding(&rects);
        }

        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            let sh = &self.shared;
            log::debug!("page {} popup show={show}", sh.key);
            if show != 0 {
                sh.popup.borrow_mut().shown = true;
                if sh.view.borrow().is_none() {
                    let f = sh.frame.borrow();
                    *sh.view.borrow_mut() =
                        Some(PaintFrame { width: f.width, height: f.height, pixels: f.pixels.clone(), serial: 0, css: f.css });
                }
            } else {
                *sh.popup.borrow_mut() = PopupState::default();
                // Restore the page pixels the popup covered.
                if let Some(view) = sh.view.borrow_mut().take() {
                    let mut f = sh.frame.borrow_mut();
                    f.width = view.width;
                    f.height = view.height;
                    f.css = view.css;
                    f.pixels = view.pixels;
                    f.serial += 1;
                }
            }
        }

        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&Rect>) {
            let Some(r) = rect else { return };
            log::debug!("page {} popup rect {},{} {}x{}", self.shared.key, r.x, r.y, r.width, r.height);
            self.shared.popup.borrow_mut().rect = IRect { x: r.x, y: r.y, w: r.width, h: r.height };
            self.shared.compose();
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let len = width as usize * height as usize * 4;
            // SAFETY: CEF guarantees `buffer` holds width*height BGRA pixels for this call.
            let src = unsafe { std::slice::from_raw_parts(buffer, len) };
            let sh = &self.shared;
            log::trace!("page {} paint {width}x{height} popup={}", sh.key, type_ == PaintElementType::POPUP);
            if type_ != PaintElementType::POPUP && sh.frame.borrow().width != width as u32 {
                log::debug!("page {} paints at {width}x{height} (scale {})", sh.key, sh.scale.borrow());
            }
            let css = paint_css((width as u32, height as u32), &sh.recent.borrow());
            let store = |f: &mut PaintFrame| {
                f.width = width as u32;
                f.height = height as u32;
                f.css = css;
                f.pixels.clear();
                f.pixels.extend_from_slice(src);
                f.serial += 1;
            };
            if type_ == PaintElementType::POPUP {
                let mut p = sh.popup.borrow_mut();
                p.width = width as u32;
                p.height = height as u32;
                p.pixels.clear();
                p.pixels.extend_from_slice(src);
            } else {
                let mut view = sh.view.borrow_mut();
                match view.as_mut() {
                    Some(v) => store(v),
                    // No popup: write straight into the shown frame.
                    None => return store(&mut sh.frame.borrow_mut()),
                }
            }
            sh.compose();
        }
    }
}

wrap_life_span_handler! {
    struct PageLife {
        shared: StdRc<Shared>,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            if let Some(url) = target_url.map(|u| u.to_string()).filter(|u| !u.is_empty()) {
                emit(WebEvent::OpenBeside { source: self.shared.key, url });
            }
            1 // cancel the native popup; the app opens a card instead
        }
    }
}

wrap_request_handler! {
    struct PageRequests {
        shared: StdRc<Shared>,
        spawn_beside: StdRc<RefCell<bool>>,
        // `resource_request_handler` runs on the IO thread: it may only touch
        // this (thread-safe) handler, never `shared`.
        guard: Option<ResourceRequestHandler>,
    }

    impl RequestHandler {
        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _is_navigation: ::std::os::raw::c_int,
            _is_download: ::std::os::raw::c_int,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut ::std::os::raw::c_int>,
        ) -> Option<ResourceRequestHandler> {
            self.guard.clone()
        }

        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: ::std::os::raw::c_int,
            is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // "Open links beside the page": a user-clicked link in the main frame
            // becomes a new card next to this one instead of replacing it.
            log::debug!(
                "before_browse page={} gesture={} redirect={} tt={:?} url={:?}",
                self.shared.key,
                user_gesture,
                is_redirect,
                request.as_ref().map(|r| r.transition_type().get_raw()),
                request.as_ref().map(|r| CefString::from(&r.url()).to_string().chars().take(200).collect::<String>())
            );
            if self.shared.kind == PageKind::Note {
                // A note stays a note: only its own page loads in it; a link
                // the user follows opens beside, anything else is dropped.
                let url = request.as_ref().map(|r| CefString::from(&r.url()).to_string()).unwrap_or_default();
                if url == self.shared.initial_url {
                    return 0;
                }
                if user_gesture != 0 && !url.is_empty() && !url.starts_with("javascript:") {
                    emit(WebEvent::OpenBeside { source: self.shared.key, url });
                }
                return 1;
            }
            if !*self.spawn_beside.borrow() || user_gesture == 0 || is_redirect != 0 {
                return 0;
            }
            let (Some(frame), Some(request)) = (frame, request) else { return 0 };
            if frame.is_main() == 0 {
                return 0;
            }
            let tt = request.transition_type();
            let source = tt.get_raw() & TransitionType::SOURCE_MASK.get_raw();
            if source != TransitionType::LINK.get_raw() {
                return 0;
            }
            let url = CefString::from(&request.url()).to_string();
            if url.is_empty() || url == self.shared.initial_url || url.starts_with("javascript:") {
                return 0;
            }
            emit(WebEvent::OpenBeside { source: self.shared.key, url });
            1
        }

        fn on_open_urlfrom_tab(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            target_url: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // Ctrl/middle-click: always a new card beside.
            if let Some(url) = target_url.map(|u| u.to_string()).filter(|u| !u.is_empty()) {
                emit(WebEvent::OpenBeside { source: self.shared.key, url });
            }
            1
        }
    }
}

wrap_display_handler! {
    struct PageDisplay {
        shared: StdRc<Shared>,
    }

    impl DisplayHandler {
        fn on_address_change(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if frame.is_some_and(|f| f.is_main() != 0)
                && self.shared.kind == PageKind::Web
                && let Some(url) = url {
                    *self.shared.url.borrow_mut() = url.to_string();
                    if let Some(h) = browser.and_then(|b| b.host()) {
                        self.shared.apply_zoom(&h);
                    }
                    emit(WebEvent::UrlChanged { page: self.shared.key, url: url.to_string() });
                }
        }

        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: ::std::os::raw::c_ulong,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            emit(WebEvent::CursorChanged { page: self.shared.key, cursor: Cursor::from_cef(type_) });
            1
        }

        fn on_favicon_urlchange(&self, browser: Option<&mut Browser>, icon_urls: Option<&mut CefStringList>) {
            if self.shared.kind != PageKind::Web {
                return;
            }
            // Chromium lists the page's icons best first, ending with /favicon.ico.
            let Some(first) = icon_urls.and_then(|l| {
                let raw: *mut cef::sys::_cef_string_list_t = l.into();
                CefStringList::from(raw).into_iter().next()
            }) else {
                return;
            };
            if *self.shared.icon_url.borrow() == first {
                return;
            }
            *self.shared.icon_url.borrow_mut() = first.clone();
            let Some(host) = browser.and_then(|b| b.host()) else { return };
            let page_url = self.shared.url.borrow().clone();
            let mut done = IconDone::new(self.shared.key, page_url);
            host.download_image(Some(&CefString::from(first.as_str())), 1, ICON_SIZE, 0, Some(&mut done));
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if self.shared.kind == PageKind::Web
                && let Some(title) = title {
                    emit(WebEvent::TitleChanged { page: self.shared.key, title: title.to_string() });
                }
        }

        fn on_console_message(
            &self,
            browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(message) = message.map(|m| m.to_string()) else { return 0 };
            if let Some(body) = message.strip_prefix(cosmetic::PREFIX) {
                if self.shared.kind == PageKind::Web
                    && let Some(browser) = browser
                {
                    cosmetic::answer(&self.shared.hide, browser, body);
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(cosmetic::COUNT_PREFIX) {
                self.shared.hide.on_count(body);
                return 1;
            }
            if let Some(body) = message.strip_prefix(consent::PREFIX) {
                if let Some((platform, what)) = consent::parse(body) {
                    let url = self.shared.url.borrow();
                    log::info!("consent: {platform} {what} on {}", url.chars().take(100).collect::<String>());
                    if what == "rejected" {
                        self.shared.banner_rejected.set(true);
                    }
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(links::PREFIX) {
                if let Some((width, links)) = links::parse(body) {
                    emit(WebEvent::Links { page: self.shared.key, width, links });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(text::PREFIX) {
                if self.shared.kind == PageKind::Web
                    && let Some((url, title, body)) = text::parse(body)
                    && *self.shared.url.borrow() == url
                {
                    emit(WebEvent::PageText {
                        page: self.shared.key,
                        url: url.to_string(),
                        title: title.to_string(),
                        text: body.to_string(),
                    });
                }
                return 1;
            }
            if self.shared.kind != PageKind::Note {
                return 0;
            }
            if let Some(text) = message.strip_prefix(NOTE_MESSAGE) {
                emit(WebEvent::NoteText { page: self.shared.key, text: text.to_string() });
                return 1; // not worth logging every keystroke
            }
            if let Some(url) = message.strip_prefix(NOTE_OPEN).filter(|u| !u.is_empty()) {
                emit(WebEvent::OpenBeside { source: self.shared.key, url: url.to_string() });
                return 1;
            }
            0
        }
    }
}

wrap_download_image_callback! {
    struct IconDone {
        key: u64,
        page_url: String,
    }

    impl DownloadImageCallback {
        fn on_download_image_finished(&self, image_url: Option<&CefString>, status: ::std::os::raw::c_int, image: Option<&mut Image>) {
            let url = image_url.map(|u| u.to_string()).unwrap_or_default();
            let Some(image) = image else {
                log::debug!("page {}: no icon at {url:?} (HTTP {status})", self.key);
                return;
            };
            let (mut w, mut h) = (0, 0);
            let Some(bin) = image.as_bitmap(1.0, ColorType::BGRA_8888, AlphaType::POSTMULTIPLIED, Some(&mut w), Some(&mut h)) else {
                return;
            };
            let (width, height) = (w.max(0) as u32, h.max(0) as u32);
            let mut pixels = vec![0u8; bin.size()];
            bin.data(Some(&mut pixels), 0);
            if width == 0 || height == 0 || pixels.len() != (width * height * 4) as usize {
                return;
            }
            log::debug!("page {}: icon {width}x{height} from {url:?}", self.key);
            emit(WebEvent::Favicon { page: self.key, page_url: self.page_url.clone(), width, height, pixels });
        }
    }
}

wrap_load_handler! {
    struct PageLoad {
        shared: StdRc<Shared>,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            browser: Option<&mut Browser>,
            is_loading: ::std::os::raw::c_int,
            _can_go_back: ::std::os::raw::c_int,
            _can_go_forward: ::std::os::raw::c_int,
        ) {
            if let Some(h) = browser.and_then(|b| b.host()) {
                self.shared.apply_zoom(&h);
            }
            emit(WebEvent::LoadingChanged { page: self.shared.key, loading: is_loading != 0 });
        }

        fn on_load_start(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, _transition_type: TransitionType) {
            let Some(frame) = frame else { return };
            if frame.is_main() != 0 {
                self.shared.committed.set(true);
                self.shared.banner_rejected.set(false);
            }
            if self.shared.kind == PageKind::Web
                && let Some(browser) = browser
            {
                cosmetic::on_load_start(&self.shared.hide, browser, frame);
            }
        }
    }
}

wrap_download_handler! {
    struct PageDownloads {
        shared: StdRc<Shared>,
    }

    impl DownloadHandler {
        fn can_download(&self, _browser: Option<&mut Browser>, _url: Option<&CefString>, _method: Option<&CefString>) -> ::std::os::raw::c_int {
            // Notes never download; web pages may (the app picks the folder).
            (self.shared.kind == PageKind::Web) as ::std::os::raw::c_int
        }

        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> ::std::os::raw::c_int {
            let (Some(item), Some(callback)) = (download_item, callback) else { return 0 };
            let id = item.id();
            let url = CefString::from(&item.url()).to_string();
            let suggested = suggested_name.map(|n| n.to_string()).unwrap_or_default();
            download::asked(id, callback.clone());
            let blank = !self.shared.committed.get();
            log::info!("page {}: download {id} of {url:?} as {suggested:?} (blank page: {blank})", self.shared.key);
            emit(WebEvent::DownloadAsked { page: self.shared.key, id, url, suggested, blank });
            1 // the app answers with download::save_to
        }

        fn on_download_updated(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            callback: Option<&mut DownloadItemCallback>,
        ) {
            let Some(item) = download_item else { return };
            if let Some(update) = download::updated(item, callback) {
                emit(update);
            }
        }
    }
}

wrap_context_menu_handler! {
    struct PageMenu {
        shared: StdRc<Shared>,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            // An empty model makes CEF skip its own menu; the app draws one instead.
            if let Some(m) = model {
                m.clear();
            }
            let Some(p) = params else { return };
            let edit = sys::cef_context_menu_edit_state_flags_t::from(p.edit_state_flags()).0;
            let has = |f: sys::cef_context_menu_edit_state_flags_t| edit & f.0 != 0;
            let non_empty = |s: String| Some(s).filter(|s| !s.is_empty());
            let image = if p.media_type() == ContextMenuMediaType::IMAGE {
                non_empty(CefString::from(&p.source_url()).to_string())
            } else {
                None
            };
            let context = menu::MenuContext {
                link: non_empty(CefString::from(&p.link_url()).to_string()),
                image,
                selection: CefString::from(&p.selection_text()).to_string(),
                editable: p.is_editable() != 0,
                can_undo: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_UNDO),
                can_redo: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_REDO),
                can_cut: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_CUT),
                can_copy: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_COPY),
                can_paste: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_PASTE),
                can_select_all: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_SELECT_ALL),
                can_back: browser.as_ref().is_some_and(|b| b.can_go_back() != 0),
                can_forward: browser.as_ref().is_some_and(|b| b.can_go_forward() != 0),
                note: self.shared.kind == PageKind::Note,
                sticky: false,
                blocking: None,
            };
            // Params are view px; the app works in CSS px.
            let s = *self.shared.scale.borrow();
            let (x, y) = (p.xcoord() as f32 / s, p.ycoord() as f32 / s);
            log::debug!("page {} context menu at {x:.0},{y:.0}: {context:?}", self.shared.key);
            emit(WebEvent::ContextMenu { page: self.shared.key, x, y, context });
        }
    }
}

wrap_find_handler! {
    struct PageFind {
        shared: StdRc<Shared>,
    }

    impl FindHandler {
        fn on_find_result(
            &self,
            _browser: Option<&mut Browser>,
            _identifier: ::std::os::raw::c_int,
            count: ::std::os::raw::c_int,
            _selection_rect: Option<&Rect>,
            active_match_ordinal: ::std::os::raw::c_int,
            final_update: ::std::os::raw::c_int,
        ) {
            emit(WebEvent::FindResult {
                page: self.shared.key,
                count,
                active: active_match_ordinal,
                done: final_update != 0,
            });
        }
    }
}

// One handler per CEF callback group makes `PageClient::new` take eight.
#[allow(clippy::too_many_arguments)]
mod client {
use super::*;
wrap_client! {
    pub(super) struct PageClient {
        render: RenderHandler,
        life: LifeSpanHandler,
        requests: RequestHandler,
        display: DisplayHandler,
        load: LoadHandler,
        menu: ContextMenuHandler,
        find: FindHandler,
        downloads: DownloadHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render.clone())
        }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life.clone())
        }
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(self.requests.clone())
        }
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }
        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load.clone())
        }
        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(self.menu.clone())
        }
        fn find_handler(&self) -> Option<FindHandler> {
            Some(self.find.clone())
        }
        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(self.downloads.clone())
        }
    }
}
}
use client::PageClient;

/// One live web page, rendered off screen.
pub struct Page {
    browser: Browser,
    shared: StdRc<Shared>,
    /// Requests [`shield`] stopped since the main frame last navigated (written on the IO thread).
    blocked: std::sync::Arc<shield::Blocked>,
}

/// Shared toggle for "open links beside the page" (Marble's default behaviour).
#[derive(Clone)]
pub struct SpawnBeside(StdRc<RefCell<bool>>);

impl SpawnBeside {
    pub fn new(on: bool) -> Self {
        Self(StdRc::new(RefCell::new(on)))
    }
    pub fn set(&self, on: bool) {
        *self.0.borrow_mut() = on;
    }
    pub fn get(&self) -> bool {
        *self.0.borrow()
    }
}

/// A cookie jar and cache of their own, kept in memory only, for the pages of
/// a private space: nothing they store reaches the profile on disk, and it is
/// gone when the last page using it and this handle are dropped.
#[derive(Clone)]
pub struct Jar(RequestContext);

impl Jar {
    pub fn in_memory() -> Option<Jar> {
        // An empty cache path makes an "incognito" context.
        let settings = RequestContextSettings { persist_session_cookies: 0, ..Default::default() };
        request_context_create_context(Some(&settings), None).map(Jar)
    }
}

impl Page {
    /// Creates a page identified by `key` (the app's card id) at `size` CSS
    /// pixels. With a `jar`, it keeps cookies and cache there instead of the profile.
    pub fn new(
        key: u64,
        url: &str,
        kind: PageKind,
        size: (i32, i32),
        scale: f32,
        spawn_beside: &SpawnBeside,
        jar: Option<&Jar>,
    ) -> Option<Page> {
        let shared = StdRc::new(Shared {
            key,
            size: RefCell::new(size),
            scale: RefCell::new(scale),
            recent: RefCell::new(vec![(size, scale)]),
            frame: RefCell::new(PaintFrame::default()),
            view: RefCell::new(None),
            popup: RefCell::new(PopupState::default()),
            selection: RefCell::new(String::new()),
            ime_caret: RefCell::new(None),
            initial_url: url.to_string(),
            url: RefCell::new(url.to_string()),
            icon_url: RefCell::new(String::new()),
            kind,
            committed: std::cell::Cell::new(false),
            hide: Default::default(),
            banner_rejected: std::cell::Cell::new(false),
        });
        let blocked = std::sync::Arc::new(shield::Blocked::default());
        let mut client = PageClient::new(
            PageRender::new(shared.clone()),
            PageLife::new(shared.clone()),
            PageRequests::new(shared.clone(), spawn_beside.0.clone(), (kind == PageKind::Web).then(|| shield::guard(blocked.clone()))),
            PageDisplay::new(shared.clone()),
            PageLoad::new(shared.clone()),
            PageMenu::new(shared.clone()),
            PageFind::new(shared.clone()),
            PageDownloads::new(shared.clone()),
        );
        let window_info = WindowInfo { windowless_rendering_enabled: 1, ..Default::default() };
        let browser_settings = BrowserSettings {
            windowless_frame_rate: 60,
            background_color: 0xffff_ffff,
            ..Default::default()
        };
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from(url)),
            Some(&browser_settings),
            None,
            jar.map(|j| j.0.clone()).as_mut(),
        )?;
        if let Some(h) = browser.host() {
            shared.apply_zoom(&h);
        }
        let page = Page { browser, shared, blocked };
        page.apply_color_scheme();
        Some(page)
    }

    /// Sends the colour scheme set with `set_color_scheme` to this page.
    pub fn apply_color_scheme(&self) {
        let (Some(dark), Some(host)) = (DARK.get(), self.browser.host()) else { return };
        let id = DEVTOOLS_ID.get() + 1;
        DEVTOOLS_ID.set(id);
        let scheme = if dark { "dark" } else { "light" };
        let msg = format!(
            r#"{{"id":{id},"method":"Emulation.setEmulatedMedia","params":{{"features":[{{"name":"prefers-color-scheme","value":"{scheme}"}}]}}}}"#
        );
        if host.send_dev_tools_message(Some(msg.as_bytes())) == 0 {
            log::warn!("page {}: couldn't set colour scheme", self.shared.key);
        }
    }

    /// How many addresses were blocked since the page last navigated.
    pub fn blocked(&self) -> u32 {
        self.blocked.count()
    }

    /// How many page elements the filter lists' cosmetic rules hide (ad
    /// slots, cookie banners), in all its frames, since it last navigated.
    pub fn hidden(&self) -> u32 {
        self.shared.hide.hidden()
    }

    /// Whether Trailboard clicked a cookie banner's reject button on this
    /// page (in any of its frames) since it last navigated.
    pub fn banner_rejected(&self) -> bool {
        self.shared.banner_rejected.get()
    }

    pub fn key(&self) -> u64 {
        self.shared.key
    }

    pub fn frame(&self) -> std::cell::Ref<'_, PaintFrame> {
        self.shared.frame.borrow()
    }

    fn host(&self) -> Option<BrowserHost> {
        self.browser.host()
    }

    /// Resize the page viewport (CSS px) and/or change its raster scale (pixels per CSS px).
    pub fn resize(&self, size: (i32, i32), scale: f32) {
        let changed_size = *self.shared.size.borrow() != size;
        let changed_scale = (*self.shared.scale.borrow() - scale).abs() > f32::EPSILON;
        *self.shared.size.borrow_mut() = size;
        *self.shared.scale.borrow_mut() = scale;
        if changed_size || changed_scale {
            let mut recent = self.shared.recent.borrow_mut();
            recent.insert(0, (size, scale));
            recent.truncate(4);
        }
        if let Some(h) = self.host() {
            if changed_scale {
                self.shared.apply_zoom(&h);
            }
            if changed_size || changed_scale {
                h.was_resized();
            }
        }
    }

    /// The page zoom Chromium has, without the raster scale: the user's zoom once applied.
    pub fn zoom_factor(&self) -> f64 {
        let level = self.host().map_or(0.0, |h| h.zoom_level());
        1.2f64.powf(level) / *self.shared.scale.borrow() as f64
    }

    /// Applies a changed user zoom ([`set_page_zoom`]); does nothing if it is unchanged.
    pub fn refresh_zoom(&self) {
        if let Some(h) = self.host() {
            self.shared.apply_zoom(&h);
        }
    }

    /// Finds `text` in the page (case-insensitive): a new search with
    /// `next == false`, otherwise the next (or previous) match of the same text.
    /// Results arrive as [`WebEvent::FindResult`].
    pub fn find(&self, text: &str, forward: bool, next: bool) {
        if let Some(h) = self.host() {
            if text.is_empty() {
                h.stop_finding(1);
            } else {
                h.find(Some(&CefString::from(text)), forward as _, 0, next as _);
            }
        }
    }

    /// Ends find in page, leaving the active match selected.
    pub fn stop_finding(&self) {
        if let Some(h) = self.host() {
            h.stop_finding(0);
        }
    }

    pub fn set_hidden(&self, hidden: bool) {
        if let Some(h) = self.host() {
            h.was_hidden(hidden as _);
        }
    }

    pub fn set_focus(&self, focus: bool) {
        if let Some(h) = self.host() {
            h.set_focus(focus as _);
        }
    }

    pub fn load(&self, url: &str) {
        if let Some(f) = self.browser.main_frame() {
            f.load_url(Some(&CefString::from(url)));
        }
    }

    /// Downloads `url` with this page's cookies (the menu's *Download link*).
    /// It arrives as [`WebEvent::DownloadAsked`] like any other download.
    pub fn download(&self, url: &str) {
        if let Some(h) = self.host() {
            h.start_download(Some(&CefString::from(url)));
        }
    }

    pub fn back(&self) {
        self.browser.go_back();
    }

    pub fn forward(&self) {
        self.browser.go_forward();
    }

    pub fn reload(&self) {
        self.browser.reload();
    }

    /// Reloads without any cache, so every request goes to the network
    /// again (and past [`shield`]); a plain reload takes scripts and images
    /// from Blink's memory cache without asking.
    pub fn reload_fresh(&self) {
        self.browser.reload_ignore_cache();
    }

    pub fn mouse_move(&self, x: i32, y: i32, modifiers: u32, leave: bool) {
        let (x, y) = self.shared.widget_point(x, y);
        if let Some(h) = self.host() {
            h.send_mouse_move_event(Some(&MouseEvent { x, y, modifiers }), leave as _);
        }
    }

    pub fn mouse_button(&self, x: i32, y: i32, modifiers: u32, button: MouseButton, up: bool, clicks: i32) {
        let ty = match button {
            MouseButton::Left => MouseButtonType::LEFT,
            MouseButton::Middle => MouseButtonType::MIDDLE,
            MouseButton::Right => MouseButtonType::RIGHT,
        };
        let (x, y) = self.shared.widget_point(x, y);
        if let Some(h) = self.host() {
            h.send_mouse_click_event(Some(&MouseEvent { x, y, modifiers }), ty, up as _, clicks);
        }
    }

    /// Scrolls the page at view px (`x`, `y`) by view px. `precise` marks
    /// touchpad-like movement, which Chromium applies as is; otherwise it
    /// animates the scroll as for a mouse wheel notch.
    pub fn mouse_wheel(&self, x: i32, y: i32, modifiers: u32, dx: i32, dy: i32, precise: bool) {
        let (x, y) = self.shared.widget_point(x, y);
        let modifiers = if precise { modifiers | PRECISE_SCROLL } else { modifiers };
        // Deltas are view pixels; scale them so a notch scrolls the same CSS distance.
        let s = *self.shared.scale.borrow();
        let (dx, dy) = ((dx as f32 * s).round() as i32, (dy as f32 * s).round() as i32);
        if let Some(h) = self.host() {
            h.send_mouse_wheel_event(Some(&MouseEvent { x, y, modifiers }), dx, dy);
        }
    }

    /// Text selected in the page right now (empty if none).
    pub fn selected_text(&self) -> String {
        self.shared.selection.borrow().clone()
    }

    /// Show `text` as the IME's in-progress composition. `cursor` is the caret or
    /// selection inside it as byte offsets, as winit reports them.
    pub fn ime_set_composition(&self, text: &str, cursor: Option<(usize, usize)>) {
        let Some(h) = self.host() else { return };
        let len = utf16_len(text);
        let underline = CompositionUnderline {
            size: std::mem::size_of::<sys::cef_composition_underline_t>(),
            range: Range { from: 0, to: len },
            color: 0xff00_0000,
            background_color: 0,
            thick: 0,
            ..Default::default()
        };
        let sel = match cursor {
            Some((a, b)) => Range { from: utf16_len(&text[..a.min(text.len())]), to: utf16_len(&text[..b.min(text.len())]) },
            None => Range { from: len, to: len },
        };
        h.ime_set_composition(Some(&CefString::from(text)), Some(&[underline]), Some(&INVALID_RANGE), Some(&sel));
    }

    /// Insert the IME's final text, replacing any composition.
    pub fn ime_commit(&self, text: &str) {
        if let Some(h) = self.host() {
            h.ime_commit_text(Some(&CefString::from(text)), Some(&INVALID_RANGE), 0);
        }
    }

    pub fn ime_cancel(&self) {
        if let Some(h) = self.host() {
            h.ime_cancel_composition();
        }
    }

    /// Where the IME composition or caret is, in CSS px within the page.
    pub fn ime_caret(&self) -> Option<(f32, f32, f32, f32)> {
        let r = (*self.shared.ime_caret.borrow())?;
        let s = *self.shared.scale.borrow();
        Some((r.x as f32 / s, r.y as f32 / s, r.w as f32 / s, r.h as f32 / s))
    }

    fn edit_frame(&self) -> Option<Frame> {
        self.browser.focused_frame().or_else(|| self.browser.main_frame())
    }

    pub fn edit(&self, cmd: EditCommand) {
        let Some(f) = self.edit_frame() else { return };
        match cmd {
            EditCommand::Undo => f.undo(),
            EditCommand::Redo => f.redo(),
            EditCommand::Cut => f.cut(),
            EditCommand::Copy => f.copy(),
            EditCommand::Paste => f.paste(),
            EditCommand::SelectAll => f.select_all(),
        }
    }

    /// Paste `text` (from the system clipboard) into the focused field: the page
    /// gets a `paste` event first and can handle it; otherwise the text is inserted.
    pub fn paste_text(&self, text: &str) {
        let Some(f) = self.edit_frame() else { return };
        let script = format!(
            "(function(t){{var e=document.activeElement||document.body;var d=new DataTransfer();\
             d.setData('text/plain',t);var ev=new ClipboardEvent('paste',{{clipboardData:d,bubbles:true,\
             cancelable:true,composed:true}});if(e.dispatchEvent(ev))document.execCommand('insertText',false,t);}})({})",
            js_string(text)
        );
        f.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
    }

    /// Runs `script` in the page's main frame.
    pub fn run_script(&self, script: &str) {
        if let Some(f) = self.browser.main_frame() {
            f.execute_java_script(Some(&CefString::from(script)), None, 0);
        }
    }

    /// Ask for the links visible in the page; they arrive as [`WebEvent::Links`].
    pub fn request_links(&self) {
        self.run_script(links::SCRIPT);
    }

    /// Ask for the page's address, title and text; they arrive as [`WebEvent::PageText`].
    pub fn request_text(&self) {
        if self.shared.kind == PageKind::Web {
            self.run_script(text::SCRIPT);
        }
    }

    pub fn kind(&self) -> PageKind {
        self.shared.kind
    }

    pub fn send_key(&self, ev: &keys::Key) {
        if let Some(h) = self.host() {
            for e in ev.to_cef() {
                h.send_key_event(Some(&e));
            }
        }
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        if let Some(h) = self.host() {
            h.close_browser(1);
        }
    }
}

/// CEF's "no range" value.
const INVALID_RANGE: Range = Range { from: u32::MAX, to: u32::MAX };

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// `s` as a JavaScript string literal.
/// `s` as a JavaScript string literal, safe inside a `<script>` element.
pub fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Line terminators that end a JS string, and anything that could close a <script>.
            '\u{2028}' | '\u{2029}' | '<' | '>' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// CEF `EVENTFLAG_*` bits.
pub mod flags {
    pub const SHIFT: u32 = 1 << 1;
    pub const CONTROL: u32 = 1 << 2;
    pub const ALT: u32 = 1 << 3;
    pub const LEFT_MOUSE: u32 = 1 << 4;
    pub const MIDDLE_MOUSE: u32 = 1 << 5;
    pub const RIGHT_MOUSE: u32 = 1 << 6;
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_paint_is_matched_to_the_view_size_it_was_laid_out_at() {
        use super::paint_css;
        // Card 1024×720 at 0.5, then read at 1440×866 at 2.
        let recent = [((1440, 866), 2.0), ((1024, 720), 0.5)];
        assert_eq!(paint_css((2880, 1732), &recent), (1440.0, 866.0));
        // The old paint, still showing after the resize.
        assert_eq!(paint_css((512, 360), &recent), (1024.0, 720.0));
        // Rounding is tolerated.
        assert_eq!(paint_css((513, 360), &recent), (1024.0, 720.0));
        // Unknown: the pixels over the newest scale.
        assert_eq!(paint_css((800, 600), &recent), (400.0, 300.0));
        assert_eq!(paint_css((10, 10), &[]), (10.0, 10.0));
    }

    use super::{js_string, utf16_len, zoom_level_for};

    #[test]
    fn js_strings_are_escaped() {
        assert_eq!(js_string("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
        assert_eq!(js_string("</script>\u{2028}"), r#""\u003c/script\u003e\u2028""#);
        assert_eq!(js_string("\u{1}é😀"), "\"\\u0001é😀\"");
    }

    #[test]
    fn utf16_lengths() {
        assert_eq!(utf16_len("aé"), 2);
        assert_eq!(utf16_len("😀"), 2);
    }

    #[test]
    fn zoom_level_matches_chromium_factor() {
        for s in [0.5f32, 1.0, 1.25, 2.0] {
            let factor = 1.2f64.powf(zoom_level_for(s));
            assert!((factor - s as f64).abs() < 1e-9, "{s}: {factor}");
        }
        assert_eq!(zoom_level_for(1.0), 0.0);
    }
}
