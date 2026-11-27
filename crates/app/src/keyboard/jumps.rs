//! Jumps, marks, saved pages.

use scene::jump::{self, Before, BoardSpot, ScrollSpot};
use scene::keys::Action as KeyAction;
use scene::nav::{self, Dir};
use scene::{caret, Node, NodeId};

use crate::app::{App, Confirm, now_ms, PageKeys};
use crate::ui::omnibox::{Omnibox, Target};

/// Why a card's scroll sections were asked for (M12 step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SectionAsk {
    /// `s`: label them.
    Label,
    /// `]s` / `[s`: step to the next / previous one.
    Step(bool),
    /// `S`: step out to the one around the target.
    Out,
    /// Only to name them in the mode pill.
    Name,
}

/// Why a card's scroll spot was asked for (M12 step 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpotAsk {
    /// `m` then a lower-case letter: keep it as that mark.
    Mark(char),
    /// A jump to a mark starts: keep it for `''`.
    Before,
}

impl App {
    /// Where we are on the board now, for a board mark or `''`.
    pub(crate) fn board_spot(&self) -> BoardSpot {
        BoardSpot { space: self.spaces.active.0, card: self.focused, camera: self.camera_goal.unwrap_or(self.camera) }
    }

    /// Ctrl+o / Ctrl+i: `n` cards back / forward through the cards visited
    /// in this space (closed ones skipped). The keys stay in a page.
    pub(crate) fn walk_jumps(&mut self, back: bool, n: usize) {
        let scene = &self.scene;
        let alive = |id: NodeId| scene.node(id).is_some();
        let jumps = &mut self.places.spaces.entry(self.spaces.active.0).or_default().jumps;
        let to = if back { jumps.back(n, self.focused, alive) } else { jumps.forward(n, self.focused, alive) };
        let Some(to) = to else {
            self.toast(if back { "No card visited before this one" } else { "No card visited after this one" });
            return;
        };
        self.places_dirty = true;
        log::info!("jumps: {} to card {}", if back { "back" } else { "forward" }, to.0);
        self.jumping = true;
        self.switch_card(to);
        self.jumping = false;
    }

    /// `m` / `M` then a letter: a lower-case mark keeps the current card's
    /// scroll spot (asked of its page), an upper-case one the board spot.
    pub(crate) fn set_mark(&mut self, c: char) {
        if jump::is_board_mark(c) {
            let spot = self.board_spot();
            log::info!("mark {c}: space {} card {:?}", spot.space, spot.card.map(|n| n.0));
            self.places.board.insert(c, spot);
            self.places_dirty = true;
            self.toast(format!("Mark {c} set: this spot on the board ('{c} comes back)"));
            return;
        }
        if !jump::is_card_mark(c) {
            self.toast(format!("“{c}” isn't a mark: a–z or A–Z"));
            return;
        }
        let Some(id) = self.focused.filter(|id| self.cards.contains_key(id)) else {
            self.toast("Pick a page to mark first");
            return;
        };
        let target = self.scroll_targets.get(&id).copied();
        self.cards[&id].page.scroll_command(&web::scroll::spot(target));
        self.spot_ask = Some((id, SpotAsk::Mark(c)));
    }

    /// `'` then a letter: a lower-case mark scrolls the current card's page
    /// back to its spot (loading its address again if the page moved on);
    /// an upper-case one goes to its board spot, in its space. `''` goes
    /// back to where the last jump left from.
    pub(crate) fn go_mark(&mut self, c: char) {
        if c == '\'' {
            match self.before.clone() {
                Some(Before::Board(spot)) => self.go_board_spot(spot, None),
                Some(Before::Scroll(id, spot)) if self.scene.node(id).is_some() => {
                    if self.focused != Some(id) {
                        self.switch_card(id);
                    }
                    self.go_scroll_spot(id, spot);
                }
                _ => self.toast("No jump to go back from"),
            }
            return;
        }
        if jump::is_board_mark(c) {
            match self.places.board.get(&c).copied() {
                Some(spot) => self.go_board_spot(spot, Some(c)),
                None => self.toast(format!("No mark {c} · M{c} on the board or m{c} in a page sets it")),
            }
            return;
        }
        let Some(id) = self.focused.filter(|id| self.scene.node(*id).is_some()) else {
            self.toast("Pick a card first");
            return;
        };
        match self.places.card_mark(self.spaces.active.0, id, c).cloned() {
            Some(spot) => self.go_scroll_spot(id, spot),
            None => {
                let have = self.places.letters(self.spaces.active.0, id);
                let have = if have.is_empty() { String::new() } else { format!(" · marks here: {have}") };
                self.toast(format!("No mark {c} on this card{have}"));
            }
        }
    }

