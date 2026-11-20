//! The `:` command line's recent lines (`scene::command::Recent`), shared by
//! every space: `$DATA/commands.json` =
//! `{ "version": 1, "recent": ["tidy", "open example.com"] }`, newest first.

use std::fs;
use std::path::{Path, PathBuf};

use scene::command::Recent;
use serde::{Deserialize, Serialize};

use crate::write_atomic;

pub const COMMANDS_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct CommandsFile {
    version: u32,
    #[serde(default)]
    recent: Vec<String>,
}

pub fn commands_path(data_dir: &Path) -> PathBuf {
    data_dir.join("commands.json")
}

/// The saved lines. A missing, unreadable or newer file gives none.
pub fn load_recent(data_dir: &Path) -> Recent {
    let path = commands_path(data_dir);
    let Ok(bytes) = fs::read(&path) else { return Recent::default() };
    match serde_json::from_slice::<CommandsFile>(&bytes) {
        Ok(f) if f.version <= COMMANDS_VERSION => Recent::from_lines(f.recent),
        Ok(f) => {
            log::warn!("{}: version {} is newer than this build supports", path.display(), f.version);
            Recent::default()
        }
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            Recent::default()
        }
    }
}

pub fn save_recent(data_dir: &Path, recent: &Recent) -> std::io::Result<()> {
    let file = CommandsFile { version: COMMANDS_VERSION, recent: recent.lines().to_vec() };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    write_atomic(&commands_path(data_dir), &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_recent(dir.path()).lines().is_empty());
        let mut r = Recent::default();
        r.push("tidy");
        r.push("open example.com");
        save_recent(dir.path(), &r).unwrap();
        assert_eq!(load_recent(dir.path()), r);

        fs::write(commands_path(dir.path()), b"{ nope").unwrap();
        assert!(load_recent(dir.path()).lines().is_empty());
        fs::write(commands_path(dir.path()), br#"{"version":9,"recent":["tidy"]}"#).unwrap();
        assert!(load_recent(dir.path()).lines().is_empty());
        // Repeats in a hand-edited file are kept once, newest first.
        fs::write(commands_path(dir.path()), br#"{"version":1,"recent":["fit","tidy","fit"]}"#).unwrap();
        assert_eq!(load_recent(dir.path()).lines(), &["fit".to_string(), "tidy".to_string()]);
    }
}
