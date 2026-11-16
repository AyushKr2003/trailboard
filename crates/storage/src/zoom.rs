//! Page zoom per site (`scene::pagezoom`), shared by every space:
//! `$DATA/zoom.json` = `{ "version": 1, "sites": { "example.com": 1.25 } }`.

use std::fs;
use std::path::{Path, PathBuf};

use scene::pagezoom::SiteZooms;
use serde::{Deserialize, Serialize};

use crate::write_atomic;

pub const ZOOM_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ZoomFile {
    version: u32,
    #[serde(default)]
    sites: std::collections::BTreeMap<String, f64>,
}

pub fn zoom_path(data_dir: &Path) -> PathBuf {
    data_dir.join("zoom.json")
}

/// The saved zooms. A missing, unreadable or newer file gives none (a newer
/// file is left alone: it is only overwritten once a zoom changes).
pub fn load_zooms(data_dir: &Path) -> SiteZooms {
    let path = zoom_path(data_dir);
    let Ok(bytes) = fs::read(&path) else { return SiteZooms::default() };
    match serde_json::from_slice::<ZoomFile>(&bytes) {
        Ok(f) if f.version <= ZOOM_VERSION => {
            let sites = f.sites.into_iter().filter(|(_, z)| z.is_finite() && *z > 0.0).collect();
            SiteZooms { sites, ..SiteZooms::default() }
        }
        Ok(f) => {
            log::warn!("{}: version {} is newer than this build supports", path.display(), f.version);
            SiteZooms::default()
        }
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            SiteZooms::default()
        }
    }
}

pub fn save_zooms(data_dir: &Path, zooms: &SiteZooms) -> std::io::Result<()> {
    let file = ZoomFile { version: ZOOM_VERSION, sites: zooms.sites.clone() };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    write_atomic(&zoom_path(data_dir), &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_zooms(dir.path()).sites.is_empty());
        let mut z = SiteZooms::default();
        z.set("https://a.com/", 1.5);
        save_zooms(dir.path(), &z).unwrap();
        assert_eq!(load_zooms(dir.path()), z);

        fs::write(zoom_path(dir.path()), b"{ nope").unwrap();
        assert!(load_zooms(dir.path()).sites.is_empty());
        fs::write(zoom_path(dir.path()), br#"{"version":9,"sites":{"a.com":2.0}}"#).unwrap();
        assert!(load_zooms(dir.path()).sites.is_empty());
        fs::write(zoom_path(dir.path()), br#"{"version":1,"sites":{"a.com":-1.0,"b.com":2.0}}"#).unwrap();
        assert_eq!(load_zooms(dir.path()).sites.len(), 1);
    }
}