    /// A board spot: its space (if it is another), its card (if still
    /// there) and its camera. The spot left becomes `''`'s.
    pub(crate) fn go_board_spot(&mut self, spot: BoardSpot, mark: Option<char>) {
        let from = self.board_spot();
        let other_space = spot.space != self.spaces.active.0;
        if other_space {
            let Some(space) = self.spaces.spaces.iter().find(|s| s.id.0 == spot.space).map(|s| s.id) else {
                self.toast("That spot's space is gone");
                return;
            };
            if self.read.is_some() {
                self.leave_read();
            }
            self.switch_space(space);
        }
        match spot.card.filter(|&id| self.scene.node(id).is_some()) {
            Some(id) => self.switch_card(id),
            None if spot.card.is_some() => self.toast(match mark {
                Some(c) => format!("Mark {c}'s card is closed; here is where it was"),
                None => "That card is closed; here is where it was".to_string(),
            }),
            None => {}
        }
        if self.read.is_none() {
            if other_space || self.still {
                self.camera = spot.camera;
                self.camera_goal = None;
                self.camera_moved();
            } else {
                self.camera_goal = Some(spot.camera);
            }
        }
        self.before = Some(Before::Board(from));
        log::info!("to board spot {:?} in space {} (mark {mark:?})", spot.card.map(|n| n.0), spot.space);
    }

    /// Card `id`'s page back to a scroll spot; where it is now is kept for
    /// `''` first. A page that has moved on loads the spot's address and
    /// scrolls once it has loaded.
    pub(crate) fn go_scroll_spot(&mut self, id: NodeId, spot: ScrollSpot) {
        let Some(url) = self.scene.node(id).map(|n| n.url.clone()) else { return };
        self.create_page(id);
        let Some(card) = self.cards.get(&id) else { return };
        let target = self.scroll_targets.get(&id).copied();
        card.page.scroll_command(&web::scroll::spot(target));
        self.spot_ask = Some((id, SpotAsk::Before));
        if jump::same_page(&spot.url, &url) && !card.loading {
            card.page.scroll_command(&web::scroll::restore(&spot.path, spot.x, spot.y));
            log::info!("card {}: to scroll spot {:?} {},{}", id.0, spot.path, spot.x, spot.y);
        } else if jump::same_page(&spot.url, &url) {
            self.restore_due.insert(id, spot);
        } else if card.page.kind() == web::PageKind::Web {
            log::info!("card {}: loading {} for its mark", id.0, spot.url);
            card.page.load(&spot.url);
            self.restore_due.insert(id, spot);
        } else {
            self.toast("That mark is on another page");
        }
    }

    /// A card's page has loaded: a mark waiting for it scrolls there.
    pub(crate) fn restore_after_load(&mut self, id: NodeId) {
        let Some(spot) = self.restore_due.remove(&id) else { return };
        if let Some(c) = self.cards.get(&id) {
            log::info!("card {}: loaded; to scroll spot {:?} {},{}", id.0, spot.path, spot.x, spot.y);
            c.page.scroll_command(&web::scroll::restore(&spot.path, spot.x, spot.y));
        }
    }

    /// A page answered where it is scrolled ([`SpotAsk`]).
    pub(crate) fn got_spot(&mut self, id: NodeId, path: String, x: f32, y: f32) {
        let Some((_, why)) = self.spot_ask.filter(|(n, _)| *n == id) else {
            log::debug!("scroll spot from card {} not asked for", id.0);
            return;
        };
        self.spot_ask = None;
        let Some(url) = self.scene.node(id).map(|n| n.url.clone()) else { return };
        let spot = ScrollSpot { url, path, x, y };
        match why {
            SpotAsk::Mark(c) => {
                log::info!("mark {c} on card {}: {:?} {x},{y}", id.0, spot.path);
                self.places.set_card_mark(self.spaces.active.0, id, c, spot);
                self.places_dirty = true;
                self.toast(format!("Mark {c} set: this spot in the page ('{c} comes back)"));
            }
            SpotAsk::Before => self.before = Some(Before::Scroll(id, spot)),
        }
    }

