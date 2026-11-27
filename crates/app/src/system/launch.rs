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

/// `trailboard 0.3.0 (CEF …, media build)`, for `--version`, the start log
/// and crash reports. `cef_dir` is where the engine was loaded from.
pub fn version_line(cef_dir: &Path) -> String {
    let build = std::fs::read_to_string(cef_dir.join("archive.json"))
        .map(|j| EngineBuild::from_archive_json(&j))
        .unwrap_or(EngineBuild::Unknown);
    let mut line = format!("trailboard {} (CEF {}", env!("CARGO_PKG_VERSION"), web::engine_version());
    if let Some(name) = build.name() {
        line.push_str(", ");
        line.push_str(name);
    }
    line.push(')');
    line
}

/// What the start log warns when the loaded engine isn't the CEF version
/// this binary was built for (the one `scripts/install-cef.sh` pins):
/// usually an engine left behind by an update. `None` when they match.
pub fn engine_mismatch(built: web::EngineVersion, loaded: web::EngineVersion) -> Option<String> {
    let v = |(a, b, c): web::EngineVersion| format!("{a}.{b}.{c}");
    let how = match loaded.cmp(&built) {
        std::cmp::Ordering::Equal => return None,
        std::cmp::Ordering::Less => "older than",
        std::cmp::Ordering::Greater => "newer than",
    };
    Some(format!(
        "the installed CEF {} is {how} the {} this Trailboard was built for; \
         install the matching one with scripts/install-cef.sh (--codecs for the media build)",
        v(loaded),
        v(built),
    ))
}

/// Which CEF build is installed: CEF's standard download, or a media build
/// with the patent-licensed codecs (H.264, AAC, HEVC). Read from the
/// `archive.json` that `scripts/install-cef.sh` and the packages write
/// beside the engine (`"codecs": true` marks a media build).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineBuild {
    Standard,
    Media,
    /// No readable `archive.json` (a hand-made install).
    Unknown,
}

impl EngineBuild {
    pub fn from_archive_json(json: &str) -> Self {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
            return Self::Unknown;
        };
        if !v.is_object() {
            return Self::Unknown;
        }
        match v.get("codecs") {
            Some(serde_json::Value::Bool(true)) => Self::Media,
            _ => Self::Standard,
        }
    }

    pub fn name(self) -> Option<&'static str> {
        match self {
            Self::Standard => Some("standard build"),
            Self::Media => Some("media build"),
            Self::Unknown => None,
        }
    }
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
        let none = Path::new("/nonexistent");
        assert!(version_line(none).starts_with(&format!("trailboard {} (CEF ", env!("CARGO_PKG_VERSION"))));
        assert!(!version_line(none).contains("build)"));
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

    #[test]
    fn engine_mismatch_names_both_versions() {
        assert_eq!(engine_mismatch((156, 0, 26), (156, 0, 26)), None);
        let old = engine_mismatch((156, 0, 26), (156, 0, 20)).unwrap();
        assert!(old.contains("CEF 156.0.20 is older than the 156.0.26"), "{old}");
        assert!(old.contains("scripts/install-cef.sh"));
        let new = engine_mismatch((156, 0, 26), (157, 0, 1)).unwrap();
        assert!(new.contains("157.0.1 is newer than"), "{new}");
        // Compared as numbers, not text: 156.0.9 is older than 156.0.26.
        assert!(engine_mismatch((156, 0, 26), (156, 0, 9)).unwrap().contains("older"));
    }

    /// The value of `name=…` (quotes and a trailing `# comment` dropped) in a
    /// shell file, from its first such line.
    fn shell_var<'a>(file: &'a str, name: &str) -> &'a str {
        let prefix = format!("{name}=");
        let line = file.lines().map(str::trim).find(|l| l.starts_with(&prefix)).unwrap_or_else(|| panic!("no {name}="));
        let value = line[prefix.len()..].split(" #").next().unwrap().trim();
        value.trim_matches('"')
    }

    /// The engine pins live in three places that must move together (see
    /// "Updating CEF" in the README): the `cef` crate this is built with,
    /// `scripts/install-cef.sh` and the Arch PKGBUILD.
    #[test]
    fn engine_pins_agree() {
        let script = include_str!("../../../../scripts/install-cef.sh");
        let pkgbuild = include_str!("../../../../packaging/arch/PKGBUILD");
        let built = web::engine_version();
        let (major, minor, patch) = web::built_engine_version();
        let tag = format!("cef-{major}.{minor}.{patch}-media");

        assert_eq!(shell_var(script, "CEF_VERSION"), built, "install-cef.sh pins another CEF than the cef crate");
        assert_eq!(shell_var(pkgbuild, "_cef"), built, "the PKGBUILD pins another CEF than the cef crate");
        assert_eq!(shell_var(script, "COD_TAG"), tag, "install-cef.sh's media release isn't this version's");
        assert!(pkgbuild.contains(&format!("/releases/download/{tag}/")), "the PKGBUILD's media release isn't {tag}");

        // Same checksums in both. The PKGBUILD sets the standard SHA-1 at
        // the top and the media pins in its first branch, the standard
        // SHA-256 in its second.
        let all = |v: &str| -> Vec<&str> {
            pkgbuild
                .lines()
                .map(str::trim)
                .filter_map(|l| l.strip_prefix(&format!("{v}=")))
                .map(|x| x.split(" #").next().unwrap().trim().trim_matches('"'))
                .collect()
        };
        assert_eq!(all("_cefsha1"), [shell_var(script, "STD_SHA1"), shell_var(script, "COD_SHA1")]);
        assert_eq!(all("_cefsha256"), [shell_var(script, "COD_SHA256"), shell_var(script, "STD_SHA256")]);
    }

    #[test]
    fn engine_build_from_archive_json() {
        let media = r#"{"type": "minimal", "name": "cef_binary_x.tar.bz2", "sha1": "", "codecs": true}"#;
        let standard = r#"{"type": "minimal", "name": "cef_binary_x.tar.bz2", "sha1": "ab"}"#;
        assert_eq!(EngineBuild::from_archive_json(media), EngineBuild::Media);
        assert_eq!(EngineBuild::from_archive_json(standard), EngineBuild::Standard);
        assert_eq!(EngineBuild::from_archive_json(r#"{"codecs": false}"#), EngineBuild::Standard);
        assert_eq!(EngineBuild::from_archive_json("not json"), EngineBuild::Unknown);
        assert_eq!(EngineBuild::from_archive_json("[]"), EngineBuild::Unknown);
        assert_eq!(EngineBuild::Media.name(), Some("media build"));
        assert_eq!(EngineBuild::Unknown.name(), None);
    }
}
