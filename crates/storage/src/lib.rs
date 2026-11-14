//! On-disk state. The board is one JSON file written atomically (temp file in
//! the same directory, fsync, rename), so a crash never leaves a torn board.
//! Each space has its own board; `spaces` holds the index of spaces.

pub mod spaces;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use scene::{Camera, Scene};
use serde::{Deserialize, Serialize};

/// 1: cards, edges, camera. 2: adds folded trails (`scene.stacks`).
pub const BOARD_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub version: u32,
    pub scene: Scene,
    pub camera: Camera,
}

impl Board {
    pub fn new(scene: Scene, camera: Camera) -> Self {
        Self { version: BOARD_VERSION, scene, camera }
    }
}

#[derive(Debug)]
pub enum LoadError {
    Missing,
    Io(std::io::Error),
    /// The file exists but could not be parsed; it is left untouched.
    Corrupt(serde_json::Error),
    UnsupportedVersion(u32),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Missing => write!(f, "no saved board"),
            LoadError::Io(e) => write!(f, "{e}"),
            LoadError::Corrupt(e) => write!(f, "board file is corrupt: {e}"),
            LoadError::UnsupportedVersion(v) => write!(f, "board version {v} is newer than this build supports"),
        }
    }
}

/// The single board of versions before spaces; `spaces::open_spaces` migrates it.
pub fn board_path(data_dir: &Path) -> PathBuf {
    data_dir.join("board.json")
}

pub fn load_board(path: &Path) -> Result<Board, LoadError> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(LoadError::Missing),
        Err(e) => return Err(LoadError::Io(e)),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(LoadError::Corrupt)?;
    let value = migrate_board(value)?;
    serde_json::from_value(value).map_err(LoadError::Corrupt)
}

/// Upgrades a board file's JSON to [`BOARD_VERSION`] one step at a time.
/// Newer versions are refused (and the file is left alone).
pub fn migrate_board(mut v: serde_json::Value) -> Result<serde_json::Value, LoadError> {
    loop {
        let Some(version) = v.get("version").and_then(|n| n.as_u64()) else {
            return Err(LoadError::Corrupt(serde::de::Error::missing_field("version")));
        };
        match version as u32 {
            BOARD_VERSION => return Ok(v),
            // Version 2 only adds `scene.stacks`, which defaults to none.
            1 => v["version"] = 2.into(),
            other => return Err(LoadError::UnsupportedVersion(other)),
        }
    }
}

pub fn save_board(path: &Path, board: &Board) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(board).map_err(std::io::Error::other)?;
    write_atomic(path, &json)
}

/// Writes `bytes` to `path` via a synced temp file and a rename, creating the directory.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene::{Rect, Vec2};

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = board_path(dir.path());
        let mut scene = Scene::new();
        let a = scene.add_page(Rect::new(0.0, 0.0, 800.0, 600.0), "https://a.example");
        scene.spawn_beside(a, "https://b.example").unwrap();
        let cam = Camera { center: Vec2::new(12.0, 34.0), zoom: 0.5 };
        save_board(&path, &Board::new(scene, cam)).unwrap();

        let back = load_board(&path).unwrap();
        assert_eq!(back.scene.nodes().count(), 2);
        assert_eq!(back.scene.edges().len(), 1);
        assert_eq!(back.camera, cam);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn missing_and_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = board_path(dir.path());
        assert!(matches!(load_board(&path), Err(LoadError::Missing)));
        fs::write(&path, b"{not json").unwrap();
        assert!(matches!(load_board(&path), Err(LoadError::Corrupt(_))));
        // A corrupt file is not overwritten by loading.
        assert_eq!(fs::read(&path).unwrap(), b"{not json");
    }

    #[test]
    fn ids_keep_counting_after_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = board_path(dir.path());
        let mut scene = Scene::new();
        let a = scene.add_page(Rect::new(0.0, 0.0, 10.0, 10.0), "a");
        save_board(&path, &Board::new(scene, Camera::default())).unwrap();
        let mut back = load_board(&path).unwrap().scene;
        let b = back.add_page(Rect::new(50.0, 0.0, 10.0, 10.0), "b");
        assert_ne!(a, b);
    }

    #[test]
    fn version_1_boards_load_and_newer_ones_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = board_path(dir.path());
        let mut scene = Scene::new();
        let a = scene.add_page(Rect::new(0.0, 0.0, 10.0, 10.0), "a");
        scene.spawn_beside(a, "b").unwrap();
        let mut v = serde_json::to_value(Board::new(scene, Camera::default())).unwrap();
        v["version"] = 1.into();
        assert!(v["scene"].get("stacks").is_none());
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let back = load_board(&path).unwrap();
        assert_eq!(back.version, BOARD_VERSION);
        assert_eq!(back.scene.nodes().count(), 2);
        assert!(back.scene.stacks().is_empty());

        v["version"] = 99.into();
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        assert!(matches!(load_board(&path), Err(LoadError::UnsupportedVersion(99))));
    }

    #[test]
    fn stacks_are_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = board_path(dir.path());
        let mut scene = Scene::new();
        let a = scene.add_page(Rect::new(0.0, 0.0, 10.0, 10.0), "a");
        let b = scene.spawn_beside(a, "b").unwrap();
        scene.fold(b).unwrap();
        save_board(&path, &Board::new(scene.clone(), Camera::default())).unwrap();
        let back = load_board(&path).unwrap().scene;
        assert_eq!(back.stacks(), scene.stacks());
        assert!(back.is_hidden(a));
    }
}
