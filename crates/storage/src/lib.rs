//! On-disk state. The board is one JSON file written atomically (temp file in
//! the same directory, fsync, rename), so a crash never leaves a torn board.
//! Each space has its own board; `spaces` holds the index of spaces.

pub mod export;
pub mod icons;
pub mod spaces;
pub mod zoom;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use scene::{Camera, Scene};
use serde::{Deserialize, Serialize};

/// 1: cards, edges, camera. 2: adds folded trails (`scene.stacks`).
/// 3: adds note cards (`node.note`), so builds without notes refuse the file
/// rather than load notes as broken pages and save them without their text.
/// 4: adds trail names and colours (`node.trail`), for the same reason.
pub const BOARD_VERSION: u32 = 5;

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
            // Version 3 only adds `note` on cards, which defaults to none.
            2 => v["version"] = 3.into(),
            // Version 4 only adds `trail` on cards, which defaults to none.
            3 => v["version"] = 4.into(),
            // Version 5 only adds `sticky` on notes, which defaults to none.
            4 => v["version"] = 5.into(),
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
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
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
    fn notes_are_saved_and_version_2_boards_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = board_path(dir.path());
        let mut scene = Scene::new();
        let page = scene.add_page(Rect::new(0.0, 0.0, 10.0, 10.0), "https://a");
        let note = scene.add_note(Rect::new(50.0, 0.0, 10.0, 10.0), "# Plan\n\n- [ ] one");
        save_board(&path, &Board::new(scene, Camera::default())).unwrap();
        let back = load_board(&path).unwrap().scene;
        assert_eq!(back.node(note).unwrap().note.as_deref(), Some("# Plan\n\n- [ ] one"));
        assert_eq!(back.node(note).unwrap().title, "Plan");
        assert_eq!(back.node(page).unwrap().note, None);
        // Pages carry no `note` key at all.
        let raw: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let nodes = raw["scene"]["nodes"].as_object().unwrap();
        assert_eq!(nodes.values().filter(|n| n.get("note").is_some()).count(), 1);

        let mut v = raw.clone();
        v["version"] = 2.into();
        for n in v["scene"]["nodes"].as_object_mut().unwrap().values_mut() {
            n.as_object_mut().unwrap().remove("note");
        }
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let b = load_board(&path).unwrap();
        assert_eq!(b.version, BOARD_VERSION);
        assert!(b.scene.all_nodes().all(|n| n.note.is_none()));
    }

    #[test]
    fn trail_marks_are_saved_and_version_3_boards_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = board_path(dir.path());
        let mut scene = Scene::new();
        let a = scene.add_page(Rect::new(0.0, 0.0, 10.0, 10.0), "a");
        let b = scene.spawn_beside(a, "b").unwrap();
        scene.name_trail(a, "Rust");
        scene.cycle_trail_colour(a);
        save_board(&path, &Board::new(scene.clone(), Camera::default())).unwrap();
        let back = load_board(&path).unwrap().scene;
        assert_eq!(back.trail_mark(b), scene.trail_mark(b));
        assert_eq!(back.trail_mark(b).colour.as_deref(), Some("blue"));

        let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["version"] = 3.into();
        for n in v["scene"]["nodes"].as_object_mut().unwrap().values_mut() {
            n.as_object_mut().unwrap().remove("trail");
        }
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let b3 = load_board(&path).unwrap();
        assert_eq!(b3.version, BOARD_VERSION);
        assert!(b3.scene.trail_mark(b).is_empty());
    }

    #[test]
    fn sticky_notes_are_saved_and_version_4_boards_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = board_path(dir.path());
        let mut scene = Scene::new();
        let a = scene.add_sticky(Rect::new(0.0, 0.0, 10.0, 10.0), "milk");
        scene.cycle_sticky_colour(a);
        let b = scene.add_note(Rect::new(50.0, 0.0, 10.0, 10.0), "plain");
        save_board(&path, &Board::new(scene.clone(), Camera::default())).unwrap();
        let back = load_board(&path).unwrap().scene;
        assert_eq!(back.node(a).unwrap().sticky.as_deref(), Some("green"));
        assert!(!back.node(b).unwrap().is_sticky());
        assert!(!fs::read_to_string(&path).unwrap().contains("\"sticky\": null"));

        let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["version"] = 4.into();
        for n in v["scene"]["nodes"].as_object_mut().unwrap().values_mut() {
            n.as_object_mut().unwrap().remove("sticky");
        }
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let b4 = load_board(&path).unwrap();
        assert_eq!(b4.version, BOARD_VERSION);
        assert!(b4.scene.all_nodes().all(|n| !n.is_sticky()));
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
