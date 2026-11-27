//! Events from pages.

use super::*;

impl App {
    pub(super) fn handle_web_events(&mut self) {
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
                WebEvent::DialogAsked { page, id, kind, origin, message, default } => {
                    self.dialog_asked(page, id, kind, &origin, &message, &default)
                }
                WebEvent::FilesAsked { page, id, mode, default, accept, .. } => self.files_asked(page, id, mode, &default, accept),
                WebEvent::DialogWithdrawn { id } => self.dialog_withdrawn(id),
                WebEvent::Field { page, report } => {
                    if let Some(id) = self.node_of(page) {
                        self.field_report(id, report);
                    }
                }
                WebEvent::OpenBeside { source, url } => {
                    let Some(src) = self.node_of(source).or_else(|| self.dock_of(source)) else { continue };
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
                        n.url = scene::address::reported(&n.url, &url);
                        self.dirty = true;
                    }
                }
                WebEvent::CursorChanged { page, cursor } => {
                    if let Some(id) = self.dock_of(page) {
                        if let Some(d) = self.docks.get_mut(&id) {
                            d.cursor = cursor;
                        }
                        let over = match self.drag {
                            Drag::Dock { id: d, .. } => d == id,
                            Drag::None => self.hit(self.cursor) == Hit::Inspector(id),
                            _ => false,
                        };
                        if over {
                            self.set_cursor(cursor);
                        }
                        continue;
                    }
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
                WebEvent::Caret { page, report } => {
                    if let Some(id) = self.node_of(page) {
                        self.caret_report(id, report);
                    }
                }
                WebEvent::Focus { page, report } => {
                    if let Some(id) = self.node_of(page) {
                        self.page_focus(id, report);
                    }
                }
                WebEvent::Media { page, playing } => {
                    if let Some(id) = self.node_of(page) {
                        self.got_media(id, playing);
                    }
                }
                WebEvent::MediaDone { page, answer } => {
                    if let Some(id) = self.node_of(page) {
                        self.media_done(id, &answer);
                    }
                }
                WebEvent::MediaTrouble { page, kind } => {
                    if let Some(id) = self.node_of(page) {
                        self.media_trouble(id, &kind);
                    }
                }
                WebEvent::Fullscreen { page, on } => {
                    if let Some(id) = self.node_of(page) {
                        self.page_fullscreen(id, on);
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
                WebEvent::Pager { page, links } => {
                    if let Some(id) = self.node_of(page) {
                        self.got_pager(id, links);
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
                    // Nor find marks: Esc needn't clear them.
                    if loading && self.find_marks == Some(id) {
                        self.find_marks = None;
                    }
                    // A new document has no caret: Caret mode ends with the old one.
                    if loading && self.caret.is_some_and(|c| c.id == id) {
                        self.caret = None;
                        if self.focused == Some(id) && matches!(self.page_keys, PageKeys::Visual(_)) {
                            self.set_page_keys(PageKeys::Page);
                        }
                    }
                    if loading {
                        self.text_due.remove(&id);
                    } else {
                        self.text_due.insert(id, Instant::now() + TEXT_SETTLE);
                        self.restore_after_load(id);
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
    pub(super) fn read_due_texts(&mut self) {
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
    pub(super) fn record_visit(&mut self, url: &str, title: &str, text: &str) -> bool {
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
    pub(super) fn save_thumb(&mut self, id: NodeId, url: &str) {
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
    pub(super) fn load_thumb(&self, url: &str) -> Option<storage::thumbs::Thumb> {
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
    pub(super) fn load_preview(&mut self, url: Option<String>) -> Option<(u32, u32)> {
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
    pub(super) fn preview_url(&self, row: &palette::Row) -> Option<String> {
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
    pub(super) fn load_sleeping_thumbs(&mut self) {
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
    pub(super) fn set_icon(&mut self, page_url: &str, width: u32, height: u32, pixels: Vec<u8>) {
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
    pub(super) fn icon_key(&self, url: &str) -> Option<u64> {
        let key = self.icons.get(&scene::icon::site(url)?)?.key;
        self.renderer.as_ref().is_some_and(|r| r.has_page(key)).then_some(key)
    }
}
