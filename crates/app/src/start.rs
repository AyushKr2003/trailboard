//! The start page: an empty card with the name, a search engine picker and
//! one search box (`start.html`), opened when Trailboard starts with nothing
//! to show. The board keeps it as `scene::note::START_URL`; like the welcome
//! page it is a `data:` URL with the theme's colours filled in. Its messages
//! (after `web::APP_ACTION`) are [`GO`] and the text typed, which the app
//! loads in the same card (it then becomes an ordinary page), `engine:` and
//! an engine's id, and `engines` to open the search engine guide.

use scene::settings::SearchEngine;

use crate::theme::Theme;

const PAGE: &str = include_str!("start.html");

/// What comes before the typed text in the page's message.
pub const GO: &str = "go:";
/// What comes before an engine's id when one is chosen (start page or guide).
pub const ENGINE: &str = "engine:";
/// Asks for the search engine guide.
pub const GUIDE: &str = "engines";

/// The start page for theme `th`, searching with `engine`.
pub fn page_url(th: &Theme, engine: SearchEngine) -> String {
    crate::engines::fill(PAGE, th, engine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_gets_the_theme_prefix_and_engines() {
        let url = page_url(&Theme::default(), SearchEngine::Google);
        assert!(url.starts_with("data:text/html;charset=utf-8,"));
        assert!(!url.contains("__THEME__") && !url.contains("__PREFIX__") && !url.contains("__DATA__"));
        assert!(url.contains("Google") && url.contains("Mojeek"), "{url}");
    }

    #[test]
    fn the_pages_send_only_known_messages() {
        for page in [PAGE, include_str!("engines.html")] {
            for msg in page.split("act(\"").skip(1).filter_map(|s| s.split('"').next()) {
                assert!([GO, ENGINE, GUIDE].contains(&msg), "unknown message {msg}");
            }
        }
    }
}
