//! Page navigation extras.

use scene::caret;
use scene::keys::{self, Action as KeyAction};
use scene::{NodeId, Rect};

use crate::app::{App, PageKeys};
use crate::ui::omnibox::{Omnibox, Target};

/// Caret / Visual mode's card (`App::caret`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct CaretCard {
    pub(crate) id: NodeId,
    pub(crate) copy: bool,
}

impl App {
    /// Card `id`'s address if it shows a web page (not a note, start page
    /// or one of our own pages); else says so and gives `None`.
    pub(crate) fn web_address(&mut self, id: NodeId) -> Option<String> {
        let web = self.cards.get(&id).is_some_and(|c| c.page.kind() == web::PageKind::Web);
        match self.scene.node(id).filter(|n| web && !n.is_note() && !n.url.is_empty()) {
            Some(n) => Some(n.url.clone()),
            None => {
                self.toast("Only a web page has an address for this");
                None
            }
        }
    }

    /// gu / gU / Ctrl+a / Ctrl+x: load the address `change` makes of card
    /// `id`'s, or toast `none` when it makes nothing.
    pub(crate) fn go_address(&mut self, id: NodeId, change: impl Fn(&str) -> Option<String>, none: &str) {
        let Some(url) = self.web_address(id) else { return };
        match change(&url) {
            Some(to) => {
                log::info!("address of card {}: {url} → {to}", id.0);
                if let Some(c) = self.cards.get(&id) {
                    c.page.load(&to);
                }
            }
            None => self.toast(none),
        }
    }

    /// yy yt ym yd yp (and with Shift, to the primary selection).
    /// `v` / `V`: a caret in card `id`'s text, in `want` (Caret or Lines).
    /// A find that ran ends first, so its match becomes where the caret
    /// starts; the page answers with the mode it began in.
    pub(crate) fn start_caret(&mut self, id: NodeId, want: caret::Mode) {
        let Some(c) = self.cards.get(&id) else { return };
        if c.page.kind() != web::PageKind::Web {
            return self.toast("No caret on this page");
        }
        let found = !self.last_find.is_empty();
        if found {
            c.page.stop_finding();
        }
        self.find_marks = None;
        let color = self.theme.accent();
        let css = format!("rgb({},{},{})", color[0], color[1], color[2]);
        c.page.run_script(&web::caret::start(want.name(), &css, &self.last_find, if found { 60 } else { 0 }));
        self.set_page_keys(PageKeys::Visual(want));
        self.caret = Some(CaretCard { id, copy: false });
        log::info!("caret: start {} on card {}", want.name(), id.0);
    }

    /// Ends Caret / Visual mode in its page (its caret and selection go).
    pub(crate) fn end_caret(&mut self) {
        if let Some(c) = self.caret.take()
            && let Some(card) = self.cards.get(&c.id)
        {
            card.page.run_script(web::caret::STOP);
        }
    }

    /// Runs a Caret / Visual mode key (`scene::keys::visual`).
    pub(crate) fn visual_action(&mut self, action: KeyAction, count: Option<u32>) {
        let (Some(mode), Some(c)) = (self.visual_mode(), self.caret) else { return };
        let Some(page) = self.cards.get(&c.id).map(|card| &card.page) else { return };
        log::debug!("visual key: {action:?} {count:?}");
        let next = match action {
            KeyAction::CaretMove(m) => {
                let n = if m.counts() { count.unwrap_or(1).clamp(1, 1000) } else { 1 };
                page.run_script(&web::caret::run(m.op(), n));
                return;
            }
            KeyAction::SwapEnds => {
                page.run_script(&web::caret::swap());
                return;
            }
            KeyAction::CopySelection(primary) => {
                page.run_script(&web::caret::copy(primary));
                self.caret = Some(CaretCard { copy: true, ..c });
                return;
            }
            KeyAction::SelectChars => Some(mode.chars()),
            KeyAction::VisualLine => Some(mode.lines()),
            KeyAction::CaretBack => mode.back(),
            a if keys::page().has(a) => return self.page_action(a, count),
            a => return self.board_action(a, count),
        };
        match next {
            Some(m) => {
                page.run_script(&web::caret::mode(m.name()));
                self.page_keys = PageKeys::Visual(m);
            }
            None => self.set_page_keys(PageKeys::Page),
        }
    }

    /// An answer from Caret / Visual mode in card `id`'s page.
    pub(crate) fn caret_report(&mut self, id: NodeId, report: web::caret::Report) {
        use web::caret::Report;
        let Some(c) = self.caret.filter(|c| c.id == id) else { return };
        let here = self.visual_mode().is_some() && self.focused == Some(id);
        match report {
            Report::Mode(m) => {
                if here && let Some(m) = caret::Mode::named(&m) {
                    self.page_keys = PageKeys::Visual(m);
                }
            }
            Report::NoText => {
                self.caret = None;
                if here {
                    self.set_page_keys(PageKeys::Page);
                }
                self.toast("No text in view for a caret");
            }
            Report::Gone => {
                self.caret = None;
                if here {
                    self.set_page_keys(PageKeys::Page);
                }
            }
            Report::Empty => self.toast("Nothing selected · v starts a selection"),
            Report::Copy { primary, text } if c.copy => {
                self.caret = None;
                let n = text.chars().count();
                let what = if n == 1 { "1 character".to_string() } else { format!("{n} characters") };
                if primary {
                    self.clipboard.set_primary(&text);
                    self.toast(format!("Put {what} in the primary selection"));
                } else {
                    self.clipboard.set_text(&text);
                    self.toast(format!("Copied {what}"));
                }
                log::info!("caret: copied {n} chars from card {}{}", id.0, if primary { " to the primary selection" } else { "" });
                if here {
                    self.set_page_keys(PageKeys::Page);
                }
            }
            Report::Copy { .. } => log::warn!("caret: card {} sent a copy nobody asked for", id.0),
        }
    }

