//! Board state and interaction: cards are live pages on an infinite canvas.
//!
//! Input model
//! - Drag empty board / middle-drag anywhere: pan. Wheel over board: pan. Ctrl+wheel: zoom.
//! - Click a card's page: focus it and give it the keys (and the wheel). Drag its title
//!   bar to move, its bottom-right corner to resize; × closes; double-click the title bar
//!   to fill the view. A title-bar click makes the card current without the keys.
//! - Keyboard first (Omarchy's app convention): keys are board keys unless a page has
//!   them (`App::typing`). h/j/k/l or arrows go to the nearest card that way
//!   (`scene::nav`), Shift with them pans, +/- zoom, Enter/Space give the current card
//!   the keys, Esc takes them back (the card stays current) and a second Esc lets go
//!   of the card, Delete closes it, Q quits. Board keys (f, N, T, S, O, R, C, [, ])
//!   act on the current card, else the one in the middle of the view.
//! - B opens the trails sidebar (`scene::sidebar`, `sidebar`) with the keys:
//!   j/k move its highlight and go to that card, h/l collapse / expand a trail,
//!   Enter gives a card's page the keys (or toggles a trail), Esc or B closes it.
//!   Clicks on a row do the same; a click elsewhere takes the keys back, the
//!   panel stays open.
//! - f labels the links and controls on the current card's page (`scene::hint`,
//!   `web::links`); typing a label opens a link beside the card, which becomes
//!   current, or clicks a control (a field also gets the keys). Esc, another
//!   key, a click or a scroll ends the labels. Shift+F fits everything.
//! - Mouse-only actions have keys too: v selects / deselects the current card
//!   (h/j/k/l keep the selection), z fills the view with it, d/u scroll its
//!   page, and the Menu key or Shift+F10 opens its page menu (Chromium picks
//!   the spot and context; j/k, Enter, Esc in menus).
//! - M arranges the current card (`scene::arrange`): h/j/k/l move it (the
//!   selection with it), Shift resizes, Ctrl takes fine steps; Enter/M keep the
//!   result as one undo step, Esc puts it back, any other key or a click keeps it.
//! - Ctrl+K palette (`palette`, `scene::palette`): fuzzy search over every card
//!   (folded ones too), the other spaces and the board commands; Up/Down pick,
//!   Enter goes there or runs it, the last row searches the web or opens an address.
//! - Ctrl+H history view (`palette::history_rows`): visited pages newest first,
//!   filtered by words; Shift+Delete forgets one (in Ctrl+K too); *Clear history*
//!   asks first (`Confirm`).
//! - Ctrl+, settings (`scene::settings`, saved by `storage::settings`): a list
//!   under the field like history; Enter, → or a click steps the highlighted
//!   setting on, Shift+Enter, ← or a right-click back. Each applies at once.
//! - Ctrl+J downloads (`palette::download_rows`, `scene::download`): files go
//!   straight to the Downloads folder; Enter opens one, Ctrl+Enter its folder,
//!   Shift+Delete cancels or removes. Pages with running downloads are parked.
//! - Ctrl+L address bar, Ctrl+T new card, Ctrl+W close, Ctrl+Shift+T reopen,
//!   Ctrl+R reload, Alt+←/→ back/forward.
//! - Ctrl+F find in the current card's page (the bar is an `omnibox::Target::Find`;
//!   `sync_find` stops the search when it closes). Ctrl +/−/0 zoom the current
//!   page per site (`scene::pagezoom`, saved by `storage::zoom`, applied by
//!   `web::set_page_zoom`); with no page current Ctrl+0 fits all.
//! - Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y) undo/redo board edits (open, close, move,
//!   resize). After clicking inside a page they go to the page instead (text undo).
//! - Shift+drag on the board draws a selection band; Shift+click a title bar adds or
//!   removes that card; Ctrl+A (no card focused) selects all. Dragging a selected
//!   card's title bar moves the whole selection; Delete/Backspace closes it. A click
//!   on the board or Esc clears it.
//!
//! - `?` with no card focused shows a sheet of shortcuts; Esc, `?` or a click hides it.
//! - Right-click in a page opens a context menu (drawn by the app); Esc, a click
//!   outside, scrolling or moving the board closes it.
//! - Ctrl+C / Ctrl+X / Ctrl+V in a page go through the desktop clipboard (see `clipboard`).
//! - IME (e.g. fcitx5) composition goes to the focused page or the address bar.
//! - F1 (anywhere) or `?` (no card focused) shows the shortcuts; Ctrl+Q quits.
//! - Ctrl+S saves the focused note, or else the whole board, as Markdown in
//!   `Documents/Trailboard` (`scene::export`, `storage::export`); a note's page
//!   menu has *Save as Markdown* too.
//! - Tab (unless keys are going to a page) toggles Read Mode: the focused card, or
//!   the one in the middle of the view, fills the window and its page lays out at
//!   the window's size. Esc, Tab or a double-click on its bar returns to the board.
//! - Looks follow Omarchy live (see `theme` and `desktop`): the theme's palette,
//!   light or dark (pages too), Hyprland's rounding and the desktop text size.
//! - Spaces: Ctrl+Shift+N new space, Ctrl+PageDown / Ctrl+PageUp next / previous,
//!   F2 rename, Ctrl+Shift+B next tint (a theme colour), Ctrl+Shift+W delete (asks
//!   first when the space has cards), Ctrl+Shift+P private on / off. Each space
//!   keeps its own board, camera, background, undo history and closed cards. A
//!   private space's pages are not recorded (history, icons, zoom on disk) and
//!   share a cookie jar in memory (`web::Jar`) that lasts until the app quits.
//!
//! Live-browser cap: at most *Pages kept awake* (a setting, default 6;
//! `TRAILBOARD_MAX_LIVE` overrides it for a run) cards keep a live
//! browser (see `scene::liveness`). The others sleep: their page is closed and the
//! card shows its last frame, dimmed, until it is focused or scrolled into view.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use compositor::{Background, Layer, Quad, Renderer, Shape, Text};
use scene::arrange::Arrange;
use scene::hint;
use scene::nav::{self, Dir};
use scene::pagezoom::{self, SiteZooms};
use scene::settings::{self as prefs, Settings};
use scene::{liveness, placement, read, selection, tidy, Camera, Change, Edge, History, Node, NodeId, Rect, Scene, Selection, TrailStep, Vec2};
use storage::spaces::{self, SpaceId, SpaceIndex};
use web::{MouseButton as WebButton, Page, SpawnBeside, WebEvent};
use winit::application::ApplicationHandler;
use web::menu::{MenuAction, MenuItem};
use web::EditCommand;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, ModifiersState, NamedKey};
use winit::platform::wayland::WindowAttributesExtWayland;
use winit::window::{CursorIcon, Window, WindowId};

use crate::clipboard::{Clipboard, Paste};
use crate::desktop::Desktop;
use crate::launch;
use crate::theme::{rgb, text_on, Theme};
use crate::keymap::{self, KeyIn};
use crate::menu::{self, ContextMenu, Entry};
use crate::omnibox::{self, Omnibox, Target};
use crate::palette::{self, Command, Item};
use crate::script::{Cmd, Script};
use crate::sidebar::Panel;

const START_URL: &str = "https://en.wikipedia.org/wiki/Special:Random";
const CARD_W: f32 = 1024.0;
const CARD_H: f32 = 720.0;
const MIN_CARD: f32 = 240.0;
/// How long after a page loads (or retitles itself) its text is read for history.
const TEXT_SETTLE: Duration = Duration::from_millis(1200);
/// One keyboard move or resize step (M, then h/j/k/l), in logical px on screen.
const ARRANGE_STEP: f32 = 40.0;
/// The fine step with Ctrl.
const ARRANGE_FINE: f32 = 8.0;
const ARRANGE_HELP: &str = "Moving · h j k l move · Shift resize · Ctrl fine · Enter done · Esc cancel";
const RESIZE_GRIP: f32 = 18.0;
const CLOSE_W: f32 = 30.0;
/// Height of the Read Mode title bar, in overlay px (scaled by `ui()`).
const READ_BAR: f32 = 34.0;
/// How long a toast stays on screen.
const TOAST_FOR: Duration = Duration::from_millis(1800);
/// Bits of a page key that hold the card id; the space epoch sits above them.
const KEY_ID_BITS: u32 = 40;

/// Texture keys of site icons: this bit set, so they never meet a page key.
const ICON_KEY: u64 = 1 << 63;
/// Texture key of the page preview beside the palette's list.
const PREVIEW_KEY: u64 = 1 << 62;
/// Sleeping cards given their page's thumbnail per frame, at most (each is a
/// database read and a JPEG decode).
const THUMBS_PER_FRAME: usize = 4;

/// The page preview beside the palette's list: whose it is, and its size
/// once uploaded as [`PREVIEW_KEY`] (`None` when the page has no thumbnail).
struct Preview {
    url: String,
    size: Option<(u32, u32)>,
}

/// A site's icon, uploaded as texture `key` whenever `serial` changes.
struct SiteIcon {
    key: u64,
    serial: u64,
    icon: storage::icons::Icon,
    /// Written under `icons/` (not when only a private space has seen it).
    saved: bool,
}

/// Browser and texture key for card `id` while `epoch` is current.
fn page_key(epoch: u64, id: NodeId) -> u64 {
    (epoch << KEY_ID_BITS) | id.0
}

/// Per-space state that lives only for this session.
#[derive(Default)]
struct Session {
    history: History,
    closed: Vec<(Node, Vec<Edge>)>,
    /// A private space's cookie jar and page zooms, kept while the app runs.
    jar: Option<web::Jar>,
    zooms: Option<std::collections::BTreeMap<String, f64>>,
}

#[derive(Debug, Clone, Copy)]
enum Drag {
    None,
    Pan { last: Vec2 },
    /// Title-bar drag of `id`; `App::group` holds the start rects of every card it moves.
    Move { id: NodeId, grab: Vec2 },
    /// Selection band from `anchor` (world) to the cursor.
    Band { anchor: Vec2 },
    Resize { id: NodeId, anchor: Vec2, start: Rect },
    /// Mouse is down inside a page; moves and release go to it.
    Page { id: NodeId, button: WebButton },
}

/// Link hints on card `id`: its visible links (rects in the page view's CSS
/// px) and their labels, `None` until the page has answered.
/// A press on a trail's header row in the sidebar (`App::sidebar_press`).
#[derive(Debug, Clone, Copy)]
struct SidebarPress {
    /// The trail, by its first card.
    head: NodeId,
    at: Vec2,
    /// Moved far enough to be a drag rather than a click.
    dragging: bool,
    /// Where the trail would go among its part's trails if let go now.
    to: Option<usize>,
}

struct LinkHints {
    id: NodeId,
    links: Vec<(Rect, web::links::Action)>,
    hints: Option<hint::Hints>,
}

struct Card {
    page: Page,
    loading: bool,
    hidden: bool,
    /// Cursor the page asked for, shown while the pointer is over its content.
    cursor: web::Cursor,
}

/// Appear animations of the overlays.
#[derive(Default)]
struct Anims {
    omnibox: crate::chrome::Appear,
    menu: crate::chrome::Appear,
    toast: crate::chrome::Appear,
    sheet: crate::chrome::Appear,
    sidebar: crate::chrome::Appear,
}

/// A small hash for telling one overlay's content from the next.
fn anim_key<T: std::hash::Hash>(v: T) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
    BuildHasherDefault::<DefaultHasher>::default().hash_one(v)
}

pub struct App {
    renderer: Option<Renderer>,
    scene: Scene,
    camera: Camera,
    /// Camera we're gliding toward (fit / read mode).
    camera_goal: Option<Camera>,
    /// Wheel and touchpad scrolling inside a page, and flings (`scene::scroll`):
    /// to card, at view px.
    page_scroll: scene::scroll::Scroller<(NodeId, i32, i32)>,
    /// Live browsers. A card in the scene but not here is asleep.
    cards: HashMap<NodeId, Card>,
    max_live: usize,
    /// The user's settings (Ctrl+,), saved in `settings.json`.
    settings: Settings,
    /// Ad and tracker blocking: filter lists and the sites allowed.
    blocking: crate::blocking::Blocking,
    /// `TRAILBOARD_MAX_LIVE` set the live-page cap for this run (until the setting changes).
    live_from_env: bool,
    /// Recency of use per card (larger is more recent), for choosing which cards sleep.
    used: HashMap<NodeId, u64>,
    use_clock: u64,
    focused: Option<NodeId>,
    /// Closed cards for Ctrl+Shift+T, newest last.
    closed: Vec<(Node, Vec<Edge>)>,
    history: History,
    selection: Selection,
    /// Cards moved by the current title-bar drag, with their start rects.
    group: Vec<(NodeId, Rect)>,
    /// The last click landed inside the focused page, so its keys (incl. Ctrl+Z) go there.
    keys_in_page: bool,
    omnibox: Option<Omnibox>,
    menu: Option<ContextMenu>,
    /// The shortcut sheet (`?`) is showing.
    hints: bool,
    /// Link hints (`f`) on a card's page: asked for, or showing labels.
    link_hints: Option<LinkHints>,
    /// Moving or resizing the current card from the keyboard (M).
    arrange: Option<Arrange>,
    /// Read Mode: this card fills the window (see `scene::read`).
    read: Option<NodeId>,
    /// The trails sidebar (B): collapsed groups, highlight and scroll.
    sidebar: scene::sidebar::Sidebar,
    sidebar_open: bool,
    /// The sidebar has the keys (j/k move its highlight).
    sidebar_keys: bool,
    /// Index of the sidebar's highlight when last seen, for a row that goes away.
    sidebar_at: Option<usize>,
    /// Sidebar row under the pointer.
    sidebar_hover: Option<usize>,
    /// A press on a trail's header in the sidebar: a click toggles the group,
    /// a drag moves the trail.
    sidebar_press: Option<SidebarPress>,
    /// The current mouse press started inside the context menu.
    menu_pressed: bool,
    clipboard: Clipboard,
    /// IME input is enabled on the window.
    ime_allowed: bool,
    /// Page with an IME composition in progress.
    ime_page: Option<NodeId>,
    /// Last IME candidate-window area sent to the window (screen px).
    ime_area: Option<[i32; 4]>,
    spawn_beside: SpawnBeside,
    drag: Drag,
    cursor: Vec2,
    mods: ModifiersState,
    last_click: Option<(Instant, Vec2, MouseButton)>,
    click_count: i32,
    /// When the camera last changed (used to re-rasterize pages at the settled zoom).
    camera_changed: Instant,
    page_scale: f32,
    /// Pages asked for on the command line, opened once the window exists.
    initial_urls: Vec<String>,
    started: Instant,
    script: Option<Script>,
    pub quit: bool,
    data_dir: std::path::PathBuf,
    spaces: SpaceIndex,
    /// False when the space index could not be read safely; nothing is saved then.
    persist: bool,
    /// Bumped on every space switch; part of each page's key, so late events and
    /// textures from the previous space's pages never land on this space's cards.
    epoch: u64,
    /// Undo history and closed cards of spaces not on screen (this session only).
    sessions: HashMap<SpaceId, Session>,
    /// The cookie jar of the active space when it is private.
    jar: Option<web::Jar>,
    /// Short message at the top of the window, until it expires.
    /// Message and when it goes.
    toast: Option<(String, Instant)>,
    /// How Chromium's sandbox confines pages, once `note_sandbox` is told.
    sandbox: Option<web::sandbox::Sandbox>,
    /// How far each overlay has come in (see `chrome::Appear`).
    anim: Anims,
    /// Waiting for Enter to do something that can't be undone.
    confirm: Option<Confirm>,
    /// Board changed since the last save.
    dirty: bool,
    last_save: Instant,
    /// The Omarchy look in effect, and what follows the desktop for changes.
    theme: Theme,
    desktop: Option<Desktop>,
    /// Page colour scheme last sent to pages (dark).
    page_scheme: Option<bool>,
    /// The note "Add to note" adds to: the one last focused, edited or made.
    note_target: Option<NodeId>,
    /// Files saved as Markdown this session, per space and note (`None`: the
    /// whole board), so saving again overwrites them.
    saved: HashMap<(SpaceId, Option<NodeId>), std::path::PathBuf>,
    /// Page zoom per site (Ctrl +/−/0), shared with `web` (`web::set_page_zoom`).
    zooms: Rc<RefCell<SiteZooms>>,
    /// Site icons by `scene::icon::site`, for every space, saved under `icons/`.
    icons: HashMap<String, SiteIcon>,
    /// Whether the pointer is over the window (for the zoomed-out name label).
    pointer_in: bool,
    /// The page find in page last searched (to stop it when the bar closes).
    finding: Option<NodeId>,
    /// Find in page's last result: matches and the active one (1-based).
    found: Option<(i32, i32)>,
    /// What find in page last looked for; the bar opens with it.
    last_find: String,
    /// When the Menu key (or Shift+F10) asked a page for its menu: the menu
    /// that answers opens with its first item highlighted.
    menu_by_key: Option<Instant>,
    /// The history index (`page-text.db`); `None` if it could not be opened.
    page_index: Option<storage::pagetext::PageIndex>,
    /// Cards whose page text is due to be read for history, and when.
    text_due: HashMap<NodeId, Instant>,
    /// The last history search (query, results), as the palette asks often.
    visits_cache: RefCell<VisitsCache>,
    /// The palette's page preview, and a serial for its uploads.
    preview: Option<Preview>,
    preview_serial: u64,
    /// Page keys of sleeping cards already looked up for a thumbnail.
    thumb_tried: HashSet<u64>,
    /// Downloads (Ctrl+J), saved in `downloads.json`.
    downloads: scene::download::Downloads,
    /// Chromium's id of each running download → (list key, page key).
    download_ids: HashMap<u32, (u64, u64)>,
    /// Pages put to sleep or closed while downloading: kept alive (off
    /// screen, unrendered) until their downloads end, since Chromium only
    /// reports a download to the page that started it.
    parked: Vec<Page>,
}

/// Last history search (history view or palette, query, results).
type VisitsCache = Option<(bool, String, Vec<storage::pagetext::Visit>)>;

/// History pages matching `query`, cached per query: for the history view
/// (`browse`, newest first) or the palette (best first).
fn visits_for(
    index: &Option<storage::pagetext::PageIndex>,
    cache: &RefCell<VisitsCache>,
    query: &str,
    browse: bool,
) -> Vec<storage::pagetext::Visit> {
    let Some(index) = index else { return Vec::new() };
    if let Some((b, q, v)) = &*cache.borrow()
        && *b == browse
        && q == query
    {
        return v.clone();
    }
    let found = if browse {
        index.browse(query, palette::HISTORY_LIST)
    } else {
        // More than shown, as pages open in cards are left out.
        index.search(query, palette::MAX_HISTORY * 3)
    };
    let v = found.unwrap_or_else(|e| {
        log::warn!("history: searching {query:?}: {e}");
        Vec::new()
    });
    cache.replace(Some((browse, query.to_string(), v.clone())));
    v
}

/// Something that waits for Enter, as it can't be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    DeleteSpace,
    ClearHistory,
}

/// Local time in ms for a UTC time in ms (the zone's offset then).
fn local_ms(utc_ms: i64) -> i64 {
    use chrono::{Local, Offset, TimeZone};
    let offset = Local.timestamp_millis_opt(utc_ms).single().map_or(0, |d| d.offset().fix().local_minus_utc());
    utc_ms + i64::from(offset) * 1000
}

