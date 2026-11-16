//! Site icons (favicons), shared by every space, one file per site under
//! `$DATA/icons/`, so cards that are asleep or restored after a restart still
//! show their site's icon. A site is `scene::pagezoom::site_of`.
//!
//! File format (little-endian): `b"TBIC"`, version `u8`, width `u16`, height
//! `u16`, the site's length `u16` and the site (UTF-8), then `width * height`
//! BGRA8 pixels with straight alpha, rows top to bottom. The site is kept in
//! the file because file names have to make some sites safe (`host:port`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::write_atomic;

pub const ICON_VERSION: u8 = 1;
const MAGIC: &[u8; 4] = b"TBIC";
const HEADER: usize = 4 + 1 + 2 + 2 + 2;
/// Largest icon kept, in px per side (title bars draw them at 16 px).
pub const MAX_SIDE: u32 = 64;

/// One site's icon: BGRA8 pixels, straight alpha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Icon {
    /// `None` unless the size is 1..=[`MAX_SIDE`] and `pixels` holds exactly that many.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Icon> {
        let ok = (1..=MAX_SIDE).contains(&width)
            && (1..=MAX_SIDE).contains(&height)
            && pixels.len() == (width * height * 4) as usize;
        ok.then_some(Icon { width, height, pixels })
    }

    /// The file holding this icon as `site`'s.
    pub fn encode(&self, site: &str) -> Vec<u8> {
        let site = &site.as_bytes()[..site.len().min(u16::MAX as usize)];
        let mut out = Vec::with_capacity(HEADER + site.len() + self.pixels.len());
        out.extend_from_slice(MAGIC);
        out.push(ICON_VERSION);
        out.extend_from_slice(&(self.width as u16).to_le_bytes());
        out.extend_from_slice(&(self.height as u16).to_le_bytes());
        out.extend_from_slice(&(site.len() as u16).to_le_bytes());
        out.extend_from_slice(site);
        out.extend_from_slice(&self.pixels);
        out
    }

    /// The site and icon in an icon file; `None` for a file that isn't one,
    /// is truncated, or is a newer version.
    pub fn decode(bytes: &[u8]) -> Option<(String, Icon)> {
        if bytes.len() < HEADER || &bytes[..4] != MAGIC || bytes[4] > ICON_VERSION || bytes[4] == 0 {
            return None;
        }
        let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let (width, height, len) = (u16_at(5) as u32, u16_at(7) as u32, u16_at(9) as usize);
        let site = std::str::from_utf8(bytes.get(HEADER..HEADER + len)?).ok().filter(|s| !s.is_empty())?;
        let icon = Icon::new(width, height, bytes[HEADER + len..].to_vec())?;
        Some((site.to_string(), icon))
    }

    /// The icon as a PNG (RGBA8, stored without compression: at most
    /// 64×64, so a few KB), for pages that show it as an image.
    pub fn png(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity((self.height * (self.width * 4 + 1)) as usize);
        for row in self.pixels.chunks_exact(self.width as usize * 4) {
            raw.push(0); // no filter
            for [b, g, r, a] in row.as_chunks::<4>().0 {
                raw.extend_from_slice(&[*r, *g, *b, *a]);
            }
        }
        // A zlib stream of stored deflate blocks (each at most 65535 bytes).
        let mut z = vec![0x78, 0x01];
        let blocks: Vec<&[u8]> = raw.chunks(0xffff).collect();
        for (i, b) in blocks.iter().enumerate() {
            z.push(u8::from(i + 1 == blocks.len()));
            let n = b.len() as u16;
            z.extend_from_slice(&n.to_le_bytes());
            z.extend_from_slice(&(!n).to_le_bytes());
            z.extend_from_slice(b);
        }
        z.extend_from_slice(&adler32(&raw).to_be_bytes());

        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&self.width.to_be_bytes());
        ihdr.extend_from_slice(&self.height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
        for (kind, data) in [(b"IHDR", &ihdr[..]), (b"IDAT", &z[..]), (b"IEND", &[][..])] {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let start = out.len();
            out.extend_from_slice(kind);
            out.extend_from_slice(data);
            let crc = crc32(&out[start..]);
            out.extend_from_slice(&crc.to_be_bytes());
        }
        out
    }
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &x in data {
        c ^= u32::from(x);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

pub fn icons_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("icons")
}

