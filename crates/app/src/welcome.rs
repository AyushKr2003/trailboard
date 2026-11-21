//! The welcome page: our own HTML (`welcome.html`) on a card the board keeps
//! as `scene::note::WELCOME_URL`, loaded as a `data:` URL with the theme's
//! colours filled in, like a note. Its buttons ask for app commands by name
//! (`web::APP_ACTION`); [`action`] says which names exist.

use crate::note::{hex, percent_encode};
use crate::theme::{Rgb, Theme, mix};

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

/// The desktop asks for less motion (`desktop::reduced_motion`), told to app
/// pages as `still` with their colours: they then show each state at once.
static STILL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Sets whether app pages are drawn without motion (asked once at start).
pub fn set_still(still: bool) {
    STILL.store(still, std::sync::atomic::Ordering::Relaxed);
}

/// The colour of the theme's shadows, opaque (pages add their own transparency).
pub(crate) fn shade(th: &Theme) -> Rgb {
    let c = th.shadow(1.0);
    [(c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8]
}

/// The theme as the page's CSS variables (colours and `radius`, the corner
/// radius in use), a `light` flag and a `still` flag (no motion), a JSON object.
pub(crate) fn colors(th: &Theme) -> String {
    // An app page is a card: its ground is the card level, the boxes on it
    // the panel level, its text the same three strengths as the chrome.
    let bg = th.card(false);
    let vars = [
        ("bg", bg),
        ("fg", th.text()),
        ("dim", th.text2()),
        ("faint", th.text3()),
        ("accent", th.accent()),
        ("accentfg", th.accent_foreground()),
        ("accenttext", th.accent_text()),
        ("card", th.panel()),
        ("hover", th.hover()),
        ("well", th.well()),
        ("rule", th.line()),
        ("rulestrong", th.line_strong()),
        ("accentsoft", th.accent_soft()),
        ("danger", th.danger()),
        // The hue shadows are made of (used with transparency).
        ("shade", shade(th)),
        ("board", mix(th.board(None), bg, 0.5)),
        // For drawing the board as the app does: its colour and dots, a
        // sticky note (paper, strip, ink) and a second trail colour.
        ("boardbg", th.board(None)),
        ("dots", th.dots()),
        ("sticky", th.sticky_paper("yellow")),
        ("stickystrip", th.sticky_strip("yellow", false)),
        ("stickyink", th.sticky_ink("yellow")),
        ("trail2", th.trail(Some("green"))),
    ];
    let body: Vec<String> = vars
        .iter()
        .map(|(k, c)| format!("\"{k}\":\"{}\"", hex(*c)))
        .collect();
    // The corner radius in use (the *Corners* setting), so app pages match the chrome.
    format!(
        "{{{},\"radius\":\"{}px\",\"light\":{},\"still\":{}}}",
        body.join(","),
        th.rounding.max(0.0),
        th.light(),
        STILL.load(std::sync::atomic::Ordering::Relaxed)
    )
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
        assert!(c.starts_with("{\"bg\":\"#") && c.contains("\"light\":") && c.ends_with(",\"still\":false}"), "{c}");
        let url = page_url(&th);
        assert!(url.starts_with("data:text/html;charset=utf-8,"));
        assert!(!url.contains("__THEME__") && !url.contains("__PREFIX__"));
        assert!(theme_script(&th).contains(&hex(th.accent())));
    }
}
