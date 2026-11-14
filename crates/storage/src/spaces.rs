//! Spaces: several boards, each in its own file, listed by an index.
//!
//! ```text
//! $DATA/spaces.json          index: version, active space, spaces in order
//! $DATA/spaces/<id>.json     one `Board` per space
//! $DATA/spaces/deleted/      boards of deleted spaces, set aside (never read back)
//! ```
//!
//! The pre-spaces `board.json` becomes space 1 the first time the index is
//! opened. A missing or unreadable index is rebuilt from the board files, so
//! no board is ever orphaned. Older index versions are upgraded step by step
//! (`migrate`) when read.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::write_atomic;

/// 1: `background` was a fixed sRGB colour. 2: `tint` names a theme colour.
/// 3: `private` (a build that doesn't know it must not open the space and
/// record its pages, so older builds refuse a v3 index).
pub const SPACES_VERSION: u32 = 3;

/// Board tints new spaces take in turn: none (the theme's background) or a
/// colour of the desktop theme, mixed lightly into its background, so every
/// tint suits light and dark themes alike.
pub const TINTS: [Option<&str>; 7] = [None, Some("blue"), Some("green"), Some("magenta"), Some("orange"), Some("cyan"), Some("red")];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SpaceId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpaceEntry {
    pub id: SpaceId,
    pub name: String,
    /// Theme colour the board is tinted with (`None`: the theme's background).
    #[serde(default)]
    pub tint: Option<String>,
    /// Pages here are not recorded (history, site icons, page zoom) and use a
    /// cookie jar and cache in memory only, dropped when the app quits.
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpaceIndex {
    pub version: u32,
    pub active: SpaceId,
    /// In Next/Previous order.
    pub spaces: Vec<SpaceEntry>,
    next_id: u64,
}

impl Default for SpaceIndex {
    fn default() -> Self {
        Self::with_ids(&[1])
    }
}

impl SpaceIndex {
    /// An index listing spaces with these ids (in order), named "Space 1", "Space 2", ….
    fn with_ids(ids: &[u64]) -> Self {
        let mut idx = SpaceIndex { version: SPACES_VERSION, active: SpaceId(ids[0]), spaces: Vec::new(), next_id: 0 };
        for &id in ids {
            idx.next_id = idx.next_id.max(id);
            let name = idx.default_name();
            let tint = tint_name(TINTS[idx.spaces.len() % TINTS.len()]);
            idx.spaces.push(SpaceEntry { id: SpaceId(id), name, tint, private: false });
        }
        idx
    }

    pub fn get(&self, id: SpaceId) -> Option<&SpaceEntry> {
        self.spaces.iter().find(|s| s.id == id)
    }

    pub fn active_entry(&self) -> &SpaceEntry {
        self.get(self.active).expect("active space is listed")
    }

    /// Position of `id` in the list (0-based).
    pub fn position(&self, id: SpaceId) -> Option<usize> {
        self.spaces.iter().position(|s| s.id == id)
    }

    /// "Space N" with the smallest N not already taken.
    pub fn default_name(&self) -> String {
        (1..)
            .map(|n| format!("Space {n}"))
            .find(|name| !self.spaces.iter().any(|s| &s.name == name))
            .expect("some name is free")
    }

    /// Adds a space right after the active one and returns its id (it does not become active).
    pub fn add(&mut self) -> SpaceId {
        self.next_id += 1;
        let id = SpaceId(self.next_id);
        let name = self.default_name();
        let tint = self.unused_tint();
        let at = self.position(self.active).map_or(self.spaces.len(), |p| p + 1);
        self.spaces.insert(at, SpaceEntry { id, name, tint, private: false });
        id
    }

    /// The first tint no space uses, or the next one in turn.
    fn unused_tint(&self) -> Option<String> {
        let unused = TINTS.iter().find(|t| !self.spaces.iter().any(|s| s.tint.as_deref() == **t));
        tint_name(*unused.unwrap_or(&TINTS[self.spaces.len() % TINTS.len()]))
    }

    /// The space `step` places after `from`, wrapping around.
    pub fn neighbour(&self, from: SpaceId, step: isize) -> SpaceId {
        let n = self.spaces.len() as isize;
        let i = self.position(from).unwrap_or(0) as isize;
        self.spaces[(i + step).rem_euclid(n) as usize].id
    }

    /// Renames a space. Whitespace is collapsed; an empty name is refused (returns false).
    pub fn rename(&mut self, id: SpaceId, name: &str) -> bool {
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        match self.spaces.iter_mut().find(|s| s.id == id) {
            Some(s) if !name.is_empty() => {
                s.name = name;
                true
            }
            _ => false,
        }
    }

    /// Steps a space's tint to the next one and returns it.
    pub fn cycle_tint(&mut self, id: SpaceId) -> Option<Option<String>> {
        let s = self.spaces.iter_mut().find(|s| s.id == id)?;
        let i = TINTS.iter().position(|t| *t == s.tint.as_deref()).map_or(0, |i| i + 1);
        s.tint = tint_name(TINTS[i % TINTS.len()]);
        Some(s.tint.clone())
    }

    /// Makes a space private or not. Returns false if it is not listed.
    pub fn set_private(&mut self, id: SpaceId, private: bool) -> bool {
        match self.spaces.iter_mut().find(|s| s.id == id) {
            Some(s) => {
                s.private = private;
                true
            }
            None => false,
        }
    }

    /// Removes a space. Returns the space to show instead (the one before it,
    /// or the new first), or `None` if it is the only space or not listed.
    /// If the removed space was active, the returned one becomes active.
    pub fn remove(&mut self, id: SpaceId) -> Option<SpaceId> {
        if self.spaces.len() < 2 {
            return None;
        }
        let i = self.position(id)?;
        self.spaces.remove(i);
        let next = self.spaces[i.saturating_sub(1)].id;
        if self.active == id {
            self.active = next;
        }
        Some(next)
    }

    /// Repairs an index read from disk: drops duplicate ids, keeps `next_id`
    /// ahead of every id, and points `active` at a listed space.
    fn repair(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.spaces.retain(|s| seen.insert(s.id));
        if self.spaces.is_empty() {
            *self = SpaceIndex::default();
        }
        self.next_id = self.spaces.iter().map(|s| s.id.0).fold(self.next_id, u64::max);
        if self.get(self.active).is_none() {
            self.active = self.spaces[0].id;
        }
    }
}

fn tint_name(t: Option<&str>) -> Option<String> {
    t.map(str::to_string)
}

/// Upgrades an index read from disk to `SPACES_VERSION`, one version at a time.
/// Returns `Err` for a version this build doesn't know (newer, or not a number).
pub fn migrate(mut v: serde_json::Value) -> Result<serde_json::Value, String> {
    loop {
        let version = v.get("version").and_then(|n| n.as_u64()).ok_or("no version")? as u32;
        if version == SPACES_VERSION {
            return Ok(v);
        }
        match version {
            1 => {
                // Fixed sRGB backgrounds became theme tints. Each of the six old
                // colours maps to the tint of the same hue; anything else to none.
                const OLD: [([u8; 3], Option<&str>); 6] = [
                    ([22, 22, 26], None),
                    ([17, 23, 34], Some("blue")),
                    ([17, 29, 25], Some("green")),
                    ([31, 21, 30], Some("magenta")),
                    ([33, 25, 18], Some("orange")),
                    ([14, 27, 31], Some("cyan")),
                ];
                for space in v.get_mut("spaces").and_then(|s| s.as_array_mut()).into_iter().flatten() {
                    let Some(obj) = space.as_object_mut() else { continue };
                    let old: Option<[u8; 3]> = obj.remove("background").and_then(|b| serde_json::from_value(b).ok());
                    let tint = old.and_then(|c| OLD.iter().find(|(o, _)| *o == c)).and_then(|(_, t)| *t);
                    obj.insert("tint".into(), tint.into());
                }
                v["version"] = 2.into();
            }
            2 => {
                // Spaces became private or not; every existing space is not.
                for space in v.get_mut("spaces").and_then(|s| s.as_array_mut()).into_iter().flatten() {
                    if let Some(obj) = space.as_object_mut() {
                        obj.insert("private".into(), false.into());
                    }
                }
                v["version"] = 3.into();
            }
            other => return Err(format!("spaces.json version {other} is newer than this build supports")),
        }
    }
}

pub fn index_path(data_dir: &Path) -> PathBuf {
    data_dir.join("spaces.json")
}

pub fn space_board_path(data_dir: &Path, id: SpaceId) -> PathBuf {
    data_dir.join("spaces").join(format!("{}.json", id.0))
}

pub fn save_index(data_dir: &Path, idx: &SpaceIndex) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(idx).map_err(std::io::Error::other)?;
    write_atomic(&index_path(data_dir), &json)
}

