//! Editing a page's text field in your own editor (`Ctrl+e` in Insert mode).
//!
//! The field's text goes to a private file; the editor runs on it, in a
//! terminal unless `TRAILBOARD_EDITOR` names a command that opens its own
//! window; when the editor ends (or, for a terminal that hands the work to
//! one already running and returns at once, when the file is saved) the
//! text goes back into the field.

use std::path::{Path, PathBuf};
use std::time::Instant;

use scene::NodeId;

/// A text field being edited in the user's editor (`Ctrl+e`).
pub(crate) struct Editing {
    pub(crate) node: NodeId,
    /// The frame the field is in and the token it was read under.
    pub(crate) frame: String,
    pub(crate) token: u64,
    /// The field's text as it was read.
    pub(crate) original: String,
    /// The file the editor has (once the text came).
    pub(crate) file: Option<std::path::PathBuf>,
    /// The editor (or its terminal) while it runs.
    pub(crate) child: Option<std::process::Child>,
    /// When the editor started, or the last step was taken.
    pub(crate) since: Instant,
    /// The file's time when written, to see the editor save it.
    pub(crate) written: Option<std::time::SystemTime>,
    /// The new text was sent to the page; waiting for its answer.
    pub(crate) sent: bool,
}

/// Terminals tried, in order, when neither `xdg-terminal-exec` nor
/// `$TERMINAL` is there. Each takes `-e program args…`.
const TERMINALS: &[&str] = &["foot", "kitty", "alacritty", "ghostty", "wezterm", "konsole", "gnome-terminal", "xterm"];

/// The command that edits `file`: `env` reads an environment variable,
/// `found` says whether a program is on `PATH`.
///
/// - `TRAILBOARD_EDITOR` is the whole command, run as is (`gvim -f`,
///   `code --wait`); `{file}` in it is the file, else the file goes last.
/// - Else `$VISUAL`, `$EDITOR` or `vi` runs in a terminal:
///   `xdg-terminal-exec`, `$TERMINAL -e`, or the first of [`TERMINALS`].
///
/// Words are split at spaces (no shell, no quoting). `None`: no terminal.
pub fn command(file: &Path, env: impl Fn(&str) -> Option<String>, found: impl Fn(&str) -> bool) -> Option<Vec<String>> {
    let file = file.to_string_lossy().into_owned();
    let set = |name: &str| env(name).filter(|v| !v.trim().is_empty());
    if let Some(own) = set("TRAILBOARD_EDITOR") {
        let mut words: Vec<String> = own.split_whitespace().map(str::to_string).collect();
        if words.iter().any(|w| w.contains("{file}")) {
            for w in &mut words {
                *w = w.replace("{file}", &file);
            }
        } else {
            words.push(file);
        }
        return Some(words);
    }
    let editor = set("VISUAL").or_else(|| set("EDITOR")).unwrap_or_else(|| "vi".to_string());
    let mut edit: Vec<String> = editor.split_whitespace().map(str::to_string).collect();
    edit.push(file);
    let mut words: Vec<String> = if found("xdg-terminal-exec") {
        vec!["xdg-terminal-exec".to_string()]
    } else if let Some(t) = set("TERMINAL") {
        let mut w: Vec<String> = t.split_whitespace().map(str::to_string).collect();
        w.push("-e".to_string());
        w
    } else {
        let t = TERMINALS.iter().find(|t| found(t))?;
        vec![t.to_string(), "-e".to_string()]
    };
    words.extend(edit);
    Some(words)
}

/// Whether `name` is a program on `PATH`.
pub fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// Where field `token`'s text waits while it is edited: the user's private
/// runtime folder (never written to disk on most systems), else the cache.
pub fn file_for(token: u64) -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"))
        .join("trailboard");
    dir.join(format!("field-{token}.txt"))
}

/// Writes `text` to `path`, readable by you only.
pub fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
    f.write_all(text.as_bytes())
}

/// The text an editor saved, as the field gets it: the one line end
/// editors add at the end of a file goes.
pub fn saved_text(file: &str) -> String {
    let t = file.strip_suffix('\n').unwrap_or(file);
    t.strip_suffix('\r').unwrap_or(t).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |n| vars.iter().find(|(k, _)| *k == n).map(|(_, v)| v.to_string())
    }

    #[test]
    fn own_command_runs_as_is() {
        let f = Path::new("/run/f.txt");
        let c = command(f, env(&[("TRAILBOARD_EDITOR", "code --wait"), ("EDITOR", "nvim")]), |_| true).unwrap();
        assert_eq!(c, ["code", "--wait", "/run/f.txt"]);
        let c = command(f, env(&[("TRAILBOARD_EDITOR", "gvim -f {file} +1")]), |_| true).unwrap();
        assert_eq!(c, ["gvim", "-f", "/run/f.txt", "+1"]);
    }

    #[test]
    fn editor_runs_in_a_terminal() {
        let f = Path::new("/run/f.txt");
        let c = command(f, env(&[("EDITOR", "nvim --clean"), ("TERMINAL", "kitty")]), |p| p == "xdg-terminal-exec").unwrap();
        assert_eq!(c, ["xdg-terminal-exec", "nvim", "--clean", "/run/f.txt"]);
        let c = command(f, env(&[("VISUAL", "hx"), ("EDITOR", "nano"), ("TERMINAL", "kitty")]), |_| false).unwrap();
        assert_eq!(c, ["kitty", "-e", "hx", "/run/f.txt"]);
        let c = command(f, env(&[("EDITOR", " ")]), |p| p == "xterm").unwrap();
        assert_eq!(c, ["xterm", "-e", "vi", "/run/f.txt"]);
        assert_eq!(command(f, env(&[]), |_| false), None);
    }

    #[test]
    fn saved_text_drops_one_line_end() {
        assert_eq!(saved_text("hello\n"), "hello");
        assert_eq!(saved_text("a\nb\n\n"), "a\nb\n");
        assert_eq!(saved_text("x\r\n"), "x");
        assert_eq!(saved_text(""), "");
    }
}
