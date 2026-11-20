//! Jump lists and marks (`scene::jump::Places`), for every space:
//! `$DATA/jumps.json` =
//!
//! ```json
//! { "version": 1,
//!   "spaces": [ { "space": 1, "jumps": [3, 4], "at": 1,
//!                 "marks": [ { "card": 3, "mark": "a", "url": "…", "path": "", "x": 0, "y": 900 } ] } ],
//!   "board": [ { "mark": "A", "space": 1, "card": 3, "camera": { "center": { "x": 0, "y": 0 }, "zoom": 1 } } ] }
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use scene::NodeId;
use scene::jump::{BoardSpot, Jumps, Places, ScrollSpot, SpacePlaces, is_board_mark, is_card_mark};
use serde::{Deserialize, Serialize};

use crate::write_atomic;

pub const JUMPS_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct JumpsFile {
    version: u32,
    #[serde(default)]
    spaces: Vec<SpaceRow>,
    #[serde(default)]
    board: Vec<BoardRow>,
}

#[derive(Serialize, Deserialize)]
struct SpaceRow {
    space: u64,
    #[serde(default)]
    jumps: Vec<NodeId>,
    #[serde(default)]
    at: usize,
    #[serde(default)]
    marks: Vec<MarkRow>,
}

#[derive(Serialize, Deserialize)]
struct MarkRow {
    card: NodeId,
    mark: char,
    #[serde(flatten)]
    spot: ScrollSpot,
}

#[derive(Serialize, Deserialize)]
struct BoardRow {
    mark: char,
    #[serde(flatten)]
    spot: BoardSpot,
}

pub fn jumps_path(data_dir: &Path) -> PathBuf {
    data_dir.join("jumps.json")
}

/// What was saved. A missing, unreadable or newer file gives nothing;
/// marks with a letter of the wrong kind are skipped.
pub fn load(data_dir: &Path) -> Places {
    let path = jumps_path(data_dir);
    let Ok(bytes) = fs::read(&path) else { return Places::default() };
    let file = match serde_json::from_slice::<JumpsFile>(&bytes) {
        Ok(f) if f.version <= JUMPS_VERSION => f,
        Ok(f) => {
            log::warn!("{}: version {} is newer than this build supports", path.display(), f.version);
            return Places::default();
        }
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            return Places::default();
        }
    };
    let mut places = Places::default();
    for row in file.spaces {
        let mut s = SpacePlaces { jumps: Jumps::from_parts(row.jumps, row.at), ..Default::default() };
        for m in row.marks.into_iter().filter(|m| is_card_mark(m.mark)) {
            s.marks.entry(m.card).or_default().insert(m.mark, m.spot);
        }
        places.spaces.insert(row.space, s);
    }
    for row in file.board.into_iter().filter(|b| is_board_mark(b.mark)) {
        places.board.insert(row.mark, row.spot);
    }
    places
}

pub fn save(data_dir: &Path, places: &Places) -> std::io::Result<()> {
    let spaces = places
        .spaces
        .iter()
        .map(|(&space, s)| SpaceRow {
            space,
            jumps: s.jumps.cards().to_vec(),
            at: s.jumps.at(),
            marks: s
                .marks
                .iter()
                .flat_map(|(&card, m)| m.iter().map(move |(&mark, spot)| MarkRow { card, mark, spot: spot.clone() }))
                .collect(),
        })
        .collect();
    let board = places.board.iter().map(|(&mark, &spot)| BoardRow { mark, spot }).collect();
    let file = JumpsFile { version: JUMPS_VERSION, spaces, board };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    write_atomic(&jumps_path(data_dir), &json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene::geom::{Camera, Vec2};

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), Places::default());
        let mut p = Places::default();
        for i in [3, 4, 5] {
            p.space(1).jumps.visit(NodeId(i));
        }
        p.space(1).jumps.back(1, Some(NodeId(5)), |_| true);
        p.set_card_mark(1, NodeId(3), 'a', ScrollSpot { url: "https://a.org/".into(), path: "1.2".into(), x: 0.0, y: 900.0 });
        p.board.insert('A', BoardSpot { space: 1, card: Some(NodeId(4)), camera: Camera { center: Vec2::new(10.0, -5.0), zoom: 0.5 } });
        p.board.insert('B', BoardSpot { space: 2, card: None, camera: Camera::default() });
        save(dir.path(), &p).unwrap();
        assert_eq!(load(dir.path()), p);

        fs::write(jumps_path(dir.path()), b"{ nope").unwrap();
        assert_eq!(load(dir.path()), Places::default());
        fs::write(jumps_path(dir.path()), br#"{"version":9}"#).unwrap();
        assert_eq!(load(dir.path()), Places::default());
        // A hand-edited file: a board mark in lower case and a card mark in
        // upper case are skipped.
        fs::write(
            jumps_path(dir.path()),
            br#"{"version":1,"spaces":[{"space":1,"marks":[{"card":3,"mark":"Q","url":"u","x":0,"y":0}]}],
                "board":[{"mark":"b","space":1,"camera":{"center":{"x":0,"y":0},"zoom":1}}]}"#,
        )
        .unwrap();
        let l = load(dir.path());
        assert!(l.board.is_empty());
        assert!(l.spaces[&1].marks.is_empty());
    }
}