    /// b / B: the command line with the saved pages listed, to open one
    /// here / beside.
    pub(crate) fn ask_saved(&mut self, beside: bool) {
        if self.saved_pages.pages().is_empty() {
            self.toast("No saved pages yet · :save-page saves the current one");
            return;
        }
        self.open_command_line(None);
        if let Some(ob) = self.omnibox.as_mut() {
            ob.insert(if beside { "saved-beside " } else { "saved " });
        }
    }

    pub(crate) fn save_saved(&self) {
        if self.persist
            && let Err(e) = storage::saved::save(&self.data_dir, &self.saved_pages)
        {
            log::error!("saving saved pages: {e}");
        }
    }

    /// `:save-page [name]`: the current card's page, under a name.
    pub(crate) fn save_page(&mut self, name: Option<&str>) {
        let Some(id) = self.focused.filter(|id| self.cards.get(id).is_some_and(|c| c.page.kind() == web::PageKind::Web)) else {
            self.toast("Pick a web page to save first");
            return;
        };
        let Some(n) = self.scene.node(id) else { return };
        let (url, title) = (n.url.clone(), n.title.clone());
        use scene::saved::Outcome;
        match self.saved_pages.save(name, &url, &title, now_ms()) {
            Ok(out) => {
                log::info!("saved pages: {out:?} {url}");
                self.save_saved();
                self.toast(match out {
                    Outcome::Added(n) => format!("Saved as “{n}” · b opens it"),
                    Outcome::Replaced(n) => format!("“{n}” now opens this page"),
                    Outcome::Already(n) => format!("Saved already, as “{n}”"),
                });
            }
            Err(e) => self.toast(e),
        }
    }

    /// The saved page `typed` names, or a toast.
    pub(crate) fn saved_url(&mut self, typed: &str) -> Option<String> {
        match self.saved_pages.find(typed) {
            Some(p) => Some(p.url.clone()),
            None => {
                self.toast(format!("No saved page called “{typed}”"));
                None
            }
        }
    }

    /// Makes card `to` current from a key: shown if folded away, brought on
    /// screen (read, in Solo). From a page, the keys go with it in Page mode.
    pub(crate) fn switch_card(&mut self, to: NodeId) {
        let page = self.entered();
        if self.focused == Some(to) {
            self.ensure_visible(to);
            return;
        }
        self.go_to_card(to);
        if page {
            self.keep_page_keys();
        }
        log::info!("card keys: to card {}", to.0);
    }

    /// The focused card's page has the keys, in Page mode (as after Enter,
    /// or as Solo gives them).
    pub(crate) fn keep_page_keys(&mut self) {
        let Some(id) = self.focused else { return };
        if self.read.is_none() {
            self.keys_in_page = true;
        }
        self.set_page_keys(self.site_keys(Some(id)));
        if let Some(c) = self.cards.get(&id) {
            c.page.set_focus(true);
        }
    }

    /// d (or Ctrl+w) in a page: close the card and go on to the next one in
    /// reading order (the previous, at the end), keeping Page mode and Solo.
    pub(crate) fn close_from_page(&mut self, id: NodeId) {
        let solo = self.read.is_some();
        let next = nav::step(&scene::tidy::order(&self.scene), Some(id), 1).filter(|&n| n != id);
        self.close_card(id);
        log::info!("closed card {} from its page; next {:?}", id.0, next.map(|n| n.0));
        let Some(next) = next else { return };
        if solo {
            self.read_card(next);
        } else {
            self.go_to_card(next);
            self.keep_page_keys();
        }
    }

