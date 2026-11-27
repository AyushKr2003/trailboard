//! Spaces.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use scene::settings::self as prefs;
use scene::{Camera, Edge, History, Node, NodeId, Scene};
use storage::spaces::SpaceId;

use crate::app::{App, DOCK_BIT, Drag, KEY_ID_BITS, now_ms, page_key, TOAST_FOR};
use crate::ui::theme::rgb;

/// Per-space state that lives only for this session.
#[derive(Default)]
pub(crate) struct Session {
    pub(crate) history: History,
    pub(crate) closed: Vec<(Node, Vec<Edge>)>,
    /// A private space's cookie jar and page zooms, kept while the app runs.
    pub(crate) jar: Option<web::Jar>,
    pub(crate) zooms: Option<std::collections::BTreeMap<String, f64>>,
}

impl App {
    pub(crate) fn page_key(&self, id: NodeId) -> u64 {
        page_key(self.epoch, id)
    }

    /// The card a page key belongs to, if that page is from the current space.
    pub(crate) fn node_of(&self, key: u64) -> Option<NodeId> {
        (key >> KEY_ID_BITS == self.epoch && key & DOCK_BIT == 0).then_some(NodeId(key & ((1 << KEY_ID_BITS) - 1)))
    }

    /// The card whose docked inspector has page key `key`.
    pub(crate) fn dock_of(&self, key: u64) -> Option<NodeId> {
        (key >> KEY_ID_BITS == self.epoch && key & DOCK_BIT != 0).then_some(NodeId(key & (DOCK_BIT - 1)))
    }

    pub(crate) fn toast(&mut self, msg: impl Into<String>) {
        self.toast_for(msg, TOAST_FOR);
    }

    pub(crate) fn toast_for(&mut self, msg: impl Into<String>, show: Duration) {
        let msg = msg.into();
        log::info!("toast: {msg}");
        self.messages.push(&msg, now_ms());
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
    pub(crate) fn space_label(&self) -> String {
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
    pub(crate) fn unload_space(&mut self) {
        for id in self.inspectors.keys().copied().collect::<Vec<_>>() {
            self.close_inspector(id);
        }
        for id in self.docks.keys().copied().collect::<Vec<_>>() {
            self.close_dock(id);
        }
        self.finish_arrange(true);
        self.save();
        let mut keys: Vec<u64> = self.scene.all_nodes().map(|n| self.page_key(n.id)).collect();
        keys.extend(self.ghosts.drain(..).map(|g| g.1));
        self.glides.clear();
        self.gathers.clear();
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
        self.spot_ask = None;
        self.restore_due.clear();
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
    pub(crate) fn load_space(&mut self) {
        let s = self.sessions.remove(&self.spaces.active).unwrap_or_default();
        self.history = s.history;
        self.closed = s.closed;
        self.enter_privacy(s.jar, s.zooms);
        self.load_board();
        self.camera_moved();
        self.dirty = false;
        // Cards closed in an earlier run are gone for good; this run's closed
        // cards (reopened with u) keep theirs.
        let closed: HashSet<NodeId> = self.closed.iter().map(|(n, _)| n.id).collect();
        let before = self.places.spaces.get(&self.spaces.active.0).cloned();
        self.places.prune(self.spaces.active.0, |id| self.scene.node(id).is_some() || closed.contains(&id));
        if self.places.spaces.get(&self.spaces.active.0) != before.as_ref() {
            self.places_dirty = true;
        }
        log::info!("space {} {:?}: {} cards", self.spaces.active.0, self.spaces.active_entry().name, self.scene.nodes().count());
        self.toast(self.space_label());
    }

    pub(crate) fn switch_space(&mut self, to: SpaceId) {
        self.link_hints = None;
        self.card_pick = None;
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
    pub(crate) fn backdrop_colours(&self) -> (crate::ui::theme::Rgb, crate::ui::theme::Rgb) {
        let tint = self.spaces.active_entry().tint.clone();
        match self.settings.backdrop {
            prefs::Backdrop::Plain => (self.theme.board(tint.as_deref()), self.theme.board(tint.as_deref())),
            _ => self.theme.mood(tint.as_deref()),
        }
    }

    pub(crate) fn new_space(&mut self, private: bool) {
        self.link_hints = None;
        self.card_pick = None;
        let id = self.spaces.add();
        self.spaces.set_private(id, private);
        self.unload_space();
        self.spaces.active = id;
        self.save_index();
        self.load_space();
    }

    pub(crate) fn rename_space(&mut self, name: &str) {
        let id = self.spaces.active;
        if self.spaces.rename(id, name) {
            self.save_index();
            self.toast(format!("Renamed to {}", self.spaces.active_entry().name));
        }
    }

    /// Ctrl+Shift+P: make the active space private, or recorded again. Its
    /// pages reopen, so they move to (or out of) the private cookie jar.
    pub(crate) fn toggle_private(&mut self) {
        self.link_hints = None;
        self.card_pick = None;
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

    pub(crate) fn cycle_tint(&mut self) {
        self.spaces.cycle_tint(self.spaces.active);
        self.save_index();
        log::info!("space {} tint {:?}", self.spaces.active.0, self.spaces.active_entry().tint);
    }
}