/// Opens a file or folder in the desktop's app for it (`xdg-open`, or
/// `TRAILBOARD_OPENER` for tests), without waiting for it.
fn open_externally(path: &str) {
    let opener = std::env::var("TRAILBOARD_OPENER").ok().filter(|o| !o.is_empty()).unwrap_or_else(|| "xdg-open".into());
    let path = path.to_string();
    std::thread::spawn(move || {
        let run = std::process::Command::new(&opener)
            .arg(&path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status();
        match run {
            Ok(s) if s.success() => {}
            Ok(s) => log::warn!("{opener} {path:?}: {s}"),
            Err(e) => log::warn!("{opener} {path:?}: {e}"),
        }
    });
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl App {
    pub fn new(initial_urls: Vec<String>, data_dir: std::path::PathBuf, settings: Settings) -> Self {
        let live_env = max_live_from_env();
        log::info!("settings: {settings:?}");
        let zooms = Rc::new(RefCell::new(storage::zoom::load_zooms(&data_dir)));
        let lookup = zooms.clone();
        web::set_page_zoom(move |url| lookup.borrow().get(url));
        let icons = storage::icons::load_icons(&data_dir)
            .into_iter()
            .enumerate()
            .map(|(i, (site, icon))| (site, SiteIcon { key: ICON_KEY | i as u64, serial: 1, icon, saved: true }))
            .collect();
        let data_dir_for_downloads = data_dir.clone();
        let page_index = match storage::pagetext::PageIndex::open(&data_dir) {
            Ok(i) => Some(i),
            Err(e) => {
                log::warn!("history is off: {e}");
                None
            }
        };
        Self {
            renderer: None,
            scene: Scene::new(),
            camera: Camera::default(),
            camera_goal: None,
            page_scroll: Default::default(),
            cards: HashMap::new(),
            max_live: live_env.unwrap_or(settings.live_pages),
            blocking: crate::blocking::Blocking::start(data_dir.clone(), settings.block_ads),
            settings,
            live_from_env: live_env.is_some(),
            used: HashMap::new(),
            use_clock: 0,
            focused: None,
            closed: Vec::new(),
            history: History::new(),
            selection: Selection::new(),
            group: Vec::new(),
            keys_in_page: false,
            omnibox: None,
            menu: None,
            hints: false,
            link_hints: None,
            arrange: None,
            read: None,
            sidebar: Default::default(),
            sidebar_open: false,
            sidebar_keys: false,
            sidebar_at: None,
            sidebar_hover: None,
            sidebar_press: None,
            menu_pressed: false,
            clipboard: Clipboard::new(),
            ime_allowed: false,
            ime_page: None,
            ime_area: None,
            spawn_beside: SpawnBeside::new(true),
            drag: Drag::None,
            cursor: Vec2::default(),
            mods: ModifiersState::empty(),
            last_click: None,
            click_count: 1,
            camera_changed: Instant::now(),
            page_scale: 1.0,
            initial_urls,
            started: Instant::now(),
            script: Script::from_env(),
            quit: false,
            data_dir,
            spaces: SpaceIndex::default(),
            persist: true,
            epoch: 0,
            sessions: HashMap::new(),
            jar: None,
            toast: None,
            sandbox: None,
            anim: Anims::default(),
            confirm: None,
            dirty: false,
            last_save: Instant::now(),
            theme: Theme::default(),
            desktop: None,
            page_scheme: None,
            note_target: None,
            saved: HashMap::new(),
            zooms,
            icons,
            pointer_in: false,
            finding: None,
            found: None,
            last_find: String::new(),
            menu_by_key: None,
            page_index,
            text_due: HashMap::new(),
            visits_cache: RefCell::new(None),
            preview: None,
            preview_serial: 0,
            thumb_tried: HashSet::new(),
            downloads: storage::downloads::load_downloads(&data_dir_for_downloads),
            download_ids: HashMap::new(),
            parked: Vec::new(),
        }
    }

    pub fn shutdown(&mut self) {
        self.save();
        self.stop_downloads();
        self.cards.clear(); // closes every browser before CEF shuts down
        self.parked.clear();
    }

    // ---------------------------------------------------------------- persistence

    fn board_path(&self) -> std::path::PathBuf {
        spaces::space_board_path(&self.data_dir, self.spaces.active)
    }

    fn save(&mut self) {
        if self.renderer.is_none() || !self.persist {
            return; // never initialized; don't clobber a saved board with an empty one
        }
        let board = storage::Board::new(self.scene.clone(), self.camera);
        let path = self.board_path();
        match storage::save_board(&path, &board) {
            Ok(()) => self.dirty = false,
            Err(e) => log::error!("saving board to {}: {e}", path.display()),
        }
        self.last_save = Instant::now();
    }

    fn save_index(&self) {
        if self.persist
            && let Err(e) = spaces::save_index(&self.data_dir, &self.spaces)
        {
            log::error!("saving the space index: {e}");
        }
    }

    fn autosave(&mut self) {
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(3) && matches!(self.drag, Drag::None) {
            self.save();
        }
    }

    /// Open the space index and load the active space. Returns false when it has no cards.
    fn restore(&mut self) -> bool {
        match spaces::open_spaces(&self.data_dir) {
            Ok(idx) => self.spaces = idx,
            Err(e) => {
                log::error!("{e}; running with one unsaved space");
                self.persist = false;
            }
        }
        log::info!(
            "space {} {:?} ({} of {})",
            self.spaces.active.0,
            self.spaces.active_entry().name,
            self.spaces.position(self.spaces.active).unwrap_or(0) + 1,
            self.spaces.spaces.len()
        );
        self.enter_privacy(None, None);
        self.load_board()
    }

    fn is_private(&self) -> bool {
        self.spaces.active_entry().private
    }

    /// Before the active space's pages open: a private space gets its cookie
    /// jar (the one it had this session, or a new one) and its page zooms.
    fn enter_privacy(&mut self, jar: Option<web::Jar>, zooms: Option<std::collections::BTreeMap<String, f64>>) {
        if !self.is_private() {
            self.jar = None;
            self.zooms.borrow_mut().private = None;
            return;
        }
        self.jar = jar.or_else(web::Jar::in_memory);
        if self.jar.is_none() {
            // Pages would share the profile's cookies; history etc. still stay off.
            log::error!("private space: couldn't make a cookie jar in memory");
        }
        self.zooms.borrow_mut().private = Some(zooms.unwrap_or_default());
        log::info!("private space: jar={}", self.jar.is_some());
    }

    /// Load the active space's board into the (empty) scene. Returns false when it has no cards.
    fn load_board(&mut self) -> bool {
        if !self.persist {
            return false;
        }
        let board_path = self.board_path();
        match storage::load_board(&board_path) {
            Ok(board) => {
                self.scene = board.scene;
                self.camera = board.camera;
                // Pages start asleep; `sync_liveness` wakes the ones on screen, up to the cap.
                // Stacking order stands in for recency (focusing brings a card to front).
                self.used = self.scene.all_nodes().map(|n| (n.id, n.z)).collect();
                self.use_clock = self.used.values().copied().max().unwrap_or(0);
                log::info!("restored {} cards from {}", self.scene.all_nodes().count(), board_path.display());
                !self.scene.is_empty()
            }
            Err(storage::LoadError::Missing) => false,
            Err(e) => {
                // Keep the unreadable file for the user instead of overwriting it.
                let aside = board_path.with_extension(format!("unreadable-{}.json", std::process::id()));
                log::error!("{e}; moving it to {}", aside.display());
                let _ = std::fs::rename(&board_path, &aside);
                false
            }
        }
    }

    // ---------------------------------------------------------------- spaces

    fn page_key(&self, id: NodeId) -> u64 {
        page_key(self.epoch, id)
    }

    /// The card a page key belongs to, if that page is from the current space.
    fn node_of(&self, key: u64) -> Option<NodeId> {
        (key >> KEY_ID_BITS == self.epoch).then_some(NodeId(key & ((1 << KEY_ID_BITS) - 1)))
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast_for(msg, TOAST_FOR);
    }

    fn toast_for(&mut self, msg: impl Into<String>, show: Duration) {
        let msg = msg.into();
        log::info!("toast: {msg}");
        self.toast = Some((msg, Instant::now() + show));
    }

    /// Tell, once, that the last run crashed and where its report is.
    pub fn note_crash(&mut self, log: &std::path::Path) {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let shown = match home.as_deref().and_then(|h| log.strip_prefix(h).ok()) {
            Some(rest) => format!("~/{}", rest.display()),
            None => log.display().to_string(),
        };
        // Longer than a toast: it comes up while the window is still opening.
        self.toast_for(format!("Trailboard closed unexpectedly last time · report in {shown}"), Duration::from_secs(8));
    }

    /// How the sandbox confines pages, for the settings panel. When the system
    /// can't run it, say so once (the log alone goes unread); the mark is
    /// cleared while it runs, so a later loss of support is told again.
    pub fn note_sandbox(&mut self, sandbox: web::sandbox::Sandbox) {
        if sandbox.is_on() {
            storage::marks::clear(&self.data_dir, storage::marks::SANDBOX_OFF_TOLD);
        } else if sandbox.unsupported()
            && self.toast.is_none() // the crash notice goes first; this waits for the next start
            && storage::marks::first_time(&self.data_dir, storage::marks::SANDBOX_OFF_TOLD)
        {
            self.toast_for("Pages run without Chromium's sandbox: this system doesn't support it · Ctrl+, for more", Duration::from_secs(10));
        }
        self.sandbox = Some(sandbox);
    }

    /// Enter or a click on a status line of the settings panel: what it means.
    fn explain_status(&mut self, status: palette::Status) {
        match status {
            palette::Status::Sandbox => {
                if let Some(s) = &self.sandbox {
                    self.toast(s.explain());
                }
            }
        }
    }

    /// "Name (2 of 3)" for the active space, "Name · Private" or
    /// "Name (2 of 3) · Private" when it is private.
    fn space_label(&self) -> String {
        let e = self.spaces.active_entry();
        let i = self.spaces.position(e.id).unwrap_or(0) + 1;
        let n = self.spaces.spaces.len();
        let mut label = if n > 1 { format!("{} ({i} of {n})", e.name) } else { e.name.clone() };
        if e.private {
            label.push_str(" · Private");
        }
        label
    }

    /// Save the current space and close its pages; keep its undo history for later.
    fn unload_space(&mut self) {
        self.finish_arrange(true);
        self.save();
        let keys: Vec<u64> = self.scene.all_nodes().map(|n| self.page_key(n.id)).collect();
        if let Some(r) = self.renderer.as_mut() {
            for k in keys {
                r.remove_page(k);
            }
        }
        for (_, card) in std::mem::take(&mut self.cards) {
            self.release(card.page); // closes the browsers
        }
        let session = Session {
            history: std::mem::take(&mut self.history),
            closed: std::mem::take(&mut self.closed),
            jar: self.jar.take(),
            zooms: self.zooms.borrow_mut().private.take(),
        };
        self.sessions.insert(self.spaces.active, session);
        self.scene = Scene::new();
        self.camera = Camera::default();
        self.camera_goal = None;
        self.used.clear();
        self.use_clock = 0;
        self.focused = None;
        self.note_target = None;
        self.keys_in_page = false;
        self.selection.clear();
        self.group.clear();
        self.drag = Drag::None;
        self.omnibox = None;
        self.menu = None;
        self.read = None;
        // Collapsed trails and the highlight are this space's; the panel stays open.
        self.sidebar = Default::default();
        self.sidebar_keys = false;
        self.sidebar_at = None;
        self.sidebar_hover = None;
        self.sidebar_press = None;
        self.ime_page = None;
        self.confirm = None;
        self.epoch += 1;
        self.text_due.clear();
        // Anything the old pages still had queued is for cards that are gone.
        drop(web::drain_events());
        self.dirty = false;
    }

    /// Show the active space (after `unload_space`).
    fn load_space(&mut self) {
        let s = self.sessions.remove(&self.spaces.active).unwrap_or_default();
        self.history = s.history;
        self.closed = s.closed;
        self.enter_privacy(s.jar, s.zooms);
        self.load_board();
        self.camera_moved();
        self.dirty = false;
        log::info!("space {} {:?}: {} cards", self.spaces.active.0, self.spaces.active_entry().name, self.scene.nodes().count());
        self.toast(self.space_label());
    }

    fn switch_space(&mut self, to: SpaceId) {
        self.link_hints = None;
        if to == self.spaces.active {
            self.toast(format!("{} is the only space · Ctrl+Shift+N makes another", self.spaces.active_entry().name));
            return;
        }
        self.unload_space();
        self.spaces.active = to;
        self.save_index();
        self.load_space();
    }

    fn new_space(&mut self, private: bool) {
        self.link_hints = None;
        let id = self.spaces.add();
        self.spaces.set_private(id, private);
        self.unload_space();
        self.spaces.active = id;
        self.save_index();
        self.load_space();
    }

    fn rename_space(&mut self, name: &str) {
        let id = self.spaces.active;
        if self.spaces.rename(id, name) {
            self.save_index();
            self.toast(format!("Renamed to {}", self.spaces.active_entry().name));
        }
    }

    /// Ctrl+Shift+P: make the active space private, or recorded again. Its
    /// pages reopen, so they move to (or out of) the private cookie jar.
    fn toggle_private(&mut self) {
        self.link_hints = None;
        let id = self.spaces.active;
        let on = !self.is_private();
        self.unload_space();
        self.spaces.set_private(id, on);
        self.save_index();
        if !on && let Some(s) = self.sessions.get_mut(&id) {
            // Leaving private: its cookies and zooms are forgotten now.
            s.jar = None;
            s.zooms = None;
        }
        self.load_space();
        log::info!("space {} private={on}", id.0);
        self.toast(if on {
            format!("{} is private · its pages aren't kept in history, cookies last until you quit", self.spaces.active_entry().name)
        } else {
            format!("{} is no longer private · its pages are kept in history", self.spaces.active_entry().name)
        });
    }

    fn cycle_tint(&mut self) {
        self.spaces.cycle_tint(self.spaces.active);
        self.save_index();
        log::info!("space {} tint {:?}", self.spaces.active.0, self.spaces.active_entry().tint);
    }

    // ---------------------------------------------------------------- look

    /// Window scale times the desktop's text size: overlays are laid out in this.
    fn ui(&self) -> f32 {
        self.scale() * self.theme.text_scale
    }

    /// Start following the desktop's look (once the window exists).
    fn start_desktop(&mut self) {
        let d = Desktop::new(crate::theme::state_dir(), crate::theme::user_shell_toml(), true);
        let t = d.read(self.theme.rounding);
        self.desktop = Some(d);
        self.set_theme(t);
    }

    /// Tells pages light or dark, from the theme and the *Page colours* setting.
    fn apply_page_scheme(&mut self) {
        let dark = self.settings.page_colours.dark(self.theme.light());
        if self.page_scheme != Some(dark) {
            log::info!("pages: {}", if dark { "dark" } else { "light" });
            web::set_color_scheme(dark);
            for c in self.cards.values() {
                c.page.apply_color_scheme();
            }
            self.page_scheme = Some(dark);
        }
    }

    fn set_theme(&mut self, t: Theme) {
        log::info!(
            "theme: {} accent={:?} background={:?} rounding={} text-scale={:.2}",
            if t.light() { "light" } else { "dark" },
            t.accent(),
            t.background(),
            t.rounding,
            t.text_scale
        );
        self.theme = t;
        self.apply_page_scheme();
        let notes: Vec<NodeId> = self.cards.keys().copied().collect();
        for id in notes {
            self.recolour_note(id);
        }
        // The open menu was laid out at the old text size.
        self.menu = None;
    }

    fn follow_desktop(&mut self) {
        if let Some(t) = self.desktop.as_mut().and_then(|d| d.poll(&self.theme)) {
            self.set_theme(t);
        }
    }

    /// Ctrl+Shift+W: delete the space, asking first unless it is empty.
    fn ask_delete_space(&mut self) {
        let e = self.spaces.active_entry();
        if self.spaces.spaces.len() < 2 {
            self.toast(format!("{} is the only space; it can't be deleted", e.name));
        } else if self.scene.is_empty() {
            self.delete_space();
        } else {
            let n = self.scene.nodes().count();
            let cards = if n == 1 { "1 card".to_string() } else { format!("{n} cards") };
            self.confirm = Some(Confirm::DeleteSpace);
            self.toast(format!("Delete {} and its {cards}? Enter deletes · Esc keeps it", e.name));
        }
    }

    fn delete_space(&mut self) {
        self.confirm = None;
        let gone = self.spaces.active;
        let name = self.spaces.active_entry().name.clone();
        self.unload_space();
        self.sessions.remove(&gone);
        let Some(next) = self.spaces.remove(gone) else { return };
        if self.persist
            && let Err(e) = spaces::discard_space(&self.data_dir, gone)
        {
            log::error!("setting aside the board of deleted space {}: {e}", gone.0);
        }
        self.spaces.active = next;
        self.save_index();
        self.load_space();
        self.toast(format!("Deleted {name} · now on {}", self.space_label()));
    }

    // ---------------------------------------------------------------- geometry

    fn window(&self) -> Option<&Arc<Window>> {
        self.renderer.as_ref().map(|r| r.window())
    }

    fn scale(&self) -> f32 {
        self.window().map_or(1.0, |w| w.scale_factor() as f32)
    }

    fn viewport(&self) -> Vec2 {
        self.renderer.as_ref().map_or(Vec2::new(1.0, 1.0), |r| {
            let (w, h) = r.viewport();
            Vec2::new(w, h)
        })
    }

    fn to_world(&self, p: Vec2) -> Vec2 {
        self.camera.screen_to_world(p, self.viewport(), self.scale())
    }

    fn to_screen_rect(&self, r: Rect) -> Rect {
        self.camera.rect_to_screen(r, self.viewport(), self.scale())
    }

    fn read_layout(&self) -> read::ReadLayout {
        read::layout(self.viewport(), self.scale(), READ_BAR * self.ui())
    }

    /// Where card `id`'s page is drawn (screen px) and its size in CSS px.
    fn page_view(&self, id: NodeId) -> Option<(Rect, Vec2)> {
        if self.read == Some(id) {
            let l = self.read_layout();
            return Some((l.content, Vec2::new(l.css.0 as f32, l.css.1 as f32)));
        }
        let n = self.scene.node(id)?;
        Some((self.to_screen_rect(n.rect), Vec2::new(n.rect.w, n.rect.h)))
    }

    /// Screen point → CSS px in card `id`'s page.
    fn page_point(&self, id: NodeId, screen: Vec2) -> Option<(i32, i32)> {
        let (r, css) = self.page_view(id)?;
        let k = css.x / r.w.max(1e-3);
        Some((((screen.x - r.x) * k).round() as i32, ((screen.y - r.y) * k).round() as i32))
    }

    /// A rect in card `id`'s page (CSS px) → screen px.
    fn page_rect_to_screen(&self, id: NodeId, p: Rect) -> Option<Rect> {
        let (r, css) = self.page_view(id)?;
        let k = r.w / css.x.max(1e-3);
        Some(Rect::new(r.x + p.x * k, r.y + p.y * k, p.w * k, p.h * k))
    }

    fn camera_moved(&mut self) {
        self.camera_changed = Instant::now();
        self.dirty = true;
        // The menu belongs to a spot on a page; it shouldn't float away from it.
        self.menu = None;
    }

    // ---------------------------------------------------------------- cards

    fn open_card(&mut self, rect: Rect, url: &str, from: Option<NodeId>) -> Option<NodeId> {
        let id = match from {
            Some(src) => self.scene.spawn_beside(src, url)?,
            None => self.scene.add_page(rect, url),
        };
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        Some(id)
    }

    /// Mark a card as just used, so it is among the last to be put to sleep.
    fn touch(&mut self, id: NodeId) {
        self.use_clock += 1;
        self.used.insert(id, self.use_clock);
    }

    /// Start the live browser for an existing card (no-op if it is already live).
    fn create_page(&mut self, id: NodeId) {
        self.touch(id);
        if self.cards.contains_key(&id) {
            return;
        }
        let Some(n) = self.scene.node(id) else { return };
        let r = n.rect;
        let (url, kind) = match &n.note {
            Some(text) => (crate::note::page_url(text, &self.theme, n.sticky.as_deref()), web::PageKind::Note),
            None => (n.url.clone(), web::PageKind::Web),
        };
        let jar = self.jar.as_ref();
        match Page::new(self.page_key(id), &url, kind, (r.w as i32, r.h as i32), self.page_scale, &self.spawn_beside, jar) {
            Some(page) => {
                self.cards.insert(id, Card { page, loading: true, hidden: false, cursor: web::Cursor::Default });
            }
            None => log::error!("failed to create browser for card {}", id.0),
        }
    }

    /// First start: the welcome note, left of the first page (`beside`).
    /// Not focused, so the page keeps the keys.
    fn add_guide(&mut self, beside: Option<NodeId>) -> NodeId {
        let (w, h) = (crate::note::GUIDE_W, crate::note::GUIDE_H);
        let rect = match beside.and_then(|id| self.scene.node(id)) {
            Some(src) => placement::left_of(&self.scene, src, w, h),
            None => placement::free_near(&self.scene, self.camera.center, w, h),
        };
        let id = self.scene.add_note(rect, crate::note::GUIDE);
        self.create_page(id);
        self.dirty = true;
        log::info!("first run: welcome note {}", id.0);
        id
    }

    /// *Welcome note* (Ctrl+K): go to the welcome note if an unedited one is
    /// on the board (unfolding it if needed), else add one in the middle of the view.
    fn show_guide(&mut self) {
        if let Some(id) = self.scene.note_with_text(crate::note::GUIDE) {
            log::info!("welcome note {} already on the board", id.0);
            self.go_to_card(id);
            return;
        }
        let (w, h) = (crate::note::GUIDE_W, crate::note::GUIDE_H);
        let rect = placement::free_near(&self.scene, self.camera.center, w, h);
        let id = self.scene.add_note(rect, crate::note::GUIDE);
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.focus(Some(id));
        self.ensure_visible(id);
        log::info!("welcome note {} added", id.0);
    }

    /// N: a new, empty note in the middle of the view, ready to type in.
    fn new_note_at_center(&mut self) -> NodeId {
        let rect = placement::free_near(&self.scene, self.camera.center, crate::note::W, crate::note::H);
        let id = self.scene.add_note(rect, "");
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.note_target = Some(id);
        self.focus(Some(id));
        self.ensure_visible(id);
        self.keys_in_page = true;
        log::info!("new note {}", id.0);
        id
    }

    /// Shift+N: a new, empty sticky note in the middle of the view, ready to type in.
    fn new_sticky_at_center(&mut self) -> NodeId {
        let (w, h) = (crate::note::STICKY_W, crate::note::STICKY_H);
        let rect = placement::free_near(&self.scene, self.camera.center, w, h);
        let id = self.scene.add_sticky(rect, "");
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.note_target = Some(id);
        self.focus(Some(id));
        self.ensure_visible(id);
        self.keys_in_page = true;
        log::info!("new sticky note {}", id.0);
        id
    }

    /// *Make sticky note* / *Make note card* on note `id`. Undoable.
    fn toggle_sticky(&mut self, id: NodeId) {
        let Some(n) = self.scene.node(id).filter(|n| n.is_note()) else { return };
        let colour = if n.is_sticky() { None } else { Some(scene::note::STICKY_COLOURS[0].to_string()) };
        let old = self.scene.set_sticky(id, colour);
        self.history.restyled(id, old);
        self.restyled(id);
        let sticky = self.scene.node(id).is_some_and(Node::is_sticky);
        log::info!("note {} sticky {sticky}", id.0);
    }

    /// C on a sticky note: its next colour. Undoable.
    fn cycle_sticky_colour(&mut self, id: NodeId) {
        let old = self.scene.cycle_sticky_colour(id);
        self.history.restyled(id, old);
        self.restyled(id);
        let colour = self.scene.node(id).and_then(|n| n.sticky.clone()).unwrap_or_default();
        log::info!("sticky note {} colour {colour}", id.0);
        self.toast(format!("Sticky note: {colour}"));
    }

    /// Note `id` became or stopped being a sticky note, or changed colour.
    fn restyled(&mut self, id: NodeId) {
        self.recolour_note(id);
        self.dirty = true;
    }

    /// Gives a live note's editor the colours of the theme and its sticky colour.
    fn recolour_note(&self, id: NodeId) {
        let (Some(n), Some(c)) = (self.scene.node(id), self.cards.get(&id)) else { return };
        if n.is_note() && c.page.kind() == web::PageKind::Note {
            c.page.run_script(&crate::note::theme_script(&self.theme, n.sticky.as_deref()));
        }
    }

    /// Ctrl+S or *Save as Markdown*: the note `note` as its own file, else
    /// the whole board, in `Documents/Trailboard` (`storage::export`).
    fn save_markdown(&mut self, note: Option<NodeId>) {
        let space = self.spaces.active_entry().name.clone();
        let (title, text, fallback) = match note.and_then(|id| self.scene.node(id)) {
            Some(n) => {
                let text = scene::export::note_markdown(n.note.as_deref().unwrap_or(""));
                (n.title.clone(), text, scene::note::UNTITLED)
            }
            None => (space.clone(), scene::export::board_markdown(&self.scene, &space), "Board"),
        };
        let key = (self.spaces.active, note);
        let dir = storage::export::export_dir();
        let stem = storage::export::file_stem(&title, fallback);
        match storage::export::save_markdown(&dir, &stem, &text, self.saved.get(&key).map(|p| p.as_path())) {
            Ok(path) => {
                log::info!("saved {} as Markdown to {}", if note.is_some() { "note" } else { "board" }, path.display());
                self.toast(format!("Saved to {}", home_relative(&path)));
                self.saved.insert(key, path);
            }
            Err(e) => {
                log::error!("saving Markdown to {}: {e}", dir.display());
                self.toast(format!("Could not save to {}: {e}", home_relative(&dir)));
            }
        }
    }

    /// "Add to note": quote `selection` from card `src` in the note last used,
    /// or in a new note beside `src` when there is none.
    fn add_to_note(&mut self, src: NodeId, selection: &str) {
        let Some(n) = self.scene.node(src) else { return };
        let Some(quote) = scene::note::quote(selection, &n.title, &n.url) else { return };
        let target = self.note_target.filter(|&t| t != src && self.scene.node(t).is_some_and(Node::is_note));
        let id = match target {
            Some(t) => {
                let old = self.scene.node(t).and_then(|n| n.note.clone()).unwrap_or_default();
                let text = scene::note::append(&old, &quote);
                self.scene.set_note(t, text.clone());
                if let Some(c) = self.cards.get(&t) {
                    c.page.run_script(&crate::note::set_text_script(&text));
                }
                t
            }
            None => {
                let Some(t) = self.scene.note_beside(src, quote, crate::note::W, crate::note::H) else { return };
                self.create_page(t);
                self.history.opened(t);
                // While reading, the new note waits on the board (not read next).
                if self.read.is_none() {
                    self.ensure_visible(t);
                }
                t
            }
        };
        self.dirty = true;
        self.note_target = Some(id);
        let title = self.scene.node(id).map(|n| n.title.clone()).unwrap_or_default();
        log::info!("added a quote from card {} to note {}", src.0, id.0);
        self.toast(format!("Added to {}", menu::cut(&title, menu::OUTLINE_CHARS)));
    }

    fn new_card_at_center(&mut self, url: &str) -> Option<NodeId> {
        let center = self.camera.center;
        let rect = placement::free_near(&self.scene, center, CARD_W, CARD_H);
        let id = self.open_card(rect, url, None)?;
        self.focus(Some(id));
        self.ensure_visible(id);
        Some(id)
    }

    /// Trailboard was started again while running: open what it was given as
    /// new cards in the space on screen and ask for the window to come forward.
    fn relaunched(&mut self, args: &[String], cwd: &str) {
        let urls: Vec<String> = args.iter().filter_map(|a| launch::target(a, std::path::Path::new(cwd), self.settings.search)).collect();
        log::info!("relaunched with {} page(s): {urls:?}", urls.len());
        if !urls.is_empty() && self.read.is_some() {
            self.leave_read();
        }
        for url in &urls {
            self.new_card_at_center(url);
        }
        if let Some(w) = self.window() {
            w.focus_window();
        }
    }

    fn close_card(&mut self, id: NodeId) {
        self.close_cards(&[id]);
    }

    /// Closes cards as one undo step; each can still be reopened with Ctrl+Shift+T.
    fn close_cards(&mut self, ids: &[NodeId]) {
        let mut gone = Vec::new();
        for &id in ids {
            if let Some(removed) = self.scene.remove(id) {
                self.closed.push(removed.clone());
                gone.push(removed);
            }
            self.drop_page(id);
        }
        self.history.closed_many(gone);
    }

    /// Put a card to sleep: close its browser but keep its last frame on screen.
    fn sleep_page(&mut self, id: NodeId) {
        if let Some(card) = self.cards.remove(&id) {
            self.release(card.page);
            log::info!("sleeping card {} ({} live)", id.0, self.cards.len());
        }
    }

    /// Stop the live browser of a card that has left the scene.
    fn drop_page(&mut self, id: NodeId) {
        self.dirty = true;
        if let Some(card) = self.cards.remove(&id) {
            self.release(card.page);
        }
        self.used.remove(&id);
        self.selection.remove(id);
        let key = self.page_key(id);
        self.thumb_tried.remove(&key);
        if let Some(r) = self.renderer.as_mut() {
            r.remove_page(key);
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        if self.read == Some(id) {
            log::info!("read mode: card {} is gone; back to the board", id.0);
            self.read = None;
            self.camera_moved();
        }
    }

    /// Ctrl+Shift+T: bring back the newest closed card that isn't already back (via undo).
    fn reopen_closed(&mut self) {
        while let Some((node, edges)) = self.closed.pop() {
            if self.restore_closed(node, edges) {
                return;
            }
        }
    }

    /// A closed card picked in the palette: bring back that one, where it was.
    fn reopen_closed_card(&mut self, id: NodeId) {
        let Some(i) = self.closed.iter().rposition(|(n, _)| n.id == id) else {
            log::info!("reopen closed card {}: no longer closed", id.0);
            return;
        };
        let (node, edges) = self.closed.remove(i);
        // Older entries for the same card (closed, undone, closed again) go too.
        self.closed.retain(|(n, _)| n.id != id);
        if !self.restore_closed(node, edges) {
            log::info!("reopen closed card {}: already on the board", id.0);
        }
    }

    /// Puts a closed card back on the board, as one undo step; false if it is there already.
    fn restore_closed(&mut self, node: Node, edges: Vec<Edge>) -> bool {
        let id = node.id;
        if !self.scene.insert(node, &edges) {
            return false;
        }
        log::info!("reopened closed card {}", id.0);
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.focus(Some(id));
        self.ensure_visible(id);
        true
    }

    fn undo(&mut self, redo: bool) {
        self.finish_arrange(true);
        let changes = if redo { self.history.redo(&mut self.scene) } else { self.history.undo(&mut self.scene) };
        log::info!("{}: {changes:?}", if redo { "redo" } else { "undo" });
        let mut shown = None;
        for change in changes {
            match change {
                Change::Added(id) => {
                    self.create_page(id);
                    shown = Some(id);
                }
                Change::Removed(id) => self.drop_page(id),
                Change::Moved(id) => shown = Some(id),
                Change::Restyled(id) => {
                    self.recolour_note(id);
                    shown = Some(id);
                }
            }
            self.dirty = true;
        }
        if let Some(id) = shown {
            self.ensure_visible(id);
        }
    }

    fn focus(&mut self, id: Option<NodeId>) {
        if self.focused == id {
            return;
        }
        self.keys_in_page = false;
        self.link_hints = None;
        self.finish_arrange(true);
        if let Some(old) = self.focused.and_then(|o| self.cards.get(&o)) {
            old.page.set_focus(false);
        }
        self.focused = id;
        if let Some(id) = id {
            if self.scene.node(id).is_some_and(Node::is_note) {
                self.note_target = Some(id);
            }
            self.scene.bring_to_front(id);
            self.dirty = true;
            if !self.cards.contains_key(&id) {
                log::info!("waking card {} on focus", id.0);
            }
            self.create_page(id); // wakes a sleeping card; marks it used either way
            if let Some(c) = self.cards.get(&id) {
                c.page.set_focus(true);
            }
        }
    }

    fn focused_page(&self) -> Option<&Page> {
        self.focused.and_then(|id| self.cards.get(&id)).map(|c| &c.page)
    }

    /// Keys go to the focused page (after a click in it, Enter, or in Read
    /// Mode); otherwise they are board keys and the focused card is just the
    /// current one.
    fn typing(&self) -> bool {
        self.focused_page().is_some() && (self.keys_in_page || self.read.is_some())
    }

    /// The card board keys act on: the current one, else the one in the
    /// middle of the view.
    fn current(&self) -> Option<NodeId> {
        read::pick(&self.scene, self.focused, self.camera.center)
    }

    /// h/j/k/l or arrows: make the nearest card that way current and bring it
    /// on screen. With no current card, the one in the middle of the view
    /// becomes current first.
    fn move_current(&mut self, dir: Dir) {
        let to = match self.focused.filter(|&id| self.scene.node(id).is_some()) {
            Some(from) => nav::neighbour(&self.scene, from, dir),
            None => self.current(),
        };
        let Some(to) = to else { return };
        // The selection stays: v adds cards to it as you go.
        self.focus(Some(to));
        self.ensure_visible(to);
    }

    /// Shift+h/j/k/l or Shift+arrows: pan a quarter of the window.
    fn pan_by_key(&mut self, dir: Dir) {
        if self.read.is_some() {
            return;
        }
        let vp = self.viewport();
        let (dx, dy) = match dir {
            Dir::Left => (vp.x / 4.0, 0.0),
            Dir::Right => (-vp.x / 4.0, 0.0),
            Dir::Up => (0.0, vp.y / 4.0),
            Dir::Down => (0.0, -vp.y / 4.0),
        };
        self.camera_goal = None;
        self.camera.pan_pixels(Vec2::new(dx, dy), self.scale());
        self.camera_moved();
    }

    /// + / -: zoom about the middle of the window, as one Ctrl+scroll notch does.
    fn zoom_by_key(&mut self, zoom_in: bool) {
        let factor = if zoom_in { 1.25 } else { 1.0 / 1.25 };
        self.camera_goal = None;
        let vp = self.viewport();
        self.camera.zoom_about(factor, vp * 0.5, vp, self.scale());
        self.camera_moved();
    }

    /// Enter or Space: give the keys to the current card's page (a note opens
    /// for writing).
    fn enter_current(&mut self) {
        let Some(id) = self.current() else { return };
        self.selection.clear();
        self.focus(Some(id));
        self.ensure_visible(id);
        self.keys_in_page = true;
        if let Some(c) = self.cards.get(&id) {
            c.page.set_focus(true);
            if self.scene.node(id).is_some_and(Node::is_note) {
                c.page.run_script(crate::note::EDIT_SCRIPT);
            }
        }
        log::info!("entered card {}", id.0);
    }

    /// Esc in a page: keys go back to the board, the card stays current.
    fn leave_page(&mut self) {
        self.keys_in_page = false;
        if let Some(p) = self.focused_page() {
            p.set_focus(false);
        }
        log::info!("left the page of card {:?}", self.focused.map(|f| f.0));
    }

    // ---------------------------------------------------------------- sidebar

    fn sidebar_panel(&self) -> Panel {
        Panel::new(self.viewport(), self.ui())
    }

    /// The sidebar is on screen (not in Read Mode, which covers the window).
    fn sidebar_shown(&self) -> bool {
        self.sidebar_open && self.read.is_none()
    }

    /// The sidebar takes board keys: open, given the keys, and no page has them.
    fn sidebar_has_keys(&self) -> bool {
        self.sidebar_shown() && self.sidebar_keys && !self.typing()
    }

    /// B: opens the sidebar with the keys on the current card's row; closes
    /// it when it has the keys; gives it the keys when it is open without.
    fn toggle_sidebar(&mut self) {
        if self.read.is_some() {
            return;
        }
        if self.sidebar_open && self.sidebar_keys {
            self.close_sidebar();
            return;
        }
        self.sidebar_open = true;
        self.take_sidebar_keys();
        log::info!("sidebar open, highlight {:?}", self.sidebar.sel);
    }

    fn close_sidebar(&mut self) {
        self.sidebar_open = false;
        self.sidebar_keys = false;
        self.sidebar_hover = None;
        self.sidebar_press = None;
        log::info!("sidebar closed");
    }

    /// The sidebar takes the keys, highlighting the current card's row.
    fn take_sidebar_keys(&mut self) {
        if self.typing() {
            self.leave_page();
        }
        self.sidebar_keys = true;
        let current = self.focused.or_else(|| self.current());
        if !current.is_some_and(|id| self.sidebar.select_card(&self.scene, id)) && self.sidebar.sel.is_none() {
            let rows = self.sidebar.rows(&self.scene);
            self.sidebar.step(&rows, 1, None);
        }
        self.sidebar_scroll_to_sel();
    }

    fn sidebar_scroll_to_sel(&mut self) {
        let rows = self.sidebar.rows(&self.scene);
        let i = self.sidebar.index(&rows);
        if i.is_some() {
            self.sidebar_at = i;
        }
        let shown = self.sidebar_panel().shown;
        self.sidebar.scroll_to(i, rows.len(), shown);
    }

    /// Keys while the sidebar has them. False for keys it leaves to the board.
    fn sidebar_key(&mut self, code: Option<KeyCode>, shift: bool) -> bool {
        use scene::sidebar::{Row, RowKey};
        let rows = self.sidebar.rows(&self.scene);
        if shift {
            // Shift+J/K or Shift+↑/↓ move the highlighted row's trail; other
            // Shift keys (Shift+H/L pan) stay board keys.
            let step = match code {
                Some(KeyCode::KeyJ | KeyCode::ArrowDown) => 1,
                Some(KeyCode::KeyK | KeyCode::ArrowUp) => -1,
                _ => return false,
            };
            self.sidebar_move_trail(&rows, step);
            self.sidebar_scroll_to_sel();
            return true;
        }
        let shown = self.sidebar_panel().shown as isize;
        let step = match code {
            Some(KeyCode::KeyJ | KeyCode::ArrowDown) => Some(1),
            Some(KeyCode::KeyK | KeyCode::ArrowUp) => Some(-1),
            Some(KeyCode::PageDown) => Some(shown),
            Some(KeyCode::PageUp) => Some(-shown),
            Some(KeyCode::Home) => Some(-(rows.len() as isize)),
            Some(KeyCode::End) => Some(rows.len() as isize),
            _ => None,
        };
        if let Some(step) = step {
            // Moving onto a card goes to it on the board (it becomes current).
            if let Some(RowKey::Card(id)) = self.sidebar.step(&rows, step, self.sidebar_at) {
                self.sidebar_go(id);
            }
            self.sidebar_scroll_to_sel();
            return true;
        }
        match code {
            Some(KeyCode::KeyH | KeyCode::ArrowLeft) => {
                self.sidebar.collapse(&rows);
            }
            Some(KeyCode::KeyL | KeyCode::ArrowRight) => {
                self.sidebar.expand(&rows);
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space) => {
                match self.sidebar.index(&rows).map(|i| &rows[i]) {
                    Some(Row::Header { group, .. }) => self.sidebar.toggle(*group),
                    Some(&Row::Card { id, .. }) => {
                        // The card's page gets the keys; the panel stays open.
                        self.sidebar_go(id);
                        self.sidebar_keys = false;
                        self.enter_current();
                    }
                    None => {}
                }
            }
            Some(KeyCode::KeyP) => self.sidebar_pin(&rows),
            Some(KeyCode::Escape | KeyCode::KeyB) => self.close_sidebar(),
            _ => return false,
        }
        self.sidebar_scroll_to_sel();
        true
    }

    /// Shift+J/K: moves the trail of the highlighted row `step` places among
    /// the pinned or the other trails. The highlight stays on its row.
    fn sidebar_move_trail(&mut self, rows: &[scene::sidebar::Row], step: isize) {
        use scene::sidebar;
        let Some(head) = self.sidebar.index(rows).and_then(|i| sidebar::trail_head(&rows[i])) else {
            self.toast("Only trails can be moved");
            return;
        };
        let Some((at, _)) = sidebar::position(&self.scene, head) else { return };
        let to = (at as isize + step).max(0) as usize;
        self.move_trail(head, to);
    }

    /// Moves the trail headed by `head` to index `to` of its part, undoably.
    fn move_trail(&mut self, head: NodeId, to: usize) {
        let old = scene::sidebar::move_group(&mut self.scene, head, to);
        if old.is_empty() {
            return;
        }
        self.history.marked(old);
        self.dirty = true;
        let at = scene::sidebar::position(&self.scene, head).map(|(i, _)| i);
        log::info!("sidebar: trail of card {} moved to {at:?}", head.0);
    }

    /// p: pins the trail of the highlighted row (a card on no trail on its
    /// own) to the top of the list, or unpins it.
    fn sidebar_pin(&mut self, rows: &[scene::sidebar::Row]) {
        use scene::sidebar::{self, Group, Row};
        let id = match self.sidebar.index(rows).map(|i| &rows[i]) {
            Some(&Row::Card { id, .. }) => id,
            Some(&Row::Header { group: Group::Trail(h), .. }) => h,
            _ => {
                self.toast("Pick a trail or a card to pin");
                return;
            }
        };
        let (old, pinned) = sidebar::toggle_pin(&mut self.scene, id);
        self.history.marked(old);
        self.dirty = true;
        log::info!("sidebar: trail of card {} pinned={pinned}", id.0);
        self.toast(if pinned { "Pinned to the top" } else { "Unpinned" });
    }

    /// Goes to card `id` from the sidebar: it is shown at its stack's front if
    /// folded away, becomes current and comes on screen (clear of the panel).
    fn sidebar_go(&mut self, id: NodeId) {
        self.go_to_card(id);
        log::info!("sidebar: card {}", id.0);
    }

    /// Goes to card `id`: shown at its stack's front if folded away, made
    /// current and brought on screen (clear of the sidebar).
    fn go_to_card(&mut self, id: NodeId) {
        if self.scene.node(id).is_none() {
            return;
        }
        if self.scene.is_hidden(id) && self.scene.show(id) {
            self.after_fold();
            self.dirty = true;
        }
        self.selection.clear();
        self.focus(Some(id));
        self.ensure_visible(id);
    }

    // ---------------------------------------------------------------- find in page, page zoom

    /// Ctrl+F: the find bar for the current card's page, with the last search
    /// selected (and run again, so its matches show at once).
    fn open_find(&mut self) {
        let Some(id) = self.focused.filter(|id| self.cards.contains_key(id)) else {
            self.toast("Pick a page to find in first");
            return;
        };
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        let text = self.last_find.clone();
        self.omnibox = Some(Omnibox::new(&text, Target::Find(id)));
        self.found = None;
        if !text.is_empty() {
            self.cards[&id].page.find(&text, true, false);
        }
        log::info!("find open on card {}", id.0);
    }

    /// Keeps the page being searched in step with the find bar: a closed bar
    /// (Esc, a click, another box) stops the search, and the bar closes when
    /// its card stops being the current one or its page goes to sleep.
    fn sync_find(&mut self) {
        let mut open = self.omnibox.as_ref().and_then(|ob| match ob.target {
            Target::Find(id) => Some(id),
            _ => None,
        });
        if let Some(id) = open
            && (self.focused != Some(id) || !self.cards.contains_key(&id))
        {
            self.omnibox = None;
            open = None;
        }
        if self.finding != open {
            if let Some(c) = self.finding.and_then(|old| self.cards.get(&old)) {
                c.page.stop_finding();
            }
            if open.is_none() {
                self.found = None;
                log::info!("find closed");
            }
            self.finding = open;
        }
    }

    /// Ctrl +/− (`dir` ±1) or Ctrl+0 (`dir` 0): zoom the current card's page,
    /// and every card on the same site with it. With no page current, they zoom
    /// the board and fit everything instead.
    fn zoom_page(&mut self, dir: i32) {
        let url = self.focused.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note()).map(|n| n.url.clone());
        let Some(url) = url else {
            if dir == 0 {
                self.fit_all();
            } else if self.read.is_none() {
                self.zoom_by_key(dir > 0);
            }
            return;
        };
        let want = if dir == 0 { 1.0 } else { pagezoom::step(self.zooms.borrow().get(&url), dir) };
        if self.zooms.borrow_mut().set(&url, want) {
            for c in self.cards.values() {
                c.page.refresh_zoom();
            }
            if self.persist
                && !self.is_private()
                && let Err(e) = storage::zoom::save_zooms(&self.data_dir, &self.zooms.borrow())
            {
                log::error!("saving page zoom: {e}");
            }
        }
        let site = pagezoom::site_of(&url);
        let label = pagezoom::label(self.zooms.borrow().get(&url));
        log::info!("page zoom {site:?} {label}");
        let name = match site.as_str() {
            "" | "about" => "Zoom",
            "file" => "Local files",
            s => s,
        };
        self.toast(format!("{name} · {label}"));
    }

    // ---------------------------------------------------------------- palette

    /// Ctrl+K: the palette, over everything but Read Mode's page.
    fn open_palette(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::Palette));
        log::info!("palette open");
    }

    /// Runs a palette row (the palette is already closed).
    fn run_palette(&mut self, row: palette::Row) {
        log::info!("palette: {:?} {:?}", row.item, row.label);
        match row.item {
            Item::Card(id) => {
                if self.read.is_some() {
                    self.leave_read();
                }
                self.go_to_card(id);
            }
            Item::Space(id) => self.switch_space(id),
            Item::Open(url) => {
                if self.new_card_at_center(&url).is_some() {
                    self.keys_in_page = true;
                }
            }
            Item::Closed(id) => {
                if self.read.is_some() {
                    self.leave_read();
                }
                self.reopen_closed_card(id);
            }
            Item::Command(c) => self.run_command(c),
            Item::Setting(k) => self.step_setting(k, 1),
            Item::Download(key) => self.open_download(key),
            Item::Status(s) => self.explain_status(s),
        }
    }

    /// Ad blocking on card `id`'s site, for its page menu: `None` for notes,
    /// pages that aren't web pages, or while blocking is off in the settings.
    fn blocking_state(&self, id: NodeId) -> Option<web::menu::Blocking> {
        let n = self.scene.node(id).filter(|n| !n.is_note() && self.settings.block_ads)?;
        let site = scene::icon::site(&n.url)?;
        let card = self.cards.get(&id);
        let blocked = card.map_or(0, |c| c.page.blocked());
        let banner_rejected = card.is_some_and(|c| c.page.banner_rejected());
        Some(web::menu::Blocking { site, on: !self.blocking.allows(&n.url), blocked, banner_rejected })
    }

    /// Turns ad blocking off on card `id`'s site, or back on, and reloads the
    /// site's live pages so the change shows.
    fn toggle_blocking(&mut self, id: Option<NodeId>) {
        let Some(n) = id.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note()) else {
            self.toast("No page here");
            return;
        };
        if !self.settings.block_ads {
            self.toast("Blocking ads is off in Settings (Ctrl+,)");
            return;
        }
        let url = n.url.clone();
        let private = self.is_private();
        let Some((site, on)) = self.blocking.toggle(&url, private) else {
            self.toast("Ads are only blocked on web pages");
            return;
        };
        let mut reloaded = 0;
        for (cid, c) in &self.cards {
            let same = self.scene.node(*cid).is_some_and(|n| !n.is_note() && scene::privacy::covers(&site, &n.url));
            if same && c.page.kind() == web::PageKind::Web {
                c.page.reload_fresh();
                reloaded += 1;
            }
        }
        log::info!("blocking {} {site}: {reloaded} page(s) reloaded", if on { "back on for" } else { "off for" });
        self.toast(match (on, private) {
            (true, _) => format!("Blocking ads on {site} again"),
            (false, false) => format!("Ads allowed on {site}"),
            (false, true) => format!("Ads allowed on {site} until you quit"),
        });
    }

    /// Ctrl+,: the settings panel, a list of settings to step through.
    fn open_settings(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::Settings));
        log::info!("settings open");
    }

    /// Moves setting `key` to its next (`dir` > 0) or previous value, applies
    /// it at once and saves it.
    fn step_setting(&mut self, key: prefs::Key, dir: i32) {
        let mut s = self.settings;
        if key == prefs::Key::LivePages {
            s.live_pages = self.max_live; // step from what is in force
        }
        if !s.step(key, dir) {
            return;
        }
        self.settings = s;
        log::info!("setting {key:?} = {}", s.value(key));
        match key {
            prefs::Key::Search | prefs::Key::ScrollSpeed => {}
            prefs::Key::LivePages => {
                self.max_live = s.live_pages;
                self.live_from_env = false;
            }
            prefs::Key::History => {
                if !s.history {
                    self.text_due.clear();
                }
            }
            prefs::Key::PageColours => self.apply_page_scheme(),
            prefs::Key::BlockAds => web::shield::set_enabled(s.block_ads),
        }
        if let Err(e) = storage::settings::save_settings(&self.data_dir, &self.settings) {
            log::warn!("settings: saving: {e}");
            self.toast("Couldn't save settings");
        }
    }

    /// The rows under a list field: the palette's or the history view's.
    fn list_rows(&self, ob: &Omnibox) -> Vec<palette::Row> {
        if ob.target == Target::Settings {
            let mut notes = HashMap::new();
            if self.live_from_env {
                notes.insert(prefs::Key::LivePages, "TRAILBOARD_MAX_LIVE".to_string());
            }
            let shown = Settings { live_pages: self.max_live, ..self.settings };
            let status: Vec<_> = self.sandbox.iter().map(|s| (palette::Status::Sandbox, s.summary().to_string())).collect();
            return palette::settings_rows(&ob.text, &shown, &notes, &status);
        }
        if ob.target == Target::Downloads {
            let missing = |p: &str| std::fs::symlink_metadata(p).is_err();
            return palette::download_rows(&ob.text, &self.downloads, missing, local_ms(now_ms()), local_ms);
        }
        if ob.target == Target::History {
            let visits = visits_for(&self.page_index, &self.visits_cache, &ob.text, true);
            return palette::history_rows(&ob.text, &self.scene, &visits, local_ms(now_ms()), local_ms);
        }
        let visits = visits_for(&self.page_index, &self.visits_cache, &ob.text, false);
        let closed: Vec<&Node> = self.closed.iter().map(|(n, _)| n).collect();
        palette::rows(&ob.text, &self.scene, &self.used, &closed, &self.spaces, &visits, self.settings.search)
    }

    /// Ctrl+H: the history view, every page kept, newest first.
    fn open_history(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::History));
        log::info!("history open");
    }

    // ---------------------------------------------------------------- downloads

    /// Ctrl+J: the downloads view, newest first.
    fn open_downloads(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::Downloads));
        log::info!("downloads open");
    }

    /// Chromium wants to save a file from page `page`: it goes to the
    /// downloads folder under a free name, at once (no dialog). A card that
    /// was opened for the link and never showed a page goes away again.
    fn download_asked(&mut self, page: u64, id: u32, url: &str, suggested: &str, blank: bool) {
        // Chromium asks again when it resumes an interrupted download by
        // itself: same download, same file.
        if let Some(&(key, _)) = self.download_ids.get(&id)
            && let Some(d) = self.downloads.get(key)
        {
            log::info!("download {id}: resuming into {}", d.path);
            web::download::save_to(id, std::path::Path::new(&d.path));
            return;
        }
        let dir = storage::downloads::download_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("downloads: making {}: {e}", dir.display()); // Chromium will fail it
        }
        let taken: HashSet<std::path::PathBuf> = self.downloads.running_paths().map(std::path::PathBuf::from).collect();
        let path = storage::downloads::target(&dir, suggested, &taken);
        if !web::download::save_to(id, &path) {
            return;
        }
        let key = self.downloads.start(url, &path.to_string_lossy(), self.is_private(), now_ms());
        self.download_ids.insert(id, (key, page));
        log::info!("download {id}: {url} -> {}", path.display());
        self.save_downloads();
        let name = self.downloads.get(key).map(|d| d.name().to_string()).unwrap_or_default();
        self.toast(format!("Downloading {name} · Ctrl+J"));
        if blank
            && let Some(card) = self.node_of(page)
            && self.scene.node(card).is_some_and(|n| !n.is_note())
        {
            // Nothing to show or undo: the page stays parked until the file is in.
            log::info!("closing card {} (it only downloaded a file)", card.0);
            let source = self.scene.edges().iter().find(|e| e.to == card).map(|e| e.from);
            let was_current = self.focused == Some(card);
            self.scene.remove(card);
            self.drop_page(card);
            // The view moved to the new card when it opened; go back to the
            // page the link was on.
            if let Some(src) = source.filter(|&s| self.scene.node(s).is_some()) {
                self.ensure_visible(src);
                if was_current {
                    self.focus(Some(src));
                }
            }
        }
    }

    fn download_updated(&mut self, id: u32, received: i64, total: i64, progress: web::download::Progress) {
        use scene::download::State;
        let Some(&(key, _)) = self.download_ids.get(&id) else { return };
        let state = match progress {
            web::download::Progress::Running => State::Running,
            web::download::Progress::Done => State::Done,
            web::download::Progress::Cancelled => State::Cancelled,
            web::download::Progress::Failed => State::Failed,
        };
        let Some(ended) = self.downloads.update(key, received, total, state) else { return };
        self.download_ids.remove(&id);
        let name = self.downloads.get(key).map(|d| d.name().to_string()).unwrap_or_default();
        log::info!("download {id}: {} {received} bytes", ended.id());
        match ended {
            State::Done => self.toast(format!("Downloaded {name} · Ctrl+J")),
            State::Failed => self.toast(format!("Download failed: {name}")),
            _ => self.toast(format!("Cancelled {name}")),
        }
        self.save_downloads();
        // Pages kept only for their downloads can close now.
        let busy: HashSet<u64> = self.download_ids.values().map(|&(_, page)| page).collect();
        let before = self.parked.len();
        self.parked.retain(|p| busy.contains(&p.key()));
        if self.parked.len() < before {
            log::info!("closed {} page(s) kept for downloads", before - self.parked.len());
        }
    }

    /// Closes a page, unless it has a download running: then it is kept,
    /// unseen, until that ends (see `parked`).
    fn release(&mut self, page: Page) {
        let key = page.key();
        if self.download_ids.values().any(|&(_, p)| p == key) {
            log::info!("page {key} keeps running for its download");
            self.parked.push(page);
        }
    }

    /// At quit: running downloads can't go on without us, so they are
    /// cancelled (Chromium removes the partial files) and listed as stopped.
    fn stop_downloads(&mut self) {
        use scene::download::State;
        if self.download_ids.is_empty() {
            return;
        }
        for (id, (key, _)) in std::mem::take(&mut self.download_ids) {
            web::download::cancel(id);
            if let Some(d) = self.downloads.get(key) {
                let (r, t) = (d.received, d.total);
                self.downloads.update(key, r, t, State::Stopped);
            }
        }
        self.save_downloads();
    }

    fn save_downloads(&mut self) {
        if let Err(e) = storage::downloads::save_downloads(&self.data_dir, &self.downloads) {
            log::warn!("downloads: saving: {e}");
        }
    }

    /// Enter on a download: opens a finished file in its app; one that
    /// didn't finish is downloaded again (by the current page, any live one,
    /// or else a new card).
    fn open_download(&mut self, key: u64) {
        use scene::download::State;
        let Some(d) = self.downloads.get(key).cloned() else { return };
        match d.state {
            State::Running => self.toast("Still downloading · Shift+Del cancels"),
            State::Done if std::fs::symlink_metadata(&d.path).is_err() => {
                self.toast(format!("{} was deleted or moved", d.name()));
            }
            State::Done => {
                log::info!("downloads: opening {}", d.path);
                open_externally(&d.path);
            }
            State::Cancelled | State::Failed | State::Stopped => {
                let page = self.focused_page().or_else(|| self.cards.values().map(|c| &c.page).next());
                match page {
                    Some(p) => {
                        log::info!("downloads: again {}", d.url);
                        p.download(&d.url);
                    }
                    // A card for it: it closes itself once the file starts (blank page).
                    None => {
                        log::info!("downloads: again in a new card {}", d.url);
                        self.new_card_at_center(&d.url);
                    }
                }
            }
        }
    }

    /// Ctrl+Enter on a download: opens the folder it is in.
    fn show_download_folder(&mut self, key: u64) {
        let Some(d) = self.downloads.get(key) else { return };
        let dir = std::path::Path::new(&d.path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        log::info!("downloads: showing {dir}");
        open_externally(&dir);
    }

    /// Shift+Delete in the downloads view: cancels a running download, or
    /// takes a finished one off the list (the file stays).
    fn drop_selected_download(&mut self) {
        let Some(ob) = self.omnibox.as_ref() else { return };
        let Some(palette::Item::Download(key)) = self.list_rows(ob).into_iter().nth(ob.sel).map(|r| r.item) else { return };
        let running = self.download_ids.iter().find(|(_, (k, _))| *k == key).map(|(&id, _)| id);
        match running {
            Some(id) => {
                log::info!("downloads: cancelling {id}");
                web::download::cancel(id);
            }
            None => {
                if let Some(d) = self.downloads.remove(key) {
                    log::info!("downloads: removed {} from the list", d.path);
                }
                self.save_downloads();
                let len = self.omnibox.as_ref().map_or(0, |ob| self.list_rows(ob).len());
                if let Some(ob) = self.omnibox.as_mut() {
                    ob.sel = ob.sel.min(len.saturating_sub(1));
                }
            }
        }
    }

    /// Shift+Delete in the palette or the history view: forgets the
    /// highlighted row's page.
    fn forget_selected(&mut self) {
        let Some(ob) = self.omnibox.as_ref() else { return };
        let Some(url) = self.list_rows(ob).into_iter().nth(ob.sel).and_then(|r| r.history) else { return };
        let Some(index) = self.page_index.as_mut() else { return };
        match index.forget(&url) {
            Ok(true) => {
                log::info!("history: forgot {url}");
                self.preview = None;
                // The row going away says it; a toast would wait under the list.
                self.visits_cache.replace(None);
            }
            Ok(false) => {}
            Err(e) => log::warn!("history: forgetting {url}: {e}"),
        }
        // Keep the highlight on the row that took its place.
        let (vp, ui) = (self.viewport(), self.ui());
        let len = self.omnibox.as_ref().map_or(0, |ob| self.list_rows(ob).len());
        if let Some(ob) = self.omnibox.as_mut() {
            ob.sel = ob.sel.min(len.saturating_sub(1));
            ob.scroll = scene::palette::keep_visible(ob.sel, ob.scroll, palette::Panel::new(vp, ui, len).shown, len);
        }
    }

    /// *Clear history*: asks first, as it can't be undone.
    fn ask_clear_history(&mut self) {
        let n = match &self.page_index {
            None => return self.toast("History is off"),
            Some(index) => index.len().unwrap_or(0),
        };
        if n == 0 {
            return self.toast("History is empty");
        }
        let pages = if n == 1 { "1 page".to_string() } else { format!("{n} pages") };
        self.omnibox = None;
        self.confirm = Some(Confirm::ClearHistory);
        self.toast(format!("Forget all {pages} in history? Enter clears · Esc keeps them"));
    }

    fn clear_history(&mut self) {
        self.confirm = None;
        let Some(index) = self.page_index.as_mut() else { return };
        // Pages already asked for their text would be kept again.
        self.text_due.clear();
        match index.clear() {
            Ok(n) => {
                log::info!("history: cleared {n} pages");
                self.preview = None;
                self.visits_cache.replace(None);
                self.toast("History cleared");
            }
            Err(e) => {
                log::warn!("history: clearing: {e}");
                self.toast("Couldn't clear history");
            }
        }
    }

    /// The toast when a question was answered no.
    fn kept(&self, confirm: Confirm) -> String {
        match confirm {
            Confirm::DeleteSpace => format!("Kept {}", self.spaces.active_entry().name),
            Confirm::ClearHistory => "Kept history".to_string(),
        }
    }

    fn run_command(&mut self, c: Command) {
        match c {
            Command::NewCard => self.omnibox = Some(Omnibox::new("", Target::NewCard)),
            Command::NewNote => {
                self.new_note_at_center();
            }
            Command::NewSticky => {
                self.new_sticky_at_center();
            }
            Command::Tidy => self.tidy_board(),
            Command::Fit => self.fit_all(),
            Command::Fold => self.toggle_fold(),
            Command::Outline => self.outline_in_view(),
            Command::NameTrail => self.ask_trail_name(),
            Command::ColourTrail => self.cycle_trail_colour(),
            Command::Sidebar => self.toggle_sidebar(),
            Command::Read => self.toggle_read(),
            Command::LinkHints => self.show_link_hints(),
            Command::Arrange => self.start_arrange(),
            Command::SaveMarkdown => {
                let note = self.focused.filter(|&id| self.scene.node(id).is_some_and(Node::is_note));
                self.save_markdown(note);
            }
            Command::Undo => self.undo(false),
            Command::Redo => self.undo(true),
            Command::Reopen => self.reopen_closed(),
            Command::CloseCard => {
                if !self.selection.is_empty() {
                    let ids = self.selection.ids();
                    self.close_cards(&ids);
                } else if let Some(id) = self.focused {
                    self.close_card(id);
                } else {
                    self.toast("No card to close");
                }
            }
            Command::Reload => match self.focused_page().filter(|p| p.kind() == web::PageKind::Web) {
                Some(p) => p.reload(),
                None => self.toast("No page to reload"),
            },
            Command::FindInPage => self.open_find(),
            Command::MarkCard => self.toggle_mark(),
            Command::FillView => self.fill_current(),
            Command::PageMenu => {
                let ev = KeyIn::from_name("menu", true, false).expect("menu key");
                self.page_menu_by_key(&ev);
            }
            Command::ZoomIn => self.zoom_page(1),
            Command::ZoomOut => self.zoom_page(-1),
            Command::ZoomReset => self.zoom_page(0),
            Command::SelectAll => self.selection.select_all(&self.scene),
            Command::NewSpace => self.new_space(false),
            Command::NewPrivateSpace => self.new_space(true),
            Command::PrivateSpace => self.toggle_private(),
            Command::NextSpace | Command::PreviousSpace => {
                let step = if c == Command::NextSpace { 1 } else { -1 };
                self.switch_space(self.spaces.neighbour(self.spaces.active, step));
            }
            Command::RenameSpace => {
                let name = self.spaces.active_entry().name.clone();
                self.omnibox = Some(Omnibox::new(&name, Target::RenameSpace));
            }
            Command::NextTint => self.cycle_tint(),
            Command::DeleteSpace => self.ask_delete_space(),
            Command::History => self.open_history(),
            Command::ClearHistory => self.ask_clear_history(),
            Command::Settings => self.open_settings(),
            Command::Downloads => self.open_downloads(),
            Command::Shortcuts => {
                self.hints = true;
                log::info!("hints shown");
            }
            Command::Welcome => self.show_guide(),
            Command::ToggleBlocking => self.toggle_blocking(self.current()),
            Command::Quit => {
                log::info!("quit (palette)");
                self.quit = true;
            }
        }
    }

    /// A press on the sidebar: a header collapses or expands its trail, a card
    /// row goes to that card. Either gives the sidebar the keys.
    fn sidebar_click(&mut self, p: Vec2, button: MouseButton) {
        use scene::sidebar::{Group, Row};
        if button != MouseButton::Left {
            return;
        }
        let rows = self.sidebar.rows(&self.scene);
        let panel = self.sidebar_panel();
        if self.typing() {
            self.leave_page();
        }
        self.sidebar_keys = true;
        let Some(i) = panel.row_at(p, self.sidebar.scroll, rows.len()) else { return };
        self.sidebar.sel = Some(rows[i].key());
        self.sidebar_at = Some(i);
        match rows[i] {
            // A trail's header toggles on release, unless it was dragged.
            Row::Header { group: Group::Trail(head), .. } => {
                self.sidebar_press = Some(SidebarPress { head, at: p, dragging: false, to: None });
            }
            Row::Header { group, .. } => self.sidebar.toggle(group),
            Row::Card { id, .. } => self.sidebar_go(id),
        }
    }

    /// The pointer moved with a trail's header pressed: past a few px it is
    /// a drag, and the row under the pointer says where the trail would go.
    fn sidebar_press_moved(&mut self, p: Vec2) {
        let Some(mut press) = self.sidebar_press else { return };
        if !press.dragging && (p.y - press.at.y).abs() < 5.0 * self.ui() {
            return;
        }
        press.dragging = true;
        let rows = self.sidebar.rows(&self.scene);
        let panel = self.sidebar_panel();
        press.to = panel
            .row_at(p, self.sidebar.scroll, rows.len())
            .and_then(|i| scene::sidebar::drop_to(&self.scene, press.head, rows[i].group()));
        self.sidebar_press = Some(press);
    }

    /// Release after a press on a trail's header: a click toggles the group,
    /// a drag moves the trail where it was let go.
    fn sidebar_release(&mut self) {
        let Some(press) = self.sidebar_press.take() else { return };
        if !press.dragging {
            self.sidebar.toggle(scene::sidebar::Group::Trail(press.head));
        } else if let Some(to) = press.to {
            self.move_trail(press.head, to);
        }
    }

    /// While a trail's header is dragged: the y of the line where it would
    /// go (above the trail it would take the place of, moving up; below it,
    /// moving down), if that row is shown.
    fn sidebar_drop_line(&self, rows: &[scene::sidebar::Row], panel: &Panel) -> Option<f32> {
        use scene::sidebar::{self, Group};
        let press = self.sidebar_press.filter(|p| p.dragging)?;
        let to = press.to?;
        let (from, _) = sidebar::position(&self.scene, press.head)?;
        let target = sidebar::groups(&self.scene)
            .into_iter()
            .filter_map(|(g, _)| match g {
                Group::Trail(h) if sidebar::drop_to(&self.scene, press.head, g) == Some(to) => Some(h),
                _ => None,
            })
            .next()?;
        let of_target = |r: &&scene::sidebar::Row| sidebar::trail_head(r) == Some(target);
        if to == from {
            return None;
        }
        let (index, below) = if to < from {
            (rows.iter().position(|r| of_target(&r))?, false)
        } else {
            (rows.iter().rposition(|r| of_target(&r))?, true)
        };
        let rect = panel.row_rect(index, self.sidebar.scroll)?;
        Some(if below { rect.bottom() } else { rect.y })
    }

    // ---------------------------------------------------------------- arrange

    /// M: move and resize the current card (else the one in the middle of the
    /// view) with h/j/k/l until Enter, M or Esc. A selected card brings the
    /// rest of the selection along when it moves.
    fn start_arrange(&mut self) {
        if self.read.is_some() {
            return;
        }
        let Some(id) = self.current() else {
            self.toast("No card here to move");
            return;
        };
        if !self.selection.contains(id) {
            self.selection.clear();
        }
        self.focus(Some(id));
        let group = selection::drag_group(&self.scene, &self.selection, id);
        self.arrange = Arrange::new(&self.scene, id, group);
        self.ensure_visible(id);
        log::info!("arranging card {}", id.0);
    }

    /// Ends arranging: `keep` records the moves as one undo step, else every
    /// card goes back where it was.
    fn finish_arrange(&mut self, keep: bool) {
        let Some(a) = self.arrange.take() else { return };
        if keep {
            self.history.moved_many(&self.scene, &a.start);
        } else {
            a.cancel(&mut self.scene);
        }
        self.dirty = true;
        log::info!("arranged card {}: {}", a.id.0, if keep { "kept" } else { "cancelled" });
    }

    /// A key while arranging: h/j/k/l or arrows move, with Shift resize, with
    /// Ctrl take a fine step; Enter, Space or M keep, Esc cancels. Returns
    /// false for any other key, which keeps the arrangement and goes on as usual.
    fn arrange_key(&mut self, code: Option<KeyCode>) -> bool {
        let Some(a) = self.arrange.as_ref() else { return false };
        if a.is_stale(&self.scene) {
            self.arrange = None;
            return false;
        }
        let dir = match code {
            Some(KeyCode::KeyH | KeyCode::ArrowLeft) => Dir::Left,
            Some(KeyCode::KeyJ | KeyCode::ArrowDown) => Dir::Down,
            Some(KeyCode::KeyK | KeyCode::ArrowUp) => Dir::Up,
            Some(KeyCode::KeyL | KeyCode::ArrowRight) => Dir::Right,
            Some(KeyCode::Escape) => {
                self.finish_arrange(false);
                return true;
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space | KeyCode::KeyM) => {
                self.finish_arrange(true);
                return true;
            }
            _ => {
                self.finish_arrange(true);
                return false;
            }
        };
        let px = if self.mods.control_key() { ARRANGE_FINE } else { ARRANGE_STEP };
        let step = px / self.camera.zoom.max(1e-3);
        if self.mods.shift_key() {
            a.resize(&mut self.scene, dir, step, MIN_CARD);
        } else {
            a.nudge(&mut self.scene, dir, step);
        }
        let id = a.id;
        self.dirty = true;
        self.ensure_visible(id);
        true
    }

    // ---------------------------------------------------------------- link hints

    /// f: label the links on the current card's page (else the one in the
    /// middle of the view), which becomes current. The labels show once the
    /// page has answered (`got_links`).
    fn show_link_hints(&mut self) {
        let Some(id) = self.current() else {
            self.toast("No page here to follow links on");
            return;
        };
        self.selection.clear();
        self.focus(Some(id)); // wakes it if asleep
        self.ensure_visible(id);
        let Some(card) = self.cards.get(&id) else {
            self.toast("No page here to follow links on");
            return;
        };
        card.page.request_links();
        self.link_hints = Some(LinkHints { id, links: Vec::new(), hints: None });
        log::info!("link hints asked of card {}", id.0);
    }

    /// The page of card `id` answered with its links (`width`: its layout
    /// width in CSS px). Ignored unless hints were asked of that card.
    fn got_links(&mut self, id: NodeId, width: f32, links: Vec<web::links::Link>) {
        // The page may be zoomed: its CSS px are not the view's.
        let Some((_, css)) = self.page_view(id) else { return };
        let Some(lh) = self.link_hints.as_mut().filter(|h| h.id == id && h.hints.is_none()) else {
            log::debug!("links from card {} not asked for", id.0);
            return;
        };
        let k = css.x / width.max(1.0);
        lh.links = links
            .into_iter()
            .map(|l| (Rect::new(l.rect[0] * k, l.rect[1] * k, l.rect[2] * k, l.rect[3] * k), l.action))
            .collect();
        if lh.links.is_empty() {
            self.link_hints = None;
            let loading = self.cards.get(&id).is_some_and(|c| c.loading);
            log::info!("link hints on card {}: no links in view", id.0);
            self.toast(if loading { "Nothing to click yet · the page is still loading" } else { "Nothing to click in view" });
            return;
        }
        let h = hint::Hints::new(lh.links.len());
        log::info!(
            "link hints on card {}: {} links, {}",
            id.0,
            lh.links.len(),
            h.labels
                .iter()
                .zip(&lh.links)
                .take(8)
                .map(|(l, (_, a))| match a {
                    web::links::Action::Open(u) => format!("{l}={u}"),
                    web::links::Action::Click => format!("{l}=click"),
                    web::links::Action::Type => format!("{l}=type"),
                })
                .collect::<Vec<_>>()
                .join(" ")
        );
        lh.hints = Some(h);
    }

    /// A key while link hints are up. `chord`: Ctrl or Alt is held.
    fn link_hint_key(&mut self, ev: &KeyIn, chord: bool) {
        let Some(lh) = self.link_hints.as_mut() else { return };
        let letter = ev.text.as_deref().and_then(|t| {
            let mut cs = t.chars();
            cs.next().filter(|c| cs.next().is_none() && c.is_alphabetic())
        });
        match (ev.named, letter, lh.hints.as_mut()) {
            (Some(NamedKey::Backspace), _, Some(h)) if !chord => {
                h.backspace();
            }
            (_, Some(c), Some(h)) if !chord => match h.type_char(c) {
                hint::Typed::Follow(i) => {
                    let (id, (rect, action)) = (lh.id, lh.links[i].clone());
                    self.link_hints = None;
                    match action {
                        web::links::Action::Open(url) => self.follow_link(id, &url),
                        web::links::Action::Click => self.click_target(id, rect, false),
                        web::links::Action::Type => self.click_target(id, rect, true),
                    }
                }
                hint::Typed::Narrowed => {}
                hint::Typed::NoMatch => log::debug!("no link hint starts with {c:?}"),
            },
            // Still waiting: letters wait too, anything else gives up.
            (_, Some(_), None) if !chord => {}
            _ => {
                self.link_hints = None;
                log::info!("link hints cancelled");
            }
        }
    }

    /// A hint on a button, field or other control: click its middle, as the
    /// mouse would. A field (`typing`) gets the keys, so you can type in it at once.
    fn click_target(&mut self, id: NodeId, rect: Rect, typing: bool) {
        if typing {
            self.focus(Some(id));
            self.keys_in_page = true;
        }
        let Some(card) = self.cards.get(&id) else { return };
        if typing {
            card.page.set_focus(true);
        }
        let c = rect.center();
        let (x, y) = (c.x.round() as i32, c.y.round() as i32);
        card.page.mouse_move(x, y, 0, false);
        card.page.mouse_button(x, y, 0, WebButton::Left, false, 1);
        card.page.mouse_button(x, y, 0, WebButton::Left, true, 1);
        log::info!("clicked a hint on card {} at {x},{y}{}", id.0, if typing { ", keys to the page" } else { "" });
    }

    /// v: add the current card to the selection, or take it out. Going to
    /// another card keeps the selection, so several can be picked by keyboard.
    fn toggle_mark(&mut self) {
        let Some(id) = self.current() else { return };
        self.focus(Some(id));
        self.ensure_visible(id);
        self.selection.toggle(id);
        let n = self.selection.ids().len();
        log::info!("selection: {n} card(s)");
        self.toast(match n {
            0 => "Selection cleared".to_string(),
            1 => "1 card selected".to_string(),
            n => format!("{n} cards selected"),
        });
    }

    /// z: the current card fills the view, as a double-click on its title bar does.
    fn fill_current(&mut self) {
        if let Some(id) = self.current() {
            self.focus(Some(id));
            self.fill_view(id);
        }
    }

    /// d / u: scroll the current card's page half its height, as the wheel
    /// would over its middle, without giving it the keys.
    fn scroll_current(&mut self, down: bool) {
        let Some(id) = self.current() else { return };
        self.focus(Some(id));
        self.ensure_visible(id);
        let Some((_, css)) = self.page_view(id) else { return };
        let Some(card) = self.cards.get(&id) else { return };
        let half = (css.y / 2.0).round() as i32;
        let (x, y) = ((css.x / 2.0) as i32, (css.y / 2.0) as i32);
        card.page.mouse_wheel(x, y, 0, 0, if down { -half } else { half }, false);
        self.link_hints = None; // scrolling moves the targets
    }

    /// Menu key or Shift+F10: the page's own menu, as a right-click gives, at
    /// the page's selection or focused field. Chromium works out where and
    /// what (link, selection, field) and asks for our menu as usual.
    fn page_menu_by_key(&mut self, ev: &KeyIn) {
        let Some(id) = self.current() else {
            self.toast("No page here for a menu");
            return;
        };
        self.focus(Some(id));
        self.ensure_visible(id);
        let Some(card) = self.cards.get(&id) else { return };
        card.page.send_key(&ev.to_web(self.mods));
        self.menu_by_key = Some(Instant::now());
        log::info!("page menu asked of card {} by key", id.0);
    }

    /// Open `url` beside card `from`, as a click on the link would, and make
    /// the new card current, so the next f follows on from it.
    fn follow_link(&mut self, from: NodeId, url: &str) {
        if self.scene.node(from).is_none() {
            return;
        }
        log::info!("followed a link hint on card {} to {url}", from.0);
        if let Some(id) = self.open_card(Rect::default(), url, Some(from)) {
            self.focus(Some(id));
            self.ensure_visible(id);
        }
    }

    /// The labels of the link hints, over their links on screen.
    fn link_hint_layer(&mut self) -> Option<Layer> {
        let lh = self.link_hints.as_ref()?;
        let h = lh.hints.as_ref()?;
        let (view, _) = self.page_view(lh.id)?;
        let th = &self.theme;
        let ui = self.ui();
        let scale = self.scale();
        let size = 11.5 * ui;
        let (pad, bh) = (size * 0.4, size * 1.45);
        let typed_n = h.typed.chars().count();
        let (bg, fg, done) = (rgb(th.accent(), 1.0), rgb(th.accent_foreground(), 1.0), rgb(th.accent_foreground(), 0.45));
        let border = rgb(th.background(), 0.9);
        let radius = th.small_radius(scale);
        let mut items = Vec::new();
        for (i, (rect, _)) in lh.links.iter().enumerate() {
            if !h.shown(i) {
                continue;
            }
            let Some(at) = self.page_rect_to_screen(lh.id, *rect) else { continue };
            let label = h.labels[i].to_uppercase();
            let (head, tail): (String, String) = (label.chars().take(typed_n).collect(), label.chars().skip(typed_n).collect());
            items.push((at, head, tail));
        }
        let mut l = Layer::default();
        let r = self.renderer.as_mut()?;
        for (at, head, tail) in items {
            let head_w = r.measure(&head, size, true);
            let w = head_w + r.measure(&tail, size, true) + pad * 2.0;
            // At the link's start, kept inside the page.
            let x = at.x.max(view.x).min(view.right() - w).max(view.x);
            let y = (at.y + (at.h - bh) / 2.0).max(view.y).min(view.bottom() - bh).max(view.y);
            let b = Rect::new(x, y, w, bh);
            l.quads.push(Quad { shape: Shape::Rect { rect: b.inflate(scale.max(1.0)), radius }, color: border, texture: None });
            l.quads.push(Quad { shape: Shape::Rect { rect: b, radius }, color: bg, texture: None });
            let ty = y + (bh - size * 1.3) / 2.0;
            if !head.is_empty() {
                l.texts.push(Text { text: head, pos: [x + pad, ty], size, color: done, clip: view, bold: true });
            }
            l.texts.push(Text { text: tail, pos: [x + pad + head_w, ty], size, color: fg, clip: view, bold: true });
        }
        Some(l)
    }

    // ---------------------------------------------------------------- camera

    fn fit_all(&mut self) {
        self.leave_read();
        if let Some(b) = self.scene.bounds() {
            self.camera_goal = Some(Camera::fit(b, self.viewport(), self.scale(), 48.0 * self.scale()));
        }
    }

    /// T: lay every trail out left to right with nothing overlapping
    /// (`scene::tidy`), as one undo step. The card in the middle of the view
    /// keeps its place on screen, so the board doesn't jump away.
    fn tidy_board(&mut self) {
        let anchor = read::pick(&self.scene, self.focused, self.camera.center)
            .and_then(|id| self.scene.node(id).map(|n| (id, n.rect)));
        let mut before = Vec::new();
        for (id, rect) in tidy::tidy(&self.scene) {
            if let Some(n) = self.scene.node_mut(id)
                && n.rect != rect
            {
                before.push((id, std::mem::replace(&mut n.rect, rect)));
            }
        }
        log::info!("tidy: moved {} card(s)", before.len());
        if before.is_empty() {
            self.toast("Already tidy");
            return;
        }
        self.history.moved_many(&self.scene, &before);
        self.dirty = true;
        if let Some((id, old)) = anchor
            && let Some(new) = self.scene.node(id).map(|n| n.rect)
        {
            let mut goal = self.camera_goal.unwrap_or(self.camera);
            goal.center = goal.center + Vec2::new(new.x - old.x, new.y - old.y);
            self.camera_goal = Some(goal);
        }
        self.toast(format!("Tidied {} card{}", before.len(), if before.len() == 1 { "" } else { "s" }));
    }

    /// S: fold the trail of the card in the middle of the view into a stack
    /// with that card in front, or unfold it if it is a stack. One undo step.
    fn toggle_fold(&mut self) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        if self.scene.stack_of(id).is_some() {
            return self.unfold_stack(id);
        } else if let Some(f) = self.scene.fold(id) {
            log::info!("fold: {:?} in front of {}", f.members.iter().map(|m| m.0).collect::<Vec<_>>(), id.0);
            self.history.folded(&f);
            self.after_fold();
            self.toast(format!("Folded {} pages · S unfolds, [ ] peek", f.members.len()));
        } else {
            self.toast("No trail to fold");
            return;
        }
        self.dirty = true;
    }

    fn unfold_stack(&mut self, id: NodeId) {
        if let Some(stack) = self.scene.unfold(id) {
            log::info!("unfold: {} card(s) around {}", stack.members.len(), id.0);
            self.history.unfolded(&stack);
            self.toast(format!("Unfolded {} pages", stack.members.len()));
            self.dirty = true;
        }
    }

    /// O: the outline of the stack in the middle of the view.
    fn outline_in_view(&mut self) {
        match read::pick(&self.scene, self.focused, self.camera.center) {
            Some(id) if self.scene.stack_of(id).is_some() => self.open_outline(id),
            _ => self.toast("No stack here · S folds a trail"),
        }
    }

    /// [ / ]: show the previous / next card of the stack in the middle of the view.
    fn peek_stack(&mut self, step: isize) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        let Some(front) = self.scene.peek(id, step) else { return };
        let stack = self.scene.stack_of(front).expect("peeked a stack");
        log::info!("peek: card {} ({} of {})", front.0, stack.front_index() + 1, stack.members.len());
        self.after_fold();
        self.dirty = true;
    }

    /// Alt+PgDn / Alt+PgUp: focus the next / previous card along the trail of
    /// the focused card (or the one in the middle of the view), showing it at
    /// its stack's front if it is folded away, and bring it on screen (in Read
    /// Mode, read it). Stops at the ends of the trail.
    fn step_trail(&mut self, step: isize) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        match self.scene.step_trail(id, step) {
            TrailStep::Moved { to, index, len } => {
                log::info!("trail: card {} ({} of {})", to.0, index + 1, len);
                self.selection.clear();
                self.after_fold();
                self.focus(Some(to));
                self.ensure_visible(to);
                let title = self.scene.node(to).map(|n| n.title.clone()).filter(|t| !t.is_empty());
                let title = title.unwrap_or_else(|| self.scene.node(to).map(|n| n.url.clone()).unwrap_or_default());
                let step = format!("{}/{} · {}", index + 1, len, menu::cut(&title, menu::OUTLINE_CHARS));
                self.toast(match self.scene.trail_mark(to).name {
                    Some(name) => format!("{name} · {step}"),
                    None => step,
                });
                self.dirty = true;
            }
            TrailStep::End { first, .. } => {
                // Focus the card we stepped from, so the next step starts there.
                self.focus(Some(id));
                self.toast(if first { "Start of the trail" } else { "End of the trail" });
            }
            TrailStep::Lone => self.toast("Not on a trail"),
        }
    }

    /// R: the address bar, as a name box for the trail in the middle of the view.
    fn ask_trail_name(&mut self) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        let name = self.scene.trail_mark(id).name.unwrap_or_default();
        self.omnibox = Some(Omnibox::new(&name, Target::NameTrail(id)));
    }

    fn name_trail(&mut self, id: NodeId, name: &str) {
        let old = self.scene.name_trail(id, name);
        if old.is_empty() {
            return;
        }
        self.history.marked(old);
        self.dirty = true;
        let mark = self.scene.trail_mark(id);
        log::info!("trail of card {} named {:?}", id.0, mark.name);
        self.toast(match mark.name {
            Some(n) => format!("Trail named {n}"),
            None => "Trail name cleared".to_string(),
        });
    }

    /// C: the trail in the middle of the view takes its next colour.
    /// C: the next colour of the trail of the current card (or the one in the
    /// middle of the view), or of that card itself if it is a sticky note.
    fn cycle_trail_colour(&mut self) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        if self.scene.node(id).is_some_and(Node::is_sticky) {
            self.cycle_sticky_colour(id);
            return;
        }
        let old = self.scene.cycle_trail_colour(id);
        self.history.marked(old);
        self.dirty = true;
        let colour = self.scene.trail_mark(id).colour;
        log::info!("trail of card {} colour {:?}", id.0, colour);
        self.toast(match colour {
            Some(c) => format!("Trail colour: {c}"),
            None => "Trail colour: accent".to_string(),
        });
    }

    /// Cards folded away lose focus and selection; their pages sleep in `sync_liveness`.
    fn after_fold(&mut self) {
        if self.focused.is_some_and(|f| self.scene.is_hidden(f)) {
            self.focus(None);
        }
        let hidden: Vec<NodeId> = self.selection.ids().into_iter().filter(|id| self.scene.is_hidden(*id)).collect();
        for id in hidden {
            self.selection.remove(id);
        }
    }

    fn fill_view(&mut self, id: NodeId) {
        if let Some(n) = self.scene.node(id) {
            let mut goal = Camera::fit(n.frame(), self.viewport(), self.scale(), 8.0 * self.scale());
            goal.zoom = goal.zoom.min(2.0);
            self.camera_goal = Some(goal);
        }
    }

    /// Read card `id`: it fills the window and gets focus. Keys stay with the
    /// board (Tab returns) until the page is clicked; other keys reach the page.
    fn read_card(&mut self, id: NodeId) {
        if self.scene.node(id).is_none() {
            return;
        }
        if self.read != Some(id) {
            log::info!("read mode: card {}", id.0);
        }
        self.read = Some(id);
        self.selection.clear();
        self.menu = None;
        self.camera_goal = None;
        self.drag = Drag::None;
        self.focus(Some(id));
        self.keys_in_page = false;
    }

    /// Tab: read the focused card (or the one in the middle of the view), or go back to the board.
    fn toggle_read(&mut self) {
        self.link_hints = None;
        if self.read.is_some() {
            self.leave_read();
        } else if let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) {
            self.read_card(id);
        }
    }

    /// Back to the board as it was, with the card that was read focused and on screen.
    fn leave_read(&mut self) {
        let Some(id) = self.read.take() else { return };
        log::info!("read mode: back to the board");
        self.menu = None;
        self.keys_in_page = false;
        self.camera_moved(); // pages go back to the board's raster scale once settled
        self.ensure_visible(id);
    }

    /// Glide just enough to bring card `id` on screen (fitting it with the focused card if possible).
    /// In Read Mode, bringing a card on screen means reading it.
    fn ensure_visible(&mut self, id: NodeId) {
        if self.read.is_some() {
            self.read_card(id);
            return;
        }
        let Some(frame) = self.scene.node(id).map(|n| n.frame()) else { return };
        let vp = self.viewport();
        // The open sidebar covers the left of the window: fit the card in the rest.
        let left = if self.sidebar_shown() { self.sidebar_panel().rect.right() } else { 0.0 };
        let on_screen = self.to_screen_rect(frame);
        if on_screen.x >= left && on_screen.y >= 0.0 && on_screen.right() <= vp.x && on_screen.bottom() <= vp.y {
            return;
        }
        let mut goal = self.camera;
        let scale = self.scale();
        let need = Camera::fit(frame, Vec2::new(vp.x - left, vp.y), scale, 32.0 * scale);
        if need.zoom < goal.zoom {
            goal.zoom = need.zoom;
        }
        goal.center = frame.center();
        goal.center.x -= left / 2.0 / goal.pixels_per_unit(scale);
        self.camera_goal = Some(goal);
    }

    fn step_camera(&mut self, dt: f32) {
        let Some(goal) = self.camera_goal else { return };
        let t = 1.0 - (-dt * 14.0).exp();
        let c = &mut self.camera;
        c.center = c.center + (goal.center - c.center) * t;
        // Interpolate zoom in log space so it feels even.
        c.zoom = (c.zoom.ln() + (goal.zoom.ln() - c.zoom.ln()) * t).exp();
        let close = (c.center.x - goal.center.x).abs() < 0.5
            && (c.center.y - goal.center.y).abs() < 0.5
            && (c.zoom - goal.zoom).abs() < 0.001;
        if close {
            *c = goal;
            self.camera_goal = None;
        }
        self.camera_moved();
    }

    /// Re-rasterize pages at a resolution matching the settled zoom, and hide
    /// pages that are off screen so they stop painting.
    fn sync_pages(&mut self) {
        // A read page is shown 1:1, so it rasterizes as if the zoom were 1.
        let reading = self.read;
        let settled =
            reading.is_some() || (self.camera_changed.elapsed() > Duration::from_millis(250) && self.camera_goal.is_none());
        self.sync_liveness(settled && !matches!(self.drag, Drag::Pan { .. }));
        if settled {
            let zoom = if reading.is_some() { 1.0 } else { self.camera.zoom };
            let want = ((zoom * self.scale()) * 4.0).round().clamp(2.0, 8.0) / 4.0;
            if (want - self.page_scale).abs() > f32::EPSILON {
                self.page_scale = want;
            }
        }
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let page_scale = self.page_scale;
        let mut updates = Vec::new();
        let read_css = self.read_layout().css;
        for n in self.scene.nodes() {
            let (on_screen, size) = match reading {
                Some(r) if r == n.id => (true, read_css),
                Some(_) => (false, (n.rect.w.round() as i32, n.rect.h.round() as i32)),
                None => (self.to_screen_rect(n.frame()).intersects(&screen), (n.rect.w.round() as i32, n.rect.h.round() as i32)),
            };
            updates.push((n.id, on_screen, size));
        }
        for (id, visible, size) in updates {
            if let Some(card) = self.cards.get_mut(&id) {
                if card.hidden == visible {
                    card.hidden = !visible;
                    card.page.set_hidden(!visible);
                }
                card.page.resize(size, page_scale);
            }
        }
    }

    /// Keep at most `max_live` live browsers. Sleeping cards on screen wake only
    /// when `allow_wake` (the camera has settled), so panning doesn't start pages.
    fn sync_liveness(&mut self, allow_wake: bool) {
        // Cards folded away in a stack never keep a live browser.
        let folded: Vec<NodeId> = self.cards.keys().copied().filter(|id| self.scene.is_hidden(*id)).collect();
        for id in folded {
            log::info!("sleeping card {} (folded)", id.0);
            self.sleep_page(id);
        }
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let candidates: Vec<liveness::Candidate> = self
            .scene
            .nodes()
            .map(|n| liveness::Candidate {
                id: n.id,
                live: self.cards.contains_key(&n.id),
                visible: match self.read {
                    Some(r) => r == n.id,
                    None => self.to_screen_rect(n.frame()).intersects(&screen),
                },
                focused: self.focused == Some(n.id),
                last_used: self.used.get(&n.id).copied().unwrap_or(0),
            })
            .collect();
        let plan = liveness::plan(&candidates, self.max_live, allow_wake);
        for id in plan.sleep {
            self.sleep_page(id);
        }
        for id in plan.wake {
            log::info!("waking card {} (on screen)", id.0);
            let used = self.used.get(&id).copied();
            self.create_page(id);
            // Coming into view isn't use; keep its old recency.
            match used {
                Some(u) => self.used.insert(id, u),
                None => self.used.remove(&id),
            };
        }
    }

    // ---------------------------------------------------------------- events from pages

    fn handle_web_events(&mut self) {
        for ev in web::drain_events() {
            match ev {
                WebEvent::Relaunch { args, cwd } => self.relaunched(&args, &cwd),
                WebEvent::Fetched { tag, ok, body } => {
                    if !self.blocking.fetched(&tag, ok, &body) {
                        log::warn!("fetch {tag}: nobody asked");
                    }
                }
                WebEvent::DownloadAsked { page, id, url, suggested, blank } => {
                    self.download_asked(page, id, &url, &suggested, blank)
                }
                WebEvent::DownloadUpdated { id, received, total, state } => {
                    self.download_updated(id, received, total, state)
                }
                WebEvent::OpenBeside { source, url } => {
                    let Some(src) = self.node_of(source) else { continue };
                    if self.scene.node(src).is_some()
                        && let Some(id) = self.open_card(Rect::default(), &url, Some(src)) {
                            self.ensure_visible(id);
                        }
                }
                WebEvent::TitleChanged { page, title } => {
                    let Some(id) = self.node_of(page) else { continue };
                    if let Some(n) = self.scene.node_mut(id) {
                        n.title = title;
                        self.dirty = true;
                    }
                    // A page that changes its title without loading (most
                    // single-page apps on navigation) has new text to keep.
                    if self.cards.get(&id).is_some_and(|c| !c.loading) {
                        self.text_due.insert(id, Instant::now() + TEXT_SETTLE);
                    }
                }
                WebEvent::UrlChanged { page, url } => {
                    if self.link_hints.as_ref().is_some_and(|h| Some(h.id) == self.node_of(page)) {
                        self.link_hints = None; // its links went with the old page
                    }
                    if let Some(n) = self.node_of(page).and_then(|id| self.scene.node_mut(id)) {
                        n.url = url;
                        self.dirty = true;
                    }
                }
                WebEvent::CursorChanged { page, cursor } => {
                    let Some(id) = self.node_of(page) else { continue };
                    if let Some(c) = self.cards.get_mut(&id) {
                        c.cursor = cursor;
                    }
                    let over_page = match self.drag {
                        Drag::Page { id: d, .. } => d == id,
                        Drag::None => self.hit(self.cursor) == Hit::Content(id),
                        _ => false,
                    };
                    if over_page {
                        log::debug!("page {page} cursor {cursor:?}");
                        self.set_cursor(cursor);
                    }
                }
                WebEvent::FindResult { page, count, active, done } => {
                    let Some(id) = self.node_of(page) else { continue };
                    if self.omnibox.as_ref().is_some_and(|ob| ob.target == Target::Find(id)) {
                        self.found = Some((count, active));
                        if done {
                            log::info!("find: {active} of {count}");
                        }
                    }
                }
                WebEvent::Links { page, width, links } => {
                    if let Some(id) = self.node_of(page) {
                        self.got_links(id, width, links);
                    }
                }
                WebEvent::ContextMenu { page, x, y, context } => {
                    if let Some(id) = self.node_of(page) {
                        self.open_menu(id, x, y, &context);
                    }
                }
                WebEvent::NoteText { page, text } => {
                    if let Some(id) = self.node_of(page)
                        && self.scene.node(id).is_some_and(|n| n.note.as_deref() != Some(text.as_str()))
                        && self.scene.set_note(id, text)
                    {
                        self.note_target = Some(id);
                        self.dirty = true;
                    }
                }
                WebEvent::Favicon { page, page_url, width, height, pixels } => {
                    if self.node_of(page).is_some() {
                        self.set_icon(&page_url, width, height, pixels);
                    }
                }
                WebEvent::LoadingChanged { page, loading } => {
                    let Some(id) = self.node_of(page) else { continue };
                    if let Some(c) = self.cards.get_mut(&id) {
                        c.loading = loading;
                    }
                    if loading {
                        self.text_due.remove(&id);
                    } else {
                        self.text_due.insert(id, Instant::now() + TEXT_SETTLE);
                    }
                }
                WebEvent::PageText { page, url, title, text } => {
                    if let Some(id) = self.node_of(page)
                        && self.record_visit(&url, &title, &text)
                    {
                        self.save_thumb(id, &url);
                    }
                }
            }
        }
    }

    /// Asks pages whose text is due (loaded and settled) for it, for history.
    fn read_due_texts(&mut self) {
        if self.text_due.is_empty() {
            return;
        }
        if self.is_private() || !self.settings.history {
            self.text_due.clear();
            return;
        }
        let now = Instant::now();
        let due: Vec<NodeId> = self.text_due.iter().filter(|(_, t)| **t <= now).map(|(id, _)| *id).collect();
        for id in due {
            self.text_due.remove(&id);
            if self.page_index.is_some()
                && let Some(c) = self.cards.get(&id)
                && !c.loading
            {
                c.page.request_text();
            }
        }
    }

    /// Keeps a page's text in the history index. Returns whether it was kept.
    fn record_visit(&mut self, url: &str, title: &str, text: &str) -> bool {
        if self.is_private() || !self.settings.history {
            return false;
        }
        let Some(index) = self.page_index.as_mut() else { return false };
        match index.record(url, title, text, now_ms()) {
            Ok(true) => {
                log::info!("history: {url} {title:?} ({} chars)", text.chars().count());
                self.visits_cache.replace(None);
                true
            }
            Ok(false) => false,
            Err(e) => {
                log::warn!("history: recording {url}: {e}");
                false
            }
        }
    }

    /// Keeps what card `id` shows as the thumbnail of `url`, the page just
    /// recorded in history.
    fn save_thumb(&mut self, id: NodeId, url: &str) {
        let Some(card) = self.cards.get(&id) else { return };
        let started = Instant::now();
        let thumb = {
            let f = card.page.frame();
            storage::thumbs::Thumb::from_frame(f.width, f.height, &f.pixels)
        };
        let Some((thumb, jpeg)) = thumb.and_then(|t| t.encode().map(|j| (t, j))) else {
            log::info!("thumb: {url}: nothing painted yet");
            return;
        };
        let Some(index) = self.page_index.as_mut() else { return };
        match index.set_thumb(url, &jpeg) {
            Ok(_) => {
                log::info!("thumb: {url} {thumb:?}, {} bytes, {} ms", jpeg.len(), started.elapsed().as_millis());
                if self.preview.as_ref().is_some_and(|p| p.url == url) {
                    self.preview = None;
                }
            }
            Err(e) => log::warn!("thumb: saving {url}: {e}"),
        }
    }

    /// The thumbnail history keeps for `url`, decoded.
    fn load_thumb(&self, url: &str) -> Option<storage::thumbs::Thumb> {
        let jpeg = match self.page_index.as_ref()?.thumb(url) {
            Ok(j) => j?,
            Err(e) => {
                log::warn!("thumb: reading {url}: {e}");
                return None;
            }
        };
        let t = storage::thumbs::Thumb::decode(&jpeg);
        if t.is_none() {
            log::warn!("thumb: {url}: unreadable ({} bytes)", jpeg.len());
        }
        t
    }

    /// Uploads the thumbnail of `url` as the palette's preview, if it isn't
    /// already. Returns its size, or `None` when there's nothing to show.
    fn load_preview(&mut self, url: Option<String>) -> Option<(u32, u32)> {
        let url = url?;
        if self.preview.as_ref().is_none_or(|p| p.url != url) {
            let size = self.load_thumb(&url).and_then(|t| {
                self.preview_serial += 1;
                let r = self.renderer.as_mut()?;
                r.update_page(PREVIEW_KEY, t.width, t.height, &t.pixels, self.preview_serial);
                Some((t.width, t.height))
            });
            log::info!("preview: {url} {size:?}");
            self.preview = Some(Preview { url, size });
        }
        self.preview.as_ref().and_then(|p| p.size)
    }

    /// The page a palette row would show in the preview: a history page, or
    /// a closed card's.
    fn preview_url(&self, row: &palette::Row) -> Option<String> {
        if let Some(url) = &row.history {
            return Some(url.clone());
        }
        match row.item {
            palette::Item::Closed(id) => {
                self.closed.iter().map(|(n, _)| n).find(|n| n.id == id && !n.is_note()).map(|n| n.url.clone())
            }
            _ => None,
        }
    }

    /// Gives sleeping cards on screen that have no picture (restored after a
    /// restart, or back from another space) their page's thumbnail, dimmed
    /// like a snapshot, a few per frame. Each card is looked up once.
    fn load_sleeping_thumbs(&mut self) {
        if self.page_index.is_none() || self.read.is_some() {
            return;
        }
        let Some(r) = self.renderer.as_ref() else { return };
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let want: Vec<(u64, String, f32)> = self
            .scene
            .draw_order()
            .into_iter()
            .filter(|n| !n.is_note() && !self.cards.contains_key(&n.id) && n.rect.h > 0.0)
            .map(|n| (self.page_key(n.id), n))
            .filter(|(key, n)| {
                !self.thumb_tried.contains(key)
                    && !r.has_page(*key)
                    && self.to_screen_rect(n.frame()).inflate(40.0).intersects(&screen)
            })
            .take(THUMBS_PER_FRAME)
            .map(|(key, n)| (key, n.url.clone(), n.rect.w / n.rect.h))
            .collect();
        for (key, url, aspect) in want {
            self.thumb_tried.insert(key);
            let Some(t) = self.load_thumb(&url) else { continue };
            let t = t.crop_to(aspect);
            log::info!("thumb for sleeping card {}: {url} {t:?}", key & ((1 << KEY_ID_BITS) - 1));
            if let Some(r) = self.renderer.as_mut() {
                r.update_page(key, t.width, t.height, &t.pixels, u64::MAX - 1);
            }
            self.dirty = true;
        }
    }

    /// Keeps `pixels` as the icon of `page_url`'s site, saving it when it is new.
    fn set_icon(&mut self, page_url: &str, width: u32, height: u32, pixels: Vec<u8>) {
        let Some(site) = scene::icon::site(page_url) else { return };
        let Some(icon) = storage::icons::Icon::new(width, height, pixels) else {
            log::warn!("icon for {site}: unusable size {width}x{height}");
            return;
        };
        let private = self.is_private();
        let known = self.icons.get(&site).filter(|s| s.icon == icon);
        if known.is_some_and(|s| s.saved || private) {
            return;
        }
        // A private space's sites are shown this session but not written down.
        let saved = if private {
            log::info!("icon for {site}: not saved (private space)");
            false
        } else if let Err(e) = storage::icons::save_icon(&self.data_dir, &site, &icon) {
            log::warn!("saving the icon for {site}: {e}");
            false
        } else {
            true
        };
        if known.is_some() {
            // Seen before only privately: now it is on disk too, the texture is the same.
            if let Some(s) = self.icons.get_mut(&site) {
                s.saved = saved;
            }
            return;
        }
        log::info!("icon for {site}: {width}x{height}");
        let next = ICON_KEY | self.icons.len() as u64;
        let entry = self.icons.entry(site).or_insert(SiteIcon { key: next, serial: 0, icon: icon.clone(), saved });
        entry.icon = icon;
        entry.saved = saved;
        entry.serial += 1;
    }

    /// The card whose name shows when titles are too small to read: the one
    /// under the pointer, or else the current card. None while something else
    /// has the pointer's attention (a drag, a menu, the palette, link hints).
    fn named_card(&self) -> Option<NodeId> {
        if !matches!(self.drag, Drag::None) || self.menu.is_some() || self.omnibox.is_some() || self.link_hints.is_some() || self.hints {
            return None;
        }
        let over_panel = self.sidebar_shown() && self.sidebar_panel().rect.contains(self.cursor);
        let hovered = if self.pointer_in && !over_panel { self.hit(self.cursor).id() } else { None };
        hovered.or(self.focused).filter(|id| self.scene.node(*id).is_some())
    }

    /// A screen-sized label with the named card's icon and title, under the card.
    fn name_label_layer(&mut self) -> Option<Layer> {
        let id = self.named_card()?;
        let n = self.scene.node(id)?;
        let th = self.theme.clone();
        let (ui, vp) = (self.ui(), self.viewport());
        let size = 13.0 * ui;
        let pad = 9.0 * ui;
        let h = size * 2.0;
        let icon_px = 16.0 * ui;
        let icon = self.icon_key(&n.url);
        let title = if n.title.is_empty() { n.url.clone() } else { n.title.clone() };
        let title = if self.cards.contains_key(&id) { title } else { format!("{title} · asleep") };
        let frame = self.to_screen_rect(n.frame());
        let tw = self.renderer.as_mut().map_or(200.0, |r| r.measure(&title, size, true));
        let lead = if icon.is_some() { icon_px + 7.0 * ui } else { 0.0 };
        let w = (pad * 2.0 + lead + tw).min(480.0 * ui).min(vp.x - 16.0 * ui);
        let rect = scene::icon::name_label(frame, Vec2::new(w, h), vp, 6.0 * ui, 8.0 * ui);
        let mut l = Layer::default();
        crate::chrome::panel(&mut l, rect, th.radius(ui), &th, ui, crate::chrome::Lift::Low, th.card(true));
        if let Some(key) = icon {
            l.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(rect.x + pad, rect.y + (h - icon_px) / 2.0, icon_px, icon_px), radius: 0.0 },
                color: [1.0; 4],
                texture: Some(key),
            });
        }
        let x = rect.x + pad + lead;
        l.texts.push(Text {
            text: title,
            pos: [x, rect.y + (h - size * 1.3) / 2.0],
            size,
            color: rgb(th.foreground(), 1.0),
            clip: Rect::new(x, rect.y, (rect.right() - pad - x).max(1.0), h),
            bold: true,
        });
        Some(l)
    }

    /// The texture key of `url`'s site icon, if there is one.
    fn icon_key(&self, url: &str) -> Option<u64> {
        let key = self.icons.get(&scene::icon::site(url)?)?.key;
        self.renderer.as_ref().is_some_and(|r| r.has_page(key)).then_some(key)
    }

    // ---------------------------------------------------------------- input

    fn hit(&self, screen: Vec2) -> Hit {
        if let Some(id) = self.read {
            return if self.read_layout().content.contains(screen) { Hit::Content(id) } else { Hit::ReadBar(id) };
        }
        let w = self.to_world(screen);
        let top = self.scene.topmost_at(w);
        // Badges stick out past their card's corner, so they are hit first,
        // unless a card drawn above covers them.
        let top_z = top.and_then(|t| self.scene.node(t)).map(|n| n.z);
        let badge = self
            .scene
            .stacks()
            .iter()
            .filter_map(|s| self.scene.node(s.front))
            .filter(|n| top_z.is_none_or(|z| n.z >= z))
            .filter(|n| self.stack_badge(n).is_some_and(|b| b.contains(screen)))
            .max_by_key(|n| n.z);
        if let Some(n) = badge {
            return Hit::Badge(n.id);
        }
        let Some(id) = top else { return Hit::Board };
        let n = self.scene.node(id).expect("hit node exists");
        let k = self.camera.pixels_per_unit(self.scale());
        let grip = RESIZE_GRIP / k.min(1.0);
        if w.y < n.rect.y {
            // A sticky note's strip only moves it; it has no ×.
            if !n.is_sticky() && w.x > n.rect.right() - CLOSE_W {
                return Hit::Close(id);
            }
            return Hit::Header(id);
        }
        if w.x > n.rect.right() - grip.min(n.rect.w / 4.0) && w.y > n.rect.bottom() - grip.min(n.rect.h / 4.0) {
            return Hit::Grip(id);
        }
        Hit::Content(id)
    }

    fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        let p = self.cursor;
        let mods = keymap::modifiers(self.mods);
        // Any click closes the shortcut sheet, and does nothing else.
        if self.hints {
            if state == ElementState::Pressed {
                self.hints = false;
                log::info!("hints hidden");
                self.menu_pressed = true; // swallow the release too
            }
            return;
        }
        // A click while asking to delete the space keeps it, and does nothing else.
        if let Some(confirm) = self.confirm {
            if state == ElementState::Pressed {
                self.confirm = None;
                self.toast(self.kept(confirm));
                self.menu_pressed = true;
            }
            return;
        }
        // A press ends link hints, then does what it always does.
        if state == ElementState::Pressed && self.link_hints.take().is_some() {
            log::info!("link hints cancelled by a click");
        }
        // A press keeps a keyboard arrangement, then does what it always does.
        if state == ElementState::Pressed {
            self.finish_arrange(true);
        }
        // The context menu takes clicks first. A click outside closes it; a right
        // click outside still goes through so it can open a new menu.
        if let Some(m) = &self.menu {
            if state == ElementState::Pressed {
                if m.rect.contains(p) {
                    self.menu_pressed = true;
                    return;
                }
                self.menu = None;
                if button != MouseButton::Right {
                    return;
                }
            } else if std::mem::take(&mut self.menu_pressed) {
                if let Some(i) = m.item_at(p) {
                    self.activate_menu_item(i);
                }
                return;
            }
        }
        if state == ElementState::Released && self.sidebar_press.is_some() && button == MouseButton::Left {
            self.menu_pressed = false;
            self.sidebar_release();
            return;
        }
        if state == ElementState::Released && std::mem::take(&mut self.menu_pressed) {
            return;
        }
        // The palette: a click on a result runs it, a click in the field or
        // the list keeps it open, a click elsewhere closes it and goes through.
        if state == ElementState::Pressed
            && let Some(ob) = self.omnibox.as_ref().filter(|ob| ob.target.is_list())
        {
            let rows = self.list_rows(ob);
            let panel = palette::Panel::new(self.viewport(), self.ui(), rows.len());
            let scroll = ob.scroll.min(rows.len().saturating_sub(panel.shown));
            if let Some(i) = panel.row_at(p, scroll, rows.len()) {
                self.menu_pressed = true; // swallow the release
                // A setting steps on (back with the right button); the panel stays.
                if let Item::Setting(k) = rows[i].item {
                    if let Some(ob) = self.omnibox.as_mut() {
                        ob.sel = i;
                    }
                    self.step_setting(k, if button == MouseButton::Right { -1 } else { 1 });
                    return;
                }
                // A status line explains itself; the panel stays.
                if let Item::Status(st) = rows[i].item {
                    if let Some(ob) = self.omnibox.as_mut() {
                        ob.sel = i;
                    }
                    self.explain_status(st);
                    return;
                }
                self.omnibox = None;
                if button == MouseButton::Left {
                    self.run_palette(rows[i].clone());
                }
                return;
            }
            if panel.contains(p) {
                self.menu_pressed = true;
                return;
            }
        }
        // The sidebar covers the board's left edge and takes its clicks; a
        // press anywhere else takes the keys from it (it stays open).
        if state == ElementState::Pressed && self.sidebar_shown() {
            if self.sidebar_panel().rect.contains(p) {
                self.sidebar_click(p, button);
                self.menu_pressed = true; // swallow the release
                return;
            }
            self.sidebar_keys = false;
        }
        log::debug!("mouse {state:?} {button:?} at {p:?} hit={:?}", self.hit(p));
        if state == ElementState::Pressed {
            // Track multi-clicks for the page and for double-click on title bars.
            let now = Instant::now();
            self.click_count = match self.last_click {
                Some((t, at, b))
                    if b == button && now - t < Duration::from_millis(400) && (at.x - p.x).abs() < 5.0 && (at.y - p.y).abs() < 5.0 =>
                {
                    self.click_count + 1
                }
                _ => 1,
            };
            self.last_click = Some((now, p, button));
            self.omnibox = None;
            self.camera_goal = None;

            if button == MouseButton::Middle {
                if let Hit::Content(id) = self.hit(p) {
                    // Middle-click in a page (open link in new card) goes to the page.
                    if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_button(x, y, mods, WebButton::Middle, false, 1);
                        self.drag = Drag::Page { id, button: WebButton::Middle };
                        return;
                    }
                }
                if self.read.is_none() {
                    self.drag = Drag::Pan { last: p };
                }
                return;
            }
            let web_button = match button {
                MouseButton::Left => WebButton::Left,
                MouseButton::Right => WebButton::Right,
                _ => return,
            };
            let shift = self.mods.shift_key();
            match self.hit(p) {
                Hit::Board if shift && button == MouseButton::Left => {
                    // Band adds to the current selection.
                    self.keys_in_page = false;
                    self.focus(None);
                    self.drag = Drag::Band { anchor: self.to_world(p) };
                }
                Hit::Board => {
                    self.keys_in_page = false;
                    self.focus(None);
                    self.selection.clear();
                    self.drag = Drag::Pan { last: p };
                }
                Hit::ReadBar(_) => {
                    self.keys_in_page = false;
                    if self.click_count >= 2 && button == MouseButton::Left {
                        self.leave_read();
                    }
                }
                Hit::Close(id) => self.close_card(id),
                Hit::Badge(id) => {
                    self.keys_in_page = false;
                    self.open_outline(id);
                    // Swallow the release; releasing over a row picks it, as in a native menu.
                    self.menu_pressed = true;
                }
                Hit::Header(id) => {
                    if shift {
                        self.selection.toggle(id);
                    } else if !self.selection.contains(id) {
                        self.selection.clear();
                    }
                    self.focus(Some(id));
                    self.keys_in_page = false;
                    if self.click_count >= 2 && !shift {
                        self.fill_view(id);
                        self.drag = Drag::None;
                    } else {
                        let n = self.scene.node(id).expect("hit node");
                        let w = self.to_world(p);
                        self.group = selection::drag_group(&self.scene, &self.selection, id);
                        self.drag = Drag::Move { id, grab: w - Vec2::new(n.rect.x, n.rect.y) };
                    }
                }
                Hit::Grip(id) => {
                    self.selection.clear();
                    self.focus(Some(id));
                    self.keys_in_page = false;
                    let start = self.scene.node(id).expect("hit node").rect;
                    self.drag = Drag::Resize { id, anchor: self.to_world(p), start };
                }
                Hit::Content(id) => {
                    self.selection.clear();
                    self.focus(Some(id));
                    self.keys_in_page = true;
                    if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_button(x, y, mods, web_button, false, self.click_count.min(3));
                    }
                    self.drag = Drag::Page { id, button: web_button };
                }
            }
        } else {
            if let Drag::Page { id, button: b } = self.drag
                && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                    card.page.mouse_button(x, y, mods, b, true, self.click_count.min(3));
                }
            match self.drag {
                Drag::Resize { id, start, .. } => self.history.moved(&self.scene, id, start),
                Drag::Move { .. } => self.history.moved_many(&self.scene, &std::mem::take(&mut self.group)),
                Drag::Band { .. } => {
                    let band = self.band().expect("band drag");
                    self.selection.extend(selection::in_band(&self.scene, band));
                    log::info!("selected {} card(s)", self.selection.len());
                }
                _ => {}
            }
            self.drag = Drag::None;
        }
    }

    fn cursor_moved(&mut self, p: Vec2) {
        self.pointer_in = true;
        if let Some(m) = self.menu.as_mut()
            && m.rect.contains(p)
            && matches!(self.drag, Drag::None)
        {
            m.hover = m.item_at(p);
            self.cursor = p;
            self.set_cursor(web::Cursor::Default);
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            m.hover = None;
        }
        let (vp, ui) = (self.viewport(), self.ui());
        let list_len = self.omnibox.as_ref().filter(|ob| ob.target.is_list()).map(|ob| self.list_rows(ob).len());
        if let (Some(len), Some(ob)) = (list_len, self.omnibox.as_mut()) {
            let panel = palette::Panel::new(vp, ui, len);
            if panel.contains(p) {
                if let Some(i) = panel.row_at(p, ob.scroll.min(len.saturating_sub(panel.shown)), len) {
                    ob.sel = i;
                }
                self.cursor = p;
                self.set_cursor(web::Cursor::Default);
                return;
            }
        }
        self.sidebar_hover = None;
        if self.sidebar_press.is_some() {
            self.sidebar_press_moved(p);
            self.cursor = p;
            self.set_cursor(web::Cursor::Default);
            return;
        }
        if self.sidebar_shown() && matches!(self.drag, Drag::None) {
            let panel = self.sidebar_panel();
            if panel.rect.contains(p) {
                let len = self.sidebar.rows(&self.scene).len();
                self.sidebar_hover = panel.row_at(p, self.sidebar.scroll, len);
                self.cursor = p;
                self.set_cursor(web::Cursor::Default);
                return;
            }
        }
        let prev = self.cursor;
        self.cursor = p;
        let mods = keymap::modifiers(self.mods);
        match self.drag {
            Drag::Pan { last } => {
                self.camera.pan_pixels(p - last, self.scale());
                self.drag = Drag::Pan { last: p };
                self.camera_moved();
            }
            Drag::Move { id, grab } => {
                self.dirty = true;
                let w = self.to_world(p);
                if let Some(&(_, start)) = self.group.iter().find(|(g, _)| *g == id) {
                    let d = Vec2::new(w.x - grab.x - start.x, w.y - grab.y - start.y);
                    selection::move_group(&mut self.scene, &self.group, d);
                }
            }
            Drag::Band { .. } => self.set_cursor(web::Cursor::Crosshair),
            Drag::Resize { id, anchor, start } => {
                self.dirty = true;
                let d = self.to_world(p) - anchor;
                if let Some(n) = self.scene.node_mut(id) {
                    n.rect.w = (start.w + d.x).max(MIN_CARD);
                    n.rect.h = (start.h + d.y).max(MIN_CARD);
                }
            }
            Drag::Page { id, button } => {
                let held = match button {
                    WebButton::Left => web::flags::LEFT_MOUSE,
                    WebButton::Middle => web::flags::MIDDLE_MOUSE,
                    WebButton::Right => web::flags::RIGHT_MOUSE,
                };
                if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                    card.page.mouse_move(x, y, mods | held, false);
                }
            }
            Drag::None => {
                let hit = self.hit(p);
                let prev_hit = self.hit(prev);
                if let Hit::Content(id) = hit
                    && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_move(x, y, mods, false);
                    }
                if let (Hit::Content(old), false) = (prev_hit, matches!(hit, Hit::Content(n) if Some(n) == prev_hit.id()))
                    && let (Some(card), Some((x, y))) = (self.cards.get(&old), self.page_point(old, p)) {
                        card.page.mouse_move(x, y, mods, true);
                    }
                let cursor = match hit {
                    Hit::Header(_) => web::Cursor::Grab,
                    Hit::Grip(_) => web::Cursor::ResizeSE,
                    Hit::Close(_) | Hit::Badge(_) => web::Cursor::Pointer,
                    Hit::Content(id) => self.cards.get(&id).map_or(web::Cursor::Default, |c| c.cursor),
                    Hit::Board | Hit::ReadBar(_) => web::Cursor::Default,
                };
                self.set_cursor(cursor);
            }
        }
    }

    fn set_cursor(&self, cursor: web::Cursor) {
        let Some(w) = self.window() else { return };
        match cursor_icon(cursor) {
            Some(icon) => {
                w.set_cursor_visible(true);
                w.set_cursor(icon);
            }
            None => w.set_cursor_visible(false),
        }
    }

    fn wheel(&mut self, delta: MouseScrollDelta, phase: TouchPhase) {
        let (dx, dy, lines) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x, y, true),
            MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32, false),
        };
        self.camera_goal = None;
        self.menu = None;
        self.link_hints = None; // scrolling a page moves its links
        if let Some(ob) = self.omnibox.as_ref().filter(|ob| ob.target.is_list()) {
            let len = self.list_rows(ob).len();
            let panel = palette::Panel::new(self.viewport(), self.ui(), len);
            if panel.rect.contains(self.cursor) {
                let rows = if lines { -dy * 3.0 } else { -dy / panel.row_h };
                let max = len.saturating_sub(panel.shown) as isize;
                let ob = self.omnibox.as_mut().expect("palette open");
                ob.scroll = (ob.scroll.min(max as usize) as isize + rows.round() as isize).clamp(0, max) as usize;
                return;
            }
        }
        if self.sidebar_shown() && self.sidebar_panel().rect.contains(self.cursor) {
            let panel = self.sidebar_panel();
            let len = self.sidebar.rows(&self.scene).len();
            let rows = if lines { -dy * 3.0 } else { -dy / panel.row_h };
            self.sidebar.scroll_by(rows.round() as isize, len, panel.shown);
            return;
        }
        if self.mods.control_key() {
            if self.read.is_some() {
                return; // the board isn't on screen
            }
            let steps = if lines { dy } else { dy / 60.0 };
            self.camera.zoom_about(1.15f32.powf(steps), self.cursor, self.viewport(), self.scale());
            self.camera_moved();
            return;
        }
        if let Hit::Content(id) | Hit::Grip(id) = self.hit(self.cursor)
            && Some(id) == self.focused {
                if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, self.cursor)) {
                    // A wheel notch goes at once and Chromium glides it;
                    // touchpad movement goes at once, precise, and may
                    // fling after (`tick`). See `scene::scroll`.
                    let speed = self.settings.scroll_speed.factor();
                    let d = Vec2::new(dx, dy);
                    let now = if lines {
                        self.page_scroll.wheel((id, x, y), d, speed)
                    } else {
                        let ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
                        self.page_scroll.touch((id, x, y), d, speed, ended)
                    };
                    if now != Vec2::default() {
                        card.page.mouse_wheel(x, y, keymap::modifiers(self.mods), now.x as i32, now.y as i32, !lines);
                    }
                }
                return;
            }
        if self.read.is_some() {
            return;
        }
        let (px, py) = if lines { (dx * 60.0, dy * 60.0) } else { (dx, dy) };
        self.camera.pan_pixels(Vec2::new(px, py), self.scale());
        self.camera_moved();
    }

    fn key(&mut self, ev: KeyIn) {
        let pressed = ev.pressed;
        let ctrl = self.mods.control_key();
        let shift = self.mods.shift_key();
        let alt = self.mods.alt_key();
        // Shortcuts follow the logical key (layout-aware), like browsers do.
        let code = ev.code;

        let is_question = ev.text.as_deref() == Some("?") || (shift && code == Some(KeyCode::Slash));
        if self.hints {
            // Esc or ? hides the sheet; other keys (except modifiers) hide it too and are dropped.
            if pressed && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)) {
                self.hints = false;
                log::info!("hints hidden");
            }
            return;
        }
        if let Some(confirm) = self.confirm {
            // Enter deletes the space; any other key (but a modifier) keeps it and is dropped.
            if pressed && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)) {
                self.confirm = None;
                match (ev.named, confirm) {
                    (Some(NamedKey::Enter), Confirm::DeleteSpace) => self.delete_space(),
                    (Some(NamedKey::Enter), Confirm::ClearHistory) => self.clear_history(),
                    _ => self.toast(self.kept(confirm)),
                }
            }
            return;
        }
        if self.link_hints.is_some() {
            // Letters type a label; Backspace takes one back; Esc or any other
            // key (but a modifier) ends the hints and is dropped.
            if pressed && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)) {
                self.link_hint_key(&ev, ctrl || alt);
            }
            return;
        }
        if self.arrange.is_some()
            && pressed
            && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super))
            && self.arrange_key(code)
        {
            return;
        }
        let f1 = ev.named == Some(NamedKey::F1) && !ctrl && !alt;
        if pressed && (f1 || (is_question && !ctrl && !alt && !self.typing() && self.omnibox.is_none() && self.menu.is_none())) {
            self.menu = None;
            self.hints = true;
            log::info!("hints shown");
            return;
        }

        if pressed && self.menu.is_some() {
            match ev.named {
                Some(NamedKey::Escape) => {
                    self.menu = None;
                    return;
                }
                Some(NamedKey::ArrowDown) => return self.menu.as_mut().expect("menu").step(1),
                _ if !ctrl && !alt && code == Some(KeyCode::KeyJ) => return self.menu.as_mut().expect("menu").step(1),
                _ if !ctrl && !alt && code == Some(KeyCode::KeyK) => return self.menu.as_mut().expect("menu").step(-1),
                Some(NamedKey::ArrowUp) => return self.menu.as_mut().expect("menu").step(-1),
                Some(NamedKey::Enter) => {
                    if let Some(i) = self.menu.as_ref().and_then(|m| m.hover) {
                        self.activate_menu_item(i);
                    }
                    return;
                }
                // Modifiers alone keep the menu (e.g. reaching for a shortcut).
                Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt) => {}
                _ => self.menu = None,
            }
        }

        // The palette's list: its length and how many rows fit, for the keys below.
        let palette_len = self.omnibox.as_ref().filter(|ob| ob.target.is_list()).map(|ob| {
            let len = self.list_rows(ob).len();
            (len, palette::Panel::new(self.viewport(), self.ui(), len).shown)
        });
        // The settings panel's highlighted setting, for Enter and ←/→.
        let setting_sel = self
            .omnibox
            .as_ref()
            .filter(|ob| ob.target == Target::Settings)
            .and_then(|ob| self.list_rows(ob).into_iter().nth(ob.sel).map(|r| r.item));
        if let Some(ob) = self.omnibox.as_mut() {
            if !pressed {
                return;
            }
            let before = ob.text.clone();
            if ctrl && code == Some(KeyCode::KeyJ) && ob.target == Target::Downloads {
                self.omnibox = None;
                return;
            }
            // Palette: Up/Down (Ctrl+K/J, Ctrl+P/N), PgUp/PgDn move the highlight.
            let step = palette_len.and_then(|(_, shown)| match ev.named {
                Some(NamedKey::ArrowDown) => Some(1),
                Some(NamedKey::ArrowUp) => Some(-1),
                Some(NamedKey::PageDown) => Some(shown as isize),
                Some(NamedKey::PageUp) => Some(-(shown as isize)),
                _ if ctrl && matches!(code, Some(KeyCode::KeyJ | KeyCode::KeyN)) => Some(1),
                _ if ctrl && code == Some(KeyCode::KeyP) => Some(-1),
                _ => None,
            });
            if let (Some(step), Some((len, shown))) = (step, palette_len) {
                if len > 0 {
                    ob.sel = (ob.sel as isize + step).clamp(0, len as isize - 1) as usize;
                    ob.scroll = scene::palette::keep_visible(ob.sel, ob.scroll, shown, len);
                }
                return;
            }
            match ev.named {
                Some(NamedKey::Escape) => self.omnibox = None,
                _ if ctrl && code == Some(KeyCode::KeyK) && ob.target == Target::Palette => self.omnibox = None,
                _ if ctrl && code == Some(KeyCode::KeyF) && matches!(ob.target, Target::Find(_)) => ob.select_all = true,
                Some(NamedKey::Enter) if matches!(ob.target, Target::Find(_)) => {
                    if let Target::Find(id) = ob.target
                        && let Some(c) = self.cards.get(&id)
                    {
                        c.page.find(&ob.text, !shift, true);
                    }
                }
                _ if ctrl && code == Some(KeyCode::KeyH) && ob.target == Target::History => self.omnibox = None,
                _ if ctrl && code == Some(KeyCode::Comma) && ob.target == Target::Settings => self.omnibox = None,
                // Settings: Enter or → steps the highlighted one on, Shift+Enter or ← back.
                Some(NamedKey::Enter | NamedKey::ArrowRight | NamedKey::ArrowLeft) if ob.target == Target::Settings => {
                    let back = ev.named == Some(NamedKey::ArrowLeft) || (shift && ev.named == Some(NamedKey::Enter));
                    match setting_sel {
                        Some(Item::Setting(k)) => self.step_setting(k, if back { -1 } else { 1 }),
                        Some(Item::Status(st)) if ev.named == Some(NamedKey::Enter) => self.explain_status(st),
                        _ => {}
                    }
                }
                Some(NamedKey::Delete) if shift && ob.target == Target::Downloads => self.drop_selected_download(),
                Some(NamedKey::Delete) if shift && ob.target.is_list() => self.forget_selected(),
                // Downloads: Ctrl+Enter shows the file in its folder.
                Some(NamedKey::Enter) if ctrl && ob.target == Target::Downloads => {
                    let ob = self.omnibox.take().expect("downloads open");
                    if let Some(palette::Item::Download(key)) = self.list_rows(&ob).into_iter().nth(ob.sel).map(|r| r.item) {
                        self.show_download_folder(key);
                    }
                }
                Some(NamedKey::Enter) if ob.target.is_list() => {
                    let ob = self.omnibox.take().expect("palette open");
                    let rows = self.list_rows(&ob);
                    if let Some(row) = rows.into_iter().nth(ob.sel) {
                        self.run_palette(row);
                    }
                }
                Some(NamedKey::Enter) => {
                    let ob = self.omnibox.take().expect("omnibox open");
                    if ob.target == Target::RenameSpace {
                        self.rename_space(&ob.text);
                    } else if let Target::NameTrail(id) = ob.target {
                        self.name_trail(id, &ob.text);
                    } else if let Some(url) = omnibox::resolve(&ob.text, self.settings.search) {
                        match (ob.target, self.focused_page()) {
                            (Target::Focused, Some(page)) => page.load(&url),
                            _ => {
                                // Typed an address: the new page is what you use next.
                                if self.new_card_at_center(&url).is_some() {
                                    self.keys_in_page = true;
                                }
                            }
                        }
                    }
                }
                Some(NamedKey::Backspace) => ob.backspace(),
                Some(NamedKey::Delete) => ob.delete(),
                Some(NamedKey::ArrowLeft) => ob.left(),
                Some(NamedKey::ArrowRight) => ob.right(),
                Some(NamedKey::Home) => ob.home(),
                Some(NamedKey::End) => ob.end(),
                _ if ctrl && code == Some(KeyCode::KeyA) => ob.select_all = true,
                _ if ctrl && matches!(code, Some(KeyCode::KeyC | KeyCode::KeyX)) => {
                    if ob.select_all && !ob.text.is_empty() {
                        let text = ob.text.clone();
                        if code == Some(KeyCode::KeyX) {
                            ob.insert("");
                        }
                        self.clipboard.set_text(&text);
                    }
                }
                _ if ctrl && code == Some(KeyCode::KeyV) => {
                    if let Some(t) = self.clipboard.text() {
                        let line = t.split_whitespace().collect::<Vec<_>>().join(" ");
                        self.omnibox.as_mut().expect("omnibox open").insert(&line);
                    }
                }
                _ => {
                    if !ctrl && !alt
                        && let Some(t) = ev.text.as_ref() {
                            let t: String = t.chars().filter(|c| !c.is_control()).collect();
                            if !t.is_empty() {
                                ob.insert(&t);
                            }
                        }
                }
            }
            // Find as you type.
            if let Some(ob) = self.omnibox.as_ref()
                && let Target::Find(id) = ob.target
                && ob.text != before
            {
                self.last_find = ob.text.clone();
                self.found = None;
                if let Some(c) = self.cards.get(&id) {
                    c.page.find(&ob.text, true, false);
                }
            }
            // New text, new results: back to the best one.
            if let Some(ob) = self.omnibox.as_mut()
                && ob.target.is_list()
                && ob.text != before
            {
                ob.sel = 0;
                ob.scroll = 0;
            }
            return;
        }

        // The trails sidebar, when it has the keys, takes its own (j/k, h/l,
        // Enter, Esc, B); the rest are board keys as usual.
        if pressed && !ctrl && !alt && self.drag_is_idle() && self.sidebar_has_keys() && self.sidebar_key(code, shift) {
            return;
        }

        if pressed {
            // App shortcuts first.
            let handled = match code {
                Some(KeyCode::KeyQ) if ctrl && !shift && !alt => {
                    log::info!("quit (Ctrl+Q)");
                    self.quit = true;
                    true
                }
                // Spaces.
                Some(KeyCode::KeyN) if ctrl && shift && !alt => {
                    self.new_space(false);
                    true
                }
                Some(KeyCode::PageDown | KeyCode::PageUp) if alt && !ctrl && !shift && self.drag_is_idle() => {
                    self.step_trail(if code == Some(KeyCode::PageDown) { 1 } else { -1 });
                    true
                }
                Some(KeyCode::PageDown | KeyCode::PageUp) if ctrl && !shift && !alt => {
                    let step = if code == Some(KeyCode::PageDown) { 1 } else { -1 };
                    self.switch_space(self.spaces.neighbour(self.spaces.active, step));
                    true
                }
                Some(KeyCode::F2) if !ctrl && !shift && !alt => {
                    let name = self.spaces.active_entry().name.clone();
                    self.omnibox = Some(Omnibox::new(&name, Target::RenameSpace));
                    true
                }
                Some(KeyCode::KeyB) if ctrl && shift && !alt => {
                    self.cycle_tint();
                    true
                }
                Some(KeyCode::KeyP) if ctrl && shift && !alt => {
                    self.toggle_private();
                    true
                }
                Some(KeyCode::KeyW) if ctrl && shift && !alt => {
                    self.ask_delete_space();
                    true
                }
                Some(KeyCode::KeyK) if ctrl && !shift && !alt => {
                    self.open_palette();
                    true
                }
                Some(KeyCode::KeyH) if ctrl && !shift && !alt => {
                    self.open_history();
                    true
                }
                Some(KeyCode::Comma) if ctrl && !shift && !alt => {
                    self.open_settings();
                    true
                }
                Some(KeyCode::KeyJ) if ctrl && !shift && !alt => {
                    self.open_downloads();
                    true
                }
                Some(KeyCode::KeyL) if ctrl => {
                    // A note has no address: the bar opens a new card instead.
                    let page = self.focused.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note());
                    let url = page.map(|n| n.url.clone()).unwrap_or_default();
                    let target = if page.is_some() { Target::Focused } else { Target::NewCard };
                    self.omnibox = Some(Omnibox::new(&url, target));
                    true
                }
                Some(KeyCode::KeyC | KeyCode::KeyX) if ctrl && !alt && self.focused_page().is_some() => {
                    let cut = code == Some(KeyCode::KeyX);
                    self.page_copy(cut, Some(&ev));
                    true
                }
                Some(KeyCode::KeyV) if ctrl && !alt && self.focused_page().is_some() => {
                    self.page_paste(Some(&ev));
                    true
                }
                Some(KeyCode::KeyZ) if ctrl && !self.typing() => {
                    self.undo(shift);
                    true
                }
                Some(KeyCode::KeyY) if ctrl && !self.typing() => {
                    self.undo(true);
                    true
                }
                Some(KeyCode::KeyT) if ctrl && shift => {
                    self.reopen_closed();
                    true
                }
                Some(KeyCode::KeyT) if ctrl => {
                    self.omnibox = Some(Omnibox::new("", Target::NewCard));
                    true
                }
                Some(KeyCode::KeyW) if ctrl => {
                    if let Some(id) = self.focused {
                        self.close_card(id);
                    }
                    true
                }
                Some(KeyCode::KeyS) if ctrl && !shift && !alt => {
                    let note = self.focused.filter(|&id| self.scene.node(id).is_some_and(Node::is_note));
                    self.save_markdown(note);
                    true
                }
                Some(KeyCode::KeyR) if ctrl => {
                    if let Some(p) = self.focused_page().filter(|p| p.kind() == web::PageKind::Web) {
                        p.reload();
                    }
                    true
                }
                Some(KeyCode::KeyF) if ctrl && !alt && !shift => {
                    self.open_find();
                    true
                }
                Some(KeyCode::ContextMenu) if !ctrl && !alt => {
                    self.page_menu_by_key(&ev);
                    true
                }
                Some(KeyCode::F10) if shift && !ctrl && !alt => {
                    self.page_menu_by_key(&ev);
                    true
                }
                Some(
                    KeyCode::Equal
                    | KeyCode::NumpadAdd
                    | KeyCode::Minus
                    | KeyCode::NumpadSubtract
                    | KeyCode::Digit0
                    | KeyCode::Numpad0,
                ) if ctrl && !alt => {
                    let dir = match code {
                        Some(KeyCode::Equal | KeyCode::NumpadAdd) => 1,
                        Some(KeyCode::Minus | KeyCode::NumpadSubtract) => -1,
                        _ => 0,
                    };
                    self.zoom_page(dir);
                    true
                }
                Some(KeyCode::ArrowLeft) if alt => {
                    if let Some(p) = self.focused_page() {
                        p.back();
                    }
                    true
                }
                Some(KeyCode::ArrowRight) if alt => {
                    if let Some(p) = self.focused_page() {
                        p.forward();
                    }
                    true
                }
                Some(KeyCode::Delete | KeyCode::Backspace)
                    if !self.selection.is_empty() && !self.typing() && !ctrl && !alt && self.drag_is_idle() =>
                {
                    let ids = self.selection.ids();
                    log::info!("closing {} selected card(s)", ids.len());
                    self.close_cards(&ids);
                    true
                }
                Some(KeyCode::KeyA) if ctrl && !shift && !self.typing() => {
                    self.selection.select_all(&self.scene);
                    true
                }
                Some(KeyCode::Tab) if !ctrl && !alt && !shift && !(self.keys_in_page && self.focused_page().is_some()) => {
                    self.toggle_read();
                    true
                }
                Some(KeyCode::KeyS) if !self.typing() && !ctrl && !alt && self.drag_is_idle() => {
                    self.toggle_fold();
                    true
                }
                Some(KeyCode::KeyO) if !self.typing() && !ctrl && !alt && !shift && self.drag_is_idle() => {
                    self.outline_in_view();
                    true
                }
                Some(KeyCode::BracketLeft | KeyCode::BracketRight)
                    if !self.typing() && !ctrl && !alt && self.drag_is_idle() =>
                {
                    self.peek_stack(if code == Some(KeyCode::BracketLeft) { -1 } else { 1 });
                    true
                }
                Some(KeyCode::Escape) if self.read.is_some() && self.drag_is_idle() => {
                    // The page sees Esc too (closes its menus); the card stays focused.
                    if let Some(p) = self.focused_page() {
                        p.send_key(&ev.to_web(self.mods));
                    }
                    self.leave_read();
                    true
                }
                Some(KeyCode::Escape) if self.focused.is_none() && !self.selection.is_empty() => {
                    self.selection.clear();
                    true
                }
                Some(KeyCode::Escape) if self.typing() && self.drag_is_idle() => {
                    // Let the page see Esc too (closes its menus), then give the
                    // keys back to the board; the card stays the current one.
                    if let Some(p) = self.focused_page() {
                        p.send_key(&ev.to_web(self.mods));
                    }
                    self.leave_page();
                    true
                }
                Some(KeyCode::Escape) if self.focused.is_some() && self.drag_is_idle() => {
                    self.selection.clear();
                    self.focus(None);
                    true
                }
                // Board shortcuts while no page has the keys.
                Some(
                    KeyCode::KeyH
                    | KeyCode::KeyJ
                    | KeyCode::KeyK
                    | KeyCode::KeyL
                    | KeyCode::ArrowLeft
                    | KeyCode::ArrowDown
                    | KeyCode::ArrowUp
                    | KeyCode::ArrowRight,
                ) if !self.typing() && !ctrl && !alt && self.drag_is_idle() => {
                    let dir = match code {
                        Some(KeyCode::KeyH | KeyCode::ArrowLeft) => Dir::Left,
                        Some(KeyCode::KeyJ | KeyCode::ArrowDown) => Dir::Down,
                        Some(KeyCode::KeyK | KeyCode::ArrowUp) => Dir::Up,
                        _ => Dir::Right,
                    };
                    if shift { self.pan_by_key(dir) } else { self.move_current(dir) }
                    true
                }
                Some(KeyCode::Equal | KeyCode::NumpadAdd | KeyCode::Minus | KeyCode::NumpadSubtract)
                    if !self.typing() && !ctrl && !alt && self.read.is_none() =>
                {
                    let zoom_in = matches!(code, Some(KeyCode::Equal | KeyCode::NumpadAdd));
                    self.zoom_by_key(zoom_in);
                    true
                }
                Some(KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space)
                    if !self.typing() && !ctrl && !alt && self.drag_is_idle() =>
                {
                    self.enter_current();
                    true
                }
                Some(KeyCode::Delete | KeyCode::Backspace)
                    if !self.typing() && self.focused.is_some() && !ctrl && !alt && self.drag_is_idle() =>
                {
                    let id = self.focused.expect("checked");
                    self.close_card(id);
                    true
                }
                Some(KeyCode::KeyQ) if !self.typing() && !ctrl && !alt && !shift => {
                    log::info!("quit (Q)");
                    self.quit = true;
                    true
                }
                Some(KeyCode::KeyF) if !self.typing() && !ctrl && !alt && shift => {
                    self.fit_all();
                    true
                }
                Some(KeyCode::KeyF) if !self.typing() && !ctrl && !alt && self.drag_is_idle() => {
                    self.show_link_hints();
                    true
                }
                Some(KeyCode::KeyT) if !self.typing() && !ctrl && !alt && self.drag_is_idle() => {
                    self.tidy_board();
                    true
                }
                Some(KeyCode::KeyM) if !self.typing() && !ctrl && !alt && !shift && self.drag_is_idle() => {
                    self.start_arrange();
                    true
                }
                Some(KeyCode::KeyN) if !self.typing() && !ctrl && !alt && shift && self.drag_is_idle() => {
                    self.new_sticky_at_center();
                    true
                }
                Some(KeyCode::KeyN) if !self.typing() && !ctrl && !alt && self.drag_is_idle() => {
                    self.new_note_at_center();
                    true
                }
                Some(KeyCode::KeyB) if !self.typing() && !ctrl && !alt && !shift && self.drag_is_idle() => {
                    self.toggle_sidebar();
                    true
                }
                Some(KeyCode::KeyV) if !self.typing() && !ctrl && !alt && !shift && self.drag_is_idle() => {
                    self.toggle_mark();
                    true
                }
                Some(KeyCode::KeyZ) if !self.typing() && !ctrl && !alt && !shift && self.drag_is_idle() && self.read.is_none() => {
                    self.fill_current();
                    true
                }
                Some(KeyCode::KeyD | KeyCode::KeyU) if !self.typing() && !ctrl && !alt && !shift => {
                    self.scroll_current(code == Some(KeyCode::KeyD));
                    true
                }
                Some(KeyCode::KeyR) if !self.typing() && !ctrl && !alt && !shift && self.drag_is_idle() => {
                    self.ask_trail_name();
                    true
                }
                Some(KeyCode::KeyC) if !self.typing() && !ctrl && !alt && !shift && self.drag_is_idle() => {
                    self.cycle_trail_colour();
                    true
                }
                _ => false,
            };
            if handled {
                return;
            }
        }
        if self.typing()
            && let Some(p) = self.focused_page()
        {
            p.send_key(&ev.to_web(self.mods));
        }
    }

    fn drag_is_idle(&self) -> bool {
        matches!(self.drag, Drag::None)
    }

    /// The selection band being drawn, in world units.
    fn band(&self) -> Option<Rect> {
        match self.drag {
            Drag::Band { anchor } => Some(Rect::from_corners(anchor, self.to_world(self.cursor))),
            _ => None,
        }
    }

    // ---------------------------------------------------------------- drawing

    fn draw(&mut self) {
        let scale = self.scale();
        let vp = self.viewport();
        let k = self.camera.pixels_per_unit(scale);

        // Upload new page frames.
        let epoch = self.epoch;
        if let Some(r) = self.renderer.as_mut() {
            for (id, card) in &self.cards {
                let f = card.page.frame();
                r.update_page(page_key(epoch, *id), f.width, f.height, &f.pixels, f.serial);
            }
            for s in self.icons.values() {
                r.update_page(s.key, s.icon.width, s.icon.height, &s.icon.pixels, s.serial);
            }
        }
        self.load_sleeping_thumbs();
        let preview_url = self
            .omnibox
            .as_ref()
            .filter(|ob| ob.target.is_list())
            .and_then(|ob| self.list_rows(ob).get(ob.sel).and_then(|row| self.preview_url(row)));
        let preview = self.load_preview(preview_url);

        let origin = self.camera.world_to_screen(Vec2::default(), vp, scale);
        let spacing = 32.0 * k;
        let th = self.theme.clone();
        let ui = self.ui();
        let tint = self.spaces.active_entry().tint.clone();
        let bg = Background {
            color: rgb(th.board(tint.as_deref()), 1.0),
            dot: rgb(th.dots(), (spacing / 24.0).clamp(0.0, 1.0)),
            grid_origin: [origin.x, origin.y],
            grid_spacing: spacing,
            dot_radius: (1.1 * scale).max(0.8),
            vignette: if th.light() { 0.04 } else { 0.13 },
        };

        let mut layers: Vec<Layer> = Vec::new();

        // Trail edges under everything: a gentle S-curve from source's right edge to target's left edge.
        let mut edges = Layer::default();
        let labels: HashMap<NodeId, scene::TrailMark> =
            if self.read.is_some() { HashMap::new() } else { self.scene.trail_labels().into_iter().collect() };
        let mut edge_colour: HashMap<NodeId, [u8; 3]> = HashMap::new();
        for (head, mark) in &labels {
            let c = th.trail(mark.colour.as_deref());
            edge_colour.extend(self.scene.trail_of(*head).into_iter().map(|id| (id, c)));
        }
        let board_edges: Vec<Edge> = if self.read.is_some() { Vec::new() } else { self.scene.visible_edges().copied().collect() };
        for e in &board_edges {
            let (Some(a), Some(b)) = (self.scene.node(e.from), self.scene.node(e.to)) else { continue };
            let fa = self.to_screen_rect(a.frame());
            let fb = self.to_screen_rect(b.frame());
            let (p0, p3) = if fb.x >= fa.right() {
                ([fa.right(), fa.y + 18.0 * k], [fb.x, fb.y + 18.0 * k])
            } else {
                ([fa.x, fa.y + 18.0 * k], [fb.right(), fb.y + 18.0 * k])
            };
            let dx = (p3[0] - p0[0]) * 0.5;
            let (p1, p2) = ([p0[0] + dx, p0[1]], [p3[0] - dx, p3[1]]);
            let colour = edge_colour.get(&e.from).copied().unwrap_or(th.accent());
            let mut prev = p0;
            for i in 1..=24 {
                let t = i as f32 / 24.0;
                let u = 1.0 - t;
                let x = u * u * u * p0[0] + 3.0 * u * u * t * p1[0] + 3.0 * u * t * t * p2[0] + t * t * t * p3[0];
                let y = u * u * u * p0[1] + 3.0 * u * u * t * p1[1] + 3.0 * u * t * t * p2[1] + t * t * t * p3[1];
                edges.quads.push(Quad {
                    shape: Shape::Segment { from: prev, to: [x, y], width: (2.0 * k).clamp(1.0, 4.0) },
                    color: rgb(colour, 0.7),
                    texture: None,
                });
                prev = [x, y];
            }
        }
        layers.push(edges);

        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let band = self.band();
        let in_band = band.map(|b| selection::in_band(&self.scene, b)).unwrap_or_default();
        let typing = self.typing();
        let board_cards = if self.read.is_some() { Vec::new() } else { self.scene.draw_order() };
        for n in board_cards {
            let frame = self.to_screen_rect(n.frame());
            if !frame.inflate(40.0).intersects(&screen) {
                continue;
            }
            let content = self.to_screen_rect(n.rect);
            let header = self.to_screen_rect(n.header());
            // Cards are windows on the board: Hyprland's rounding, scaled with the zoom.
            let radius = (th.rounding * k).min(th.radius(scale));
            let ring = |m: f32| if radius > 0.0 { radius + m } else { 0.0 };
            let focused = self.focused == Some(n.id);
            let card = self.cards.get(&n.id);
            let mut l = Layer::default();
            // A stack shows the edges of up to two cards behind its front, down and to the right.
            let stack = self.scene.stack_of(n.id).map(|s| (s.front_index() + 1, s.members.len()));
            if let Some((_, len)) = stack {
                // At least a few px apart, so a stack still reads when zoomed far out.
                let step = (10.0 * k).max(5.0 * scale);
                for i in (1..len.min(3)).rev() {
                    let back = frame.translate(Vec2::new(step * i as f32, step * i as f32));
                    l.quads.push(Quad {
                        shape: Shape::Shadow { rect: back.translate(Vec2::new(0.0, 4.0 * k)), radius, blur: (14.0 * k).max(2.0) },
                        color: th.shadow(0.3),
                        texture: None,
                    });
                    l.quads.push(Quad {
                        shape: Shape::Rect { rect: back, radius },
                        color: rgb(th.stack_edge(), 1.0),
                        texture: None,
                    });
                    l.quads.push(Quad {
                        shape: Shape::Rect { rect: back.inflate(-(1.0 * scale).max(1.0)), radius: (radius - 1.0).max(0.0) },
                        color: rgb(th.card(false), 1.0),
                        texture: None,
                    });
                }
            }
            // Two shadows, scaled with the zoom: a wide ambient one and a tight
            // contact one under the edge. The focused card floats a little higher.
            let lift = if focused { 1.35 } else { 1.0 };
            l.quads.push(Quad {
                shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 12.0 * k * lift)), radius, blur: (36.0 * k * lift).max(3.0) },
                color: th.shadow(if focused { 0.5 } else { 0.38 }),
                texture: None,
            });
            l.quads.push(Quad {
                shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 2.0 * k)), radius, blur: (5.0 * k).max(1.5) },
                color: th.shadow(0.35),
                texture: None,
            });
            if focused {
                // A soft glow of the accent around the current card, brighter
                // while it has the keys.
                let glow = if typing { 0.32 } else { 0.16 };
                l.quads.push(Quad {
                    shape: Shape::Shadow { rect: frame, radius: ring(2.0 * scale), blur: 16.0 * scale.max(1.0) },
                    color: rgb(th.accent(), glow),
                    texture: None,
                });
            } else if !(self.selection.contains(n.id) || in_band.contains(&n.id)) {
                // A hairline edge keeps cards crisp against the board.
                let bw = scale.max(1.0);
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: frame.inflate(bw), radius: ring(bw) },
                    color: rgb(crate::chrome::edge(&th), 0.9),
                    texture: None,
                });
            }
            if self.selection.contains(n.id) || in_band.contains(&n.id) {
                // Selection ring outside the focus ring, so both can show.
                let m = if focused { 6.0 } else { 3.0 };
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: frame.inflate(m * scale), radius: ring(m * scale) },
                    color: rgb(th.marked(), 1.0),
                    texture: None,
                });
            }
            if focused {
                // The page with the keys gets a heavier ring than a card that is only current.
                // The page with the keys gets a heavy, solid ring; a card that is
                // only current (board keys) a thin, softer one.
                // A card being moved by keyboard (M) gets the heavy ring too.
                let arranging = self.arrange.as_ref().is_some_and(|a| a.id == n.id);
                let (w, alpha) = if typing || arranging { (3.5, 1.0) } else { (2.0, 0.6) };
                let w = w * scale.max(1.0);
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: frame.inflate(w), radius: ring(w) },
                    color: rgb(th.accent(), alpha),
                    texture: None,
                });
            }
            // A sticky note's frame is its strip, in its colour.
            let sticky = n.sticky.as_deref().filter(|_| n.is_sticky());
            l.quads.push(Quad {
                shape: Shape::Rect { rect: frame, radius },
                color: rgb(sticky.map_or_else(|| th.card(focused), |c| th.sticky_strip(c, focused)), 1.0),
                texture: None,
            });
            let key = page_key(epoch, n.id);
            let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
            let asleep = card.is_none();
            l.quads.push(Quad {
                // Round the bottom corners like the frame, so the page doesn't square them off.
                shape: Shape::RoundBottom { rect: content, radius },
                color: match (has_tex, asleep) {
                    (true, false) => [1.0; 4],
                    (true, true) => [0.78, 0.78, 0.8, 1.0], // dimmed snapshot
                    // First paint pending, or asleep and never painted.
                    (false, _) => rgb(sticky.map_or_else(|| th.placeholder(), |c| th.sticky_paper(c)), 1.0),
                },
                texture: has_tex.then_some(key),
            });
            if asleep && !has_tex {
                let size = 15.0 * k;
                if size >= 6.0 {
                    let msg = "Click to load";
                    let w = self.renderer.as_mut().map_or(100.0, |r| r.measure(msg, size, false));
                    l.texts.push(Text {
                        text: msg.into(),
                        pos: [content.x + (content.w - w) / 2.0, content.y + (content.h - size * 1.3) / 2.0],
                        size,
                        color: rgb(th.dim_text(), 1.0),
                        clip: content,
                        bold: false,
                    });
                }
            }
            // Loading bar along the bottom of the title bar (a sticky note has none).
            if sticky.is_none() && card.is_some_and(|c| c.loading) {
                let t = (self.started.elapsed().as_secs_f32() * 0.8).fract();
                let w = header.w * 0.25;
                let x = header.x + (header.w - w) * t;
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(x, header.bottom() - 2.0, w, 2.0), radius: th.small_radius(1.0).min(1.0) },
                    color: rgb(th.accent(), 1.0),
                    texture: None,
                });
            }
            if sticky.is_none() && header.h >= 6.0 {
                // A faint highlight along the title bar's top and a hairline
                // where it meets the page.
                let hair = scale.max(1.0).min(header.h / 6.0);
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(header.x + radius, header.y, (header.w - 2.0 * radius).max(0.0), hair), radius: 0.0 },
                    color: if th.light() { rgb([255, 255, 255], 0.6) } else { rgb(th.foreground(), if focused { 0.09 } else { 0.05 }) },
                    texture: None,
                });
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(header.x, header.bottom() - hair, header.w, hair), radius: 0.0 },
                    color: th.shadow(0.35),
                    texture: None,
                });
            }
            let font = 13.0 * k;
            let pad = 10.0 * k;
            // The site's icon at the start of the title bar; it stays a little
            // further out than the title, so cards can be told apart by it.
            let icon = if sticky.is_none() { self.icon_key(&n.url) } else { None };
            let icon_px = 16.0 * k;
            let mut title_x = header.x + pad;
            if let Some(key) = icon.filter(|_| icon_px >= 4.0) {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(title_x, header.y + (header.h - icon_px) / 2.0, icon_px, icon_px), radius: 0.0 },
                    color: if asleep { [0.78, 0.78, 0.8, 1.0] } else { [1.0; 4] },
                    texture: Some(key),
                });
                title_x += icon_px + 6.0 * k;
            }
            if font >= 6.0 && sticky.is_none() {
                let title = if n.title.is_empty() { n.url.as_str() } else { n.title.as_str() };
                let title = if asleep { format!("{title} · asleep") } else { title.to_string() };
                let title = match stack {
                    Some((i, len)) => format!("{i}/{len} · {title}"),
                    None => title,
                };
                // Requests the blocker stopped on this page: a small shield
                // with the count, just left of ×. Not on an allowed site.
                let mut title_right = header.right() - CLOSE_W * k;
                let badge = self
                    .blocking_state(n.id)
                    .filter(|b| b.on)
                    .and_then(|b| scene::privacy::blocked_badge(b.blocked));
                if let Some(count) = badge {
                    let size = 11.5 * k;
                    let tw = self.renderer.as_mut().map_or(size * 1.2, |r| r.measure(&count, size, true));
                    let (sw, sh) = (9.0 * k, 11.0 * k);
                    let w = sw + 3.5 * k + tw;
                    // Only while the title keeps some room.
                    if title_right - w - 4.0 * k - title_x >= 48.0 * k {
                        let x = title_right - w;
                        let sy = header.y + (header.h - sh) / 2.0;
                        let shield = Rect::new(x, sy, sw, sh);
                        let ink = if focused { th.foreground() } else { th.dim_text() };
                        l.quads.push(Quad {
                            shape: Shape::RoundBottom { rect: shield, radius: sw / 2.0 },
                            color: rgb(ink, 0.85),
                            texture: None,
                        });
                        // Hollow it out where that stays visible, so it reads as an outline.
                        let edge = 1.4 * k;
                        if edge >= 1.0 {
                            let inner = Rect::new(shield.x + edge, shield.y + edge, sw - 2.0 * edge, sh - 2.0 * edge);
                            l.quads.push(Quad {
                                shape: Shape::RoundBottom { rect: inner, radius: inner.w / 2.0 },
                                color: rgb(th.card(focused), 1.0),
                                texture: None,
                            });
                        }
                        l.texts.push(Text {
                            text: count,
                            pos: [x + sw + 3.5 * k, header.y + (header.h - size * 1.3) / 2.0],
                            size,
                            color: rgb(ink, 0.85),
                            clip: header,
                            bold: true,
                        });
                        title_right = x - 4.0 * k;
                    }
                }
                let clip = Rect::new(title_x, header.y, (title_right - pad - title_x).max(1.0), header.h);
                l.texts.push(Text {
                    text: title,
                    pos: [title_x, header.y + (header.h - font * 1.3) / 2.0],
                    size: font,
                    color: match (focused, asleep) {
                        (true, _) => rgb(th.foreground(), 1.0),
                        (false, false) => rgb(crate::theme::mix(th.foreground(), th.dim_text(), 0.4), 1.0),
                        (false, true) => rgb(th.dim_text(), 1.0),
                    },
                    clip,
                    bold: true,
                });
                let close_size = 16.0 * k;
                l.texts.push(Text {
                    text: "×".into(),
                    pos: [header.right() - CLOSE_W * k + (CLOSE_W * k - close_size * 0.6) / 2.0, header.y + (header.h - close_size * 1.3) / 2.0],
                    size: close_size,
                    color: rgb(th.dim_text(), 1.0),
                    clip: header,
                    bold: false,
                });
            }
            // A stack's size on its top-left corner (clear of ×), readable at any zoom.
            if let (Some((_, len)), Some(pill)) = (stack, self.stack_badge(n)) {
                let size = 11.0 * self.ui();
                let text = len.to_string();
                let tw = self.renderer.as_mut().map_or(size * 0.6, |r| r.measure(&text, size, true));
                let (w, h) = (pill.w, pill.h);
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: pill, radius: if th.rounding > 0.0 { h / 2.0 } else { 0.0 } },
                    color: rgb(th.accent(), 1.0),
                    texture: None,
                });
                l.texts.push(Text {
                    text,
                    pos: [pill.x + (w - tw) / 2.0, pill.y + (h - size * 1.3) / 2.0],
                    size,
                    color: rgb(th.accent_foreground(), 1.0),
                    clip: pill,
                    bold: true,
                });
            }
            // A marked trail's name above its first card, in its colour (just a
            // dot for a colour without a name). Screen-sized, like the badge.
            if let Some(mark) = labels.get(&n.id) {
                let size = 12.0 * ui;
                let h = size * 1.7;
                let pad = size * 0.7;
                let colour = th.trail(mark.colour.as_deref());
                let name = mark.name.clone().unwrap_or_default();
                let tw = if name.is_empty() { 0.0 } else { self.renderer.as_mut().map_or(100.0, |r| r.measure(&name, size, true)) };
                let w = if name.is_empty() { h } else { (tw + 2.0 * pad).min(frame.w.max(h)) };
                // Clear of a stack's badge, which sticks out above the corner.
                let gap = if stack.is_some() { 9.0 } else { 5.0 } * ui;
                let tag = Rect::new(frame.x, frame.y - gap - h, w, h);
                let round = if th.rounding > 0.0 || name.is_empty() { h / 2.0 } else { 0.0 };
                l.quads.push(Quad { shape: Shape::Rect { rect: tag, radius: round }, color: rgb(colour, 1.0), texture: None });
                if !name.is_empty() {
                    l.texts.push(Text {
                        text: name,
                        pos: [tag.x + pad, tag.y + (h - size * 1.3) / 2.0],
                        size,
                        color: rgb(text_on(colour), 1.0),
                        clip: tag.inflate(-pad * 0.5),
                        bold: true,
                    });
                }
            }
            layers.push(l);
        }

        if let Some(id) = self.read {
            let l = self.read_layer(id);
            layers.push(l);
        }

        // Empty-board hint: a quiet heading, then the keys as keycaps.
        if self.scene.is_empty() && self.omnibox.is_none()
            && let Some(r) = self.renderer.as_mut()
        {
            layers.push(empty_hint(r, vp, &th, ui));
        }

        if self.read.is_none()
            && 13.0 * k < scene::icon::READABLE_TITLE
            && let Some(l) = self.name_label_layer()
        {
            layers.push(l);
        }

        if self.read.is_none()
            && let Some(l) = self.link_hint_layer()
        {
            layers.push(l);
        }

        let sidebar_w = if self.sidebar_shown() {
            let panel = self.sidebar_panel();
            let rows = self.sidebar.rows(&self.scene);
            self.sidebar.scroll_to(None, rows.len(), panel.shown);
            let keys = self.sidebar_has_keys();
            let marks = crate::sidebar::View {
                scroll: self.sidebar.scroll,
                sel: if keys { self.sidebar.index(&rows) } else { None },
                hover: self.sidebar_hover,
                current: self.focused,
                drop_line: self.sidebar_drop_line(&rows, &panel),
            };
            let came = self.anim.sidebar.update(Some(0), Instant::now());
            if let Some(r) = self.renderer.as_mut() {
                let mut l = crate::sidebar::layer(r, &panel, &rows, &marks, &self.scene, &th, ui);
                // Slides in from the window's left edge.
                crate::chrome::fade(&mut l, 0.4 + 0.6 * came, Vec2::new(-panel.rect.w * 0.25 * (1.0 - came), 0.0));
                layers.push(l);
            }
            panel.rect.w
        } else {
            self.anim.sidebar.update(None, Instant::now());
            0.0
        };

        let shown = self.omnibox.as_ref().map(|ob| anim_key(std::mem::discriminant(&ob.target)));
        let came = self.anim.omnibox.update(shown, Instant::now());
        if let Some(ob) = &self.omnibox {
            let r = palette::field_rect(vp, ui);
            let h = r.h;
            let size = 17.0 * ui;
            let pad = 18.0 * ui;
            let placeholder = match ob.target {
                Target::RenameSpace => "Space name",
                Target::NameTrail(_) => "Trail name (empty to clear)",
                Target::Palette => "Find a card, space or command",
                Target::Find(_) => "Find in page",
                Target::History => "Search history",
                Target::Settings => "Settings · Enter or → changes, ← goes back",
                Target::Downloads => "Downloads · Enter opens, Ctrl+Enter its folder, Shift+Del cancels or removes",
                _ => "Search or enter address",
            };
            // The list (if this field has one) hangs from the field: one sheet.
            let list = ob.target.is_list().then(|| {
                let mut rows = self.list_rows(ob);
                // The highlighted setting shows that ← and → change it.
                if ob.target == Target::Settings
                    && let Some(row) = rows.get_mut(ob.sel).filter(|r| matches!(r.item, Item::Setting(_)))
                {
                    row.detail = format!("‹  {}  ›", row.detail);
                }
                let panel = palette::Panel::new(vp, ui, rows.len());
                let scroll = ob.scroll.min(rows.len().saturating_sub(panel.shown));
                (rows, panel, scroll)
            });
            let selected = ob.select_all && !ob.text.is_empty();
            let (shown, color) = if ob.text.is_empty() {
                (placeholder.to_string(), rgb(th.dim_text(), 1.0))
            } else if selected {
                // Drawn on the accent, so in the colour that reads on it.
                (ob.text.clone(), rgb(th.accent_foreground(), 1.0))
            } else {
                (ob.text.clone(), rgb(th.foreground(), 1.0))
            };
            let caret_x = self.renderer.as_mut().map_or(0.0, |rd| rd.measure(ob.before_caret(), size, false));
            let text_w = self.renderer.as_mut().map_or(0.0, |rd| rd.measure(&ob.text, size, false));
            // Lists are modal: the board dims behind them.
            let mut fading: Vec<Layer> = Vec::new();
            if list.is_some() {
                // The veil only fades; the sheet also slides into place.
                layers.push(crate::chrome::scrim(screen, &th, came));
            }
            let mut l = Layer::default();
            let radius = th.radius(scale);
            let sheet = match &list {
                Some((_, panel, _)) => Rect::new(r.x, r.y, r.w, panel.rect.bottom() - r.y),
                None => r,
            };
            crate::chrome::panel(&mut l, sheet, radius, &th, ui, crate::chrome::Lift::High, th.panel());
            if list.is_some() {
                // A hairline between the field and its results.
                let line = ui.max(1.0);
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(r.x, r.bottom() - line, r.w, line), radius: 0.0 },
                    color: rgb(crate::chrome::edge(&th), 1.0),
                    texture: None,
                });
            }
            let caret_h = size * 1.35;
            let caret_y = r.y + (h - caret_h) / 2.0;
            if selected {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(r.x + pad, caret_y, text_w, caret_h), radius: th.small_radius(scale) },
                    color: rgb(th.accent(), 1.0),
                    texture: None,
                });
            } else {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(r.x + pad + caret_x, caret_y, 2.0 * scale.max(1.0), caret_h), radius: 0.0 },
                    color: rgb(th.accent(), 1.0),
                    texture: None,
                });
            }
            l.texts.push(Text {
                text: shown,
                pos: [r.x + pad, r.y + (h - size * 1.3) / 2.0],
                size,
                color,
                clip: r,
                bold: false,
            });
            // Find in page: "3 of 17" (or "No matches") at the right end.
            if matches!(ob.target, Target::Find(_)) && !ob.text.is_empty()
                && let Some((count, active)) = self.found
            {
                // Chromium picks an active match only once Enter steps to one.
                let msg = match (count, active) {
                    (0, _) => "No matches".to_string(),
                    (1, 0) => "1 match".to_string(),
                    (n, 0) => format!("{n} matches"),
                    (n, a) => format!("{a} of {n}"),
                };
                let w = self.renderer.as_mut().map_or(0.0, |rd| rd.measure(&msg, size, false));
                l.texts.push(Text {
                    text: msg,
                    pos: [r.right() - pad - w, r.y + (h - size * 1.3) / 2.0],
                    size,
                    color: rgb(th.dim_text(), 1.0),
                    clip: r,
                    bold: false,
                });
            }
            fading.push(l);
            if let Some((rows, panel, scroll)) = list {
                let empty = match (&ob.target, ob.text.trim().is_empty(), &self.page_index) {
                    (Target::History, _, None) => "History is off",
                    (Target::History, true, _) => "No pages in history",
                    (Target::Settings, _, _) => "No settings match",
                    (Target::Downloads, true, _) => "No downloads yet",
                    _ => "Nothing found",
                };
                if let Some(rd) = self.renderer.as_mut() {
                    fading.push(palette::layer(rd, &panel, &rows, ob.sel, scroll, empty, &th, ui));
                }
                if let Some(size) = preview
                    && let Some(at) = panel.preview_rect(vp, size, ui)
                {
                    fading.push(palette::preview_layer(at, PREVIEW_KEY, &th, ui));
                }
            }
            for mut l in fading {
                crate::chrome::arrive(&mut l, came, 8.0 * ui);
                layers.push(l);
            }
        }

        if let Some(b) = band {
            let r = self.to_screen_rect(b);
            let t = scale.max(1.0);
            let line = rgb(th.marked(), 0.9);
            let mut l = Layer::default();
            l.quads.push(Quad { shape: Shape::Rect { rect: r, radius: 0.0 }, color: rgb(th.marked(), 0.12), texture: None });
            for edge in [
                Rect::new(r.x, r.y, r.w, t),
                Rect::new(r.x, r.bottom() - t, r.w, t),
                Rect::new(r.x, r.y, t, r.h),
                Rect::new(r.right() - t, r.y, t, r.h),
            ] {
                l.quads.push(Quad { shape: Shape::Rect { rect: edge, radius: 0.0 }, color: line, texture: None });
            }
            layers.push(l);
        }

        let came = self.anim.menu.update(self.menu.as_ref().map(|m| anim_key((m.rect.x as i32, m.rect.y as i32))), Instant::now());
        if let Some(m) = &self.menu {
            let mut l = menu_layer(m, ui, &th);
            crate::chrome::arrive(&mut l, came, 4.0 * ui);
            layers.push(l);
        }

        // Space name in the top-left corner once there is more than one space.
        if (self.spaces.spaces.len() > 1 || self.is_private()) && self.omnibox.is_none() && self.read.is_none() {
            let label = self.space_label();
            let color = th.swatch(tint.as_deref());
            layers.push(self.pill(&label, Vec2::new(sidebar_w + 12.0 * ui, 12.0 * ui), 13.0 * ui, Some(color), false));
        }
        // While arranging by keyboard, the keys stay on screen at the bottom.
        if self.arrange.is_some() && self.omnibox.is_none() {
            let size = 14.0 * ui;
            let msg = ARRANGE_HELP;
            let w = self.renderer.as_mut().map_or(300.0, |r| r.measure(msg, size, false));
            let pos = Vec2::new((vp.x - w) / 2.0 - 16.0 * ui, vp.y - 54.0 * ui);
            layers.push(self.pill(msg, pos, size, None, false));
        }
        // Over the settings panel a toast hangs below the list (it answers a
        // row there); other lists hide it.
        let settings_bottom = self
            .omnibox
            .as_ref()
            .filter(|ob| ob.target == Target::Settings)
            .map(|ob| palette::Panel::new(vp, ui, self.list_rows(ob).len()).rect.bottom());
        if let Some((msg, at)) = &self.toast {
            if self.confirm.is_some() || Instant::now() < *at {
                if self.omnibox.is_none() || settings_bottom.is_some() {
                    let size = 15.0 * ui;
                    let msg = msg.clone();
                    let w = self.renderer.as_mut().map_or(300.0, |r| r.measure(&msg, size, false));
                    // Below Read Mode's bar, so it doesn't cover the title.
                    let top = if self.read.is_some() { READ_BAR * ui } else { 0.0 };
                    let y = settings_bottom.map_or(top + 18.0 * ui, |b| b + 14.0 * ui);
                    let pos = Vec2::new((vp.x - w) / 2.0 - 16.0 * ui, y);
                    let warn = self.confirm.is_some();
                    let came = self.anim.toast.update(Some(anim_key(&msg)), Instant::now());
                    let mut l = self.pill(&msg, pos, size, None, warn);
                    crate::chrome::arrive(&mut l, came, 10.0 * ui);
                    layers.push(l);
                }
            } else {
                self.toast = None;
            }
        }
        if self.toast.is_none() || (self.omnibox.is_some() && settings_bottom.is_none()) {
            self.anim.toast.update(None, Instant::now());
        }

        let came = self.anim.sheet.update(self.hints.then_some(0), Instant::now());
        if self.hints
            && let Some(r) = self.renderer.as_mut()
        {
            let mut l = crate::hints::layer(r, ui, screen, &th);
            crate::chrome::fade(&mut l, came, Vec2::new(0.0, 0.0));
            layers.push(l);
        }

        if let Some(r) = self.renderer.as_mut() {
            r.render(bg, &layers);
        }
    }

    /// Read Mode: card `id`'s page across the window under a title bar.
    fn read_layer(&mut self, id: NodeId) -> Layer {
        let th = self.theme.clone();
        let ui = self.ui();
        let scale = self.scale();
        let lay = self.read_layout();
        let key = self.page_key(id);
        let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
        let loading = self.cards.get(&id).is_some_and(|c| c.loading);
        let title = self.scene.node(id).map_or(String::new(), |n| if n.title.is_empty() { n.url.clone() } else { n.title.clone() });
        let mut l = Layer::default();
        l.quads.push(Quad {
            shape: Shape::Rect { rect: lay.content, radius: 0.0 },
            color: if has_tex { [1.0; 4] } else { rgb(th.placeholder(), 1.0) },
            texture: has_tex.then_some(key),
        });
        let bar = lay.bar;
        l.quads.push(Quad { shape: Shape::Rect { rect: bar, radius: 0.0 }, color: rgb(th.card(true), 1.0), texture: None });
        l.quads.push(Quad {
            shape: Shape::Rect { rect: Rect::new(bar.x, bar.bottom() - scale.max(1.0), bar.w, scale.max(1.0)), radius: 0.0 },
            color: rgb(th.divider(), 1.0),
            texture: None,
        });
        if loading {
            let t = (self.started.elapsed().as_secs_f32() * 0.8).fract();
            let w = bar.w * 0.25;
            l.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(bar.x + (bar.w - w) * t, bar.bottom() - 2.0 * scale.max(1.0), w, 2.0 * scale.max(1.0)), radius: 0.0 },
                color: rgb(th.accent(), 1.0),
                texture: None,
            });
        }
        let size = 13.0 * ui;
        let pad = 14.0 * ui;
        let hint = "Tab · Esc  Board";
        let hint_w = self.renderer.as_mut().map_or(120.0, |r| r.measure(hint, size, false));
        let y = bar.y + (bar.h - size * 1.3) / 2.0;
        l.texts.push(Text {
            text: title,
            pos: [bar.x + pad, y],
            size,
            color: rgb(th.foreground(), 1.0),
            clip: Rect::new(bar.x + pad, bar.y, (bar.w - hint_w - pad * 3.0).max(1.0), bar.h),
            bold: true,
        });
        l.texts.push(Text { text: hint.into(), pos: [bar.right() - pad - hint_w, y], size, color: rgb(th.dim_text(), 1.0), clip: bar, bold: false });
        l
    }

    /// A rounded label with its top-left at `pos` (screen px). `swatch` draws a
    /// colour dot before the text; `warn` uses a red outline.
    fn pill(&mut self, text: &str, pos: Vec2, size: f32, swatch: Option<[u8; 3]>, warn: bool) -> Layer {
        let th = &self.theme;
        let scale = self.scale();
        let radius = th.radius(scale);
        let pad = size * 1.05;
        let h = size * 2.3;
        let dot = if swatch.is_some() { size * 0.8 + size * 0.6 } else { 0.0 };
        let w = self.renderer.as_mut().map_or(200.0, |r| r.measure(text, size, false)) + pad * 2.0 + dot;
        let r = Rect::new(pos.x, pos.y, w, h);
        let mut l = Layer::default();
        // A question that loses data gets a red edge.
        let (rim, bw) = if warn { (th.danger(), 1.5 * scale.max(1.0)) } else { (crate::chrome::edge(th), scale.max(1.0)) };
        crate::chrome::panel_edged(&mut l, r, radius, th, scale, crate::chrome::Lift::Mid, th.panel(), rim, bw);
        if let Some(c) = swatch {
            // A true circle keeps its round shape whatever the rounding.
            let d = size * 0.8;
            l.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(r.x + pad, r.y + (h - d) / 2.0, d, d), radius: d / 2.0 },
                color: rgb(c, 1.0),
                texture: None,
            });
        }
        l.texts.push(Text {
            text: text.into(),
            pos: [r.x + pad + dot, r.y + (h - size * 1.3) / 2.0],
            size,
            color: rgb(self.theme.foreground(), 1.0),
            clip: r,
            bold: false,
        });
        l
    }

    /// Sends this frame's part of a touchpad fling to its page.
    fn step_page_scroll(&mut self, dt: f32) {
        let Some(((id, x, y), d)) = self.page_scroll.step(dt) else { return };
        match self.cards.get(&id) {
            Some(card) => card.page.mouse_wheel(x, y, keymap::modifiers(self.mods), d.x as i32, d.y as i32, true),
            // The card closed or fell asleep.
            None => self.page_scroll.stop(),
        }
    }

    /// One iteration of app work outside winit callbacks.
    pub fn tick(&mut self, dt: f32) {
        self.run_script();
        self.follow_desktop();
        self.handle_web_events();
        if let Some(msg) = self.blocking.tick(Instant::now(), self.settings.block_ads) {
            self.toast(msg);
        }
        self.read_due_texts();
        self.sync_find();
        self.autosave();
        self.step_camera(dt);
        self.step_page_scroll(dt);
        self.sync_pages();
        self.sync_ime();
        if let Some(w) = self.window() {
            w.request_redraw();
        }
    }
}

