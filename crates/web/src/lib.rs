//! Chromium (CEF) in off-screen-rendering mode. Each card owns one windowless
//! browser whose frames arrive as BGRA pixel buffers; the app uploads them to
//! GPU textures. All CEF callbacks run on the main thread because we drive CEF
//! with `external_message_pump` + [`pump`], so plain `Rc`/`RefCell` are enough.
//! The one exception is request blocking ([`shield`]), asked on Chromium's IO thread.

pub mod caret;
pub mod consent;
pub mod cosmetic;
pub mod dialog;
pub mod download;
pub mod field;
pub mod focus;
pub mod inspector;
pub mod keys;
pub mod links;
pub mod media;
pub mod menu;
pub mod pager;
pub mod permission;
pub mod popup;
pub mod prefs;
pub mod render_node;
pub(crate) mod resources;
pub mod sandbox;
pub mod scroll;
pub mod shield;
pub mod text;
pub mod youtube;

use cef::{args::Args, *};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc as StdRc;
use std::sync::atomic::{AtomicU64, Ordering};

use popup::IRect;

/// Something a page asked the app to do. Drained once per frame with [`drain_events`].
#[derive(Debug, Clone)]
pub enum WebEvent {
    /// A link (or `window.open`) on page `source` should open as a new card beside it.
    OpenBeside { source: u64, url: String },
    TitleChanged { page: u64, title: String },
    UrlChanged { page: u64, url: String },
    /// `can_back` / `can_forward`: the page has somewhere to go back or forward to.
    LoadingChanged { page: u64, loading: bool, can_back: bool, can_forward: bool },
    /// The mouse cursor the page wants while the pointer is over it.
    CursorChanged { page: u64, cursor: Cursor },
    /// The user right-clicked at (`x`, `y`) CSS px in the page; the app shows its own menu.
    ContextMenu { page: u64, x: f32, y: f32, context: menu::MenuContext },
    /// The text of note page `page` changed (sent by the note editor, see [`PageKind::Note`]).
    NoteText { page: u64, text: String },
    /// One of the app's own pages ([`PageKind::App`]) asked for one of its
    /// commands by name (a button on the welcome page). The app decides what
    /// names mean; web pages can never send this.
    AppAction { page: u64, action: String },
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
    /// The site at `origin` on page `page` wants `kinds` and the settings say
    /// to ask. Answer with [`permission::answer`].
    PermissionAsked { page: u64, id: u64, origin: String, kinds: Vec<permission::Kind> },
    /// Chromium withdrew permission request `id` (the page navigated or closed).
    PermissionWithdrawn { id: u64 },
    /// Page `page` shows `alert`, `confirm`, `prompt` or asks before it is
    /// left. Answer with [`dialog::answer`]. `default`: a prompt's text.
    DialogAsked { page: u64, id: u64, kind: dialog::Kind, origin: String, message: String, default: String },
    /// Page `page` opened a file chooser. Answer with [`dialog::choose`].
    /// `default`: a path or name Chromium suggests; `accept`: the types the
    /// page wants (`.pdf`, `image/*`).
    FilesAsked { page: u64, id: u64, mode: dialog::FileMode, title: String, default: String, accept: Vec<String> },
    /// Chromium dropped dialog `id` itself (the page navigated or closed).
    DialogWithdrawn { id: u64 },
    /// An answer about a text field from [`Page::read_field`] or
    /// [`Page::write_field`]. Any page can send this; only act on a token you gave.
    Field { page: u64, report: field::Report },
    /// Where page `page`'s keyboard focus went (see [`focus`]). Any page can
    /// send this about itself; the app only acts on the card it is using.
    Focus { page: u64, report: focus::Report },
    /// An answer about page `page`'s scroll sections (see [`scroll`]),
    /// to one of [`Page::scroll_command`]. Any page can send this.
    Scroll { page: u64, report: scroll::Report },
    /// Page `page`'s next / previous page candidates, answering
    /// [`Page::request_pager`]. Any page can send this; only expect it after asking.
    Pager { page: u64, links: Vec<pager::Link> },
    /// An answer from Caret / Visual mode in page `page` (see [`caret`]).
    /// Any page can send this; only take a copy after asking for one.
    Caret { page: u64, report: caret::Report },
    /// Web page `page` started or stopped playing media (see [`media`]):
    /// a video, muted or not, or audible audio, in any of its frames. A page
    /// can claim this about itself only.
    Media { page: u64, playing: bool },
    /// Web page `page`'s answer to [`Page::media_control`]: a line
    /// `scene::media::Status` reads. Only expect it after asking.
    MediaDone { page: u64, answer: String },
    /// A video in web page `page` can't play: `format` or `drm` (see
    /// [`media`]). The page could claim this falsely; it only shows a message.
    MediaTrouble { page: u64, kind: String },
    /// Page `page` went into full screen (a video's full screen button, or
    /// `media full`) or out of it. The page fills its view either way; the
    /// app decides how big that is.
    Fullscreen { page: u64, on: bool },
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
    /// One of the app's own pages (the welcome page). Like a note it never
    /// navigates away (links open beside) and its title and address are not
    /// reported; it has no text to send, but may ask for commands with a
    /// console message starting with [`APP_ACTION`] ([`WebEvent::AppAction`]).
    App,
    /// Chromium's page inspector for another page ([`Page::open_inspector`]).
    /// Its title and address are not reported (the address is the inspector's
    /// own), it is never zoomed, blocked or filtered, and it downloads nothing.
    Inspector,
}

