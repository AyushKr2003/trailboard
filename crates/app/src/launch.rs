//! What to open for the words Trailboard was started with: `trailboard URL…`
//! from a shell, `trailboard %U` from the desktop entry, or the same handed to
//! the running instance by a second launch (`WebEvent::Relaunch`).

use std::path::Path;

use crate::omnibox;

/// The page to open for one command-line argument, read against the directory
/// the command ran in: an existing file or folder becomes a `file://` URL, a
/// URL is kept, a host gets `https://`, and anything else is searched for.
/// Switches (`-x`, `--x`) and blank words open nothing.
pub fn target(arg: &str, cwd: &Path) -> Option<String> {
    let arg = arg.trim();
    if arg.is_empty() || arg.starts_with('-') {
        return None;
    }
    // A scheme first: "https://a.b" is never a relative path, and checking
    // the disk for it is pointless.
    let has_scheme = arg.split_once(':').is_some_and(|(s, _)| {
        s.len() > 1 && s.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    });
    if !has_scheme {
        let path = cwd.join(arg); // an absolute `arg` replaces `cwd`
        if path.exists() {
            let path = path.canonicalize().unwrap_or(path);
            return Some(file_url(&path));
        }
    }
    omnibox::resolve(arg)
}

/// `file://` URL of an absolute path, with the bytes a URL path can't hold
/// percent-encoded.
fn file_url(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut out = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_hosts_and_searches() {
        let cwd = Path::new("/nonexistent-dir");
        assert_eq!(target("https://a.b/c?d", cwd).as_deref(), Some("https://a.b/c?d"));
        assert_eq!(target("example.com", cwd).as_deref(), Some("https://example.com"));
        assert_eq!(target("about:blank", cwd).as_deref(), Some("about:blank"));
        assert_eq!(target("rust lang", cwd).as_deref(), Some("https://duckduckgo.com/?q=rust+lang"));
        assert_eq!(target("--new-window", cwd), None);
        assert_eq!(target("  ", cwd), None);
    }

    #[test]
    fn files_against_the_working_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a b#1.html"), "x").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/page.html"), "x").unwrap();
        let base = format!("file://{}", root.display());

        assert_eq!(target("a b#1.html", &root), Some(format!("{base}/a%20b%231.html")));
        assert_eq!(target("sub/../sub/page.html", &root), Some(format!("{base}/sub/page.html")));
        let abs = root.join("sub/page.html");
        assert_eq!(target(abs.to_str().unwrap(), Path::new("/")), Some(format!("{base}/sub/page.html")));
        assert_eq!(target("sub", &root), Some(format!("{base}/sub")));
        // A missing file that looks like a host is a host, as typed.
        assert_eq!(target("missing.html", &root).as_deref(), Some("https://missing.html"));
    }
}
