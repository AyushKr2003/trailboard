//! Downloads.

use super::*;

impl App {
    /// Ctrl+J: the downloads view, newest first.
    pub(super) fn open_downloads(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::Downloads));
        log::info!("downloads open");
    }

    /// Chromium wants to save a file from page `page`: it goes to the folder
    /// the settings name under a free name, at once; or, set to ask, the
    /// path waits in a field (`Target::SaveAs`). A card that was opened for
    /// the link and never showed a page goes away again.
    pub(super) fn download_asked(&mut self, page: u64, id: u32, url: &str, suggested: &str, blank: bool) {
        // Chromium asks again when it resumes an interrupted download by
        // itself: same download, same file.
        if let Some(&(key, _)) = self.download_ids.get(&id)
            && let Some(d) = self.downloads.get(key)
        {
            log::info!("download {id}: resuming into {}", d.path);
            web::download::save_to(id, std::path::Path::new(&d.path));
            return;
        }
        let dir = match self.settings.download_to {
            prefs::DownloadTo::Desktop => storage::downloads::desktop_dir(),
            prefs::DownloadTo::Downloads | prefs::DownloadTo::Ask => storage::downloads::download_dir(),
        };
        let taken: HashSet<std::path::PathBuf> = self.downloads.running_paths().map(std::path::PathBuf::from).collect();
        let path = storage::downloads::target(&dir, suggested, &taken);
        if self.settings.download_to == prefs::DownloadTo::Ask {
            // One question at a time: an earlier download still asking is cancelled.
            if let Some(old) = self.save_as.take() {
                web::download::refuse(old.id);
                if old.blank {
                    self.close_blank_card(old.page);
                }
            }
            log::info!("download {id}: asking where to save {url}");
            self.save_as = Some(SaveAs { id, page, url: url.to_string(), blank });
            self.menu = None;
            self.omnibox = Some(Omnibox::new(&home_relative(&path), Target::SaveAs(id)));
            return;
        }
        self.start_download(page, id, url, &path, blank);
    }

    /// Enter in the save field: the download goes to the path typed (`~`
    /// for the home folder; a folder means the suggested name in it).
    pub(super) fn save_download_as(&mut self, id: u32, text: &str) {
        let Some(pending) = self.save_as.take().filter(|s| s.id == id) else { return };
        let text = text.trim();
        let home = std::env::var("HOME").unwrap_or_default();
        let mut path = match text.strip_prefix("~/") {
            Some(rest) => std::path::Path::new(&home).join(rest),
            None => std::path::PathBuf::from(text),
        };
        if text.is_empty() || path.is_relative() {
            web::download::refuse(id);
            self.toast("Download cancelled · give a full path, like ~/Downloads/file.pdf");
            if pending.blank {
                self.close_blank_card(pending.page);
            }
            return;
        }
        if path.is_dir() {
            let name = pending.url.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("download");
            path = path.join(name);
        }
        self.start_download(pending.page, id, &pending.url, &path, pending.blank);
    }

    /// Saves download `id` of `url` at `path` and lists it (Ctrl+J).
    pub(super) fn start_download(&mut self, page: u64, id: u32, url: &str, path: &std::path::Path, blank: bool) {
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            log::warn!("downloads: making {}: {e}", dir.display()); // Chromium will fail it
        }
        let path = path.to_path_buf();
        if !web::download::save_to(id, &path) {
            return;
        }
        let key = self.downloads.start(url, &path.to_string_lossy(), self.is_private(), now_ms());
        self.download_ids.insert(id, (key, page));
        log::info!("download {id}: {url} -> {}", path.display());
        self.save_downloads();
        let name = self.downloads.get(key).map(|d| d.name().to_string()).unwrap_or_default();
        self.toast(format!("Downloading {name} · Ctrl+J"));
        if blank {
            self.close_blank_card(page);
        }
    }

    /// Closes the card of page `page`, opened for a link that turned out to
    /// be a file, so it never showed anything (its page stays parked until
    /// the download ends, if there is one).
    pub(super) fn close_blank_card(&mut self, page: u64) {
        if let Some(card) = self.node_of(page)
            && self.scene.node(card).is_some_and(|n| !n.is_note())
        {
            // Nothing to show or undo.
            log::info!("closing card {} (it only downloaded a file)", card.0);
            let source = self.scene.edges().iter().find(|e| e.to == card).map(|e| e.from);
            let was_current = self.focused == Some(card);
            self.scene.remove(card);
            self.drop_page(card);
            // The view moved to the new card when it opened; go back to the
            // page the link was on.
            if let Some(src) = source.filter(|&s| self.scene.node(s).is_some()) {
                self.ensure_visible(src);
                if was_current {
                    self.focus(Some(src));
                }
            }
        }
    }

    pub(super) fn download_updated(&mut self, id: u32, received: i64, total: i64, progress: web::download::Progress) {
        use scene::download::State;
        let Some(&(key, _)) = self.download_ids.get(&id) else { return };
        let state = match progress {
            web::download::Progress::Running => State::Running,
            web::download::Progress::Done => State::Done,
            web::download::Progress::Cancelled => State::Cancelled,
            web::download::Progress::Failed => State::Failed,
        };
        let Some(ended) = self.downloads.update(key, received, total, state) else { return };
        self.download_ids.remove(&id);
        let name = self.downloads.get(key).map(|d| d.name().to_string()).unwrap_or_default();
        log::info!("download {id}: {} {received} bytes", ended.id());
        match ended {
            State::Done => self.toast(format!("Downloaded {name} · Ctrl+J")),
            State::Failed => self.toast(format!("Download failed: {name}")),
            _ => self.toast(format!("Cancelled {name}")),
        }
        self.save_downloads();
        // Pages kept only for their downloads can close now.
        let busy: HashSet<u64> = self.download_ids.values().map(|&(_, page)| page).collect();
        let before = self.parked.len();
        self.parked.retain(|p| busy.contains(&p.key()));
        if self.parked.len() < before {
            log::info!("closed {} page(s) kept for downloads", before - self.parked.len());
        }
    }

    /// Closes a page, unless it has a download running: then it is kept,
    /// unseen, until that ends (see `parked`).
    pub(super) fn release(&mut self, page: Page) {
        let key = page.key();
        if self.download_ids.values().any(|&(_, p)| p == key) {
            log::info!("page {key} keeps running for its download");
            self.parked.push(page);
        }
    }

    /// At quit: running downloads can't go on without us, so they are
    /// cancelled (Chromium removes the partial files) and listed as stopped.
    pub(super) fn stop_downloads(&mut self) {
        use scene::download::State;
        if self.download_ids.is_empty() {
            return;
        }
        for (id, (key, _)) in std::mem::take(&mut self.download_ids) {
            web::download::cancel(id);
            if let Some(d) = self.downloads.get(key) {
                let (r, t) = (d.received, d.total);
                self.downloads.update(key, r, t, State::Stopped);
            }
        }
        self.save_downloads();
    }

    pub(super) fn save_downloads(&mut self) {
        if let Err(e) = storage::downloads::save_downloads(&self.data_dir, &self.downloads) {
            log::warn!("downloads: saving: {e}");
        }
    }

    /// Enter on a download: opens a finished file in its app; one that
    /// didn't finish is downloaded again (by the current page, any live one,
    /// or else a new card).
    pub(super) fn open_download(&mut self, key: u64) {
        use scene::download::State;
        let Some(d) = self.downloads.get(key).cloned() else { return };
        match d.state {
            State::Running => self.toast("Still downloading · Shift+Del cancels"),
            State::Done if std::fs::symlink_metadata(&d.path).is_err() => {
                self.toast(format!("{} was deleted or moved", d.name()));
            }
            State::Done => {
                log::info!("downloads: opening {}", d.path);
                open_externally(&d.path);
            }
            State::Cancelled | State::Failed | State::Stopped => {
                let page = self.focused_page().or_else(|| self.cards.values().map(|c| &c.page).next());
                match page {
                    Some(p) => {
                        log::info!("downloads: again {}", d.url);
                        p.download(&d.url);
                    }
                    // A card for it: it closes itself once the file starts (blank page).
                    None => {
                        log::info!("downloads: again in a new card {}", d.url);
                        self.new_card_at_center(&d.url);
                    }
                }
            }
        }
    }

    /// Ctrl+Enter on a download: opens the folder it is in.
    pub(super) fn show_download_folder(&mut self, key: u64) {
        let Some(d) = self.downloads.get(key) else { return };
        let dir = std::path::Path::new(&d.path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        log::info!("downloads: showing {dir}");
        open_externally(&dir);
    }

    /// Shift+Delete in the downloads view: cancels a running download, or
    /// takes a finished one off the list (the file stays).
    pub(super) fn drop_selected_download(&mut self) {
        let Some(ob) = self.omnibox.as_ref() else { return };
        let Some(palette::Item::Download(key)) = self.list_rows(ob).into_iter().nth(ob.sel).map(|r| r.item) else { return };
        let running = self.download_ids.iter().find(|(_, (k, _))| *k == key).map(|(&id, _)| id);
        match running {
            Some(id) => {
                log::info!("downloads: cancelling {id}");
                web::download::cancel(id);
            }
            None => {
                if let Some(d) = self.downloads.remove(key) {
                    log::info!("downloads: removed {} from the list", d.path);
                }
                self.save_downloads();
                let len = self.omnibox.as_ref().map_or(0, |ob| self.list_rows(ob).len());
                if let Some(ob) = self.omnibox.as_mut() {
                    ob.sel = ob.sel.min(len.saturating_sub(1));
                }
            }
        }
    }

    /// Shift+Delete in the palette or the history view: forgets the
    /// highlighted row's page.
    pub(super) fn forget_selected(&mut self) {
        let Some(ob) = self.omnibox.as_ref() else { return };
        let row = self.list_rows(ob).into_iter().nth(ob.sel);
        // A saved page in the `saved` list: forget it.
        let saved = match row.as_ref().map(|r| &r.item) {
            Some(palette::Item::Complete(line)) => scene::command::parse(line)
                .ok()
                .filter(|p| matches!(p.spec.cmd, scene::command::Cmd::Saved | scene::command::Cmd::SavedBeside))
                .and_then(|p| p.arg),
            _ => None,
        };
        let forgot_saved = saved.is_some_and(|name| {
            let gone = self.saved_pages.remove(&name);
            if let Some(p) = &gone {
                log::info!("saved pages: forgot {:?} ({})", p.name, p.url);
                self.save_saved();
            }
            gone.is_some()
        });
        if forgot_saved || matches!(row.as_ref().map(|r| &r.item), Some(palette::Item::Recent(_))) {
            if let Some(palette::Item::Recent(line)) = row.as_ref().map(|r| &r.item)
                && self.recent_commands.forget(line)
            {
                log::info!("command line: forgot {line:?}");
                self.save_recent_commands();
            }
            let panel = self.omnibox.as_ref().map(|ob| self.list_panel(ob, &self.list_rows(ob)));
            if let (Some(ob), Some(panel)) = (self.omnibox.as_mut(), panel) {
                ob.sel = ob.sel.min(panel.len().saturating_sub(1));
                ob.scroll = panel.keep_visible(ob.sel, ob.scroll);
            }
            return;
        }
        let Some(url) = row.and_then(|r| r.history) else { return };
        let Some(index) = self.page_index.as_mut() else { return };
        match index.forget(&url) {
            Ok(true) => {
                log::info!("history: forgot {url}");
                self.preview = None;
                // The row going away says it; a toast would wait under the list.
                self.visits_cache.replace(None);
            }
            Ok(false) => {}
            Err(e) => log::warn!("history: forgetting {url}: {e}"),
        }
        // Keep the highlight on the row that took its place.
        let panel = self.omnibox.as_ref().map(|ob| self.list_panel(ob, &self.list_rows(ob)));
        if let (Some(ob), Some(panel)) = (self.omnibox.as_mut(), panel) {
            ob.sel = ob.sel.min(panel.len().saturating_sub(1));
            ob.scroll = panel.keep_visible(ob.sel, ob.scroll);
        }
    }

    /// *Clear history*: asks first, as it can't be undone.
    pub(super) fn ask_clear_history(&mut self) {
        let n = match &self.page_index {
            None => return self.toast("History is off"),
            Some(index) => index.len().unwrap_or(0),
        };
        if n == 0 {
            return self.toast("History is empty");
        }
        let pages = if n == 1 { "1 page".to_string() } else { format!("{n} pages") };
        self.omnibox = None;
        self.confirm = Some(Confirm::ClearHistory);
        self.toast(format!("Forget all {pages} in history? Enter clears · Esc keeps them"));
    }

    pub(super) fn clear_history(&mut self) {
        self.confirm = None;
        let Some(index) = self.page_index.as_mut() else { return };
        // Pages already asked for their text would be kept again.
        self.text_due.clear();
        match index.clear() {
            Ok(n) => {
                log::info!("history: cleared {n} pages");
                self.preview = None;
                self.visits_cache.replace(None);
                self.toast("History cleared");
            }
            Err(e) => {
                log::warn!("history: clearing: {e}");
                self.toast("Couldn't clear history");
            }
        }
    }

    /// The toast when a question was answered no.
    pub(super) fn kept(&self, confirm: Confirm) -> String {
        match confirm {
            Confirm::DeleteSpace => format!("Kept {}", self.spaces.active_entry().name),
            Confirm::CloseOthers(_) => "Kept the cards".to_string(),
            Confirm::ClearHistory => "Kept history".to_string(),
            Confirm::ClearCookies => "Kept cookies".to_string(),
            Confirm::Permission | Confirm::Dialog => String::new(), // answered elsewhere
        }
    }

    /// A question was answered no (any key but Enter, or a click).
    pub(super) fn decline(&mut self, confirm: Confirm) {
        if confirm == Confirm::Permission {
            self.answer_ask(false);
        } else if confirm == Confirm::Dialog {
            self.answer_dialog(false, "");
        } else {
            self.toast(self.kept(confirm));
        }
    }

    pub(super) fn run_command(&mut self, c: Command) {
        match c {
            Command::NewCard => self.new_card(),
            Command::StartPage => {
                let id = self.add_start();
                self.ensure_visible(id);
            }
            Command::NewNote => {
                self.new_note_at_center();
            }
            Command::NewSticky => {
                self.new_sticky_at_center();
            }
            Command::Tidy => self.tidy_board(),
            Command::Fit => self.fit_all(),
            Command::Fold => self.toggle_fold(),
            Command::Outline => self.outline_in_view(),
            Command::NameTrail => self.ask_trail_name(),
            Command::ColourTrail => self.cycle_trail_colour(),
            Command::JoinTrail => self.start_card_pick(false),
            Command::JoinTrailKeep => self.start_card_pick(true),
            Command::LinkCard => self.start_link_pick(),
            Command::LeaveTrail => self.leave_trail(),
            Command::Sidebar => self.toggle_sidebar(),
            Command::Read => self.toggle_read(),
            Command::LinkHints => self.show_link_hints(),
            Command::Arrange => self.start_arrange(),
            Command::SaveMarkdown => {
                let note = self.focused.filter(|&id| self.scene.node(id).is_some_and(Node::is_note));
                self.save_markdown(note);
            }
            Command::Undo => self.undo(false),
            Command::Redo => self.undo(true),
            Command::Reopen => {
                self.reopen_closed();
            }
            Command::CloseCard => {
                if !self.selection.is_empty() {
                    let ids = self.selection.ids();
                    self.close_cards(&ids);
                } else if let Some(id) = self.focused {
                    self.close_card(id);
                } else {
                    self.toast("No card to close");
                }
            }
            Command::Reload => match self.focused_page().filter(|p| p.kind() == web::PageKind::Web) {
                Some(p) => p.reload(),
                None => self.toast("No page to reload"),
            },
            Command::CycleAwake | Command::MuteCard => match self.current() {
                Some(id) if c == Command::CycleAwake => self.set_awake(id, None),
                Some(id) => self.toggle_mute(id),
                None => self.toast("No card"),
            },
            Command::MediaToggle => self.media_op(scene::media::Op::Toggle),
            Command::MediaFull => self.media_op(scene::media::Op::Full),
            Command::FindInPage => self.open_find(false),
            Command::MarkCard => self.toggle_mark(),
            Command::FillView => self.fill_current(),
            Command::PageMenu => {
                let ev = KeyIn::from_name("menu", true, false).expect("menu key");
                self.page_menu_by_key(&ev);
            }
            Command::ZoomIn => self.zoom_page(1),
            Command::ZoomOut => self.zoom_page(-1),
            Command::ZoomReset => self.zoom_page(0),
            Command::SelectAll => self.selection.select_all(&self.scene),
            Command::NewSpace => self.new_space(false),
            Command::NewPrivateSpace => self.new_space(true),
            Command::PrivateSpace => self.toggle_private(),
            Command::NextSpace | Command::PreviousSpace => {
                let step = if c == Command::NextSpace { 1 } else { -1 };
                self.switch_space(self.spaces.neighbour(self.spaces.active, step));
            }
            Command::RenameSpace => {
                let name = self.spaces.active_entry().name.clone();
                self.omnibox = Some(Omnibox::new(&name, Target::RenameSpace));
            }
            Command::NextTint => self.cycle_tint(),
            Command::BackgroundPhoto => self.open_photo_list(),
            Command::DeleteSpace => self.ask_delete_space(),
            Command::History => self.open_history(),
            Command::ClearHistory => self.ask_clear_history(),
            Command::ClearCookies => self.ask_clear_cookies(),
            Command::Settings => self.open_settings(),
            Command::Downloads => self.open_downloads(),
            Command::Shortcuts => self.show_hints(),
            Command::Welcome => self.show_guide(),
            Command::ToggleBlocking => self.toggle_blocking(self.current()),
            Command::AllowedSites => self.open_allowed_sites(),
            Command::Messages => self.open_messages(),
            Command::Quit => {
                log::info!("quit (palette)");
                self.quit = true;
            }
        }
    }

    /// A press on the sidebar: a header collapses or expands its trail, a card
    /// row goes to that card. Either gives the sidebar the keys.
    pub(super) fn sidebar_click(&mut self, p: Vec2, button: MouseButton) {
        use scene::sidebar::{Group, Row};
        if button != MouseButton::Left {
            return;
        }
        use crate::sidebar::Spot;
        let (rows, panel) = self.sidebar_layout();
        let n = self.spaces.spaces.len();
        let closable = |i: usize| matches!(rows.get(i), Some(Row::Card { .. }));
        if let Some(spot) = panel.spot_at(p, n, self.sidebar.scroll, &closable) {
            match spot {
                Spot::Back | Spot::Forward | Spot::Reload => {
                    if let Some(c) = self.focused.and_then(|id| self.cards.get(&id)).filter(|c| c.page.kind() == web::PageKind::Web) {
                        match spot {
                            Spot::Back => c.page.back(),
                            Spot::Forward => c.page.forward(),
                            _ => c.page.reload(),
                        }
                    }
                }
                // A peek stays; a kept sheet goes.
                Spot::Keep if self.sidebar_peek => self.sidebar_peek = false,
                Spot::Keep => self.close_sidebar(),
                Spot::Address => self.open_address_bar(),
                Spot::SpaceName => self.open_sidebar_space_menu(panel.space_line()),
                Spot::NewCard => self.new_card(),
                Spot::Close(i) => {
                    if let Some(Row::Card { id, .. }) = rows.get(i) {
                        self.close_card(*id);
                    }
                    self.sidebar_spot = None;
                }
                Spot::Settings => {
                    self.close_sidebar();
                    self.open_settings();
                }
                Spot::NewSpace => {
                    self.close_sidebar();
                    self.new_space(false);
                }
                Spot::Space(c) => {
                    if let Some(to) = self.spaces.spaces.get(c).map(|s| s.id)
                        && to != self.spaces.active
                    {
                        self.switch_space(to);
                    }
                }
            }
            return;
        }
        if self.entered() {
            self.leave_page();
        }
        self.sidebar_keys = true;
        let Some(i) = panel.row_at(p, self.sidebar.scroll) else { return };
        self.sidebar.sel = Some(rows[i].key());
        self.sidebar_at = Some(i);
        match rows[i] {
            // A trail's header toggles on release, unless it was dragged. A
            // second click soon after names the trail, and takes back the
            // first click's toggle so the group stays as it was.
            Row::Header { group: Group::Trail(head), .. } => {
                let again = self.sidebar_toggled.take().is_some_and(|(t, h)| h == head && t.elapsed() < Duration::from_millis(400));
                if again {
                    self.sidebar.toggle(Group::Trail(head));
                    self.ask_trail_name_of(head);
                } else {
                    self.sidebar_press = Some(SidebarPress { head, at: p, dragging: false, to: None });
                }
            }
            Row::Header { group, .. } => self.sidebar.toggle(group),
            Row::Card { id, .. } => self.sidebar_go(id),
        }
    }

    /// The pointer moved with a trail's header pressed: past a few px it is
    /// a drag, and the row under the pointer says where the trail would go.
    pub(super) fn sidebar_press_moved(&mut self, p: Vec2) {
        let Some(mut press) = self.sidebar_press else { return };
        if !press.dragging && (p.y - press.at.y).abs() < 5.0 * self.ui() {
            return;
        }
        press.dragging = true;
        let (rows, panel) = self.sidebar_layout();
        press.to = panel
            .row_at(p, self.sidebar.scroll)
            .and_then(|i| scene::sidebar::drop_to(&self.scene, press.head, rows[i].group()));
        self.sidebar_press = Some(press);
    }

    /// Release after a press on a trail's header: a click toggles the group,
    /// a drag moves the trail where it was let go.
    pub(super) fn sidebar_release(&mut self) {
        let Some(press) = self.sidebar_press.take() else { return };
        if !press.dragging {
            self.sidebar.toggle(scene::sidebar::Group::Trail(press.head));
            self.sidebar_toggled = Some((Instant::now(), press.head));
        } else if let Some(to) = press.to {
            self.move_trail(press.head, to);
        }
    }

    /// While a trail's header is dragged: the y of the line where it would
    /// go (above the trail it would take the place of, moving up; below it,
    /// moving down), if that row is shown.
    pub(super) fn sidebar_drop_line(&self, rows: &[scene::sidebar::Row], panel: &Panel) -> Option<f32> {
        use scene::sidebar::{self, Group};
        let press = self.sidebar_press.filter(|p| p.dragging)?;
        let to = press.to?;
        let (from, _) = sidebar::position(&self.scene, press.head)?;
        let target = sidebar::groups(&self.scene)
            .into_iter()
            .filter_map(|(g, _)| match g {
                Group::Trail(h) if sidebar::drop_to(&self.scene, press.head, g) == Some(to) => Some(h),
                _ => None,
            })
            .next()?;
        let of_target = |r: &&scene::sidebar::Row| sidebar::trail_head(r) == Some(target);
        if to == from {
            return None;
        }
        let (index, below) = if to < from {
            (rows.iter().position(|r| of_target(&r))?, false)
        } else {
            (rows.iter().rposition(|r| of_target(&r))?, true)
        };
        let rect = panel.row_rect(index, self.sidebar.scroll)?;
        Some(if below { rect.bottom() } else { rect.y })
    }
}