/// Prefix of the console message an app page sends to run a command.
pub const APP_ACTION: &str = "\u{1}trailboard-do\u{1}";

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
    note_activity();
}

// ---------------------------------------------------------------------------
// When to pump
// ---------------------------------------------------------------------------

/// The longest CEF goes unpumped even when it asked for nothing (CEF's own
/// external-pump sample does the same: some work is never scheduled).
pub const MAX_PUMP_DELAY: std::time::Duration = std::time::Duration::from_millis(33);

/// When CEF next wants [`Engine::pump`], in nanoseconds since [`epoch`];
/// `u64::MAX` when it asked for nothing. Written from any thread.
static PUMP_DUE: AtomicU64 = AtomicU64::new(0);
/// Bumped by every event and every paint, so the app can tell a pump that
/// changed something from one that didn't.
static ACTIVITY: AtomicU64 = AtomicU64::new(0);
static WAKER: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

fn epoch() -> std::time::Instant {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(std::time::Instant::now)
}

fn nanos_at(t: std::time::Instant) -> u64 {
    t.saturating_duration_since(epoch()).as_nanos().min(u64::MAX as u128 - 1) as u64
}

/// Gives `web` a way to wake the app's event loop from any thread (CEF asks
/// for pump work from its own threads). Set once, before CEF starts.
pub fn set_waker(wake: Box<dyn Fn() + Send + Sync>) {
    let _ = WAKER.set(wake);
}

/// Wakes the app's event loop (from any thread): call after handing the
/// main thread something through a channel.
pub fn wake() {
    if let Some(w) = WAKER.get() {
        w();
    }
}

/// CEF wants pumping in `delay_ms` (0 or less: now). Any thread.
fn schedule_pump(delay_ms: i64) {
    let due = std::time::Instant::now() + std::time::Duration::from_millis(delay_ms.max(0) as u64);
    PUMP_DUE.fetch_min(nanos_at(due), Ordering::AcqRel);
    wake();
}

/// Page paints received from CEF, and the bytes `on_paint` copied for them,
/// since the start (for `TRAILBOARD_PERF`).
static PAINTS: AtomicU64 = AtomicU64::new(0);
static PAINT_BYTES: AtomicU64 = AtomicU64::new(0);

/// Paints CEF delivered and bytes copied out of its buffers, since the start.
pub fn paint_stats() -> (u64, u64) {
    (PAINTS.load(Ordering::Relaxed), PAINT_BYTES.load(Ordering::Relaxed))
}

fn note_activity() {
    ACTIVITY.fetch_add(1, Ordering::Relaxed);
}

/// A count that grows whenever CEF sent an event or a page painted.
pub fn activity() -> u64 {
    ACTIVITY.load(Ordering::Relaxed)
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

type ScriptLookup = StdRc<dyn Fn(&str) -> bool>;

thread_local! {
    /// Whether a page at a URL runs JavaScript (see [`set_javascript`]).
    static JAVASCRIPT: RefCell<Option<ScriptLookup>> = const { RefCell::new(None) };
}

/// Sets how web pages find whether they run JavaScript, from their URL.
/// Pages ask before each navigation of their main frame; after changing
/// what it answers, call [`Page::refresh_javascript`] on live pages.
pub fn set_javascript(lookup: impl Fn(&str) -> bool + 'static) {
    JAVASCRIPT.with_borrow_mut(|j| *j = Some(StdRc::new(lookup)));
}

fn javascript_on(url: &str) -> bool {
    let lookup = JAVASCRIPT.with_borrow(|j| j.clone());
    lookup.is_none_or(|f| f(url))
}

/// Sends one DevTools protocol command to a page (its answer is ignored).
fn send_dev_tools(host: &BrowserHost, method: &str, params: &str) -> bool {
    send_dev_tools_id(host, method, params).is_some()
}

/// A DevTools message id no other message of ours uses.
pub(crate) fn next_devtools_id() -> i32 {
    let id = DEVTOOLS_ID.get() + 1;
    DEVTOOLS_ID.set(id);
    id
}

/// [`send_dev_tools`], giving the message's id (its result carries it).
pub(crate) fn send_dev_tools_id(host: &BrowserHost, method: &str, params: &str) -> Option<i32> {
    let id = next_devtools_id();
    let msg = format!(r#"{{"id":{id},"method":"{method}","params":{params}}}"#);
    (host.send_dev_tools_message(Some(msg.as_bytes())) != 0).then_some(id)
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
            process_type: Option<&CefStringUtf16>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(cl) = command_line else { return };
            let browser = process_type.is_none_or(|t| t.to_string().is_empty());
            // Video decoding on the iGPU when NVIDIA's driver holds the first
            // render node (`render_node`); the GPU process inherits it.
            let node_switch = CefString::from("render-node-override");
            if browser && cl.has_switch(Some(&node_switch)) != 1 {
                let nodes = render_node::render_nodes(Path::new("/sys/class/drm"));
                if let Some(node) = render_node::override_for(&nodes) {
                    cl.append_switch_with_value(Some(&node_switch), Some(&CefString::from(node.as_str())));
                    let _ = RENDER_NODE.set(node);
                }
            }
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
            // A debug knob for trying engine switches without a rebuild
            // (e.g. `--disable-gpu-compositing`); child processes get the
            // ones they need from the browser process.
            if browser {
                let extra = std::env::var("TRAILBOARD_CEF_ARGS").unwrap_or_default();
                let mut added = Vec::new();
                for (name, value) in extra_switches(&extra) {
                    match &value {
                        Some(v) => cl.append_switch_with_value(Some(&name.as_str().into()), Some(&v.as_str().into())),
                        None => cl.append_switch(Some(&name.as_str().into())),
                    }
                    added.push(format!("--{name}{}", value.map(|v| format!("={v}")).unwrap_or_default()));
                }
                if !added.is_empty() {
                    let _ = EXTRA_ARGS.set(added.join(" "));
                }
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(TrailProcess::new())
        }
    }
}

wrap_browser_process_handler! {
    struct TrailProcess;

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            log::info!("chromium context ready");
            prefs::context_ready();
        }

        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            schedule_pump(delay_ms);
        }

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