    pub(crate) fn copy_from_card(&mut self, id: NodeId, what: keys::Yank, primary: bool) {
        use keys::Yank;
        let Some(n) = self.scene.node(id) else { return };
        let addressed = !n.is_note() && !n.url.is_empty() && n.url != scene::note::START_URL;
        let (text, name) = match what {
            Yank::Url if addressed => (n.url.clone(), "the address"),
            Yank::Title => (n.title.clone(), "the title"),
            Yank::Markdown if addressed => (scene::export::link(&n.title, &n.url), "a Markdown link"),
            Yank::Site if addressed => match scene::address::site(&n.url) {
                Some(site) => (site.to_string(), "the site"),
                None => return self.toast("This address has no site"),
            },
            Yank::Clean if addressed => {
                let clean = scene::address::clean(&n.url);
                let name = if clean == n.url { "the address (it had no tracking parts)" } else { "the address without tracking parts" };
                (clean, name)
            }
            _ => return self.toast("This card has no address"),
        };
        if text.is_empty() {
            return;
        }
        if primary {
            self.clipboard.set_primary(&text);
            self.toast(format!("Put {name} in the primary selection"));
        } else {
            self.clipboard.set_text(&text);
            self.toast(format!("Copied {name}"));
        }
        log::info!("copied {what:?} of card {}{}", id.0, if primary { " to the primary selection" } else { "" });
    }

    /// go / gO: the address bar holding the card's address, the caret at
    /// its end, to edit; Enter loads it here / beside.
    pub(crate) fn edit_address(&mut self, beside: bool) {
        let page = self.focused.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note() && n.url != scene::note::START_URL);
        let url = page.map(|n| n.url.clone()).unwrap_or_default();
        let target = match (beside, page.is_some()) {
            (true, _) => Target::Beside,
            (false, true) => Target::Focused,
            (false, false) => Target::NewCard,
        };
        let mut ob = Omnibox::new(&url, target);
        ob.select_all = false;
        self.omnibox = Some(ob);
    }

    /// gh / `:home`: a start page beside the current card (its own page
    /// stays), taking the keys so its search box can be typed in.
    pub(crate) fn open_home(&mut self) {
        let Some(src) = self.current().filter(|&id| self.scene.node(id).is_some()) else {
            let id = self.add_start();
            self.ensure_visible(id);
            return;
        };
        let Some(id) = self.open_card(Rect::default(), scene::note::START_URL, Some(src)) else { return };
        if let Some(n) = self.scene.node_mut(id) {
            n.title = scene::note::START_TITLE.to_string();
        }
        log::info!("start page {} beside card {}", id.0, src.0);
        self.focus(Some(id));
        self.ensure_visible(id);
        self.give_keys(PageKeys::Insert);
    }

    /// F11 / `:fullscreen`.
    pub(crate) fn toggle_fullscreen(&mut self) {
        let Some(w) = self.window() else { return };
        let full = w.fullscreen().is_none();
        w.set_fullscreen(full.then_some(winit::window::Fullscreen::Borderless(None)));
        log::info!("full screen {}", if full { "on" } else { "off" });
    }

    /// ]] / [[: ask card `id`'s page for its next / previous links;
    /// `got_pager` follows the best.
    pub(crate) fn ask_pager(&mut self, id: NodeId, next: bool) {
        if self.web_address(id).is_none() {
            return;
        }
        let Some(c) = self.cards.get(&id) else { return };
        c.page.request_pager();
        self.pager_ask = Some((id, next));
        log::debug!("pager asked of card {} ({})", id.0, if next { "next" } else { "previous" });
    }

    pub(crate) fn got_pager(&mut self, id: NodeId, links: Vec<web::pager::Link>) {
        let Some((asked, next)) = self.pager_ask.filter(|(a, _)| *a == id) else { return };
        self.pager_ask = None;
        let Some(url) = self.scene.node(asked).map(|n| n.url.clone()) else { return };
        let links: Vec<scene::address::PagerLink> =
            links.into_iter().map(|l| scene::address::PagerLink { rel: l.rel, text: l.text, href: l.href }).collect();
        match scene::address::pick_pager(&links, next, &url) {
            Some(i) => {
                let l = &links[i];
                log::info!("pager on card {}: {} {:?} rel={:?} → {}", id.0, if next { "next" } else { "previous" }, l.text, l.rel, l.href);
                let href = l.href.clone();
                self.follow_here(id, &href);
            }
            None => {
                log::info!("pager on card {}: no {} link among {}", id.0, if next { "next" } else { "previous" }, links.len());
                self.toast(if next { "No next page link here" } else { "No previous page link here" });
            }
        }
    }
}
