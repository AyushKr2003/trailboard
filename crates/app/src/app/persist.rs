//! Persistence.

use std::time::{Duration, Instant};

use storage::spaces;

use super::{App, Drag};

impl App {
    pub(crate) fn board_path(&self) -> std::path::PathBuf {
        spaces::space_board_path(&self.data_dir, self.spaces.active)
    }

    pub(crate) fn save(&mut self) {
        if self.renderer.is_none() || !self.persist {
            return; // never initialized; don't clobber a saved board with an empty one
        }
        let board = storage::Board::new(self.scene.without_inspectors(), self.camera);
        let path = self.board_path();
        match storage::save_board(&path, &board) {
            Ok(()) => self.dirty = false,
            Err(e) => log::error!("saving board to {}: {e}", path.display()),
        }
        self.save_places();
        self.last_save = Instant::now();
    }

    /// Jump lists and marks, if they changed.
    pub(crate) fn save_places(&mut self) {
        if !self.places_dirty || !self.persist {
            return;
        }
        match storage::jumps::save(&self.data_dir, &self.places) {
            Ok(()) => self.places_dirty = false,
            Err(e) => log::error!("saving jumps and marks: {e}"),
        }
    }

    pub(crate) fn save_index(&self) {
        if self.persist
            && let Err(e) = spaces::save_index(&self.data_dir, &self.spaces)
        {
            log::error!("saving the space index: {e}");
        }
    }

    pub(crate) fn autosave(&mut self) {
        if (self.dirty || self.places_dirty) && self.last_save.elapsed() > Duration::from_secs(3) && matches!(self.drag, Drag::None) {
            self.save();
        }
    }

    /// Open the space index and load the active space. Returns false when it has no cards.
    pub(crate) fn restore(&mut self) -> bool {
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

    pub(crate) fn is_private(&self) -> bool {
        self.spaces.active_entry().private
    }

    /// Before the active space's pages open: a private space gets its cookie
    /// jar (the one it had this session, or a new one) and its page zooms.
    pub(crate) fn enter_privacy(&mut self, jar: Option<web::Jar>, zooms: Option<std::collections::BTreeMap<String, f64>>) {
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
    pub(crate) fn load_board(&mut self) -> bool {
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
}