static RENDER_NODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static EXTRA_ARGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The switches `TRAILBOARD_CEF_ARGS` added, if any; for the start log.
pub fn extra_cef_args() -> Option<&'static str> {
    EXTRA_ARGS.get().map(String::as_str)
}

/// The render node Trailboard pointed Chromium's video decoding at, when it
/// did (see `render_node`); for the start log.
pub fn render_node_override() -> Option<&'static str> {
    RENDER_NODE.get().map(String::as_str)
}

/// A CEF version as (major, minor, patch), e.g. (156, 0, 26).
pub type EngineVersion = (i32, i32, i32);

/// The CEF version this was compiled against (the one `install-cef.sh` pins).
pub fn built_engine_version() -> EngineVersion {
    (sys::CEF_VERSION_MAJOR, sys::CEF_VERSION_MINOR, sys::CEF_VERSION_PATCH)
}

// `libcef.so` exports this; the `cef` crate doesn't wrap it.
unsafe extern "C" {
    fn cef_version_info(entry: std::os::raw::c_int) -> std::os::raw::c_int;
}

/// The version of the `libcef.so` actually loaded, which can differ from
/// [`built_engine_version`] when the installed engine wasn't updated with
/// the binary. Asks the library itself, so it needs no running CEF.
pub fn loaded_engine_version() -> EngineVersion {
    // SAFETY: a plain query of constants compiled into libcef; entries 0–2
    // are the CEF major, minor and patch numbers.
    unsafe { (cef_version_info(0), cef_version_info(1), cef_version_info(2)) }
}

