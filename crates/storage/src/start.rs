//! The start page's groups and parts (`scene::startpage`), shared by every
//! space: `$DATA/start.json` =
//! `{ "version": 2, "groups": [{ "name": "Work", "pins": [{ "title": "", "url": "https://…" }] }],
//! "show_greeting": true, "name": "", "show_clock": true, "hour24": false, "show_pins": true,
//! "show_recent": true }`. (`hour24` was added within version 2; files without it read as false.)
//!
//! Version 1 had one flat `pins` list (no groups, no greeting); it is read
//! as a single group, "Pinned". The start page sends the version 2 shape
//! (without `version`) when the user changes something; [`from_json`] reads
//! both, field by field, so a missing or wrong field keeps its default.

use std::fs;
use std::path::{Path, PathBuf};

use scene::startpage::{FIRST_GROUP, Group, Pin, StartPage};
use serde::Serialize;
use serde_json::Value;

use crate::write_atomic;

pub const START_VERSION: u32 = 2;

#[derive(Serialize)]
struct PinOut<'a> {
    title: &'a str,
    url: &'a str,
}

#[derive(Serialize)]
struct GroupOut<'a> {
    name: &'a str,
    pins: Vec<PinOut<'a>>,
}

#[derive(Serialize)]
struct StartFile<'a> {
    version: u32,
    groups: Vec<GroupOut<'a>>,
    show_greeting: bool,
    name: &'a str,
    show_clock: bool,
    hour24: bool,
    show_pins: bool,
    show_recent: bool,
}

pub fn start_path(data_dir: &Path) -> PathBuf {
    data_dir.join("start.json")
}

fn pins(v: &Value) -> Vec<Pin> {
    let Some(list) = v.as_array() else { return Vec::new() };
    list.iter()
        .filter_map(|p| {
            let url = p["url"].as_str()?;
            Some(Pin { title: p["title"].as_str().unwrap_or("").to_string(), url: url.to_string() })
        })
        .collect()
}

/// A start page from JSON (the file's, of any version, or the page's),
/// cleaned; `None` if it isn't a JSON object.
pub fn from_json(text: &str) -> Option<StartPage> {
    let v: Value = serde_json::from_str(text).ok()?;
    if !v.is_object() {
        return None;
    }
    let mut s = StartPage::default();
    for (key, into) in [
        ("show_greeting", &mut s.show_greeting),
        ("show_clock", &mut s.show_clock),
        ("hour24", &mut s.hour24),
        ("show_pins", &mut s.show_pins),
        ("show_recent", &mut s.show_recent),
    ] {
        if let Some(b) = v[key].as_bool() {
            *into = b;
        }
    }
    if let Some(n) = v["name"].as_str() {
        s.name = n.to_string();
    }
    if let Some(groups) = v["groups"].as_array() {
        s.groups = groups
            .iter()
            .filter(|g| g.is_object())
            .map(|g| Group { name: g["name"].as_str().unwrap_or("").to_string(), pins: pins(&g["pins"]) })
            .collect();
    } else {
        // Version 1: one flat list.
        let flat = pins(&v["pins"]);
        if !flat.is_empty() {
            s.groups = vec![Group { name: FIRST_GROUP.into(), pins: flat }];
        }
    }
    Some(s.cleaned())
}

/// `s` as JSON, in the file's shape (the page reads it too).
pub fn to_json(s: &StartPage) -> String {
    let file = StartFile {
        version: START_VERSION,
        groups: s
            .groups
            .iter()
            .map(|g| GroupOut { name: &g.name, pins: g.pins.iter().map(|p| PinOut { title: &p.title, url: &p.url }).collect() })
            .collect(),
        show_greeting: s.show_greeting,
        name: &s.name,
        show_clock: s.show_clock,
        hour24: s.hour24,
        show_pins: s.show_pins,
        show_recent: s.show_recent,
    };
    serde_json::to_string_pretty(&file).expect("plain data serialises")
}

/// The saved start page. A missing or unreadable file gives the default;
/// so does a newer one, which is left alone until something changes.
pub fn load_start(data_dir: &Path) -> StartPage {
    let path = start_path(data_dir);
    let Ok(text) = fs::read_to_string(&path) else { return StartPage::default() };
    let version = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["version"].as_u64());
    match version {
        Some(v) if v <= u64::from(START_VERSION) => from_json(&text).unwrap_or_default(),
        Some(v) => {
            log::warn!("{}: version {v} is newer than this build supports", path.display());
            StartPage::default()
        }
        None => {
            log::warn!("{}: not a start page file", path.display());
            StartPage::default()
        }
    }
}

pub fn save_start(data_dir: &Path, s: &StartPage) -> std::io::Result<()> {
    write_atomic(&start_path(data_dir), to_json(s).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_start(dir.path()), StartPage::default());
        let mut s = StartPage { show_clock: false, hour24: true, name: "Sam".into(), ..StartPage::default() };
        s.pin("https://a.org/", "A \"quoted\" <title>");
        s.pin("https://b.org/", "");
        s.groups.push(Group { name: "Empty".into(), pins: vec![] });
        save_start(dir.path(), &s).unwrap();
        assert_eq!(load_start(dir.path()), s);

        fs::write(start_path(dir.path()), b"{ nope").unwrap();
        assert_eq!(load_start(dir.path()), StartPage::default());
        fs::write(start_path(dir.path()), br#"{"version":9,"show_clock":false}"#).unwrap();
        assert_eq!(load_start(dir.path()), StartPage::default());
    }

    #[test]
    fn version_1_becomes_one_group() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            start_path(dir.path()),
            br#"{"version":1,"pins":[{"title":"A","url":"https://a.org/"},{"title":"","url":"https://b.org/"}],"show_clock":false,"show_pins":true,"show_recent":true}"#,
        )
        .unwrap();
        let s = load_start(dir.path());
        assert!(!s.show_clock && s.show_greeting && !s.hour24);
        assert_eq!(s.groups.len(), 1);
        assert_eq!(s.groups[0].name, FIRST_GROUP);
        assert_eq!(s.groups[0].pins.iter().map(|p| p.url.as_str()).collect::<Vec<_>>(), ["https://a.org/", "https://b.org/"]);
        fs::write(start_path(dir.path()), br#"{"version":1,"pins":[]}"#).unwrap();
        assert!(load_start(dir.path()).groups.is_empty());
    }

    #[test]
    fn page_messages_are_read_field_by_field() {
        let s = from_json(
            r#"{"show_recent":false,"show_pins":"yes","name":42,"groups":[{"name":"W","pins":[{"url":"https://a.org/"},{"title":"x"},{"url":"file:///etc/passwd"}]},"junk"]}"#,
        )
        .unwrap();
        assert!(!s.show_recent && s.show_pins && s.show_clock && s.name.is_empty());
        assert_eq!(s.groups, vec![Group { name: "W".into(), pins: vec![Pin { title: String::new(), url: "https://a.org/".into() }] }]);
        assert_eq!(from_json("[1]"), None);
        assert_eq!(from_json("nope"), None);
    }
}
