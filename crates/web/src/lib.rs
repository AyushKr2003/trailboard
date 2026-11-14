//! Chromium (CEF) in off-screen-rendering mode. Each card owns one windowless
//! browser whose frames arrive as BGRA pixel buffers; the app uploads them to
//! GPU textures. All CEF callbacks run on the main thread because we drive CEF
//! with `external_message_pump` + [`pump`], so plain `Rc`/`RefCell` are enough.

pub mod keys;
pub mod links;
pub mod menu;
pub mod popup;

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
}

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
        }
    }
}

/// Outcome of [`start`].
pub enum Startup {
    /// This process is a CEF helper (renderer, GPU, …) that has finished; exit with this code.
    Subprocess(i32),
    /// This is the browser process and CEF is initialized.
    Browser(Engine),
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
    let settings = Settings {
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        no_sandbox: 1,
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
        return Err("CEF initialize failed".into());
    }
    Ok(Startup::Browser(Engine { _private: () }))
}

/// Handle to an initialized CEF. Call [`Engine::shutdown`] after every page is dropped.
pub struct Engine {
    _private: (),
}

impl Engine {
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
    kind: PageKind,
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

    /// Re-assert our page zoom; Chromium keeps zoom per origin, so a navigation can reset it.
    fn apply_zoom(&self, host: &BrowserHost) {
        let want = zoom_level_for(*self.scale.borrow());
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
                        Some(PaintFrame { width: f.width, height: f.height, pixels: f.pixels.clone(), serial: 0 });
                }
            } else {
                *sh.popup.borrow_mut() = PopupState::default();
                // Restore the page pixels the popup covered.
                if let Some(view) = sh.view.borrow_mut().take() {
                    let mut f = sh.frame.borrow_mut();
                    f.width = view.width;
                    f.height = view.height;
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
            let store = |f: &mut PaintFrame| {
                f.width = width as u32;
                f.height = height as u32;
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
    }

    impl RequestHandler {
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

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if self.shared.kind == PageKind::Web
                && let Some(title) = title {
                    emit(WebEvent::TitleChanged { page: self.shared.key, title: title.to_string() });
                }
        }

        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(message) = message.map(|m| m.to_string()) else { return 0 };
            if let Some(body) = message.strip_prefix(links::PREFIX) {
                if let Some((width, links)) = links::parse(body) {
                    emit(WebEvent::Links { page: self.shared.key, width, links });
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
            };
            // Params are view px; the app works in CSS px.
            let s = *self.shared.scale.borrow();
            let (x, y) = (p.xcoord() as f32 / s, p.ycoord() as f32 / s);
            log::debug!("page {} context menu at {x:.0},{y:.0}: {context:?}", self.shared.key);
            emit(WebEvent::ContextMenu { page: self.shared.key, x, y, context });
        }
    }
}

wrap_client! {
    struct PageClient {
        render: RenderHandler,
        life: LifeSpanHandler,
        requests: RequestHandler,
        display: DisplayHandler,
        load: LoadHandler,
        menu: ContextMenuHandler,
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
    }
}

/// One live web page, rendered off screen.
pub struct Page {
    browser: Browser,
    shared: StdRc<Shared>,
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

impl Page {
    /// Creates a page identified by `key` (the app's card id) at `size` CSS pixels.
    pub fn new(key: u64, url: &str, kind: PageKind, size: (i32, i32), scale: f32, spawn_beside: &SpawnBeside) -> Option<Page> {
        let shared = StdRc::new(Shared {
            key,
            size: RefCell::new(size),
            scale: RefCell::new(scale),
            frame: RefCell::new(PaintFrame::default()),
            view: RefCell::new(None),
            popup: RefCell::new(PopupState::default()),
            selection: RefCell::new(String::new()),
            ime_caret: RefCell::new(None),
            initial_url: url.to_string(),
            kind,
        });
        let mut client = PageClient::new(
            PageRender::new(shared.clone()),
            PageLife::new(shared.clone()),
            PageRequests::new(shared.clone(), spawn_beside.0.clone()),
            PageDisplay::new(shared.clone()),
            PageLoad::new(shared.clone()),
            PageMenu::new(shared.clone()),
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
            None,
        )?;
        if let Some(h) = browser.host() {
            shared.apply_zoom(&h);
        }
        let page = Page { browser, shared };
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
        if let Some(h) = self.host() {
            if changed_scale {
                self.shared.apply_zoom(&h);
            }
            if changed_size || changed_scale {
                h.was_resized();
            }
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

    pub fn back(&self) {
        self.browser.go_back();
    }

    pub fn forward(&self) {
        self.browser.go_forward();
    }

    pub fn reload(&self) {
        self.browser.reload();
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

    pub fn mouse_wheel(&self, x: i32, y: i32, modifiers: u32, dx: i32, dy: i32) {
        let (x, y) = self.shared.widget_point(x, y);
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
