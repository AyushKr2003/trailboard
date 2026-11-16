//! Ad and tracker blocking on disk (`scene::privacy`), shared by every space:
//!
//! - `$DATA/privacy.json` = `{ "version": 1, "allowed": ["example.com"] }`,
//!   the sites where the user turned blocking off.
//! - `$DATA/filters/<id>.txt`: the fetched filter lists, as downloaded; their
//!   modification time says when they were fetched. `filters/user.txt` holds
//!   the user's own rules and is never overwritten.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use scene::privacy::{self, AllowedSites, LISTS, USER_LIST};
use serde::{Deserialize, Serialize};

use crate::write_atomic;

pub const PRIVACY_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct PrivacyFile {
    version: u32,
    #[serde(default)]
    allowed: Vec<String>,
}

pub fn privacy_path(data_dir: &Path) -> PathBuf {
    data_dir.join("privacy.json")
}

/// The sites where blocking is off. A missing, unreadable or newer file gives
/// none (a newer file is left alone until the user changes a site).
pub fn load_allowed(data_dir: &Path) -> AllowedSites {
    let path = privacy_path(data_dir);
    let Ok(bytes) = fs::read(&path) else { return AllowedSites::default() };
    match serde_json::from_slice::<PrivacyFile>(&bytes) {
        Ok(f) if f.version <= PRIVACY_VERSION => {
            let mut sites: Vec<String> = f.allowed.into_iter().map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect();
            sites.sort();
            sites.dedup();
            AllowedSites { sites }
        }
        Ok(f) => {
            log::warn!("{}: version {} is newer than this build supports", path.display(), f.version);
            AllowedSites::default()
        }
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            AllowedSites::default()
        }
    }
}

pub fn save_allowed(data_dir: &Path, allowed: &AllowedSites) -> std::io::Result<()> {
    let file = PrivacyFile { version: PRIVACY_VERSION, allowed: allowed.sites.clone() };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    write_atomic(&privacy_path(data_dir), &json)
}

pub fn filters_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("filters")
}

pub fn list_path(data_dir: &Path, id: &str) -> PathBuf {
    filters_dir(data_dir).join(format!("{id}.txt"))
}

/// Seconds since list `id` was saved, `None` if it isn't (or its time can't be read).
pub fn list_age(data_dir: &Path, id: &str, now: SystemTime) -> Option<u64> {
    let modified = fs::metadata(list_path(data_dir, id)).and_then(|m| m.modified()).ok()?;
    Some(now.duration_since(modified).map(|d| d.as_secs()).unwrap_or(0))
}

pub fn save_list(data_dir: &Path, id: &str, text: &str) -> std::io::Result<()> {
    write_atomic(&list_path(data_dir, id), text.as_bytes())
}

/// Whether `path`, from a change in the filters folder, is the user's own
/// list (`filters/user.txt`). Editors that save through a temporary file
/// and a rename report the final name too, so only that name counts.
pub fn is_user_list(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n == format!("{USER_LIST}.txt").as_str())
}

/// The user's own rules, `None` if `filters/user.txt` isn't there (or can't be read).
pub fn load_user_list(data_dir: &Path) -> Option<String> {
    fs::read(list_path(data_dir, USER_LIST)).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// The saved text of list (or part) `id`, `None` if missing or unreadable.
pub fn load_list(data_dir: &Path, id: &str) -> Option<String> {
    let path = list_path(data_dir, id);
    match fs::read(&path) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            None
        }
    }
}

/// The text of every list there is, ready for the engine: the fetched ones
/// in [`LISTS`] order (with their saved `!#include` parts spliced in and the
/// `!#if` blocks that don't apply dropped, see [`privacy::preprocess`]),
/// then the user's own, as written. Missing or unreadable lists are left out.
pub fn load_lists(data_dir: &Path) -> Vec<(String, String)> {
    let fetched = LISTS.iter().filter_map(|l| {
        let text = load_list(data_dir, l.id)?;
        let part = |name: &str| load_list(data_dir, &privacy::include_id(l.id, name));
        Some((l.id.to_string(), privacy::preprocess(&text, &part)))
    });
    let user = load_list(data_dir, USER_LIST).map(|t| (USER_LIST.to_string(), t));
    fetched.chain(user).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn allowed_round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_allowed(dir.path()), AllowedSites::default());
        let a = AllowedSites { sites: vec!["example.com".into(), "localhost:8761".into()] };
        save_allowed(dir.path(), &a).unwrap();
        assert_eq!(load_allowed(dir.path()), a);

        let path = privacy_path(dir.path());
        fs::write(&path, br#"{"version":1,"allowed":[" B.org ","a.net","a.net",""]}"#).unwrap();
        assert_eq!(load_allowed(dir.path()).sites, ["a.net", "b.org"]);
        fs::write(&path, br#"{"version":2,"allowed":["a.net"]}"#).unwrap();
        assert_eq!(load_allowed(dir.path()), AllowedSites::default(), "newer file: none");
        fs::write(&path, b"not json").unwrap();
        assert_eq!(load_allowed(dir.path()), AllowedSites::default());
    }

    #[test]
    fn lists_in_order_with_ages() {
        let dir = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        assert!(load_lists(dir.path()).is_empty());
        assert_eq!(list_age(dir.path(), "easylist", now), None);
        save_list(dir.path(), USER_LIST, "||mine.example^").unwrap();
        save_list(dir.path(), "easyprivacy", "[Adblock Plus]\n||t.example^").unwrap();
        save_list(dir.path(), "easylist", "[Adblock Plus]\n||a.example^").unwrap();
        let ids: Vec<String> = load_lists(dir.path()).into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, ["easylist", "easyprivacy", "user"]);
        let age = list_age(dir.path(), "easylist", now + Duration::from_secs(100)).unwrap();
        assert!((99..=101).contains(&age), "{age}");
        assert_eq!(list_age(dir.path(), "easylist", now - Duration::from_secs(100)), Some(0), "clock went back");
    }

    #[test]
    fn lists_come_with_their_parts() {
        let dir = tempfile::tempdir().unwrap();
        save_list(dir.path(), "ublock-filters", "! Title: uBlock filters\n||a^\n!#include filters-2024.txt\n!#if env_firefox\n||ff^\n!#endif\n").unwrap();
        let lists = load_lists(dir.path());
        assert_eq!(lists[0].1, "! Title: uBlock filters\n||a^\n", "a part not fetched yet is left out");
        save_list(dir.path(), &scene::privacy::include_id("ublock-filters", "filters-2024.txt"), "! part\n||b^").unwrap();
        let lists = load_lists(dir.path());
        assert_eq!(lists.len(), 1, "a part is not a list of its own");
        assert_eq!(lists[0].1, "! Title: uBlock filters\n||a^\n! part\n||b^\n");
    }

    #[test]
    fn user_list_path_and_text() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_user_list(&filters_dir(dir.path()).join("user.txt")));
        assert!(is_user_list(Path::new("user.txt")));
        for other in ["user.txt.tmp", ".user.txt.swp", "user.txt~", "easylist.txt", "filters"] {
            assert!(!is_user_list(&filters_dir(dir.path()).join(other)), "{other}");
        }
        assert_eq!(load_user_list(dir.path()), None);
        save_list(dir.path(), USER_LIST, "||mine.example^").unwrap();
        assert_eq!(load_user_list(dir.path()).as_deref(), Some("||mine.example^"));
    }
}
