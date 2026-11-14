//! Chromium (CEF) in off-screen-rendering mode. Each card owns one windowless
//! browser whose frames arrive as BGRA pixel buffers; the app uploads them to
//! GPU textures. All CEF callbacks run on the main thread because we drive CEF
//! with `external_message_pump` + [`pump`], so plain `Rc`/`RefCell` are enough.

pub mod keys;

use cef::{args::Args, *};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc as StdRc;

/// Something a page asked the app to do. Drained once per frame with [`drain_events`].
#[derive(Debug, Clone)]
pub enum WebEvent {
    /// A link (or `window.open`) on page `source` should open as a new card beside it.
    OpenBeside { source: u64, url: String },
    TitleChanged { page: u64, title: String },
    UrlChanged { page: u64, url: String },
    LoadingChanged { page: u64, loading: bool },
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
    scale: RefCell<f32>,
    frame: RefCell<PaintFrame>,
    /// URL the page was created with; the first navigation to it is never redirected beside.
    initial_url: String,
}

wrap_render_handler! {
    struct PageRender {
        shared: StdRc<Shared>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let (w, h) = *self.shared.size.borrow();
                rect.x = 0;
                rect.y = 0;
                rect.width = w.max(1);
                rect.height = h.max(1);
            }
        }

        fn screen_info(&self, _browser: Option<&mut Browser>, info: Option<&mut ScreenInfo>) -> ::std::os::raw::c_int {
            match info {
                Some(info) => {
                    info.device_scale_factor = *self.shared.scale.borrow();
                    1
                }
                None => 0,
            }
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
            // Popup widgets (e.g. <select> dropdowns) arrive as PET_POPUP; not composited yet.
            if type_ != PaintElementType::default() || buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let len = width as usize * height as usize * 4;
            // SAFETY: CEF guarantees `buffer` holds width*height BGRA pixels for this call.
            let src = unsafe { std::slice::from_raw_parts(buffer, len) };
            let mut f = self.shared.frame.borrow_mut();
            f.width = width as u32;
            f.height = height as u32;
            f.pixels.clear();
            f.pixels.extend_from_slice(src);
            f.serial += 1;
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
                request.as_ref().map(|r| CefString::from(&r.url()).to_string())
            );
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
        fn on_address_change(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if frame.is_some_and(|f| f.is_main() != 0)
                && let Some(url) = url {
                    emit(WebEvent::UrlChanged { page: self.shared.key, url: url.to_string() });
                }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(title) = title {
                emit(WebEvent::TitleChanged { page: self.shared.key, title: title.to_string() });
            }
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
            _browser: Option<&mut Browser>,
            is_loading: ::std::os::raw::c_int,
            _can_go_back: ::std::os::raw::c_int,
            _can_go_forward: ::std::os::raw::c_int,
        ) {
            emit(WebEvent::LoadingChanged { page: self.shared.key, loading: is_loading != 0 });
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
    pub fn new(key: u64, url: &str, size: (i32, i32), scale: f32, spawn_beside: &SpawnBeside) -> Option<Page> {
        let shared = StdRc::new(Shared {
            key,
            size: RefCell::new(size),
            scale: RefCell::new(scale),
            frame: RefCell::new(PaintFrame::default()),
            initial_url: url.to_string(),
        });
        let mut client = PageClient::new(
            PageRender::new(shared.clone()),
            PageLife::new(shared.clone()),
            PageRequests::new(shared.clone(), spawn_beside.0.clone()),
            PageDisplay::new(shared.clone()),
            PageLoad::new(shared.clone()),
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
        Some(Page { browser, shared })
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

    /// Resize the page viewport (CSS px) and/or change its device scale.
    pub fn resize(&self, size: (i32, i32), scale: f32) {
        let changed_size = *self.shared.size.borrow() != size;
        let changed_scale = (*self.shared.scale.borrow() - scale).abs() > f32::EPSILON;
        *self.shared.size.borrow_mut() = size;
        *self.shared.scale.borrow_mut() = scale;
        if let Some(h) = self.host() {
            if changed_scale {
                h.notify_screen_info_changed();
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
        if let Some(h) = self.host() {
            h.send_mouse_click_event(Some(&MouseEvent { x, y, modifiers }), ty, up as _, clicks);
        }
    }

    pub fn mouse_wheel(&self, x: i32, y: i32, modifiers: u32, dx: i32, dy: i32) {
        if let Some(h) = self.host() {
            h.send_mouse_wheel_event(Some(&MouseEvent { x, y, modifiers }), dx, dy);
        }
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
