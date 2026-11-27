//! Cookies.

use super::*;

impl App {
    /// *Clear cookies*: asks first, as it signs you out everywhere.
    pub(super) fn ask_clear_cookies(&mut self) {
        self.omnibox = None;
        self.confirm = Some(Confirm::ClearCookies);
        self.toast("Delete all cookies? Sites will sign you out · Enter deletes · Esc keeps them");
    }

    pub(super) fn clear_cookies(&mut self) {
        self.confirm = None;
        if web::prefs::clear_cookies() {
            log::info!("cookies cleared");
            self.toast("Cookies deleted");
        } else {
            self.toast("Couldn't delete cookies");
        }
    }

    /// The rows under a list field: the palette's or the history view's.
    pub(super) fn list_rows(&self, ob: &Omnibox) -> Vec<palette::Row> {
        if ob.target == Target::Photo {
            let current = self.spaces.active_entry().photo.as_deref();
            return palette::photo_rows(&ob.text, &self.photo_files, current, self.wallpaper.is_some());
        }
        if ob.target == Target::AllowedSites {
            return palette::allowed_rows(&ob.text, &self.blocking.allowed_sites(), &self.scene);
        }
        if ob.target == Target::Messages {
            return self.message_rows(&ob.text);
        }
        if ob.target == Target::Downloads {
            let missing = |p: &str| std::fs::symlink_metadata(p).is_err();
            return palette::download_rows(&ob.text, &self.downloads, missing, local_ms(now_ms()), local_ms);
        }
        if ob.target == Target::History {
            let visits = visits_for(&self.page_index, &self.visits_cache, &ob.text, true);
            return palette::history_rows(&ob.text, &self.scene, &visits, local_ms(now_ms()), local_ms);
        }
        if palette::is_command(ob.query()) {
            return palette::command_rows(ob.query(), &self.recent_commands, &self.keys.aliases, &|k, x| self.command_values(k, x));
        }
        let visits = visits_for(&self.page_index, &self.visits_cache, &ob.text, false);
        let closed: Vec<&Node> = self.closed.iter().map(|(n, _)| n).collect();
        palette::rows(&ob.text, &self.scene, &self.used, &closed, &self.spaces, &visits, &self.settings)
    }

    /// The panel the list under field `ob` is laid out in, for its `rows`:
    /// the palette's rows sit in labelled sections.
    pub(super) fn list_panel(&self, ob: &Omnibox, rows: &[palette::Row]) -> palette::Panel {
        match ob.target {
            Target::Palette => palette::Panel::new(self.viewport(), self.ui(), palette::sections(rows, ob.query())),
            _ => palette::Panel::plain(self.viewport(), self.ui(), rows.len()),
        }
    }

    /// Ctrl+H: the history view, every page kept, newest first.
    pub(super) fn open_history(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::History));
        log::info!("history open");
    }
}