// ------------------------------------------------------------ clipboard, IME, context menu

impl App {
    /// Ctrl+C / Ctrl+X (`key`) or the menu's Copy / Cut (`None`) in the focused page.
    fn page_copy(&mut self, cut: bool, key: Option<&KeyIn>) {
        let Some(page) = self.focused_page() else { return };
        // Read the selection before a cut removes it.
        let text = page.selected_text();
        match key {
            Some(k) => page.send_key(&k.to_web(self.mods)),
            None => page.edit(if cut { EditCommand::Cut } else { EditCommand::Copy }),
        }
        log::debug!("copy from page: {} chars", text.chars().count());
        self.clipboard.copied_from_page(text);
    }

    /// Ctrl+V (`key`) or the menu's Paste (`None`) in the focused page.
    fn page_paste(&mut self, key: Option<&KeyIn>) {
        let plan = self.clipboard.paste_plan();
        let Some(page) = self.focused_page() else { return };
        match plan {
            Paste::Text(t) => {
                log::debug!("paste desktop text: {} chars", t.chars().count());
                page.paste_text(&t);
            }
            Paste::Native => match key {
                Some(k) => page.send_key(&k.to_web(self.mods)),
                None => page.edit(EditCommand::Paste),
            },
        }
    }

    fn ime(&mut self, ime: Ime) {
        log::debug!("ime {ime:?}");
        if let Some(ob) = self.omnibox.as_mut() {
            // The address bar takes committed text only; it doesn't show compositions.
            if let Ime::Commit(t) = ime {
                ob.insert(&t);
                ob.sel = 0;
                ob.scroll = 0;
            }
            return;
        }
        let Some(id) = self.focused else { return };
        let Some(page) = self.cards.get(&id).map(|c| &c.page) else { return };
        match ime {
            Ime::Preedit(text, cursor) if !text.is_empty() => {
                page.ime_set_composition(&text, cursor);
                self.ime_page = Some(id);
            }
            Ime::Preedit(..) | Ime::Disabled => {
                if self.ime_page.take().is_some() {
                    page.ime_cancel();
                }
            }
            Ime::Commit(text) => {
                page.ime_commit(&text);
                self.ime_page = None;
            }
            Ime::Enabled => {}
        }
    }

