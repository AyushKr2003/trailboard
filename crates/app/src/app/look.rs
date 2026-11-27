//! Look.

use super::*;

impl App {
    /// Window scale times the desktop's text size: overlays are laid out in this.
    pub(super) fn ui(&self) -> f32 {
        self.scale() * self.theme.text_scale
    }

    /// Start following the desktop's look (once the window exists).
    pub(super) fn start_desktop(&mut self) {
        let d = Desktop::new(crate::theme::state_dir(), crate::theme::user_shell_toml(), true);
        let t = d.read(&self.theme);
        self.desktop = Some(d);
        self.set_theme(t);
    }

    /// Tells pages light or dark, from the theme and the *Page colours* setting.
    pub(super) fn apply_page_scheme(&mut self) {
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

    pub(super) fn set_theme(&mut self, mut t: Theme) {
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

    pub(super) fn follow_desktop(&mut self) {
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
    pub(super) fn update_card_size(&mut self) {
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
    pub(super) fn ask_delete_space(&mut self) {
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

    pub(super) fn delete_space(&mut self) {
        self.confirm = None;
        let gone = self.spaces.active;
        let name = self.spaces.active_entry().name.clone();
        self.unload_space();
        self.sessions.remove(&gone);
        let Some(next) = self.spaces.remove(gone) else { return };
        self.places.forget_space(gone.0);
        self.places_dirty = true;
        if matches!(&self.before, Some(Before::Board(b)) if b.space == gone.0) {
            self.before = None;
        }
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
}
