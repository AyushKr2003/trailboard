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
//!   of the card, Delete or d closes it (u reopens), Q quits. Board keys (f, N, T, S, O, R, C, [, ])
//!   act on the current card, else the one in the middle of the view.
//! - B opens the trails sidebar (`scene::sidebar`, `sidebar`) with the keys:
//!   j/k move its highlight and go to that card, h/l collapse / expand a trail,
//!   Enter gives a card's page the keys (or toggles a trail), r names the
//!   row's trail, Esc or B closes it. Clicks on a row do the same (a
//!   double-click on a trail's header names it); a click elsewhere takes the keys back, the
//!   panel stays open. The pointer resting on the window's left edge peeks it
//!   without the keys (`sidebar::Peek`); it goes once the pointer leaves, and
//!   B keeps it. The chips at its foot switch spaces.
//! - f labels the links and controls on the current card's page (`scene::hint`,
//!   `web::links`); typing a label opens a link beside the card, which becomes
//!   current, or clicks a control (a field also gets the keys). Esc, another
//!   key, a click or a scroll ends the labels. Shift+F fits everything.
//! - Mouse-only actions have keys too: v selects / deselects the current card
//!   (h/j/k/l keep the selection), z fills the view with it, Ctrl+d/u scroll its
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
//! - Ctrl+, settings (`scene::settings`, saved by `storage::settings`): a
//!   settings page card (`settings_page`, `scene::note::SETTINGS_URL`) with
//!   sections, switches and drop-downs, found again if it is on the board. Each
//!   change applies at once
//!   (browsing ones through `web::prefs`, `web::permission` and the per-site
//!   JavaScript and images of `scene::sitesettings`).
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
//! - Tab (unless keys are going to a page) toggles Solo: the focused card, or
//!   the one in the middle of the view, fills the window and its page lays out at
//!   the window's size. Esc or Tab returns to the board.
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
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use compositor::{Background, Layer, Quad, Renderer, Shape, Text};
use scene::arrange::Arrange;
use scene::hint;
use scene::keys::{self, Action as KeyAction, Outcome};
use scene::nav::{self, Dir};
use scene::pagezoom::{self, SiteZooms};
use scene::scrolltarget;
use scene::settings::{self as prefs, Settings};
use scene::{liveness, motion, placement, read, selection, tidy, Camera, Change, Edge, History, Node, NodeId, Rect, Scene, Selection, TrailStep, Vec2};
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

const MIN_CARD: f32 = 240.0;
/// How long after a page loads (or retitles itself) its text is read for history.
const TEXT_SETTLE: Duration = Duration::from_millis(1200);
/// How long the outline around a newly chosen scroll section shows.
const SCROLL_FLASH: Duration = Duration::from_millis(900);
/// One keyboard move or resize step (M, then h/j/k/l), in logical px on screen.
const ARRANGE_STEP: f32 = 40.0;
/// The fine step with Ctrl.
const ARRANGE_FINE: f32 = 8.0;
const ARRANGE_HELP: &str = "Moving · h j k l move · Shift resize · Ctrl fine · Enter done · Esc cancel";
const RESIZE_GRIP: f32 = 18.0;
/// A card's name sits on a label above it, the same size on screen at any
/// zoom: its site's icon and title, and under the pointer (or on the current
/// card) its buttons. The label is the card's handle: drag it to move the
/// card. Sizes in px, times the UI scale.
const BAR_H: f32 = 26.0;
/// Between the label and the card.
const BAR_GAP: f32 = 6.0;
/// Width of a label button (close, reload, forward, back).
const BAR_BTN: f32 = 24.0;
/// A card narrower than this on screen has no room for back, forward and
/// reload on its label; narrower than `BAR_CLOSE_MIN`, none for close either.
const BAR_NAV_MIN: f32 = 280.0;
const BAR_CLOSE_MIN: f32 = 96.0;
/// Below this zoom every card shows its label (landmarks on a far-out
/// board); above it only the current card, the one under the pointer and
/// selected ones do.
const LABELS_ALL_BELOW: f32 = 0.45;
/// Height of the Solo title bar, in overlay px (scaled by `ui()`).
/// None: the page fills the whole window (Tab or Esc goes back).
const READ_BAR: f32 = 0.0;

/// A card moving between the board and Solo (`scene::read::morph`).
#[derive(Debug, Clone, Copy)]
struct ReadMorph {
    id: NodeId,
    /// Growing into Solo (else shrinking back to its place on the board).
    entering: bool,
    start: Instant,
}

impl ReadMorph {
    /// How far into Solo the card is: 0 on the board, 1 filling the window.
    fn progress(&self) -> f32 {
        let t = crate::chrome::ease_out(self.start.elapsed().as_secs_f32() * 1000.0 / read::MORPH_MS as f32);
        if self.entering { t } else { 1.0 - t }
    }

    fn done(&self) -> bool {
        self.start.elapsed() >= Duration::from_millis(read::MORPH_MS)
    }
}
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

/// Where an entered card's keys go (M12). Board mode is no card entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageKeys {
    /// Our page keys first (`scene::keys::page`); keys that type nothing go on.
    Page,
    /// Every key to the page; Esc back to Page.
    Insert,
    /// Every key to the page, Esc included; Shift+Esc back to Page.
    Passthrough,
}

impl PageKeys {
    fn label(self) -> &'static str {
        match self {
            PageKeys::Page => "Page",
            PageKeys::Insert => "Insert",
            PageKeys::Passthrough => "Passthrough · Shift+Esc to leave",
        }
    }
}

struct LinkHints {
    id: NodeId,
    /// Follow a link in the same card (Page mode's f) instead of beside it.
    here: bool,
    /// `;` hints: what choosing a label does instead of following it.
    kind: Option<hint::Kind>,
    /// Labels already chosen while `kind` keeps the labels up (`;m`).
    chosen: Vec<usize>,
    links: Vec<(Rect, web::links::Action)>,
    /// `s`: labels on the page's scroll sections instead of its links
    /// (section number, where it is in the view's CSS px), in label order.
    sections: Vec<(u32, Rect)>,
    hints: Option<hint::Hints>,
}

/// Why a card's scroll sections were asked for (M12 step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionAsk {
    /// `s`: label them.
    Label,
    /// `]s` / `[s`: step to the next / previous one.
    Step(bool),
    /// `S`: step out to the one around the target.
    Out,
    /// Only to name them in the mode pill.
    Name,
}

/// A brief outline around the section a key just chose or scrolled.
struct ScrollFlash {
    id: NodeId,
    section: u32,
    /// In the view's CSS px.
    rect: Rect,
    at: Instant,
}

struct Card {
    page: Page,
    loading: bool,
    /// The page has history to go back / forward to (its title bar's arrows).
    can_back: bool,
    can_forward: bool,
    hidden: bool,
    /// Cursor the page asked for, shown while the pointer is over its content.
    cursor: web::Cursor,
}

/// Appear animations of the overlays.
#[derive(Default)]
struct Anims {
    tip: crate::chrome::Appear,
    omnibox: crate::chrome::Appear,
    menu: crate::chrome::Appear,
    toast: crate::chrome::Appear,
    sheet: crate::chrome::Appear,
    sidebar: crate::chrome::Appear,
    welcome: crate::chrome::Appear,
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
    /// Camera we're gliding toward (fit / Solo).
    camera_goal: Option<Camera>,
    /// Wheel and touchpad scrolling inside a page, and flings (`scene::scroll`):
    /// to card, at view px.
    page_scroll: scene::scroll::Scroller<(NodeId, i32, i32)>,
    /// Wheel and touchpad panning of the board: glides and flings (`scene::scroll::Pan`).
    board_pan: scene::scroll::Pan,
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
    /// The card current before `focused`, for Ctrl+^ (this space's).
    alt_card: Option<NodeId>,
    /// Closed cards for Ctrl+Shift+T, newest last.
    closed: Vec<(Node, Vec<Edge>)>,
    history: History,
    selection: Selection,
    /// Cards moved by the current title-bar drag, with their start rects.
    group: Vec<(NodeId, Rect)>,
    /// The last click landed inside the focused page, so its keys (incl. Ctrl+Z) go there.
    keys_in_page: bool,
    /// The board's key bindings (`scene::keys`, M12).
    board_keys: keys::Bindings<KeyAction>,
    /// Page mode's key bindings.
    page_keys_table: keys::Bindings<KeyAction>,
    /// A count or sequence typed toward a binding (`5j`).
    pending: keys::Pending,
    /// Where an entered card's keys go.
    page_keys: PageKeys,
    /// Keys pressed in Page mode that went on to the page, so their release does too.
    forwarded: HashSet<i32>,
    omnibox: Option<Omnibox>,
    /// Lines run on the `:` command line, newest first (`storage::commands`).
    recent_commands: scene::command::Recent,
    menu: Option<ContextMenu>,
    /// Where the title-bar shields were drawn last frame (screen px, with a
    /// little room around them), for clicks.
    shields: Vec<(NodeId, Rect)>,
    /// Where the trail tags were drawn last frame (screen px), in draw
    /// order, for clicks: a click on one names its trail.
    tags: Vec<(NodeId, Rect)>,
    /// The shortcut sheet (`?`) is showing.
    hints: bool,
    /// Its filter and scroll.
    sheet: crate::hints::Sheet,
    /// Link hints (`f`) on a card's page: asked for, or showing labels.
    link_hints: Option<LinkHints>,
    /// Each card's chosen scroll section (`web::scroll`); a card not here
    /// scrolls the automatic pick. Forgotten when its page navigates.
    scroll_targets: HashMap<NodeId, u32>,
    /// The sections a card reported last, for naming them in the pill.
    sections: HashMap<NodeId, Vec<scrolltarget::Section>>,
    /// A section list asked of a card, and what for.
    section_ask: Option<(NodeId, SectionAsk)>,
    /// What the last scroll key moved, for the mode pill (`section 2.1 · 40%`):
    /// the card, the section, and what to say after its name.
    scroll_note: Option<(NodeId, u32, String)>,
    scroll_flash: Option<ScrollFlash>,
    /// Moving or resizing the current card from the keyboard (M).
    arrange: Option<Arrange>,
    /// Solo: this card fills the window (see `scene::read`).
    read: Option<NodeId>,
    /// The card growing into Solo or shrinking back to the board.
    read_morph: Option<ReadMorph>,
    /// The CSS size each live card's last uploaded frame was laid out at, so
    /// a frame from before a resize is drawn at its own size, not stretched.
    page_css: HashMap<NodeId, (f32, f32)>,
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
    /// The last click that toggled a trail's header, for a double-click
    /// (which names the trail instead).
    sidebar_toggled: Option<(Instant, NodeId)>,
    /// The sidebar was opened by the pointer on the window's edge, and goes
    /// when the pointer leaves it (`sidebar::Peek`).
    sidebar_peek: bool,
    peek: crate::sidebar::Peek,
    /// The space chip under the pointer in the sidebar's foot.
    sidebar_spot: Option<crate::sidebar::Spot>,
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
    /// Cards' labels fading in and out (quiet labels).
    label_fades: crate::chrome::Fades,
    /// The board's photo (*Board background: Photo*), read off the main thread.
    photo: crate::backdrop::Photo,
    /// The desktop's wallpaper file, looked up now and then (`backdrop::desktop_wallpaper`).
    wallpaper: Option<PathBuf>,
    wallpaper_checked: Option<Instant>,
    /// Pictures the photo list offers, found when it opens.
    photo_files: Vec<PathBuf>,
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
    /// The start page's pins and parts, shared by every space (`start.json`).
    start_page: scene::startpage::StartPage,
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
    /// JavaScript and images per site (page menu), saved in `sites.json`.
    sites: scene::sitesettings::SiteSettings,
    /// The controls over the board, as laid out last frame (for the pointer).
    hud: crate::hud::Hud,
    /// The control under the pointer, and since when (for its tooltip).
    hud_hover: Option<(crate::hud::Control, Instant)>,
    /// The left button is held on a control.
    hud_down: bool,
    /// The control whose menu is open.
    hud_open: Option<crate::hud::Control>,
    /// The size of a new card, shaped like the screen (`placement::card_size`).
    card_size: Vec2,
    /// What the pointer is over on the board (a label's buttons light up).
    hover: Hit,
    /// Where each card's label was drawn last frame (screen px), in draw
    /// order, and the buttons on them, for the pointer.
    bars: Vec<(NodeId, Rect)>,
    bar_buttons: Vec<(Hit, Rect)>,
    /// The buttons of the guidance drawn last frame (an empty board's
    /// welcome, a view with no card in it), and what each does.
    nudge: Vec<(Rect, crate::hud::Go)>,
    /// The desktop asks for less motion (`desktop::reduced_motion`): cards,
    /// spaces, Solo and the sidebar change without animating.
    still: bool,
    /// Cards gliding to new places or growing in (`scene::motion`).
    glides: motion::Glides,
    /// Closed cards shrinking away: the card as it was, its page's texture
    /// (removed once it is gone), and since when.
    ghosts: Vec<(Node, u64, Instant)>,
    /// A space switch under way: since when, the side the board slides in
    /// from (±1), and the backdrop colours of the space it left.
    space_in: Option<(Instant, f32, [f32; 4], [f32; 4])>,
    /// The sidebar as last drawn and its right edge, to slide it out once
    /// it is hidden; and since when it has been sliding.
    sidebar_last: Option<(Layer, f32)>,
    sidebar_out: Option<Instant>,
    /// A download waiting for the user to say where it goes (`Target::SaveAs`).
    save_as: Option<SaveAs>,
    /// Sites asking for a permission, waiting for Enter (allow) or Esc; the
    /// first is the question showing (`Confirm::Permission`).
    asks: Vec<Ask>,
}

/// A download waiting for a path (*Save downloads to: Ask each time*).
struct SaveAs {
    id: u32,
    page: u64,
    url: String,
    blank: bool,
}

/// A site's permission request waiting for the user.
struct Ask {
    id: u64,
    origin: String,
    kinds: Vec<web::permission::Kind>,
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
    /// Close every card but this one (co).
    CloseOthers(NodeId),
    ClearHistory,
    ClearCookies,
    /// The first of `App::asks`: Enter allows, anything else blocks.
    Permission,
}

