//! The start page: an empty card with the name, a search engine picker, one
//! search box and the pages opened last (`start.html`), opened when
//! Trailboard starts with nothing to show. The board keeps it as `scene::note::START_URL`; like the welcome
//! page it is a `data:` URL with the theme's colours filled in. Its messages
//! (after `web::APP_ACTION`) are [`GO`] and the text typed (or a recent
//! page's address), which the app
//! loads in the same card (it then becomes an ordinary page), `engine:` and
//! an engine's id, and `engines` to open the search engine guide.

use scene::settings::SearchEngine;

use crate::note::percent_encode;
use crate::theme::Theme;

const PAGE: &str = include_str!("start.html");

/// What comes before the typed text in the page's message.
pub const GO: &str = "go:";
/// What comes before an engine's id when one is chosen (start page or guide).
pub const ENGINE: &str = "engine:";
/// Asks for the search engine guide.
pub const GUIDE: &str = "engines";

/// Recent pages the start page lists at most.
pub const RECENT: usize = 6;

/// The start page for theme `th`, searching with `engine`, listing
/// `recent` pages (title, address; newest first).
pub fn page_url(th: &Theme, engine: SearchEngine, recent: &[(String, String)]) -> String {
    let q = web::js_string;
    let list: Vec<String> = recent.iter().take(RECENT).map(|(t, u)| format!("[{},{}]", q(t), q(u))).collect();
    // The titles go in last, so nothing in them is taken for a placeholder.
    let html = PAGE
        .replacen("__PREFIX__", &q(web::APP_ACTION), 1)
        .replacen("__THEME__", &crate::welcome::colors(th), 1)
        .replacen("__DATA__", &crate::engines::data(engine), 1)
        .replacen("__RECENT__", &format!("[{}]", list.join(",")), 1);
    format!("data:text/html;charset=utf-8,{}", percent_encode(&html))
}

/// The pages a start page offers from `visits` (newest first): web pages
/// only, each once, at most [`RECENT`].
pub fn recent(visits: &[storage::pagetext::Visit]) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    visits
        .iter()
        .filter(|v| v.url.starts_with("https://") || v.url.starts_with("http://"))
        .filter(|v| seen.insert(v.url.as_str()))
        .take(RECENT)
        .map(|v| (v.title.trim().to_string(), v.url.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_gets_the_theme_prefix_and_engines() {
        let recent = vec![("Odd __THEME__ title".to_string(), "https://example.org/".to_string())];
        let url = page_url(&Theme::default(), SearchEngine::Google, &recent);
        assert!(url.starts_with("data:text/html;charset=utf-8,"));
        for p in ["tbTheme%28__THEME__", "__PREFIX__", "__DATA__", "__RECENT__"] {
            assert!(!url.contains(p), "{p}");
        }
        assert!(url.contains("Google") && url.contains("Mojeek"), "{url}");
        // The theme went where it belongs, not into the title.
        assert!(url.contains("Odd%20__THEME__%20title"), "{url}");
        assert!(url.contains("window.tbTheme%28%7B"), "{url}");
    }

    #[test]
    fn recent_pages_are_web_pages_once_each() {
        use storage::pagetext::Visit;
        let v = |url: &str, title: &str| Visit { url: url.into(), title: title.into(), seen_ms: 0 };
        let visits = vec![
            v("https://a.org/", " A "),
            v("trailboard:welcome", "Welcome"),
            v("https://a.org/", "A again"),
            v("http://b.org/x", ""),
        ];
        assert_eq!(recent(&visits), vec![("A".to_string(), "https://a.org/".to_string()), (String::new(), "http://b.org/x".to_string())]);
        let many: Vec<Visit> = (0..20).map(|i| v(&format!("https://{i}.org/"), "")).collect();
        assert_eq!(recent(&many).len(), RECENT);
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
