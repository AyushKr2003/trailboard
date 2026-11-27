//! Permissions.

use super::*;

impl App {
    /// A site asks for a permission and the settings say to ask: queue the
    /// question; the first one shows.
    pub(super) fn permission_asked(&mut self, id: u64, origin: String, kinds: Vec<web::permission::Kind>) {
        self.asks.push(Ask { id, origin, kinds });
        self.show_next_ask();
    }

    pub(super) fn show_next_ask(&mut self) {
        if self.confirm.is_some() {
            return; // another question first (it may be this one)
        }
        let Some(ask) = self.asks.first() else { return self.show_next_dialog() };
        let what: Vec<&str> = ask.kinds.iter().map(|k| k.name()).collect();
        let site = pagezoom::site_of(&ask.origin);
        let msg = format!("{site} wants your {} · Enter allows · Esc blocks", what.join(" and "));
        log::info!("permission: asking: {msg}");
        self.confirm = Some(Confirm::Permission);
        self.toast(msg);
    }

    /// The user answered the question showing.
    pub(super) fn answer_ask(&mut self, allow: bool) {
        self.confirm = None;
        if self.asks.is_empty() {
            return;
        }
        let ask = self.asks.remove(0);
        web::permission::remember(&ask.origin, &ask.kinds, allow);
        let answered = web::permission::answer(ask.id, allow);
        log::info!("permission: {} {:?} for {} (still waiting: {answered})", if allow { "allowed" } else { "blocked" }, ask.kinds, ask.origin);
        let what: Vec<&str> = ask.kinds.iter().map(|k| k.name()).collect();
        self.toast(format!("{} {} for {}", if allow { "Allowed" } else { "Blocked" }, what.join(" and "), pagezoom::site_of(&ask.origin)));
        self.show_next_ask();
    }

    /// Chromium withdrew request `id` (its page went away or navigated).
    pub(super) fn permission_withdrawn(&mut self, id: u64) {
        let Some(i) = self.asks.iter().position(|a| a.id == id) else { return };
        self.asks.remove(i);
        if i == 0 && self.confirm == Some(Confirm::Permission) {
            self.confirm = None;
            self.toast = None;
            self.show_next_ask();
        }
    }
}
