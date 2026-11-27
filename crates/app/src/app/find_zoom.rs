//! Find in page, page zoom.

use super::*;

impl App {
    /// Ctrl+F or `/` (`back` false), `?` (true): the find bar for the current
    /// card's page, with the last search selected (and run again, so its
    /// matches show at once). Searching back, typing and Enter go up the page.
    pub(super) fn open_find(&mut self, back: bool) {
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
        self.find_back = back;
        if !text.is_empty() {
            self.cards[&id].page.find(&text, !back, false);
        }
        log::info!("find open on card {}{}", id.0, if back { " (backward)" } else { "" });
    }

    /// Keeps the page being searched in step with the find bar: a closed bar
    /// (Esc, a click, another box) stops the search, and the bar closes when
    /// its card stops being the current one or its page goes to sleep.
    pub(super) fn sync_find(&mut self) {
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
            if let Some(old) = self.finding
                && let Some(c) = self.cards.get(&old)
            {
                c.page.stop_finding();
                // Its match stays selected until Esc.
                if !self.last_find.is_empty() {
                    self.find_marks = Some(old);
                }
            }
            if open.is_some() {
                self.find_marks = None;
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
    pub(super) fn zoom_page(&mut self, dir: i32) {
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
}
