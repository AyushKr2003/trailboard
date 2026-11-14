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
//! - f labels the links on the current card's page (`scene::hint`, `web::links`);
//!   typing a label opens that link beside the card, which becomes current. Esc,
//!   another key, a click or a scroll ends the labels. Shift+F fits everything.
//! - M arranges the current card (`scene::arrange`): h/j/k/l move it (the
//!   selection with it), Shift resizes, Ctrl takes fine steps; Enter/M keep the
//!   result as one undo step, Esc puts it back, any other key or a click keeps it.
//! - Ctrl+L address bar, Ctrl+T new card, Ctrl+W close, Ctrl+Shift+T reopen,
//!   Ctrl+R reload, Alt+←/→ back/forward, Ctrl+0 fit all.
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
//!   first when the space has cards). Each space keeps its own board, camera,
//!   background, undo history and closed cards.
//!
//! Live-browser cap: at most `TRAILBOARD_MAX_LIVE` (default 6) cards keep a live
//! browser (see `scene::liveness`). The others sleep: their page is closed and the
//! card shows its last frame, dimmed, until it is focused or scrolled into view.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use compositor::{Background, Layer, Quad, Renderer, Shape, Text};
use scene::arrange::Arrange;
use scene::hint;
use scene::nav::{self, Dir};
use scene::{liveness, placement, read, selection, tidy, Camera, Change, Edge, History, Node, NodeId, Rect, Scene, Selection, TrailStep, Vec2};
use storage::spaces::{self, SpaceId, SpaceIndex};
use web::{MouseButton as WebButton, Page, SpawnBeside, WebEvent};
use winit::application::ApplicationHandler;
use web::menu::{MenuAction, MenuItem};
use web::EditCommand;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, ModifiersState, NamedKey};
use winit::platform::wayland::WindowAttributesExtWayland;
use winit::window::{CursorIcon, Window, WindowId};

use crate::clipboard::{Clipboard, Paste};
use crate::desktop::Desktop;
use crate::theme::{rgb, text_on, Theme};
use crate::keymap::{self, KeyIn};
use crate::menu::{self, ContextMenu, Entry};
use crate::omnibox::{self, Omnibox, Target};
use crate::script::{Cmd, Script};
use crate::sidebar::Panel;

const START_URL: &str = "https://en.wikipedia.org/wiki/Special:Random";
const CARD_W: f32 = 1024.0;
const CARD_H: f32 = 720.0;
const MIN_CARD: f32 = 240.0;
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

/// Browser and texture key for card `id` while `epoch` is current.
fn page_key(epoch: u64, id: NodeId) -> u64 {
    (epoch << KEY_ID_BITS) | id.0
}

/// Per-space state that lives only for this session.
#[derive(Default)]
struct Session {
    history: History,
    closed: Vec<(Node, Vec<Edge>)>,
}
/// Default number of cards that keep a live browser.
const DEFAULT_MAX_LIVE: usize = 6;

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
struct LinkHints {
    id: NodeId,
    links: Vec<(Rect, String)>,
    hints: Option<hint::Hints>,
}

struct Card {
    page: Page,
    loading: bool,
    hidden: bool,
    /// Cursor the page asked for, shown while the pointer is over its content.
    cursor: web::Cursor,
}

pub struct App {
    renderer: Option<Renderer>,
    scene: Scene,
    camera: Camera,
    /// Camera we're gliding toward (fit / read mode).
    camera_goal: Option<Camera>,
    /// Live browsers. A card in the scene but not here is asleep.
    cards: HashMap<NodeId, Card>,
    max_live: usize,
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
    initial_url: Option<String>,
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
    /// Short message at the top of the window, until it expires.
    toast: Option<(String, Instant)>,
    /// Waiting for Enter to delete the current space.
    confirm_delete: bool,
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
}

impl App {
    pub fn new(initial_url: Option<String>, data_dir: std::path::PathBuf) -> Self {
        Self {
            renderer: None,
            scene: Scene::new(),
            camera: Camera::default(),
            camera_goal: None,
            cards: HashMap::new(),
            max_live: max_live_from_env(),
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
            initial_url,
            started: Instant::now(),
            script: Script::from_env(),
            quit: false,
            data_dir,
            spaces: SpaceIndex::default(),
            persist: true,
            epoch: 0,
            sessions: HashMap::new(),
            toast: None,
            confirm_delete: false,
            dirty: false,
            last_save: Instant::now(),
            theme: Theme::default(),
            desktop: None,
            page_scheme: None,
            note_target: None,
            saved: HashMap::new(),
        }
    }

