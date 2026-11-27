//! Messages.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use scene::keys::{self, Action as KeyAction};
use scene::liveness::Awake;
use scene::userkeys::{self, Step};
use scene::{hint, jump, Node, NodeId, read};

use crate::app::{App, local_ms, now_ms, PageKeys};
use crate::ui::omnibox::{Omnibox, Target};
use crate::ui::palette::{self, Command, Item};

/// A `:later` line waiting to run.
pub(crate) struct LaterLine {
    /// Stays the same while it waits (the messages list's row).
    pub(crate) id: u64,
    pub(crate) at: Instant,
    pub(crate) line: String,
    /// The hint's link it had (`{hint-url}`).
    pub(crate) hint_url: Option<String>,
}

impl App {
    /// `:messages`: the toasts shown this session and the `:later` lines
    /// waiting, in a list (Enter copies a message or runs a line now,
    /// Shift+Delete forgets or cancels it).
    pub(crate) fn open_messages(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::Messages));
        log::info!("messages open: {} shown, {} waiting", self.messages.len(), self.later.len());
    }

    /// The messages list's rows.
    pub(crate) fn message_rows(&self, query: &str) -> Vec<palette::Row> {
        let now = Instant::now();
        let waiting: Vec<palette::Waiting> = self
            .later
            .iter()
            .map(|l| palette::Waiting { id: l.id, line: &l.line, left_ms: l.at.saturating_duration_since(now).as_millis() as u64 })
            .collect();
        palette::message_rows(query, &waiting, &self.messages, local_ms(now_ms()), local_ms)
    }

    /// Shift+Delete in the messages list: forgets a message, or cancels a
    /// waiting line. The list stays, its highlight kept in range.
    pub(crate) fn drop_message_row(&mut self, item: &Item) {
        match item {
            Item::Message(id) => {
                self.messages.remove(*id);
                log::info!("message {id} forgotten");
            }
            Item::Waiting(id) => {
                if let Some(i) = self.later.iter().position(|l| l.id == *id) {
                    let l = self.later.remove(i);
                    log::info!("later {:?} cancelled", l.line);
                    // Said on the toast, which is logged too.
                    self.toast(format!("Cancelled :{}", l.line));
                }
            }
            _ => return,
        }
        let n = self.omnibox.as_ref().map(|ob| self.message_rows(&ob.text).len()).unwrap_or(0);
        if let Some(ob) = self.omnibox.as_mut() {
            ob.sel = ob.sel.min(n.saturating_sub(1));
        }
    }

    /// The command line again, holding `line` (a completion to finish).
    pub(crate) fn fill_command_line(&mut self, line: &str) {
        let mut ob = Omnibox::new(&format!(":{line}"), Target::Palette);
        ob.select_all = false;
        self.omnibox = Some(ob);
    }

    /// Values the command line offers for an argument: the spaces' names
    /// (this one last) or the cards' titles (most recently used first).
    pub(crate) fn command_values(&self, kind: scene::command::Kind, setting: &str) -> Vec<(String, String)> {
        use scene::command::Kind;
        use scene::setcmd;
        let plain = |v: Vec<String>| v.into_iter().map(|v| (v, String::new())).collect();
        match kind {
            Kind::Setting if setting.is_empty() => setcmd::name_rows(&self.settings_in_force()),
            Kind::Setting => setcmd::find(setting).map_or_else(|_| Vec::new(), |x| setcmd::value_rows(&self.settings_in_force(), x)),
            Kind::Saved => self.saved_pages.pages().iter().map(|p| (p.name.clone(), palette::host(&p.url).to_string())).collect(),
            Kind::Space => {
                let mut names: Vec<String> = self.spaces.spaces.iter().filter(|s| s.id != self.spaces.active).map(|s| s.name.clone()).collect();
                names.push(self.spaces.active_entry().name.clone());
                plain(names)
            }
            Kind::Card => {
                let mut cards: Vec<&Node> = self.scene.all_nodes().filter(|n| !n.title.trim().is_empty()).collect();
                cards.sort_by_key(|n| std::cmp::Reverse((self.used.get(&n.id).copied().unwrap_or(0), n.z)));
                let mut seen = HashSet::new();
                plain(cards.into_iter().map(|n| n.title.trim().to_string()).filter(|t| seen.insert(t.clone())).collect())
            }
            _ => Vec::new(),
        }
    }

    /// The card `typed` names: a number counts in reading order (`3`), else
    /// the best title, the card used most recently first.
    pub(crate) fn card_named(&self, typed: &str) -> Option<NodeId> {
        if let Ok(n) = typed.trim().parse::<usize>() {
            return n.checked_sub(1).and_then(|i| scene::tidy::order(&self.scene).get(i).copied());
        }
        let cards: Vec<(NodeId, String)> = self.scene.all_nodes().map(|n| (n.id, n.title.trim().to_string())).collect();
        // Most recently used first, so the same title goes to the card used last.
        let mut order: Vec<usize> = (0..cards.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.used.get(&cards[i].0).copied().unwrap_or(0)));
        let sorted: Vec<String> = order.iter().map(|&i| cards[i].1.clone()).collect();
        Self::best_name(typed, &sorted).map(|i| cards[order[i]].0)
    }

    /// The best of `names` for `typed`: the same name (case ignored), else
    /// the best fuzzy match.
    pub(crate) fn best_name(typed: &str, names: &[String]) -> Option<usize> {
        if let Some(i) = names.iter().position(|n| n.trim().eq_ignore_ascii_case(typed.trim())) {
            return Some(i);
        }
        let cands: Vec<scene::palette::Candidate> = names.iter().map(|n| scene::palette::Candidate { text: n, alt: "" }).collect();
        scene::palette::rank(typed, &cands).first().map(|r| r.index)
    }

    pub(crate) fn save_recent_commands(&self) {
        if let Err(e) = storage::commands::save_recent(&self.data_dir, &self.recent_commands) {
            log::warn!("command line: saving recent lines: {e}");
        }
    }

    /// Runs a typed command line: a chain of commands (`|`), aliases and
    /// key names (`scene::userkeys`); a line that isn't one says why.
    /// Lines are remembered, except in a private space.
    pub(crate) fn run_line(&mut self, line: &str) {
        let steps = match userkeys::steps(line, &self.keys.aliases) {
            Ok(s) => s,
            Err(e) => {
                log::info!("command line: {line:?}: {e}");
                self.toast(e);
                return;
            }
        };
        let line = line.trim().trim_start_matches(':').trim();
        self.last_line = Some(line.to_string());
        if !self.is_private() {
            self.recent_commands.push(line);
            self.save_recent_commands();
        }
        self.run_steps(&steps, None);
    }

    /// Runs the steps of a chain in turn; `hint_url` fills `{hint-url}`.
    pub(crate) fn run_steps(&mut self, steps: &[Step], hint_url: Option<&str>) {
        for s in steps {
            match s {
                Step::Action(a) => self.run_key_action(*a, None),
                Step::Line(l) => self.exec_line(l, hint_url),
            }
        }
    }

    /// `{url}`, `{title}` (the current card's page) and `{hint-url}` for
    /// `userkeys::fill`.
    pub(crate) fn placeholder(&self, name: &str, hint_url: Option<&str>) -> Option<String> {
        let page = self.focused.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note() && !n.url.is_empty());
        match name {
            "url" => page.map(|n| n.url.clone()),
            "title" => page.map(|n| if n.title.trim().is_empty() { n.url.clone() } else { n.title.clone() }),
            "hint-url" => hint_url.map(str::to_string),
            _ => None,
        }
    }

    /// Runs one command line (`scene::command`), placeholders filled.
    pub(crate) fn exec_line(&mut self, line: &str, hint_url: Option<&str>) {
        use scene::command::Cmd;
        // Lines that keep their placeholders for later (a binding, a hint's
        // line) or fill them word by word (`run`).
        let raw = |l: &str| {
            let name = l.trim().trim_start_matches(':').split_whitespace().next().unwrap_or("");
            scene::command::lookup(name).is_ok_and(|s| {
                matches!(s.cmd, Cmd::Bind | Cmd::Unbind | Cmd::Alias | Cmd::Hint | Cmd::HintMany | Cmd::Run | Cmd::Later | Cmd::Repeat)
            })
        };
        let filled = if raw(line) {
            line.to_string()
        } else {
            match userkeys::fill(line, &|n| self.placeholder(n, hint_url)) {
                Ok(l) => l,
                Err(e) => return self.toast(e),
            }
        };
        let parsed = match scene::command::parse(&filled) {
            Ok(p) => p,
            Err(e) => {
                log::info!("command line: {filled:?}: {e}");
                self.toast(e.to_string());
                return;
            }
        };
        log::info!("command line: {:?} → {:?} {:?}", filled, parsed.spec.cmd, parsed.arg);
        let arg = parsed.arg.clone().unwrap_or_default();
        let url = || self.settings.resolve_url(&arg);
        let page = |app: &mut Self, action: KeyAction, count: Option<u32>| {
            if app.focused_page().is_some() {
                app.page_action(action, count);
            } else {
                app.toast("No page here");
            }
        };
        match parsed.spec.cmd {
            Cmd::Open => match url() {
                Some(u) if self.focused_page().is_some() => self.load_here(&u),
                Some(u) if self.new_card_at_center(&u).is_some() => self.give_keys(PageKeys::Page),
                _ => {}
            },
            Cmd::Beside => {
                if let Some(u) = url() {
                    self.open_beside_current(&u);
                }
            }
            Cmd::New => match parsed.arg.as_ref().and_then(|_| url()) {
                Some(u) => {
                    if self.new_card_at_center(&u).is_some() {
                        self.give_keys(PageKeys::Page);
                    }
                }
                None => self.run_command(Command::NewCard),
            },
            Cmd::StartPage => self.run_command(Command::StartPage),
            Cmd::Note => self.run_command(Command::NewNote),
            Cmd::Sticky => self.run_command(Command::NewSticky),
            Cmd::Close => self.run_command(Command::CloseCard),
            Cmd::Exit => self.run_command(Command::Quit),
            Cmd::Save => self.run_command(Command::SaveMarkdown),
            Cmd::Reload if parsed.arg.is_some() => page(self, KeyAction::ReloadFresh, None),
            Cmd::Reload => page(self, KeyAction::Reload, None),
            Cmd::Back => page(self, KeyAction::Back, parsed.count()),
            Cmd::Forward => page(self, KeyAction::Forward, parsed.count()),
            Cmd::Find => {
                if parsed.arg.is_some() {
                    self.last_find = arg.clone();
                }
                self.open_find(false);
            }
            Cmd::Copy => {
                let what = match arg.as_str() {
                    "title" => keys::Yank::Title,
                    "markdown" => keys::Yank::Markdown,
                    "site" => keys::Yank::Site,
                    "clean" => keys::Yank::Clean,
                    _ => keys::Yank::Url,
                };
                page(self, KeyAction::Copy(what, false), None);
            }
            Cmd::PageNext => page(self, KeyAction::NextPage, None),
            Cmd::PagePrev => page(self, KeyAction::PrevPage, None),
            Cmd::Up => page(self, KeyAction::UrlUp, parsed.count()),
            Cmd::Root => page(self, KeyAction::UrlRoot, None),
            Cmd::Increment => page(self, KeyAction::Increment, parsed.count()),
            Cmd::Decrement => page(self, KeyAction::Decrement, parsed.count()),
            Cmd::Stop => match self.focused.and_then(|id| self.cards.get(&id)) {
                Some(c) if c.loading => page(self, KeyAction::Stop, None),
                _ => self.toast("Nothing is loading"),
            },
            Cmd::Home => self.open_home(),
            Cmd::ViewSource => page(self, KeyAction::ViewSource, None),
            Cmd::Inspect => self.inspect_command(&arg),
            Cmd::SaveFile => page(self, KeyAction::DownloadPage, None),
            Cmd::Fullscreen => self.toggle_fullscreen(),
            Cmd::Zoom => self.run_command(match arg.as_str() {
                "in" => Command::ZoomIn,
                "out" => Command::ZoomOut,
                _ => Command::ZoomReset,
            }),
            Cmd::Tidy => self.run_command(Command::Tidy),
            Cmd::Fit => self.run_command(Command::Fit),
            Cmd::Fold => self.run_command(Command::Fold),
            Cmd::Undo => self.run_command(Command::Undo),
            Cmd::Redo => self.run_command(Command::Redo),
            Cmd::Reopen => self.run_command(Command::Reopen),
            Cmd::Solo => self.run_command(Command::Read),
            Cmd::Sidebar => self.run_command(Command::Sidebar),
            Cmd::CardNext => {
                self.card_key(KeyAction::NextCard, parsed.count());
            }
            Cmd::CardPrev => {
                self.card_key(KeyAction::PrevCard, parsed.count());
            }
            Cmd::CardOnly => self.ask_close_others(),
            Cmd::Clone => self.clone_card(),
            Cmd::Awake => match self.current() {
                Some(id) => self.set_awake(id, parsed.arg.as_deref().and_then(Awake::parse)),
                None => self.toast("No card"),
            },
            Cmd::Mute => match self.current() {
                Some(id) => self.toggle_mute(id),
                None => self.toast("No card"),
            },
            Cmd::Media => match scene::media::parse(&arg) {
                Ok(op) => self.media_op(op),
                Err(e) => self.toast(e),
            },
            Cmd::Saved => match self.saved_url(&arg) {
                Some(u) if self.focused_page().is_some() => self.load_here(&u),
                Some(u) if self.new_card_at_center(&u).is_some() => self.give_keys(PageKeys::Page),
                _ => {}
            },
            Cmd::SavedBeside => {
                if let Some(u) = self.saved_url(&arg) {
                    self.open_beside_current(&u);
                }
            }
            Cmd::SavePage => self.save_page(parsed.arg.as_deref()),
            Cmd::Mark | Cmd::ToMark => {
                let mut cs = arg.chars();
                match (cs.next(), cs.next()) {
                    (Some(c), None) if parsed.spec.cmd == Cmd::Mark => self.set_mark(c),
                    (Some(c), None) if jump::is_card_mark(c) || jump::is_board_mark(c) || c == '\'' => self.go_mark(c),
                    _ => self.toast("A mark is one letter: a–z or A–Z"),
                }
            }
            Cmd::JumpBack => self.walk_jumps(true, parsed.count().unwrap_or(1).min(100) as usize),
            Cmd::JumpForward => self.walk_jumps(false, parsed.count().unwrap_or(1).min(100) as usize),
            Cmd::Card => match self.card_named(&arg) {
                // From a page the keys go with it (and Solo reads it), as gt does.
                Some(to) => self.switch_card(to),
                None if arg.parse::<usize>().is_ok() => self.toast(format!("No card {arg} here")),
                None => self.toast(format!("No card called “{arg}”")),
            },
            Cmd::Name if parsed.arg.is_some() => match read::pick(&self.scene, self.focused, self.camera.center) {
                Some(id) => self.name_trail(id, &arg),
                None => self.toast("No trail here"),
            },
            Cmd::Name => self.run_command(Command::NameTrail),
            Cmd::Colour => self.run_command(Command::ColourTrail),
            Cmd::Join | Cmd::JoinKeep if parsed.arg.is_some() => match self.card_named(&arg) {
                Some(target) => {
                    let cards = self.relink_cards();
                    self.join_cards(&cards, target, parsed.spec.cmd == Cmd::JoinKeep);
                }
                None => self.toast(format!("No card called “{arg}”")),
            },
            Cmd::Join => self.run_command(Command::JoinTrail),
            Cmd::JoinKeep => self.run_command(Command::JoinTrailKeep),
            Cmd::Link if parsed.arg.is_some() => match self.card_named(&arg) {
                Some(target) => {
                    let cards = self.relink_cards();
                    self.link_cards(&cards, target);
                }
                None => self.toast(format!("No card called “{arg}”")),
            },
            Cmd::Link => self.run_command(Command::LinkCard),
            Cmd::Leave => self.run_command(Command::LeaveTrail),
            Cmd::SelectAll => self.run_command(Command::SelectAll),
            Cmd::Space => {
                let names: Vec<String> = self.spaces.spaces.iter().map(|s| s.name.clone()).collect();
                match Self::best_name(&arg, &names) {
                    Some(i) => self.switch_space(self.spaces.spaces[i].id),
                    None => self.toast(format!("No space called “{arg}”")),
                }
            }
            Cmd::SpaceNew | Cmd::SpacePrivate => {
                self.new_space(parsed.spec.cmd == Cmd::SpacePrivate);
                if parsed.arg.is_some() {
                    self.rename_space(&arg);
                }
            }
            Cmd::SpaceRename => self.rename_space(&arg),
            Cmd::SpaceDelete => self.run_command(Command::DeleteSpace),
            Cmd::Private => self.run_command(Command::PrivateSpace),
            Cmd::Tint => self.run_command(Command::NextTint),
            Cmd::Photo => self.run_command(Command::BackgroundPhoto),
            Cmd::History => {
                self.open_history();
                if parsed.arg.is_some()
                    && let Some(ob) = self.omnibox.as_mut()
                {
                    ob.insert(&arg);
                }
            }
            Cmd::Downloads => self.run_command(Command::Downloads),
            Cmd::Settings => self.run_command(Command::Settings),
            Cmd::Keys => self.run_command(Command::Shortcuts),
            Cmd::Messages if arg == "clear" => {
                self.messages.clear();
                log::info!("messages cleared");
            }
            Cmd::Messages => self.run_command(Command::Messages),
            Cmd::Bind | Cmd::Unbind | Cmd::Alias => {
                let entry = storage::keyfile::parse_line(&format!("{} {arg}", parsed.spec.name));
                match entry.and_then(|e| e.ok_or_else(|| "nothing to do".to_string())).and_then(|e| self.keys.apply(&e)) {
                    Ok(done) => {
                        log::info!("keys: {done} (this run)");
                        self.toast(format!("{done} · until restart (the keys file keeps it)"));
                    }
                    Err(e) => self.toast(format!("{}: {e}", parsed.spec.name)),
                }
            }
            Cmd::ReloadKeys => self.load_keys(true),
            Cmd::KeysFile => self.open_keys_file(),
            Cmd::Run => {
                let words = userkeys::words(&arg).and_then(|ws| {
                    ws.iter().map(|w| userkeys::fill(w, &|n| self.placeholder(n, hint_url))).collect::<Result<Vec<_>, _>>()
                });
                match words {
                    Ok(ws) => self.run_program(&ws),
                    Err(e) => self.toast(format!("run: {e}")),
                }
            }
            Cmd::Hint | Cmd::HintMany => {
                self.ask_link_hints(false, Some(hint::Kind::Run(parsed.spec.cmd == Cmd::HintMany)));
                if let Some(lh) = self.link_hints.as_mut() {
                    lh.line = Some(arg.clone());
                }
            }
            Cmd::Welcome => self.run_command(Command::Welcome),
            Cmd::Ads => self.run_command(Command::ToggleBlocking),
            Cmd::Set => self.run_set(&arg),
            Cmd::Reset => match scene::setcmd::find(&arg) {
                Ok(x) => self.apply_setting_change(x, scene::setcmd::reset(x)),
                Err(e) => self.toast(format!("reset: {e}")),
            },
            Cmd::Toggle => self.run_toggle(&arg),
            Cmd::Site | Cmd::SiteSession => match scene::setcmd::parse_site(&arg) {
                Ok((what, on)) => self.switch_site(self.focused, what, on, parsed.spec.cmd == Cmd::SiteSession),
                Err(e) => self.toast(e),
            },
            Cmd::AdsAllowed => self.run_command(Command::AllowedSites),
            Cmd::ClearHistory => self.run_command(Command::ClearHistory),
            Cmd::ClearCookies => self.run_command(Command::ClearCookies),
            // What these hold runs with its placeholders filled then (and
            // its bars kept), so they stay raw above.
            Cmd::Repeat => match userkeys::held(&filled, &self.keys.aliases) {
                Ok((n, steps)) => {
                    for _ in 0..n.min(u64::from(scene::command::MAX_TIMES)) {
                        self.run_steps(&steps, hint_url);
                    }
                }
                Err(e) => self.toast(e),
            },
            Cmd::Later => match scene::command::split_times(&arg, "milliseconds") {
                Ok((ms, line)) => {
                    let ms = ms.min(scene::command::MAX_LATER_MS);
                    if ms >= 1000 {
                        self.toast(format!("In {} s: {line}", (ms as f64 / 1000.0 * 10.0).round() / 10.0));
                    }
                    self.later_ids += 1;
                    self.later.push(LaterLine {
                        id: self.later_ids,
                        at: Instant::now() + Duration::from_millis(ms),
                        line,
                        hint_url: hint_url.map(str::to_string),
                    });
                }
                Err(e) => self.toast(format!("later: {e}")),
            },
        }
    }

    /// Runs a palette row (the palette is already closed). `beside` (Shift+Enter
    /// or Shift+click) opens a page row beside the current card instead of
    /// in the middle of the view; other rows run as usual.
    pub(crate) fn run_palette(&mut self, row: palette::Row, beside: bool) {
        log::info!("palette: {:?} {:?}{}", row.item, row.label, if beside { " (beside)" } else { "" });
        match row.item {
            Item::Card(id) => {
                if self.read.is_some() {
                    self.leave_read();
                }
                self.go_to_card(id);
            }
            Item::Space(id) => self.switch_space(id),
            Item::Open(url) if beside => self.open_beside_current(&url),
            Item::Open(url) => {
                if self.new_card_at_center(&url).is_some() {
                    self.give_keys(PageKeys::Page);
                }
            }
            Item::Closed(id) => {
                if self.read.is_some() {
                    self.leave_read();
                }
                self.reopen_closed_card(id);
            }
            Item::Command(c) => self.run_command(c),
            Item::Download(key) => self.open_download(key),
            Item::AllowedSite(site) => self.block_site_again(&site),
            Item::Photo(path) => self.set_space_photo(path),
            Item::Message(id) => {
                if let Some(text) = self.messages.get(id).map(|m| m.text.clone()) {
                    self.clipboard.set_text(&text);
                    self.toast("Copied the message");
                }
            }
            Item::Waiting(id) => {
                if let Some(i) = self.later.iter().position(|l| l.id == id) {
                    let l = self.later.remove(i);
                    self.run_later(l);
                }
            }
            Item::Run(line) | Item::Recent(line) => self.run_line(&line),
            Item::Complete(line) if userkeys::steps(&line, &self.keys.aliases).is_ok() => self.run_line(&line),
            Item::Complete(line) => self.fill_command_line(&line),
        }
    }

    /// Ad blocking on card `id`'s site, for its page menu: `None` for notes,
    /// pages that aren't web pages, or while blocking is off in the settings.
    pub(crate) fn blocking_state(&self, id: NodeId) -> Option<web::menu::Blocking> {
        let n = self.scene.node(id).filter(|n| !n.is_note() && self.settings.block_ads)?;
        let site = scene::icon::site(&n.url)?;
        let card = self.cards.get(&id);
        let blocked = card.map_or(0, |c| c.page.blocked());
        let hidden = card.map_or(0, |c| c.page.hidden());
        let banner_rejected = card.is_some_and(|c| c.page.banner_rejected());
        Some(web::menu::Blocking { site, on: !self.blocking.allows(&n.url), blocked, hidden, banner_rejected })
    }

    /// JavaScript and images on card `id`'s site, for its page menu: `None`
    /// for notes and pages that aren't on a web site.
    pub(crate) fn site_content_state(&self, id: NodeId) -> Option<web::menu::SiteContent> {
        use scene::sitesettings::Content;
        let n = self.scene.node(id).filter(|n| !n.is_note())?;
        if self.cards.get(&id).is_some_and(|c| c.page.kind() != web::PageKind::Web) {
            return None;
        }
        let site = scene::icon::site(&n.url)?;
        Some(web::menu::SiteContent {
            site,
            javascript: self.site_rules().allows(&n.url, Content::JavaScript, self.settings.javascript),
            images: self.site_rules().allows(&n.url, Content::Images, self.settings.images),
        })
    }

    /// Turns ad blocking off on card `id`'s site, or back on, and reloads the
    /// site's live pages so the change shows.
    pub(crate) fn toggle_blocking(&mut self, id: Option<NodeId>) {
        let Some(url) = id.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note()).map(|n| n.url.clone()) else {
            self.toast("No page here");
            return;
        };
        self.switch_site_ads(&url, None, false);
    }

    /// Reloads the live pages of `site` after blocking changed there (`on`:
    /// back on), so the change shows.
    pub(crate) fn reload_site(&self, site: &str, on: bool) {
        let mut reloaded = 0;
        for (cid, c) in &self.cards {
            let same = self.scene.node(*cid).is_some_and(|n| !n.is_note() && scene::privacy::covers(site, &n.url));
            if same && c.page.kind() == web::PageKind::Web {
                c.page.reload_fresh();
                reloaded += 1;
            }
        }
        log::info!("blocking {} {site}: {reloaded} page(s) reloaded", if on { "back on for" } else { "off for" });
    }
}