/// The desktop's locale (`en_IN.UTF-8`): `LC_ALL`, `LC_MESSAGES` or `LANG`.
pub fn system_locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|l| !l.is_empty())).unwrap_or_default()
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
        let mut zooms = storage::zoom::load_zooms(&data_dir);
        zooms.default = settings.default_zoom_factor();
        let zooms = Rc::new(RefCell::new(zooms));
        let sites = storage::sitesettings::load_sites(&data_dir);
        let start_page = storage::start::load_start(&data_dir);
        let recent_commands = storage::commands::load_recent(&data_dir);
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
        let app = Self {
            renderer: None,
            scene: Scene::new(),
            camera: Camera::default(),
            camera_goal: None,
            page_scroll: Default::default(),
            board_pan: Default::default(),
            cards: HashMap::new(),
            max_live: live_env.unwrap_or(settings.live_pages),
            blocking: crate::blocking::Blocking::start(data_dir.clone(), settings.block_ads),
            settings,
            live_from_env: live_env.is_some(),
            used: HashMap::new(),
            use_clock: 0,
            focused: None,
            alt_card: None,
            closed: Vec::new(),
            history: History::new(),
            selection: Selection::new(),
            group: Vec::new(),
            keys_in_page: false,
            board_keys: keys::board(),
            page_keys_table: keys::page(),
            pending: keys::Pending::default(),
            page_keys: PageKeys::Page,
            forwarded: HashSet::new(),
            omnibox: None,
            recent_commands,
            menu: None,
            shields: Vec::new(),
            tags: Vec::new(),
            hints: false,
            sheet: Default::default(),
            link_hints: None,
            scroll_targets: HashMap::new(),
            sections: HashMap::new(),
            section_ask: None,
            scroll_note: None,
            scroll_flash: None,
            arrange: None,
            read: None,
            read_morph: None,
            page_css: HashMap::new(),
            sidebar: Default::default(),
            sidebar_open: false,
            sidebar_keys: false,
            sidebar_at: None,
            sidebar_hover: None,
            sidebar_press: None,
            sidebar_toggled: None,
            sidebar_peek: false,
            peek: Default::default(),
            sidebar_spot: None,
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
            label_fades: crate::chrome::Fades::default(),
            photo: crate::backdrop::Photo::default(),
            wallpaper: None,
            wallpaper_checked: None,
            photo_files: Vec::new(),
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
            start_page,
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
            sites,
            hud: crate::hud::Hud::default(),
            hud_hover: None,
            hud_down: false,
            hud_open: None,
            card_size: Vec2::new(scene::CARD_W, scene::CARD_H),
            hover: Hit::Board,
            bars: Vec::new(),
            bar_buttons: Vec::new(),
            nudge: Vec::new(),
            still: {
                if let Some(k) = std::env::var("TRAILBOARD_SLOW_MOTION").ok().and_then(|v| v.parse::<f32>().ok()) {
                    motion::set_slow(k);
                }
                crate::desktop::reduced_motion()
            },
            glides: motion::Glides::default(),
            ghosts: Vec::new(),
            space_in: None,
            sidebar_last: None,
            sidebar_out: None,
            save_as: None,
            asks: Vec::new(),
        };
        app.apply_browsing();
        app.apply_site_content();
        app.apply_permissions();
        app
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

    /// How the sandbox confines pages, for the settings page. When the system
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
        let mut keys: Vec<u64> = self.scene.all_nodes().map(|n| self.page_key(n.id)).collect();
        keys.extend(self.ghosts.drain(..).map(|g| g.1));
        self.glides.clear();
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
        self.alt_card = None;
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
        // The new board slides in from the side the space is on, while the
        // tint cross-fades from the old one.
        let dir = match (self.spaces.position(self.spaces.active), self.spaces.position(to)) {
            (Some(a), Some(b)) if b < a => -1.0,
            _ => 1.0,
        };
        let (near, far) = self.backdrop_colours();
        self.unload_space();
        self.spaces.active = to;
        self.save_index();
        self.load_space();
        if !self.still {
            self.space_in = Some((Instant::now(), dir, rgb(near, 1.0), rgb(far, 1.0)));
        }
    }

    /// The board's two colours behind the cards: the space's mood, or its
    /// flat colour twice for *Plain*.
    fn backdrop_colours(&self) -> (crate::theme::Rgb, crate::theme::Rgb) {
        let tint = self.spaces.active_entry().tint.clone();
        match self.settings.backdrop {
            prefs::Backdrop::Plain => (self.theme.board(tint.as_deref()), self.theme.board(tint.as_deref())),
            _ => self.theme.mood(tint.as_deref()),
        }
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
        let t = d.read(&self.theme);
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

    fn set_theme(&mut self, mut t: Theme) {
        t.rounding = self.settings.corner_radius(t.hyprland);
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
        self.check_wallpaper(false);
        let before = self.desktop.as_ref().and_then(Desktop::full_window);
        if let Some(t) = self.desktop.as_mut().and_then(|d| d.poll(&self.theme)) {
            self.set_theme(t);
        }
        if self.desktop.as_ref().and_then(Desktop::full_window) != before {
            self.update_card_size();
        }
    }

    /// New cards take the shape of a full window on an empty workspace
    /// (from Hyprland), else of the monitor the window is on, else 16:9.
    fn update_card_size(&mut self) {
        let full = self.desktop.as_ref().and_then(Desktop::full_window).map(|(w, h)| Vec2::new(w, h));
        // Wayland names the window's monitor only once it is shown: until
        // then, the first monitor.
        let screen = self.window().and_then(|w| w.current_monitor().or_else(|| w.available_monitors().next())).map(|m| {
            let s = m.size().to_logical::<f32>(m.scale_factor());
            Vec2::new(s.width, s.height)
        });
        let size = placement::card_size(full, screen);
        if size != self.card_size {
            log::info!("new cards: {}×{} (full window {full:?}, screen {screen:?})", size.x, size.y);
            self.card_size = size;
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

    /// Card `n`'s page size on the board (CSS px). Pages lay out at Read
    /// Mode's width and the card shows them scaled (`read::card_css`), so a
    /// card always looks like the page in a full window; notes fill their card.
    fn card_css(&self, n: &Node) -> Vec2 {
        let card = Vec2::new(n.rect.w, n.rect.h);
        if n.note.is_some() {
            return read::card_css(card, 0);
        }
        read::card_css(card, self.read_layout().css.0)
    }

    /// Where card `id`'s page is drawn (screen px) and its size in CSS px.
    fn page_view(&self, id: NodeId) -> Option<(Rect, Vec2)> {
        if self.read == Some(id) {
            let l = self.read_layout();
            return Some((l.content, Vec2::new(l.css.0 as f32, l.css.1 as f32)));
        }
        let n = self.scene.node(id)?;
        Some((self.to_screen_rect(n.rect), self.card_css(n)))
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
            Some(src) => self.scene.spawn_beside_sized(src, url, self.card_size)?,
            None => self.scene.add_page(rect, url),
        };
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        // It grows in: out of the card that opened it, else in its own place.
        if !self.still
            && let Some(to) = self.scene.node(id).map(|n| n.rect)
        {
            let start = match from.and_then(|s| self.scene.node(s)) {
                Some(src) => motion::open_from(src.rect, to),
                None => motion::grow_from(to),
            };
            self.glides.start(id, start, true, Instant::now());
        }
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
        let css = self.card_css(n);
        let (url, kind) = match &n.note {
            Some(text) => (
                crate::note::page_url(text, &self.theme, n.sticky.as_deref()),
                web::PageKind::Note,
            ),
            None if n.url == scene::note::WELCOME_URL => {
                (crate::welcome::page_url(&self.theme), web::PageKind::App)
            }
            None if n.url == scene::note::ENGINES_URL => (
                crate::engines::page_url(&self.theme, self.settings.search),
                web::PageKind::App,
            ),
            None if n.url == scene::note::START_URL => {
                // A private space shows no history.
                let visits = match (&self.page_index, self.is_private()) {
                    (Some(index), false) => index.recent(4 * crate::start::RECENT).unwrap_or_default(),
                    _ => Vec::new(),
                };
                let mut recent = crate::start::recent(&visits);
                if let Some(index) = &self.page_index {
                    for r in &mut recent {
                        r.thumb = index.thumb(&r.url).ok().flatten().map(|j| crate::start::data_url("image/jpeg", &j));
                    }
                }
                let icons = self.start_icons(&recent);
                (crate::start::page_url(&self.theme, self.settings.search, &self.start_page, &recent, &icons), web::PageKind::App)
            }
            None if n.url == scene::note::SETTINGS_URL => (
                crate::settings_page::page_url(&self.theme, &self.settings_data()),
                web::PageKind::App,
            ),
            None => (n.url.clone(), web::PageKind::Web),
        };
        let jar = self.jar.as_ref();
        match Page::new(self.page_key(id), &url, kind, (css.x as i32, css.y as i32), self.page_scale, &self.spawn_beside, jar) {
            Some(page) => {
                self.cards.insert(id, Card { page, loading: true, can_back: false, can_forward: false, hidden: false, cursor: web::Cursor::Default });
            }
            None => log::error!("failed to create browser for card {}", id.0),
        }
    }

    /// First start: the welcome page, left of the first page (`beside`).
    /// Not focused, so the page keeps the keys.
    fn add_guide(&mut self, beside: Option<NodeId>) -> NodeId {
        let rect = match beside.and_then(|id| self.scene.node(id)) {
            Some(src) => placement::left_of(&self.scene, src, self.card_size.x, self.card_size.y),
            None => placement::free_near(&self.scene, self.camera.center, self.card_size.x, self.card_size.y),
        };
        let id = self.scene.add_welcome(rect);
        self.create_page(id);
        self.dirty = true;
        log::info!("first run: welcome page {}", id.0);
        id
    }

    /// Nothing to show: an empty card with a search box in the middle of the
    /// view, taking the keys.
    fn add_start(&mut self) -> NodeId {
        let rect = placement::free_near(&self.scene, self.camera.center, self.card_size.x, self.card_size.y);
        let id = self.scene.add_start(rect);
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.focus(Some(id));
        self.give_keys(PageKeys::Insert);
        log::info!("start page {}", id.0);
        id
    }

    /// Ctrl+T, *New card*: the floating address bar or a start page card,
    /// as the *New card opens* setting says.
    /// Ctrl+L or the sidebar's address pill: the address bar with the current
    /// card's address, to load something else there. A note has no address:
    /// the bar opens a new card instead.
    fn open_address_bar(&mut self) {
        let page = self.focused.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note());
        let url = page.filter(|n| n.url != scene::note::START_URL).map(|n| n.url.clone()).unwrap_or_default();
        let target = if page.is_some() { Target::Focused } else { Target::NewCard };
        self.omnibox = Some(Omnibox::new(&url, target));
    }

    fn new_card(&mut self) {
        match self.settings.new_card {
            prefs::NewCard::AddressBar => self.omnibox = Some(Omnibox::new("", Target::NewCard)),
            prefs::NewCard::StartPage => {
                let id = self.add_start();
                self.ensure_visible(id);
            }
        }
    }

    /// Whether card `id` is a start page that nothing was typed in yet.
    fn is_start(&self, id: NodeId) -> bool {
        self.scene.node(id).is_some_and(|n| !n.is_note() && n.url == scene::note::START_URL)
    }

    /// Something was typed in start page `id`: load it there, so the card
    /// becomes an ordinary page.
    fn start_go(&mut self, id: NodeId, input: &str) {
        let Some(url) = self.settings.resolve_url(input) else { return };
        let Some(n) = self.scene.node_mut(id) else { return };
        log::info!("start page {}: {url}", id.0);
        n.url = url.clone();
        n.title = url;
        if let Some(card) = self.cards.remove(&id) {
            self.release(card.page);
        }
        self.create_page(id);
        self.dirty = true;
        self.focus(Some(id));
        self.give_keys(PageKeys::Page);
    }

    /// A message from a start page or the search engine guide (card `id`).
    /// Returns whether it was one of theirs.
    fn engine_page_action(&mut self, id: NodeId, action: &str) -> bool {
        if let Some(input) = action.strip_prefix(crate::start::GO) {
            if self.is_start(id) {
                self.start_go(id, input);
            }
        } else if let Some(url) = action.strip_prefix(crate::start::BESIDE) {
            // Like a link opened in a new card: beside the start page,
            // joined to it by a trail, which keeps its place.
            if self.is_start(id) && scene::startpage::pinnable(url)
                && let Some(new) = self.open_card(Rect::default(), url, Some(id))
            {
                log::info!("start page {}: opened {url} beside it as card {}", id.0, new.0);
                self.ensure_visible(new);
            }
        } else if let Some(engine) = action.strip_prefix(crate::start::ENGINE) {
            match prefs::SearchEngine::from_id(engine) {
                Some(e) => self.set_engine(e),
                None => log::warn!("unknown search engine {engine:?}"),
            }
        } else if action == crate::start::GUIDE {
            self.show_engines(id);
        } else if let Some(json) = action.strip_prefix(crate::start::SAVE) {
            match storage::start::from_json(json) {
                Some(start) => self.set_start_page(id, start),
                None => log::warn!("start page {}: unreadable {json:?}", id.0),
            }
        } else {
            return false;
        }
        true
    }

    /// The icons a start page shows: its pinned sites' and `recent` pages'.
    fn start_icons(&self, recent: &[crate::start::Recent]) -> crate::start::Icons {
        let urls = self.start_page.pins().map(|p| p.url.as_str()).chain(recent.iter().map(|r| r.url.as_str()));
        crate::start::icons(urls, |site| self.icons.get(site).map(|i| i.icon.png()))
    }

    /// Start page `from` changed the pins or parts: keep them, and show
    /// them on the other live start pages.
    fn set_start_page(&mut self, from: NodeId, start: scene::startpage::StartPage) {
        if start == self.start_page {
            return;
        }
        log::info!(
            "start page: groups {:?}, greeting {} {:?}, clock {} (24h {}), pins {}, recent {}",
            start.groups.iter().map(|g| (g.name.as_str(), g.pins.len())).collect::<Vec<_>>(),
            start.show_greeting,
            start.name,
            start.show_clock,
            start.hour24,
            start.show_pins,
            start.show_recent
        );
        self.start_page = start;
        if let Err(e) = storage::start::save_start(&self.data_dir, &self.start_page) {
            log::warn!("start page: saving: {e}");
            self.toast("Couldn't save the start page");
        }
        let script = crate::start::state_script(&self.start_page, &self.start_icons(&[]));
        for (id, card) in &self.cards {
            if *id != from && self.is_start(*id) && card.page.kind() == web::PageKind::App {
                card.page.run_script(&script);
            }
        }
    }

    /// Search with `e` from now on (chosen on a start page or the guide).
    fn set_engine(&mut self, e: prefs::SearchEngine) {
        if self.settings.search == e {
            return;
        }
        self.settings.search = e;
        log::info!("setting Search = {}", e.name());
        self.save_settings();
        self.engine_changed();
    }

    /// Tells live start pages and the guide which engine is in use.
    fn engine_changed(&self) {
        let script = crate::engines::engine_script(self.settings.search);
        for (id, card) in &self.cards {
            let ours = self.scene.node(*id).is_some_and(|n| n.url == scene::note::START_URL || n.url == scene::note::ENGINES_URL);
            if ours && card.page.kind() == web::PageKind::App {
                card.page.run_script(&script);
            }
        }
    }

    /// The search engine guide: go to it if it is on the board, else open it
    /// beside card `from`.
    fn show_engines(&mut self, from: NodeId) {
        self.keys_in_page = false;
        if let Some(id) = self.scene.engines_card() {
            self.go_to_card(id);
            return;
        }
        let Some(id) = self.scene.spawn_beside(from, scene::note::ENGINES_URL) else { return };
        if let Some(n) = self.scene.node_mut(id) {
            n.title = scene::note::ENGINES_TITLE.to_string();
        }
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.focus(Some(id));
        self.ensure_visible(id);
        log::info!("search engine guide {}", id.0);
    }

    /// A button on the welcome page: the same as its keys.
    fn welcome_action(&mut self, action: crate::welcome::Action) {
        use crate::welcome::Action;
        self.keys_in_page = false; // the board's own UI takes over
        match action {
            Action::NewCard => self.run_command(Command::NewCard),
            Action::Palette => self.open_palette(),
            Action::Shortcuts => self.run_command(Command::Shortcuts),
            Action::Settings => self.run_command(Command::Settings),
        }
    }

    /// *Welcome* (Ctrl+K): go to the welcome page if it is on the board
    /// (unfolding it if needed), else add one in the middle of the view.
    fn show_guide(&mut self) {
        if let Some(id) = self.scene.welcome_card() {
            log::info!("welcome page {} already on the board", id.0);
            self.go_to_card(id);
            return;
        }
        let rect = placement::free_near(&self.scene, self.camera.center, self.card_size.x, self.card_size.y);
        let id = self.scene.add_welcome(rect);
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.focus(Some(id));
        self.ensure_visible(id);
        log::info!("welcome page {} added", id.0);
    }

    /// N: a new, empty note in the middle of the view, ready to type in.
    fn new_note_at_center(&mut self) -> NodeId {
        let rect = placement::free_near(&self.scene, self.camera.center, self.card_size.x, self.card_size.y);
        let id = self.scene.add_note(rect, "");
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.note_target = Some(id);
        self.focus(Some(id));
        self.ensure_visible(id);
        self.give_keys(PageKeys::Insert);
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
        self.give_keys(PageKeys::Insert);
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
        } else if c.page.kind() == web::PageKind::App {
            c.page.run_script(&crate::welcome::theme_script(&self.theme));
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
                let Some(t) = self.scene.note_beside(src, quote, self.card_size.x, self.card_size.y) else { return };
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

    /// Shift+Enter in the palette or an address field: open `url` beside the
    /// current card, joined to it by a trail edge as a clicked link would be,
    /// and make it current with the keys in its page. With no card to open
    /// beside, it opens in the middle of the view like Enter.
    fn open_beside_current(&mut self, url: &str) {
        let src = self.current().filter(|&id| self.scene.node(id).is_some());
        let id = match src {
            Some(src) => {
                let Some(id) = self.open_card(Rect::default(), url, Some(src)) else { return };
                log::info!("opened card {} beside card {}", id.0, src.0);
                self.focus(Some(id));
                self.ensure_visible(id);
                Some(id)
            }
            None => self.new_card_at_center(url),
        };
        if id.is_some() {
            self.give_keys(PageKeys::Page);
        }
    }

    /// Loads `url` in the current card: a web page navigates, a start page
    /// becomes the page, and a note or the welcome page (which never
    /// navigate) gets it opened beside.
    fn load_here(&mut self, url: &str) {
        let Some(page) = self.focused_page() else { return };
        if page.kind() == web::PageKind::Web {
            page.load(url);
        } else if let Some(id) = self.focused.filter(|&id| self.is_start(id)) {
            self.start_go(id, url);
        } else {
            self.open_beside_current(url);
        }
    }

    /// p / P: the clipboard's text as an address (or a search) here / beside.
    fn open_clipboard(&mut self, beside: bool) {
        let text = self.clipboard.text().unwrap_or_default();
        let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let Some(url) = self.settings.resolve_url(&line) else {
            self.toast("The clipboard has no text to open");
            return;
        };
        log::info!("opening the clipboard {}", if beside { "beside" } else { "here" });
        if beside { self.open_beside_current(&url) } else { self.load_here(&url) }
    }

    fn new_card_at_center(&mut self, url: &str) -> Option<NodeId> {
        let center = self.camera.center;
        let rect = placement::free_near(&self.scene, center, self.card_size.x, self.card_size.y);
        let id = self.open_card(rect, url, None)?;
        self.focus(Some(id));
        self.ensure_visible(id);
        Some(id)
    }

    /// Trailboard was started again while running: open what it was given as
    /// new cards in the space on screen and ask for the window to come forward.
    fn relaunched(&mut self, args: &[String], cwd: &str) {
        let urls: Vec<String> = args.iter().filter_map(|a| launch::target(a, std::path::Path::new(cwd), &self.settings)).collect();
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
            // A card on screen shrinks away; its page's picture stays until then.
            if !self.still
                && self.card_on_screen(id)
                && let Some(n) = self.scene.node(id)
            {
                let ghost = (n.clone(), self.page_key(id), Instant::now());
                self.ghosts.retain(|g| g.0.id != id);
                self.ghosts.push(ghost);
            }
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
        // A card shrinking away keeps its picture until it is gone (`draw`).
        let ghost = self.ghosts.iter().any(|g| g.1 == key);
        if let Some(r) = self.renderer.as_mut().filter(|_| !ghost) {
            r.remove_page(key);
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        if self.read == Some(id) {
            log::info!("solo: card {} is gone; back to the board", id.0);
            self.read = None;
            self.camera_moved();
        }
    }

    /// Ctrl+Shift+T: bring back the newest closed card that isn't already back (via undo).
    fn reopen_closed(&mut self) -> bool {
        while let Some((node, edges)) = self.closed.pop() {
            if self.restore_closed(node, edges) {
                return true;
            }
        }
        false
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
        if let (Some(old), Some(_)) = (self.focused, id) {
            self.alt_card = Some(old);
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
        self.entered() && self.page_keys != PageKeys::Page
    }

    /// A card is entered: its page has the keys (Page, Insert or
    /// Passthrough), after Enter or a click in it, or in Solo.
    fn entered(&self) -> bool {
        self.focused_page().is_some() && (self.keys_in_page || self.read.is_some())
    }

    /// Page mode: a card is entered and our page keys come first.
    fn page_mode(&self) -> bool {
        self.entered() && self.page_keys == PageKeys::Page
    }

    /// The focused card takes the keys, in `mode`.
    fn give_keys(&mut self, mode: PageKeys) {
        self.keys_in_page = true;
        self.set_page_keys(mode);
    }

    fn set_page_keys(&mut self, mode: PageKeys) {
        if self.page_keys != mode {
            log::info!("keys: {mode:?}");
        }
        self.page_keys = mode;
        self.pending.clear();
    }

    /// The mode a card starts in when entered: Insert where you type
    /// (notes, the start page), Page elsewhere.
    fn entry_mode(&self, id: NodeId) -> PageKeys {
        if self.scene.node(id).is_some_and(Node::is_note) || self.is_start(id) {
            PageKeys::Insert
        } else {
            PageKeys::Page
        }
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
        self.give_keys(self.entry_mode(id));
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

    /// Runs a board key's action (`scene::keys::board`). A count repeats the
    /// actions that make sense more than once (`3j`); the guards are the ones
    /// each key always had (most wait while a drag is on).
    fn board_action(&mut self, action: KeyAction, count: Option<u32>) {
        /// More repeats than this do nothing more on a board.
        const MAX_REPEAT: u32 = 100;
        let times = if action.repeats() { count.unwrap_or(1).clamp(1, MAX_REPEAT) } else { 1 };
        let idle = self.drag_is_idle();
        log::debug!("board key: {action:?} ×{times}");
        match action {
            KeyAction::Move(dir) if idle => (0..times).for_each(|_| self.move_current(dir)),
            KeyAction::Pan(dir) if idle => (0..times).for_each(|_| self.pan_by_key(dir)),
            KeyAction::ZoomIn | KeyAction::ZoomOut if self.read.is_none() => {
                (0..times).for_each(|_| self.zoom_by_key(action == KeyAction::ZoomIn))
            }
            KeyAction::ScrollDown | KeyAction::ScrollUp => {
                (0..times).for_each(|_| self.scroll_current(action == KeyAction::ScrollDown))
            }
            KeyAction::Enter if idle => self.enter_current(),
            KeyAction::Close if idle && !self.selection.is_empty() => {
                let ids = self.selection.ids();
                log::info!("closing {} selected card(s)", ids.len());
                self.close_cards(&ids);
            }
            KeyAction::Close if idle => {
                if let Some(id) = self.focused {
                    self.close_card(id);
                }
            }
            KeyAction::Quit => {
                log::info!("quit (Q)");
                self.quit = true;
            }
            KeyAction::Fit => self.fit_all(),
            KeyAction::LinkHints if idle => self.show_link_hints(),
            KeyAction::Hint(kind) if idle => self.ask_link_hints(false, Some(kind)),
            KeyAction::Tidy if idle => self.tidy_board(),
            KeyAction::Arrange if idle => self.start_arrange(),
            KeyAction::NewNote if idle => {
                self.new_note_at_center();
            }
            KeyAction::NewSticky if idle => {
                self.new_sticky_at_center();
            }
            KeyAction::Sidebar if idle => self.toggle_sidebar(),
            KeyAction::Select if idle => self.toggle_mark(),
            KeyAction::SelectAll => self.selection.select_all(&self.scene),
            KeyAction::Fill if idle && self.read.is_none() => self.fill_current(),
            KeyAction::NameTrail if idle => self.ask_trail_name(),
            KeyAction::TrailColour if idle => self.cycle_trail_colour(),
            KeyAction::Fold if idle => self.toggle_fold(),
            KeyAction::Outline if idle => self.outline_in_view(),
            KeyAction::StackPrev if idle => self.peek_stack(-1),
            KeyAction::StackNext if idle => self.peek_stack(1),
            KeyAction::Undo => self.undo(false),
            KeyAction::Redo => self.undo(true),
            KeyAction::CommandLine => self.open_command_line(),
            _ if idle && self.card_key(action, count) => {}
            _ => {}
        }
    }

    /// Keys that go from card to card (M12 step 7), on the board and in a
    /// page; false for any other action. From a page, the keys stay with
    /// the page of the card gone to, in Page mode.
    fn card_key(&mut self, action: KeyAction, count: Option<u32>) -> bool {
        const MAX_REPEAT: u32 = 100;
        let times = count.unwrap_or(1).clamp(1, MAX_REPEAT) as i64;
        let order = || scene::tidy::order(&self.scene);
        match action {
            KeyAction::NextCard | KeyAction::PrevCard => {
                let by = if action == KeyAction::NextCard { times } else { -times };
                match nav::step(&order(), self.focused, by) {
                    Some(to) => self.switch_card(to),
                    None => self.toast("No cards here"),
                }
            }
            KeyAction::FirstCard | KeyAction::LastCard => {
                let o = order();
                let to = if action == KeyAction::FirstCard { o.first() } else { o.last() };
                match to.copied() {
                    Some(to) => self.switch_card(to),
                    None => self.toast("No cards here"),
                }
            }
            KeyAction::NthCard(n) => match nav::nth(&order(), n) {
                Some(to) => self.switch_card(to),
                None => self.toast(format!("No card {n} here")),
            },
            KeyAction::OtherCard => match self.alt_card.filter(|&a| Some(a) != self.focused && self.scene.node(a).is_some()) {
                Some(to) => self.switch_card(to),
                None => self.toast("No card used before this one"),
            },
            KeyAction::Move(dir) => {
                // Ctrl+w then a direction, from a page.
                if let Some(to) = self.focused.and_then(|from| nav::neighbour(&self.scene, from, dir)) {
                    self.switch_card(to);
                }
            }
            KeyAction::PickCard => {
                self.open_command_line();
                if let Some(ob) = self.omnibox.as_mut() {
                    ob.insert("card ");
                }
            }
            KeyAction::Reopen => {
                let page = self.entered();
                let mut any = false;
                for _ in 0..times {
                    any |= self.reopen_closed();
                }
                if !any {
                    self.toast("No closed card to reopen");
                } else if page {
                    self.keep_page_keys();
                }
            }
            KeyAction::CloseOthers => self.ask_close_others(),
            KeyAction::Clone => self.clone_card(),
            _ => return false,
        }
        true
    }

    /// Makes card `to` current from a key: shown if folded away, brought on
    /// screen (read, in Solo). From a page, the keys go with it in Page mode.
    fn switch_card(&mut self, to: NodeId) {
        let page = self.entered();
        if self.focused == Some(to) {
            self.ensure_visible(to);
            return;
        }
        self.go_to_card(to);
        if page {
            self.keep_page_keys();
        }
        log::info!("card keys: to card {}", to.0);
    }

    /// The focused card's page has the keys, in Page mode (as after Enter,
    /// or as Solo gives them).
    fn keep_page_keys(&mut self) {
        let Some(id) = self.focused else { return };
        if self.read.is_none() {
            self.keys_in_page = true;
        }
        self.set_page_keys(PageKeys::Page);
        if let Some(c) = self.cards.get(&id) {
            c.page.set_focus(true);
        }
    }

    /// d (or Ctrl+w) in a page: close the card and go on to the next one in
    /// reading order (the previous, at the end), keeping Page mode and Solo.
    fn close_from_page(&mut self, id: NodeId) {
        let solo = self.read.is_some();
        let next = nav::step(&scene::tidy::order(&self.scene), Some(id), 1).filter(|&n| n != id);
        self.close_card(id);
        log::info!("closed card {} from its page; next {:?}", id.0, next.map(|n| n.0));
        let Some(next) = next else { return };
        if solo {
            self.read_card(next);
        } else {
            self.go_to_card(next);
            self.keep_page_keys();
        }
    }

    /// co / `:card-only`: close every other card in the space, asking first.
    fn ask_close_others(&mut self) {
        let Some(id) = self.focused.filter(|&id| self.scene.node(id).is_some()) else {
            self.toast("Pick a card to keep first");
            return;
        };
        let n = self.scene.all_nodes().filter(|n| n.id != id).count();
        if n == 0 {
            self.toast("No other cards here");
            return;
        }
        let cards = if n == 1 { "the 1 other card".to_string() } else { format!("the {n} other cards") };
        self.confirm = Some(Confirm::CloseOthers(id));
        self.toast(format!("Close {cards} in {}? Enter closes · Esc keeps them", self.spaces.active_entry().name));
    }

    fn close_others(&mut self, keep: NodeId) {
        if self.scene.node(keep).is_none() {
            return;
        }
        let others: Vec<NodeId> = self.scene.all_nodes().map(|n| n.id).filter(|&id| id != keep).collect();
        log::info!("closing the {} card(s) other than card {}", others.len(), keep.0);
        self.close_cards(&others);
        self.go_to_card(keep);
        let n = others.len();
        self.toast(format!("Closed {n} card{} · :undo brings them back", if n == 1 { "" } else { "s" }));
    }

    /// gC / `:clone`: the card's page opened again beside it, joined by a
    /// trail edge (its own back and forward list starts afresh).
    fn clone_card(&mut self) {
        let Some(n) = self.focused.and_then(|id| self.scene.node(id)) else {
            self.toast("Pick a card to clone first");
            return;
        };
        if n.is_note() || n.url.is_empty() {
            self.toast("Only a page can be cloned");
            return;
        }
        let url = n.url.clone();
        let page = self.entered();
        self.open_beside_current(&url);
        if !page {
            self.keys_in_page = false;
        }
    }

    /// Runs a Page mode key's action (`scene::keys::page`) on the entered card.
    fn page_action(&mut self, action: KeyAction, count: Option<u32>) {
        const MAX_REPEAT: u32 = 100;
        let times = if action.repeats() { count.unwrap_or(1).clamp(1, MAX_REPEAT) } else { 1 };
        let Some(id) = self.focused else { return };
        log::debug!("page key: {action:?} ×{times}");
        use web::scroll::{Axis, Edge as ScrollEdge, Unit};
        let n = times as i32;
        match action {
            KeyAction::Scroll(dir) => {
                let (axis, n) = match dir {
                    Dir::Down => (Axis::Y, n),
                    Dir::Up => (Axis::Y, -n),
                    Dir::Left => (Axis::X, -n),
                    Dir::Right => (Axis::X, n),
                };
                self.scroll_section(id, axis, n, Unit::Line);
            }
            KeyAction::ScrollDown => self.scroll_section(id, Axis::Y, n, Unit::Half),
            KeyAction::ScrollUp => self.scroll_section(id, Axis::Y, -n, Unit::Half),
            KeyAction::Top => self.scroll_edge(id, ScrollEdge::Top),
            KeyAction::Bottom => self.scroll_edge(id, ScrollEdge::Bottom),
            KeyAction::LeftEdge => self.scroll_edge(id, ScrollEdge::Left),
            KeyAction::RightEdge => self.scroll_edge(id, ScrollEdge::Right),
            KeyAction::Sections => self.ask_sections(id, SectionAsk::Label),
            KeyAction::NextSection => self.ask_sections(id, SectionAsk::Step(true)),
            KeyAction::PrevSection => self.ask_sections(id, SectionAsk::Step(false)),
            KeyAction::OuterSection => self.ask_sections(id, SectionAsk::Out),
            KeyAction::LinkHintsHere => self.show_link_hints_to(true),
            KeyAction::LinkHints => self.show_link_hints_to(false),
            KeyAction::Hint(kind) => self.ask_link_hints(false, Some(kind)),
            KeyAction::Back | KeyAction::Forward => {
                if let Some(p) = self.focused_page() {
                    for _ in 0..times {
                        if action == KeyAction::Back { p.back() } else { p.forward() }
                    }
                }
            }
            KeyAction::Reload | KeyAction::ReloadFresh => {
                if let Some(p) = self.focused_page().filter(|p| p.kind() == web::PageKind::Web) {
                    if action == KeyAction::Reload { p.reload() } else { p.reload_fresh() }
                }
            }
            KeyAction::Insert => self.set_page_keys(PageKeys::Insert),
            KeyAction::Passthrough => self.set_page_keys(PageKeys::Passthrough),
            KeyAction::FocusField => {
                if let Some(p) = self.focused_page() {
                    p.focus_first_field();
                }
            }
            KeyAction::CopyUrl | KeyAction::CopyTitle => {
                let Some(n) = self.scene.node(id) else { return };
                let (text, what) = if action == KeyAction::CopyUrl { (n.url.clone(), "address") } else { (n.title.clone(), "title") };
                if text.is_empty() {
                    return;
                }
                self.clipboard.set_text(&text);
                self.toast(format!("Copied the {what}"));
            }
            KeyAction::CopyMarkdown => {
                let Some(n) = self.scene.node(id) else { return };
                if n.url.is_empty() {
                    return;
                }
                let text = scene::export::link(&n.title, &n.url);
                self.clipboard.set_text(&text);
                self.toast("Copied a Markdown link");
            }
            KeyAction::PasteHere => self.open_clipboard(false),
            KeyAction::PasteBeside => self.open_clipboard(true),
            KeyAction::AddressBar => self.open_address_bar(),
            KeyAction::AddressBarBeside => self.omnibox = Some(Omnibox::new("", Target::Beside)),
            KeyAction::Find => self.open_find(),
            KeyAction::FindNext | KeyAction::FindPrev if !self.last_find.is_empty() => {
                if let Some(p) = self.focused_page() {
                    for _ in 0..times {
                        p.find(&self.last_find, action == KeyAction::FindNext, true);
                    }
                }
            }
            KeyAction::FindNext | KeyAction::FindPrev => self.open_find(),
            KeyAction::CommandLine => self.open_command_line(),
            KeyAction::Close => self.close_from_page(id),
            // Cards from the page (`card_key`); board actions don't come
            // from the page table.
            _ => {
                self.card_key(action, count);
            }
        }
    }

    // ------------------------------------------------------------ scroll sections

    /// Scrolls card `id`'s scroll target (else the automatic pick) `n` steps
    /// (positive: down / right), going on to the section around it at its
    /// edge. The page answers with what moved (`got_scroll`).
    fn scroll_section(&mut self, id: NodeId, axis: web::scroll::Axis, n: i32, unit: web::scroll::Unit) {
        let Some(card) = self.cards.get(&id) else { return };
        let target = self.scroll_targets.get(&id).copied();
        card.page.scroll_command(&web::scroll::by(target, axis, n, unit, !self.still));
        self.link_hints = None; // scrolling moves the targets
    }

    /// gg / G / 0 / $: the scroll target to an edge, at once.
    fn scroll_edge(&mut self, id: NodeId, edge: web::scroll::Edge) {
        let Some(card) = self.cards.get(&id) else { return };
        let target = self.scroll_targets.get(&id).copied();
        card.page.scroll_command(&web::scroll::to(target, edge));
        self.link_hints = None;
    }

    /// Ask card `id`'s page for its sections in view; `got_scroll` acts on
    /// the answer as `why` says.
    fn ask_sections(&mut self, id: NodeId, why: SectionAsk) {
        let Some(card) = self.cards.get(&id) else { return };
        card.page.scroll_command(&web::scroll::list());
        self.section_ask = Some((id, why));
        log::debug!("sections asked of card {} ({why:?})", id.0);
    }

    /// A click (or a hint's click) at `(x, y)` in card `id`'s page (CSS px
    /// of the view): the innermost section there becomes the scroll target.
    fn pick_section_at(&mut self, id: NodeId, x: i32, y: i32) {
        let Some((_, css)) = self.page_view(id) else { return };
        let Some(card) = self.cards.get(&id) else { return };
        card.page.scroll_command(&web::scroll::pick(x as f32, y as f32, css.x));
    }

    /// Choose section `section` as card `id`'s scroll target (the page's
    /// own number goes back to the automatic pick, as after a navigation,
    /// except when chosen on purpose with `keep_page`).
    fn set_scroll_target(&mut self, id: NodeId, section: u32, keep_page: bool) {
        if section == web::scroll::PAGE && !keep_page {
            self.scroll_targets.remove(&id);
        } else {
            self.scroll_targets.insert(id, section);
        }
        log::info!("scroll target of card {}: {} (#{section})", id.0, self.section_name(id, section));
        let rect = self.sections.get(&id).and_then(|v| v.iter().find(|s| s.id == section)).map(|s| s.rect);
        if let Some(rect) = rect {
            self.scroll_flash = Some(ScrollFlash { id, section, rect, at: Instant::now() });
        }
        self.scroll_note = Some((id, section, String::new()));
    }

    /// The pill's name for a section of card `id`: `page`, `section 2.1`.
    fn section_name(&self, id: NodeId, section: u32) -> String {
        if section == web::scroll::PAGE {
            return "page".into();
        }
        let named = self.sections.get(&id).and_then(|v| scrolltarget::names(v).remove(&section));
        match named {
            Some(n) if n != "page" => format!("section {n}"),
            _ => "section".into(),
        }
    }

    /// A page forgets its sections when it navigates (or closes): its
    /// scroll target goes back to the automatic pick.
    fn forget_sections(&mut self, id: NodeId) {
        self.scroll_targets.remove(&id);
        self.sections.remove(&id);
        if self.scroll_note.as_ref().is_some_and(|(n, ..)| *n == id) {
            self.scroll_note = None;
        }
        if self.scroll_flash.as_ref().is_some_and(|f| f.id == id) {
            self.scroll_flash = None;
        }
        if self.section_ask.is_some_and(|(n, _)| n == id) {
            self.section_ask = None;
        }
    }

    /// An answer from card `id`'s page about its scroll sections.
    fn got_scroll(&mut self, id: NodeId, report: web::scroll::Report) {
        use web::scroll::{Note, Report};
        let Some((_, css)) = self.page_view(id) else { return };
        let scaled = |width: f32, r: [f32; 4]| {
            let k = css.x / width.max(1.0);
            Rect::new(r[0] * k, r[1] * k, r[2] * k, r[3] * k)
        };
        match report {
            Report::List { width, auto, sections } => {
                let Some((_, why)) = self.section_ask.filter(|(n, _)| *n == id) else {
                    log::debug!("sections from card {} not asked for", id.0);
                    return;
                };
                self.section_ask = None;
                let list: Vec<scrolltarget::Section> = sections
                    .iter()
                    .map(|s| scrolltarget::Section { id: s.id, parent: s.parent, rect: scaled(width, s.rect) })
                    .collect();
                log::info!(
                    "sections of card {}: {} ({})",
                    id.0,
                    list.len(),
                    sections.iter().map(|s| format!("#{}<{}", s.id, s.parent.map_or(-1, i64::from))).collect::<Vec<_>>().join(" ")
                );
                self.sections.insert(id, list.clone());
                // Where stepping starts: the chosen target if it is still in
                // view, else what the automatic pick would scroll now.
                let current = self.scroll_targets.get(&id).copied().filter(|t| list.iter().any(|s| s.id == *t)).unwrap_or(auto);
                match why {
                    SectionAsk::Label => self.label_sections(id, &list),
                    SectionAsk::Step(forward) => {
                        if list.len() < 2 {
                            self.toast("Only the page scrolls here");
                        }
                        if let Some(next) = scrolltarget::step(&list, Some(current), forward) {
                            self.set_scroll_target(id, next, true);
                        }
                    }
                    SectionAsk::Out => match scrolltarget::outer(&list, current) {
                        Some(o) => self.set_scroll_target(id, o, true),
                        None => {
                            self.set_scroll_target(id, current, true);
                            self.toast("This is the whole page");
                        }
                    },
                    // The pill names sections from the list just kept.
                    SectionAsk::Name => {}
                }
            }
            Report::Did { width, scrolled, rect, pct, note, reset } => {
                if self.focused != Some(id) {
                    return;
                }
                if reset {
                    log::info!("scroll target of card {} is gone; automatic again", id.0);
                    self.scroll_targets.remove(&id);
                }
                let rect = scaled(width, rect);
                let known = scrolled == web::scroll::PAGE || self.sections.get(&id).is_some_and(|v| v.iter().any(|s| s.id == scrolled));
                let text = match note {
                    Note::NoSideways => {
                        self.toast("Nothing here scrolls sideways");
                        String::new()
                    }
                    Note::Shown => "back in view".to_string(),
                    Note::Moved | Note::Edge => format!("{pct}%"),
                };
                log::info!("scrolled card {}: {} · {text} ({note:?})", id.0, self.section_name(id, scrolled));
                // Outline what moved when it isn't what moved last (the
                // target changed, or the scroll went on to an outer section).
                let fresh = self.scroll_flash.as_ref().is_none_or(|f| f.id != id || f.section != scrolled || f.at.elapsed() > SCROLL_FLASH);
                let changed = self.scroll_flash.as_ref().is_none_or(|f| f.id != id || f.section != scrolled);
                if changed || (fresh && note == Note::Shown) {
                    self.scroll_flash = Some(ScrollFlash { id, section: scrolled, rect, at: Instant::now() });
                } else if let Some(f) = self.scroll_flash.as_mut() {
                    f.rect = rect;
                }
                self.scroll_note = Some((id, scrolled, text));
                if !known && self.section_ask.is_none() {
                    self.ask_sections(id, SectionAsk::Name);
                }
            }
            Report::Wheel { width, at, delta, jump } => {
                // Only the card whose keys asked; and a wheel can only scroll
                // the page that asked for it.
                if self.focused != Some(id) {
                    return;
                }
                let k = css.x / width.max(1.0);
                let Some(card) = self.cards.get(&id) else { return };
                let (x, y) = ((at.0 * k).round() as i32, (at.1 * k).round() as i32);
                let (dx, dy) = ((delta.0 as f32 * k).round() as i32, (delta.1 as f32 * k).round() as i32);
                log::info!("scrolled card {} by wheel at {x},{y} ({dx},{dy})", id.0);
                card.page.mouse_wheel(x, y, 0, dx, dy, jump);
            }
            Report::Picked { id: section } => {
                if self.focused != Some(id) {
                    return;
                }
                if section == web::scroll::PAGE {
                    self.scroll_targets.remove(&id);
                } else {
                    self.scroll_targets.insert(id, section);
                }
                log::info!("scroll target of card {} by click: #{section}", id.0);
                if self.scroll_note.as_ref().is_some_and(|(n, ..)| *n == id) {
                    self.scroll_note = None;
                }
            }
        }
    }

    /// `s`: labels on card `id`'s sections in view; typing one picks it.
    fn label_sections(&mut self, id: NodeId, list: &[scrolltarget::Section]) {
        let order = scrolltarget::order(list);
        let sections: Vec<(u32, Rect)> = order
            .iter()
            .filter_map(|o| list.iter().find(|s| s.id == *o))
            .map(|s| (s.id, s.rect))
            .collect();
        if sections.len() < 2 {
            self.toast("Only the page scrolls here");
            if let Some(&(only, _)) = sections.first() {
                self.set_scroll_target(id, only, false);
            }
            return;
        }
        let hints = hint::Hints::new(sections.len());
        log::info!(
            "section labels on card {}: {}",
            id.0,
            hints.labels.iter().zip(&sections).map(|(l, (s, _))| format!("{l}=#{s}")).collect::<Vec<_>>().join(" ")
        );
        self.link_hints = Some(LinkHints { id, here: true, kind: None, chosen: Vec::new(), links: Vec::new(), sections, hints: Some(hints) });
    }

    /// Where the entered page's focus went: a field the user put the caret
    /// in switches to Insert; leaving a field (or a click outside one)
    /// switches back to Page. A page focusing a field on its own as it loads
    /// changes nothing. Only the entered card is listened to.
    fn page_focus(&mut self, id: NodeId, report: web::focus::Report) {
        if self.focused != Some(id) || !self.entered() || self.page_keys == PageKeys::Passthrough {
            return;
        }
        match report {
            web::focus::Report::Field { editable: true, user: true } if self.page_keys == PageKeys::Page => {
                self.set_page_keys(PageKeys::Insert);
            }
            web::focus::Report::Field { editable: false, .. } if self.page_keys == PageKeys::Insert => {
                // A note or the start page types without a field we can see.
                if self.entry_mode(id) == PageKeys::Page {
                    self.set_page_keys(PageKeys::Page);
                }
            }
            web::focus::Report::NoField => self.toast("No text field here"),
            _ => {}
        }
    }

    // ---------------------------------------------------------------- sidebar

    /// The sidebar's rows and where they go.
    fn sidebar_layout(&mut self) -> (Vec<scene::sidebar::Row>, Panel) {
        let rows = self.sidebar.rows(&self.scene);
        let panel = Panel::new(self.viewport(), self.ui(), &crate::sidebar::slots(&rows));
        (rows, panel)
    }

    /// The sidebar's sheet, for hit tests that don't need its rows.
    fn sidebar_rect(&self) -> Rect {
        Panel::sheet(self.viewport(), self.ui())
    }

    /// The pointer resting on the window's left edge peeks the sidebar (no
    /// keys); it goes again once the pointer has been away from it a moment.
    fn sidebar_peek_tick(&mut self) {
        let busy = self.read.is_some() || self.omnibox.is_some() || self.menu.is_some() || self.hints || !matches!(self.drag, Drag::None);
        let at_edge = self.pointer_in && !busy && self.cursor.x <= 2.0 * self.scale();
        let reach = self.sidebar_rect().inflate(24.0 * self.ui());
        let near = self.pointer_in && (reach.contains(self.cursor) || self.cursor.x <= reach.right()) || self.sidebar_press.is_some() || self.menu.is_some();
        match self.peek.update(Instant::now(), self.pointer_in, at_edge, near, self.sidebar_open, self.sidebar_peek) {
            Some(crate::sidebar::PeekAct::Open) => {
                self.sidebar_open = true;
                self.sidebar_peek = true;
                self.sidebar_keys = false;
                self.sidebar_scroll_to_sel();
                log::info!("sidebar peek");
            }
            Some(crate::sidebar::PeekAct::Close) => self.close_sidebar(),
            None => {}
        }
    }

    /// The sidebar is on screen (not in Solo, which covers the window).
    fn sidebar_shown(&self) -> bool {
        self.sidebar_open && self.read.is_none()
    }

    /// The sidebar takes board keys: open, given the keys, and no page has them.
    fn sidebar_has_keys(&self) -> bool {
        self.sidebar_shown() && self.sidebar_keys && !self.entered()
    }

    /// B: opens the sidebar with the keys on the current card's row; closes
    /// it when it has the keys; gives it the keys when it is open without.
    fn toggle_sidebar(&mut self) {
        if self.read.is_some() {
            return;
        }
        if self.sidebar_open && self.sidebar_keys && !self.sidebar_peek {
            self.close_sidebar();
            return;
        }
        // B on a peek keeps it open.
        self.sidebar_peek = false;
        self.sidebar_open = true;
        self.take_sidebar_keys();
        log::info!("sidebar open, highlight {:?}", self.sidebar.sel);
    }

    fn close_sidebar(&mut self) {
        self.sidebar_open = false;
        self.sidebar_peek = false;
        self.sidebar_spot = None;
        self.sidebar_keys = false;
        self.sidebar_hover = None;
        self.sidebar_press = None;
        log::info!("sidebar closed");
    }

    /// The sidebar takes the keys, highlighting the current card's row.
    fn take_sidebar_keys(&mut self) {
        if self.entered() {
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
        let (rows, panel) = self.sidebar_layout();
        let i = self.sidebar.index(&rows);
        if i.is_some() {
            self.sidebar_at = i;
        }
        self.sidebar.scroll = panel.keep_visible(i, self.sidebar.scroll);
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
        let shown = self.sidebar_layout().1.fits(self.sidebar.scroll) as isize;
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
            Some(KeyCode::KeyR) => self.sidebar_name(&rows),
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

    /// R: the name box for the trail of the highlighted row (a card on no
    /// trail is named on its own, and becomes a trail of its own).
    fn sidebar_name(&mut self, rows: &[scene::sidebar::Row]) {
        use scene::sidebar::{Group, Row};
        match self.sidebar.index(rows).map(|i| &rows[i]) {
            Some(&Row::Card { id, .. } | &Row::Header { group: Group::Trail(id), .. }) => self.ask_trail_name_of(id),
            _ => self.toast("Pick a trail or a card to name"),
        }
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
        let want = if dir == 0 { self.zooms.borrow().default } else { pagezoom::step(self.zooms.borrow().get(&url), dir) };
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

    /// Ctrl+K: the palette, over everything but Solo's page.
    fn open_palette(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::Palette));
        log::info!("palette open");
    }

    /// `:`: the palette as a command line (`scene::command`).
    fn open_command_line(&mut self) {
        self.open_palette();
        let mut ob = Omnibox::new(":", Target::Palette);
        ob.select_all = false;
        self.omnibox = Some(ob);
        log::info!("command line open");
    }

    /// The command line again, holding `line` (a completion to finish).
    fn fill_command_line(&mut self, line: &str) {
        let mut ob = Omnibox::new(&format!(":{line}"), Target::Palette);
        ob.select_all = false;
        self.omnibox = Some(ob);
    }

    /// Values the command line offers for an argument: the spaces' names
    /// (this one last) or the cards' titles (most recently used first).
    fn command_values(&self, kind: scene::command::Kind) -> Vec<String> {
        use scene::command::Kind;
        match kind {
            Kind::Space => {
                let mut names: Vec<String> = self.spaces.spaces.iter().filter(|s| s.id != self.spaces.active).map(|s| s.name.clone()).collect();
                names.push(self.spaces.active_entry().name.clone());
                names
            }
            Kind::Card => {
                let mut cards: Vec<&Node> = self.scene.all_nodes().filter(|n| !n.title.trim().is_empty()).collect();
                cards.sort_by_key(|n| std::cmp::Reverse((self.used.get(&n.id).copied().unwrap_or(0), n.z)));
                let mut seen = HashSet::new();
                cards.into_iter().map(|n| n.title.trim().to_string()).filter(|t| seen.insert(t.clone())).collect()
            }
            _ => Vec::new(),
        }
    }

    /// The best of `names` for `typed`: the same name (case ignored), else
    /// the best fuzzy match.
    fn best_name(typed: &str, names: &[String]) -> Option<usize> {
        if let Some(i) = names.iter().position(|n| n.trim().eq_ignore_ascii_case(typed.trim())) {
            return Some(i);
        }
        let cands: Vec<scene::palette::Candidate> = names.iter().map(|n| scene::palette::Candidate { text: n, alt: "" }).collect();
        scene::palette::rank(typed, &cands).first().map(|r| r.index)
    }

    fn save_recent_commands(&self) {
        if let Err(e) = storage::commands::save_recent(&self.data_dir, &self.recent_commands) {
            log::warn!("command line: saving recent lines: {e}");
        }
    }

    /// Runs a command line (`scene::command`); a line that isn't a command
    /// says why. Lines are remembered, except in a private space.
    fn run_line(&mut self, line: &str) {
        use scene::command::Cmd;
        let parsed = match scene::command::parse(line) {
            Ok(p) => p,
            Err(e) => {
                log::info!("command line: {line:?}: {e}");
                self.toast(e.to_string());
                return;
            }
        };
        log::info!("command line: {:?} → {:?} {:?}", line, parsed.spec.cmd, parsed.arg);
        if !self.is_private() {
            self.recent_commands.push(line.trim().trim_start_matches(':').trim());
            self.save_recent_commands();
        }
        let arg = parsed.arg.clone().unwrap_or_default();
        let url = || self.settings.resolve_url(&arg);
        let page = |app: &mut Self, action: KeyAction, count: Option<u32>| {
            if app.focused_page().is_some() {
                app.page_action(action, count);
            } else {
                app.toast("No page here");
            }
        };
        match parsed.spec.cmd {
            Cmd::Open => match url() {
                Some(u) if self.focused_page().is_some() => self.load_here(&u),
                Some(u) if self.new_card_at_center(&u).is_some() => self.give_keys(PageKeys::Page),
                _ => {}
            },
            Cmd::Beside => {
                if let Some(u) = url() {
                    self.open_beside_current(&u);
                }
            }
            Cmd::New => match parsed.arg.as_ref().and_then(|_| url()) {
                Some(u) => {
                    if self.new_card_at_center(&u).is_some() {
                        self.give_keys(PageKeys::Page);
                    }
                }
                None => self.run_command(Command::NewCard),
            },
            Cmd::StartPage => self.run_command(Command::StartPage),
            Cmd::Note => self.run_command(Command::NewNote),
            Cmd::Sticky => self.run_command(Command::NewSticky),
            Cmd::Close => self.run_command(Command::CloseCard),
            Cmd::Exit => self.run_command(Command::Quit),
            Cmd::Save => self.run_command(Command::SaveMarkdown),
            Cmd::Reload if parsed.arg.is_some() => page(self, KeyAction::ReloadFresh, None),
            Cmd::Reload => page(self, KeyAction::Reload, None),
            Cmd::Back => page(self, KeyAction::Back, parsed.count()),
            Cmd::Forward => page(self, KeyAction::Forward, parsed.count()),
            Cmd::Find => {
                if parsed.arg.is_some() {
                    self.last_find = arg.clone();
                }
                self.open_find();
            }
            Cmd::Copy => {
                let action = match arg.as_str() {
                    "title" => KeyAction::CopyTitle,
                    "markdown" => KeyAction::CopyMarkdown,
                    _ => KeyAction::CopyUrl,
                };
                page(self, action, None);
            }
            Cmd::Zoom => self.run_command(match arg.as_str() {
                "in" => Command::ZoomIn,
                "out" => Command::ZoomOut,
                _ => Command::ZoomReset,
            }),
            Cmd::Tidy => self.run_command(Command::Tidy),
            Cmd::Fit => self.run_command(Command::Fit),
            Cmd::Fold => self.run_command(Command::Fold),
            Cmd::Undo => self.run_command(Command::Undo),
            Cmd::Redo => self.run_command(Command::Redo),
            Cmd::Reopen => self.run_command(Command::Reopen),
            Cmd::Solo => self.run_command(Command::Read),
            Cmd::Sidebar => self.run_command(Command::Sidebar),
            Cmd::CardNext => {
                self.card_key(KeyAction::NextCard, parsed.count());
            }
            Cmd::CardPrev => {
                self.card_key(KeyAction::PrevCard, parsed.count());
            }
            Cmd::CardOnly => self.ask_close_others(),
            Cmd::Clone => self.clone_card(),
            Cmd::Card if arg.parse::<usize>().is_ok() => {
                // A number counts in reading order (`:card 3`).
                let n: usize = arg.parse().unwrap_or(0);
                match n.checked_sub(1).and_then(|i| scene::tidy::order(&self.scene).get(i).copied()) {
                    Some(to) => self.switch_card(to),
                    None => self.toast(format!("No card {n} here")),
                }
            }
            Cmd::Card => {
                let cards: Vec<(NodeId, String)> = self.scene.all_nodes().map(|n| (n.id, n.title.trim().to_string())).collect();
                let names: Vec<String> = cards.iter().map(|(_, t)| t.clone()).collect();
                // Most recently used first, so the same title goes to the card used last.
                let mut order: Vec<usize> = (0..cards.len()).collect();
                order.sort_by_key(|&i| std::cmp::Reverse(self.used.get(&cards[i].0).copied().unwrap_or(0)));
                let sorted: Vec<String> = order.iter().map(|&i| names[i].clone()).collect();
                match Self::best_name(&arg, &sorted) {
                    // From a page the keys go with it (and Solo reads it), as gt does.
                    Some(i) => self.switch_card(cards[order[i]].0),
                    None => self.toast(format!("No card called “{arg}”")),
                }
            }
            Cmd::Name if parsed.arg.is_some() => match read::pick(&self.scene, self.focused, self.camera.center) {
                Some(id) => self.name_trail(id, &arg),
                None => self.toast("No trail here"),
            },
            Cmd::Name => self.run_command(Command::NameTrail),
            Cmd::Colour => self.run_command(Command::ColourTrail),
            Cmd::SelectAll => self.run_command(Command::SelectAll),
            Cmd::Space => {
                let names: Vec<String> = self.spaces.spaces.iter().map(|s| s.name.clone()).collect();
                match Self::best_name(&arg, &names) {
                    Some(i) => self.switch_space(self.spaces.spaces[i].id),
                    None => self.toast(format!("No space called “{arg}”")),
                }
            }
            Cmd::SpaceNew | Cmd::SpacePrivate => {
                self.new_space(parsed.spec.cmd == Cmd::SpacePrivate);
                if parsed.arg.is_some() {
                    self.rename_space(&arg);
                }
            }
            Cmd::SpaceRename => self.rename_space(&arg),
            Cmd::SpaceDelete => self.run_command(Command::DeleteSpace),
            Cmd::Private => self.run_command(Command::PrivateSpace),
            Cmd::Tint => self.run_command(Command::NextTint),
            Cmd::Photo => self.run_command(Command::BackgroundPhoto),
            Cmd::History => {
                self.open_history();
                if parsed.arg.is_some()
                    && let Some(ob) = self.omnibox.as_mut()
                {
                    ob.insert(&arg);
                }
            }
            Cmd::Downloads => self.run_command(Command::Downloads),
            Cmd::Settings => self.run_command(Command::Settings),
            Cmd::Keys => self.run_command(Command::Shortcuts),
            Cmd::Welcome => self.run_command(Command::Welcome),
            Cmd::Ads => self.run_command(Command::ToggleBlocking),
            Cmd::AdsAllowed => self.run_command(Command::AllowedSites),
            Cmd::ClearHistory => self.run_command(Command::ClearHistory),
            Cmd::ClearCookies => self.run_command(Command::ClearCookies),
        }
    }

    /// Runs a palette row (the palette is already closed). `beside` (Shift+Enter
    /// or Shift+click) opens a page row beside the current card instead of
    /// in the middle of the view; other rows run as usual.
    fn run_palette(&mut self, row: palette::Row, beside: bool) {
        log::info!("palette: {:?} {:?}{}", row.item, row.label, if beside { " (beside)" } else { "" });
        match row.item {
            Item::Card(id) => {
                if self.read.is_some() {
                    self.leave_read();
                }
                self.go_to_card(id);
            }
            Item::Space(id) => self.switch_space(id),
            Item::Open(url) if beside => self.open_beside_current(&url),
            Item::Open(url) => {
                if self.new_card_at_center(&url).is_some() {
                    self.give_keys(PageKeys::Page);
                }
            }
            Item::Closed(id) => {
                if self.read.is_some() {
                    self.leave_read();
                }
                self.reopen_closed_card(id);
            }
            Item::Command(c) => self.run_command(c),
            Item::Download(key) => self.open_download(key),
            Item::AllowedSite(site) => self.block_site_again(&site),
            Item::Photo(path) => self.set_space_photo(path),
            Item::Run(line) | Item::Recent(line) => self.run_line(&line),
            Item::Complete(line) if scene::command::parse(&line).is_ok() => self.run_line(&line),
            Item::Complete(line) => self.fill_command_line(&line),
        }
    }

    /// Ad blocking on card `id`'s site, for its page menu: `None` for notes,
    /// pages that aren't web pages, or while blocking is off in the settings.
    fn blocking_state(&self, id: NodeId) -> Option<web::menu::Blocking> {
        let n = self.scene.node(id).filter(|n| !n.is_note() && self.settings.block_ads)?;
        let site = scene::icon::site(&n.url)?;
        let card = self.cards.get(&id);
        let blocked = card.map_or(0, |c| c.page.blocked());
        let hidden = card.map_or(0, |c| c.page.hidden());
        let banner_rejected = card.is_some_and(|c| c.page.banner_rejected());
        Some(web::menu::Blocking { site, on: !self.blocking.allows(&n.url), blocked, hidden, banner_rejected })
    }

    /// JavaScript and images on card `id`'s site, for its page menu: `None`
    /// for notes and pages that aren't on a web site.
    fn site_content_state(&self, id: NodeId) -> Option<web::menu::SiteContent> {
        use scene::sitesettings::Content;
        let n = self.scene.node(id).filter(|n| !n.is_note())?;
        if self.cards.get(&id).is_some_and(|c| c.page.kind() != web::PageKind::Web) {
            return None;
        }
        let site = scene::icon::site(&n.url)?;
        Some(web::menu::SiteContent {
            site,
            javascript: self.sites.allows(&n.url, Content::JavaScript, self.settings.javascript),
            images: self.sites.allows(&n.url, Content::Images, self.settings.images),
        })
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
        self.reload_site(&site, on);
        self.toast(match (on, private) {
            (true, _) => format!("Blocking ads on {site} again"),
            (false, false) => format!("Ads allowed on {site}"),
            (false, true) => format!("Ads allowed on {site} until you quit"),
        });
    }

    /// Reloads the live pages of `site` after blocking changed there (`on`:
    /// back on), so the change shows.
    fn reload_site(&self, site: &str, on: bool) {
        let mut reloaded = 0;
        for (cid, c) in &self.cards {
            let same = self.scene.node(*cid).is_some_and(|n| !n.is_note() && scene::privacy::covers(site, &n.url));
            if same && c.page.kind() == web::PageKind::Web {
                c.page.reload_fresh();
                reloaded += 1;
            }
        }
        log::info!("blocking {} {site}: {reloaded} page(s) reloaded", if on { "back on for" } else { "off for" });
    }

    // ---------------------------------------------------------------- board background

    /// The picture the board shows now, if *Board background* is *Photo*:
    /// the space's own, else the desktop's wallpaper.
    fn board_photo(&self) -> Option<PathBuf> {
        if self.settings.backdrop != prefs::Backdrop::Photo {
            return None;
        }
        self.spaces.active_entry().photo.as_ref().map(PathBuf::from).or_else(|| self.wallpaper.clone())
    }

    /// Looks the desktop's wallpaper up again (it changes without the theme
    /// changing), at most every two seconds unless `now`.
    fn check_wallpaper(&mut self, now: bool) {
        if !now && self.wallpaper_checked.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return;
        }
        self.wallpaper_checked = Some(Instant::now());
        let w = crate::backdrop::desktop_wallpaper();
        if w != self.wallpaper {
            log::info!("desktop wallpaper: {w:?}");
            self.wallpaper = w;
        }
    }

    /// *Board background* changed in the settings.
    fn backdrop_changed(&mut self) {
        self.check_wallpaper(true);
        self.photo.retry();
        if self.settings.backdrop == prefs::Backdrop::Photo && self.board_photo().is_none() {
            self.toast("No desktop wallpaper found · pick a photo from the space's menu");
        }
    }

    /// *Background photo…*: the list of pictures to show behind this space.
    fn open_photo_list(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.check_wallpaper(true);
        self.photo_files = storage::photo::pictures(&crate::backdrop::picture_dirs(), 400);
        self.omnibox = Some(Omnibox::new("", Target::Photo));
        log::info!("photo list open: {} pictures, wallpaper {:?}", self.photo_files.len(), self.wallpaper);
    }

    /// The space's background becomes `path` (`None`: the desktop's
    /// wallpaper), and the board shows photos if it didn't.
    fn set_space_photo(&mut self, path: Option<String>) {
        let id = self.spaces.active;
        let name = path.as_deref().map_or_else(
            || "desktop wallpaper".to_string(),
            |p| Path::new(p).file_stem().map_or_else(|| p.to_string(), |s| s.to_string_lossy().to_string()),
        );
        self.spaces.set_photo(id, path);
        self.save_index();
        self.photo.retry();
        if self.settings.backdrop != prefs::Backdrop::Photo {
            self.choose_setting(prefs::Key::Backdrop, prefs::Backdrop::Photo.id());
        }
        log::info!("space {} photo {:?}", id.0, self.spaces.active_entry().photo);
        self.toast(format!("Background: {name}"));
    }

    /// The list of sites where ads are allowed, from the palette.
    fn open_allowed_sites(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::AllowedSites));
        log::info!("allowed sites open: {:?}", self.blocking.allowed_sites());
    }

    /// Blocks ads on `site` again, from the allowed-sites list (which stays
    /// open, its highlight kept in range).
    fn block_site_again(&mut self, site: &str) {
        if !self.blocking.block_again(site) {
            return;
        }
        self.reload_site(site, true);
        self.toast(format!("Blocking ads on {site} again"));
        let len = self.omnibox.as_ref().map_or(0, |ob| self.list_rows(ob).len());
        if let Some(ob) = self.omnibox.as_mut() {
            ob.sel = ob.sel.min(len.saturating_sub(1));
        }
    }

    /// Ctrl+,: the settings page. Go to it if it is on the board (unfolding
    /// it if needed), else add one in the middle of the view.
    fn open_settings(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.omnibox = None;
        self.finish_arrange(true);
        if let Some(id) = self.scene.settings_card() {
            log::info!("settings page {} already on the board", id.0);
            self.go_to_card(id);
            return;
        }
        let rect = placement::free_near(&self.scene, self.camera.center, self.card_size.x, self.card_size.y);
        let id = self.scene.add_settings(rect);
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.focus(Some(id));
        self.ensure_visible(id);
        log::info!("settings page {} added", id.0);
    }

    /// The settings page's data: the settings as in force, where the number
    /// of live pages comes from, and the sandbox.
    fn settings_data(&self) -> String {
        let shown = Settings {
            live_pages: self.max_live,
            ..self.settings.clone()
        };
        let mut notes = Vec::new();
        if self.live_from_env {
            notes.push((prefs::Key::LivePages, "TRAILBOARD_MAX_LIVE".to_string()));
        }
        let facts: Vec<(&str, String, String)> = self
            .sandbox
            .iter()
            .map(|s| ("Sandbox", s.summary().to_string(), s.explain().to_string()))
            .collect();
        crate::settings_page::data(&shown, &notes, &facts)
    }

    /// Shows live settings pages the settings as they are now.
    fn settings_changed(&self) {
        let script = crate::settings_page::data_script(&self.settings_data());
        for (id, card) in &self.cards {
            let ours = self
                .scene
                .node(*id)
                .is_some_and(|n| !n.is_note() && n.url == scene::note::SETTINGS_URL);
            if ours && card.page.kind() == web::PageKind::App {
                card.page.run_script(&script);
            }
        }
    }

    /// A message from the settings page (card `id`). Returns whether it was one of its.
    fn settings_page_action(&mut self, id: NodeId, action: &str) -> bool {
        use crate::settings_page as sp;
        if !self
            .scene
            .node(id)
            .is_some_and(|n| !n.is_note() && n.url == scene::note::SETTINGS_URL)
        {
            return false;
        }
        if let Some(msg) = action.strip_prefix(sp::SET) {
            match sp::parse_set(msg) {
                Some((key, value)) => self.choose_setting(key, value),
                None => log::warn!("settings page: unknown setting {msg:?}"),
            }
        } else if let Some(server) = action.strip_prefix(sp::PROXY) {
            self.set_proxy_server(server);
        } else if let Some(px) = action.strip_prefix(sp::CORNER_RADIUS) {
            self.set_corner_radius(px);
        } else if let Some(msg) = action.strip_prefix(sp::KEYWORD_SET) {
            match sp::parse_keyword(msg) {
                Some((key, name, url)) => self.set_keyword(key, name, url),
                None => log::warn!("settings page: bad keyword {msg:?}"),
            }
        } else if let Some(key) = action.strip_prefix(sp::KEYWORD_REMOVE) {
            self.remove_keyword(key);
        } else if action == sp::KEYWORD_RESET {
            self.settings.keywords = scene::search::defaults();
            log::info!("setting keywords = defaults");
            self.toast("Search keywords back to the defaults");
            self.save_settings();
        } else if let Some(name) = action.strip_prefix(sp::RUN) {
            match sp::command(name) {
                Some(c) => {
                    self.keys_in_page = false; // the board's own UI takes over
                    self.run_command(c);
                }
                None => log::warn!("settings page: unknown command {name:?}"),
            }
        } else if action == crate::start::GUIDE {
            self.show_engines(id);
        } else {
            return false;
        }
        true
    }

    /// Sets `key` to the choice `value` (`Settings::choices`), applies it at
    /// once and saves it.
    fn choose_setting(&mut self, key: prefs::Key, value: &str) {
        let mut s = self.settings.clone();
        if key == prefs::Key::LivePages {
            s.live_pages = self.max_live; // compare with what is in force
        }
        if !s.choose(key, value) {
            self.settings_changed(); // the page shows what is really set
            return;
        }
        let before = std::mem::replace(&mut self.settings, s);
        log::info!("setting {key:?} = {}", self.settings.value(key));
        match key {
            prefs::Key::Search => self.engine_changed(),
            prefs::Key::NewCard | prefs::Key::ScrollSpeed | prefs::Key::DownloadTo | prefs::Key::SpacePanel => {}
            prefs::Key::Corners => self.set_theme(self.theme.clone()),
            prefs::Key::Backdrop => self.backdrop_changed(),
            prefs::Key::LivePages => {
                self.max_live = self.settings.live_pages;
                self.live_from_env = false;
            }
            prefs::Key::History => {
                if !self.settings.history {
                    self.text_due.clear();
                }
            }
            prefs::Key::PageColours => self.apply_page_scheme(),
            prefs::Key::BlockAds => web::shield::set_enabled(self.settings.block_ads),
            prefs::Key::DefaultZoom => {
                self.zooms.borrow_mut().default = self.settings.default_zoom_factor();
                for c in self.cards.values() {
                    c.page.refresh_zoom();
                }
            }
            prefs::Key::Spellcheck | prefs::Key::ThirdPartyCookies | prefs::Key::DoNotTrack => self.apply_browsing(),
            prefs::Key::Language => {
                // Spelling follows at once; Chromium's network stack only
                // reads the languages sites are told when it starts.
                self.apply_browsing();
                let name = self.settings.value(key);
                self.toast(format!("Page language: {name} · sites see it after a restart"));
            }
            prefs::Key::Proxy => {
                self.apply_browsing();
                if self.settings.proxy == prefs::Proxy::Custom
                    && self.settings.proxy_server.is_empty()
                {
                    self.toast("Type the proxy server under Proxy");
                }
            }
            prefs::Key::JavaScript | prefs::Key::Images => self.site_content_changed(&before),
            prefs::Key::Camera | prefs::Key::Microphone | prefs::Key::Location | prefs::Key::Notifications => {
                self.apply_permissions();
            }
        }
        self.save_settings();
    }

    fn save_settings(&mut self) {
        if let Err(e) = storage::settings::save_settings(&self.data_dir, &self.settings) {
            log::warn!("settings: saving: {e}");
            self.toast("Couldn't save settings");
        }
        self.settings_changed();
    }

    /// Hands the language, spelling, cookie, tracking and proxy settings to
    /// Chromium (the profile and every private space's cookie jar).
    fn apply_browsing(&self) {
        let s = &self.settings;
        let tag = prefs::language_tag(s.language, &system_locale());
        let proxy = match s.proxy {
            prefs::Proxy::System => web::prefs::Proxy::System,
            prefs::Proxy::Direct => web::prefs::Proxy::Direct,
            prefs::Proxy::Custom => web::prefs::Proxy::Fixed(s.proxy_server.clone()),
        };
        let p = web::prefs::Prefs {
            accept_languages: prefs::accept_languages(&tag),
            spellcheck: s.spellcheck,
            dictionary: tag,
            block_third_party_cookies: s.block_third_party_cookies,
            proxy,
        };
        log::info!("browsing prefs: {p:?}, do not track {}", s.do_not_track);
        web::prefs::apply(&p);
        for jar in self.jar.iter().chain(self.sessions.values().filter_map(|s| s.jar.as_ref())) {
            jar.refresh_prefs();
        }
        web::prefs::set_do_not_track(s.do_not_track);
    }

    /// Hands JavaScript and images (the defaults and each site's own) to the pages.
    fn apply_site_content(&self) {
        use scene::sitesettings::Content;
        let (js, images) = (self.settings.javascript, self.settings.images);
        let sites = self.sites.clone();
        web::set_javascript(move |url| sites.allows(url, Content::JavaScript, js));
        let sites = self.sites.clone();
        web::prefs::set_images(move |url| sites.allows(url, Content::Images, images));
    }

    /// After JavaScript or images changed (the setting, from `before`, or a
    /// site's own): applies it and reloads the live pages it changes.
    fn site_content_changed(&mut self, before: &Settings) {
        let old_sites = self.sites.clone();
        self.site_content_changed_from(&old_sites, before);
    }

    fn site_content_changed_from(&mut self, old_sites: &scene::sitesettings::SiteSettings, before: &Settings) {
        use scene::sitesettings::Content;
        self.apply_site_content();
        let mut reloaded = 0;
        for (id, c) in &self.cards {
            if c.page.kind() != web::PageKind::Web {
                continue;
            }
            let Some(url) = self.scene.node(*id).map(|n| n.url.clone()) else { continue };
            let images_were = old_sites.allows(&url, Content::Images, before.images);
            let js_was = old_sites.allows(&url, Content::JavaScript, before.javascript);
            let images_now = self.sites.allows(&url, Content::Images, self.settings.images);
            let js_now = self.sites.allows(&url, Content::JavaScript, self.settings.javascript);
            if js_was != js_now {
                c.page.refresh_javascript(); // reloads
                reloaded += 1;
            } else if images_were != images_now {
                c.page.reload();
                reloaded += 1;
            }
        }
        log::info!("site content: {reloaded} page(s) reloaded");
    }

    /// Hands the camera, microphone, location and notification settings to
    /// the pages. Answers given this session are forgotten, so a change
    /// applies to sites already asked.
    fn apply_permissions(&self) {
        use web::permission::{Decision, Kind};
        let s = &self.settings;
        let decide = |p: prefs::Permission| match p {
            prefs::Permission::Ask => Decision::Ask,
            prefs::Permission::Allow => Decision::Allow,
            prefs::Permission::Block => Decision::Block,
        };
        let (camera, mic, location, notes) = (decide(s.camera), decide(s.microphone), decide(s.location), decide(s.notifications));
        web::permission::set_policy(move |k| match k {
            Kind::Camera => camera,
            Kind::Microphone => mic,
            Kind::Location => location,
            Kind::Notifications => notes,
            Kind::Other => Decision::Block,
        });
        web::permission::forget_answers();
    }

    /// Page menu: JavaScript or images on or off for card `id`'s site.
    fn toggle_site_content(&mut self, id: NodeId, what: scene::sitesettings::Content) {
        use scene::sitesettings::Content;
        let Some(url) = self.scene.node(id).filter(|n| !n.is_note()).map(|n| n.url.clone()) else { return };
        let default = match what {
            Content::JavaScript => self.settings.javascript,
            Content::Images => self.settings.images,
        };
        let on = !self.sites.allows(&url, what, default);
        let old = self.sites.clone();
        if !self.sites.set(&url, what, on, default) {
            return;
        }
        let before = self.settings.clone();
        self.site_content_changed_from(&old, &before);
        if self.persist
            && let Err(e) = storage::sitesettings::save_sites(&self.data_dir, &self.sites)
        {
            log::error!("saving site settings: {e}");
        }
        let site = pagezoom::site_of(&url);
        self.toast(format!("{} {} on {site}", what.name(), if on { "on" } else { "off" }));
    }

    /// The slider under *Corners: Custom* let go at `text` px.
    fn set_corner_radius(&mut self, text: &str) {
        let Some(px) = text.trim().parse::<u8>().ok().filter(|&r| r <= prefs::MAX_CORNER_RADIUS) else {
            log::warn!("settings page: bad corner radius {text:?}");
            return;
        };
        self.settings.corner_radius = px;
        log::info!("setting Corners = {}", self.settings.value(prefs::Key::Corners));
        self.set_theme(self.theme.clone());
        self.save_settings();
    }

    /// Enter in the proxy field: the server for *Proxy: Custom*.
    /// Adds a search keyword, or changes the one with that keyword.
    fn set_keyword(&mut self, key: &str, name: &str, url: &str) {
        match scene::search::check(key, name, url) {
            Ok(k) => {
                log::info!("setting keyword {} = {} ({})", k.key, k.url, k.name);
                self.toast(format!("“{} …” searches {}", k.key, k.name));
                scene::search::put(&mut self.settings.keywords, k);
                self.save_settings();
            }
            Err(e) => self.toast(e),
        }
    }

    fn remove_keyword(&mut self, key: &str) {
        let before = self.settings.keywords.len();
        self.settings.keywords.retain(|k| k.key != key);
        if self.settings.keywords.len() != before {
            log::info!("setting keyword {key} removed");
            self.save_settings();
        }
    }

    fn set_proxy_server(&mut self, text: &str) {
        let server = text.trim().to_string();
        if server.is_empty() {
            self.settings.proxy = prefs::Proxy::System;
            self.toast("No proxy server given · Proxy follows the system");
        } else {
            self.toast(format!("Proxy: {server}"));
        }
        self.settings.proxy_server = server;
        log::info!("setting Proxy = {}", self.settings.value(prefs::Key::Proxy));
        self.apply_browsing();
        self.save_settings();
    }

    // ---------------------------------------------------------------- permissions

    /// A site asks for a permission and the settings say to ask: queue the
    /// question; the first one shows.
    fn permission_asked(&mut self, id: u64, origin: String, kinds: Vec<web::permission::Kind>) {
        self.asks.push(Ask { id, origin, kinds });
        self.show_next_ask();
    }

    fn show_next_ask(&mut self) {
        if self.confirm.is_some() {
            return; // another question first (it may be this one)
        }
        let Some(ask) = self.asks.first() else { return };
        let what: Vec<&str> = ask.kinds.iter().map(|k| k.name()).collect();
        let site = pagezoom::site_of(&ask.origin);
        let msg = format!("{site} wants your {} · Enter allows · Esc blocks", what.join(" and "));
        log::info!("permission: asking: {msg}");
        self.confirm = Some(Confirm::Permission);
        self.toast(msg);
    }

    /// The user answered the question showing.
    fn answer_ask(&mut self, allow: bool) {
        self.confirm = None;
        if self.asks.is_empty() {
            return;
        }
        let ask = self.asks.remove(0);
        web::permission::remember(&ask.origin, &ask.kinds, allow);
        let answered = web::permission::answer(ask.id, allow);
        log::info!("permission: {} {:?} for {} (still waiting: {answered})", if allow { "allowed" } else { "blocked" }, ask.kinds, ask.origin);
        let what: Vec<&str> = ask.kinds.iter().map(|k| k.name()).collect();
        self.toast(format!("{} {} for {}", if allow { "Allowed" } else { "Blocked" }, what.join(" and "), pagezoom::site_of(&ask.origin)));
        self.show_next_ask();
    }

    /// Chromium withdrew request `id` (its page went away or navigated).
    fn permission_withdrawn(&mut self, id: u64) {
        let Some(i) = self.asks.iter().position(|a| a.id == id) else { return };
        self.asks.remove(i);
        if i == 0 && self.confirm == Some(Confirm::Permission) {
            self.confirm = None;
            self.toast = None;
            self.show_next_ask();
        }
    }

    // ---------------------------------------------------------------- cookies

    /// *Clear cookies*: asks first, as it signs you out everywhere.
    fn ask_clear_cookies(&mut self) {
        self.omnibox = None;
        self.confirm = Some(Confirm::ClearCookies);
        self.toast("Delete all cookies? Sites will sign you out · Enter deletes · Esc keeps them");
    }

    fn clear_cookies(&mut self) {
        self.confirm = None;
        if web::prefs::clear_cookies() {
            log::info!("cookies cleared");
            self.toast("Cookies deleted");
        } else {
            self.toast("Couldn't delete cookies");
        }
    }

    /// The rows under a list field: the palette's or the history view's.
    fn list_rows(&self, ob: &Omnibox) -> Vec<palette::Row> {
        if ob.target == Target::Photo {
            let current = self.spaces.active_entry().photo.as_deref();
            return palette::photo_rows(&ob.text, &self.photo_files, current, self.wallpaper.is_some());
        }
        if ob.target == Target::AllowedSites {
            return palette::allowed_rows(&ob.text, &self.blocking.allowed_sites(), &self.scene);
        }
        if ob.target == Target::Downloads {
            let missing = |p: &str| std::fs::symlink_metadata(p).is_err();
            return palette::download_rows(&ob.text, &self.downloads, missing, local_ms(now_ms()), local_ms);
        }
        if ob.target == Target::History {
            let visits = visits_for(&self.page_index, &self.visits_cache, &ob.text, true);
            return palette::history_rows(&ob.text, &self.scene, &visits, local_ms(now_ms()), local_ms);
        }
        if palette::is_command(&ob.text) {
            return palette::command_rows(&ob.text, &self.recent_commands, &|k| self.command_values(k));
        }
        let visits = visits_for(&self.page_index, &self.visits_cache, &ob.text, false);
        let closed: Vec<&Node> = self.closed.iter().map(|(n, _)| n).collect();
        palette::rows(&ob.text, &self.scene, &self.used, &closed, &self.spaces, &visits, &self.settings)
    }

    /// The panel the list under field `ob` is laid out in, for its `rows`:
    /// the palette's rows sit in labelled sections.
    fn list_panel(&self, ob: &Omnibox, rows: &[palette::Row]) -> palette::Panel {
        match ob.target {
            Target::Palette => palette::Panel::new(self.viewport(), self.ui(), palette::sections(rows, &ob.text)),
            _ => palette::Panel::plain(self.viewport(), self.ui(), rows.len()),
        }
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

    /// Chromium wants to save a file from page `page`: it goes to the folder
    /// the settings name under a free name, at once; or, set to ask, the
    /// path waits in a field (`Target::SaveAs`). A card that was opened for
    /// the link and never showed a page goes away again.
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
        let dir = match self.settings.download_to {
            prefs::DownloadTo::Desktop => storage::downloads::desktop_dir(),
            prefs::DownloadTo::Downloads | prefs::DownloadTo::Ask => storage::downloads::download_dir(),
        };
        let taken: HashSet<std::path::PathBuf> = self.downloads.running_paths().map(std::path::PathBuf::from).collect();
        let path = storage::downloads::target(&dir, suggested, &taken);
        if self.settings.download_to == prefs::DownloadTo::Ask {
            // One question at a time: an earlier download still asking is cancelled.
            if let Some(old) = self.save_as.take() {
                web::download::refuse(old.id);
                if old.blank {
                    self.close_blank_card(old.page);
                }
            }
            log::info!("download {id}: asking where to save {url}");
            self.save_as = Some(SaveAs { id, page, url: url.to_string(), blank });
            self.menu = None;
            self.omnibox = Some(Omnibox::new(&home_relative(&path), Target::SaveAs(id)));
            return;
        }
        self.start_download(page, id, url, &path, blank);
    }

    /// Enter in the save field: the download goes to the path typed (`~`
    /// for the home folder; a folder means the suggested name in it).
    fn save_download_as(&mut self, id: u32, text: &str) {
        let Some(pending) = self.save_as.take().filter(|s| s.id == id) else { return };
        let text = text.trim();
        let home = std::env::var("HOME").unwrap_or_default();
        let mut path = match text.strip_prefix("~/") {
            Some(rest) => std::path::Path::new(&home).join(rest),
            None => std::path::PathBuf::from(text),
        };
        if text.is_empty() || path.is_relative() {
            web::download::refuse(id);
            self.toast("Download cancelled · give a full path, like ~/Downloads/file.pdf");
            if pending.blank {
                self.close_blank_card(pending.page);
            }
            return;
        }
        if path.is_dir() {
            let name = pending.url.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("download");
            path = path.join(name);
        }
        self.start_download(pending.page, id, &pending.url, &path, pending.blank);
    }

    /// Saves download `id` of `url` at `path` and lists it (Ctrl+J).
    fn start_download(&mut self, page: u64, id: u32, url: &str, path: &std::path::Path, blank: bool) {
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            log::warn!("downloads: making {}: {e}", dir.display()); // Chromium will fail it
        }
        let path = path.to_path_buf();
        if !web::download::save_to(id, &path) {
            return;
        }
        let key = self.downloads.start(url, &path.to_string_lossy(), self.is_private(), now_ms());
        self.download_ids.insert(id, (key, page));
        log::info!("download {id}: {url} -> {}", path.display());
        self.save_downloads();
        let name = self.downloads.get(key).map(|d| d.name().to_string()).unwrap_or_default();
        self.toast(format!("Downloading {name} · Ctrl+J"));
        if blank {
            self.close_blank_card(page);
        }
    }

    /// Closes the card of page `page`, opened for a link that turned out to
    /// be a file, so it never showed anything (its page stays parked until
    /// the download ends, if there is one).
    fn close_blank_card(&mut self, page: u64) {
        if let Some(card) = self.node_of(page)
            && self.scene.node(card).is_some_and(|n| !n.is_note())
        {
            // Nothing to show or undo.
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
        let row = self.list_rows(ob).into_iter().nth(ob.sel);
        if let Some(palette::Item::Recent(line)) = row.as_ref().map(|r| &r.item) {
            if self.recent_commands.forget(line) {
                log::info!("command line: forgot {line:?}");
                self.save_recent_commands();
            }
            let panel = self.omnibox.as_ref().map(|ob| self.list_panel(ob, &self.list_rows(ob)));
            if let (Some(ob), Some(panel)) = (self.omnibox.as_mut(), panel) {
                ob.sel = ob.sel.min(panel.len().saturating_sub(1));
                ob.scroll = panel.keep_visible(ob.sel, ob.scroll);
            }
            return;
        }
        let Some(url) = row.and_then(|r| r.history) else { return };
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
        let panel = self.omnibox.as_ref().map(|ob| self.list_panel(ob, &self.list_rows(ob)));
        if let (Some(ob), Some(panel)) = (self.omnibox.as_mut(), panel) {
            ob.sel = ob.sel.min(panel.len().saturating_sub(1));
            ob.scroll = panel.keep_visible(ob.sel, ob.scroll);
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
            Confirm::CloseOthers(_) => "Kept the cards".to_string(),
            Confirm::ClearHistory => "Kept history".to_string(),
            Confirm::ClearCookies => "Kept cookies".to_string(),
            Confirm::Permission => String::new(), // `answer_ask` says what happened
        }
    }

    /// A question was answered no (any key but Enter, or a click).
    fn decline(&mut self, confirm: Confirm) {
        if confirm == Confirm::Permission {
            self.answer_ask(false);
        } else {
            self.toast(self.kept(confirm));
        }
    }

    fn run_command(&mut self, c: Command) {
        match c {
            Command::NewCard => self.new_card(),
            Command::StartPage => {
                let id = self.add_start();
                self.ensure_visible(id);
            }
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
            Command::Reopen => {
                self.reopen_closed();
            }
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
            Command::BackgroundPhoto => self.open_photo_list(),
            Command::DeleteSpace => self.ask_delete_space(),
            Command::History => self.open_history(),
            Command::ClearHistory => self.ask_clear_history(),
            Command::ClearCookies => self.ask_clear_cookies(),
            Command::Settings => self.open_settings(),
            Command::Downloads => self.open_downloads(),
            Command::Shortcuts => self.show_hints(),
            Command::Welcome => self.show_guide(),
            Command::ToggleBlocking => self.toggle_blocking(self.current()),
            Command::AllowedSites => self.open_allowed_sites(),
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
        use crate::sidebar::Spot;
        let (rows, panel) = self.sidebar_layout();
        let n = self.spaces.spaces.len();
        let closable = |i: usize| matches!(rows.get(i), Some(Row::Card { .. }));
        if let Some(spot) = panel.spot_at(p, n, self.sidebar.scroll, &closable) {
            match spot {
                Spot::Back | Spot::Forward | Spot::Reload => {
                    if let Some(c) = self.focused.and_then(|id| self.cards.get(&id)).filter(|c| c.page.kind() == web::PageKind::Web) {
                        match spot {
                            Spot::Back => c.page.back(),
                            Spot::Forward => c.page.forward(),
                            _ => c.page.reload(),
                        }
                    }
                }
                // A peek stays; a kept sheet goes.
                Spot::Keep if self.sidebar_peek => self.sidebar_peek = false,
                Spot::Keep => self.close_sidebar(),
                Spot::Address => self.open_address_bar(),
                Spot::SpaceName => self.open_sidebar_space_menu(panel.space_line()),
                Spot::NewCard => self.new_card(),
                Spot::Close(i) => {
                    if let Some(Row::Card { id, .. }) = rows.get(i) {
                        self.close_card(*id);
                    }
                    self.sidebar_spot = None;
                }
                Spot::Settings => {
                    self.close_sidebar();
                    self.open_settings();
                }
                Spot::NewSpace => {
                    self.close_sidebar();
                    self.new_space(false);
                }
                Spot::Space(c) => {
                    if let Some(to) = self.spaces.spaces.get(c).map(|s| s.id)
                        && to != self.spaces.active
                    {
                        self.switch_space(to);
                    }
                }
            }
            return;
        }
        if self.entered() {
            self.leave_page();
        }
        self.sidebar_keys = true;
        let Some(i) = panel.row_at(p, self.sidebar.scroll) else { return };
        self.sidebar.sel = Some(rows[i].key());
        self.sidebar_at = Some(i);
        match rows[i] {
            // A trail's header toggles on release, unless it was dragged. A
            // second click soon after names the trail, and takes back the
            // first click's toggle so the group stays as it was.
            Row::Header { group: Group::Trail(head), .. } => {
                let again = self.sidebar_toggled.take().is_some_and(|(t, h)| h == head && t.elapsed() < Duration::from_millis(400));
                if again {
                    self.sidebar.toggle(Group::Trail(head));
                    self.ask_trail_name_of(head);
                } else {
                    self.sidebar_press = Some(SidebarPress { head, at: p, dragging: false, to: None });
                }
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
        let (rows, panel) = self.sidebar_layout();
        press.to = panel
            .row_at(p, self.sidebar.scroll)
            .and_then(|i| scene::sidebar::drop_to(&self.scene, press.head, rows[i].group()));
        self.sidebar_press = Some(press);
    }

    /// Release after a press on a trail's header: a click toggles the group,
    /// a drag moves the trail where it was let go.
    fn sidebar_release(&mut self) {
        let Some(press) = self.sidebar_press.take() else { return };
        if !press.dragging {
            self.sidebar.toggle(scene::sidebar::Group::Trail(press.head));
            self.sidebar_toggled = Some((Instant::now(), press.head));
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
        self.show_link_hints_to(false);
    }

    /// Link hints; `here`: a chosen link loads in the same card (Page mode's
    /// f) instead of opening beside it.
    fn show_link_hints_to(&mut self, here: bool) {
        self.ask_link_hints(here, None);
    }

    /// Link hints, of `kind` if given (`;y`, `;d`…): only the targets that
    /// kind can use are labelled.
    fn ask_link_hints(&mut self, here: bool, kind: Option<hint::Kind>) {
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
        use web::links::Filter;
        let filter = match kind {
            None => Filter::All,
            Some(hint::Kind::Yank | hint::Kind::Background | hint::Kind::Many) => Filter::Links,
            Some(hint::Kind::Download) => Filter::Downloads,
            Some(hint::Kind::Image) => Filter::Images,
            Some(hint::Kind::Field) => Filter::Fields,
        };
        card.page.request_links(filter);
        self.link_hints = Some(LinkHints { id, here, kind, chosen: Vec::new(), links: Vec::new(), sections: Vec::new(), hints: None });
        log::info!("link hints asked of card {}{}", id.0, kind.map(|k| format!(" (;{})", k.key())).unwrap_or_default());
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
            let kind = lh.kind;
            self.link_hints = None;
            let loading = self.cards.get(&id).is_some_and(|c| c.loading);
            log::info!("link hints on card {}: no links in view", id.0);
            let what = match kind {
                None => "Nothing to click",
                Some(hint::Kind::Yank | hint::Kind::Background | hint::Kind::Many) => "No links",
                Some(hint::Kind::Download) => "No links or images",
                Some(hint::Kind::Image) => "No images",
                Some(hint::Kind::Field) => "No text fields",
            };
            self.toast(if loading { format!("{what} yet · the page is still loading") } else { format!("{what} in view") });
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
                    web::links::Action::Image(u) => format!("{l}=image {u}"),
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
                hint::Typed::Follow(i) if !lh.sections.is_empty() => {
                    let (id, (section, _)) = (lh.id, lh.sections[i]);
                    self.link_hints = None;
                    self.set_scroll_target(id, section, true);
                }
                hint::Typed::Follow(i) if lh.kind.is_some_and(hint::Kind::keeps) => {
                    h.restart();
                    if !lh.chosen.contains(&i) {
                        lh.chosen.push(i);
                    }
                    let (id, action) = (lh.id, lh.links[i].1.clone());
                    if let web::links::Action::Open(url) | web::links::Action::Image(url) = action {
                        self.open_behind(id, &url);
                    }
                }
                hint::Typed::Follow(i) => {
                    let (id, here, kind, (rect, action)) = (lh.id, lh.here, lh.kind, lh.links[i].clone());
                    self.link_hints = None;
                    match kind {
                        Some(k) => self.use_hint(id, k, rect, action),
                        None => match action {
                            web::links::Action::Open(url) if here => self.follow_here(id, &url),
                            web::links::Action::Open(url) | web::links::Action::Image(url) => self.follow_link(id, &url),
                            web::links::Action::Click => self.click_target(id, rect, false),
                            web::links::Action::Type => self.click_target(id, rect, true),
                        },
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

    /// A label of a `;` hint kind was chosen on card `id`'s page.
    fn use_hint(&mut self, id: NodeId, kind: hint::Kind, rect: Rect, action: web::links::Action) {
        use web::links::Action as A;
        let url = match &action {
            A::Open(u) | A::Image(u) => Some(u.clone()),
            A::Click | A::Type => None,
        };
        match (kind, url) {
            (hint::Kind::Yank, Some(url)) => {
                self.clipboard.set_text(&url);
                log::info!("hint: copied {url}");
                self.toast(if matches!(action, A::Image(_)) { "Copied the image's address" } else { "Copied the link's address" });
            }
            (hint::Kind::Download, Some(url)) => {
                log::info!("hint: download {url}");
                if let Some(c) = self.cards.get(&id) {
                    c.page.download(&url);
                }
            }
            (hint::Kind::Image, Some(url)) => self.follow_link(id, &url),
            (hint::Kind::Background | hint::Kind::Many, Some(url)) => {
                self.open_behind(id, &url);
                self.toast("Opened beside · still here");
            }
            (hint::Kind::Field, _) => self.click_target(id, rect, true),
            (_, None) => log::debug!("hint kind {kind:?} on a target without an address"),
        }
    }

    /// Open `url` beside card `from` without going there: the current card,
    /// the view and where the keys are stay as they were.
    fn open_behind(&mut self, from: NodeId, url: &str) {
        if self.scene.node(from).is_none() {
            return;
        }
        log::info!("opened a link hint from card {} beside it, staying: {url}", from.0);
        self.open_card(Rect::default(), url, Some(from));
    }

    /// A hint on a button, field or other control: click its middle, as the
    /// mouse would. A field (`typing`) gets the keys, so you can type in it at once.
    fn click_target(&mut self, id: NodeId, rect: Rect, typing: bool) {
        if typing {
            self.focus(Some(id));
            self.give_keys(PageKeys::Insert);
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
        // What was clicked is where the reader is: scroll there next.
        self.pick_section_at(id, x, y);
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
        self.scroll_section(id, web::scroll::Axis::Y, if down { 1 } else { -1 }, web::scroll::Unit::Half);
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
    /// Page mode's f: a web page loads the link itself; a note or one of our
    /// own pages never navigates, so the link opens beside it as usual.
    fn follow_here(&mut self, id: NodeId, url: &str) {
        match self.cards.get(&id).map(|c| &c.page) {
            Some(p) if p.kind() == web::PageKind::Web => {
                log::info!("followed a link hint in card {} to {url}", id.0);
                p.load(url);
            }
            _ => self.follow_link(id, url),
        }
    }

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
        let mut l = Layer::default();
        // Section labels (`s`) sit at their sections' top-left corners, each
        // section outlined; link labels at the middle of their link's start.
        let spots = if lh.sections.is_empty() {
            None
        } else {
            let rects: Vec<Rect> = lh.sections.iter().filter_map(|(_, r)| self.page_rect_to_screen(lh.id, *r)).collect();
            let longest = h.labels.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
            let label = Vec2::new(longest * size * 0.75 + pad * 2.0, bh);
            for (i, r) in rects.iter().enumerate() {
                if h.shown(i) {
                    crate::chrome::outline(&mut l, r.inflate(-scale), radius, (1.5 * ui).max(1.0), rgb(th.accent(), 0.55));
                }
            }
            (rects.len() == lh.sections.len()).then(|| scrolltarget::label_spots(&rects, label, view))
        };
        let count = if lh.sections.is_empty() { lh.links.len() } else { lh.sections.len() };
        for i in 0..count {
            if !h.shown(i) {
                continue;
            }
            let at = match &spots {
                Some(s) => Rect::new(s[i].x, s[i].y, 0.0, bh),
                None => {
                    let rect = if lh.sections.is_empty() { lh.links[i].0 } else { continue };
                    let Some(at) = self.page_rect_to_screen(lh.id, rect) else { continue };
                    at
                }
            };
            let label = h.labels[i].to_uppercase();
            let (head, tail): (String, String) = (label.chars().take(typed_n).collect(), label.chars().skip(typed_n).collect());
            items.push((at, head, tail, lh.chosen.contains(&i)));
        }
        let r = self.renderer.as_mut()?;
        for (at, head, tail, chosen) in items {
            // A label already chosen (`;m`) is quieter, but can be chosen again.
            let (bg, fg) = if chosen { (rgb(th.accent(), 0.45), rgb(th.accent_foreground(), 0.7)) } else { (bg, fg) };
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

    /// The brief outline around a newly chosen or scrolled section, fading.
    fn scroll_flash_layer(&mut self) -> Option<Layer> {
        let f = self.scroll_flash.as_ref()?;
        let age = f.at.elapsed();
        if age > SCROLL_FLASH || self.focused != Some(f.id) {
            return None;
        }
        let (view, _) = self.page_view(f.id)?;
        let at = self.page_rect_to_screen(f.id, f.rect)?;
        // Inside the page's view, so the page's own outline stays visible.
        let x0 = at.x.max(view.x);
        let y0 = at.y.max(view.y);
        let rect = Rect::new(x0, y0, at.right().min(view.right()) - x0, at.bottom().min(view.bottom()) - y0);
        if rect.w < 4.0 || rect.h < 4.0 {
            return None;
        }
        let t = age.as_secs_f32() / SCROLL_FLASH.as_secs_f32();
        let alpha = if self.still { 0.9 } else { 0.9 * (1.0 - t * t) };
        let ui = self.ui();
        let mut l = Layer::default();
        let radius = self.theme.small_radius(self.scale());
        crate::chrome::outline(&mut l, rect, radius, (2.0 * ui).max(1.5), rgb(self.theme.accent(), alpha));
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
        if !self.still {
            // Cards glide to their places rather than jumping.
            let now = Instant::now();
            for &(id, old) in &before {
                self.glides.start(id, old, false, now);
            }
        }
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
        self.ask_trail_name_of(id);
    }

    /// The name box for the trail through card `id` (from R, the sidebar or
    /// a click on the trail's tag), holding its current name.
    fn ask_trail_name_of(&mut self, id: NodeId) {
        self.menu = None;
        self.link_hints = None;
        let name = self.scene.trail_mark(id).name.unwrap_or_default();
        self.omnibox = Some(Omnibox::new(&name, Target::NameTrail(id)));
        log::info!("name box for the trail of card {}: {name:?}", id.0);
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
            log::info!("solo: card {}", id.0);
        }
        // From the board, the card grows out of its place (if it is on screen).
        if self.read.is_none() {
            self.read_morph = (self.card_on_screen(id) && !self.still).then(|| ReadMorph { id, entering: true, start: Instant::now() });
        }
        self.read = Some(id);
        self.selection.clear();
        self.menu = None;
        self.camera_goal = None;
        self.drag = Drag::None;
        self.focus(Some(id));
        self.keys_in_page = false;
        self.set_page_keys(PageKeys::Page);
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
        log::info!("solo: back to the board");
        self.menu = None;
        self.keys_in_page = false;
        self.camera_moved();
        self.ensure_visible(id);
        // Pages go straight back to the board's raster scale (where the camera
        // is heading), so they lay out once instead of at 1:1 and again once it rests.
        let zoom = self.camera_goal.map_or(self.camera.zoom, |g| g.zoom);
        self.page_scale = self.board_page_scale(zoom);
        // It shrinks back into its place (following the camera if that glides).
        self.read_morph = (!self.still).then(|| ReadMorph { id, entering: false, start: Instant::now() });
    }

    /// Whether any of card `id` is in the window.
    fn card_on_screen(&self, id: NodeId) -> bool {
        let vp = self.viewport();
        self.scene.node(id).is_some_and(|n| self.to_screen_rect(n.frame()).intersects(&Rect::new(0.0, 0.0, vp.x, vp.y)))
    }

    /// Glide just enough to bring card `id` on screen (fitting it with the focused card if possible).
    /// In Solo, bringing a card on screen means reading it.
    fn ensure_visible(&mut self, id: NodeId) {
        if self.read.is_some() {
            self.read_card(id);
            return;
        }
        let Some(frame) = self.scene.node(id).map(|n| n.frame()) else { return };
        let vp = self.viewport();
        // The open sidebar covers the left of the window: fit the card in the rest.
        let left = if self.sidebar_shown() { self.sidebar_rect().right() } else { 0.0 };
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
        // The scale first, so pages that wake below start at it rather than
        // at the old one and then lay out again.
        if settled {
            self.page_scale = self.settled_page_scale();
        }
        self.sync_liveness(settled && !matches!(self.drag, Drag::Pan { .. }));
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let page_scale = self.page_scale;
        let mut updates = Vec::new();
        let read_css = self.read_layout().css;
        for n in self.scene.nodes() {
            let css = self.card_css(n);
            let board = (css.x as i32, css.y as i32);
            let (on_screen, size) = match reading {
                Some(r) if r == n.id => (true, read_css),
                Some(_) => (false, board),
                None => (self.to_screen_rect(n.frame()).intersects(&screen), board),
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

    /// The raster scale for pages once the camera rests: the zoom's
    /// resolution in quarter steps (0.5–2), 1:1 in Solo.
    fn settled_page_scale(&self) -> f32 {
        if self.read.is_some() {
            return Self::page_scale_for(1.0, self.scale());
        }
        self.board_page_scale(self.camera.zoom)
    }

    /// The raster scale for pages on the board at camera `zoom`. Pages lay
    /// out at Solo's width (`card_css`), so a card of the default width
    /// shows `CARD_W / read width` world units per CSS px. One scale for all
    /// cards: Chromium shares page zoom (our raster scale) across a site.
    fn board_page_scale(&self, zoom: f32) -> f32 {
        let css = read::card_css(Vec2::new(scene::CARD_W, scene::CARD_H), self.read_layout().css.0);
        Self::page_scale_for(zoom * scene::CARD_W / css.x, self.scale())
    }

    /// The raster scale for camera `zoom` on a window of `scale`.
    fn page_scale_for(zoom: f32, scale: f32) -> f32 {
        ((zoom * scale) * 4.0).round().clamp(2.0, 8.0) / 4.0
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
                WebEvent::PermissionAsked { page, id, origin, kinds } => {
                    if self.node_of(page).is_some() {
                        self.permission_asked(id, origin, kinds);
                    } else {
                        web::permission::answer(id, false); // a page of another space
                    }
                }
                WebEvent::PermissionWithdrawn { id } => self.permission_withdrawn(id),
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
                    if let Some(id) = self.node_of(page) {
                        self.forget_sections(id);
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
                WebEvent::Focus { page, report } => {
                    if let Some(id) = self.node_of(page) {
                        self.page_focus(id, report);
                    }
                }
                WebEvent::Scroll { page, report } => {
                    if let Some(id) = self.node_of(page) {
                        self.got_scroll(id, report);
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
                WebEvent::AppAction { page, action } => {
                    if let Some(id) = self.node_of(page)
                        && (self.settings_page_action(id, &action)
                            || self.engine_page_action(id, &action))
                    {
                        continue;
                    }
                    let Some(action) = self.node_of(page).and(crate::welcome::action(&action)) else {
                        log::warn!("app page {page} asked for unknown action {action:?}");
                        continue;
                    };
                    log::info!("welcome page: {action:?}");
                    self.welcome_action(action);
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
                WebEvent::LoadingChanged { page, loading, can_back, can_forward } => {
                    let Some(id) = self.node_of(page) else { continue };
                    if let Some(c) = self.cards.get_mut(&id) {
                        c.loading = loading;
                        c.can_back = can_back;
                        c.can_forward = can_forward;
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
        // Trail tags stick out above their card the same way.
        let tag = self
            .tags
            .iter()
            .rfind(|(t, r)| r.contains(screen) && self.scene.node(*t).is_some_and(|n| top_z.is_none_or(|z| n.z >= z)));
        if let Some(&(id, _)) = tag {
            return Hit::Tag(id);
        }
        // A card's label sits above it, off the card: its buttons, then the
        // label itself (the card's handle). A card drawn above covers them.
        let shown = |id: NodeId| self.scene.node(id).is_some_and(|n| top_z.is_none_or(|z| n.z >= z));
        if let Some(&(hit, _)) = self.bar_buttons.iter().rfind(|(h, r)| r.contains(screen) && h.id().is_some_and(shown)) {
            return hit;
        }
        if let Some(&(id, _)) = self.shields.iter().rfind(|(id, r)| r.contains(screen) && shown(*id)) {
            return Hit::Shield(id);
        }
        if let Some(&(id, _)) = self.bars.iter().rfind(|(id, r)| r.contains(screen) && shown(*id)) {
            return Hit::Header(id);
        }
        let Some(id) = top else { return Hit::Board };
        let n = self.scene.node(id).expect("hit node exists");
        let k = self.camera.pixels_per_unit(self.scale());
        let grip = RESIZE_GRIP / k.min(1.0);
        if w.y < n.rect.y {
            // A sticky note's strip moves it.
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
        if state == ElementState::Pressed {
            self.board_pan.stop(); // a grab holds the board where it is
            self.pending.clear(); // a click ends a half-typed count
        }
        // A click outside the shortcut sheet closes it; none does anything else.
        if self.hints {
            if state == ElementState::Pressed && !self.sheet.panel.contains(p) {
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
                self.decline(confirm);
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
            let panel = self.list_panel(ob, &rows);
            let scroll = ob.scroll.min(panel.max_scroll());
            if let Some(i) = panel.row_at(p, scroll) {
                self.menu_pressed = true; // swallow the release
                // An allowed site is blocked again; the list stays.
                if let Item::AllowedSite(site) = &rows[i].item {
                    if let Some(ob) = self.omnibox.as_mut() {
                        ob.sel = i;
                    }
                    if button == MouseButton::Left {
                        self.block_site_again(site);
                    }
                    return;
                }
                self.omnibox = None;
                if button == MouseButton::Left {
                    self.run_palette(rows[i].clone(), self.mods.shift_key());
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
            if self.sidebar_rect().contains(p) {
                self.sidebar_click(p, button);
                self.menu_pressed = true; // swallow the release
                return;
            }
            self.sidebar_keys = false;
        }
        // The board's own controls take clicks before the board does: a
        // press runs one; a press between them does nothing.
        if self.read.is_none() && button == MouseButton::Left {
            if state == ElementState::Released && std::mem::take(&mut self.hud_down) {
                return;
            }
            if state == ElementState::Pressed {
                if let Some(&(_, go)) = self.nudge.iter().find(|(r, _)| r.contains(p)) {
                    self.hud_down = true;
                    self.omnibox = None;
                    match go {
                        crate::hud::Go::Command(cmd) => self.run_command(cmd),
                        crate::hud::Go::Palette => self.open_palette(),
                    }
                    return;
                }
                if let Some(c) = self.hud.at(p) {
                    self.hud_down = true;
                    self.omnibox = None;
                    self.hud_press(c);
                    return;
                }
                if self.hud.covers(p) {
                    self.hud_down = true;
                    return;
                }
            }
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
                Hit::Board if button == MouseButton::Right => {
                    // What can be done on the board, where the pointer is.
                    self.keys_in_page = false;
                    self.focus(None);
                    self.open_board_menu(p);
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
                Hit::Back(id) | Hit::Forward(id) | Hit::Reload(id) => {
                    self.keys_in_page = false;
                    self.focus(Some(id));
                    if let Some(c) = self.cards.get(&id) {
                        match self.hit(p) {
                            Hit::Back(_) => c.page.back(),
                            Hit::Forward(_) => c.page.forward(),
                            _ => c.page.reload(),
                        }
                    }
                    self.menu_pressed = true; // swallow the release
                }
                Hit::Shield(id) => {
                    self.keys_in_page = false;
                    self.focus(Some(id));
                    self.open_shield_menu(id);
                    self.menu_pressed = true;
                }
                Hit::Tag(id) => {
                    self.keys_in_page = false;
                    self.ask_trail_name_of(id);
                    self.menu_pressed = true; // swallow the release
                }
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
                    // Insert until the page says the click wasn't in a field
                    // (`WebEvent::Focus`), so typing right after a click lands.
                    self.give_keys(PageKeys::Insert);
                    if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_button(x, y, mods, web_button, false, self.click_count.min(3));
                        // The innermost section clicked in becomes the scroll target.
                        if web_button == WebButton::Left {
                            self.pick_section_at(id, x, y);
                        }
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
        let list = self.omnibox.as_ref().filter(|ob| ob.target.is_list()).map(|ob| self.list_panel(ob, &self.list_rows(ob)));
        if let (Some(panel), Some(ob)) = (list, self.omnibox.as_mut())
            && panel.contains(p)
        {
            if let Some(i) = panel.row_at(p, ob.scroll.min(panel.max_scroll())) {
                ob.sel = i;
            }
            self.cursor = p;
            self.set_cursor(web::Cursor::Default);
            return;
        }
        self.sidebar_hover = None;
        self.sidebar_spot = None;
        // Over one of the board's own controls: it lights up, and after a
        // moment says its name and key.
        let over = if matches!(self.drag, Drag::None) && self.read.is_none() && self.sidebar_press.is_none() && self.omnibox.as_ref().is_none_or(|ob| !ob.target.is_list()) {
            self.hud.at(p)
        } else {
            None
        };
        let on_chrome = over.is_some()
            || (matches!(self.drag, Drag::None) && self.read.is_none() && (self.hud.covers(p) || self.nudge.iter().any(|(r, _)| r.contains(p))));
        match (over, self.hud_hover) {
            (Some(c), Some((old, _))) if c == old => {}
            (Some(c), _) => self.hud_hover = Some((c, Instant::now())),
            (None, _) => self.hud_hover = None,
        }
        if on_chrome {
            self.hover = Hit::Board;
            self.cursor = p;
            let button = over.is_some() || self.nudge.iter().any(|(r, _)| r.contains(p));
            self.set_cursor(if button { web::Cursor::Pointer } else { web::Cursor::Default });
            return;
        }
        if self.sidebar_press.is_some() {
            self.sidebar_press_moved(p);
            self.cursor = p;
            self.set_cursor(web::Cursor::Default);
            return;
        }
        if self.sidebar_shown() && matches!(self.drag, Drag::None) && self.sidebar_rect().contains(p) {
            let (rows, panel) = self.sidebar_layout();
            let closable = |i: usize| matches!(rows.get(i), Some(scene::sidebar::Row::Card { .. }));
            self.sidebar_hover = panel.row_at(p, self.sidebar.scroll);
            self.sidebar_spot = panel.spot_at(p, self.spaces.spaces.len(), self.sidebar.scroll, &closable);
            self.cursor = p;
            // The head's controls, the close buttons and the foot are buttons.
            self.set_cursor(if self.sidebar_spot.is_some() { web::Cursor::Pointer } else { web::Cursor::Default });
            return;
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
                self.hover = hit;
                let cursor = match hit {
                    Hit::Header(_) => web::Cursor::Grab,
                    Hit::Grip(_) => web::Cursor::ResizeSE,
                    Hit::Close(_) | Hit::Back(_) | Hit::Forward(_) | Hit::Reload(_) | Hit::Badge(_) | Hit::Shield(_) | Hit::Tag(_) => web::Cursor::Pointer,
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
        // The shortcut sheet is in front of everything: the wheel scrolls it.
        if self.hints {
            let speed = self.settings.scroll_speed.factor();
            if lines {
                self.sheet.scroll_by(-dy * self.sheet.line * 2.0 * speed);
            } else {
                self.sheet.touch(dy, speed, matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled));
            }
            return;
        }
        self.camera_goal = None;
        self.menu = None;
        self.link_hints = None; // scrolling a page moves its links
        if let Some(ob) = self.omnibox.as_ref().filter(|ob| ob.target.is_list()) {
            let panel = self.list_panel(ob, &self.list_rows(ob));
            if panel.rect.contains(self.cursor) {
                let rows = if lines { -dy * 3.0 } else { -dy / panel.row_h };
                let ob = self.omnibox.as_mut().expect("palette open");
                ob.scroll = panel.scroll_by(ob.scroll, rows.round() as isize);
                return;
            }
        }
        if self.sidebar_shown() && self.sidebar_rect().contains(self.cursor) {
            let (_, panel) = self.sidebar_layout();
            let rows = if lines {
                -dy * 3.0
            } else {
                -dy / (crate::sidebar::ROW_H * self.ui())
            };
            self.sidebar.scroll = panel.scroll_by(self.sidebar.scroll, rows.round() as isize);
            return;
        }
        if self.mods.control_key() {
            if self.read.is_some() {
                return; // the board isn't on screen
            }
            let steps = if lines { dy } else { dy / 60.0 };
            self.camera.zoom_about(
                1.15f32.powf(steps),
                self.cursor,
                self.viewport(),
                self.scale(),
            );
            self.camera_moved();
            return;
        }
        if let Hit::Content(id) | Hit::Grip(id) = self.hit(self.cursor)
            && Some(id) == self.focused
        {
            if let (Some(card), Some((x, y))) =
                (self.cards.get(&id), self.page_point(id, self.cursor))
            {
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
                    card.page.mouse_wheel(
                        x,
                        y,
                        keymap::modifiers(self.mods),
                        now.x as i32,
                        now.y as i32,
                        !lines,
                    );
                }
            }
            self.board_pan.stop();
            return;
        }
        if self.read.is_some() {
            return;
        }
        // The board pans like a page scrolls: a notch glides (`step_board_pan`),
        // the touchpad follows the fingers and flings.
        let speed = self.settings.scroll_speed.factor();
        let d = Vec2::new(dx, dy);
        if lines {
            self.board_pan.wheel(d, speed);
        } else {
            let ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
            let now = self.board_pan.touch(d, speed, ended);
            if now != Vec2::default() {
                self.camera.pan_pixels(now, self.scale());
                self.camera_moved();
            }
        }
    }

    /// This frame's part of a wheel glide or touchpad fling on the board.
    /// Runs every frame, busy or not: the touchpad's fling speed is timed by it.
    fn step_board_pan(&mut self, dt: f32) {
        // The camera is going somewhere else, or the board is off screen.
        if self.board_pan.busy() && (self.camera_goal.is_some() || self.read.is_some()) {
            self.board_pan.stop();
        }
        let d = self.board_pan.step(dt);
        if d != Vec2::default() {
            self.camera.pan_pixels(d, self.scale());
            self.camera_moved();
        }
    }

    /// Shows the shortcut sheet, from the top with no filter.
    fn show_hints(&mut self) {
        self.hints = true;
        self.sheet = Default::default();
        log::info!("hints shown");
    }

    /// A key while the shortcut sheet shows: typing filters it, the arrows,
    /// Page keys and Home/End scroll it, Esc clears the filter and then
    /// closes it, as do ? and F1. Other keys do nothing.
    fn sheet_key(&mut self, ev: &KeyIn, chord: bool) {
        let sh = &mut self.sheet;
        let close = |app: &mut Self| {
            app.hints = false;
            log::info!("hints hidden");
        };
        match ev.named {
            Some(NamedKey::Escape) if !sh.filter.is_empty() => sh.clear_filter(),
            Some(NamedKey::Escape | NamedKey::F1) => close(self),
            Some(NamedKey::ArrowDown) => sh.scroll_by(sh.line),
            Some(NamedKey::ArrowUp) => sh.scroll_by(-sh.line),
            Some(NamedKey::PageDown) => sh.scroll_by(sh.page),
            Some(NamedKey::PageUp) => sh.scroll_by(-sh.page),
            Some(NamedKey::Home) => sh.scroll_to(0.0),
            Some(NamedKey::End) => sh.scroll_to(sh.max),
            Some(NamedKey::Backspace) if chord => sh.clear_filter(),
            Some(NamedKey::Backspace) => sh.backspace(),
            _ if chord => {}
            // ? closes the sheet it opened, unless it is being typed into a filter.
            _ if ev.text.as_deref() == Some("?") && sh.filter.is_empty() => close(self),
            _ => {
                if let Some(t) = ev.text.as_deref().filter(|t| !t.trim().is_empty() || !sh.filter.is_empty()) {
                    sh.type_text(t);
                }
            }
        }
    }

    fn key(&mut self, ev: KeyIn) {
        let pressed = ev.pressed;
        let ctrl = self.mods.control_key();
        let shift = self.mods.shift_key();
        let alt = self.mods.alt_key();
        // Shortcuts follow the logical key (layout-aware), like browsers do.
        let code = ev.code;

        let is_question = ev.text.as_deref() == Some("?") || (shift && code == Some(KeyCode::Slash));
        // A half-typed count or sequence ends when something else takes the keys.
        if pressed
            && (self.hints
                || self.confirm.is_some()
                || self.link_hints.is_some()
                || self.arrange.is_some()
                || self.menu.is_some()
                || self.omnibox.is_some()
                || self.typing())
        {
            self.pending.clear();
        }
        if self.hints {
            if pressed {
                self.sheet_key(&ev, ctrl || alt);
            }
            return;
        }
        if let Some(confirm) = self.confirm {
            // Enter deletes the space; any other key (but a modifier) keeps it and is dropped.
            if pressed && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)) {
                self.confirm = None;
                match (ev.named, confirm) {
                    (Some(NamedKey::Enter), Confirm::DeleteSpace) => self.delete_space(),
                    (Some(NamedKey::Enter), Confirm::CloseOthers(keep)) => self.close_others(keep),
                    (Some(NamedKey::Enter), Confirm::ClearHistory) => self.clear_history(),
                    (Some(NamedKey::Enter), Confirm::ClearCookies) => self.clear_cookies(),
                    (Some(NamedKey::Enter), Confirm::Permission) => self.answer_ask(true),
                    _ => self.decline(confirm),
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
            self.pending.clear();
            self.show_hints();
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
        let palette_len = self
            .omnibox
            .as_ref()
            .filter(|ob| ob.target.is_list())
            .map(|ob| self.list_panel(ob, &self.list_rows(ob)));
        // The allowed-sites list's highlighted site.
        let setting_sel = self
            .omnibox
            .as_ref()
            .filter(|ob| ob.target == Target::AllowedSites)
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
            let step = palette_len.as_ref().and_then(|panel| match ev.named {
                Some(NamedKey::ArrowDown) => Some(1),
                Some(NamedKey::ArrowUp) => Some(-1),
                Some(NamedKey::PageDown) => Some(panel.fits(ob.scroll) as isize),
                Some(NamedKey::PageUp) => Some(-(panel.fits(ob.scroll) as isize)),
                _ if ctrl && matches!(code, Some(KeyCode::KeyJ | KeyCode::KeyN)) => Some(1),
                _ if ctrl && code == Some(KeyCode::KeyP) => Some(-1),
                _ => None,
            });
            if let (Some(step), Some(panel)) = (step, palette_len) {
                let len = panel.len();
                if len > 0 {
                    ob.sel = (ob.sel as isize + step).clamp(0, len as isize - 1) as usize;
                    ob.scroll = panel.keep_visible(ob.sel, ob.scroll);
                }
                return;
            }
            match ev.named {
                Some(NamedKey::Escape) => self.omnibox = None,
                // Command line: Tab puts the highlighted row's line in the field.
                Some(NamedKey::Tab) if ob.target == Target::Palette && palette::is_command(&ob.text) => {
                    let ob = self.omnibox.take().expect("palette open");
                    let row = self.list_rows(&ob).into_iter().nth(ob.sel).map(|r| r.item);
                    match row {
                        Some(Item::Run(line) | Item::Recent(line) | Item::Complete(line)) => self.fill_command_line(&line),
                        _ => self.omnibox = Some(ob),
                    }
                }
                _ if ctrl && code == Some(KeyCode::KeyK) && ob.target == Target::Palette => self.omnibox = None,
                _ if ctrl && code == Some(KeyCode::KeyF) && matches!(ob.target, Target::Find(_)) => ob.select_all = true,
                Some(NamedKey::Enter) if matches!(ob.target, Target::Find(_)) => {
                    if let Target::Find(id) = ob.target
                        && let Some(c) = self.cards.get(&id)
                    {
                        c.page.find(&ob.text, !shift, true);
                    }
                }
                _ if ctrl && code == Some(KeyCode::KeyH) && ob.target == Target::History => {
                    self.omnibox = None
                }
                Some(NamedKey::Delete) if shift && ob.target == Target::Downloads => {
                    self.drop_selected_download()
                }
                // Allowed sites: Enter or Shift+Delete blocks ads there again; the list stays.
                Some(NamedKey::Enter | NamedKey::Delete) if ob.target == Target::AllowedSites => {
                    if (ev.named == Some(NamedKey::Enter) || shift)
                        && let Some(Item::AllowedSite(site)) = &setting_sel
                    {
                        self.block_site_again(site);
                    }
                }
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
                        self.run_palette(row, shift);
                    }
                }
                Some(NamedKey::Enter) => {
                    let ob = self.omnibox.take().expect("omnibox open");
                    if ob.target == Target::RenameSpace {
                        self.rename_space(&ob.text);
                    } else if let Target::SaveAs(id) = ob.target {
                        self.save_download_as(id, &ob.text);
                    } else if let Target::NameTrail(id) = ob.target {
                        self.name_trail(id, &ob.text);
                    } else if let Some(url) = self.settings.resolve_url(&ob.text) {
                        match ob.target {
                            // Shift+Enter: beside the current card instead.
                            _ if shift => self.open_beside_current(&url),
                            Target::Beside => self.open_beside_current(&url),
                            Target::Focused if self.focused_page().is_some() => self.load_here(&url),
                            _ => {
                                // Typed an address: the new page is what you use next.
                                if self.new_card_at_center(&url).is_some() {
                                    self.give_keys(PageKeys::Page);
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

        // Board keys (`scene::keys`), while no card is entered. A key that
        // isn't one goes on to the app's shortcuts below.
        if pressed
            && !self.entered()
            && let Some(chord) = ev.chord(self.mods)
        {
            match self.pending.feed(&self.board_keys, chord, Instant::now(), true) {
                Outcome::Run { action, count } => return self.board_action(action, count),
                Outcome::Wait | Outcome::Cancelled | Outcome::Unbound { swallow: true } => return,
                Outcome::Unbound { swallow: false } => {}
            }
        }
        // Page mode: our page keys first. A character that isn't one does
        // nothing; other keys (Enter, Tab, arrows, Space, shortcuts) go on to
        // the app's shortcuts and then to the page.
        let mut to_page = self.typing();
        if self.page_mode() {
            if pressed {
                let chord = ev.chord(self.mods);
                if let Some(chord) = chord {
                    match self.pending.feed(&self.page_keys_table, chord, Instant::now(), true) {
                        Outcome::Run { action, count } => return self.page_action(action, count),
                        Outcome::Wait | Outcome::Cancelled | Outcome::Unbound { swallow: true } => return,
                        Outcome::Unbound { swallow: false } => {}
                    }
                }
                if chord.is_some_and(|c| c.is_bare() && matches!(c.key, keys::Key::Char(_))) {
                    return;
                }
                to_page = true;
                self.forwarded.insert(ev.native);
            } else {
                to_page = self.forwarded.remove(&ev.native);
            }
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
                    self.open_address_bar();
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
                Some(KeyCode::KeyT) if ctrl && shift => {
                    self.reopen_closed();
                    true
                }
                Some(KeyCode::KeyT) if ctrl => {
                    self.new_card();
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
                Some(KeyCode::Tab) if !ctrl && !alt && !shift && !self.typing() => {
                    self.toggle_read();
                    true
                }
                // Passthrough: Shift+Esc gives our keys back; a plain Esc is the page's.
                Some(KeyCode::Escape) if self.typing() && self.page_keys == PageKeys::Passthrough && shift => {
                    self.set_page_keys(PageKeys::Page);
                    true
                }
                Some(KeyCode::Escape) if self.typing() && self.page_keys == PageKeys::Passthrough => false,
                // Insert: the page sees Esc (closes its menus), the field lets
                // go of the caret, and our page keys are back.
                Some(KeyCode::Escape) if self.typing() && self.drag_is_idle() => {
                    if let Some(p) = self.focused_page() {
                        p.send_key(&ev.to_web(self.mods));
                        p.blur_field();
                    }
                    self.set_page_keys(PageKeys::Page);
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
                Some(KeyCode::Escape) if self.page_mode() && self.drag_is_idle() => {
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
                _ => false,
            };
            if handled {
                return;
            }
        }
        if to_page
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
            self.page_css.clear();
            for (id, card) in &self.cards {
                let f = card.page.frame();
                r.update_page(page_key(epoch, *id), f.width, f.height, &f.pixels, f.serial);
                self.page_css.insert(*id, f.css);
            }
            for s in self.icons.values() {
                r.update_page(s.key, s.icon.width, s.icon.height, &s.icon.pixels, s.serial);
            }
        }
        self.load_sleeping_thumbs();
        if self.read_morph.is_some_and(|m| m.done() || self.scene.node(m.id).is_none()) {
            self.read_morph = None;
        }
        // While a card grows into Solo the board still shows behind it.
        let board_hidden = self.read.is_some() && self.read_morph.is_none();
        let preview_url = self
            .omnibox
            .as_ref()
            .filter(|ob| ob.target.is_list())
            .and_then(|ob| self.list_rows(ob).get(ob.sel).and_then(|row| self.preview_url(row)));
        let preview = self.load_preview(preview_url);

        let origin = self.camera.world_to_screen(Vec2::default(), vp, scale);
        let th = self.theme.clone();
        let ui = self.ui();
        let tint = self.spaces.active_entry().tint.clone();
        // The grid keeps its dots 20 to 40 px apart at any zoom. Zooming in,
        // dots fade in halfway between them, and at 40 px those become the
        // grid: its density never jumps.
        let base = (32.0 * k).max(1e-3);
        let spacing = base * 2f32.powf((20.0 * scale / base).log2().ceil());
        // Behind the cards: the space's photo once it is read, else its
        // colour mood (a soft gradient with grain), else its flat colour.
        let photo = self.board_photo();
        let update = self.photo.update(photo.as_deref(), (vp.x.max(0.0) as u32, vp.y.max(0.0) as u32));
        if let (Some(t), Some(r)) = (update.upload, self.renderer.as_mut()) {
            r.update_page(crate::backdrop::TEXTURE, t.width, t.height, &t.pixels, self.photo.serial);
        }
        if let Some((p, e)) = update.failed {
            let name = p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().to_string());
            self.toast(format!("Can't show {name} as the background: {e}"));
        }
        let photo_on = photo.as_deref().is_some_and(|p| self.photo.ready(p)) && !board_hidden;
        let (near, far) = self.backdrop_colours();
        let (grain, dots) = match self.settings.backdrop {
            prefs::Backdrop::Plain => (0.0, 1.0),
            _ => (1.5, if photo_on { 0.0 } else { 0.7 }),
        };
        // Just switched space: the colours cross-fade from the old space's.
        let now = Instant::now();
        let space_t = match self.space_in {
            Some((at, ..)) if motion::progress(at, motion::SPACE, now) < 1.0 => Some(motion::ease_out(motion::progress(at, motion::SPACE, now))),
            _ => {
                self.space_in = None;
                None
            }
        };
        let mix = |old: [f32; 4], new: [f32; 4]| match space_t {
            Some(t) => std::array::from_fn(|i| old[i] + (new[i] - old[i]) * t),
            None => new,
        };
        let (old_near, old_far) = self.space_in.map_or((rgb(near, 1.0), rgb(far, 1.0)), |s| (s.2, s.3));
        let bg = Background {
            color: mix(old_near, rgb(near, 1.0)),
            color2: mix(old_far, rgb(far, 1.0)),
            grain,
            dot: rgb(th.dots(), dots),
            grid_origin: [origin.x, origin.y],
            grid_spacing: spacing,
            dot_radius: (1.0 * scale).max(0.75),
            fine: ((spacing / (20.0 * scale)) - 1.0).clamp(0.0, 1.0),
        };

        let mut layers: Vec<Layer> = Vec::new();
        if photo_on {
            // The photo fills the window and stays put while the board moves,
            // a little dimmed so cards and labels stand off it.
            let dim = if th.light() { 0.94 } else { 0.78 };
            let mut l = Layer::default();
            l.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(0.0, 0.0, vp.x, vp.y), radius: 0.0 },
                color: [dim, dim, dim, 1.0],
                texture: Some(crate::backdrop::TEXTURE),
            });
            layers.push(l);
        }
        let line = crate::chrome::hair(scale);
        // What the pointer is over lights up, unless something else has the pointer.
        let hover = if matches!(self.drag, Drag::None) && self.menu.is_none() && self.omnibox.is_none() && !self.hints && self.pointer_in {
            self.hover
        } else {
            Hit::Board
        };

        // Trail edges under everything: a gentle S-curve from the source's
        // title bar to the target's. A trail is drawn in its own colour, or a
        // quiet neutral if it has none; the current card's trail is brought
        // out, so its path can be followed through a busy board.
        // Everything from here to the cards' last layer is the board, which
        // slides in on a space switch.
        let board_from = layers.len();
        // A card on its way somewhere is drawn where it is now (`glides`).
        let glides = &self.glides;
        let shown_frame = |n: &Node| match glides.at(n.id, n.rect, now) {
            Some((r, _)) => Rect::new(r.x, r.y - n.header_h(), r.w, r.h + n.header_h()),
            None => n.frame(),
        };
        let mut edges = Layer::default();
        let labels: HashMap<NodeId, scene::TrailMark> =
            if board_hidden { HashMap::new() } else { self.scene.trail_labels().into_iter().collect() };
        let mut edge_colour: HashMap<NodeId, [u8; 3]> = HashMap::new();
        for (head, mark) in &labels {
            if let Some(c) = mark.colour.as_deref() {
                let c = th.color(c);
                edge_colour.extend(self.scene.trail_of(*head).into_iter().map(|id| (id, c)));
            }
        }
        let current_trail: HashSet<NodeId> = self.focused.map(|f| self.scene.trail_of(f).into_iter().collect()).unwrap_or_default();
        let board_edges: Vec<Edge> = if board_hidden { Vec::new() } else { self.scene.visible_edges().copied().collect() };
        for e in &board_edges {
            let (Some(a), Some(b)) = (self.scene.node(e.from), self.scene.node(e.to)) else { continue };
            let fa = self.to_screen_rect(shown_frame(a));
            let fb = self.to_screen_rect(shown_frame(b));
            // Near the top of each card, where its name is.
            let (ya, yb) = (fa.y + (28.0 * k).min(fa.h / 2.0), fb.y + (28.0 * k).min(fb.h / 2.0));
            let (p0, p3) = if fb.x >= fa.right() { ([fa.right(), ya], [fb.x, yb]) } else { ([fa.x, ya], [fb.right(), yb]) };
            let dx = (p3[0] - p0[0]) * 0.5;
            let (p1, p2) = ([p0[0] + dx, p0[1]], [p3[0] - dx, p3[1]]);
            let hot = current_trail.contains(&e.from) && current_trail.contains(&e.to);
            let marked = edge_colour.get(&e.from).copied();
            let (colour, alpha) = match (marked, hot) {
                (Some(c), true) => (c, 1.0),
                (Some(c), false) => (c, 0.7),
                (None, true) => (th.accent(), 0.9),
                (None, false) => (th.line_strong(), 1.0),
            };
            let width = ((if hot { 2.0 } else { 1.5 }) * k).clamp(scale.max(1.0), 3.0 * scale);
            let mut prev = p0;
            for i in 1..=28 {
                let t = i as f32 / 28.0;
                let u = 1.0 - t;
                let x = u * u * u * p0[0] + 3.0 * u * u * t * p1[0] + 3.0 * u * t * t * p2[0] + t * t * t * p3[0];
                let y = u * u * u * p0[1] + 3.0 * u * u * t * p1[1] + 3.0 * u * t * t * p2[1] + t * t * t * p3[1];
                edges.quads.push(Quad { shape: Shape::Segment { from: prev, to: [x, y], width }, color: rgb(colour, alpha), texture: None });
                prev = [x, y];
            }
            // A dot where the line leaves a card and where it arrives.
            let d = (7.0 * k).clamp(3.0 * scale, 8.0 * scale);
            for p in [p0, p3] {
                crate::chrome::fill(&mut edges, Rect::new(p[0] - d / 2.0, p[1] - d / 2.0, d, d), d / 2.0, rgb(colour, 1.0));
            }
        }
        layers.push(edges);

        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let band = self.band();
        let in_band = band.map(|b| selection::in_band(&self.scene, b)).unwrap_or_default();
        let typing = self.entered();
        let board_cards = if board_hidden { Vec::new() } else { self.scene.draw_order() };
        let morphing = self.read_morph.map(|m| m.id);
        self.shields.clear();
        self.tags.clear();
        self.bars.clear();
        self.bar_buttons.clear();
        let mut in_view = false;
        let now = Instant::now();
        let crowd = k / scale < LABELS_ALL_BELOW;
        for n in board_cards {
            // The card on its way to or from Solo is drawn by `read_morph_layer`.
            if morphing == Some(n.id) {
                in_view = true;
                continue;
            }
            // Gliding or growing in: drawn where it is on the way, fading in if new.
            let glide = self.glides.at(n.id, n.rect, now);
            let moved;
            let n = match glide {
                Some((r, _)) => {
                    moved = Node { rect: r, ..n.clone() };
                    &moved
                }
                None => n,
            };
            let frame = self.to_screen_rect(n.frame());
            in_view |= frame.intersects(&screen);
            if !frame.inflate(64.0 * scale).intersects(&screen) {
                continue;
            }
            let content = self.to_screen_rect(n.rect);
            // Cards are sheets: the corner radius in use, scaled with the
            // zoom but kept visible when zoomed far out.
            let radius = th.card_corner(k, scale, frame.w, frame.h);
            let focused = self.focused == Some(n.id);
            let hovered = hover.id() == Some(n.id);
            let selected = self.selection.contains(n.id) || in_band.contains(&n.id);
            let card = self.cards.get(&n.id);
            let asleep = card.is_none();
            let sticky = n.sticky.as_deref().filter(|_| n.is_sticky());
            let mut l = Layer::default();

            // A stack shows the edges of up to three cards behind its front, down and to the right.
            let stack = self.scene.stack_of(n.id).map(|s| (s.front_index() + 1, s.members.len()));
            if let Some((_, len)) = stack {
                // At least a few px apart, so a stack still reads when zoomed far out.
                let step = (12.0 * k).max(5.0 * scale);
                let behind = len.min(4) - 1;
                let last = frame.translate(Vec2::new(step * behind as f32, step * behind as f32));
                l.quads.push(Quad {
                    shape: Shape::Shadow { rect: last.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (20.0 * k).max(2.0) },
                    color: th.shadow(0.24),
                    texture: None,
                });
                for i in (1..=behind).rev() {
                    let back = frame.translate(Vec2::new(step * i as f32, step * i as f32));
                    crate::chrome::fill(&mut l, back, radius, rgb(th.card(false), 1.0));
                    crate::chrome::outline(&mut l, back, radius, line, rgb(th.line_strong(), 1.0));
                }
            }
            // One light from above: a wide, soft shadow around the card and a
            // tight one under its edge. The current card floats a little
            // higher, the one under the pointer a touch. Both keep some size
            // when zoomed out, so cards still float over the board.
            let (ambient, key) = if focused {
                ((12.0, 44.0, 0.40), (2.0, 6.0, 0.30))
            } else if hovered {
                ((9.0, 34.0, 0.32), (1.5, 4.0, 0.26))
            } else {
                ((6.0, 26.0, 0.26), (1.0, 3.0, 0.22))
            };
            for (dy, blur, a) in [ambient, key] {
                let lo = 0.35 * scale;
                l.quads.push(Quad {
                    shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, (dy * k).max(dy * lo))), radius, blur: (blur * k).max(blur * lo).max(1.5) },
                    color: th.shadow(a),
                    texture: None,
                });
            }
            // A sticky note's frame is its strip, in its colour.
            crate::chrome::fill(&mut l, frame, radius, rgb(sticky.map_or_else(|| th.card(focused), |c| th.sticky_strip(c, focused)), 1.0));
            let key = page_key(epoch, n.id);
            let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
            // A live page's last frame at its own size: right after a resize
            // (leaving Solo, resizing the card) it is cropped or filled
            // out, never squashed, until the page paints at the new size.
            let extent = match (card, self.page_css.get(&n.id)) {
                (Some(_), Some(&css)) => read::extent(self.card_css(n), css),
                _ => [1.0, 1.0],
            };
            l.quads.push(Quad {
                // The page is the card: clipped to its rounded shape. (A
                // sticky note's page sits under its strip: rounded below only.)
                shape: Shape::Page { rect: content, radius, round_bottom: sticky.is_some(), extent },
                color: match (has_tex, asleep) {
                    (true, false) => [1.0; 4],
                    (true, true) => [0.72, 0.72, 0.74, 1.0], // a sleeping card's last frame, dimmed
                    // First paint pending, or asleep and never painted.
                    (false, _) => rgb(sticky.map_or_else(|| th.placeholder(), |c| th.sticky_paper(c)), 1.0),
                },
                texture: has_tex.then_some(key),
            });
            if asleep && !has_tex && sticky.is_none() {
                // Never painted: the site's icon and the title, centred, so
                // the card still says what it is.
                let s = (32.0 * k).clamp(12.0 * scale, 32.0 * scale);
                // Readable when the card has room for it, else left out.
                let size = (14.0 * k).max(11.0 * scale).min(14.0 * scale);
                let size = if content.w >= 120.0 * scale && content.h >= 60.0 * scale { size } else { 0.0 };
                let gap = (10.0 * k).max(6.0 * scale);
                let block = s + if size >= 9.0 { gap + size } else { 0.0 };
                let top = content.center().y - block / 2.0;
                if let Some(key) = self.icon_key(&n.url) {
                    l.quads.push(Quad {
                        shape: Shape::Rect { rect: Rect::new((content.center().x - s / 2.0).round(), top.round(), s, s), radius: 0.0 },
                        color: [0.85, 0.85, 0.87, 1.0],
                        texture: Some(key),
                    });
                } else if let Some(r) = self.renderer.as_mut() {
                    crate::icons::draw(&mut l, r, crate::icons::Icon::Moon, content.center().x, top + s / 2.0, s * 0.7, rgb(th.text3(), 1.0));
                }
                if size >= 9.0 {
                    let title = if n.title.trim().is_empty() { palette::host(&n.url).to_string() } else { n.title.trim().to_string() };
                    let room = content.w - 24.0 * k;
                    let w = self.renderer.as_mut().map_or(room, |r| r.measure(&title, size, false)).min(room);
                    let clip = Rect::new(content.center().x - w / 2.0, top + s + gap - size, w, size * 2.0);
                    crate::chrome::text(&mut l, title, clip.x, top + s + gap + size / 2.0, size, rgb(th.text2(), 1.0), clip, false);
                }
            }
            if sticky.is_none() {
                // The hairline that keeps a card crisp against the board, over
                // the page's own edge too (a white page on a light board);
                // a step stronger under the pointer.
                let edge = if hovered && !focused { th.line_strong() } else { th.line() };
                crate::chrome::outline(&mut l, frame, radius, line, rgb(edge, 1.0));
            }
            // Loading: a bar gliding along the card's top edge.
            if sticky.is_none() && card.is_some_and(|c| c.loading) {
                let t = (self.started.elapsed().as_secs_f32() * 0.7).fract();
                // Eased, so it gathers and lets go rather than sliding at one speed.
                let t = t * t * (3.0 - 2.0 * t);
                let w = frame.w * 0.3;
                let x = frame.x - w + (frame.w + w) * t;
                let (x0, x1) = (x.max(frame.x + radius), (x + w).min(frame.right() - radius));
                if x1 > x0 {
                    crate::chrome::fill(&mut l, Rect::new(x0, frame.y, x1 - x0, 2.0 * scale.max(1.0)), 0.0, rgb(th.accent(), 1.0));
                }
            }
            // The corner that resizes shows itself under the pointer.
            if hover == Hit::Grip(n.id) {
                let s = 14.0 * scale;
                let at = Rect::new(content.right() - s - 3.0 * scale, content.bottom() - s - 3.0 * scale, s, s);
                crate::chrome::fill(&mut l, at.inflate(2.0 * scale), th.small_radius(scale), rgb(th.panel(), 0.92));
                if let Some(r) = self.renderer.as_mut() {
                    crate::icons::draw_scaled(&mut l, r, crate::icons::Icon::Grip, at, rgb(th.text(), 1.0));
                }
            }
            // Rings sit outside the frame: the selection's, then the current
            // card's. The page with the keys (or a card being moved by
            // keyboard) gets a solid ring; a card that is only current a
            // thin, calm one.
            let arranging = self.arrange.as_ref().is_some_and(|a| a.id == n.id);
            let ring_w = if !focused { 0.0 } else if typing || arranging { 2.5 * scale.max(1.0) } else { 1.5 * scale.max(1.0) };
            if focused {
                let alpha = if typing || arranging { 1.0 } else { 0.8 };
                crate::chrome::outline(&mut l, frame.inflate(ring_w), if radius > 0.0 { radius + ring_w } else { 0.0 }, ring_w, rgb(th.accent(), alpha));
            }
            if selected {
                let w = 2.0 * scale.max(1.0);
                let m = ring_w + w + scale.max(1.0);
                crate::chrome::outline(&mut l, frame.inflate(m), if radius > 0.0 { radius + m } else { 0.0 }, w, rgb(th.marked(), 1.0));
            }

            // The label above the card, left to right: a stack's size, a
            // marked trail's tag, the site's icon and the title; at its right
            // end, on the card under the pointer or the current one, what the
            // blocker stopped and the buttons.
            let bar_h = BAR_H * ui;
            let bar = Rect::new(frame.x, frame.y - BAR_GAP * ui - bar_h, frame.w, bar_h);
            let mid = bar.center().y;
            let small = th.small_radius(ui / th.text_scale);
            let mut lead = 0.0;
            // A stack's size on its top-left corner, readable at any zoom.
            if let (Some((_, len)), Some(pill)) = (stack, self.stack_badge(n)) {
                let size = 11.0 * ui;
                let text = len.to_string();
                let tw = self.renderer.as_mut().map_or(size * 0.6, |r| r.measure(&text, size, true));
                let over = hover == Hit::Badge(n.id);
                crate::chrome::fill(&mut l, pill, if th.rounding > 0.0 { pill.h / 2.0 } else { 0.0 }, rgb(if over { th.accent() } else { th.text() }, 1.0));
                crate::chrome::text(&mut l, text, pill.x + (pill.w - tw) / 2.0, pill.center().y, size, rgb(if over { th.accent_foreground() } else { th.panel() }, 1.0), pill, true);
                lead = pill.right() + 6.0 * ui - frame.x;
            }
            // A marked trail's name before its first card's own, in the
            // trail's colour (just a dot for a colour without a name).
            if let Some(mark) = labels.get(&n.id) {
                let size = 11.0 * ui;
                let pad = 8.0 * ui;
                let h = 20.0 * ui;
                let colour = th.trail(mark.colour.as_deref());
                let name = mark.name.clone().unwrap_or_default();
                let tw = if name.is_empty() { 0.0 } else { self.renderer.as_mut().map_or(100.0, |r| r.measure(&name, size, true)) };
                let w = if name.is_empty() { h } else { (tw + 2.0 * pad).min((frame.w - lead).max(h)) };
                let tag = Rect::new(frame.x + lead, (mid - h / 2.0).round(), w, h);
                let round = if th.rounding > 0.0 || name.is_empty() { h / 2.0 } else { 0.0 };
                let face = if hover == Hit::Tag(n.id) { crate::theme::blend(colour, text_on(colour), 0.12) } else { colour };
                crate::chrome::fill(&mut l, if name.is_empty() { tag.inflate(-4.0 * ui) } else { tag }, round, rgb(face, 1.0));
                self.tags.push((n.id, tag));
                if !name.is_empty() {
                    crate::chrome::text(&mut l, name, tag.x + pad, tag.center().y, size, rgb(text_on(colour), 1.0), tag.inflate(-pad * 0.5), true);
                }
                lead += w + 8.0 * ui;
            }
            // Quiet labels: the title shows on the current card, the one
            // under the pointer, selected ones and stacks, and on every card
            // when zoomed far out; it fades in and out. The label's place
            // stays the card's handle either way.
            let label_on = focused || hovered || selected || stack.is_some() || crowd;
            let label_alpha = self.label_fades.alpha(n.id.0, label_on, now);
            let label_from = (l.quads.len(), l.texts.len());
            if sticky.is_none() && frame.w - lead >= 40.0 * ui {
                self.bars.push((n.id, bar));
                let mut right = bar.right();
                let controls = hovered || focused;
                if controls && frame.w >= BAR_CLOSE_MIN * ui {
                    // Close always; back, forward and reload on a live web
                    // page whose card is wide enough on screen.
                    let btn = BAR_BTN * ui;
                    let mut buttons: Vec<(Hit, crate::icons::Icon, bool)> = vec![(Hit::Close(n.id), crate::icons::Icon::Close, true)];
                    if let Some(c) = card.filter(|c| c.page.kind() == web::PageKind::Web && frame.w >= BAR_NAV_MIN * ui) {
                        buttons.push((Hit::Reload(n.id), crate::icons::Icon::Reload, true));
                        buttons.push((Hit::Forward(n.id), crate::icons::Icon::Forward, c.can_forward));
                        buttons.push((Hit::Back(n.id), crate::icons::Icon::Back, c.can_back));
                    }
                    for (hit, icon, enabled) in buttons {
                        let cell = Rect::new(right - btn, bar.y, btn, bar.h);
                        right -= btn;
                        let hot = hover == hit && enabled;
                        if hot {
                            let face = if hit == Hit::Close(n.id) { crate::theme::blend(th.panel(), th.danger(), 0.32) } else { th.active() };
                            crate::chrome::fill(&mut l, cell.inflate(-ui), small, rgb(face, 1.0));
                        }
                        let ink = match (enabled, hot) {
                            (false, _) => rgb(th.text3(), 0.55),
                            (true, true) => rgb(th.text(), 1.0),
                            (true, false) => rgb(th.text2(), 1.0),
                        };
                        if enabled {
                            self.bar_buttons.push((hit, cell));
                        }
                        if let Some(r) = self.renderer.as_mut() {
                            crate::icons::draw(&mut l, r, icon, cell.center().x, mid, 14.0 * ui, ink);
                        }
                    }
                    right -= 4.0 * ui;
                    // What the blocker stopped on this page (requests blocked
                    // and elements hidden): a shield with the count. Not on
                    // an allowed site.
                    let badge = self
                        .blocking_state(n.id)
                        .filter(|b| b.on)
                        .and_then(|b| scene::privacy::blocked_badge(b.blocked.saturating_add(b.hidden)));
                    if let Some(count) = badge {
                        let size = 11.0 * ui;
                        let tw = self.renderer.as_mut().map_or(size * 1.2, |r| r.measure(&count, size, true));
                        let s = 13.0 * ui;
                        let w = 5.0 * ui + s + 3.0 * ui + tw + 5.0 * ui;
                        // Only while the title keeps some room.
                        if right - w - (frame.x + lead) >= 96.0 * ui {
                            let cell = Rect::new(right - w, bar.y, w, bar.h);
                            self.shields.push((n.id, cell));
                            let over = hover == Hit::Shield(n.id);
                            if over {
                                crate::chrome::fill(&mut l, cell.inflate(-ui), small, rgb(th.active(), 1.0));
                            }
                            let ink = rgb(if over { th.text() } else { th.text2() }, 1.0);
                            if let Some(r) = self.renderer.as_mut() {
                                crate::icons::draw(&mut l, r, crate::icons::Icon::Shield, cell.x + 5.0 * ui + s / 2.0, mid, s, ink);
                            }
                            crate::chrome::text(&mut l, count, cell.x + 5.0 * ui + s + 3.0 * ui, mid, size, ink, cell, true);
                            right = cell.x - 4.0 * ui;
                        }
                    }
                } else if asleep && frame.w - lead >= 72.0 * ui {
                    // A sleeping card says so with a moon at the end of its label.
                    if let Some(r) = self.renderer.as_mut() {
                        crate::icons::draw(&mut l, r, crate::icons::Icon::Moon, right - 8.0 * ui, mid, 13.0 * ui, rgb(th.text3(), 1.0));
                    }
                    right -= 22.0 * ui;
                }
                let mut x = frame.x + lead;
                let label_x0 = x;
                let size = 12.0 * ui;
                let icon = self.icon_key(&n.url);
                if let Some(key) = icon {
                    let s = 14.0 * ui;
                    l.quads.push(Quad {
                        shape: Shape::Rect { rect: Rect::new(x.round(), (mid - s / 2.0).round(), s, s), radius: 0.0 },
                        color: if asleep { [0.72, 0.72, 0.74, 1.0] } else { [1.0; 4] },
                        texture: Some(key),
                    });
                    x += s + 6.0 * ui;
                }
                let title = if n.title.trim().is_empty() { n.url.clone() } else { n.title.trim().to_string() };
                let title = match stack {
                    Some((i, len)) => format!("{i}/{len} · {title}"),
                    None => title,
                };
                let ink = match (focused || hovered, asleep) {
                    (true, _) => th.text(),
                    (false, false) => th.text2(),
                    (false, true) => th.text3(),
                };
                let room = (right - 6.0 * ui - x).max(1.0);
                let tw = self.renderer.as_mut().map_or(room, |r| r.measure(&title, size, focused)).min(room);
                crate::chrome::text(&mut l, title, x, mid, size, rgb(ink, 1.0), Rect::new(x, bar.y, room, bar.h), focused);
                // Where the page is from, quietly, after the title of the card
                // in hand: a title can say anything, the address can't.
                let host = palette::host(&n.url);
                if controls && !n.is_note() && scene::icon::site(&n.url).is_some() && !host.is_empty() {
                    let hs = 11.0 * ui;
                    let hw = self.renderer.as_mut().map_or(0.0, |r| r.measure(host, hs, false));
                    let hx = x + tw + 10.0 * ui;
                    if hx + hw <= right - 6.0 * ui {
                        crate::chrome::text(&mut l, host, hx, mid, hs, rgb(th.text3(), 1.0), bar, false);
                    }
                }
                if photo_on {
                    // Over a photo the label sits on a small sheet of the
                    // panel colour, so it reads whatever the picture is.
                    let wide = controls || asleep;
                    let end = if wide { bar.right() } else { (x + tw + 9.0 * ui).min(bar.right()) };
                    let sheet = Rect::new(label_x0 - 7.0 * ui, bar.y + 2.0 * ui, end - label_x0 + 7.0 * ui, bar.h - 4.0 * ui);
                    let round = if th.rounding > 0.0 { sheet.h / 2.0 } else { 0.0 };
                    let mut back = Layer::default();
                    crate::chrome::fill(&mut back, sheet, round, rgb(th.panel(), 0.82));
                    l.quads.splice(label_from.0..label_from.0, back.quads);
                }
            }
            if label_alpha < 1.0 {
                let ease = crate::chrome::ease_out(label_alpha);
                l.quads[label_from.0..].iter_mut().for_each(|q| q.color[3] *= ease);
                l.texts[label_from.1..].iter_mut().for_each(|t| t.color[3] *= ease);
            }
            if let Some((_, a)) = glide.filter(|g| g.1 < 1.0) {
                crate::chrome::fade(&mut l, a, Vec2::new(0.0, 0.0));
            }
            layers.push(l);
        }
        let scene = &self.scene;
        self.label_fades.retain(|id| scene.node(NodeId(id)).is_some());
        self.glides.settle(now);

        // Closed cards shrink away, then their pictures go.
        let mut done = Vec::new();
        for (n, key, at) in &self.ghosts {
            let t = motion::progress(*at, motion::CLOSE, now);
            if t >= 1.0 || board_hidden {
                done.push(*key);
                continue;
            }
            if self.scene.node(n.id).is_some() {
                continue; // back already (undo)
            }
            let (r, a) = motion::closing(self.to_screen_rect(n.rect), t);
            let radius = th.card_corner(k, scale, r.w, r.h);
            let has_tex = self.renderer.as_ref().is_some_and(|rd| rd.has_page(*key));
            let mut l = Layer::default();
            l.quads.push(Quad { shape: Shape::Shadow { rect: r.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (26.0 * k).max(2.0) }, color: th.shadow(0.26 * a), texture: None });
            l.quads.push(Quad {
                shape: Shape::Rect { rect: r, radius },
                color: if has_tex { [1.0, 1.0, 1.0, a] } else { rgb(th.card(false), a) },
                texture: has_tex.then_some(*key),
            });
            layers.push(l);
        }
        if !done.is_empty() {
            let gone: Vec<u64> = self.ghosts.iter().filter(|g| done.contains(&g.1) && self.scene.node(g.0.id).is_none()).map(|g| g.1).collect();
            self.ghosts.retain(|g| !done.contains(&g.1));
            if let Some(rd) = self.renderer.as_mut() {
                for key in gone {
                    rd.remove_page(key);
                }
            }
        }
        // A new space's board slides in from its side as the tint changes.
        if let (Some(t), Some((_, dir, ..))) = (space_t, self.space_in) {
            let shift = Vec2::new(dir * motion::SPACE_SLIDE * ui * (1.0 - t), 0.0);
            for l in &mut layers[board_from..] {
                crate::chrome::fade(l, t, shift);
            }
        }

        if let Some(m) = self.read_morph {
            if let Some(l) = self.read_morph_layer(m) {
                layers.push(l);
            }
        } else if let Some(id) = self.read {
            let l = self.read_layer(id);
            layers.push(l);
        }

        // Over the board's cards, or over the page in Solo.
        if let Some(l) = self.scroll_flash_layer() {
            layers.push(l);
        }
        if let Some(l) = self.link_hint_layer() {
            layers.push(l);
        }

        if let Some(b) = band {
            let r = self.to_screen_rect(b);
            let mut l = Layer::default();
            crate::chrome::fill(&mut l, r, 0.0, rgb(th.marked(), 0.10));
            crate::chrome::outline(&mut l, r, 0.0, line, rgb(th.marked(), 0.9));
            layers.push(l);
        }

        let sidebar_w = if self.sidebar_shown() {
            let (rows, panel) = self.sidebar_layout();
            self.sidebar.scroll = panel.keep_visible(None, self.sidebar.scroll);
            let keys = self.sidebar_has_keys();
            // Icons are looked up before the renderer is borrowed to draw.
            let icons: HashMap<NodeId, u64> = rows
                .iter()
                .filter_map(|r| match r {
                    scene::sidebar::Row::Card { id, .. } => Some(*id),
                    _ => None,
                })
                .filter_map(|id| Some((id, self.icon_key(&self.scene.node(id)?.url)?)))
                .collect();
            let icon = |n: &scene::Node| icons.get(&n.id).copied();
            let chips: Vec<crate::sidebar::SpaceChip> = self
                .spaces
                .spaces
                .iter()
                .map(|e| crate::sidebar::SpaceChip { name: e.name.clone(), colour: e.tint.as_deref().map(|t| th.color(t)), private: e.private })
                .collect();
            let space = self.spaces.active_entry().name.clone();
            // What the head shows of the current card.
            let head = self
                .focused
                .and_then(|id| Some((self.scene.node(id)?, self.cards.get(&id))))
                .map(|(n, c)| {
                    let web = c.is_some_and(|c| c.page.kind() == web::PageKind::Web);
                    let addressed = !n.is_note() && n.url != scene::note::START_URL;
                    crate::sidebar::Current {
                        web,
                        can_back: c.is_some_and(|c| c.can_back),
                        can_forward: c.is_some_and(|c| c.can_forward),
                        host: if addressed { crate::sidebar::pill_host(&n.url) } else { String::new() },
                        secure: n.url.starts_with("https:"),
                    }
                })
                .unwrap_or_default();
            let marks = crate::sidebar::View {
                scroll: self.sidebar.scroll,
                sel: if keys { self.sidebar.index(&rows) } else { None },
                hover: self.sidebar_hover,
                spot: self.sidebar_spot,
                head,
                kept: !self.sidebar_peek,
                viewport: vp,
                current: self.focused,
                drop_line: self.sidebar_drop_line(&rows, &panel),
                space: &space,
                spaces: &chips,
                active_space: self.spaces.position(self.spaces.active).unwrap_or(0),
                icon: &icon,
            };
            let came = self.anim.sidebar.update(Some(0), Instant::now());
            if let Some(r) = self.renderer.as_mut() {
                let mut l = crate::sidebar::layer(r, &panel, &rows, &marks, &self.scene, &th, ui);
                if !self.still {
                    self.sidebar_last = Some((l.clone(), panel.rect.right()));
                }
                // Slides in from past the window's left edge, settling quickly.
                crate::chrome::fade(&mut l, 0.5 + 0.5 * came, Vec2::new(-panel.rect.right() * 0.6 * (1.0 - came), 0.0));
                layers.push(l);
            }
            self.sidebar_out = None;
            panel.rect.right()
        } else {
            self.anim.sidebar.update(None, Instant::now());
            // Hidden: it slides back out past the edge as it came (not into Solo).
            if self.read.is_some() {
                self.sidebar_last = None;
            }
            if let Some((l, right)) = &self.sidebar_last {
                let at = *self.sidebar_out.get_or_insert(now);
                let t = motion::progress(at, motion::SIDEBAR_OUT, now);
                if t < 1.0 {
                    let e = motion::ease_out(t);
                    let mut l = l.clone();
                    crate::chrome::fade(&mut l, 1.0 - e, Vec2::new(-right * 0.6 * e, 0.0));
                    layers.push(l);
                } else {
                    self.sidebar_last = None;
                    self.sidebar_out = None;
                }
            }
            0.0
        };

        // The board's own control: the sidebar's button and the space. Solo
        // is the page alone, with a way back that shows when the pointer
        // nears the top of the window.
        let chrome_shown = self.read.is_none() && self.read_morph.is_none();
        self.nudge.clear();
        let mut tip_shown = None;
        if chrome_shown {
            let entry = self.spaces.active_entry();
            let (space, private, swatch) = (entry.name.clone(), entry.private, th.swatch(tint.as_deref()));
            let space_w = self.renderer.as_mut().map_or(80.0, |r| r.measure(&space, crate::chrome::type_::BODY * ui, true));
            // The panel can be turned off in the settings (*Show the space panel*).
            // The open sidebar carries the space's name and its own buttons.
            self.hud = if self.settings.space_panel && !self.sidebar_shown() { crate::hud::layout(vp, ui, sidebar_w, space_w) } else { crate::hud::Hud::default() };
            let free = self.menu.is_none() && self.omnibox.is_none() && !self.hints && self.confirm.is_none();
            let hud_hover = self.hud_hover.filter(|_| free && self.pointer_in);
            let view = crate::hud::View {
                space: &space,
                swatch,
                private,
                sidebar_open: self.sidebar_open && !self.sidebar_peek,
                hover: hud_hover.map(|(c, _)| c),
                down: self.hud_down,
                open: self.hud_open.filter(|_| self.menu.is_some()),
            };
            let mid_x = sidebar_w + (vp.x - sidebar_w) / 2.0;
            if let Some(r) = self.renderer.as_mut() {
                layers.push(crate::hud::layer(r, &self.hud, &view, &th, ui));
                // Guidance where the board has nothing to show: an empty
                // board says how to start; a view with no card in it (the
                // board is somewhere else) offers the way back.
                let hot = |rect: Rect| free && rect.contains(self.cursor);
                let empty = self.scene.is_empty() && self.omnibox.is_none() && self.ghosts.is_empty();
                let came = self.anim.welcome.update(empty.then(|| anim_key(entry.id.0)), Instant::now());
                if empty {
                    let wv = crate::hud::WelcomeView {
                        space: &space,
                        swatch,
                        private,
                        on_photo: photo_on,
                        pointer: (free && self.pointer_in && matches!(self.drag, Drag::None)).then_some(self.cursor),
                        down: self.hud_down,
                    };
                    let area = Rect::new(sidebar_w, 0.0, (vp.x - sidebar_w).max(0.0), vp.y);
                    let (mut l, buttons) = crate::hud::welcome(r, &wv, area, &th, ui);
                    crate::chrome::arrive(&mut l, came, 8.0 * ui);
                    layers.push(l);
                    self.nudge = buttons;
                } else if !self.scene.is_empty() && !in_view && !board_hidden && self.camera_goal.is_none() && matches!(self.drag, Drag::None) {
                    let bottom = vp.y - 16.0 * ui;
                    let probe = crate::hud::nudge(r, "No cards in view.", "Fit everything", "Shift+F", mid_x, bottom, false, &th, ui).1;
                    let (l, button) = crate::hud::nudge(r, "No cards in view.", "Fit everything", "Shift+F", mid_x, bottom, hot(probe), &th, ui);
                    layers.push(l);
                    self.nudge = vec![(button, crate::hud::Go::Command(Command::Fit))];
                }
            }
            tip_shown = hud_hover.filter(|(_, since)| !self.hud_down && since.elapsed() >= crate::hud::TIP_DWELL).map(|(c, _)| c);
        }

        let shown = self.omnibox.as_ref().map(|ob| anim_key(std::mem::discriminant(&ob.target)));
        let came = self.anim.omnibox.update(shown, Instant::now());
        if let Some(ob) = &self.omnibox {
            // The lists are centred sheets over everything; a lone box (a
            // name, an address, find) sits clear of an open sidebar.
            let r = if ob.target.is_list() { palette::field_rect(vp, ui) } else { palette::field_rect_from(vp, ui, sidebar_w) };
            use crate::icons::Icon;
            let (icon, placeholder, hint): (Icon, &str, &[(&str, &str)]) = match ob.target {
                Target::RenameSpace => (Icon::Board, "Space name", &[("Enter", "Rename")]),
                Target::NameTrail(_) => (Icon::Trail, "Trail name (empty to clear)", &[("Enter", "Name")]),
                Target::Palette if palette::is_command(&ob.text) => (
                    Icon::Search,
                    "",
                    &[("↑", ""), ("↓", "Move"), ("Tab", "Complete"), ("Enter", "Run"), ("Esc", "Close")],
                ),
                Target::Palette => (
                    Icon::Search,
                    "Find a card, space or command · : for a command line",
                    &[("↑", ""), ("↓", "Move"), ("Enter", "Open"), ("Shift+Enter", "Open beside"), ("Esc", "Close")],
                ),
                Target::Find(_) => (Icon::Find, "Find in page", &[("Enter", "Next"), ("Shift+Enter", "Previous")]),
                Target::History => (
                    Icon::History,
                    "Search history",
                    &[("Enter", "Open"), ("Shift+Enter", "Open beside"), ("Shift+Del", "Forget"), ("Esc", "Close")],
                ),
                Target::Downloads => (
                    Icon::Download,
                    "Search downloads",
                    &[("Enter", "Open file"), ("Ctrl+Enter", "Show folder"), ("Shift+Del", "Cancel or remove"), ("Esc", "Close")],
                ),
                Target::AllowedSites => (Icon::Shield, "Sites with ads allowed", &[("Enter", "Block ads again"), ("Esc", "Close")]),
                Target::Photo => (Icon::Picture, "Find a picture or type its path", &[("Enter", "Use as background"), ("Esc", "Close")]),
                Target::SaveAs(_) => (Icon::Download, "Save download as", &[("Enter", "Save"), ("Esc", "Cancel")]),
                Target::Beside => (Icon::Globe, "Search or enter address", &[("Enter", "Open beside")]),
                _ => (Icon::Globe, "Search or enter address", &[("Enter", "Open"), ("Shift+Enter", "Open beside")]),
            };
            // The list (if this field has one) hangs from the field: one sheet.
            let list = ob.target.is_list().then(|| {
                let rows = self.list_rows(ob);
                let panel = self.list_panel(ob, &rows);
                let scroll = ob.scroll.min(panel.max_scroll());
                (rows, panel, scroll)
            });
            // Find in page: "3 of 17" (or "No matches") at the right end.
            let note = match (&ob.target, self.found) {
                (Target::Find(_), Some((count, active))) if !ob.text.is_empty() => Some(match (count, active) {
                    // Chromium picks an active match only once Enter steps to one.
                    (0, _) => "No matches".to_string(),
                    (1, 0) => "1 match".to_string(),
                    (n, 0) => format!("{n} matches"),
                    (n, a) => format!("{a} of {n}"),
                }),
                // An address bar: a keyword names the site it searches (`gh foo`).
                (Target::Focused | Target::NewCard | Target::Beside, _) => match self.settings.resolve(&ob.text) {
                    // Short: the query is in the field already.
                    Some(scene::search::Resolved::Search { keyword: Some(_), site, .. }) => Some(format!("Search {site}")),
                    _ => None,
                },
                _ => None,
            };
            let field = palette::Field {
                icon,
                text: &ob.text,
                before_caret: ob.before_caret(),
                placeholder,
                selected: ob.select_all && !ob.text.is_empty(),
                note,
                // A lone box says what Enter does at its end; a list says it in its foot.
                hint: if list.is_some() { &[] } else { hint },
            };
            // Lists are modal: the board dims behind them.
            let mut fading: Vec<Layer> = Vec::new();
            if list.is_some() {
                // The veil only fades; the sheet also slides into place.
                layers.push(crate::chrome::scrim(screen, &th, came));
            }
            let mut l = Layer::default();
            let radius = th.radius(scale);
            let sheet = match &list {
                Some((_, panel, _)) => Rect::new(r.x, r.y, r.w, panel.foot.bottom() - r.y),
                None => r,
            };
            crate::chrome::panel(&mut l, sheet, radius, &th, ui, crate::chrome::Lift::High, th.panel());
            if list.is_some() {
                // A hairline between the field and its results.
                crate::chrome::fill(&mut l, Rect::new(r.x + line, r.bottom() - line, r.w - 2.0 * line, line), 0.0, rgb(th.line(), 1.0));
            }
            fading.push(l);
            if let Some(rd) = self.renderer.as_mut() {
                fading.push(palette::field_layer(rd, r, &field, &th, ui));
            }
            if let Some((rows, panel, scroll)) = list {
                let command_empty = palette::is_command(&ob.text).then(|| palette::command_empty(&ob.text));
                let empty = match (&ob.target, ob.text.trim().is_empty(), &self.page_index) {
                    _ if command_empty.is_some() => command_empty.as_deref().unwrap_or_default(),
                    (Target::History, _, None) => "History is off. Turn it on in the settings.",
                    (Target::History, true, _) => "No pages in history yet",
                    (Target::Downloads, true, _) => "No downloads yet",
                    (Target::AllowedSites, true, _) => "Ads are blocked on every site",
                    (Target::Photo, true, _) => "No pictures in Pictures or the theme's backgrounds · type a picture's path",
                    _ => "Nothing found",
                };
                // Site icons are looked up before the renderer is borrowed to draw.
                let icons: Vec<Option<u64>> = rows
                    .iter()
                    .map(|row| {
                        let url = match &row.item {
                            Item::Card(id) => self.scene.all_nodes().find(|n| n.id == *id).filter(|n| !n.is_note()).map(|n| n.url.clone()),
                            Item::Open(url) => Some(url.clone()),
                            _ => None,
                        };
                        url.and_then(|u| self.icon_key(&u))
                    })
                    .collect();
                if let Some(rd) = self.renderer.as_mut() {
                    fading.push(palette::layer(rd, &panel, &rows, &icons, ob.sel, scroll, empty, &th, ui));
                    fading.push(palette::foot_layer(rd, panel.foot, hint, rows.len(), &th, ui));
                }
                if let Some(size) = preview
                    && let Some(at) = panel.preview_rect(vp, size, ui)
                {
                    fading.push(palette::preview_layer(at, PREVIEW_KEY, &th, ui));
                }
            }
            for mut l in fading {
                crate::chrome::arrive(&mut l, came, 6.0 * ui);
                layers.push(l);
            }
        }

        if self.menu.is_none() {
            self.hud_open = None;
        }
        let came = self.anim.menu.update(self.menu.as_ref().map(|m| anim_key((m.rect.x as i32, m.rect.y as i32))), Instant::now());
        if let (Some(m), Some(r)) = (&self.menu, self.renderer.as_mut()) {
            let mut l = menu_layer(r, m, ui, &th);
            crate::chrome::arrive(&mut l, came, 4.0 * ui);
            layers.push(l);
        }

        // Messages sit at the foot of the board, in its middle: a key guide
        // while arranging by keyboard, then whatever was just said.
        let mid_x = sidebar_w + (vp.x - sidebar_w) / 2.0;
        let mut bottom = vp.y - 16.0 * ui;
        if self.nudge.iter().any(|(_, g)| *g == crate::hud::Go::Command(Command::Fit)) {
            bottom -= 52.0 * ui;
        }
        if self.arrange.is_some() && self.omnibox.is_none() {
            let l = self.message(ARRANGE_HELP, mid_x, bottom, false);
            bottom -= 48.0 * ui;
            layers.push(l);
        }
        // A half-typed count or key sequence on the board (`5`).
        if !self.pending.is_empty() && self.omnibox.is_none() && !self.entered() {
            let typed = self.pending.display();
            let l = self.message(&typed, mid_x, bottom, false);
            bottom -= 48.0 * ui;
            layers.push(l);
        }
        // An entered card's mode, and what is half-typed in it, in the corner.
        if self.entered() && self.omnibox.is_none() && self.link_hints.is_none() {
            let typed = self.pending.display();
            let mut text = self.page_keys.label().to_string();
            if self.page_keys == PageKeys::Page
                && let Some((id, section, note)) = self.scroll_note.as_ref().filter(|(n, ..)| Some(*n) == self.focused)
            {
                text = format!("{text} · {}", self.section_name(*id, *section));
                if !note.is_empty() {
                    text = format!("{text} · {note}");
                }
            }
            if !typed.is_empty() {
                text = format!("{text} · {typed}");
            }
            let l = self.message(&text, 0.0, vp.y - 16.0 * ui, false);
            layers.push(l);
        }
        // `;` hints say what a label does, in the same corner.
        if let Some(lh) = self.link_hints.as_ref().filter(|h| h.hints.is_some())
            && let Some(kind) = lh.kind
            && self.omnibox.is_none()
        {
            let mut text = format!("Hints · {}", kind.label());
            if kind.keeps() {
                text = format!("{text} · {} opened · Esc to stop", lh.chosen.len());
            }
            let l = self.message(&text, 0.0, vp.y - 16.0 * ui, false);
            layers.push(l);
        }
        if let Some((msg, at)) = &self.toast {
            if self.confirm.is_some() || Instant::now() < *at {
                let msg = msg.clone();
                let warn = self.confirm.is_some();
                let came = self.anim.toast.update(Some(anim_key(&msg)), Instant::now());
                let mut l = self.message(&msg, mid_x, bottom, warn);
                // It rises into place.
                crate::chrome::arrive(&mut l, came, -8.0 * ui);
                layers.push(l);
            } else {
                self.toast = None;
            }
        }
        if self.toast.is_none() {
            self.anim.toast.update(None, Instant::now());
        }

        let came = self.anim.sheet.update(self.hints.then_some(0), Instant::now());
        if self.hints
            && let Some(r) = self.renderer.as_mut()
        {
            for mut l in crate::hints::layers(r, ui, screen, &th, &mut self.sheet) {
                crate::chrome::fade(&mut l, came, Vec2::new(0.0, 0.0));
                layers.push(l);
            }
        }

        // The name and key of the control the pointer rests on.
        let came = self.anim.tip.update(tip_shown.map(anim_key), Instant::now());
        if let (Some(c), Some(r)) = (tip_shown, self.renderer.as_mut())
            && let Some(mut l) = crate::hud::tip(r, &self.hud, c, vp, &th, ui)
        {
            crate::chrome::fade(&mut l, came, Vec2::new(0.0, 0.0));
            layers.push(l);
        }

        if let Some(r) = self.renderer.as_mut() {
            r.render(bg, &layers);
        }
    }

    /// A card growing from its place on the board into Solo, or
    /// shrinking back: the board dims behind it and its corners square off
    /// as it fills the window. The page's pixels keep
    /// their own scale (`read::morph`), so nothing stretches on the way.
    fn read_morph_layer(&mut self, m: ReadMorph) -> Option<Layer> {
        let n = self.scene.node(m.id)?;
        let (header, content) = (self.to_screen_rect(n.header()), self.to_screen_rect(n.rect));
        let card_css = self.card_css(n);
        let p = m.progress();
        let lay = self.read_layout();
        let mo = read::morph(header, content, card_css, &lay, p);
        let th = self.theme.clone();
        let scale = self.scale();
        let vp = self.viewport();
        let k = self.camera.pixels_per_unit(scale);
        let radius = th.card_corner(k, scale, content.w, content.h) * (1.0 - p);
        let tint = self.spaces.active_entry().tint.clone();
        let key = self.page_key(m.id);
        let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
        let extent = self.page_css.get(&m.id).map_or([1.0, 1.0], |&css| read::extent(mo.css, css));
        let frame = Rect::new(mo.bar.x, mo.bar.y, mo.bar.w, mo.content.bottom() - mo.bar.y);
        let mut l = Layer::default();
        // The board recedes behind the page.
        l.quads.push(Quad {
            shape: Shape::Rect { rect: Rect::new(0.0, 0.0, vp.x, vp.y), radius: 0.0 },
            color: rgb(th.board(tint.as_deref()), 0.75 * p),
            texture: None,
        });
        l.quads.push(Quad {
            shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 12.0 * scale)), radius, blur: 36.0 * scale },
            color: th.shadow(0.5 * (1.0 - p * 0.5)),
            texture: None,
        });
        l.quads.push(Quad { shape: Shape::Rect { rect: frame, radius }, color: rgb(th.card(true), 1.0), texture: None });
        l.quads.push(Quad {
            shape: Shape::Page { rect: mo.content, radius, round_bottom: true, extent },
            color: if has_tex { [1.0; 4] } else { rgb(th.placeholder(), 1.0) },
            texture: has_tex.then_some(key),
        });
        Some(l)
    }

    /// Solo: card `id`'s page across the whole window.
    fn read_layer(&mut self, id: NodeId) -> Layer {
        let th = self.theme.clone();
        let scale = self.scale();
        let lay = self.read_layout();
        let key = self.page_key(id);
        let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
        let loading = self.cards.get(&id).is_some_and(|c| c.loading);
        let mut l = Layer::default();
        let extent = self.page_css.get(&id).map_or([1.0, 1.0], |&css| read::extent(Vec2::new(lay.css.0 as f32, lay.css.1 as f32), css));
        l.quads.push(Quad {
            shape: Shape::Page { rect: lay.content, radius: 0.0, round_bottom: false, extent },
            color: if has_tex { [1.0; 4] } else { rgb(th.placeholder(), 1.0) },
            texture: has_tex.then_some(key),
        });
        // No title bar: the page fills the window. A loading page shows a
        // thin moving line across its top edge.
        if loading {
            let c = lay.content;
            let t = (self.started.elapsed().as_secs_f32() * 0.8).fract();
            let w = c.w * 0.25;
            l.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(c.x + (c.w - w) * t, c.y, w, 2.0 * scale.max(1.0)), radius: 0.0 },
                color: rgb(th.accent(), 1.0),
                texture: None,
            });
        }
        l
    }

    /// A message centred on `mid_x` with its bottom at `bottom` (screen px):
    /// a toast, or a question. `warn` gives it a red edge: answering it
    /// loses something.
    fn message(&mut self, text: &str, mid_x: f32, bottom: f32, warn: bool) -> Layer {
        let th = &self.theme;
        let ui = self.scale() * th.text_scale;
        let size = crate::chrome::type_::BODY * ui;
        // The palette's and menus' sheet: same fill, edge, corners and a
        // high lift, so it reads over cards as well as over the board.
        let (pad, h) = (16.0 * ui, 40.0 * ui);
        let vp = self.viewport();
        let max_w = (vp.x - 32.0 * ui).max(80.0);
        let tw = self.renderer.as_mut().map_or(200.0, |r| r.measure(text, size, false));
        let w = (tw + 2.0 * pad).min(max_w);
        let r = Rect::new((mid_x - w / 2.0).clamp(16.0 * ui, (vp.x - w - 16.0 * ui).max(16.0 * ui)).round(), (bottom - h).round(), w.round(), h);
        let mut l = Layer::default();
        let (rim, bw) = if warn { (th.danger(), (1.5 * ui).max(1.0)) } else { (crate::chrome::edge(th), crate::chrome::hair(ui)) };
        crate::chrome::panel_edged(&mut l, r, th.radius(self.scale()), th, ui, crate::chrome::Lift::High, th.panel(), rim, bw);
        let clip = Rect::new(r.x + pad, r.y, (r.w - 2.0 * pad).max(1.0), r.h);
        let shown = self.renderer.as_mut().map_or_else(|| text.to_string(), |rd| rd.ellipsize(text, size, false, clip.w));
        crate::chrome::text(&mut l, shown, r.x + pad, r.center().y, size, rgb(th.text(), 1.0), clip, false);
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
        // The save field closed (Esc, or something else opened): the download is off.
        if self.save_as.as_ref().is_some_and(|s| self.omnibox.as_ref().is_none_or(|ob| ob.target != Target::SaveAs(s.id))) {
            let s = self.save_as.take().expect("checked");
            if web::download::refuse(s.id) {
                log::info!("download {}: cancelled (no path given)", s.id);
                self.toast("Download cancelled");
            }
            if s.blank {
                self.close_blank_card(s.page);
            }
        }
        // A permission question dropped by another (a space switch, say) comes back.
        if self.confirm.is_none() && !self.asks.is_empty() {
            self.show_next_ask();
        }
        if let Some(msg) = self.blocking.tick(Instant::now(), self.settings.block_ads) {
            self.toast(msg);
        }
        self.read_due_texts();
        // Nothing entered: the next card entered starts afresh.
        if !self.entered() && self.page_keys != PageKeys::Page {
            self.page_keys = PageKeys::Page;
        }
        // A key sequence left half-typed runs (or is dropped) after a pause.
        let page = self.page_mode();
        let table = if page { &self.page_keys_table } else { &self.board_keys };
        if let Some(Outcome::Run { action, count }) = self.pending.expire(table, Instant::now()) {
            if page { self.page_action(action, count) } else { self.board_action(action, count) }
        }
        self.sidebar_peek_tick();
        self.sync_find();
        self.autosave();
        self.step_camera(dt);
        self.step_page_scroll(dt);
        self.step_board_pan(dt);
        if self.hints {
            self.sheet.step(dt);
        }
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
        // Only where text goes: an input method would take Page mode's keys.
        let page_keys = self.typing();
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
            let f = palette::field_rect(self.viewport(), self.ui());
            Some([f.x as i32, f.y as i32, f.w as i32, f.h as i32])
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
            site: self.site_content_state(id),
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
        let ksize = crate::chrome::type_::CAPTION * scale;
        let label_w = match self.renderer.as_mut() {
            Some(r) => items
                .iter()
                .filter(|e| !e.is_separator())
                .map(|e| {
                    let keys = crate::chrome::keycaps(e.keys()).unwrap_or_default();
                    let widths: Vec<f32> = keys.iter().map(|k| r.measure(k, ksize, false)).collect();
                    let kw = if keys.is_empty() { 0.0 } else { crate::chrome::keycaps_width(&widths, ksize) + 24.0 * scale };
                    r.measure(e.label(), menu::FONT * scale, e.is_current()) + kw
                })
                .fold(0.0, f32::max),
            None => 0.0,
        };
        ContextMenu::new(id, items, anchor, label_w, self.viewport(), scale)
    }

    /// A press on one of the board's own controls.
    fn hud_press(&mut self, c: crate::hud::Control) {
        use crate::hud::Control;
        log::info!("control: {c:?}");
        self.keys_in_page = false;
        match c {
            Control::Space => {
                let items = self.space_menu_items();
                self.open_hud_menu(c, items);
            }
            _ => {
                if let Some(cmd) = c.command() {
                    self.run_command(cmd);
                }
            }
        }
    }

    /// Opens `items` as the menu of control `c`, under it. A second press
    /// on the control closes it.
    /// The spaces menu: every space, then making, renaming, tinting and deleting them.
    fn space_menu_items(&self) -> Vec<Entry> {
        let active = self.spaces.active;
        let mut items: Vec<Entry> = self
            .spaces
            .spaces
            .iter()
            .map(|e| Entry::Space { id: e.id, label: if e.private { format!("{} · private", e.name) } else { e.name.clone() }, current: e.id == active })
            .collect();
        items.push(Entry::separator());
        items.extend([Command::NewSpace, Command::NewPrivateSpace].map(Entry::command));
        items.push(Entry::separator());
        items.extend([Command::RenameSpace, Command::NextTint, Command::BackgroundPhoto, Command::PrivateSpace, Command::DeleteSpace].map(Entry::command));
        items
    }

    /// The sidebar's space line: the spaces menu under it.
    fn open_sidebar_space_menu(&mut self, at: Rect) {
        let anchor = Vec2::new(at.x + 6.0 * self.ui(), at.bottom() + 4.0 * self.ui());
        let m = self.layout_menu(menu::NO_CARD, self.space_menu_items(), anchor);
        log::info!("sidebar space menu: {:?}", menu_labels(&m.items));
        self.menu = Some(m);
        self.hud_open = None;
        self.menu_pressed = false;
    }

    fn open_hud_menu(&mut self, c: crate::hud::Control, items: Vec<Entry>) {
        if self.hud_open == Some(c) && self.menu.is_some() {
            self.menu = None;
            self.hud_open = None;
            return;
        }
        let Some(at) = self.hud.rect(c) else { return };
        let anchor = Vec2::new(at.x, at.bottom() + 8.0 * self.ui());
        let m = self.layout_menu(menu::NO_CARD, items, anchor);
        log::info!("menu of {c:?}: {:?}", menu_labels(&m.items));
        self.menu = Some(m);
        self.hud_open = Some(c);
        self.menu_pressed = false;
    }

    /// A right-click on the empty board: what can be made and done there,
    /// each with its key.
    fn open_board_menu(&mut self, at: Vec2) {
        let groups: [&[Command]; 4] = [
            &[Command::NewCard, Command::NewNote, Command::NewSticky],
            &[Command::Tidy, Command::Fold, Command::Fit],
            &[Command::Reopen, Command::Undo, Command::Redo],
            &[Command::History, Command::Downloads, Command::Settings, Command::Shortcuts],
        ];
        let mut items: Vec<Entry> = Vec::new();
        for (i, g) in groups.iter().enumerate() {
            if i > 0 {
                items.push(Entry::separator());
            }
            items.extend(g.iter().copied().map(Entry::command));
        }
        let m = self.layout_menu(menu::NO_CARD, items, at);
        log::info!("board menu: {:?}", menu_labels(&m.items));
        self.menu = Some(m);
        self.hud_open = None;
        self.menu_pressed = false;
    }

    /// A click on a label's shield: the site's blocking entries (the
    /// end of the page menu), under the shield.
    fn open_shield_menu(&mut self, id: NodeId) {
        let Some(b) = self.blocking_state(id) else { return };
        let Some(at) = self.shields.iter().find(|(s, _)| *s == id).map(|(_, r)| *r) else { return };
        let items = web::menu::blocking_items(&b).into_iter().map(Entry::Page).collect();
        let anchor = Vec2::new(at.x, at.bottom() + 2.0 * self.ui());
        let m = self.layout_menu(id, items, anchor);
        log::info!("shield menu on card {}: {:?}", id.0, menu_labels(&m.items));
        self.menu = Some(m);
    }

    /// The count badge on a stack's front, screen px. Sized by digit count, so
    /// hit testing needs no text measuring.
    fn stack_badge(&self, n: &scene::Node) -> Option<Rect> {
        let len = self.scene.stack_of(n.id)?.members.len();
        let size = 11.0 * self.ui();
        let frame = self.to_screen_rect(n.frame());
        let h = (size * 1.7).round();
        let w = (size * (0.62 * len.to_string().len() as f32 + 1.0)).max(h);
        // At the start of the card's label.
        let ui = self.ui();
        Some(Rect::new(frame.x, (frame.y - BAR_GAP * ui - BAR_H * ui / 2.0 - h / 2.0).round(), w, h))
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
            Some(Entry::Command { cmd, label, .. }) => {
                log::info!("menu: {label}");
                self.run_command(cmd);
                return;
            }
            Some(Entry::Space { id: space, label, .. }) => {
                log::info!("menu: space {label}");
                if space != self.spaces.active {
                    self.switch_space(space);
                }
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
            MenuAction::ToggleJavaScript => self.toggle_site_content(id, scene::sitesettings::Content::JavaScript),
            MenuAction::ToggleImages => self.toggle_site_content(id, scene::sitesettings::Content::Images),
            MenuAction::ReplaceMisspelling(word) => {
                if let Some(c) = self.cards.get(&id) {
                    c.page.replace_misspelling(&word);
                }
            }
            MenuAction::AddToDictionary(word) => {
                if let Some(c) = self.cards.get(&id) {
                    c.page.add_to_dictionary(&word);
                }
            }
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

fn menu_layer(r: &mut Renderer, m: &ContextMenu, ui: f32, th: &Theme) -> Layer {
    let mut l = Layer::default();
    let radius = th.radius(ui / th.text_scale);
    let small = th.small_radius(ui / th.text_scale);
    crate::chrome::panel(&mut l, m.rect, radius, th, ui, crate::chrome::Lift::Mid, th.panel());
    let size = menu::FONT * ui;
    let ksize = crate::chrome::type_::CAPTION * ui;
    for (i, (item, row)) in m.items.iter().zip(&m.rows).enumerate() {
        if item.is_separator() {
            let line = crate::chrome::hair(ui);
            crate::chrome::fill(&mut l, Rect::new(m.rect.x + line, (row.center().y - line / 2.0).round(), m.rect.w - 2.0 * line, line), 0.0, rgb(th.line(), 1.0));
            continue;
        }
        let hot = m.hover == Some(i);
        if hot {
            crate::chrome::fill(&mut l, *row, small, rgb(th.active(), 1.0));
        }
        let mid = row.center().y;
        // The lead column: the row's icon, or a tick on the card in front
        // (or the space showing) for rows that have none.
        let icon_x = row.x + menu::ICON_X * ui;
        match item.icon() {
            Some(icon) => {
                let ink = match (item.is_enabled(), hot) {
                    (false, _) => th.text3(),
                    (true, true) => th.text(),
                    (true, false) => th.text2(),
                };
                crate::icons::draw(&mut l, r, icon, icon_x, mid, 16.0 * ui, rgb(ink, 1.0));
            }
            None if item.is_current() => crate::icons::draw(&mut l, r, crate::icons::Icon::Check, icon_x, mid, 14.0 * ui, rgb(th.accent_text(), 1.0)),
            None => {}
        }
        let mut right = row.right() - 8.0 * ui;
        if let Some(keys) = crate::chrome::keycaps(item.keys()) {
            let widths: Vec<f32> = keys.iter().map(|k| r.measure(k, ksize, false)).collect();
            right = crate::chrome::draw_keycaps(&mut l, &keys, &widths, right, mid, ksize, th, ui, *row) - 12.0 * ui;
        }
        let x = row.x + (menu::TEXT_PAD - menu::PAD) * ui;
        let ink = match (item.is_enabled(), hot) {
            (false, _) => th.text3(),
            (true, true) => th.text(),
            (true, false) => crate::theme::blend(th.text(), th.text2(), 0.35),
        };
        crate::chrome::text(&mut l, item.label(), x, mid, size, rgb(ink, 1.0), Rect::new(x, row.y, (right - x).max(1.0), row.h), item.is_current());
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
                Cmd::RightClick(x, y) => {
                    self.cursor_moved(Vec2::new(x, y));
                    self.mouse_input(ElementState::Pressed, MouseButton::Right);
                    self.mouse_input(ElementState::Released, MouseButton::Right);
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
                Cmd::ShieldClick(id) => match self.shields.iter().find(|(s, _)| s.0 == id).map(|(_, r)| *r) {
                    Some(r) => {
                        self.cursor_moved(r.center());
                        self.mouse_input(ElementState::Pressed, MouseButton::Left);
                        self.mouse_input(ElementState::Released, MouseButton::Left);
                    }
                    None => log::warn!("script: card {id} shows no shield"),
                },
                Cmd::TagClick(id) => match self.tags.iter().find(|(t, _)| t.0 == id).map(|(_, r)| *r) {
                    Some(r) => {
                        self.cursor_moved(r.center());
                        self.mouse_input(ElementState::Pressed, MouseButton::Left);
                        self.mouse_input(ElementState::Released, MouseButton::Left);
                    }
                    None => log::warn!("script: no trail tag on card {id}"),
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
                            "script card {}: world ({:.0},{:.0} {:.0}x{:.0}) screen ({:.0},{:.0} {:.0}x{:.0}) live={} sel={} hidden={} blocked={} elements_hidden={} title={:?} {}",
                            n.id.0, r.x, r.y, r.w, r.h, f.x, f.y, f.w, f.h, self.cards.contains_key(&n.id),
                            self.selection.contains(n.id), self.scene.is_hidden(n.id),
                            self.cards.get(&n.id).map_or(0, |c| c.page.blocked()),
                            self.cards.get(&n.id).map_or(0, |c| c.page.hidden()), n.title, n.url
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
                    log::info!(
                        "script focus: card {:?} entered={} mode={:?} solo={:?} order={:?}",
                        self.focused.map(|f| f.0), self.entered(), self.page_keys, self.read.map(|r| r.0),
                        scene::tidy::order(&self.scene).iter().map(|i| i.0).collect::<Vec<_>>()
                    );
                    let l = self.read_layout();
                    log::info!(
                        "script read: {:?} css={:?} page_scale={} focused={:?} keys_in_page={} mode={} camera=({:.0},{:.0} z{:.3})",
                        self.read.map(|r| r.0), l.css, self.page_scale, self.focused.map(|f| f.0), self.keys_in_page,
                        if self.entered() { format!("{:?}", self.page_keys) } else { "Board".into() },
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
                    let (rows, panel) = self.sidebar_layout();
                    log::info!(
                        "script sidebar: open={} peek={} keys={} sel={:?} scroll={} shown={} width={:.0} rows={}",
                        self.sidebar_open, self.sidebar_peek, self.sidebar_has_keys(), self.sidebar.index(&rows), self.sidebar.scroll,
                        panel.fits(self.sidebar.scroll), panel.rect.w, rows.len()
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
                    log::info!("script icons: sites={sites:?} named={:?}", self.focused.map(|id| id.0));
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
                        let default = self.cards[&id].page.default_zoom_factor();
                        log::info!(
                            "script zoom card {}: site {:?} want {} chromium/scale {page:.3} default/scale {default:.3} {url:?}",
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
                        let panel = self.list_panel(ob, &rows);
                        let scroll = ob.scroll.min(panel.max_scroll());
                        log::info!(
                            "script palette: query={:?} sel={} scroll={} shown={} rows={}",
                            ob.text, ob.sel, scroll, panel.fits(scroll), rows.len()
                        );
                        for (i, row) in rows.iter().enumerate() {
                            let at = panel.row_rect(i, scroll).map(|r| r.center());
                            let at = at.map_or_else(|| "off".to_string(), |c| format!("({:.0},{:.0})", c.x, c.y));
                            let head = panel.label_rect(i, scroll).map_or("", |(_, h)| h);
                            log::info!(
                                "script palette row {i} at {at}: {:?} {:?} detail={:?} marks={:?} section={head:?}",
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
    /// The title bar's back, forward and reload buttons.
    Back(NodeId),
    Forward(NodeId),
    Reload(NodeId),
    Grip(NodeId),
    Content(NodeId),
    /// Solo's title bar.
    ReadBar(NodeId),
    /// A stack's count badge.
    Badge(NodeId),
    /// The blocked-request count on a card's title bar.
    Shield(NodeId),
    /// A trail's tag above its first card on the board.
    Tag(NodeId),
}

impl Hit {
    fn id(&self) -> Option<NodeId> {
        match *self {
            Hit::Board => None,
            Hit::Header(id) | Hit::Close(id) | Hit::Back(id) | Hit::Forward(id) | Hit::Reload(id) | Hit::Grip(id) | Hit::Content(id) | Hit::ReadBar(id) | Hit::Badge(id) | Hit::Shield(id) | Hit::Tag(id) => Some(id),
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
        self.start_desktop();
        self.update_card_size();
        let first_run = spaces::is_first_run(&self.data_dir);
        let restored = self.restore();
        // At the restored camera's resolution, so pages don't lay out twice.
        self.page_scale = self.settled_page_scale();
        let urls = std::mem::take(&mut self.initial_urls);
        let mut last = None;
        for url in &urls {
            last = self.new_card_at_center(url).or(last);
        }
        if urls.is_empty() && !restored {
            last = Some(self.add_start());
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
                // The window may have moved to another monitor.
                self.update_card_size();
            }
            WindowEvent::ScaleFactorChanged { .. } => self.camera_moved(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => self.cursor_moved(Vec2::new(position.x as f32, position.y as f32)),
            WindowEvent::CursorLeft { .. } => {
                self.pointer_in = false;
                self.hud_hover = None;
                self.hover = Hit::Board;
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