    pub fn shutdown(&mut self) {
        self.save();
        self.cards.clear(); // closes every browser before CEF shuts down
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
        self.load_board()
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
        let msg = msg.into();
        log::info!("toast: {msg}");
        self.toast = Some((msg, Instant::now()));
    }

    /// "Name (2 of 3)" for the active space.
    fn space_label(&self) -> String {
        let e = self.spaces.active_entry();
        let i = self.spaces.position(e.id).unwrap_or(0) + 1;
        format!("{} ({i} of {})", e.name, self.spaces.spaces.len())
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
        self.cards.clear(); // closes the browsers
        let session = Session { history: std::mem::take(&mut self.history), closed: std::mem::take(&mut self.closed) };
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
        self.ime_page = None;
        self.confirm_delete = false;
        self.epoch += 1;
        // Anything the old pages still had queued is for cards that are gone.
        drop(web::drain_events());
        self.dirty = false;
    }

    /// Show the active space (after `unload_space`).
    fn load_space(&mut self) {
        let s = self.sessions.remove(&self.spaces.active).unwrap_or_default();
        self.history = s.history;
        self.closed = s.closed;
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

    fn new_space(&mut self) {
        self.link_hints = None;
        let id = self.spaces.add();
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
        let dark = !self.theme.light();
        if self.page_scheme != Some(dark) {
            web::set_color_scheme(dark);
            for c in self.cards.values() {
                c.page.apply_color_scheme();
            }
            self.page_scheme = Some(dark);
        }
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
            self.confirm_delete = true;
            self.toast(format!("Delete {} and its {cards}? Enter deletes · Esc keeps it", e.name));
        }
    }

    fn delete_space(&mut self) {
        self.confirm_delete = false;
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
        match Page::new(self.page_key(id), &url, kind, (r.w as i32, r.h as i32), self.page_scale, &self.spawn_beside) {
            Some(page) => {
                self.cards.insert(id, Card { page, loading: true, hidden: false, cursor: web::Cursor::Default });
            }
            None => log::error!("failed to create browser for card {}", id.0),
        }
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
        if self.cards.remove(&id).is_some() {
            log::info!("sleeping card {} ({} live)", id.0, self.cards.len());
        }
    }

