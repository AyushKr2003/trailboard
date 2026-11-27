//! The start page: an empty card with a clock, a search engine picker, one
//! search box, the user's pinned sites and the pages opened last
//! (`start.html`), opened when Trailboard starts with nothing to show or on
//! *New card*. The board keeps it as `scene::note::START_URL`; like the
//! welcome page it is a `data:` URL with the theme's colours filled in.
//!
//! Its messages (after `web::APP_ACTION`) are [`GO`] and the text typed (or
//! a pinned or recent page's address), which the app loads in the same card
//! (it then becomes an ordinary page); [`BESIDE`] and an address, opened in
//! a new card beside the start page, joined by a trail as a link would be
//! (Ctrl+click, middle click); [`ENGINE`] and an engine's id;
//! [`GUIDE`] to open the search engine guide; and [`SAVE`] and the page's
//! groups and parts as JSON (`storage::start`) whenever the user changes them.

use std::collections::BTreeMap;

use scene::settings::SearchEngine;
use scene::startpage::StartPage;

use crate::pages::note::percent_encode;
use crate::ui::theme::Theme;

const PAGE: &str = include_str!("start.html");

/// What comes before the typed text in the page's message.
pub const GO: &str = "go:";
/// What comes before an address to open beside the start page.
pub const BESIDE: &str = "beside:";
/// What comes before an engine's id when one is chosen (start page or guide).
pub const ENGINE: &str = "engine:";
/// Asks for the search engine guide.
pub const GUIDE: &str = "engines";
/// What comes before the page's pins and parts (JSON) when they change.
pub const SAVE: &str = "start:";

/// Recent pages the start page shows at most (one row of small cards).
pub const RECENT: usize = 4;

/// A page opened lately, as the start page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recent {
    pub title: String,
    pub url: String,
    /// What its card last showed, a JPEG `data:` URL, if history kept one.
    pub thumb: Option<String>,
}

/// `bytes` as a `data:` URL of type `mime`.
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    use base64::Engine as _;
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Site icons the page can show, as `data:` URLs by site
/// (`scene::icon::site`), for the pinned and recent pages.
pub type Icons = BTreeMap<String, String>;

/// The icons of the sites of `urls` that `icon` knows, as PNG `data:` URLs.
pub fn icons<'a>(urls: impl IntoIterator<Item = &'a str>, icon: impl Fn(&str) -> Option<Vec<u8>>) -> Icons {
    let mut out = Icons::new();
    for site in urls.into_iter().filter_map(scene::icon::site) {
        if !out.contains_key(&site)
            && let Some(png) = icon(&site)
        {
            out.insert(site, data_url("image/png", &png));
        }
    }
    out
}

/// What the page gets about its pins: the pins and parts and the icons, as
/// a JS expression (a JSON string the page parses, so nothing in a title
/// can escape it).
fn state(start: &StartPage, icons: &Icons) -> String {
    let icons = serde_json::to_string(icons).expect("strings serialise");
    let json = format!("{{\"start\":{},\"icons\":{icons}}}", storage::start::to_json(start));
    // No `__` in it, so nothing in a title is taken for a placeholder.
    web::js_string(&json).replace("__", "_\\u005f")
}

/// Tells a live start page its pins and parts changed (another start page
/// changed them).
pub fn state_script(start: &StartPage, icons: &Icons) -> String {
    format!("window.tbStart && window.tbStart(JSON.parse({}))", state(start, icons))
}

/// The start page for theme `th`, searching with `engine`, showing `start`
/// and `recent` pages (newest first).
pub fn page_url(th: &Theme, engine: SearchEngine, start: &StartPage, recent: &[Recent], icons: &Icons) -> String {
    let q = web::js_string;
    let list: Vec<String> = recent
        .iter()
        .take(RECENT)
        .map(|r| format!("[{},{},{}]", q(&r.title), q(&r.url), r.thumb.as_deref().map_or("null".into(), q)))
        .collect();
    // The titles go in last, so nothing in them is taken for a placeholder.
    let html = PAGE
        .replacen("__PREFIX__", &q(web::APP_ACTION), 1)
        .replacen("__THEME__", &crate::pages::welcome::colors(th), 1)
        .replacen("__DATA__", &crate::pages::engines::data(engine), 1)
        .replacen("__STATE__", &state(start, icons), 1)
        .replacen("__RECENT__", &format!("[{}]", list.join(",")), 1);
    format!("data:text/html;charset=utf-8,{}", percent_encode(&html))
}

/// The pages a start page offers from `visits` (newest first): web pages
/// only, each once, at most [`RECENT`].
/// Their thumbnails are filled in by the app.
pub fn recent(visits: &[storage::pagetext::Visit]) -> Vec<Recent> {
    let mut seen = std::collections::HashSet::new();
    visits
        .iter()
        .filter(|v| v.url.starts_with("https://") || v.url.starts_with("http://"))
        .filter(|v| seen.insert(v.url.as_str()))
        .take(RECENT)
        .map(|v| Recent { title: v.title.trim().to_string(), url: v.url.clone(), thumb: None })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_gets_the_theme_prefix_and_engines() {
        let recent = vec![Recent { title: "Odd __THEME__ title".into(), url: "https://example.org/".into(), thumb: Some(data_url("image/jpeg", b"jpg")) }];
        let mut start = StartPage::default();
        start.pin("https://pinned.org/", "Pinned </script> __RECENT__");
        let url = page_url(&Theme::default(), SearchEngine::Google, &start, &recent, &Icons::new());
        assert!(url.starts_with("data:text/html;charset=utf-8,"));
        for p in ["tbTheme%28__THEME__", "__PREFIX__", "__DATA__", "__RECENT__", "__STATE__", "%3C/script%3E%20__"] {
            assert!(!url.contains(p), "{p}");
        }
        assert!(url.contains("Google") && url.contains("Mojeek"), "{url}");
        // The theme went where it belongs, not into the title.
        assert!(url.contains("Odd%20__THEME__%20title"), "{url}");
        assert!(url.contains("window.tbTheme%28%7B"), "{url}");
        assert!(url.contains("pinned.org"), "{url}");
        assert!(url.contains("image%2Fjpeg") && url.contains("anBn"), "{url}");
    }

    #[test]
    fn icons_are_png_data_urls_per_site() {
        let urls = ["https://a.org/x", "https://a.org/y", "https://b.org/", "about:blank"];
        let icons = icons(urls, |site| (site == "a.org").then(|| vec![1, 2, 3]));
        assert_eq!(icons.len(), 1);
        assert_eq!(icons["a.org"], "data:image/png;base64,AQID");
        let script = state_script(&StartPage::default(), &icons);
        assert!(script.starts_with("window.tbStart && window.tbStart(JSON.parse(\""), "{script}");
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
        let got: Vec<_> = recent(&visits).into_iter().map(|r| (r.title, r.url)).collect();
        assert_eq!(got, [("A".to_string(), "https://a.org/".to_string()), (String::new(), "http://b.org/x".to_string())]);
        let many: Vec<Visit> = (0..20).map(|i| v(&format!("https://{i}.org/"), "")).collect();
        assert_eq!(recent(&many).len(), RECENT);
    }

    #[test]
    fn the_pages_send_only_known_messages() {
        for page in [PAGE, include_str!("engines.html")] {
            for msg in page.split("act(\"").skip(1).filter_map(|s| s.split('"').next()) {
                assert!([GO, BESIDE, ENGINE, GUIDE, SAVE].contains(&msg), "unknown message {msg}");
            }
        }
    }
}