    /// Enable IME while text can go somewhere, and keep the candidate window by the caret.
    fn sync_ime(&mut self) {
        let page_keys = self.keys_in_page && self.focused_page().is_some();
        // A test script feeds IME events itself; keep a running input method out of it.
        let allowed = (self.omnibox.is_some() || page_keys) && self.script.is_none();
        // A composition left in a page that lost focus is dropped.
        if let Some(id) = self.ime_page
            && (Some(id) != self.focused || !page_keys)
        {
            if let Some(c) = self.cards.get(&id) {
                c.page.ime_cancel();
            }
            self.ime_page = None;
        }
        let area = if self.omnibox.is_some() {
            let (vp, s) = (self.viewport(), self.ui());
            let w = (720.0 * s).min(vp.x - 32.0 * s);
            Some([((vp.x - w) / 2.0) as i32, (18.0 * s) as i32, w as i32, (48.0 * s) as i32])
        } else if page_keys {
            self.focused.and_then(|id| {
                let (x, y, w, h) = self.cards.get(&id)?.page.ime_caret()?;
                let r = self.page_rect_to_screen(id, Rect::new(x, y, w.max(1.0), h.max(1.0)))?;
                Some([r.x as i32, r.y as i32, r.w.ceil() as i32, r.h.ceil() as i32])
            })
        } else {
            None
        };
        let Some(w) = self.window().cloned() else { return };
        if allowed != self.ime_allowed {
            log::debug!("ime allowed={allowed}");
            w.set_ime_allowed(allowed);
            self.ime_allowed = allowed;
            self.ime_area = None;
        }
        if allowed
            && let Some(a) = area
            && self.ime_area != Some(a)
        {
            log::debug!("ime area {a:?}");
            w.set_ime_cursor_area(PhysicalPosition::new(a[0], a[1]), PhysicalSize::new(a[2].max(1) as u32, a[3].max(1) as u32));
            self.ime_area = Some(a);
        }
    }