/// Ids of the board files in `spaces/`, ascending.
fn board_ids(data_dir: &Path) -> Vec<u64> {
    let mut ids: Vec<u64> = fs::read_dir(data_dir.join("spaces"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.strip_suffix(".json")?.parse().ok()
        })
        .collect();
    ids.sort_unstable();
    ids
}

/// Reads the space index, creating it on first use. Moves the old single
/// `board.json` into space 1, and rebuilds a missing or unreadable index from
/// the board files on disk (an unreadable index is set aside, not overwritten).
pub fn open_spaces(data_dir: &Path) -> std::io::Result<SpaceIndex> {
    let path = index_path(data_dir);
    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) if value.get("version").and_then(|n| n.as_u64()).is_some() => {
                let old = value.get("version").and_then(|n| n.as_u64());
                // Written by a newer build: don't touch it, and don't write over it.
                let value = migrate(value).map_err(std::io::Error::other)?;
                match serde_json::from_value::<SpaceIndex>(value) {
                    Ok(mut idx) => {
                        idx.repair();
                        if old != Some(SPACES_VERSION as u64) {
                            log::info!("upgraded spaces.json from version {} to {SPACES_VERSION}", old.unwrap_or(0));
                            save_index(data_dir, &idx)?;
                        }
                        return Ok(idx);
                    }
                    Err(e) => {
                        let aside = path.with_extension(format!("unreadable-{}.json", std::process::id()));
                        log::error!("spaces index is corrupt ({e}); moving it to {} and rebuilding", aside.display());
                        fs::rename(&path, &aside)?;
                    }
                }
            }
            other => {
                let why = other.err().map_or("no version".to_string(), |e| e.to_string());
                let aside = path.with_extension(format!("unreadable-{}.json", std::process::id()));
                log::error!("spaces index is corrupt ({why}); moving it to {} and rebuilding", aside.display());
                fs::rename(&path, &aside)?;
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    // Migrate the single board of earlier versions into space 1.
    let old = crate::board_path(data_dir);
    let first = space_board_path(data_dir, SpaceId(1));
    if old.exists() && !first.exists() {
        fs::create_dir_all(first.parent().expect("has parent"))?;
        fs::rename(&old, &first)?;
        log::info!("moved {} to {}", old.display(), first.display());
    }
    let ids = board_ids(data_dir);
    let idx = if ids.is_empty() { SpaceIndex::default() } else { SpaceIndex::with_ids(&ids) };
    save_index(data_dir, &idx)?;
    Ok(idx)
}

