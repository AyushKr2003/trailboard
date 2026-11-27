//! Typing a path for a page's file chooser (M12 step 15): `~` for the
//! home folder, Tab completion, and `*` in the last part to pick several.
//! The folder reads are passed in, so the logic is tested without a disk.

use std::path::{Path, PathBuf};

/// A folder's entries: (name, is a folder).
pub type Listing = Vec<(String, bool)>;

/// The path `text` names: `~` and `~/…` start at `home`; anything else
/// must be absolute (`None` otherwise).
pub fn expand(text: &str, home: &Path) -> Option<PathBuf> {
    let text = text.trim();
    if text == "~" {
        return Some(home.to_path_buf());
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return Some(home.join(rest));
    }
    let p = PathBuf::from(text);
    p.is_absolute().then_some(p)
}

/// `path` as typed: under `home` it starts with `~/`.
pub fn shorten(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

/// Tab: the text with its last part completed as far as the folder's
/// names agree (a lone folder gets its `/`). Names starting with a dot
/// only when the part does. `None`: nothing matches, or nothing to add.
pub fn complete(text: &str, home: &Path, list: impl Fn(&Path) -> Listing) -> Option<String> {
    let (head, part) = match text.rfind('/') {
        Some(i) => text.split_at(i + 1),
        None if text == "~" => return Some("~/".to_string()),
        None => return None,
    };
    let dir = expand(head, home)?;
    let mut hits: Vec<(String, bool)> =
        list(&dir).into_iter().filter(|(n, _)| n.starts_with(part) && (part.starts_with('.') || !n.starts_with('.'))).collect();
    hits.sort();
    let first = hits.first()?;
    let common = hits.iter().fold(first.0.clone(), |acc, (n, _)| {
        acc.chars().zip(n.chars()).take_while(|(a, b)| a == b).map(|(a, _)| a).collect()
    });
    let mut out = format!("{head}{common}");
    if hits.len() == 1 && first.1 {
        out.push('/');
    }
    (out != text).then_some(out)
}

/// Whether `name` fits `pattern`, where `*` stands for any run of characters.
pub fn fits(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || name.len() < first.len() + last.len() || !name.ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    true
}

/// The files `path` picks: itself, or with a `*` in its last part every
/// file there that fits (sorted; hidden ones only for a pattern starting
/// with a dot).
pub fn pick(path: &Path, list: impl Fn(&Path) -> Listing) -> Vec<PathBuf> {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else { return vec![path.to_path_buf()] };
    if !name.contains('*') {
        return vec![path.to_path_buf()];
    }
    let dir = path.parent().unwrap_or(Path::new("/"));
    let mut names: Vec<String> = list(dir)
        .into_iter()
        .filter(|(n, folder)| !folder && fits(&name, n) && (name.starts_with('.') || !n.starts_with('.')))
        .map(|(n, _)| n)
        .collect();
    names.sort();
    names.into_iter().map(|n| dir.join(n)).collect()
}

/// A folder's entries from the disk (empty when it can't be read).
pub fn read_dir(dir: &Path) -> Listing {
    std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(|e| e.ok())
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path().is_dir()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(dir: &Path) -> Listing {
        let names: &[(&str, bool)] = match dir.to_str().unwrap().trim_end_matches('/') {
            "/home/u" => &[("Documents", true), ("Downloads", true), ("Desktop", true), (".config", true), ("notes.txt", false)],
            "/home/u/Documents" => &[("cv.pdf", false), ("cv-old.pdf", false), ("photo.png", false), (".hidden.pdf", false)],
            _ => &[],
        };
        names.iter().map(|(n, d)| (n.to_string(), *d)).collect()
    }

    #[test]
    fn paths_expand_from_home_or_must_be_absolute() {
        let home = Path::new("/home/u");
        assert_eq!(expand("~", home), Some(PathBuf::from("/home/u")));
        assert_eq!(expand(" ~/a/b ", home), Some(PathBuf::from("/home/u/a/b")));
        assert_eq!(expand("/etc/x", home), Some(PathBuf::from("/etc/x")));
        assert_eq!(expand("a/b", home), None);
        assert_eq!(shorten(Path::new("/home/u/Documents"), home), "~/Documents");
        assert_eq!(shorten(Path::new("/etc"), home), "/etc");
    }

    #[test]
    fn tab_completes_as_far_as_names_agree() {
        let home = Path::new("/home/u");
        assert_eq!(complete("~/Do", home, disk), None); // Documents and Downloads
        assert_eq!(complete("~/n", home, disk).as_deref(), Some("~/notes.txt"));
        assert_eq!(complete("~/Doc", home, disk).as_deref(), Some("~/Documents/"));
        assert_eq!(complete("~/Documents/cv", home, disk).as_deref(), None); // cv.pdf and cv-old.pdf agree on "cv" only
        assert_eq!(complete("~/Documents/p", home, disk).as_deref(), Some("~/Documents/photo.png"));
        assert_eq!(complete("~/Documents/.h", home, disk).as_deref(), Some("~/Documents/.hidden.pdf"));
        assert_eq!(complete("~/De", home, disk).as_deref(), Some("~/Desktop/"));
        assert_eq!(complete("~", home, disk).as_deref(), Some("~/"));
        assert_eq!(complete("~/zz", home, disk), None);
        assert_eq!(complete("rel/x", home, disk), None);
    }

    #[test]
    fn stars_pick_several_files() {
        assert!(fits("*.pdf", "cv.pdf"));
        assert!(fits("cv*", "cv-old.pdf"));
        assert!(fits("c*o*.pdf", "cv-old.pdf"));
        assert!(!fits("*.pdf", "photo.png"));
        assert!(!fits("a*a", "a"));
        assert!(fits("x", "x") && !fits("x", "y"));
        let got = pick(Path::new("/home/u/Documents/*.pdf"), disk);
        assert_eq!(got, [PathBuf::from("/home/u/Documents/cv-old.pdf"), PathBuf::from("/home/u/Documents/cv.pdf")]);
        assert_eq!(pick(Path::new("/home/u/Documents/cv.pdf"), disk), [PathBuf::from("/home/u/Documents/cv.pdf")]);
        assert!(pick(Path::new("/home/u/Documents/*.doc"), disk).is_empty());
    }
}
