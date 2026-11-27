//! Sidebar.

use super::*;

impl App {
    /// The sidebar's rows and where they go.
    pub(super) fn sidebar_layout(&mut self) -> (Vec<scene::sidebar::Row>, Panel) {
        let rows = self.sidebar.rows(&self.scene);
        let panel = Panel::new(self.viewport(), self.ui(), &crate::sidebar::slots(&rows));
        (rows, panel)
    }

    /// The sidebar's sheet, for hit tests that don't need its rows.
    pub(super) fn sidebar_rect(&self) -> Rect {
        Panel::sheet(self.viewport(), self.ui())
    }

    /// The pointer resting on the window's left edge peeks the sidebar (no
    /// keys); it goes again once the pointer has been away from it a moment.
    pub(super) fn sidebar_peek_tick(&mut self) {
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
    pub(super) fn sidebar_shown(&self) -> bool {
        self.sidebar_open && self.read.is_none()
    }

    /// The sidebar takes board keys: open, given the keys, and no page has them.
    pub(super) fn sidebar_has_keys(&self) -> bool {
        self.sidebar_shown() && self.sidebar_keys && !self.entered()
    }

    /// B: opens the sidebar with the keys on the current card's row; closes
    /// it when it has the keys; gives it the keys when it is open without.
    pub(super) fn toggle_sidebar(&mut self) {
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

    pub(super) fn close_sidebar(&mut self) {
        self.sidebar_open = false;
        self.sidebar_peek = false;
        self.sidebar_spot = None;
        self.sidebar_keys = false;
        self.sidebar_hover = None;
        self.sidebar_press = None;
        log::info!("sidebar closed");
    }

    /// The sidebar takes the keys, highlighting the current card's row.
    pub(super) fn take_sidebar_keys(&mut self) {
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

    pub(super) fn sidebar_scroll_to_sel(&mut self) {
        let (rows, panel) = self.sidebar_layout();
        let i = self.sidebar.index(&rows);
        if i.is_some() {
            self.sidebar_at = i;
        }
        self.sidebar.scroll = panel.keep_visible(i, self.sidebar.scroll);
    }

    /// Keys while the sidebar has them. False for keys it leaves to the board.
    pub(super) fn sidebar_key(&mut self, code: Option<KeyCode>, shift: bool) -> bool {
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
            Some(KeyCode::Escape) => self.close_sidebar(),
            _ => return false,
        }
        self.sidebar_scroll_to_sel();
        true
    }

    /// Shift+J/K: moves the trail of the highlighted row `step` places among
    /// the pinned or the other trails. The highlight stays on its row.
    pub(super) fn sidebar_move_trail(&mut self, rows: &[scene::sidebar::Row], step: isize) {
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
    pub(super) fn move_trail(&mut self, head: NodeId, to: usize) {
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
    pub(super) fn sidebar_pin(&mut self, rows: &[scene::sidebar::Row]) {
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
    pub(super) fn sidebar_name(&mut self, rows: &[scene::sidebar::Row]) {
        use scene::sidebar::{Group, Row};
        match self.sidebar.index(rows).map(|i| &rows[i]) {
            Some(&Row::Card { id, .. } | &Row::Header { group: Group::Trail(id), .. }) => self.ask_trail_name_of(id),
            _ => self.toast("Pick a trail or a card to name"),
        }
    }

    /// Goes to card `id` from the sidebar: it is shown at its stack's front if
    /// folded away, becomes current and comes on screen (clear of the panel).
    pub(super) fn sidebar_go(&mut self, id: NodeId) {
        self.go_to_card(id);
        log::info!("sidebar: card {}", id.0);
    }

    /// Goes to card `id`: shown at its stack's front if folded away, made
    /// current and brought on screen (clear of the sidebar).
    pub(super) fn go_to_card(&mut self, id: NodeId) {
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
}
