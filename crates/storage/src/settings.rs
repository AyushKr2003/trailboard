//! The user's settings (`scene::settings`), shared by every space:
//! `$DATA/settings.json` =
//! `{ "version": 1, "search": "duckduckgo", "live_pages": 6, "history": true, "page_colours": "theme",
//! "scroll_speed": "normal", "block_ads": true, "new_card": "address_bar", "default_zoom": 100,
//! "language": "system", "spellcheck": true, "block_third_party_cookies": false,
//! "do_not_track": false, "javascript": true, "images": true, "camera": "ask",
//! "microphone": "ask", "location": "ask", "notifications": "ask",
//! "download_to": "downloads", "proxy": "system", "proxy_server": "",
//! "space_panel": true, "corners": "rounded", "corner_radius": 10,
//! "board_background": "mood" }`.
//!
//! Each field is read on its own: a missing or unknown value (say, an engine
//! a newer build added, or a field an older build didn't write) keeps its
//! default and the rest still load. Fields were only ever added, so the
//! version stays 1.

use std::fs;
use std::path::{Path, PathBuf};

use scene::settings::{self, Backdrop, Corners, DownloadTo, NewCard, PageColours, Permission, Proxy, ScrollSpeed, SearchEngine, Settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::write_atomic;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Serialize)]
struct SettingsFile<'a> {
    version: u32,
    search: &'a str,
    live_pages: usize,
    history: bool,
    page_colours: &'a str,
    scroll_speed: &'a str,
    block_ads: bool,
    new_card: &'a str,
    default_zoom: u16,
    language: &'a str,
    spellcheck: bool,
    block_third_party_cookies: bool,
    do_not_track: bool,
    javascript: bool,
    images: bool,
    camera: &'a str,
    microphone: &'a str,
    location: &'a str,
    notifications: &'a str,
    download_to: &'a str,
    proxy: &'a str,
    proxy_server: &'a str,
    space_panel: bool,
    corners: &'a str,
    corner_radius: u8,
    board_background: &'a str,
}

#[derive(Deserialize)]
struct Head {
    version: u32,
}

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("settings.json")
}

/// The saved settings. A missing or unreadable file gives the defaults; so
/// does a newer one, which is left alone until a setting changes.
pub fn load_settings(data_dir: &Path) -> Settings {
    let path = settings_path(data_dir);
    let Ok(bytes) = fs::read(&path) else { return Settings::default() };
    let (head, v) = match (serde_json::from_slice::<Head>(&bytes), serde_json::from_slice::<Value>(&bytes)) {
        (Ok(h), Ok(v)) => (h, v),
        (Err(e), _) | (_, Err(e)) => {
            log::warn!("{}: {e}", path.display());
            return Settings::default();
        }
    };
    if head.version > SETTINGS_VERSION {
        log::warn!("{}: version {} is newer than this build supports", path.display(), head.version);
        return Settings::default();
    }
    let mut s = Settings::default();
    let flag = |name: &str, into: &mut bool| {
        if let Some(b) = v[name].as_bool() {
            *into = b;
        }
    };
    let text = |name: &str| v[name].as_str();
    if let Some(e) = text("search").and_then(SearchEngine::from_id) {
        s.search = e;
    }
    if let Some(n) = v["live_pages"].as_u64().filter(|&n| (1..=64).contains(&n)) {
        s.live_pages = n as usize;
    }
    flag("history", &mut s.history);
    if let Some(c) = text("page_colours").and_then(PageColours::from_id) {
        s.page_colours = c;
    }
    if let Some(c) = text("scroll_speed").and_then(ScrollSpeed::from_id) {
        s.scroll_speed = c;
    }
    flag("block_ads", &mut s.block_ads);
    if let Some(c) = text("new_card").and_then(NewCard::from_id) {
        s.new_card = c;
    }
    if let Some(z) = v["default_zoom"].as_u64().filter(|&z| (25..=500).contains(&z)) {
        s.default_zoom = z as u16;
    }
    if let Some(l) = text("language").and_then(settings::language_from_id) {
        s.language = l;
    }
    flag("spellcheck", &mut s.spellcheck);
    flag("block_third_party_cookies", &mut s.block_third_party_cookies);
    flag("do_not_track", &mut s.do_not_track);
    flag("javascript", &mut s.javascript);
    flag("images", &mut s.images);
    flag("space_panel", &mut s.space_panel);
    for (name, into) in [
        ("camera", &mut s.camera),
        ("microphone", &mut s.microphone),
        ("location", &mut s.location),
        ("notifications", &mut s.notifications),
    ] {
        if let Some(p) = text(name).and_then(Permission::from_id) {
            *into = p;
        }
    }
    if let Some(d) = text("download_to").and_then(DownloadTo::from_id) {
        s.download_to = d;
    }
    if let Some(p) = text("proxy").and_then(Proxy::from_id) {
        s.proxy = p;
    }
    if let Some(p) = text("proxy_server") {
        s.proxy_server = p.trim().to_string();
    }
    if let Some(b) = text("board_background").and_then(Backdrop::from_id) {
        s.backdrop = b;
    }
    if let Some(c) = text("corners").and_then(Corners::from_id) {
        s.corners = c;
    }
    if let Some(r) = v["corner_radius"].as_u64().filter(|&r| r <= u64::from(settings::MAX_CORNER_RADIUS)) {
        s.corner_radius = r as u8;
    }
    s
}

