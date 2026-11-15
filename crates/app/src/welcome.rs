//! The welcome page: our own HTML (`welcome.html`) on a card the board keeps
//! as `scene::note::WELCOME_URL`, loaded as a `data:` URL with the theme's
//! colours filled in, like a note. Its buttons ask for app commands by name
//! (`web::APP_ACTION`); [`action`] says which names exist.

use crate::note::{hex, percent_encode};
use crate::theme::{Theme, mix};

/// Size of the welcome card's content, world units: wide enough for its
/// two-column layout, as tall as a page card (it sits beside the first one).
pub const W: f32 = 880.0;
pub const H: f32 = 720.0;

const PAGE: &str = include_str!("welcome.html");

/// What a button on the welcome page can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewCard,
    Palette,
    Shortcuts,
    Settings,
}

/// The action a button named `name` asks for; anything else is ignored.
pub fn action(name: &str) -> Option<Action> {
    Some(match name {
        "new-card" => Action::NewCard,
        "palette" => Action::Palette,
        "shortcuts" => Action::Shortcuts,
        "settings" => Action::Settings,
        _ => return None,
    })
}

/// The theme as the page's CSS variables (and a `light` flag), a JSON object.
pub(crate) fn colors(th: &Theme) -> String {
    let bg = th.placeholder();
    let vars = [
        ("bg", bg),
        ("fg", th.foreground()),
        ("dim", th.dim_text()),
        ("accent", th.accent()),
        ("accentfg", th.accent_foreground()),
        ("card", th.card(false)),
        ("rule", th.divider()),
        ("board", mix(th.board(None), bg, 0.5)),
    ];
    let body: Vec<String> = vars.iter().map(|(k, c)| format!("\"{k}\":\"{}\"", hex(*c))).collect();
    format!("{{{},\"light\":{}}}", body.join(","), th.light())
}

/// The welcome page for theme `th`.
pub fn page_url(th: &Theme) -> String {
    let html = PAGE.replacen("__PREFIX__", &web::js_string(web::APP_ACTION), 1).replacen("__THEME__", &colors(th), 1);
    format!("data:text/html;charset=utf-8,{}", percent_encode(&html))
}

/// Script that recolours a live welcome page.
pub fn theme_script(th: &Theme) -> String {
    format!("window.tbTheme&&tbTheme({})", colors(th))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_name_known_actions_only() {
        assert_eq!(action("new-card"), Some(Action::NewCard));
        assert_eq!(action("settings"), Some(Action::Settings));
        assert_eq!(action("quit"), None);
        assert_eq!(action(""), None);
        // Every action the page asks for is known.
        for name in PAGE.split("act('").skip(1).filter_map(|s| s.split('\'').next()) {
            assert!(action(name).is_some(), "welcome.html asks for unknown action {name}");
        }
    }

    #[test]
    fn the_page_gets_the_theme_and_the_prefix() {
        let th = Theme::default();
        let c = colors(&th);
        assert!(c.starts_with("{\"bg\":\"#") && c.contains("\"light\":"), "{c}");
        let url = page_url(&th);
        assert!(url.starts_with("data:text/html;charset=utf-8,"));
        assert!(!url.contains("__THEME__") && !url.contains("__PREFIX__"));
        assert!(theme_script(&th).contains(&hex(th.accent())));
    }
}
