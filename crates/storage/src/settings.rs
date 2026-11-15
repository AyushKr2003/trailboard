//! The user's settings (`scene::settings`), shared by every space:
//! `$DATA/settings.json` =
//! `{ "version": 1, "search": "duckduckgo", "live_pages": 6, "history": true, "page_colours": "theme",
//! "scroll_speed": "normal", "block_ads": true }`.
//!
//! Each field is read on its own: a missing or unknown value (say, an engine
//! a newer build added) keeps its default and the rest still load.

use std::fs;
use std::path::{Path, PathBuf};

use scene::settings::{PageColours, ScrollSpeed, SearchEngine, Settings};
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
    if let Some(e) = v["search"].as_str().and_then(SearchEngine::from_id) {
        s.search = e;
    }
    if let Some(n) = v["live_pages"].as_u64().filter(|&n| (1..=64).contains(&n)) {
        s.live_pages = n as usize;
    }
    if let Some(b) = v["history"].as_bool() {
        s.history = b;
    }
    if let Some(c) = v["page_colours"].as_str().and_then(PageColours::from_id) {
        s.page_colours = c;
    }
    if let Some(c) = v["scroll_speed"].as_str().and_then(ScrollSpeed::from_id) {
        s.scroll_speed = c;
    }
    if let Some(b) = v["block_ads"].as_bool() {
        s.block_ads = b;
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
    }
}