    fn open_menu(&mut self, id: NodeId, x: f32, y: f32, ctx: &web::menu::MenuContext) {
        let Some(at) = self.page_rect_to_screen(id, Rect::new(x, y, 0.0, 0.0)) else { return };
        let anchor = Vec2::new(at.x, at.y);
        let ctx = web::menu::MenuContext {
            sticky: self.scene.node(id).is_some_and(Node::is_sticky),
            blocking: self.blocking_state(id),
            ..ctx.clone()
        };
        let items = web::menu::items(&ctx).into_iter().map(Entry::Page).collect();
        let m = self.layout_menu(id, items, anchor);
        log::info!("context menu on card {}: {:?}", id.0, menu_labels(&m.items));
        self.menu = Some(m);
        self.menu_pressed = false;
        if self.menu_by_key.take().is_some_and(|t| t.elapsed() < Duration::from_secs(2))
            && let Some(m) = self.menu.as_mut()
        {
            m.step(1);
        }
    }

    fn layout_menu(&mut self, id: NodeId, items: Vec<Entry>, anchor: Vec2) -> ContextMenu {
        let scale = self.ui();
        let label_w = match self.renderer.as_mut() {
            Some(r) => items
                .iter()
                .filter(|e| !e.is_separator())
                .map(|e| r.measure(e.label(), menu::FONT * scale, false))
                .fold(0.0, f32::max),
            None => 0.0,
        };
        ContextMenu::new(id, items, anchor, label_w, self.viewport(), scale)
    }