    /// co / `:card-only`: close every other card in the space, asking first.
    pub(crate) fn ask_close_others(&mut self) {
        let Some(id) = self.focused.filter(|&id| self.scene.node(id).is_some()) else {
            self.toast("Pick a card to keep first");
            return;
        };
        let n = self.scene.all_nodes().filter(|n| n.id != id).count();
        if n == 0 {
            self.toast("No other cards here");
            return;
        }
        let cards = if n == 1 { "the 1 other card".to_string() } else { format!("the {n} other cards") };
        self.confirm = Some(Confirm::CloseOthers(id));
        self.toast(format!("Close {cards} in {}? Enter closes · Esc keeps them", self.spaces.active_entry().name));
    }

    pub(crate) fn close_others(&mut self, keep: NodeId) {
        if self.scene.node(keep).is_none() {
            return;
        }
        let others: Vec<NodeId> = self.scene.all_nodes().map(|n| n.id).filter(|&id| id != keep).collect();
        log::info!("closing the {} card(s) other than card {}", others.len(), keep.0);
        self.close_cards(&others);
        self.go_to_card(keep);
        let n = others.len();
        self.toast(format!("Closed {n} card{} · :undo brings them back", if n == 1 { "" } else { "s" }));
    }

    /// gC / `:clone`: the card's page opened again beside it, joined by a
    /// trail edge (its own back and forward list starts afresh).
    pub(crate) fn clone_card(&mut self) {
        let Some(n) = self.focused.and_then(|id| self.scene.node(id)) else {
            self.toast("Pick a card to clone first");
            return;
        };
        if n.is_note() || n.url.is_empty() {
            self.toast("Only a page can be cloned");
            return;
        }
        let url = n.url.clone();
        let page = self.entered();
        self.open_beside_current(&url);
        if !page {
            self.keys_in_page = false;
        }
    }