pub fn save_settings(data_dir: &Path, s: &Settings) -> std::io::Result<()> {
    let file = SettingsFile {
        version: SETTINGS_VERSION,
        search: s.search.id(),
        live_pages: s.live_pages,
        history: s.history,
        page_colours: s.page_colours.id(),
        scroll_speed: s.scroll_speed.id(),
        block_ads: s.block_ads,
        new_card: s.new_card.id(),
        default_zoom: s.default_zoom,
        language: s.language,
        spellcheck: s.spellcheck,
        block_third_party_cookies: s.block_third_party_cookies,
        do_not_track: s.do_not_track,
        javascript: s.javascript,
        images: s.images,
        camera: s.camera.id(),
        microphone: s.microphone.id(),
        location: s.location.id(),
        notifications: s.notifications.id(),
        download_to: s.download_to.id(),
        proxy: s.proxy.id(),
        proxy_server: &s.proxy_server,
        space_panel: s.space_panel,
        corners: s.corners.id(),
        corner_radius: s.corner_radius,
        board_background: s.backdrop.id(),
    };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    write_atomic(&settings_path(data_dir), &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
        let s = Settings {
            search: SearchEngine::Kagi,
            live_pages: 3,
            history: false,
            page_colours: PageColours::Dark,
            scroll_speed: ScrollSpeed::Faster,
            block_ads: false,
            new_card: NewCard::StartPage,
            default_zoom: 125,
            language: "hi-IN",
            spellcheck: false,
            block_third_party_cookies: true,
            do_not_track: true,
            javascript: false,
            images: false,
            camera: Permission::Allow,
            microphone: Permission::Block,
            location: Permission::Block,
            notifications: Permission::Allow,
            download_to: DownloadTo::Ask,
            proxy: Proxy::Custom,
            proxy_server: "socks5://127.0.0.1:9050".into(),
            space_panel: false,
            corners: Corners::Custom,
            corner_radius: 6,
            backdrop: Backdrop::Photo,
        };
        save_settings(dir.path(), &s).unwrap();
        assert_eq!(load_settings(dir.path()), s);

        let path = settings_path(dir.path());
        fs::write(&path, b"{ nope").unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
        fs::write(&path, br#"{"version":2,"search":"google"}"#).unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
        // Unknown or bad values keep their defaults; the others load.
        fs::write(&path, br#"{"version":1,"search":"altavista","live_pages":0,"history":false,"page_colours":7}"#).unwrap();
        assert_eq!(load_settings(dir.path()), Settings { history: false, ..Settings::default() });
        fs::write(&path, br#"{"version":1,"search":"bing"}"#).unwrap();
        assert_eq!(load_settings(dir.path()).search, SearchEngine::Bing);
        // A file from before the browsing settings: they take their defaults.
        fs::write(&path, br#"{"version":1,"search":"bing","block_ads":false,"default_zoom":9000,"language":"xx","camera":"maybe"}"#).unwrap();
        assert_eq!(load_settings(dir.path()), Settings { search: SearchEngine::Bing, block_ads: false, ..Settings::default() });
    }
}
