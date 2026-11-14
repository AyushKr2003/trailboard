//! Where "Save as Markdown" writes: `Trailboard/` in the user's Documents
//! folder, one file per save named after the note or space.
//!
//! The Documents folder is `$XDG_DOCUMENTS_DIR` when set, else the one named in
//! `~/.config/user-dirs.dirs` (`xdg-user-dirs`), else `~/Documents`. A name
//! already taken gets a number (`Trip ideas 2.md`); saving the same note or
//! space again in one session overwrites its earlier file unless its name has
//! changed since.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// Folder inside Documents that saves go to.
pub const FOLDER: &str = "Trailboard";

/// Longest file name taken from a title, in characters (before ` 2.md`).
const NAME_CHARS: usize = 80;

/// `Documents/Trailboard` for this user (see the module docs).
pub fn export_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let user_dirs = fs::read_to_string(config.join("user-dirs.dirs")).ok();
    documents_dir(std::env::var_os("XDG_DOCUMENTS_DIR"), &home, user_dirs.as_deref()).join(FOLDER)
}

/// The Documents folder from the environment variable, the `user-dirs.dirs`
/// text, or `home/Documents`, in that order. Relative paths are ignored.
pub fn documents_dir(env: Option<OsString>, home: &Path, user_dirs: Option<&str>) -> PathBuf {
    if let Some(p) = env.map(PathBuf::from).filter(|p| p.is_absolute()) {
        return p;
    }
    let from_file = user_dirs.and_then(|text| {
        text.lines().find_map(|line| {
            let value = line.trim().strip_prefix("XDG_DOCUMENTS_DIR=")?;
            let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
            let p = match value.strip_prefix("$HOME") {
                Some(rest) => home.join(rest.trim_start_matches('/')),
                None => PathBuf::from(value),
            };
            p.is_absolute().then_some(p)
        })
    });
    from_file.unwrap_or_else(|| home.join("Documents"))
}

/// A file name (without `.md`) for `title`: path separators and control
/// characters become spaces, leading dots go, cut at [`NAME_CHARS`];
/// `fallback` when nothing is left.
pub fn file_stem(title: &str, fallback: &str) -> String {
    let clean: String = title.chars().map(|c| if c == '/' || c == '\\' || c.is_control() { ' ' } else { c }).collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_start_matches('.').trim();
    let cut: String = clean.chars().take(NAME_CHARS).collect();
    let cut = cut.trim().to_string();
    if cut.is_empty() { fallback.to_string() } else { cut }
}

/// Writes `text` as `dir/stem.md` (atomically, creating `dir`) and returns the
/// path. `previous` is the file this note or space was saved to before: it is
/// overwritten when it is still `stem.md` or `stem N.md` in `dir`; otherwise a
/// name nobody has taken is used.
pub fn save_markdown(dir: &Path, stem: &str, text: &str, previous: Option<&Path>) -> std::io::Result<PathBuf> {
    let path = match previous.filter(|p| is_named(p, dir, stem)) {
        Some(p) => p.to_path_buf(),
        None => free_name(dir, stem),
    };
    crate::write_atomic(&path, text.as_bytes())?;
    Ok(path)
}

/// `dir/stem.md`, or `dir/stem 2.md`, `dir/stem 3.md`, … if taken.
fn free_name(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.md"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} {n}.md")))
        .find(|p| !p.exists())
        .expect("some number is free")
}

/// `p` is `dir/stem.md` or `dir/stem N.md`.
fn is_named(p: &Path, dir: &Path, stem: &str) -> bool {
    if p.parent() != Some(dir) {
        return false;
    }
    let Some(name) = p.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".md")) else {
        return false;
    };
    name == stem
        || name
            .strip_prefix(stem)
            .and_then(|r| r.strip_prefix(' '))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_folder_comes_from_env_file_or_home() {
        let home = Path::new("/home/u");
        let file = "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOCUMENTS_DIR=\"$HOME/Docs\"\n";
        assert_eq!(documents_dir(Some("/data/docs".into()), home, Some(file)), Path::new("/data/docs"));
        assert_eq!(documents_dir(Some("rel".into()), home, Some(file)), Path::new("/home/u/Docs"));
        assert_eq!(documents_dir(None, home, Some("XDG_DOCUMENTS_DIR=\"/srv/d\"")), Path::new("/srv/d"));
        assert_eq!(documents_dir(None, home, Some("XDG_DOCUMENTS_DIR=\"$HOME/\"")), Path::new("/home/u"));
        assert_eq!(documents_dir(None, home, Some("XDG_DOCUMENTS_DIR=\"rel\"")), Path::new("/home/u/Documents"));
        assert_eq!(documents_dir(None, home, None), Path::new("/home/u/Documents"));
    }

    #[test]
    fn file_stems_are_safe() {
        assert_eq!(file_stem("Trip ideas", "Note"), "Trip ideas");
        assert_eq!(file_stem("a/b\\c\td\n", "Note"), "a b c d");
        assert_eq!(file_stem("..hidden", "Note"), "hidden");
        assert_eq!(file_stem(" / . ", "Note"), "Note");
        assert_eq!(file_stem(&"é".repeat(100), "Note").chars().count(), NAME_CHARS);
    }

    #[test]
    fn saves_take_a_free_name_and_overwrite_their_own() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Trailboard");
        let a = save_markdown(&dir, "Trip", "one\n", None).unwrap();
        assert_eq!(a, dir.join("Trip.md"));
        let b = save_markdown(&dir, "Trip", "other note\n", None).unwrap();
        assert_eq!(b, dir.join("Trip 2.md"));
        // The second note saved again: same file.
        assert_eq!(save_markdown(&dir, "Trip", "other, edited\n", Some(&b)).unwrap(), b);
        assert_eq!(fs::read_to_string(&b).unwrap(), "other, edited\n");
        assert_eq!(fs::read_to_string(&a).unwrap(), "one\n");
        // Renamed since: a new file, the old one kept.
        let c = save_markdown(&dir, "Holiday", "renamed\n", Some(&b)).unwrap();
        assert_eq!(c, dir.join("Holiday.md"));
        assert!(b.exists());
        assert!(!dir.join("Holiday.md.tmp").exists());
    }

    #[test]
    fn own_names_are_recognised() {
        let d = Path::new("/d");
        assert!(is_named(Path::new("/d/T.md"), d, "T"));
        assert!(is_named(Path::new("/d/T 12.md"), d, "T"));
        assert!(!is_named(Path::new("/d/T x.md"), d, "T"));
        assert!(!is_named(Path::new("/d/T .md"), d, "T"));
        assert!(!is_named(Path::new("/e/T.md"), d, "T"));
        assert!(!is_named(Path::new("/d/Tea.md"), d, "T"));
    }
}
