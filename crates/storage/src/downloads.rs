//! Downloads: the folder files go to, the name each one gets there, and the
//! list (`scene::download`) kept in `$DATA/downloads.json` =
//! `{ "version": 1, "downloads": [ { "url", "path", "received", "total", "state", "started_ms" } ] }`,
//! newest first. Entries that don't read are skipped; a newer version is
//! ignored and left alone until a download changes the list.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use scene::download::{numbered, safe_name, Download, Downloads, State};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::write_atomic;

pub const DOWNLOADS_VERSION: u32 = 1;

pub fn downloads_path(data_dir: &Path) -> PathBuf {
    data_dir.join("downloads.json")
}

#[derive(Serialize)]
struct Entry<'a> {
    url: &'a str,
    path: &'a str,
    received: i64,
    total: i64,
    state: &'a str,
    started_ms: i64,
}

#[derive(Serialize)]
struct File<'a> {
    version: u32,
    downloads: Vec<Entry<'a>>,
}

#[derive(Deserialize)]
struct Head {
    version: u32,
}

/// The saved list (empty when missing, unreadable or newer).
pub fn load_downloads(data_dir: &Path) -> Downloads {
    let path = downloads_path(data_dir);
    let Ok(bytes) = fs::read(&path) else { return Downloads::default() };
    let (head, v) = match (serde_json::from_slice::<Head>(&bytes), serde_json::from_slice::<Value>(&bytes)) {
        (Ok(h), Ok(v)) => (h, v),
        (Err(e), _) | (_, Err(e)) => {
            log::warn!("{}: {e}", path.display());
            return Downloads::default();
        }
    };
    if head.version > DOWNLOADS_VERSION {
        log::warn!("{}: version {} is newer than this build supports", path.display(), head.version);
        return Downloads::default();
    }
    let entries = v["downloads"].as_array().map(Vec::as_slice).unwrap_or_default();
    let items = entries
        .iter()
        .filter_map(|e| {
            Some(Download {
                key: 0,
                url: e["url"].as_str()?.to_string(),
                path: e["path"].as_str().filter(|p| p.starts_with('/'))?.to_string(),
                received: e["received"].as_i64().unwrap_or(0),
                total: e["total"].as_i64().unwrap_or(0),
                state: e["state"].as_str().and_then(State::from_id)?,
                started_ms: e["started_ms"].as_i64().unwrap_or(0),
                private: false,
            })
        })
        .collect();
    Downloads::from_saved(items)
}

pub fn save_downloads(data_dir: &Path, list: &Downloads) -> std::io::Result<()> {
    let downloads = list
        .saved()
        .into_iter()
        .map(|d| Entry {
            url: &d.url,
            path: &d.path,
            received: d.received,
            total: d.total,
            state: d.state.id(),
            started_ms: d.started_ms,
        })
        .collect();
    let json = serde_json::to_vec_pretty(&File { version: DOWNLOADS_VERSION, downloads }).map_err(std::io::Error::other)?;
    write_atomic(&downloads_path(data_dir), &json)
}

/// The folder downloads go to: `TRAILBOARD_DOWNLOAD_DIR` if set, else the
/// desktop's Downloads folder (`XDG_DOWNLOAD_DIR` in `user-dirs.dirs`), else
/// `~/Downloads`.
pub fn download_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("TRAILBOARD_DOWNLOAD_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let config = std::env::var("XDG_CONFIG_HOME").ok().filter(|c| c.starts_with('/')).unwrap_or_else(|| format!("{home}/.config"));
    let dirs = fs::read_to_string(Path::new(&config).join("user-dirs.dirs")).unwrap_or_default();
    user_download_dir(&dirs, &home).unwrap_or_else(|| Path::new(&home).join("Downloads"))
}

/// `XDG_DOWNLOAD_DIR` from the text of `user-dirs.dirs`: a quoted path,
/// absolute or starting with `$HOME/`. Set to `$HOME` itself means the folder
/// is turned off (per the spec), which we treat as unset.
pub fn user_download_dir(dirs: &str, home: &str) -> Option<PathBuf> {
    let line = dirs.lines().map(str::trim).find(|l| l.starts_with("XDG_DOWNLOAD_DIR="))?;
    let value = line["XDG_DOWNLOAD_DIR=".len()..].trim().strip_prefix('"')?.strip_suffix('"')?;
    let value = value.replace("\\\"", "\"");
    let path = if value == "$HOME" || value == "$HOME/" {
        return None;
    } else if let Some(rest) = value.strip_prefix("$HOME/") {
        Path::new(home).join(rest)
    } else if value.starts_with('/') {
        PathBuf::from(value)
    } else {
        return None;
    };
    (path != Path::new(home)).then_some(path)
}

