//! Per-site JavaScript and images (`scene::sitesettings`), shared by every
//! space: `$DATA/sites.json` =
//! `{ "version": 1, "sites": { "example.com": { "javascript": false, "images": true } } }`.
//! A field left out follows the default setting.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use scene::sitesettings::{SiteRule, SiteSettings};
use serde::{Deserialize, Serialize};

use crate::write_atomic;

pub const SITES_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Default)]
struct RuleFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    javascript: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    images: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct SitesFile {
    version: u32,
    #[serde(default)]
    sites: BTreeMap<String, RuleFile>,
}

pub fn sites_path(data_dir: &Path) -> PathBuf {
    data_dir.join("sites.json")
}

/// The saved exceptions. A missing, unreadable or newer file gives none (a
/// newer file is left alone until an exception changes).
pub fn load_sites(data_dir: &Path) -> SiteSettings {
    let path = sites_path(data_dir);
    let Ok(bytes) = fs::read(&path) else { return SiteSettings::default() };
    match serde_json::from_slice::<SitesFile>(&bytes) {
        Ok(f) if f.version <= SITES_VERSION => SiteSettings {
            sites: f
                .sites
                .into_iter()
                .map(|(site, r)| (site, SiteRule { javascript: r.javascript, images: r.images }))
                .filter(|(_, r)| r.javascript.is_some() || r.images.is_some())
                .collect(),
        },
        Ok(f) => {
            log::warn!("{}: version {} is newer than this build supports", path.display(), f.version);
            SiteSettings::default()
        }
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            SiteSettings::default()
        }
    }
}

pub fn save_sites(data_dir: &Path, sites: &SiteSettings) -> std::io::Result<()> {
    let file = SitesFile {
        version: SITES_VERSION,
        sites: sites.sites.iter().map(|(s, r)| (s.clone(), RuleFile { javascript: r.javascript, images: r.images })).collect(),
    };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    write_atomic(&sites_path(data_dir), &json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene::sitesettings::Content;

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_sites(dir.path()).sites.is_empty());
        let mut s = SiteSettings::default();
        s.set("https://a.com/", Content::JavaScript, false, true);
        s.set("https://b.com/", Content::Images, false, true);
        save_sites(dir.path(), &s).unwrap();
        assert_eq!(load_sites(dir.path()), s);
        let text = fs::read_to_string(sites_path(dir.path())).unwrap();
        assert!(!text.contains("null"), "unset fields are left out: {text}");

        fs::write(sites_path(dir.path()), b"{ nope").unwrap();
        assert!(load_sites(dir.path()).sites.is_empty());
        fs::write(sites_path(dir.path()), br#"{"version":9,"sites":{"a.com":{"javascript":false}}}"#).unwrap();
        assert!(load_sites(dir.path()).sites.is_empty());
        fs::write(sites_path(dir.path()), br#"{"version":1,"sites":{"a.com":{},"b.com":{"images":false}}}"#).unwrap();
        assert_eq!(load_sites(dir.path()).sites.len(), 1, "empty rules are dropped");
    }
}
