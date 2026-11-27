//! Cards.

use super::*;

impl App {
    pub(super) fn open_card(&mut self, rect: Rect, url: &str, from: Option<NodeId>) -> Option<NodeId> {
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
    pub(super) fn touch(&mut self, id: NodeId) {
        self.use_clock += 1;
        self.used.insert(id, self.use_clock);
    }

    /// Start the live browser for an existing card (no-op if it is already live).
    pub(super) fn create_page(&mut self, id: NodeId) {
        self.touch(id);
        if self.cards.contains_key(&id) {
            return;
        }
        let Some(n) = self.scene.node(id) else { return };
        // Only `open_inspector` starts an inspector; a stray one (brought
        // back by undo) is closed by `check_inspectors`.
        if n.is_inspector() {
            return;
        }
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
                self.cards.insert(id, Card { page, loading: true, can_back: false, can_forward: false, hidden: false, cursor: web::Cursor::Default, fps: FULL_FPS, held: false });
            }
            None => log::error!("failed to create browser for card {}", id.0),
        }
    }

    /// First start: the welcome page, left of the first page (`beside`).
    /// Not focused, so the page keeps the keys.
    pub(super) fn add_guide(&mut self, beside: Option<NodeId>) -> NodeId {
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
    pub(super) fn add_start(&mut self) -> NodeId {
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
    pub(super) fn open_address_bar(&mut self) {
        let page = self.focused.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note());
        let url = page.filter(|n| n.url != scene::note::START_URL).map(|n| n.url.clone()).unwrap_or_default();
        let target = if page.is_some() { Target::Focused } else { Target::NewCard };
        self.omnibox = Some(Omnibox::new(&url, target));
    }

    pub(super) fn new_card(&mut self) {
        match self.settings.new_card {
            prefs::NewCard::AddressBar => self.omnibox = Some(Omnibox::new("", Target::NewCard)),
            prefs::NewCard::StartPage => {
                let id = self.add_start();
                self.ensure_visible(id);
            }
        }
    }

    /// Whether card `id` is a start page that nothing was typed in yet.
    pub(super) fn is_start(&self, id: NodeId) -> bool {
        self.scene.node(id).is_some_and(|n| !n.is_note() && n.url == scene::note::START_URL)
    }

    /// Something was typed in start page `id`: load it there, so the card
    /// becomes an ordinary page.
    pub(super) fn start_go(&mut self, id: NodeId, input: &str) {
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
    pub(super) fn engine_page_action(&mut self, id: NodeId, action: &str) -> bool {
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
    pub(super) fn start_icons(&self, recent: &[crate::start::Recent]) -> crate::start::Icons {
        let urls = self.start_page.pins().map(|p| p.url.as_str()).chain(recent.iter().map(|r| r.url.as_str()));
        crate::start::icons(urls, |site| self.icons.get(site).map(|i| i.icon.png()))
    }

    /// Start page `from` changed the pins or parts: keep them, and show
    /// them on the other live start pages.
    pub(super) fn set_start_page(&mut self, from: NodeId, start: scene::startpage::StartPage) {
        if start == self.start_page {
            return;
        }
        log::info!(
            "start page: groups {:?}, greeting {} {:?}, clock {} (24h {}, seconds {}), pins {}, recent {}",
            start.groups.iter().map(|g| (g.name.as_str(), g.pins.len())).collect::<Vec<_>>(),
            start.show_greeting,
            start.name,
            start.show_clock,
            start.hour24,
            start.show_seconds,
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
    pub(super) fn set_engine(&mut self, e: prefs::SearchEngine) {
        if self.settings.search == e {
            return;
        }
        self.settings.search = e;
        log::info!("setting Search = {}", e.name());
        self.save_settings();
        self.engine_changed();
    }

    /// Tells live start pages and the guide which engine is in use.
    pub(super) fn engine_changed(&self) {
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
    pub(super) fn show_engines(&mut self, from: NodeId) {
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
    pub(super) fn welcome_action(&mut self, action: crate::welcome::Action) {
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
    pub(super) fn show_guide(&mut self) {
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
    pub(super) fn new_note_at_center(&mut self) -> NodeId {
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
    pub(super) fn new_sticky_at_center(&mut self) -> NodeId {
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
    pub(super) fn toggle_sticky(&mut self, id: NodeId) {
        let Some(n) = self.scene.node(id).filter(|n| n.is_note()) else { return };
        let colour = if n.is_sticky() { None } else { Some(scene::note::STICKY_COLOURS[0].to_string()) };
        let old = self.scene.set_sticky(id, colour);
        self.history.restyled(id, old);
        self.restyled(id);
        let sticky = self.scene.node(id).is_some_and(Node::is_sticky);
        log::info!("note {} sticky {sticky}", id.0);
    }

    /// C on a sticky note: its next colour. Undoable.
    pub(super) fn cycle_sticky_colour(&mut self, id: NodeId) {
        let old = self.scene.cycle_sticky_colour(id);
        self.history.restyled(id, old);
        self.restyled(id);
        let colour = self.scene.node(id).and_then(|n| n.sticky.clone()).unwrap_or_default();
        log::info!("sticky note {} colour {colour}", id.0);
        self.toast(format!("Sticky note: {colour}"));
    }

    /// Note `id` became or stopped being a sticky note, or changed colour.
    pub(super) fn restyled(&mut self, id: NodeId) {
        self.recolour_note(id);
        self.dirty = true;
    }

    /// Gives a live note's editor the colours of the theme and its sticky colour.
    pub(super) fn recolour_note(&self, id: NodeId) {
        let (Some(n), Some(c)) = (self.scene.node(id), self.cards.get(&id)) else { return };
        if n.is_note() && c.page.kind() == web::PageKind::Note {
            c.page.run_script(&crate::note::theme_script(&self.theme, n.sticky.as_deref()));
        } else if c.page.kind() == web::PageKind::App {
            c.page.run_script(&crate::welcome::theme_script(&self.theme));
        }
    }

    /// Ctrl+S or *Save as Markdown*: the note `note` as its own file, else
    /// the whole board, in `Documents/Trailboard` (`storage::export`).
    pub(super) fn save_markdown(&mut self, note: Option<NodeId>) {
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
    pub(super) fn add_to_note(&mut self, src: NodeId, selection: &str) {
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
    pub(super) fn open_beside_current(&mut self, url: &str) {
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
    pub(super) fn load_here(&mut self, url: &str) {
        let Some(page) = self.focused_page() else { return };
        if page.kind() == web::PageKind::Web {
            page.load(url);
        } else if let Some(id) = self.focused.filter(|&id| self.is_start(id)) {
            self.start_go(id, url);
        } else {
            self.open_beside_current(url);
        }
    }

    /// p / P: the clipboard's text as an address (or a search) here /
    /// beside; pp / PP the primary selection's (`primary`).
    pub(super) fn open_clipboard(&mut self, beside: bool, primary: bool) {
        let text = if primary { self.clipboard.primary() } else { self.clipboard.text() }.unwrap_or_default();
        let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let Some(url) = self.settings.resolve_url(&line) else {
            self.toast(if primary { "The primary selection has no text to open" } else { "The clipboard has no text to open" });
            return;
        };
        log::info!("opening the {} {}", if primary { "primary selection" } else { "clipboard" }, if beside { "beside" } else { "here" });
        if beside { self.open_beside_current(&url) } else { self.load_here(&url) }
    }

    pub(super) fn new_card_at_center(&mut self, url: &str) -> Option<NodeId> {
        let center = self.camera.center;
        let rect = placement::free_near(&self.scene, center, self.card_size.x, self.card_size.y);
        let id = self.open_card(rect, url, None)?;
        self.focus(Some(id));
        self.ensure_visible(id);
        Some(id)
    }

    /// Trailboard was started again while running: open what it was given as
    /// new cards in the space on screen and ask for the window to come forward.
    pub(super) fn relaunched(&mut self, args: &[String], cwd: &str) {
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

    pub(super) fn close_card(&mut self, id: NodeId) {
        self.close_cards(&[id]);
    }

    /// Closes cards as one undo step; each can still be reopened with Ctrl+Shift+T.
    pub(super) fn close_cards(&mut self, ids: &[NodeId]) {
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
            // An inspector can't be reopened: it isn't kept with the closed cards.
            if self.scene.node(id).is_some_and(Node::is_inspector) {
                self.close_inspector(id);
                continue;
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
    pub(super) fn sleep_page(&mut self, id: NodeId) {
        self.close_dock(id);
        self.media.remove(&id);
        if let Some(card) = self.cards.remove(&id) {
            self.release(card.page);
            log::info!("sleeping card {} ({} live)", id.0, self.cards.len());
        }
    }

    /// Stop the live browser of a card that has left the scene.
    pub(super) fn drop_page(&mut self, id: NodeId) {
        self.close_dock(id);
        self.dirty = true;
        if let Some(card) = self.cards.remove(&id) {
            self.release(card.page);
        }
        self.used.remove(&id);
        self.media.remove(&id);
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
    pub(super) fn reopen_closed(&mut self) -> bool {
        while let Some((node, edges)) = self.closed.pop() {
            if self.restore_closed(node, edges) {
                return true;
            }
        }
        false
    }

    /// A closed card picked in the palette: bring back that one, where it was.
    pub(super) fn reopen_closed_card(&mut self, id: NodeId) {
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
    pub(super) fn restore_closed(&mut self, node: Node, edges: Vec<Edge>) -> bool {
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

    pub(super) fn undo(&mut self, redo: bool) {
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

    pub(super) fn focus(&mut self, id: Option<NodeId>) {
        if self.focused == id {
            return;
        }
        self.keys_in_page = false;
        self.link_hints = None;
        self.finish_arrange(true);
        if let Some(old) = self.focused.and_then(|o| self.cards.get(&o)) {
            old.page.set_focus(false);
        }
        if let Some(d) = self.focused.and_then(|o| self.docks.get(&o)) {
            d.page.set_focus(false);
        }
        self.dock_keys = false;
        if let (Some(old), Some(_)) = (self.focused, id) {
            self.alt_card = Some(old);
            // Going to another card is a jump: '' comes back here.
            self.before = Some(Before::Board(self.board_spot()));
        }
        self.focused = id;
        if let Some(id) = id {
            if !self.jumping {
                self.places.space(self.spaces.active.0).jumps.visit(id);
                self.places_dirty = true;
            }
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

    pub(super) fn focused_page(&self) -> Option<&Page> {
        let id = self.focused?;
        if self.dock_keys
            && let Some(d) = self.docks.get(&id)
        {
            return Some(&d.page);
        }
        self.cards.get(&id).map(|c| &c.page)
    }

    /// Keys go to the focused page (after a click in it, Enter, or in Read
    /// Mode); otherwise they are board keys and the focused card is just the
    /// current one.
    pub(super) fn typing(&self) -> bool {
        self.entered() && matches!(self.page_keys, PageKeys::Insert | PageKeys::Passthrough)
    }

    /// Caret or Visual mode in the entered card (`v`, `V`).
    pub(super) fn visual_mode(&self) -> Option<caret::Mode> {
        match self.page_keys {
            PageKeys::Visual(m) if self.entered() => Some(m),
            _ => None,
        }
    }

    /// The entered card's mode as its ring shows it (`Theme::keys_ring`).
    pub(super) fn ring_keys(&self) -> crate::theme::Keys {
        use crate::theme::Keys;
        match self.page_keys {
            PageKeys::Page => Keys::Page,
            PageKeys::Insert => Keys::Insert,
            PageKeys::Passthrough => Keys::Passthrough,
            PageKeys::Visual(_) => Keys::Caret,
        }
    }

    /// A card is entered: its page has the keys (Page, Insert or
    /// Passthrough), after Enter or a click in it, or in Solo.
    pub(super) fn entered(&self) -> bool {
        self.focused_page().is_some() && (self.keys_in_page || self.read.is_some())
    }

    /// Page mode: a card is entered and our page keys come first.
    pub(super) fn page_mode(&self) -> bool {
        self.entered() && self.page_keys == PageKeys::Page
    }

    /// The focused card takes the keys, in `mode`.
    pub(super) fn give_keys(&mut self, mode: PageKeys) {
        self.keys_in_page = true;
        // The inspector has its own keys for everything; Shift+Esc leaves.
        let mode = if self.on_inspector() {
            PageKeys::Passthrough
        } else if mode == PageKeys::Page {
            self.site_keys(self.focused)
        } else {
            mode
        };
        self.set_page_keys(mode);
    }

    /// Page, or Passthrough for a card whose site starts there (`:site
    /// passthrough`) and for the inspector, which has keys for everything.
    pub(super) fn site_keys(&self, id: Option<NodeId>) -> PageKeys {
        let node = id.and_then(|id| self.scene.node(id));
        if node.is_some_and(Node::is_inspector) {
            return PageKeys::Passthrough;
        }
        let url = node.filter(|n| !n.is_note()).map(|n| n.url.as_str());
        match url {
            Some(u) if self.site_rules().allows(u, scene::sitesettings::Content::Passthrough, false) => PageKeys::Passthrough,
            _ => PageKeys::Page,
        }
    }

    pub(super) fn set_page_keys(&mut self, mode: PageKeys) {
        if self.page_keys != mode {
            log::info!("keys: {mode:?}");
        }
        if !matches!(mode, PageKeys::Visual(_)) {
            self.end_caret();
        }
        self.page_keys = mode;
        self.pending.clear();
    }

    /// The mode a card starts in when entered: Insert where you type
    /// (notes, the start page), Page elsewhere.
    pub(super) fn entry_mode(&self, id: NodeId) -> PageKeys {
        if self.scene.node(id).is_some_and(Node::is_note) || self.is_start(id) {
            PageKeys::Insert
        } else {
            self.site_keys(Some(id))
        }
    }

    /// The card board keys act on: the current one, else the one in the
    /// middle of the view.
    pub(super) fn current(&self) -> Option<NodeId> {
        read::pick(&self.scene, self.focused, self.camera.center)
    }

    /// h/j/k/l or arrows: make the nearest card that way current and bring it
    /// on screen. With no current card, the one in the middle of the view
    /// becomes current first.
    pub(super) fn move_current(&mut self, dir: Dir) {
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
    pub(super) fn pan_by_key(&mut self, dir: Dir) {
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
    pub(super) fn zoom_by_key(&mut self, zoom_in: bool) {
        let factor = if zoom_in { 1.25 } else { 1.0 / 1.25 };
        self.camera_goal = None;
        let vp = self.viewport();
        self.camera.zoom_about(factor, vp * 0.5, vp, self.scale());
        self.camera_moved();
    }

    /// Enter or Space: give the keys to the current card's page (a note opens
    /// for writing).
    pub(super) fn enter_current(&mut self) {
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
    pub(super) fn leave_page(&mut self) {
        self.keys_in_page = false;
        if let Some(p) = self.focused_page() {
            p.set_focus(false);
        }
        log::info!("left the page of card {:?}", self.focused.map(|f| f.0));
    }

    /// Runs a board key's action (`scene::keys::board`). A count repeats the
    /// actions that make sense more than once (`3j`); the guards are the ones
    /// each key always had (most wait while a drag is on).
    pub(super) fn board_action(&mut self, action: KeyAction, count: Option<u32>) {
        /// More repeats than this do nothing more on a board.
        const MAX_REPEAT: u32 = 100;
        let times = if action.repeats() { count.unwrap_or(1).clamp(1, MAX_REPEAT) } else { 1 };
        let idle = self.drag_is_idle();
        log::debug!("board key: {action:?} ×{times}");
        match action {
            KeyAction::Line(n) => self.run_user_line(n, count),
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
            KeyAction::JoinTrail if idle => self.start_card_pick(false),
            KeyAction::JoinTrailKeep if idle => self.start_card_pick(true),
            KeyAction::LinkCard if idle => self.start_link_pick(),
            KeyAction::LeaveTrail if idle => self.leave_trail(),
            KeyAction::Fold if idle => self.toggle_fold(),
            KeyAction::Outline if idle => self.outline_in_view(),
            KeyAction::StackPrev if idle => self.peek_stack(-1),
            KeyAction::StackNext if idle => self.peek_stack(1),
            KeyAction::Undo => self.undo(false),
            KeyAction::Redo => self.undo(true),
            KeyAction::CommandLine => self.open_command_line(count),
            KeyAction::RepeatLine => self.repeat_line(count),
            KeyAction::Fullscreen => self.toggle_fullscreen(),
            KeyAction::Inspect => self.toggle_inspector(None),
            KeyAction::Media(op) => self.media_op(op.times(count.unwrap_or(1))),
            _ if idle && self.card_key(action, count) => {}
            // A page key from a user line on the board: it acts on the
            // current card's page.
            a if idle && self.focused_page().is_some() && keys::page().has(a) => self.page_action(a, count),
            _ => {}
        }
    }

    /// Keys that go from card to card (M12 step 7), on the board and in a
    /// page; false for any other action. From a page, the keys stay with
    /// the page of the card gone to, in Page mode.
    pub(super) fn card_key(&mut self, action: KeyAction, count: Option<u32>) -> bool {
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
                self.open_command_line(None);
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
            KeyAction::JumpBack | KeyAction::JumpForward => self.walk_jumps(action == KeyAction::JumpBack, times as usize),
            KeyAction::SetMark(c) => self.set_mark(c),
            KeyAction::GoMark(c) => self.go_mark(c),
            KeyAction::OpenSaved | KeyAction::OpenSavedBeside => self.ask_saved(action == KeyAction::OpenSavedBeside),
            KeyAction::DarkPages => {
                let change = scene::setcmd::dark_pages(&self.settings, self.theme.light());
                self.apply_setting_change(scene::setcmd::Setting::Key(prefs::Key::PageColours), change);
            }
            KeyAction::SpacePanel => {
                let x = scene::setcmd::Setting::Key(prefs::Key::SpacePanel);
                if let Ok(change) = scene::setcmd::toggle(&self.settings, x, &[]) {
                    self.apply_setting_change(x, change);
                }
            }
            KeyAction::SiteToggle(what, session) => self.switch_site(self.focused, what, None, session),
            KeyAction::Sidebar => {
                // From a page: back to the board, where the sidebar takes the keys.
                if self.read.is_none() {
                    self.leave_page();
                    self.toggle_sidebar();
                }
            }
            _ => return false,
        }
        true
    }
}
