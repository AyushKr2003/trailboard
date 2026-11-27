//! Downloads: the list the downloads view (Ctrl+J) shows, the names files are
//! saved under, and how sizes and progress read. Pure: the app feeds it
//! Chromium's updates and does the file system work.

/// Where a download stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Running,
    Done,
    /// Cancelled by the user (or the page).
    Cancelled,
    /// Chromium gave up (network, disk, server).
    Failed,
    /// Trailboard quit while it was running; there is no resuming it.
    Stopped,
}

impl State {
    pub fn id(self) -> &'static str {
        match self {
            State::Running => "running",
            State::Done => "done",
            State::Cancelled => "cancelled",
            State::Failed => "failed",
            State::Stopped => "stopped",
        }
    }

    pub fn from_id(s: &str) -> Option<State> {
        Some(match s {
            "running" => State::Running,
            "done" => State::Done,
            "cancelled" => State::Cancelled,
            "failed" => State::Failed,
            "stopped" => State::Stopped,
            _ => return None,
        })
    }
}

/// One file in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    /// The list's own key for it (stable for the session, not saved).
    pub key: u64,
    pub url: String,
    /// Where it is (or will be) saved.
    pub path: String,
    pub received: i64,
    /// Total size in bytes; 0 or less when the server didn't say.
    pub total: i64,
    pub state: State,
    /// When it started, UTC ms.
    pub started_ms: i64,
    /// Started in a private space: listed this session only, never saved.
    pub private: bool,
}

impl Download {
    /// The file name, from its path.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

/// Downloads kept in `downloads.json` at most (newest first); older ones drop off.
pub const KEEP: usize = 200;

/// The downloads list, newest first.
#[derive(Debug, Default, Clone)]
pub struct Downloads {
    items: Vec<Download>,
    next_key: u64,
}

impl Downloads {
    /// The list as saved last time. Anything still running then was cut off
    /// when Trailboard quit.
    pub fn from_saved(saved: Vec<Download>) -> Downloads {
        let mut d = Downloads::default();
        for mut item in saved.into_iter().take(KEEP) {
            if item.state == State::Running {
                item.state = State::Stopped;
            }
            item.key = d.next();
            item.private = false;
            d.items.push(item);
        }
        d
    }

    fn next(&mut self) -> u64 {
        self.next_key += 1;
        self.next_key
    }

    /// Adds a download that is starting; returns its key.
    pub fn start(&mut self, url: &str, path: &str, private: bool, now_ms: i64) -> u64 {
        let key = self.next();
        self.items.insert(
            0,
            Download {
                key,
                url: url.to_string(),
                path: path.to_string(),
                received: 0,
                total: 0,
                state: State::Running,
                started_ms: now_ms,
                private,
            },
        );
        self.items.truncate(KEEP * 2); // private ones aren't saved, so keep a little more
        key
    }

    /// Chromium's latest word on download `key`. Returns the new state when it
    /// just changed (finished, failed…), so the app can say so once. A
    /// download that has ended stays as it ended.
    pub fn update(&mut self, key: u64, received: i64, total: i64, state: State) -> Option<State> {
        let d = self.items.iter_mut().find(|d| d.key == key)?;
        if d.state != State::Running {
            return None;
        }
        d.received = received.max(0);
        if total > 0 {
            d.total = total;
        }
        if state == State::Done && d.total <= 0 {
            d.total = d.received;
        }
        (state != State::Running).then(|| {
            d.state = state;
            state
        })
    }

    /// Takes download `key` off the list (the file stays).
    pub fn remove(&mut self, key: u64) -> Option<Download> {
        let i = self.items.iter().position(|d| d.key == key)?;
        Some(self.items.remove(i))
    }