    /// Stop the live browser of a card that has left the scene.
    fn drop_page(&mut self, id: NodeId) {
        self.dirty = true;
        self.cards.remove(&id);
        self.used.remove(&id);
        self.selection.remove(id);
        let key = self.page_key(id);
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
            let id = node.id;
            if self.scene.insert(node, &edges) {
                self.create_page(id);
                self.history.opened(id);
                self.dirty = true;
                self.focus(Some(id));
                self.ensure_visible(id);
                return;
            }
        }
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
        self.selection.clear();
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
    fn sidebar_key(&mut self, code: Option<KeyCode>) -> bool {
        use scene::sidebar::{Row, RowKey};
        let rows = self.sidebar.rows(&self.scene);
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
            Some(KeyCode::Escape | KeyCode::KeyB) => self.close_sidebar(),
            _ => return false,
        }
        self.sidebar_scroll_to_sel();
        true
    }

    /// Goes to card `id` from the sidebar: it is shown at its stack's front if
    /// folded away, becomes current and comes on screen (clear of the panel).
    fn sidebar_go(&mut self, id: NodeId) {
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
        log::info!("sidebar: card {}", id.0);
    }

    /// A press on the sidebar: a header collapses or expands its trail, a card
    /// row goes to that card. Either gives the sidebar the keys.
    fn sidebar_click(&mut self, p: Vec2, button: MouseButton) {
        use scene::sidebar::Row;
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
            Row::Header { group, .. } => self.sidebar.toggle(group),
            Row::Card { id, .. } => self.sidebar_go(id),
        }
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
            .map(|l| (Rect::new(l.rect[0] * k, l.rect[1] * k, l.rect[2] * k, l.rect[3] * k), l.url))
            .collect();
        if lh.links.is_empty() {
            self.link_hints = None;
            let loading = self.cards.get(&id).is_some_and(|c| c.loading);
            log::info!("link hints on card {}: no links in view", id.0);
            self.toast(if loading { "No links yet · the page is still loading" } else { "No links in view" });
            return;
        }
        let h = hint::Hints::new(lh.links.len());
        log::info!(
            "link hints on card {}: {} links, {}",
            id.0,
            lh.links.len(),
            h.labels.iter().zip(&lh.links).take(6).map(|(l, (_, u))| format!("{l}={u}")).collect::<Vec<_>>().join(" ")
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
                    let (id, url) = (lh.id, lh.links[i].1.clone());
                    self.link_hints = None;
                    self.follow_link(id, &url);
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
                WebEvent::OpenBeside { source, url } => {
                    let Some(src) = self.node_of(source) else { continue };
                    if self.scene.node(src).is_some()
                        && let Some(id) = self.open_card(Rect::default(), &url, Some(src)) {
                            self.ensure_visible(id);
                        }
                }
                WebEvent::TitleChanged { page, title } => {
                    if let Some(n) = self.node_of(page).and_then(|id| self.scene.node_mut(id)) {
                        n.title = title;
                        self.dirty = true;
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
                WebEvent::LoadingChanged { page, loading } => {
                    if let Some(c) = self.node_of(page).and_then(|id| self.cards.get_mut(&id)) {
                        c.loading = loading;
                    }
                }
            }
        }
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
        if self.confirm_delete {
            if state == ElementState::Pressed {
                self.confirm_delete = false;
                self.toast(format!("Kept {}", self.spaces.active_entry().name));
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
        if state == ElementState::Released && std::mem::take(&mut self.menu_pressed) {
            return;
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
        self.sidebar_hover = None;
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

    fn wheel(&mut self, delta: MouseScrollDelta) {
        let (dx, dy, lines) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x, y, true),
            MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32, false),
        };
        self.camera_goal = None;
        self.menu = None;
        self.link_hints = None; // scrolling a page moves its links
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
                    let (px, py) = if lines { (dx * 100.0, dy * 100.0) } else { (dx, dy) };
                    card.page.mouse_wheel(x, y, keymap::modifiers(self.mods), px as i32, py as i32);
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
        if self.confirm_delete {
            // Enter deletes the space; any other key (but a modifier) keeps it and is dropped.
            if pressed && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)) {
                if ev.named == Some(NamedKey::Enter) {
                    self.delete_space();
                } else {
                    self.confirm_delete = false;
                    self.toast(format!("Kept {}", self.spaces.active_entry().name));
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

        if let Some(ob) = self.omnibox.as_mut() {
            if !pressed {
                return;
            }
            match ev.named {
                Some(NamedKey::Escape) => self.omnibox = None,
                Some(NamedKey::Enter) => {
                    let ob = self.omnibox.take().expect("omnibox open");
                    if ob.target == Target::RenameSpace {
                        self.rename_space(&ob.text);
                    } else if let Target::NameTrail(id) = ob.target {
                        self.name_trail(id, &ob.text);
                    } else if let Some(url) = omnibox::resolve(&ob.text) {
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
            return;
        }

        // The trails sidebar, when it has the keys, takes its own (j/k, h/l,
        // Enter, Esc, B); the rest are board keys as usual.
        if pressed && !ctrl && !alt && !shift && self.drag_is_idle() && self.sidebar_has_keys() && self.sidebar_key(code) {
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
                    self.new_space();
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
                Some(KeyCode::KeyW) if ctrl && shift && !alt => {
                    self.ask_delete_space();
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
                Some(KeyCode::Digit0) if ctrl => {
                    self.fit_all();
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
        }

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
            l.quads.push(Quad {
                shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (22.0 * k).max(2.0) },
                color: th.shadow(if focused { 0.55 } else { 0.4 }),
                texture: None,
            });
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
            let font = 13.0 * k;
            if font >= 6.0 && sticky.is_none() {
                let pad = 10.0 * k;
                let title = if n.title.is_empty() { n.url.as_str() } else { n.title.as_str() };
                let title = if asleep { format!("{title} · asleep") } else { title.to_string() };
                let title = match stack {
                    Some((i, len)) => format!("{i}/{len} · {title}"),
                    None => title,
                };
                let clip = Rect::new(header.x + pad, header.y, (header.w - pad * 2.0 - CLOSE_W * k).max(1.0), header.h);
                l.texts.push(Text {
                    text: title,
                    pos: [header.x + pad, header.y + (header.h - font * 1.3) / 2.0],
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

        // Empty-board hint.
        if self.scene.is_empty() && self.omnibox.is_none() {
            let size = 16.0 * ui;
            let msg = "Press N or Ctrl+T to open a page · ? for shortcuts";
            let w = self.renderer.as_mut().map_or(300.0, |r| r.measure(msg, size, false));
            layers.push(Layer {
                quads: vec![],
                texts: vec![Text {
                    text: msg.into(),
                    pos: [(vp.x - w) / 2.0, vp.y / 2.0 - size],
                    size,
                    color: rgb(th.dim_text(), 1.0),
                    clip: screen,
                    bold: false,
                }],
            });
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
            };
            if let Some(r) = self.renderer.as_mut() {
                layers.push(crate::sidebar::layer(r, &panel, &rows, &marks, &self.scene, &th, ui));
            }
            panel.rect.w
        } else {
            0.0
        };

        if let Some(ob) = &self.omnibox {
            let w = (720.0 * ui).min(vp.x - 32.0 * ui);
            let h = 48.0 * ui;
            let r = Rect::new((vp.x - w) / 2.0, 18.0 * ui, w, h);
            let size = 17.0 * ui;
            let pad = 16.0 * ui;
            let placeholder = match ob.target {
                Target::RenameSpace => "Space name",
                Target::NameTrail(_) => "Trail name (empty to clear)",
                _ => "Search or enter address",
            };
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
            let mut l = Layer::default();
            l.quads.push(Quad {
                shape: Shape::Shadow { rect: r.translate(Vec2::new(0.0, 8.0 * ui)), radius: th.radius(scale), blur: 28.0 * ui },
                color: th.shadow(0.6),
                texture: None,
            });
            l.quads.push(Quad { shape: Shape::Rect { rect: r, radius: th.radius(scale) }, color: rgb(th.panel(), 1.0), texture: None });
            if selected {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(r.x + pad, r.y + 12.0 * ui, text_w, h - 24.0 * ui), radius: th.small_radius(scale) },
                    color: rgb(th.accent(), 1.0),
                    texture: None,
                });
            } else {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(r.x + pad + caret_x, r.y + 12.0 * ui, 2.0 * scale.max(1.0), h - 24.0 * ui), radius: 0.0 },
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
            layers.push(l);
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

        if let Some(m) = &self.menu {
            layers.push(menu_layer(m, ui, &th));
        }

        // Space name in the top-left corner once there is more than one space.
        if self.spaces.spaces.len() > 1 && self.omnibox.is_none() && self.read.is_none() {
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
        if let Some((msg, at)) = &self.toast {
            if self.confirm_delete || at.elapsed() < TOAST_FOR {
                if self.omnibox.is_none() {
                    let size = 15.0 * ui;
                    let msg = msg.clone();
                    let w = self.renderer.as_mut().map_or(300.0, |r| r.measure(&msg, size, false));
                    // Below Read Mode's bar, so it doesn't cover the title.
                    let top = if self.read.is_some() { READ_BAR * ui } else { 0.0 };
                    let pos = Vec2::new((vp.x - w) / 2.0 - 16.0 * ui, top + 18.0 * ui);
                    let warn = self.confirm_delete;
                    layers.push(self.pill(&msg, pos, size, None, warn));
                }
            } else {
                self.toast = None;
            }
        }

        if self.hints
            && let Some(r) = self.renderer.as_mut()
        {
            layers.push(crate::hints::layer(r, ui, screen, &th));
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
        l.quads.push(Quad {
            shape: Shape::Shadow { rect: r.translate(Vec2::new(0.0, 4.0 * scale)), radius, blur: 16.0 * scale },
            color: th.shadow(0.5),
            texture: None,
        });
        // A border in the divider colour (danger for a question that loses data).
        let border = if warn { th.danger() } else { th.divider() };
        let bw = if warn { 1.5 } else { 1.0 } * scale.max(1.0);
        let outer = if radius > 0.0 { radius + bw } else { 0.0 };
        l.quads.push(Quad { shape: Shape::Rect { rect: r.inflate(bw), radius: outer }, color: rgb(border, 1.0), texture: None });
        l.quads.push(Quad { shape: Shape::Rect { rect: r, radius }, color: rgb(th.panel(), 1.0), texture: None });
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

    /// One iteration of app work outside winit callbacks.
    pub fn tick(&mut self, dt: f32) {
        self.run_script();
        self.follow_desktop();
        self.handle_web_events();
        self.autosave();
        self.step_camera(dt);
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
        let ctx = web::menu::MenuContext { sticky: self.scene.node(id).is_some_and(Node::is_sticky), ..ctx.clone() };
        let items = web::menu::items(&ctx).into_iter().map(Entry::Page).collect();
        let m = self.layout_menu(id, items, anchor);
        log::info!("context menu on card {}: {:?}", id.0, menu_labels(&m.items));
        self.menu = Some(m);
        self.menu_pressed = false;
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
            MenuAction::Search(q) => open_beside(self, &omnibox::search_url(&q)),
            MenuAction::AddToNote(text) => self.add_to_note(id, &text),
            MenuAction::SaveMarkdown => self.save_markdown(Some(id)),
            MenuAction::ToggleSticky => self.toggle_sticky(id),
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

fn menu_layer(m: &ContextMenu, scale: f32, th: &Theme) -> Layer {
    let mut l = Layer::default();
    let radius = th.radius(scale / th.text_scale);
    l.quads.push(Quad {
        shape: Shape::Shadow { rect: m.rect.translate(Vec2::new(0.0, 4.0 * scale)), radius, blur: 18.0 * scale },
        color: th.shadow(0.55),
        texture: None,
    });
    let outer = if radius > 0.0 { radius + 1.0 } else { 0.0 };
    l.quads.push(Quad { shape: Shape::Rect { rect: m.rect.inflate(1.0), radius: outer }, color: rgb(th.divider(), 1.0), texture: None });
    l.quads.push(Quad { shape: Shape::Rect { rect: m.rect, radius }, color: rgb(th.panel(), 1.0), texture: None });
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
                Cmd::Wheel(x, y) => self.wheel(MouseScrollDelta::LineDelta(x, y)),
                Cmd::CtrlWheel(y) => {
                    let saved = self.mods;
                    self.mods = ModifiersState::CONTROL;
                    self.wheel(MouseScrollDelta::LineDelta(0.0, y));
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
                Cmd::Undo => self.undo(false),
                Cmd::Redo => self.undo(true),
                Cmd::Cards => {
                    for n in self.scene.all_nodes() {
                        let (r, f) = (n.rect, self.to_screen_rect(n.frame()));
                        log::info!(
                            "script card {}: world ({:.0},{:.0} {:.0}x{:.0}) screen ({:.0},{:.0} {:.0}x{:.0}) live={} sel={} hidden={} title={:?} {}",
                            n.id.0, r.x, r.y, r.w, r.h, f.x, f.y, f.w, f.h, self.cards.contains_key(&n.id),
                            self.selection.contains(n.id), self.scene.is_hidden(n.id), n.title, n.url
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
                            "script space {}: {:?} tint={:?} board={:?}{}",
                            e.id.0, e.name, e.tint, self.theme.board(e.tint.as_deref()), if e.id == self.spaces.active { " active" } else { "" }
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
                            scene::sidebar::Row::Header { label, count, collapsed, .. } => {
                                format!("header {label:?} count={count}{}", if *collapsed { " collapsed" } else { "" })
                            }
                            scene::sidebar::Row::Card { id, depth, folded, .. } => {
                                let title = self.scene.node(*id).map(|n| n.title.clone()).unwrap_or_default();
                                format!("card {} depth={depth}{} {title:?}", id.0, if *folded { " folded" } else { "" })
                            }
                        };
                        log::info!("script sidebar row {i} at {at}: {what}");
                    }
                }
                Cmd::Open(url) => {
                    self.new_card_at_center(&url);
                }
                Cmd::Log(msg) => log::info!("script log: {msg} (cards={}, edges={})", self.scene.nodes().count(), self.scene.edges().len()),
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

fn max_live_from_env() -> usize {
    match std::env::var("TRAILBOARD_MAX_LIVE").ok().map(|v| v.trim().parse::<usize>()) {
        None => DEFAULT_MAX_LIVE,
        Some(Ok(n)) if n >= 1 => n,
        Some(_) => {
            log::warn!("TRAILBOARD_MAX_LIVE must be a whole number ≥ 1; using {DEFAULT_MAX_LIVE}");
            DEFAULT_MAX_LIVE
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
        let restored = self.restore();
        let url = match self.initial_url.take() {
            Some(u) => Some(u),
            None if !restored => Some(START_URL.to_string()),
            None => None,
        };
        if let Some(url) = url
            && let Some(id) = self.new_card_at_center(&url)
                && !restored {
                    self.fill_view(id);
                    if let Some(goal) = self.camera_goal.take() {
                        self.camera = goal;
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
                if let Some(id) = self.hit(self.cursor).id()
                    && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, self.cursor)) {
                        card.page.mouse_move(x, y, 0, true);
                    }
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_input(state, button),
            WindowEvent::MouseWheel { delta, .. } => self.wheel(delta),
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => self.key(KeyIn::from_winit(&event)),
            WindowEvent::Ime(ime) => self.ime(ime),
            WindowEvent::Focused(false) => self.mods = ModifiersState::empty(),
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
}
