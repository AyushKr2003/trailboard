//! Saved pages (`scene::saved::Saved`), shared by every space:
//! `$DATA/saved.json` =
//! `{ "version": 1, "pages": [ { "name": "Rust docs", "url": "https://doc.rust-lang.org/", "added": 0 } ] }`,
//! newest first.

use std::fs;
use std::path::{Path, PathBuf};

use scene::saved::{Saved, SavedPage};
use serde::{Deserialize, Serialize};

use crate::write_atomic;

pub const SAVED_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct SavedFile {
    version: u32,
    #[serde(default)]
    pages: Vec<SavedPage>,
}

pub fn saved_path(data_dir: &Path) -> PathBuf {
    data_dir.join("saved.json")
}

/// The saved pages. A missing, unreadable or newer file gives none.
pub fn load(data_dir: &Path) -> Saved {
    let path = saved_path(data_dir);
    let Ok(bytes) = fs::read(&path) else { return Saved::default() };
    match serde_json::from_slice::<SavedFile>(&bytes) {
        Ok(f) if f.version <= SAVED_VERSION => {
            let n = f.pages.len();
            let s = Saved::from_pages(f.pages);
            if s.pages().len() < n {
                log::warn!("{}: skipped {} bad or repeated page(s)", path.display(), n - s.pages().len());
            }
            s
        }
        Ok(f) => {
            log::warn!("{}: version {} is newer than this build supports", path.display(), f.version);
            Saved::default()
        }
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            Saved::default()
        }
    }
}

pub fn save(data_dir: &Path, saved: &Saved) -> std::io::Result<()> {
    let file = SavedFile { version: SAVED_VERSION, pages: saved.pages().to_vec() };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    write_atomic(&saved_path(data_dir), &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).pages().is_empty());
        let mut s = Saved::default();
        s.save(Some("Rust docs"), "https://doc.rust-lang.org/", "", 1).unwrap();
        s.save(None, "https://example.com/", "Example", 2).unwrap();
        save(dir.path(), &s).unwrap();
        assert_eq!(load(dir.path()), s);
        fs::write(saved_path(dir.path()), b"[").unwrap();
        assert!(load(dir.path()).pages().is_empty());
        fs::write(saved_path(dir.path()), br#"{"version":2,"pages":[{"name":"a","url":"b"}]}"#).unwrap();
        assert!(load(dir.path()).pages().is_empty());
        fs::write(saved_path(dir.path()), br#"{"version":1,"pages":[{"name":"a","url":"https://a"},{"name":"A","url":"https://b"}]}"#).unwrap();
        assert_eq!(load(dir.path()).pages().len(), 1);
    }
}
