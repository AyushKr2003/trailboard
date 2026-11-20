//! The user's key file, `$XDG_CONFIG_HOME/trailboard/keys` (M12 step 10):
//! plain text, one entry a line, our own format. `scene::userkeys` gives
//! the entries meaning; this reads them and says which lines are wrong.
//!
//! ```text
//! # A comment (a whole line).
//! bind page gs open https://web.archive.org/web/{url}
//! bind board,page <C-p> run mpv {url}
//! unbind page d
//! alias play run mpv {hint-url}
//! bind page ;v hint play
//! ```
//!
//! The file is the user's: we only write it when it is missing and asked
//! for (`:keys-file`), with a commented example.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use scene::keys;
use scene::userkeys::{self, Entry};

pub const FILE: &str = "keys";

/// `$XDG_CONFIG_HOME/trailboard/keys` (`~/.config/trailboard/keys`).
pub fn path() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/")).join(".config"));
    config.join("trailboard").join(FILE)
}

/// What the file holds: its entries and its bad lines, each with its
/// number (from 1).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Read {
    pub entries: Vec<(usize, Entry)>,
    pub errors: Vec<(usize, String)>,
}

/// Reads `text`: every good line is an entry, every bad one an error.
pub fn parse(text: &str) -> Read {
    let mut out = Read::default();
    for (i, line) in text.lines().enumerate() {
        match parse_line(line) {
            Ok(Some(e)) => out.entries.push((i + 1, e)),
            Ok(None) => {}
            Err(msg) => out.errors.push((i + 1, msg)),
        }
    }
    out
}

/// One line: `None` for a blank line or a comment.
pub fn parse_line(line: &str) -> Result<Option<Entry>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let (word, rest) = split(line);
    let entry = match word {
        "bind" => {
            let (modes, keys, target) = mode_keys(rest, "bind")?;
            if target.is_empty() {
                return Err(format!("bind {} needs a command after the keys", keys::display(&keys)));
            }
            Entry::Bind { modes, keys, target: target.to_string() }
        }
        "unbind" => {
            let (modes, keys, extra) = mode_keys(rest, "unbind")?;
            if !extra.is_empty() {
                return Err(format!("unbind takes modes and keys only, not “{extra}”"));
            }
            Entry::Unbind { modes, keys }
        }
        "alias" => {
            let (name, target) = split(rest);
            if name.is_empty() || target.is_empty() {
                return Err("alias needs a name and a command".into());
            }
            Entry::Alias { name: name.to_string(), target: target.to_string() }
        }
        other => return Err(format!("“{other}” isn't bind, unbind or alias")),
    };
    Ok(Some(entry))
}

fn split(s: &str) -> (&str, &str) {
    let s = s.trim();
    s.split_once(char::is_whitespace).map_or((s, ""), |(a, b)| (a, b.trim()))
}

fn mode_keys<'a>(rest: &'a str, word: &str) -> Result<(Vec<userkeys::Mode>, Vec<keys::Chord>, &'a str), String> {
    let (modes, rest) = split(rest);
    let (seq, rest) = split(rest);
    if modes.is_empty() || seq.is_empty() {
        return Err(format!("{word} needs modes and keys ({word} page gs …)"));
    }
    Ok((userkeys::modes(modes)?, keys::parse(seq)?, rest))
}

/// Reads the file at `path`; a missing file is no entries.
pub fn load(path: &Path) -> io::Result<Read> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(parse(&text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Read::default()),
        Err(e) => Err(e),
    }
}

/// What a new key file says: how it works, every line commented out.
pub const TEMPLATE: &str = "\
# Trailboard keys. One entry a line; lines starting with # are notes.
# :reload-keys reads this file again; a wrong line is reported with its
# number and the rest still load.
#
#   bind <modes> <keys> <command>   modes: board, page, insert, hints,
#                                   command (several: board,page)
#   unbind <modes> <keys>           unbinding a start (;) drops all under it
#   alias <name> <command>          then :name runs it (words after go on)
#
# Keys: letters stand for themselves; <C-d> Ctrl+D, <A-x> Alt+X,
# <S-Tab>, <Space>, <Enter>, <Esc>, <F5>, <lt> for <. In insert, hints
# and command a binding is one key with Ctrl or Alt, or an F key.
#
# A command is a key's name (scroll-down, top, hints-here, next-card…;
# the ? sheet lists what keys do) or a : command line. Chain several
# with |. {url} and {title} are the current page's, {hint-url} the link
# chosen by :hint. :run starts a program without a shell.
#
# bind page gs open https://web.archive.org/web/{url}
# bind page ,v run mpv {url}
# alias play run mpv {hint-url}
# bind page ;v hint play
# bind page ;V hint-many play
# bind board,page <C-p> card-prev
# bind insert <F9> run foot
# unbind page d
";

/// Makes the file with [`TEMPLATE`] if there is none; true if it made it.
pub fn ensure(path: &Path) -> io::Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    crate::write_atomic(path, TEMPLATE.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene::userkeys::Mode;

    #[test]
    fn lines_read_into_entries() {
        let r = parse(
            "# note\n\
             \n\
             bind board,page gs  open https://e.com/{url} | tidy\n\
             unbind page d\n\
             alias Play run mpv {hint-url}\n\
             bind page <C-p>\n\
             bind nowhere x close\n\
             unbind page d extra\n\
             frobnicate\n\
             bind page <Nope> close\n\
             alias lonely\n",
        );
        assert_eq!(
            r.entries,
            vec![
                (3, Entry::Bind { modes: vec![Mode::Board, Mode::Page], keys: keys::parse("gs").unwrap(), target: "open https://e.com/{url} | tidy".into() }),
                (4, Entry::Unbind { modes: vec![Mode::Page], keys: keys::parse("d").unwrap() }),
                (5, Entry::Alias { name: "Play".into(), target: "run mpv {hint-url}".into() }),
            ]
        );
        assert_eq!(r.errors.iter().map(|(n, _)| *n).collect::<Vec<_>>(), vec![6, 7, 8, 9, 10, 11]);
        assert!(r.errors[1].1.contains("nowhere"));
        assert!(r.errors[4].1.contains("Nope"));
    }

    #[test]
    fn the_template_is_all_comments_and_its_examples_work() {
        assert!(parse(TEMPLATE).entries.is_empty() && parse(TEMPLATE).errors.is_empty());
        let examples: String = TEMPLATE
            .lines()
            .filter_map(|l| l.strip_prefix("# "))
            .filter(|l| ["bind ", "unbind ", "alias "].iter().any(|w| l.starts_with(w)))
            .map(|l| format!("{l}\n"))
            .collect();
        let r = parse(&examples);
        assert_eq!(r.entries.len(), 8, "{examples}");
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let (_, errors) = scene::userkeys::Keys::build(&r.entries);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn ensure_writes_once_and_load_reads() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("trailboard").join(FILE);
        assert_eq!(load(&p).unwrap(), Read::default(), "missing is empty");
        assert!(ensure(&p).unwrap());
        fs::write(&p, "bind page x close\n").unwrap();
        assert!(!ensure(&p).unwrap(), "never overwrites");
        assert_eq!(load(&p).unwrap().entries.len(), 1);
    }
}