/// Sets a deleted space's board aside under `spaces/deleted/`, so it can be
/// recovered by hand. A space that never saved a board has nothing to move.
pub fn discard_space(data_dir: &Path, id: SpaceId) -> std::io::Result<()> {
    let from = space_board_path(data_dir, id);
    if !from.exists() {
        return Ok(());
    }
    let dir = data_dir.join("spaces").join("deleted");
    fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    fs::rename(&from, dir.join(format!("{}-{stamp}.json", id.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{load_board, save_board, Board};
    use scene::{Camera, Rect, Scene};

    fn board_with(urls: &[&str]) -> Board {
        let mut s = Scene::new();
        for u in urls {
            s.add_page(Rect::new(0.0, 0.0, 10.0, 10.0), *u);
        }
        Board::new(s, Camera::default())
    }

    #[test]
    fn add_inserts_after_active_with_fresh_name_and_colour() {
        let mut idx = SpaceIndex::default();
        let b = idx.add();
        let c = idx.add(); // after the active space (1), so before b
        let names: Vec<_> = idx.spaces.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Space 1", "Space 3", "Space 2"]);
        assert_eq!(idx.spaces[1].id, c);
        assert_eq!(idx.spaces[2].id, b);
        assert_eq!(idx.active, SpaceId(1), "adding doesn't switch");
        let mut tints: Vec<_> = idx.spaces.iter().map(|s| s.tint.clone()).collect();
        tints.sort();
        tints.dedup();
        assert_eq!(tints.len(), 3);
        assert_eq!(idx.spaces[0].tint, None, "the first space uses the theme's own background");
    }

    #[test]
    fn default_name_reuses_gaps() {
        let mut idx = SpaceIndex::default();
        let b = idx.add();
        idx.add();
        assert!(idx.rename(b, "Research"));
        assert_eq!(idx.default_name(), "Space 2");
    }

    #[test]
    fn neighbour_wraps_both_ways() {
        let mut idx = SpaceIndex::default();
        idx.active = idx.add();
        idx.active = idx.add();
        let ids: Vec<_> = idx.spaces.iter().map(|s| s.id).collect();
        assert_eq!(idx.neighbour(ids[2], 1), ids[0]);
        assert_eq!(idx.neighbour(ids[0], -1), ids[2]);
        assert_eq!(idx.neighbour(ids[1], 1), ids[2]);
        let solo = SpaceIndex::default();
        assert_eq!(solo.neighbour(SpaceId(1), 1), SpaceId(1));
    }

    #[test]
    fn rename_trims_and_refuses_empty() {
        let mut idx = SpaceIndex::default();
        assert!(idx.rename(SpaceId(1), "  Trip   planning \n"));
        assert_eq!(idx.active_entry().name, "Trip planning");
        assert!(!idx.rename(SpaceId(1), "   "));
        assert!(!idx.rename(SpaceId(9), "x"));
        assert_eq!(idx.active_entry().name, "Trip planning");
    }

    #[test]
    fn remove_picks_previous_and_keeps_last_space() {
        let mut idx = SpaceIndex::default();
        assert_eq!(idx.remove(SpaceId(1)), None, "the only space stays");
        let b = idx.add();
        idx.active = b;
        assert_eq!(idx.remove(b), Some(SpaceId(1)));
        assert_eq!(idx.active, SpaceId(1));
        let c = idx.add();
        // Removing the first space shows the new first one.
        assert_eq!(idx.remove(SpaceId(1)), Some(c));
        assert_eq!(idx.active, c);
        assert_eq!(idx.remove(SpaceId(42)), None);
    }

    #[test]
    fn cycle_tint_walks_the_tints_and_back_to_none() {
        let mut idx = SpaceIndex::default();
        assert_eq!(idx.active_entry().tint, None);
        assert_eq!(idx.cycle_tint(SpaceId(1)), Some(Some("blue".into())));
        for _ in 1..TINTS.len() {
            idx.cycle_tint(SpaceId(1));
        }
        assert_eq!(idx.active_entry().tint, None);
        assert_eq!(idx.cycle_tint(SpaceId(9)), None);
    }

    #[test]
    fn version_1_backgrounds_become_tints() {
        let dir = tempfile::tempdir().unwrap();
        let v1 = r#"{"version":1,"active":2,"spaces":[
            {"id":1,"name":"Space 1","background":[22,22,26]},
            {"id":2,"name":"Research","background":[17,29,25]},
            {"id":3,"name":"Odd","background":[1,2,3]}],"next_id":3}"#;
        fs::write(index_path(dir.path()), v1).unwrap();
        let idx = open_spaces(dir.path()).unwrap();
        let tints: Vec<_> = idx.spaces.iter().map(|s| s.tint.as_deref()).collect();
        assert_eq!(tints, [None, Some("green"), None]);
        assert_eq!(idx.active, SpaceId(2));
        assert_eq!(idx.spaces[1].name, "Research");
        // The upgrade is written back, so the file is now the current version.
        let back: serde_json::Value = serde_json::from_slice(&fs::read(index_path(dir.path())).unwrap()).unwrap();
        assert_eq!(back["version"], SPACES_VERSION);
        assert!(back["spaces"][0].get("background").is_none());
        assert!(idx.spaces.iter().all(|s| !s.private));
    }

    #[test]
    fn version_2_spaces_are_not_private_and_private_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let v2 = r#"{"version":2,"active":1,"spaces":[
            {"id":1,"name":"Space 1","tint":null},
            {"id":2,"name":"Work","tint":"blue"}],"next_id":2}"#;
        fs::write(index_path(dir.path()), v2).unwrap();
        let mut idx = open_spaces(dir.path()).unwrap();
        assert!(idx.spaces.iter().all(|s| !s.private));
        let back: serde_json::Value = serde_json::from_slice(&fs::read(index_path(dir.path())).unwrap()).unwrap();
        assert_eq!(back["version"], 3);
        assert_eq!(back["spaces"][1]["private"], false);

        assert!(idx.set_private(SpaceId(2), true));
        assert!(!idx.set_private(SpaceId(9), true));
        save_index(dir.path(), &idx).unwrap();
        let again = open_spaces(dir.path()).unwrap();
        assert!(!again.spaces[0].private);
        assert!(again.spaces[1].private);
        // New spaces start recorded, even next to a private one.
        idx.active = SpaceId(2);
        let id = idx.add();
        assert!(!idx.get(id).unwrap().private);
    }

    #[test]
    fn migrate_refuses_unknown_versions() {
        assert!(migrate(serde_json::json!({"version": 99})).is_err());
        assert!(migrate(serde_json::json!({"spaces": []})).is_err());
    }

    #[test]
    fn first_open_migrates_board_json_into_space_one() {
        let dir = tempfile::tempdir().unwrap();
        save_board(&crate::board_path(dir.path()), &board_with(&["a", "b"])).unwrap();
        let idx = open_spaces(dir.path()).unwrap();
        assert_eq!(idx.spaces.len(), 1);
        assert_eq!(idx.active, SpaceId(1));
        assert!(!crate::board_path(dir.path()).exists());
        let b = load_board(&space_board_path(dir.path(), SpaceId(1))).unwrap();
        assert_eq!(b.scene.nodes().count(), 2);
        assert!(index_path(dir.path()).exists());
        // Opening again reads the index as written.
        assert_eq!(open_spaces(dir.path()).unwrap(), idx);
    }

    #[test]
    fn fresh_data_dir_gets_one_empty_space() {
        let dir = tempfile::tempdir().unwrap();
        let idx = open_spaces(dir.path()).unwrap();
        assert_eq!(idx, SpaceIndex::default());
    }

    #[test]
    fn index_round_trip_keeps_order_names_and_ids() {
        let dir = tempfile::tempdir().unwrap();
        let mut idx = open_spaces(dir.path()).unwrap();
        let b = idx.add();
        idx.rename(b, "Reading");
        idx.active = b;
        save_index(dir.path(), &idx).unwrap();
        let back = open_spaces(dir.path()).unwrap();
        assert_eq!(back, idx);
        // Ids keep counting after a reload, even past deleted spaces.
        let mut back = back;
        back.remove(b);
        assert_eq!(back.add(), SpaceId(3));
    }

    #[test]
    fn corrupt_index_is_set_aside_and_rebuilt_from_boards() {
        let dir = tempfile::tempdir().unwrap();
        save_board(&space_board_path(dir.path(), SpaceId(2)), &board_with(&["x"])).unwrap();
        save_board(&space_board_path(dir.path(), SpaceId(5)), &board_with(&["y"])).unwrap();
        fs::write(index_path(dir.path()), b"{oops").unwrap();
        let idx = open_spaces(dir.path()).unwrap();
        let ids: Vec<_> = idx.spaces.iter().map(|s| s.id.0).collect();
        assert_eq!(ids, [2, 5]);
        assert_eq!(idx.active, SpaceId(2));
        let aside = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("spaces.unreadable-"));
        assert!(aside);
    }

    #[test]
    fn newer_index_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let idx = SpaceIndex { version: SPACES_VERSION + 1, ..Default::default() };
        save_index(dir.path(), &idx).unwrap();
        let before = fs::read(index_path(dir.path())).unwrap();
        assert!(open_spaces(dir.path()).is_err());
        assert_eq!(fs::read(index_path(dir.path())).unwrap(), before);
    }

    #[test]
    fn repair_fixes_dangling_active_and_duplicates() {
        let mut idx = SpaceIndex::default();
        idx.add();
        idx.spaces.push(idx.spaces[0].clone());
        idx.active = SpaceId(77);
        idx.next_id = 0;
        idx.repair();
        assert_eq!(idx.spaces.len(), 2);
        assert_eq!(idx.active, SpaceId(1));
        assert_eq!(idx.add(), SpaceId(3));
    }

    #[test]
    fn discard_moves_board_aside() {
        let dir = tempfile::tempdir().unwrap();
        let p = space_board_path(dir.path(), SpaceId(3));
        save_board(&p, &board_with(&["z"])).unwrap();
        discard_space(dir.path(), SpaceId(3)).unwrap();
        assert!(!p.exists());
        assert_eq!(fs::read_dir(dir.path().join("spaces/deleted")).unwrap().count(), 1);
        // Deleted boards don't come back when the index is rebuilt.
        assert!(board_ids(dir.path()).is_empty());
        discard_space(dir.path(), SpaceId(9)).unwrap();
    }
}