    pub fn get(&self, key: u64) -> Option<&Download> {
        self.items.iter().find(|d| d.key == key)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Download> {
        self.items.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Paths of downloads still running: taken, though no file may exist yet.
    pub fn running_paths(&self) -> impl Iterator<Item = &str> {
        self.items.iter().filter(|d| d.state == State::Running).map(|d| d.path.as_str())
    }

    /// What `downloads.json` keeps: the newest [`KEEP`] that weren't private.
    pub fn saved(&self) -> Vec<&Download> {
        self.items.iter().filter(|d| !d.private).take(KEEP).collect()
    }
}

/// A file name that is safe to save under, from what the server or page
/// suggested: one path component, no control characters, not hidden, not
/// empty, at most 200 bytes (the extension kept).
pub fn safe_name(suggested: &str) -> String {
    let last = suggested.rsplit(['/', '\\']).next().unwrap_or("");
    let clean: String = last.chars().map(|c| if c.is_control() { '_' } else { c }).collect();
    let clean = clean.trim().trim_start_matches(['.', ' ']).trim_end_matches(['.', ' ']);
    let name = if clean.is_empty() { "download" } else { clean };
    if name.len() <= 200 {
        return name.to_string();
    }
    let (stem, ext) = split_ext(name);
    let ext = if ext.len() > 20 { "" } else { ext };
    let mut cut = 200 - ext.len();
    while !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{ext}", &stem[..cut])
}

/// `report.tar.gz` → (`report`, `.tar.gz`); `a.pdf` → (`a`, `.pdf`); `README` → (`README`, ``).
fn split_ext(name: &str) -> (&str, &str) {
    let Some(dot) = name.rfind('.').filter(|&i| i > 0) else { return (name, "") };
    let (stem, ext) = name.split_at(dot);
    // Keep compressed tarballs whole: "a (1).tar.gz", not "a.tar (1).gz".
    if let Some(t) = stem.rfind(".tar").filter(|&i| i > 0 && i + 4 == stem.len()) {
        return name.split_at(t);
    }
    (stem, ext)
}

/// The `n`th name to try for `name` when it is taken: `a.pdf`, `a (1).pdf`, `a (2).pdf`…
pub fn numbered(name: &str, n: u32) -> String {
    if n == 0 {
        return name.to_string();
    }
    let (stem, ext) = split_ext(name);
    format!("{stem} ({n}){ext}")
}

/// `1536` → "1.5 KB": one decimal below 10, whole numbers above (1000-based, as GNOME's Files).
pub fn size_label(bytes: i64) -> String {
    let b = bytes.max(0) as f64;
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if b < 1000.0 {
        return format!("{} bytes", bytes.max(0));
    }
    let mut v = b / 1000.0;
    let mut unit = UNITS[0];
    for u in &UNITS[1..] {
        if v < 999.5 {
            break;
        }
        v /= 1000.0;
        unit = u;
    }
    if v < 9.95 { format!("{v:.1} {unit}") } else { format!("{v:.0} {unit}") }
}

/// What a row says about a download, after its site: progress while it runs
/// ("4.2 MB of 10 MB · 42%", or just "4.2 MB" when the size is unknown), else
/// how it ended. `missing`: a finished file is no longer where it was saved.
pub fn status(d: &Download, missing: bool) -> String {
    match d.state {
        State::Running if d.total > 0 => {
            let pct = (d.received as f64 * 100.0 / d.total as f64).floor().clamp(0.0, 100.0);
            format!("{} of {} · {pct:.0}%", size_label(d.received), size_label(d.total))
        }
        State::Running => size_label(d.received),
        State::Done if missing => "Deleted or moved".to_string(),
        State::Done => size_label(d.total.max(d.received)),
        State::Cancelled => "Cancelled".to_string(),
        State::Failed => "Failed".to_string(),
        State::Stopped => "Stopped when Trailboard closed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(safe_name("report.pdf"), "report.pdf");
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("a\\b\\c.txt"), "c.txt");
        assert_eq!(safe_name(".bashrc"), "bashrc");
        assert_eq!(safe_name("  ..  "), "download");
        assert_eq!(safe_name(""), "download");
        assert_eq!(safe_name("a\nb\tc.txt"), "a_b_c.txt");
        let long = format!("{}.pdf", "é".repeat(150));
        let s = safe_name(&long);
        assert!(s.len() <= 200 && s.ends_with(".pdf"), "{s}");

        assert_eq!(numbered("a.pdf", 0), "a.pdf");
        assert_eq!(numbered("a.pdf", 1), "a (1).pdf");
        assert_eq!(numbered("src.tar.gz", 2), "src (2).tar.gz");
        assert_eq!(numbered("README", 3), "README (3)");
        assert_eq!(numbered("v1.2.zip", 1), "v1.2 (1).zip");
        assert_eq!(numbered(".tar.gz", 1), ".tar (1).gz");
    }

    #[test]
    fn sizes() {
        assert_eq!(size_label(0), "0 bytes");
        assert_eq!(size_label(999), "999 bytes");
        assert_eq!(size_label(1536), "1.5 KB");
        assert_eq!(size_label(42_000), "42 KB");
        assert_eq!(size_label(999_600), "1.0 MB");
        assert_eq!(size_label(4_200_000), "4.2 MB");
        assert_eq!(size_label(3_000_000_000), "3.0 GB");
    }

    #[test]
    fn list_lifecycle() {
        let mut l = Downloads::default();
        let a = l.start("https://x.org/a.pdf", "/d/a.pdf", false, 10);
        let b = l.start("https://x.org/b.zip", "/d/b.zip", true, 20);
        assert_eq!(l.iter().map(|d| d.key).collect::<Vec<_>>(), [b, a]); // newest first
        assert_eq!(l.running_paths().count(), 2);

        assert_eq!(l.update(a, 500, 1000, State::Running), None);
        assert_eq!(status(l.get(a).unwrap(), false), "500 bytes of 1.0 KB · 50%");
        assert_eq!(l.update(a, 1000, 1000, State::Done), Some(State::Done));
        // Late updates after the end change nothing.
        assert_eq!(l.update(a, 1, 1, State::Failed), None);
        assert_eq!(status(l.get(a).unwrap(), false), "1.0 KB");
        assert_eq!(status(l.get(a).unwrap(), true), "Deleted or moved");

        // Unknown size: the received bytes become the size once done.
        assert_eq!(status(l.get(b).unwrap(), false), "0 bytes");
        l.update(b, 2500, -1, State::Done);
        assert_eq!(l.get(b).unwrap().total, 2500);

        // Private downloads are never saved.
        assert_eq!(l.saved().iter().map(|d| d.key).collect::<Vec<_>>(), [a]);
        assert!(l.remove(a).is_some());
        assert!(l.get(a).is_none());
        assert_eq!(l.update(999, 0, 0, State::Done), None);
    }

    #[test]
    fn running_at_quit_becomes_stopped() {
        let mut l = Downloads::default();
        let k = l.start("u", "/d/f", false, 1);
        l.update(k, 10, 100, State::Running);
        let saved: Vec<Download> = l.saved().into_iter().cloned().collect();
        let back = Downloads::from_saved(saved);
        let d = back.iter().next().unwrap();
        assert_eq!(d.state, State::Stopped);
        assert_eq!(status(d, false), "Stopped when Trailboard closed");
        assert_eq!(back.running_paths().count(), 0);
        for s in [State::Running, State::Done, State::Cancelled, State::Failed, State::Stopped] {
            assert_eq!(State::from_id(s.id()), Some(s));
        }
    }
}