    /// The count badge on a stack's front, screen px. Sized by digit count, so
    /// hit testing needs no text measuring.
    fn stack_badge(&self, n: &scene::Node) -> Option<Rect> {
        let len = self.scene.stack_of(n.id)?.members.len();
        let size = 11.0 * self.ui();
        let frame = self.to_screen_rect(n.frame());
        let h = size * 1.6;
        let w = (size * (0.62 * len.to_string().len() as f32 + 1.0)).max(h);
        Some(Rect::new(frame.x - w * 0.4, frame.y - h * 0.4, w, h))
    }

    /// O, or a click on the count badge: list the stack's members under its
    /// badge, to bring one to the front (or unfold the trail).
    fn open_outline(&mut self, front: NodeId) {
        let Some(stack) = self.scene.stack_of(front) else { return };
        let front = stack.front;
        let members: Vec<(NodeId, String)> = stack
            .ids()
            .filter_map(|id| self.scene.all_nodes().find(|n| n.id == id))
            .map(|n| (n.id, if n.title.trim().is_empty() { n.url.clone() } else { n.title.clone() }))
            .collect();
        let refs: Vec<(NodeId, &str)> = members.iter().map(|(id, t)| (*id, t.as_str())).collect();
        let items = menu::outline(&refs, front);
        let Some(badge) = self.scene.node(front).and_then(|n| self.stack_badge(n)) else { return };
        let anchor = Vec2::new(badge.x, badge.bottom() + 4.0 * self.ui());
        let m = self.layout_menu(front, items, anchor);
        log::info!("outline of stack {}: {:?}", front.0, menu_labels(&m.items));
        self.menu = Some(m);
        self.menu_pressed = false;
    }

