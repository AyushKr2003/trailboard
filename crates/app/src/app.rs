//! Board state and interaction: cards are live pages on an infinite canvas.
//!
//! Input model
//! - Drag empty board / middle-drag anywhere: pan. Wheel over board: pan. Ctrl+wheel: zoom.
//! - Click a card: focus it (keyboard and wheel go to the page). Drag its title bar to move,
//!   its bottom-right corner to resize; × closes; double-click the title bar to fill the view.
//! - Esc (with no omnibox) unfocuses the card. With no card focused: F fits all, N/T new card.
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
use scene::{liveness, placement, selection, Camera, Change, Edge, History, Node, NodeId, Rect, Scene, Selection, Vec2};
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
use crate::theme::{rgb, Theme};
use crate::keymap::{self, KeyIn};
use crate::menu::{self, ContextMenu};
use crate::omnibox::{self, Omnibox, Target};
use crate::script::{Cmd, Script};

const START_URL: &str = "https://en.wikipedia.org/wiki/Special:Random";
const CARD_W: f32 = 1024.0;
const CARD_H: f32 = 720.0;
const MIN_CARD: f32 = 240.0;
const RESIZE_GRIP: f32 = 18.0;
const CLOSE_W: f32 = 30.0;
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
                self.used = self.scene.nodes().map(|n| (n.id, n.z)).collect();
                self.use_clock = self.used.values().copied().max().unwrap_or(0);
                log::info!("restored {} cards from {}", self.scene.nodes().count(), board_path.display());
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
        self.save();
        let keys: Vec<u64> = self.scene.nodes().map(|n| self.page_key(n.id)).collect();
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
        self.keys_in_page = false;
        self.selection.clear();
        self.group.clear();
        self.drag = Drag::None;
        self.omnibox = None;
        self.menu = None;
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

    fn page_point(&self, id: NodeId, screen: Vec2) -> Option<(i32, i32)> {
        let n = self.scene.node(id)?;
        let w = self.to_world(screen);
        Some(((w.x - n.rect.x).round() as i32, (w.y - n.rect.y).round() as i32))
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
        let (url, r) = (n.url.clone(), n.rect);
        match Page::new(self.page_key(id), &url, (r.w as i32, r.h as i32), self.page_scale, &self.spawn_beside) {
            Some(page) => {
                self.cards.insert(id, Card { page, loading: true, hidden: false, cursor: web::Cursor::Default });
            }
            None => log::error!("failed to create browser for {url}"),
        }
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
        if let Some(old) = self.focused.and_then(|o| self.cards.get(&o)) {
            old.page.set_focus(false);
        }
        self.focused = id;
        if let Some(id) = id {
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

    // ---------------------------------------------------------------- camera

    fn fit_all(&mut self) {
        if let Some(b) = self.scene.bounds() {
            self.camera_goal = Some(Camera::fit(b, self.viewport(), self.scale(), 48.0 * self.scale()));
        }
    }

    fn fill_view(&mut self, id: NodeId) {
        if let Some(n) = self.scene.node(id) {
            let mut goal = Camera::fit(n.frame(), self.viewport(), self.scale(), 8.0 * self.scale());
            goal.zoom = goal.zoom.min(2.0);
            self.camera_goal = Some(goal);
        }
    }

    /// Glide just enough to bring card `id` on screen (fitting it with the focused card if possible).
    fn ensure_visible(&mut self, id: NodeId) {
        let Some(frame) = self.scene.node(id).map(|n| n.frame()) else { return };
        let vp = self.viewport();
        let on_screen = self.to_screen_rect(frame);
        if on_screen.x >= 0.0 && on_screen.y >= 0.0 && on_screen.right() <= vp.x && on_screen.bottom() <= vp.y {
            return;
        }
        let mut goal = self.camera;
        let need = Camera::fit(frame, vp, self.scale(), 32.0 * self.scale());
        if need.zoom < goal.zoom {
            goal.zoom = need.zoom;
        }
        goal.center = frame.center();
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
        let settled = self.camera_changed.elapsed() > Duration::from_millis(250) && self.camera_goal.is_none();
        self.sync_liveness(settled && !matches!(self.drag, Drag::Pan { .. }));
        if settled {
            let want = ((self.camera.zoom * self.scale()) * 4.0).round().clamp(2.0, 8.0) / 4.0;
            if (want - self.page_scale).abs() > f32::EPSILON {
                self.page_scale = want;
            }
        }
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let page_scale = self.page_scale;
        let mut updates = Vec::new();
        for n in self.scene.nodes() {
            let on_screen = self.to_screen_rect(n.frame()).intersects(&screen);
            updates.push((n.id, on_screen, (n.rect.w.round() as i32, n.rect.h.round() as i32)));
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
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let candidates: Vec<liveness::Candidate> = self
            .scene
            .nodes()
            .map(|n| liveness::Candidate {
                id: n.id,
                live: self.cards.contains_key(&n.id),
                visible: self.to_screen_rect(n.frame()).intersects(&screen),
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
                WebEvent::ContextMenu { page, x, y, context } => {
                    if let Some(id) = self.node_of(page) {
                        self.open_menu(id, x, y, &context);
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
        let w = self.to_world(screen);
        let Some(id) = self.scene.topmost_at(w) else { return Hit::Board };
        let n = self.scene.node(id).expect("hit node exists");
        let k = self.camera.pixels_per_unit(self.scale());
        let grip = RESIZE_GRIP / k.min(1.0);
        if w.y < n.rect.y {
            if w.x > n.rect.right() - CLOSE_W {
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
                self.drag = Drag::Pan { last: p };
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
                Hit::Close(id) => self.close_card(id),
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
                    Hit::Close(_) => web::Cursor::Pointer,
                    Hit::Content(id) => self.cards.get(&id).map_or(web::Cursor::Default, |c| c.cursor),
                    Hit::Board => web::Cursor::Default,
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
        if self.mods.control_key() {
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
        let f1 = ev.named == Some(NamedKey::F1) && !ctrl && !alt;
        if pressed && (f1 || (is_question && !ctrl && !alt && self.focused.is_none() && self.omnibox.is_none() && self.menu.is_none())) {
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
                    } else if let Some(url) = omnibox::resolve(&ob.text) {
                        match (ob.target, self.focused_page()) {
                            (Target::Focused, Some(page)) => page.load(&url),
                            _ => {
                                self.new_card_at_center(&url);
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
                    let url = self.focused.and_then(|id| self.scene.node(id)).map(|n| n.url.clone()).unwrap_or_default();
                    let target = if self.focused.is_some() { Target::Focused } else { Target::NewCard };
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
                Some(KeyCode::KeyZ) if ctrl && !self.keys_in_page => {
                    self.undo(shift);
                    true
                }
                Some(KeyCode::KeyY) if ctrl && !self.keys_in_page => {
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
                Some(KeyCode::KeyR) if ctrl => {
                    if let Some(p) = self.focused_page() {
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
                    if !self.selection.is_empty() && !self.keys_in_page && !ctrl && !alt && self.drag_is_idle() =>
                {
                    let ids = self.selection.ids();
                    log::info!("closing {} selected card(s)", ids.len());
                    self.close_cards(&ids);
                    true
                }
                Some(KeyCode::KeyA) if ctrl && !shift && self.focused.is_none() => {
                    self.selection.select_all(&self.scene);
                    true
                }
                Some(KeyCode::Escape) if self.focused.is_none() && !self.selection.is_empty() => {
                    self.selection.clear();
                    true
                }
                Some(KeyCode::Escape) if self.focused.is_some() && self.drag_is_idle() => {
                    self.selection.clear();
                    // Let the page see Esc too (closes its menus), then leave the page.
                    if let Some(p) = self.focused_page() {
                        p.send_key(&ev.to_web(self.mods));
                    }
                    self.focus(None);
                    true
                }
                // Board shortcuts when no page has focus.
                Some(KeyCode::KeyF) if self.focused.is_none() && !ctrl => {
                    self.fit_all();
                    true
                }
                Some(KeyCode::KeyN | KeyCode::KeyT) if self.focused.is_none() && !ctrl => {
                    self.omnibox = Some(Omnibox::new("", Target::NewCard));
                    true
                }
                _ => false,
            };
            if handled {
                return;
            }
        }
        if let Some(p) = self.focused_page() {
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
        for e in self.scene.edges() {
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
            let mut prev = p0;
            for i in 1..=24 {
                let t = i as f32 / 24.0;
                let u = 1.0 - t;
                let x = u * u * u * p0[0] + 3.0 * u * u * t * p1[0] + 3.0 * u * t * t * p2[0] + t * t * t * p3[0];
                let y = u * u * u * p0[1] + 3.0 * u * u * t * p1[1] + 3.0 * u * t * t * p2[1] + t * t * t * p3[1];
                edges.quads.push(Quad {
                    shape: Shape::Segment { from: prev, to: [x, y], width: (2.0 * k).clamp(1.0, 4.0) },
                    color: rgb(th.accent(), 0.7),
                    texture: None,
                });
                prev = [x, y];
            }
        }
        layers.push(edges);

        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let band = self.band();
        let in_band = band.map(|b| selection::in_band(&self.scene, b)).unwrap_or_default();
        for n in self.scene.draw_order() {
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
            l.quads.push(Quad {
                shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (22.0 * k).max(2.0) },
                color: th.shadow(if focused { 0.55 } else { 0.4 }),
                texture: None,
            });
            if self.selection.contains(n.id) || in_band.contains(&n.id) {
                // Selection ring outside the focus ring, so both can show.
                let m = if focused { 5.0 } else { 3.0 };
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: frame.inflate(m * scale), radius: ring(m * scale) },
                    color: rgb(th.marked(), 1.0),
                    texture: None,
                });
            }
            if focused {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: frame.inflate(2.0 * scale.max(1.0)), radius: ring(2.0 * scale.max(1.0)) },
                    color: rgb(th.accent(), 1.0),
                    texture: None,
                });
            }
            l.quads.push(Quad {
                shape: Shape::Rect { rect: frame, radius },
                color: rgb(th.card(focused), 1.0),
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
                    (false, _) => rgb(th.placeholder(), 1.0), // first paint pending, or asleep and never painted
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
            // Loading bar along the bottom of the title bar.
            if card.is_some_and(|c| c.loading) {
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
            if font >= 6.0 {
                let pad = 10.0 * k;
                let title = if n.title.is_empty() { n.url.as_str() } else { n.title.as_str() };
                let title = if asleep { format!("{title} · asleep") } else { title.to_string() };
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

        if let Some(ob) = &self.omnibox {
            let w = (720.0 * ui).min(vp.x - 32.0 * ui);
            let h = 48.0 * ui;
            let r = Rect::new((vp.x - w) / 2.0, 18.0 * ui, w, h);
            let size = 17.0 * ui;
            let pad = 16.0 * ui;
            let placeholder = match ob.target {
                Target::RenameSpace => "Space name",
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
            layers.push(menu_layer(m, ui, screen, &th));
        }

        // Space name in the top-left corner once there is more than one space.
        if self.spaces.spaces.len() > 1 && self.omnibox.is_none() {
            let label = self.space_label();
            let color = th.swatch(tint.as_deref());
            layers.push(self.pill(&label, Vec2::new(12.0 * ui, 12.0 * ui), 13.0 * ui, Some(color), false));
        }
        if let Some((msg, at)) = &self.toast {
            if self.confirm_delete || at.elapsed() < TOAST_FOR {
                if self.omnibox.is_none() {
                    let size = 15.0 * ui;
                    let msg = msg.clone();
                    let w = self.renderer.as_mut().map_or(300.0, |r| r.measure(&msg, size, false));
                    let pos = Vec2::new((vp.x - w) / 2.0 - 16.0 * ui, 18.0 * ui);
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
                let n = self.scene.node(id)?;
                let r = self.to_screen_rect(Rect::new(n.rect.x + x, n.rect.y + y, w.max(1.0), h.max(1.0)));
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
        let Some(n) = self.scene.node(id) else { return };
        let anchor = self.camera.world_to_screen(Vec2::new(n.rect.x + x, n.rect.y + y), self.viewport(), self.scale());
        let items = web::menu::items(ctx);
        let scale = self.ui();
        let label_w = match self.renderer.as_mut() {
            Some(r) => items
                .iter()
                .map(|i| match i {
                    MenuItem::Action { label, .. } => r.measure(label, menu::FONT * scale, false),
                    MenuItem::Separator => 0.0,
                })
                .fold(0.0, f32::max),
            None => 0.0,
        };
        let m = ContextMenu::new(id, items, anchor, label_w, self.viewport(), scale);
        log::info!("context menu on card {}: {:?}", id.0, menu_labels(&m.items));
        self.menu = Some(m);
        self.menu_pressed = false;
    }

    fn activate_menu_item(&mut self, i: usize) {
        let Some(m) = self.menu.take() else { return };
        let Some(MenuItem::Action { action, enabled: true, label }) = m.items.get(i).cloned() else { return };
        log::info!("menu: {label}");
        let id = m.page;
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

fn menu_labels(items: &[MenuItem]) -> Vec<&str> {
    items
        .iter()
        .map(|i| match i {
            MenuItem::Action { label, .. } => label.as_str(),
            MenuItem::Separator => "-",
        })
        .collect()
}

fn menu_layer(m: &ContextMenu, scale: f32, screen: Rect, th: &Theme) -> Layer {
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
        match item {
            MenuItem::Separator => {
                let y = row.y + row.h / 2.0;
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(row.x + 4.0 * scale, y, row.w - 8.0 * scale, scale.max(1.0)), radius: 0.0 },
                    color: rgb(th.divider(), 1.0),
                    texture: None,
                });
            }
            MenuItem::Action { label, enabled, .. } => {
                if m.hover == Some(i) {
                    l.quads.push(Quad { shape: Shape::Rect { rect: *row, radius: th.small_radius(scale / th.text_scale) }, color: rgb(th.selection(), 1.0), texture: None });
                }
                l.texts.push(Text {
                    text: label.clone(),
                    pos: [row.x + (menu::TEXT_PAD - menu::PAD) * scale, row.y + (row.h - size * 1.3) / 2.0],
                    size,
                    color: rgb(if *enabled { th.foreground() } else { th.dim_text() }, 1.0),
                    clip: screen,
                    bold: false,
                });
            }
        }
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
                    let Some(n) = self.scene.node(NodeId(id)) else {
                        log::warn!("script: no card {id}");
                        continue;
                    };
                    let p = self.camera.world_to_screen(Vec2::new(n.rect.x + x, n.rect.y + y), self.viewport(), self.scale());
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
                    for n in self.scene.nodes() {
                        let (r, f) = (n.rect, self.to_screen_rect(n.frame()));
                        log::info!(
                            "script card {}: world ({:.0},{:.0} {:.0}x{:.0}) screen ({:.0},{:.0} {:.0}x{:.0}) live={} sel={} title={:?} {}",
                            n.id.0, r.x, r.y, r.w, r.h, f.x, f.y, f.w, f.h, self.cards.contains_key(&n.id),
                            self.selection.contains(n.id), n.title, n.url
                        );
                    }
                    for e in self.scene.edges() {
                        log::info!("script edge {} -> {}", e.from.0, e.to.0);
                    }
                }
                Cmd::Spaces => {
                    for e in &self.spaces.spaces {
                        log::info!(
                            "script space {}: {:?} tint={:?} board={:?}{}",
                            e.id.0, e.name, e.tint, self.theme.board(e.tint.as_deref()), if e.id == self.spaces.active { " active" } else { "" }
                        );
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
}

impl Hit {
    fn id(&self) -> Option<NodeId> {
        match *self {
            Hit::Board => None,
            Hit::Header(id) | Hit::Close(id) | Hit::Grip(id) | Hit::Content(id) => Some(id),
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
