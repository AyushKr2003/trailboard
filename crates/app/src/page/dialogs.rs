//! Page dialogs.

use scene::NodeId;
use scene::pagezoom;

use crate::app::{App, Confirm};
use crate::ui::omnibox::{Omnibox, Target};

/// A page's dialog waiting for the user.
pub(crate) struct PageDialog {
    pub(crate) id: u64,
    pub(crate) node: NodeId,
    pub(crate) kind: web::dialog::Kind,
    pub(crate) site: String,
    pub(crate) message: String,
    /// A prompt's text to start with.
    pub(crate) default: String,
    /// It has been put in front of the user.
    pub(crate) shown: bool,
}

impl App {
    /// Page `page` shows `alert`, `confirm`, `prompt` or asks before it is
    /// left: queued behind any question showing; the first shows.
    pub(crate) fn dialog_asked(&mut self, page: u64, id: u64, kind: web::dialog::Kind, origin: &str, message: &str, default: &str) {
        let Some(node) = self.node_of(page).filter(|n| self.cards.contains_key(n)) else {
            // A card closed already: its page may go (closing asked nothing; Ctrl+Shift+T brings the address back).
            web::dialog::answer(id, matches!(kind, web::dialog::Kind::Leave { .. }), "");
            return;
        };
        let url = self.scene.node(node).map(|n| n.url.clone()).unwrap_or_default();
        let site = pagezoom::site_of(if origin.is_empty() { &url } else { origin });
        let message = web::dialog::tidy_message(message, 2000);
        self.dialogs.push(PageDialog { id, node, kind, site, message, default: default.to_string(), shown: false });
        self.show_next_ask();
    }

    /// Puts the first page dialog in front of the user, once nothing else
    /// asks and no field is open: a prompt as a field, the rest as a question.
    pub(crate) fn show_next_dialog(&mut self) {
        if self.confirm.is_some() || self.omnibox.is_some() {
            return;
        }
        let Some(d) = self.dialogs.first_mut() else { return };
        d.shown = true;
        log::info!("dialog {}: showing {:?} from {}: {:?}", d.id, d.kind, d.site, d.message.chars().take(80).collect::<String>());
        self.menu = None;
        self.toast = None;
        if d.kind == web::dialog::Kind::Prompt {
            let mut ob = Omnibox::new(&d.default, Target::Prompt(d.id));
            ob.select_all = !d.default.is_empty();
            self.omnibox = Some(ob);
        } else {
            self.confirm = Some(Confirm::Dialog);
        }
    }

    /// The dialog showing was answered: `ok` (OK, Leave) or not, with a prompt's `text`.
    pub(crate) fn answer_dialog(&mut self, ok: bool, text: &str) {
        if self.confirm == Some(Confirm::Dialog) {
            self.confirm = None;
        }
        if self.dialogs.first().is_none_or(|d| !d.shown) {
            return;
        }
        let d = self.dialogs.remove(0);
        let waited = web::dialog::answer(d.id, ok, text);
        log::info!("dialog {}: {} (was waiting: {waited})", d.id, if ok { "OK" } else { "cancelled" });
        self.show_next_ask();
    }

    /// Chromium dropped dialog `id` (its page navigated or closed).
    pub(crate) fn dialog_withdrawn(&mut self, id: u64) {
        let Some(i) = self.dialogs.iter().position(|d| d.id == id) else { return };
        let d = self.dialogs.remove(i);
        log::info!("dialog {id}: withdrawn");
        if d.shown {
            if self.confirm == Some(Confirm::Dialog) {
                self.confirm = None;
            }
            if self.omnibox.as_ref().is_some_and(|ob| ob.target == Target::Prompt(id)) {
                self.omnibox = None;
            }
        }
    }

    /// Each frame: dialogs of cards that went away are dropped, a prompt or
    /// file chooser whose field closed is cancelled, the next question shows.
    pub(crate) fn tend_dialogs(&mut self) {
        let gone: Vec<(u64, bool)> = self
            .dialogs
            .iter()
            .filter(|d| !self.cards.contains_key(&d.node))
            .map(|d| (d.id, matches!(d.kind, web::dialog::Kind::Leave { .. })))
            .collect();
        for (id, leave) in gone {
            web::dialog::answer(id, leave, "");
            self.dialog_withdrawn(id);
        }
        if let Some(d) = self.dialogs.first()
            && d.shown
            && d.kind == web::dialog::Kind::Prompt
            && self.omnibox.as_ref().is_none_or(|ob| ob.target != Target::Prompt(d.id))
        {
            self.answer_dialog(false, "");
        }
        if let Some(c) = &self.choosing
            && self.omnibox.as_ref().is_none_or(|ob| ob.target != Target::Files(c.id))
        {
            let c = self.choosing.take().expect("checked");
            web::dialog::choose(c.id, &[]);
            log::info!("file chooser {}: cancelled", c.id);
        }
        if self.confirm.is_none() {
            self.show_next_ask();
        }
    }

    /// The question sheet for the dialog showing: title, message, keys.
    pub(crate) fn dialog_words(d: &PageDialog) -> (String, String, &'static [(&'static str, &'static str)]) {
        use web::dialog::Kind;
        match d.kind {
            Kind::Alert => (format!("{} says", d.site), d.message.clone(), &[("y", ""), ("Enter", "OK")]),
            Kind::Confirm => (format!("{} asks", d.site), d.message.clone(), &[("y", ""), ("Enter", "OK"), ("n", ""), ("Esc", "Cancel")]),
            Kind::Prompt => (format!("{} asks", d.site), d.message.clone(), &[]),
            Kind::Leave { reload } => (
                d.site.clone(),
                format!("{} this page? Changes you made may not be saved.", if reload { "Reload" } else { "Leave" }),
                if reload {
                    &[("y", ""), ("Enter", "Reload"), ("n", ""), ("Esc", "Stay")]
                } else {
                    &[("y", ""), ("Enter", "Leave"), ("n", ""), ("Esc", "Stay")]
                },
            ),
        }
    }
}