    fn activate_menu_item(&mut self, i: usize) {
        let Some(m) = self.menu.take() else { return };
        let id = m.page;
        let (action, label) = match m.items.get(i).cloned() {
            Some(Entry::Page(MenuItem::Action { action, enabled: true, label })) => (action, label),
            Some(Entry::Member { id: member, label, .. }) => {
                log::info!("outline: {label}");
                if self.scene.show(member) {
                    let stack = self.scene.stack_of(member).expect("shown in a stack");
                    log::info!("show: card {} ({} of {})", member.0, stack.front_index() + 1, stack.members.len());
                    self.after_fold();
                    self.dirty = true;
                }
                return;
            }
            Some(Entry::Unfold) => {
                log::info!("outline: unfold");
                self.unfold_stack(id);
                return;
            }
            _ => return,
        };
        log::info!("menu: {label}");
        // Edits act on the focused page; the right-click focused this card.
        if self.focused != Some(id) {
            self.focus(Some(id));
        }
        self.keys_in_page = true;
        let open_beside = |app: &mut App, url: &str| {
            if let Some(new) = app.open_card(Rect::default(), url, Some(id)) {
                app.ensure_visible(new);
            }
        };
        match action {
            MenuAction::OpenBeside(url) => open_beside(self, &url),
            MenuAction::Download(url) => {
                if let Some(c) = self.cards.get(&id) {
                    log::info!("menu: download {url}");
                    c.page.download(&url);
                }
            }
            MenuAction::Search(q) => {
                let url = omnibox::search_url(&q, self.settings.search);
                open_beside(self, &url)
            }
            MenuAction::AddToNote(text) => self.add_to_note(id, &text),
            MenuAction::SaveMarkdown => self.save_markdown(Some(id)),
            MenuAction::ToggleSticky => self.toggle_sticky(id),
            MenuAction::ToggleBlocking => self.toggle_blocking(Some(id)),
            MenuAction::CopyText(t) => self.clipboard.set_text(&t),
            MenuAction::Copy => self.page_copy(false, None),
            MenuAction::Cut => self.page_copy(true, None),
            MenuAction::Paste => self.page_paste(None),
            MenuAction::Undo | MenuAction::Redo | MenuAction::SelectAll => {
                if let Some(p) = self.focused_page() {
                    p.edit(match action {
                        MenuAction::Undo => EditCommand::Undo,
                        MenuAction::Redo => EditCommand::Redo,
                        _ => EditCommand::SelectAll,
                    });
                }
            }
            MenuAction::Back | MenuAction::Forward | MenuAction::Reload => {
                if let Some(p) = self.focused_page() {
                    match action {
                        MenuAction::Back => p.back(),
                        MenuAction::Forward => p.forward(),
                        _ => p.reload(),
                    }
                }
            }
        }
    }
}