/// Where a file Chromium suggests calling `suggested` is saved in `dir`: its
/// safe name, numbered "a (1).pdf" if a file (or Chromium's partial
/// `.crdownload`) of that name exists or another download in progress
/// (`taken`) is headed there.
pub fn target(dir: &Path, suggested: &str, taken: &HashSet<PathBuf>) -> PathBuf {
    let name = safe_name(suggested);
    let free = |p: &Path| {
        let partial = PathBuf::from(format!("{}.crdownload", p.display()));
        !taken.contains(p) && fs::symlink_metadata(p).is_err() && fs::symlink_metadata(&partial).is_err()
    };
    for n in 0..10_000 {
        let p = dir.join(numbered(&name, n));
        if free(&p) {
            return p;
        }
    }
    dir.join(numbered(&name, std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_dirs() {
        let home = "/home/u";
        let f = "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/Téléchargements\"\n";
        assert_eq!(user_download_dir(f, home), Some(PathBuf::from("/home/u/Téléchargements")));
        assert_eq!(user_download_dir("XDG_DOWNLOAD_DIR=\"/data/dl\"", home), Some(PathBuf::from("/data/dl")));
        assert_eq!(user_download_dir("XDG_DOWNLOAD_DIR=\"$HOME/\"", home), None);
        assert_eq!(user_download_dir("XDG_DOWNLOAD_DIR=\"$HOME\"", home), None);
        assert_eq!(user_download_dir("XDG_DOWNLOAD_DIR=relative", home), None);
        assert_eq!(user_download_dir("", home), None);
    }

    #[test]
    fn targets_skip_taken_names() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let mut taken = HashSet::new();
        assert_eq!(target(d, "a.pdf", &taken), d.join("a.pdf"));
        fs::write(d.join("a.pdf"), b"x").unwrap();
        assert_eq!(target(d, "a.pdf", &taken), d.join("a (1).pdf"));
        fs::write(d.join("a (1).pdf.crdownload"), b"x").unwrap();
        taken.insert(d.join("a (2).pdf"));
        assert_eq!(target(d, "a.pdf", &taken), d.join("a (3).pdf"));
        assert_eq!(target(d, "../evil", &taken), d.join("evil"));
    }

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_downloads(dir.path()).is_empty());
        let mut l = Downloads::default();
        let a = l.start("https://x.org/a.pdf", "/d/a.pdf", false, 5);
        l.update(a, 10, 10, State::Done);
        l.start("https://x.org/b", "/d/b", false, 6); // running at quit
        l.start("https://x.org/p", "/d/p", true, 7); // private: not saved
        save_downloads(dir.path(), &l).unwrap();
        let back: Vec<Download> = load_downloads(dir.path()).iter().cloned().collect();
        assert_eq!(back.len(), 2);
        assert_eq!((back[0].path.as_str(), back[0].state), ("/d/b", State::Stopped));
        assert_eq!((back[1].path.as_str(), back[1].state, back[1].total), ("/d/a.pdf", State::Done, 10));

        let path = downloads_path(dir.path());
        fs::write(&path, b"{ nope").unwrap();
        assert!(load_downloads(dir.path()).is_empty());
        fs::write(&path, br#"{"version":2,"downloads":[]}"#).unwrap();
        assert!(load_downloads(dir.path()).is_empty());
        // Bad entries are skipped, good ones load.
        fs::write(
            &path,
            br#"{"version":1,"downloads":[{"url":"u","path":"rel","state":"done"},{"url":"u","path":"/ok","state":"weird"},{"url":"u","path":"/ok","state":"failed"}]}"#,
        )
        .unwrap();
        let l = load_downloads(dir.path());
        assert_eq!(l.iter().map(|d| (d.path.as_str(), d.state)).collect::<Vec<_>>(), [("/ok", State::Failed)]);
    }
}
