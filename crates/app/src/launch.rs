//! What to open for the words Trailboard was started with: `trailboard URL…`
//! from a shell, `trailboard %U` from the desktop entry, or the same handed to
//! the running instance by a second launch (`WebEvent::Relaunch`).

use std::path::Path;

use scene::settings::Settings;

/// The page to open for one command-line argument, read against the directory
/// the command ran in: an existing file or folder becomes a `file://` URL, a
/// URL is kept, a host gets `https://`, and anything else is searched for
/// with the settings' engine (or a keyword's site: `gh foo`).
/// Switches (`-x`, `--x`) and blank words open nothing.
pub fn target(arg: &str, cwd: &Path, settings: &Settings) -> Option<String> {
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
    settings.resolve_url(arg)
}

/// What a command line asks for before anything starts.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Open the window (with these arguments' pages).
    Run,
    /// `--version` / `-V`: print the version and exit.
    Version,
    /// `--help` / `-h`: print the usage and exit.
    Help,
}

/// Read the arguments after the program name. `--version` and `--help` win
/// anywhere before a `--`; other switches are left to Chromium (its helper
/// processes re-run this binary with `--type=…`).
pub fn command<S: AsRef<str>>(args: &[S]) -> Command {
    for a in args {
        match a.as_ref() {
            "--" => break,
            "--version" | "-V" => return Command::Version,
            "--help" | "-h" => return Command::Help,
            _ => {}
        }
    }
    Command::Run
}

/// `trailboard 0.1.0 (CEF …)`, for `--version` and crash reports.
pub fn version_line() -> String {
    format!("trailboard {} (CEF {})", env!("CARGO_PKG_VERSION"), web::engine_version())
}

pub const USAGE: &str = "\
Usage: trailboard [URL | FILE | WORDS]...

Opens each argument as a card: a URL, a file or folder (relative to the
current directory), a host name, or words to search for. With Trailboard
already running, the pages open in its window.

Options:
  -h, --help     Show this help and exit
  -V, --version  Show the version and exit

Data (board, history, settings, crash.log) is kept in
$XDG_DATA_HOME/trailboard (default ~/.local/share/trailboard).
";

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
    fn version_and_help_switches() {
        assert_eq!(command::<&str>(&[]), Command::Run);
        assert_eq!(command(&["example.com"]), Command::Run);
        assert_eq!(command(&["--version"]), Command::Version);
        assert_eq!(command(&["a.com", "-V"]), Command::Version);
        assert_eq!(command(&["-h"]), Command::Help);
        assert_eq!(command(&["--help", "--version"]), Command::Help);
        assert_eq!(command(&["--", "--version"]), Command::Run);
        assert_eq!(command(&["--type=renderer", "--lang=en-US"]), Command::Run);
        assert!(version_line().starts_with(&format!("trailboard {} (CEF ", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn urls_hosts_and_searches() {
        let cwd = Path::new("/nonexistent-dir");
        assert_eq!(target("https://a.b/c?d", cwd, &Settings::default()).as_deref(), Some("https://a.b/c?d"));
        assert_eq!(target("example.com", cwd, &Settings::default()).as_deref(), Some("https://example.com"));
        assert_eq!(target("about:blank", cwd, &Settings::default()).as_deref(), Some("about:blank"));
        assert_eq!(target("rust lang", cwd, &Settings::default()).as_deref(), Some("https://duckduckgo.com/?q=rust+lang"));
        assert_eq!(target("--new-window", cwd, &Settings::default()), None);
        assert_eq!(target("  ", cwd, &Settings::default()), None);
    }

    #[test]
    fn files_against_the_working_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a b#1.html"), "x").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/page.html"), "x").unwrap();
        let base = format!("file://{}", root.display());

        assert_eq!(target("a b#1.html", &root, &Settings::default()), Some(format!("{base}/a%20b%231.html")));
        assert_eq!(target("sub/../sub/page.html", &root, &Settings::default()), Some(format!("{base}/sub/page.html")));
        let abs = root.join("sub/page.html");
        assert_eq!(target(abs.to_str().unwrap(), Path::new("/"), &Settings::default()), Some(format!("{base}/sub/page.html")));
        assert_eq!(target("sub", &root, &Settings::default()), Some(format!("{base}/sub")));
        // A missing file that looks like a host is a host, as typed.
        assert_eq!(target("missing.html", &root, &Settings::default()).as_deref(), Some("https://missing.html"));
    }
}