/// `path` with the home folder written as `~`.
fn home_relative(path: &std::path::Path) -> String {
    match std::env::var_os("HOME").map(std::path::PathBuf::from) {
        Some(home) if home.as_os_str().len() > 1 => match path.strip_prefix(&home) {
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        },
        _ => path.display().to_string(),
    }
}

fn menu_labels(items: &[Entry]) -> Vec<&str> {
    items.iter().map(Entry::label).collect()
}

/// What an empty board says: a heading and how to start, keys as keycaps.
fn empty_hint(r: &mut compositor::Renderer, vp: Vec2, th: &Theme, ui: f32) -> Layer {
    enum Part {
        Words(&'static str),
        Keys(&'static [&'static str]),
    }
    use Part::*;
    let parts = [Words("Press"), Keys(&["N"]), Words("or"), Keys(&["Ctrl", "T"]), Words("to open a page  ·"), Keys(&["?"]), Words("for shortcuts")];
    let (size, ksize, space) = (15.0 * ui, 12.5 * ui, 7.0 * ui);
    let widths: Vec<Vec<f32>> = parts
        .iter()
        .map(|p| match p {
            Words(w) => vec![r.measure(w, size, false)],
            Keys(keys) => keys.iter().map(|k| r.measure(k, ksize, false)).collect(),
        })
        .collect();
    let part_w = |p: &Part, w: &[f32]| match p {
        Words(_) => w[0],
        Keys(_) => crate::chrome::keycaps_width(w, ksize),
    };
    let total: f32 = parts.iter().zip(&widths).map(|(p, w)| part_w(p, w)).sum::<f32>() + space * (parts.len() - 1) as f32;
    let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
    let mut l = Layer::default();
    let title = "An empty board";
    let tsize = 22.0 * ui;
    let tw = r.measure(title, tsize, true);
    let mid = vp.y / 2.0;
    l.texts.push(Text {
        text: title.into(),
        pos: [(vp.x - tw) / 2.0, mid - tsize * 2.4],
        size: tsize,
        color: rgb(crate::theme::mix(th.foreground(), th.dim_text(), 0.45), 1.0),
        clip: screen,
        bold: true,
    });
    let mut x = (vp.x - total) / 2.0;
    for (p, w) in parts.iter().zip(&widths) {
        match p {
            Words(text) => l.texts.push(Text {
                text: (*text).into(),
                pos: [x, mid - size * 1.3 / 2.0],
                size,
                color: rgb(th.dim_text(), 1.0),
                clip: screen,
                bold: false,
            }),
            Keys(keys) => {
                let right = x + crate::chrome::keycaps_width(w, ksize);
                crate::chrome::draw_keycaps(&mut l, keys, w, right, mid, ksize, th, ui, screen);
            }
        }
        x += part_w(p, w) + space;
    }
    l
}

fn menu_layer(m: &ContextMenu, scale: f32, th: &Theme) -> Layer {
    let mut l = Layer::default();
    let radius = th.radius(scale / th.text_scale);
    crate::chrome::panel(&mut l, m.rect, radius, th, scale, crate::chrome::Lift::Mid, th.panel());
    let size = menu::FONT * scale;
    for (i, (item, row)) in m.items.iter().zip(&m.rows).enumerate() {
        if item.is_separator() {
                let y = row.y + row.h / 2.0;
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(row.x + 4.0 * scale, y, row.w - 8.0 * scale, scale.max(1.0)), radius: 0.0 },
                    color: rgb(th.divider(), 1.0),
                    texture: None,
                });
                continue;
        }
        if m.hover == Some(i) {
            l.quads.push(Quad { shape: Shape::Rect { rect: *row, radius: th.small_radius(scale / th.text_scale) }, color: rgb(th.selection(), 1.0), texture: None });
        }
        let current = matches!(item, Entry::Member { current: true, .. });
        if current {
            // A short accent bar marks the card in front.
            let bar = Rect::new(row.x + 2.0 * scale, row.y + row.h * 0.25, (2.0 * scale).max(1.0), row.h * 0.5);
            l.quads.push(Quad { shape: Shape::Rect { rect: bar, radius: 0.0 }, color: rgb(th.accent(), 1.0), texture: None });
        }
        l.texts.push(Text {
            text: item.label().to_string(),
            pos: [row.x + (menu::TEXT_PAD - menu::PAD) * scale, row.y + (row.h - size * 1.3) / 2.0],
            size,
            color: rgb(if item.is_enabled() { th.foreground() } else { th.dim_text() }, 1.0),
            clip: *row,
            bold: current,
        });
    }
    l
}

impl App {
    fn run_script(&mut self) {
        if self.renderer.is_none() {
            return;
        }
        while let Some(cmd) = self.script.as_mut().and_then(Script::next) {
            log::info!("script: {cmd:?}");
            match cmd {
                Cmd::Wait(_) => {}
                Cmd::Move(x, y) => self.cursor_moved(Vec2::new(x, y)),
                Cmd::Click(x, y, shift) => {
                    let saved = self.mods;
                    if shift {
                        self.mods = ModifiersState::SHIFT;
                    }
                    self.cursor_moved(Vec2::new(x, y));
                    self.mouse_input(ElementState::Pressed, MouseButton::Left);
                    self.mouse_input(ElementState::Released, MouseButton::Left);
                    self.mods = saved;
                }
                Cmd::PageClick { id, x, y, right } => {
                    let Some(at) = self.page_rect_to_screen(NodeId(id), Rect::new(x, y, 0.0, 0.0)) else {
                        log::warn!("script: no card {id}");
                        continue;
                    };
                    let p = Vec2::new(at.x, at.y);
                    let button = if right { MouseButton::Right } else { MouseButton::Left };
                    self.cursor_moved(p);
                    self.mouse_input(ElementState::Pressed, button);
                    self.mouse_input(ElementState::Released, button);
                }
                Cmd::Wheel(x, y) => self.wheel(MouseScrollDelta::LineDelta(x, y), TouchPhase::Moved),
                Cmd::Touchpad { dx, dy, end } => {
                    let phase = if end { TouchPhase::Ended } else { TouchPhase::Moved };
                    self.wheel(MouseScrollDelta::PixelDelta(PhysicalPosition::new(dx as f64, dy as f64)), phase);
                }
                Cmd::CtrlWheel(y) => {
                    let saved = self.mods;
                    self.mods = ModifiersState::CONTROL;
                    self.wheel(MouseScrollDelta::LineDelta(0.0, y), TouchPhase::Moved);
                    self.mods = saved;
                }
                Cmd::Fit => self.fit_all(),
                Cmd::ImePreedit(t) => {
                    let n = t.len();
                    self.ime(Ime::Preedit(t, Some((n, n))));
                }
                Cmd::ImeCommit(t) => self.ime(Ime::Commit(t)),
                Cmd::MenuClick(label) => {
                    let row = self.menu.as_ref().and_then(|m| {
                        let i = menu_labels(&m.items).iter().position(|l| *l == label)?;
                        Some(m.rows[i])
                    });
                    match row {
                        Some(r) => {
                            self.cursor_moved(r.center());
                            self.mouse_input(ElementState::Pressed, MouseButton::Left);
                            self.mouse_input(ElementState::Released, MouseButton::Left);
                        }
                        None => log::warn!("script: no open menu item {label:?}"),
                    }
                }
                Cmd::BadgeClick(id) => match self.scene.node(NodeId(id)).and_then(|n| self.stack_badge(n)) {
                    Some(b) => {
                        self.cursor_moved(b.center());
                        self.mouse_input(ElementState::Pressed, MouseButton::Left);
                        self.mouse_input(ElementState::Released, MouseButton::Left);
                    }
                    None => log::warn!("script: card {id} is no stack's front"),
                },
                Cmd::Key(combo) => match keymap::parse_combo(&combo) {
                    Some((mods, name)) => self.script_key(name, mods),
                    None => log::warn!("script: bad key combo {combo:?}"),
                },
                Cmd::Type(text) => {
                    for c in text.chars() {
                        let shift = c.is_ascii_uppercase();
                        let mods = if shift { ModifiersState::SHIFT } else { ModifiersState::empty() };
                        self.script_key(&c.to_string(), mods);
                    }
                }
                Cmd::Drag { from: (x0, y0), to: (x1, y1), shift } => {
                    let saved = self.mods;
                    if shift {
                        self.mods = ModifiersState::SHIFT;
                    }
                    self.cursor_moved(Vec2::new(x0, y0));
                    self.mouse_input(ElementState::Pressed, MouseButton::Left);
                    for i in 1..=8 {
                        let t = i as f32 / 8.0;
                        self.cursor_moved(Vec2::new(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t));
                    }
                    self.mouse_input(ElementState::Released, MouseButton::Left);
                    self.mods = saved;
                }
                Cmd::Button { at: (x, y), pressed } => {
                    self.cursor_moved(Vec2::new(x, y));
                    let state = if pressed { ElementState::Pressed } else { ElementState::Released };
                    self.mouse_input(state, MouseButton::Left);
                }
                Cmd::Undo => self.undo(false),
                Cmd::Redo => self.undo(true),
                Cmd::Cards => {
                    for n in self.scene.all_nodes() {
                        let (r, f) = (n.rect, self.to_screen_rect(n.frame()));
                        log::info!(
                            "script card {}: world ({:.0},{:.0} {:.0}x{:.0}) screen ({:.0},{:.0} {:.0}x{:.0}) live={} sel={} hidden={} blocked={} title={:?} {}",
                            n.id.0, r.x, r.y, r.w, r.h, f.x, f.y, f.w, f.h, self.cards.contains_key(&n.id),
                            self.selection.contains(n.id), self.scene.is_hidden(n.id),
                            self.cards.get(&n.id).map_or(0, |c| c.page.blocked()), n.title, n.url
                        );
                        if let Some(text) = &n.note {
                            log::info!("script note {}: {:?}", n.id.0, text);
                        }
                        if let Some(c) = n.sticky.as_ref().filter(|_| n.is_sticky()) {
                            log::info!("script sticky {}: colour={c}", n.id.0);
                        }
                        if !n.trail.is_empty() {
                            log::info!("script trail {}: name={:?} colour={:?}", n.id.0, n.trail.name, n.trail.colour);
                        }
                    }
                    for st in self.scene.stacks() {
                        log::info!(
                            "script stack: {:?} front={} anchor=({:.0},{:.0})",
                            st.ids().map(|i| i.0).collect::<Vec<_>>(), st.front.0, st.anchor.x, st.anchor.y
                        );
                    }
                    for e in self.scene.edges() {
                        log::info!("script edge {} -> {}", e.from.0, e.to.0);
                    }
                    for (head, mark) in self.scene.trail_labels() {
                        log::info!("script label on {}: name={:?} colour={:?}", head.0, mark.name, mark.colour);
                    }
                    let l = self.read_layout();
                    log::info!(
                        "script read: {:?} css={:?} page_scale={} focused={:?} keys_in_page={} camera=({:.0},{:.0} z{:.3})",
                        self.read.map(|r| r.0), l.css, self.page_scale, self.focused.map(|f| f.0), self.keys_in_page,
                        self.camera.center.x, self.camera.center.y, self.camera.zoom
                    );
                }
                Cmd::Spaces => {
                    for e in &self.spaces.spaces {
                        log::info!(
                            "script space {}: {:?} tint={:?} board={:?} private={}{}",
                            e.id.0, e.name, e.tint, self.theme.board(e.tint.as_deref()), e.private,
                            if e.id == self.spaces.active { " active" } else { "" }
                        );
                    }
                }
                Cmd::Sidebar => {
                    let panel = self.sidebar_panel();
                    let rows = self.sidebar.rows(&self.scene);
                    log::info!(
                        "script sidebar: open={} keys={} sel={:?} scroll={} shown={} width={:.0} rows={}",
                        self.sidebar_open, self.sidebar_has_keys(), self.sidebar.index(&rows), self.sidebar.scroll,
                        panel.shown, panel.rect.w, rows.len()
                    );
                    for (i, row) in rows.iter().enumerate() {
                        let at = panel.row_rect(i, self.sidebar.scroll).map(|r| r.center());
                        let at = at.map_or_else(|| "off".to_string(), |c| format!("({:.0},{:.0})", c.x, c.y));
                        let what = match row {
                            scene::sidebar::Row::Header { label, count, collapsed, mark, .. } => format!(
                                "header {label:?} count={count}{}{} place={:?}",
                                if *collapsed { " collapsed" } else { "" },
                                if mark.pinned { " pinned" } else { "" },
                                mark.place
                            ),
                            scene::sidebar::Row::Card { id, depth, folded, .. } => {
                                let title = self.scene.node(*id).map(|n| n.title.clone()).unwrap_or_default();
                                format!("card {} depth={depth}{} {title:?}", id.0, if *folded { " folded" } else { "" })
                            }
                        };
                        log::info!("script sidebar row {i} at {at}: {what}");
                    }
                }
                Cmd::History(query) => match &self.page_index {
                    None => log::info!("script history: off"),
                    Some(index) => {
                        let len = index.len().unwrap_or(0);
                        log::info!("script history: {len} pages");
                        for v in index.recent(5).unwrap_or_default() {
                            log::info!("script history recent: {} {:?}", v.url, v.title);
                        }
                        for v in index.search(&query, 10).unwrap_or_default() {
                            log::info!("script history found {query:?}: {} {:?}", v.url, v.title);
                        }
                    }
                },
                Cmd::Icons => {
                    let mut sites: Vec<_> = self.icons.iter().map(|(s, i)| format!("{s}={}x{}", i.icon.width, i.icon.height)).collect();
                    sites.sort();
                    log::info!("script icons: sites={sites:?} named={:?}", self.named_card().map(|id| id.0));
                    for n in self.scene.nodes() {
                        let site = scene::icon::site(&n.url);
                        let drawn = self.icon_key(&n.url).is_some();
                        log::info!("script icon card {}: site {site:?} drawn {drawn}", n.id.0);
                    }
                }
                Cmd::Zooms => {
                    log::info!("script zooms: sites={:?}", self.zooms.borrow().sites);
                    let mut ids: Vec<_> = self.cards.keys().copied().collect();
                    ids.sort();
                    for id in ids {
                        let url = self.scene.node(id).map(|n| n.url.clone()).unwrap_or_default();
                        let page = self.cards[&id].page.zoom_factor();
                        log::info!(
                            "script zoom card {}: site {:?} want {} chromium/scale {page:.3} {url:?}",
                            id.0, pagezoom::site_of(&url), pagezoom::label(self.zooms.borrow().get(&url))
                        );
                    }
                }
                Cmd::Find => match self.omnibox.as_ref().filter(|ob| matches!(ob.target, Target::Find(_))) {
                    None => log::info!("script find: closed"),
                    Some(ob) => log::info!("script find: {:?} on {:?} found={:?}", ob.text, ob.target, self.found),
                },
                Cmd::Palette => match self.omnibox.as_ref().filter(|ob| ob.target.is_list()) {
                    None => log::info!("script palette: closed"),
                    Some(ob) => {
                        let rows = self.list_rows(ob);
                        let panel = palette::Panel::new(self.viewport(), self.ui(), rows.len());
                        let scroll = ob.scroll.min(rows.len().saturating_sub(panel.shown));
                        log::info!(
                            "script palette: query={:?} sel={} scroll={} shown={} rows={}",
                            ob.text, ob.sel, scroll, panel.shown, rows.len()
                        );
                        for (i, row) in rows.iter().enumerate() {
                            let at = panel.row_rect(i, scroll).map(|r| r.center());
                            let at = at.map_or_else(|| "off".to_string(), |c| format!("({:.0},{:.0})", c.x, c.y));
                            log::info!(
                                "script palette row {i} at {at}: {:?} {:?} detail={:?} marks={:?}",
                                row.item, row.label, row.detail, row.positions
                            );
                        }
                    }
                },
                Cmd::Open(url) => {
                    self.new_card_at_center(&url);
                }
                Cmd::Log(msg) => log::info!("script log: {msg} (cards={}, edges={})", self.scene.nodes().count(), self.scene.edges().len()),
                Cmd::Panic => panic!("script: panic on purpose"),
                Cmd::Quit => self.quit = true,
            }
        }
    }
}

impl App {
    /// Press and release a named key with `mods` held, through the normal key path.
    fn script_key(&mut self, name: &str, mods: ModifiersState) {
        let shift = mods.shift_key();
        let (Some(down), Some(up)) = (KeyIn::from_name(name, true, shift), KeyIn::from_name(name, false, shift)) else {
            // Not on the US layout: deliver it as text only.
            let text = KeyIn { pressed: true, code: None, named: None, text: Some(name.to_string()), native: 0 };
            let saved = std::mem::replace(&mut self.mods, mods);
            self.key(text.clone());
            self.key(KeyIn { pressed: false, text: None, ..text });
            self.mods = saved;
            return;
        };
        let saved = std::mem::replace(&mut self.mods, mods);
        self.key(down);
        self.key(up);
        self.mods = saved;
    }
}

/// Window cursor for a page cursor; `None` hides it.
fn cursor_icon(c: web::Cursor) -> Option<CursorIcon> {
    use web::Cursor as C;
    Some(match c {
        C::Default => CursorIcon::Default,
        C::Pointer => CursorIcon::Pointer,
        C::Text => CursorIcon::Text,
        C::VerticalText => CursorIcon::VerticalText,
        C::Wait => CursorIcon::Wait,
        C::Progress => CursorIcon::Progress,
        C::Help => CursorIcon::Help,
        C::Crosshair => CursorIcon::Crosshair,
        C::Cell => CursorIcon::Cell,
        C::Move => CursorIcon::Move,
        C::Grab => CursorIcon::Grab,
        C::Grabbing => CursorIcon::Grabbing,
        C::NotAllowed => CursorIcon::NotAllowed,
        C::NoDrop => CursorIcon::NoDrop,
        C::Copy => CursorIcon::Copy,
        C::Alias => CursorIcon::Alias,
        C::ContextMenu => CursorIcon::ContextMenu,
        C::ZoomIn => CursorIcon::ZoomIn,
        C::ZoomOut => CursorIcon::ZoomOut,
        C::AllScroll => CursorIcon::AllScroll,
        C::ResizeE => CursorIcon::EResize,
        C::ResizeN => CursorIcon::NResize,
        C::ResizeNE => CursorIcon::NeResize,
        C::ResizeNW => CursorIcon::NwResize,
        C::ResizeS => CursorIcon::SResize,
        C::ResizeSE => CursorIcon::SeResize,
        C::ResizeSW => CursorIcon::SwResize,
        C::ResizeW => CursorIcon::WResize,
        C::ResizeNS => CursorIcon::NsResize,
        C::ResizeEW => CursorIcon::EwResize,
        C::ResizeNESW => CursorIcon::NeswResize,
        C::ResizeNWSE => CursorIcon::NwseResize,
        C::ResizeCol => CursorIcon::ColResize,
        C::ResizeRow => CursorIcon::RowResize,
        C::Hidden => return None,
    })
}

/// `TRAILBOARD_MAX_LIVE`, if set: it overrides the *Pages kept awake* setting for this run.
fn max_live_from_env() -> Option<usize> {
    match std::env::var("TRAILBOARD_MAX_LIVE").ok().map(|v| v.trim().parse::<usize>()) {
        None => None,
        Some(Ok(n)) if n >= 1 => Some(n),
        Some(_) => {
            log::warn!("TRAILBOARD_MAX_LIVE must be a whole number ≥ 1; ignored");
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Board,
    Header(NodeId),
    Close(NodeId),
    Grip(NodeId),
    Content(NodeId),
    /// Read Mode's title bar.
    ReadBar(NodeId),
    /// A stack's count badge.
    Badge(NodeId),
}

impl Hit {
    fn id(&self) -> Option<NodeId> {
        match *self {
            Hit::Board => None,
            Hit::Header(id) | Hit::Close(id) | Hit::Grip(id) | Hit::Content(id) | Hit::ReadBar(id) | Hit::Badge(id) => Some(id),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Trailboard")
            .with_name("trailboard", "trailboard")
            .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let display = Box::new(event_loop.owned_display_handle());
        match pollster::block_on(Renderer::new(window, display)) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                log::error!("GPU init failed: {e}");
                event_loop.exit();
                return;
            }
        }
        self.page_scale = self.scale();
        self.start_desktop();
        let first_run = spaces::is_first_run(&self.data_dir);
        let restored = self.restore();
        let mut urls = std::mem::take(&mut self.initial_urls);
        if urls.is_empty() && !restored {
            urls.push(START_URL.to_string());
        }
        let mut last = None;
        for url in &urls {
            last = self.new_card_at_center(url).or(last);
        }
        let guide = (first_run && self.persist).then(|| self.add_guide(last));
        if !restored {
            let shown = [last, guide].into_iter().flatten().filter_map(|id| self.scene.node(id)).map(|n| n.frame()).reduce(|a, b| a.union(&b));
            if let Some(r) = shown {
                let mut goal = Camera::fit(r, self.viewport(), self.scale(), 8.0 * self.scale());
                goal.zoom = goal.zoom.min(2.0);
                self.camera = goal;
                // `ensure_visible` on the new page left a goal of its own.
                self.camera_goal = None;
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => self.camera_moved(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => self.cursor_moved(Vec2::new(position.x as f32, position.y as f32)),
            WindowEvent::CursorLeft { .. } => {
                self.pointer_in = false;
                if let Some(id) = self.hit(self.cursor).id()
                    && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, self.cursor)) {
                        card.page.mouse_move(x, y, 0, true);
                    }
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_input(state, button),
            WindowEvent::MouseWheel { delta, phase, .. } => self.wheel(delta, phase),
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => self.key(KeyIn::from_winit(&event)),
            WindowEvent::Ime(ime) => self.ime(ime),
            WindowEvent::Focused(false) => self.mods = ModifiersState::empty(),
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
}
