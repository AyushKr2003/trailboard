//! What pages tell the app (`WebEvent`), and the page kinds, cursors and edit commands that go with it.

use crate::*;

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
    pub(crate) fn from_cef(t: CursorType) -> Cursor {
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
    pub(crate) static EVENTS: RefCell<Vec<WebEvent>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn emit(e: WebEvent) {
    EVENTS.with_borrow_mut(|q| q.push(e));
    note_activity();
}