/// Must run first in `main`. Handles CEF helper subprocesses (which re-execute
/// this same binary) and initializes CEF in the browser process, where
/// `accept_language` gives the languages pages ask for (`hi-IN,hi,en`; empty
/// for Chromium's own), which Chromium's network stack reads only now.
pub fn start(cef_dir: &Path, cache_dir: &Path, accept_language: impl FnOnce() -> String) -> Result<Startup, String> {
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
    // A running Trailboard owns the profile (this one will hand over to it).
    if !profile_in_use(cache_dir) {
        prefs::forget_saved_zooms(&cache_dir.join("Default"));
    }
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
        accept_language_list: CefString::from(accept_language().as_str()),
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

/// Whether a live Chromium holds the profile in `cache_dir`: its
/// `SingletonLock` is a link to `host-pid`, and that process exists.
fn profile_in_use(cache_dir: &Path) -> bool {
    let Ok(target) = std::fs::read_link(cache_dir.join("SingletonLock")) else { return false };
    let target = target.to_string_lossy();
    let pid = target.rsplit('-').next().and_then(|p| p.parse::<u32>().ok());
    pid.is_none_or(|pid| Path::new(&format!("/proc/{pid}")).exists())
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

    /// Run pending CEF work. Call when [`Engine::next_due`] has come, and
    /// after input was sent to a page.
    pub fn pump(&self) {
        // Work done now may ask for more (it lowers this again); otherwise
        // the next pump is the fallback one.
        PUMP_DUE.store(nanos_at(std::time::Instant::now() + MAX_PUMP_DELAY), Ordering::Release);
        do_message_loop_work();
    }

    /// When CEF next wants [`Engine::pump`] (in the past: now).
    pub fn next_due(&self) -> std::time::Instant {
        epoch() + std::time::Duration::from_nanos(PUMP_DUE.load(Ordering::Acquire))
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

/// The switches in `TRAILBOARD_CEF_ARGS`: whitespace-separated `--name` or
/// `--name=value` (the dashes are optional), as (name, value).
fn extra_switches(s: &str) -> Vec<(String, Option<String>)> {
    s.split_whitespace()
        .filter_map(|w| {
            let w = w.trim_start_matches('-');
            let (name, value) = match w.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (w, None),
            };
            (!name.is_empty()).then(|| (name.to_string(), value))
        })
        .collect()
}

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
    /// What the last few serials changed, oldest first: `(serial, rect in
    /// pixels)`, `None` for the whole frame. See [`PaintFrame::changed_since`].
    damage: Vec<(u64, Option<IRect>)>,
}

/// Serials remembered in [`PaintFrame::damage`]; a consumer further behind
/// uploads the whole frame.
const DAMAGE_KEPT: usize = 16;

impl PaintFrame {
    /// A new serial, which changed `rect` (`None`: everything, or unknown).
    fn bump(&mut self, rect: Option<IRect>) {
        self.serial += 1;
        if self.damage.len() == DAMAGE_KEPT {
            self.damage.remove(0);
        }
        self.damage.push((self.serial, rect));
    }

    /// The pixels that changed after `serial` up to now, as one rect (clamped
    /// to the frame), or `None` if that isn't known (too far behind, a resize,
    /// a popup), in which case the whole frame should be taken.
    pub fn changed_since(&self, serial: u64) -> Option<IRect> {
        if serial >= self.serial {
            return Some(IRect::default());
        }
        let first = self.damage.first()?.0;
        if first > serial + 1 {
            return None; // forgotten
        }
        let rects: Option<Vec<IRect>> = self.damage.iter().filter(|(s, _)| *s > serial).map(|(_, r)| *r).collect();
        let r = popup::bounding(&rects?)?;
        let (x0, y0) = (r.x.max(0), r.y.max(0));
        let (x1, y1) = ((r.x + r.w).min(self.width as i32), (r.y + r.h).min(self.height as i32));
        Some(IRect { x: x0, y: y0, w: (x1 - x0).max(0), h: (y1 - y0).max(0) })
    }
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
    /// The main frame's scriptlets, registered to run before its page's scripts.
    early: cosmetic::Early,
    /// A frame clicked a cookie banner's reject button ([`consent`]) since
    /// the main frame last started loading.
    banner_rejected: std::cell::Cell<bool>,
    /// What each frame says about its media ([`media`]).
    media: RefCell<media::Frames>,
    /// The page is in full screen (`on_fullscreen_mode_change`).
    fullscreen: std::cell::Cell<bool>,
    /// The page keeps its focus whatever the app asks ([`Page::hold_focus`]).
    hold_focus: std::cell::Cell<bool>,
    /// JavaScript is off in this page (last sent with [`Shared::apply_javascript`]).
    script_off: std::cell::Cell<bool>,
    /// An inspector page's line to the page it inspects.
    bridge: RefCell<Option<inspector::Bridge>>,
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
            PageKind::Note | PageKind::App | PageKind::Inspector => 1.0,
        }
    }

    /// Turns JavaScript on or off in the page as [`set_javascript`] says for
    /// `url` (where it is going). Web pages only: the app's own pages need theirs.
    fn apply_javascript(&self, host: &BrowserHost, url: &str) {
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
        f.bump(None);
        note_activity();
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
                        Some(PaintFrame { width: f.width, height: f.height, pixels: f.pixels.clone(), serial: 0, css: f.css, damage: Vec::new() });
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
                    f.bump(None);
                    note_activity();
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
            dirty_rects: Option<&[Rect]>,
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
            PAINTS.fetch_add(1, Ordering::Relaxed);
            let sh = &self.shared;
            log::trace!("page {} paint {width}x{height} popup={}", sh.key, type_ == PaintElementType::POPUP);
            if type_ != PaintElementType::POPUP && sh.frame.borrow().width != width as u32 {
                log::debug!("page {} paints at {width}x{height} (scale {})", sh.key, sh.scale.borrow());
            }
            let css = paint_css((width as u32, height as u32), &sh.recent.borrow());
            // Only what Chromium says it repainted changed, unless the size did.
            let dirty: Vec<IRect> =
                dirty_rects.unwrap_or_default().iter().map(|r| IRect { x: r.x, y: r.y, w: r.width, h: r.height }).collect();
            let store = |f: &mut PaintFrame| {
                let resized = (f.width, f.height) != (width as u32, height as u32);
                f.width = width as u32;
                f.height = height as u32;
                f.css = css;
                // The rest of the frame already holds these pixels.
                let n = popup::copy_dirty(&mut f.pixels, src, f.width, f.height, !resized, &dirty);
                PAINT_BYTES.fetch_add(n as u64, Ordering::Relaxed);
                f.bump(if resized { None } else { popup::bounding(&dirty) });
                note_activity();
            };
            if type_ == PaintElementType::POPUP {
                let mut p = sh.popup.borrow_mut();
                let same = (p.width, p.height) == (width as u32, height as u32);
                p.width = width as u32;
                p.height = height as u32;
                let n = popup::copy_dirty(&mut p.pixels, src, width as u32, height as u32, same, &dirty);
                PAINT_BYTES.fetch_add(n as u64, Ordering::Relaxed);
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
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: ::std::os::raw::c_int,
            is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // JavaScript on or off as the site it goes to has it, before it loads.
            if frame.as_ref().is_some_and(|f| f.is_main() != 0)
                && let (Some(host), Some(r)) = (browser.and_then(|b| b.host()), request.as_ref())
            {
                let url = CefString::from(&r.url()).to_string();
                self.shared.apply_javascript(&host, &url);
                // The site's scriptlets, in place before its first script runs.
                if self.shared.kind == PageKind::Web {
                    self.shared.early.before_navigation(&host, &url);
                }
            }
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
            if matches!(self.shared.kind, PageKind::Note | PageKind::App) {
                // A note stays a note (an app page, itself): only its own page loads in it; a link
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

        fn on_fullscreen_mode_change(&self, _browser: Option<&mut Browser>, fullscreen: ::std::os::raw::c_int) {
            let on = fullscreen != 0;
            self.shared.fullscreen.set(on);
            emit(WebEvent::Fullscreen { page: self.shared.key, on });
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
            if let Some(what) = message.strip_prefix(youtube::PREFIX) {
                let url = self.shared.url.borrow();
                log::info!("youtube: {what} on {}", url.chars().take(100).collect::<String>());
                return 1;
            }
            if let Some(body) = message.strip_prefix(media::PREFIX) {
                if self.shared.kind == PageKind::Web
                    && let Some((token, state)) = media::parse(body)
                    && let Some(playing) = self.shared.media.borrow_mut().report(token, state)
                {
                    emit(WebEvent::Media { page: self.shared.key, playing });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(media::DONE) {
                if self.shared.kind == PageKind::Web && body.len() <= 120 {
                    emit(WebEvent::MediaDone { page: self.shared.key, answer: body.to_string() });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(media::TROUBLE) {
                if self.shared.kind == PageKind::Web && matches!(body, "format" | "drm") {
                    emit(WebEvent::MediaTrouble { page: self.shared.key, kind: body.to_string() });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(focus::PREFIX) {
                if let Some(report) = focus::parse(body) {
                    emit(WebEvent::Focus { page: self.shared.key, report });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(scroll::PREFIX) {
                if let Some(report) = scroll::parse(body) {
                    emit(WebEvent::Scroll { page: self.shared.key, report });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(caret::PREFIX) {
                log::trace!("caret: {}", body.chars().take(80).collect::<String>().replace('\n', " | "));
                if let Some(report) = caret::parse(body) {
                    emit(WebEvent::Caret { page: self.shared.key, report });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(field::PREFIX) {
                if let Some(report) = field::parse(body) {
                    emit(WebEvent::Field { page: self.shared.key, report });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(pager::PREFIX) {
                emit(WebEvent::Pager { page: self.shared.key, links: pager::parse(body) });
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
            if self.shared.kind == PageKind::Inspector {
                let Some(body) = message.strip_prefix(inspector::PREFIX) else { return 0 };
                if let Some(b) = self.shared.bridge.borrow().as_ref() {
                    b.on_frontend_message(body);
                }
                return 1;
            }
            if self.shared.kind == PageKind::App {
                if let Some(action) = message.strip_prefix(APP_ACTION) {
                    emit(WebEvent::AppAction { page: self.shared.key, action: action.to_string() });
                    return 1;
                }
                return 0;
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
            can_go_back: ::std::os::raw::c_int,
            can_go_forward: ::std::os::raw::c_int,
        ) {
            if let Some(h) = browser.and_then(|b| b.host()) {
                self.shared.apply_zoom(&h);
            }
            emit(WebEvent::LoadingChanged { page: self.shared.key, loading: is_loading != 0, can_back: can_go_back != 0, can_forward: can_go_forward != 0 });
        }

        fn on_load_start(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, _transition_type: TransitionType) {
            let Some(frame) = frame else { return };
            if frame.is_main() != 0 && self.shared.kind == PageKind::Inspector {
                frame.execute_java_script(Some(&CefString::from(inspector::SHIM)), None, 0);
            }
            if frame.is_main() != 0 {
                self.shared.committed.set(true);
                self.shared.banner_rejected.set(false);
                let cleared = self.shared.media.borrow_mut().clear();
                if let Some(playing) = cleared {
                    emit(WebEvent::Media { page: self.shared.key, playing });
                }
                // Chromium drops a zoom set before the page commits. Web pages
                // get it again when their address changes; app pages (the
                // start page) would otherwise wait until they finish loading,
                // laid out at the wrong size until then.
                if let Some(h) = browser.as_ref().and_then(|b| b.host()) {
                    self.shared.apply_zoom(&h);
                }
            }
            // Focus reports for Insert mode, in every frame of pages you browse
            // or use (notes take their keys another way).
            if self.shared.kind != PageKind::Note {
                frame.execute_java_script(Some(&CefString::from(focus::script().as_str())), None, 0);
            }
            // Media reports, so a card playing something stays awake.
            if self.shared.kind == PageKind::Web {
                frame.execute_java_script(Some(&CefString::from(media::script().as_str())), None, 0);
            }
            // Link hints inside frames: an idle listener in each frame below
            // the main one answers the main frame's hint script.
            if self.shared.kind == PageKind::Web && frame.is_main() == 0 {
                frame.execute_java_script(Some(&CefString::from(links::frame_script())), None, 0);
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
            // The inspector draws its own menus in the page.
            if self.shared.kind == PageKind::Inspector {
                return;
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
                misspelled: CefString::from(&p.misspelled_word()).to_string(),
                suggestions: {
                    let mut list = CefStringList::new();
                    p.dictionary_suggestions(Some(&mut list));
                    list.into_iter().collect()
                },
                site: None,
                inspect: None,
            };
            // Params are view px; the app works in CSS px.
            let s = *self.shared.scale.borrow();
            let (x, y) = (p.xcoord() as f32 / s, p.ycoord() as f32 / s);
            let context = menu::MenuContext {
                inspect: (self.shared.kind == PageKind::Web).then_some((x.round() as i32, y.round() as i32)),
                ..context
            };
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

// One handler per CEF callback group makes `PageClient::new` take eleven.
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
        permissions: PermissionHandler,
        dialogs: JsdialogHandler,
        files: DialogHandler,
    }

    impl Client {
        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(self.permissions.clone())
        }
        fn jsdialog_handler(&self) -> Option<JsdialogHandler> {
            Some(self.dialogs.clone())
        }
        fn dialog_handler(&self) -> Option<DialogHandler> {
            Some(self.files.clone())
        }
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
        let settings = RequestContextSettings {
            persist_session_cookies: 0,
            accept_language_list: CefString::from(prefs::accept_language().as_str()),
            ..Default::default()
        };
        let ctx = request_context_create_context(Some(&settings), None)?;
        prefs::apply_to(&ctx);
        Some(Jar(ctx))
    }

    /// Sets the current browsing preferences ([`prefs::apply`]) on this jar.
    pub fn refresh_prefs(&self) {
        prefs::apply_to(&self.0);
    }
}

impl Page {
    /// The state and handlers of a new page `key` of `kind` at `url`.
    fn parts(key: u64, url: &str, kind: PageKind, size: (i32, i32), scale: f32, spawn_beside: &SpawnBeside) -> (StdRc<Shared>, std::sync::Arc<shield::Blocked>, Client) {
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
            early: Default::default(),
            banner_rejected: std::cell::Cell::new(false),
            media: Default::default(),
            fullscreen: Default::default(),
            hold_focus: std::cell::Cell::new(false),
            script_off: std::cell::Cell::new(false),
            bridge: RefCell::new(None),
        });
        let blocked = std::sync::Arc::new(shield::Blocked::default());
        let client = PageClient::new(
            PageRender::new(shared.clone()),
            PageLife::new(shared.clone()),
            PageRequests::new(shared.clone(), spawn_beside.0.clone(), (kind == PageKind::Web).then(|| shield::guard(blocked.clone()))),
            PageDisplay::new(shared.clone()),
            PageLoad::new(shared.clone()),
            PageMenu::new(shared.clone()),
            PageFind::new(shared.clone()),
            PageDownloads::new(shared.clone()),
            permission::handler(key),
            dialog::js_handler(key),
            dialog::file_handler(key),
        );
        (shared, blocked, client)
    }

    fn browser_settings() -> BrowserSettings {
        BrowserSettings {
            windowless_frame_rate: 60,
            background_color: 0xffff_ffff,
            ..Default::default()
        }
    }

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
        let (shared, blocked, mut client) = Self::parts(key, url, kind, size, scale, spawn_beside);
        let window_info = WindowInfo { windowless_rendering_enabled: 1, ..Default::default() };
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from(url)),
            Some(&Self::browser_settings()),
            None,
            jar.map(|j| j.0.clone()).as_mut(),
        )?;
        if let Some(h) = browser.host() {
            shared.apply_zoom(&h);
            shared.apply_javascript(&h, url);
            if kind == PageKind::Web {
                shared.early.watch(&h);
                // The media script also runs before each document's own
                // scripts (in the main frame and frames in its process), so
                // it sees a DRM request made at once. Load start still
                // installs it in other frames; it installs once.
                let params = serde_json::json!({ "source": media::script() }).to_string();
                send_dev_tools(&h, "Page.addScriptToEvaluateOnNewDocument", &params);
            }
        }
        let page = Page { browser, shared, blocked };
        page.apply_color_scheme();
        Some(page)
    }

    /// Opens Chromium's inspector for this page as a page of its own, `key`,
    /// rendered off screen like any other at `size` CSS px: the inspector's
    /// own pages, talking to this page through [`inspector::Bridge`]. `at`:
    /// a point in this page (CSS px) whose element it opens on.
    pub fn open_inspector(&self, key: u64, size: (i32, i32), scale: f32, at: Option<(f32, f32)>, spawn_beside: &SpawnBeside) -> Option<Page> {
        let target = self.host()?;
        let page = Page::new(key, inspector::FRONTEND_URL, PageKind::Inspector, size, scale, spawn_beside, None)?;
        let bridge = inspector::Bridge::new(&target, page.browser.clone(), at.map(|p| self.view_point(p)))?;
        *page.shared.bridge.borrow_mut() = Some(bridge);
        log::info!("page {}: inspector {key} open", self.shared.key);
        Some(page)
    }

    /// Points `inspector`, this page's open inspector, at the element at `at` (CSS px).
    pub fn inspect_at(&self, inspector: &Page, at: (f32, f32)) {
        if let Some(b) = inspector.shared.bridge.borrow().as_ref() {
            b.inspect(self.view_point(at));
        }
    }

    /// This page is an inspector of `page`'s browser (still the same one).
    pub fn inspects(&self, page: &Page) -> bool {
        self.shared.bridge.borrow().as_ref().is_some_and(|b| b.target_id() == page.browser.identifier())
    }

    /// A point in CSS px as Chromium's view px.
    fn view_point(&self, (x, y): (f32, f32)) -> (i32, i32) {
        let s = *self.shared.scale.borrow();
        ((x * s).round() as i32, (y * s).round() as i32)
    }

    /// Sends the colour scheme set with `set_color_scheme` to this page.
    pub fn apply_color_scheme(&self) {
        let (Some(dark), Some(host)) = (DARK.get(), self.browser.host()) else { return };
        let scheme = if dark { "dark" } else { "light" };
        let params = format!(r#"{{"features":[{{"name":"prefers-color-scheme","value":"{scheme}"}}]}}"#);
        if !send_dev_tools(&host, "Emulation.setEmulatedMedia", &params) {
            log::warn!("page {}: couldn't set colour scheme", self.shared.key);
        }
    }

    /// Asks [`set_javascript`] again for the page's address; if the answer
    /// changed, applies it and reloads (scripts that ran can't be taken back,
    /// and ones that didn't need a new load to run).
    pub fn refresh_javascript(&self) {
        let Some(host) = self.browser.host() else { return };
        let was = self.shared.script_off.get();
        let url = self.shared.url.borrow().clone();
        self.shared.apply_javascript(&host, &url);
        if self.shared.script_off.get() != was {
            self.browser.reload();
        }
    }

    /// Puts `word` in place of the misspelt word the menu was opened on.
    pub fn replace_misspelling(&self, word: &str) {
        if let Some(h) = self.browser.host() {
            h.replace_misspelling(Some(&CefString::from(word)));
        }
    }

    /// Stops marking `word` as misspelt.
    pub fn add_to_dictionary(&self, word: &str) {
        if let Some(h) = self.browser.host() {
            h.add_word_to_dictionary(Some(&CefString::from(word)));
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

    /// Chromium's default zoom as a factor over the raster scale (its default
    /// stays 100%, so this is 1 / scale).
    pub fn default_zoom_factor(&self) -> f64 {
        let level = self.host().map_or(0.0, |h| h.default_zoom_level());
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

    /// Ends find in page and clears its marks, the active match's selection
    /// included (Esc after a search). A search the find bar already ended
    /// left that match as the page's selection, so it is dropped too.
    pub fn clear_find(&self) {
        if let Some(h) = self.host() {
            h.stop_finding(1);
        }
        self.run_script("window.getSelection && window.getSelection().removeAllRanges();");
    }

    pub fn set_hidden(&self, hidden: bool) {
        if let Some(h) = self.host() {
            h.was_hidden(hidden as _);
        }
    }

    /// Gives the page its focus or takes it away. While the page holds its
    /// focus ([`Page::hold_focus`]) taking it away does nothing.
    pub fn set_focus(&self, focus: bool) {
        if !focus && self.shared.hold_focus.get() {
            return;
        }
        if let Some(h) = self.host() {
            h.set_focus(focus as _);
        }
    }

    /// Keeps the page focused (`on`) whichever card has the keys, as an
    /// *Always active* card does. Turning it off leaves the focus as it is.
    pub fn hold_focus(&self, on: bool) {
        self.shared.hold_focus.set(on);
        if on && let Some(h) = self.host() {
            h.set_focus(1);
        }
    }

    /// How often the page paints, frames a second (60 normally).
    pub fn set_frame_rate(&self, fps: i32) {
        if let Some(h) = self.host() {
            h.set_windowless_frame_rate(fps.clamp(1, 60));
        }
    }

    /// Whether any of the page's frames plays media ([`WebEvent::Media`]).
    pub fn playing(&self) -> bool {
        self.shared.media.borrow().playing()
    }

    pub fn set_muted(&self, muted: bool) {
        if let Some(h) = self.host() {
            h.set_audio_muted(muted as _);
        }
    }

    pub fn muted(&self) -> bool {
        self.host().is_some_and(|h| h.is_audio_muted() != 0)
    }

    /// Runs a media control (`op` with its number, see [`media::control`])
    /// in the frame whose media played or showed last. False when no frame
    /// has media. The answer comes as [`WebEvent::MediaDone`].
    ///
    /// The main frame gets it through the DevTools protocol as if from a
    /// click, so playing with sound and full screen are allowed there;
    /// other frames through plain script (a page may refuse those).
    pub fn media_control(&self, op: &str, value: f64, rates: &[u16]) -> bool {
        let Some(target) = self.shared.media.borrow().target().map(str::to_string) else { return false };
        let Some(script) = media::control(&target, op, value, rates) else { return false };
        let Some(host) = self.host() else { return false };
        let params = format!(r#"{{"expression":{},"userGesture":true}}"#, js_string(&script));
        let main_sent = send_dev_tools(&host, "Runtime.evaluate", &params);
        let main_id = self.browser.main_frame().map(|f| CefString::from(&f.identifier()).to_string());
        let mut ids = CefStringList::new();
        self.browser.frame_identifiers(Some(&mut ids));
        for id in ids.into_iter() {
            if Some(&id) == main_id.as_ref() && main_sent {
                continue;
            }
            if let Some(f) = self.browser.frame_by_identifier(Some(&CefString::from(id.as_str()))) {
                f.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
            }
        }
        true
    }

    /// The page is in full screen.
    pub fn fullscreen(&self) -> bool {
        self.shared.fullscreen.get()
    }

    /// Takes the page out of full screen.
    pub fn exit_fullscreen(&self) {
        if let Some(h) = self.host() {
            h.exit_fullscreen(1);
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

    /// Stops loading (`Ctrl+s`).
    pub fn stop(&self) {
        self.browser.stop_load();
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

    /// Sends the focused text field's text as a [`WebEvent::Field`] report
    /// under `token`; returns the frame it runs in, for [`Page::write_field`].
    pub fn read_field(&self, token: u64) -> Option<String> {
        let f = self.edit_frame()?;
        f.execute_java_script(Some(&CefString::from(field::read(token).as_str())), None, 0);
        Some(CefString::from(&f.identifier()).to_string())
    }

    /// Puts `text` into the field [`Page::read_field`] read under `token` in `frame`.
    pub fn write_field(&self, frame: &str, token: u64, text: &str) -> bool {
        let Some(f) = self.browser.frame_by_identifier(Some(&CefString::from(frame))) else { return false };
        f.execute_java_script(Some(&CefString::from(field::write(token, text).as_str())), None, 0);
        true
    }

    /// Runs `script` in the page's main frame.
    pub fn run_script(&self, script: &str) {
        if let Some(f) = self.browser.main_frame() {
            f.execute_java_script(Some(&CefString::from(script)), None, 0);
        }
    }

    /// Focus the first visible text field (`gi`); the answer arrives as a
    /// [`WebEvent::Focus`] (a user's focus, or [`focus::Report::NoField`]).
    pub fn focus_first_field(&self) {
        self.run_script(&focus::focus_first_field());
    }

    /// Take the focus out of the focused field, in whichever frame has it.
    pub fn blur_field(&self) {
        if let Some(f) = self.edit_frame() {
            f.execute_java_script(Some(&CefString::from(focus::BLUR)), None, 0);
        }
    }

    /// Ask for the targets of `filter` visible in the page; they arrive as
    /// [`WebEvent::Links`].
    pub fn request_links(&self, filter: links::Filter) {
        self.run_script(&links::script(filter));
    }

    /// Ask for the page's next / previous links; they arrive as [`WebEvent::Pager`].
    pub fn request_pager(&self) {
        self.run_script(pager::SCRIPT);
    }

    /// Run a scroll section command ([`scroll::list`], [`scroll::by`]…); the
    /// answer arrives as [`WebEvent::Scroll`].
    pub fn scroll_command(&self, script: &str) {
        self.run_script(script);
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
        // The bridge holds this page's browser: let it go first.
        self.shared.bridge.borrow_mut().take();
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
    use super::{extra_switches, IRect, PaintFrame, DAMAGE_KEPT};

    #[test]
    fn extra_switches_reads_names_and_values() {
        let s = |n: &str, v: Option<&str>| (n.to_string(), v.map(str::to_string));
        assert_eq!(
            extra_switches("  --disable-gpu-compositing\t--use-gl=angle disable-gpu -- --=x"),
            vec![s("disable-gpu-compositing", None), s("use-gl", Some("angle")), s("disable-gpu", None)]
        );
        assert!(extra_switches("").is_empty());
    }

    fn r(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect { x, y, w, h }
    }

    fn frame() -> PaintFrame {
        PaintFrame { width: 100, height: 50, ..Default::default() }
    }

    #[test]
    fn changed_since_unions_the_paints_after_a_serial() {
        let mut f = frame();
        f.bump(Some(r(10, 10, 2, 16))); // 1: a caret
        f.bump(Some(r(40, 5, 10, 10))); // 2
        f.bump(Some(r(0, 30, 5, 5))); // 3
        assert_eq!(f.changed_since(3), Some(IRect::default()), "nothing new");
        assert_eq!(f.changed_since(2), Some(r(0, 30, 5, 5)));
        assert_eq!(f.changed_since(1), Some(r(0, 5, 50, 30)));
        assert_eq!(f.changed_since(0), Some(r(0, 5, 50, 30)));
    }

    #[test]
    fn changed_since_is_clamped_to_the_frame() {
        let mut f = frame();
        f.bump(Some(r(-5, 40, 200, 30)));
        assert_eq!(f.changed_since(0), Some(r(0, 40, 100, 10)));
    }

    #[test]
    fn changed_since_doesnt_know_after_a_whole_frame_change_or_when_too_far_behind() {
        let mut f = frame();
        f.bump(Some(r(1, 1, 1, 1)));
        f.bump(None); // a popup, a resize
        f.bump(Some(r(1, 1, 1, 1)));
        assert_eq!(f.changed_since(0), None);
        assert_eq!(f.changed_since(1), None);
        assert_eq!(f.changed_since(2), Some(r(1, 1, 1, 1)));
        for _ in 0..DAMAGE_KEPT {
            f.bump(Some(r(1, 1, 1, 1)));
        }
        assert_eq!(f.changed_since(2), None, "serial 3 is forgotten");
        assert_eq!(f.changed_since(3), Some(r(1, 1, 1, 1)));
    }

    /// The engine the tests load is the one at `CEF_PATH`, which the build
    /// already checked is this version (`cef-dll-sys` reads its archive.json).
    #[test]
    fn the_loaded_engine_reports_its_version() {
        assert_eq!(super::loaded_engine_version(), super::built_engine_version());
        assert!(super::built_engine_version().0 > 100);
    }

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