/// File name for `site`: lowercase letters, digits, `.`, `-` and `_` kept,
/// anything else (`:` of a port, `/`, `..` tricks) made safe. `None` for an
/// empty site.
pub fn file_name(site: &str) -> Option<String> {
    if site.is_empty() {
        return None;
    }
    let safe: String = site
        .chars()
        .map(|c| match c.to_ascii_lowercase() {
            c @ ('a'..='z' | '0'..='9' | '-' | '_') => c,
            '.' => '.',
            _ => '_',
        })
        .collect();
    // No hidden files and no "." / "..".
    let safe = if safe.starts_with('.') { format!("_{safe}") } else { safe };
    Some(format!("{safe}.icon"))
}

pub fn save_icon(data_dir: &Path, site: &str, icon: &Icon) -> std::io::Result<()> {
    let Some(name) = file_name(site) else { return Ok(()) };
    let dir = icons_dir(data_dir);
    fs::create_dir_all(&dir)?;
    write_atomic(&dir.join(name), &icon.encode(site))
}

/// Every saved icon with its site. Unreadable files are skipped.
pub fn load_icons(data_dir: &Path) -> Vec<(String, Icon)> {
    let Ok(entries) = fs::read_dir(icons_dir(data_dir)) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entries.flatten() {
        if e.path().extension().is_none_or(|x| x != "icon") {
            continue;
        }
        match fs::read(e.path()).ok().as_deref().and_then(Icon::decode) {
            Some(found) => out.push(found),
            None => log::warn!("{}: not a readable icon", e.path().display()),
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(w: u32, h: u32) -> Icon {
        Icon::new(w, h, (0..w * h * 4).map(|i| i as u8).collect()).unwrap()
    }

    #[test]
    fn encode_decode() {
        let i = icon(16, 8);
        assert_eq!(Icon::decode(&i.encode("localhost:8080")), Some(("localhost:8080".into(), i.clone())));
        let mut bytes = i.encode("a.org");
        bytes.pop();
        assert_eq!(Icon::decode(&bytes), None, "truncated");
        let mut bytes = i.encode("a.org");
        bytes[4] = ICON_VERSION + 1;
        assert_eq!(Icon::decode(&bytes), None, "newer version");
        assert_eq!(Icon::decode(&i.encode("")), None, "no site");
        assert_eq!(Icon::decode(b"TBIC"), None);
        assert_eq!(Icon::decode(b"TBIC\x01\x01\x00\x01\x00\x09\x00ab"), None, "site cut short");
        assert_eq!(Icon::decode(b"nope\x01\x01\x00\x01\x00\x01\x00aabcd"), None);
    }

    #[test]
    fn png_is_well_formed() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        let i = Icon::new(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let png = i.png();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[png.len() - 12..], b"\0\0\0\0IEND\xaeB`\x82");
        // Pixels go in as RGBA, one filter byte per row.
        let raw = [0, 3, 2, 1, 4, 7, 6, 5, 8];
        assert!(png.windows(raw.len()).any(|w| w == raw));
        assert!(icon(MAX_SIDE, MAX_SIDE).png().len() < 20_000);
    }

    #[test]
    fn sizes_are_bounded() {
        assert!(Icon::new(0, 4, vec![]).is_none());
        assert!(Icon::new(MAX_SIDE + 1, 1, vec![0; ((MAX_SIDE + 1) * 4) as usize]).is_none());
        assert!(Icon::new(2, 2, vec![0; 15]).is_none());
        assert!(Icon::new(2, 2, vec![0; 16]).is_some());
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("example.com").as_deref(), Some("example.com.icon"));
        assert_eq!(file_name("Localhost:8080").as_deref(), Some("localhost_8080.icon"));
        assert_eq!(file_name("../etc").as_deref(), Some("_.._etc.icon"));
        assert_eq!(file_name("a/b").as_deref(), Some("a_b.icon"));
        assert_eq!(file_name(""), None);
    }

    #[test]
    fn save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_icons(dir.path()).is_empty());
        save_icon(dir.path(), "example.com", &icon(4, 4)).unwrap();
        save_icon(dir.path(), "a.org", &icon(2, 3)).unwrap();
        save_icon(dir.path(), "localhost:8080", &icon(1, 1)).unwrap();
        save_icon(dir.path(), "example.com", &icon(8, 8)).unwrap(); // replaced
        fs::write(icons_dir(dir.path()).join("bad.icon"), b"junk").unwrap();
        let all = load_icons(dir.path());
        let sites: Vec<_> = all.iter().map(|(s, i)| (s.as_str(), i.width)).collect();
        assert_eq!(sites, [("a.org", 2), ("example.com", 8), ("localhost:8080", 1)]);
    }
}
