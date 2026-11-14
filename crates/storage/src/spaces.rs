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
//! no board is ever orphaned.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::write_atomic;

pub const SPACES_VERSION: u32 = 1;

/// Background colours new spaces take in turn (dark, so pages stand out).
pub const BACKGROUNDS: [[u8; 3]; 6] = [
    [22, 22, 26], // graphite
    [17, 23, 34], // night blue
    [17, 29, 25], // pine
    [31, 21, 30], // plum
    [33, 25, 18], // umber
    [14, 27, 31], // teal
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SpaceId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpaceEntry {
    pub id: SpaceId,
    pub name: String,
    /// Board colour, sRGB.
    pub background: [u8; 3],
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
            let background = BACKGROUNDS[idx.spaces.len() % BACKGROUNDS.len()];
            idx.spaces.push(SpaceEntry { id: SpaceId(id), name, background });
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
        let background = self.unused_background();
        let at = self.position(self.active).map_or(self.spaces.len(), |p| p + 1);
        self.spaces.insert(at, SpaceEntry { id, name, background });
        id
    }

    /// The first palette colour no space uses, or the next one in turn.
    fn unused_background(&self) -> [u8; 3] {
        BACKGROUNDS
            .iter()
            .find(|c| !self.spaces.iter().any(|s| &s.background == *c))
            .copied()
            .unwrap_or(BACKGROUNDS[self.spaces.len() % BACKGROUNDS.len()])
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

    /// Steps a space's background to the next palette colour and returns it.
    pub fn cycle_background(&mut self, id: SpaceId) -> Option<[u8; 3]> {
        let s = self.spaces.iter_mut().find(|s| s.id == id)?;
        let i = BACKGROUNDS.iter().position(|c| *c == s.background).map_or(0, |i| i + 1);
        s.background = BACKGROUNDS[i % BACKGROUNDS.len()];
        Some(s.background)
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
        Ok(bytes) => match serde_json::from_slice::<SpaceIndex>(&bytes) {
            Ok(mut idx) if idx.version <= SPACES_VERSION => {
                idx.repair();
                return Ok(idx);
            }
            Ok(idx) => {
                // Written by a newer build: don't touch it, and don't write over it.
                return Err(std::io::Error::other(format!(
                    "spaces.json version {} is newer than this build supports",
                    idx.version
                )));
            }
            Err(e) => {
                let aside = path.with_extension(format!("unreadable-{}.json", std::process::id()));
                log::error!("spaces index is corrupt ({e}); moving it to {} and rebuilding", aside.display());
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
        let mut colours: Vec<_> = idx.spaces.iter().map(|s| s.background).collect();
        colours.dedup();
        assert_eq!(colours.len(), 3);
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
    fn cycle_background_walks_the_palette() {
        let mut idx = SpaceIndex::default();
        assert_eq!(idx.active_entry().background, BACKGROUNDS[0]);
        assert_eq!(idx.cycle_background(SpaceId(1)), Some(BACKGROUNDS[1]));
        for _ in 1..BACKGROUNDS.len() {
            idx.cycle_background(SpaceId(1));
        }
        assert_eq!(idx.active_entry().background, BACKGROUNDS[0]);
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