    /// Runs a Page mode key's action (`scene::keys::page`) on the entered card.
    pub(crate) fn page_action(&mut self, action: KeyAction, count: Option<u32>) {
        const MAX_REPEAT: u32 = 100;
        let times = if action.repeats() { count.unwrap_or(1).clamp(1, MAX_REPEAT) } else { 1 };
        let Some(id) = self.focused else { return };
        log::debug!("page key: {action:?} ×{times}");
        use web::scroll::{Axis, Edge as ScrollEdge, Unit};
        let n = times as i32;
        match action {
            KeyAction::Scroll(dir) => {
                let (axis, n) = match dir {
                    Dir::Down => (Axis::Y, n),
                    Dir::Up => (Axis::Y, -n),
                    Dir::Left => (Axis::X, -n),
                    Dir::Right => (Axis::X, n),
                };
                self.scroll_section(id, axis, n, Unit::Line);
            }
            KeyAction::ScrollDown => self.scroll_section(id, Axis::Y, n, Unit::Half),
            KeyAction::ScrollUp => self.scroll_section(id, Axis::Y, -n, Unit::Half),
            KeyAction::Top => self.scroll_edge(id, ScrollEdge::Top),
            KeyAction::Bottom => self.scroll_edge(id, ScrollEdge::Bottom),
            KeyAction::LeftEdge => self.scroll_edge(id, ScrollEdge::Left),
            KeyAction::RightEdge => self.scroll_edge(id, ScrollEdge::Right),
            KeyAction::Sections => self.ask_sections(id, SectionAsk::Label),
            KeyAction::NextSection => self.ask_sections(id, SectionAsk::Step(true)),
            KeyAction::PrevSection => self.ask_sections(id, SectionAsk::Step(false)),
            KeyAction::OuterSection => self.ask_sections(id, SectionAsk::Out),
            KeyAction::LinkHintsHere => self.show_link_hints_to(true),
            KeyAction::LinkHints => self.show_link_hints_to(false),
            KeyAction::Hint(kind) => self.ask_link_hints(false, Some(kind)),
            KeyAction::Back | KeyAction::Forward => {
                if let Some(p) = self.focused_page() {
                    for _ in 0..times {
                        if action == KeyAction::Back { p.back() } else { p.forward() }
                    }
                }
            }
            KeyAction::Reload | KeyAction::ReloadFresh => {
                if let Some(p) = self.focused_page().filter(|p| p.kind() == web::PageKind::Web) {
                    if action == KeyAction::Reload { p.reload() } else { p.reload_fresh() }
                }
            }
            KeyAction::Insert => self.set_page_keys(PageKeys::Insert),
            KeyAction::Passthrough => self.set_page_keys(PageKeys::Passthrough),
            KeyAction::Caret => self.start_caret(id, caret::Mode::Caret),
            KeyAction::VisualLine => self.start_caret(id, caret::Mode::Lines),
            KeyAction::FocusField => {
                if let Some(p) = self.focused_page() {
                    p.focus_first_field();
                }
            }
            KeyAction::Copy(what, primary) => self.copy_from_card(id, what, primary),
            KeyAction::PasteHere => self.open_clipboard(false, false),
            KeyAction::PasteBeside => self.open_clipboard(true, false),
            KeyAction::PrimaryHere => self.open_clipboard(false, true),
            KeyAction::PrimaryBeside => self.open_clipboard(true, true),
            KeyAction::AddressBar => self.open_address_bar(),
            KeyAction::AddressBarBeside => self.omnibox = Some(Omnibox::new("", Target::Beside)),
            KeyAction::AddressBarBehind => self.omnibox = Some(Omnibox::new("", Target::Behind)),
            KeyAction::EditAddress | KeyAction::EditAddressBeside => self.edit_address(action == KeyAction::EditAddressBeside),
            KeyAction::NextPage | KeyAction::PrevPage => self.ask_pager(id, action == KeyAction::NextPage),
            KeyAction::UrlUp => {
                let n = count.unwrap_or(1).clamp(1, MAX_REPEAT);
                self.go_address(id, |u| scene::address::up(u, n), "Already at the top of the site");
            }
            KeyAction::UrlRoot => self.go_address(id, scene::address::root, "Already at the site's root"),
            KeyAction::Increment | KeyAction::Decrement => {
                let by = count.unwrap_or(1).max(1) as i64;
                let by = if action == KeyAction::Increment { by } else { -by };
                self.go_address(id, |u| scene::address::step_number(u, by), "No number in the address to change");
            }
            KeyAction::Stop => match self.cards.get(&id) {
                Some(c) if c.loading && c.page.kind() == web::PageKind::Web => {
                    c.page.stop();
                    log::info!("stopped loading card {}", id.0);
                    self.toast("Stopped loading");
                }
                // Nothing loading: Ctrl+S saves as Markdown, as it always did.
                _ => {
                    let note = self.focused.filter(|&id| self.scene.node(id).is_some_and(Node::is_note));
                    self.save_markdown(note);
                }
            },
            KeyAction::Home => self.open_home(),
            KeyAction::ViewSource => {
                if let Some(url) = self.web_address(id) {
                    self.open_beside_current(&scene::address::view_source(&url));
                }
            }
            KeyAction::DownloadPage => {
                if let Some(url) = self.web_address(id)
                    && let Some(c) = self.cards.get(&id)
                {
                    log::info!("downloading the page of card {}: {url}", id.0);
                    c.page.download(&url);
                }
            }
            KeyAction::Fullscreen => self.toggle_fullscreen(),
            KeyAction::Inspect => self.toggle_inspector(None),
            KeyAction::Media(op) => self.media_op(op.times(count.unwrap_or(1))),
            KeyAction::Find => self.open_find(false),
            KeyAction::FindBack => self.open_find(true),
            KeyAction::FindNext | KeyAction::FindPrev if !self.last_find.is_empty() => {
                // n keeps the way the last search went (/ down, ? up); N the other.
                let forward = (action == KeyAction::FindNext) != self.find_back;
                if let Some(p) = self.focused_page() {
                    for _ in 0..times {
                        p.find(&self.last_find, forward, true);
                    }
                    self.find_marks = Some(id);
                }
            }
            KeyAction::FindNext | KeyAction::FindPrev => self.open_find(self.find_back),
            KeyAction::CommandLine => self.open_command_line(count),
            KeyAction::RepeatLine => self.repeat_line(count),
            KeyAction::Close => self.close_from_page(id),
            KeyAction::Line(n) => self.run_user_line(n, count),
            // Cards from the page (`card_key`); board actions don't come
            // from the page table.
            _ => {
                self.card_key(action, count);
            }
        }
    }
}
