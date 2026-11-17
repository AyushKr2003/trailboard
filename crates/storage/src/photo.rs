//! Board photos: a picture file (JPEG, PNG or WebP) read into BGRA pixels
//! for a space's background (*Board background: Photo*), shrunk on reading
//! so a large wallpaper never sits in memory at full size, then cut and
//! scaled to cover the window ([`cover`]).

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

use crate::thumbs::{self, Thumb};

/// A photo is kept at most this big after reading (px on its longer side).
pub const MAX_SIDE: u32 = 2560;
/// Files bigger than this are not read.
pub const MAX_BYTES: u64 = 64 << 20;
/// Pictures larger than this many pixels are refused before decoding.
const MAX_PIXELS: usize = 12_000 * 12_000;

/// File extensions read as pictures.
const EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "jpe"];

/// Whether `path` names a picture this module reads (by its extension).
pub fn is_picture(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The picture at `path`, shrunk to fit [`MAX_SIDE`].
pub fn load(path: &Path) -> Result<Thumb, String> {
    let len = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_BYTES {
        return Err(format!("{} MB is too big for a background", len >> 20));
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let img = decode(&bytes).ok_or_else(|| "not a JPEG, PNG or WebP picture".to_string())?;
    let (w, h) = thumbs::fit(img.width, img.height, MAX_SIDE, MAX_SIDE);
    Ok(if (w, h) == (img.width, img.height) { img } else { thumbs::shrink(img.width, img.height, &img.pixels, w, h) })
}

/// A picture in memory, by what its first bytes say it is.
pub fn decode(bytes: &[u8]) -> Option<Thumb> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        decode_jpeg(bytes)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        decode_png(bytes)
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        decode_webp(bytes)
    } else {
        None
    }
}

fn decode_jpeg(bytes: &[u8]) -> Option<Thumb> {
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::BGRA).set_max_width(12_000).set_max_height(12_000);
    let mut d = JpegDecoder::new_with_options(ZCursor::new(bytes), opts);
    let pixels = d.decode().ok()?;
    let (w, h) = d.dimensions()?;
    Thumb::new(w as u32, h as u32, pixels)
}

fn decode_png(bytes: &[u8]) -> Option<Thumb> {
    let opts = DecoderOptions::default()
        .png_set_strip_to_8bit(true)
        .png_set_add_alpha_channel(true)
        .set_max_width(12_000)
        .set_max_height(12_000);
    let mut d = zune_png::PngDecoder::new_with_options(ZCursor::new(bytes), opts);
    d.decode_headers().ok()?;
    let (w, h) = d.dimensions()?;
    let space = d.colorspace()?;
    let raw = d.decode_raw().ok()?;
    let pixels = match space {
        ColorSpace::RGBA => raw.as_chunks::<4>().0.iter().flat_map(|&[r, g, b, a]| [b, g, r, a]).collect(),
        ColorSpace::RGB => raw.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [b, g, r, 255]).collect(),
        ColorSpace::LumaA => raw.as_chunks::<2>().0.iter().flat_map(|&[l, a]| [l, l, l, a]).collect(),
        ColorSpace::Luma => raw.iter().flat_map(|&l| [l, l, l, 255]).collect(),
        _ => return None,
    };
    Thumb::new(w as u32, h as u32, pixels)
}

fn decode_webp(bytes: &[u8]) -> Option<Thumb> {
    let mut d = image_webp::WebPDecoder::new(Cursor::new(bytes)).ok()?;
    let (w, h) = d.dimensions();
    if (w as usize).saturating_mul(h as usize) > MAX_PIXELS {
        return None;
    }
    let mut buf = vec![0; d.output_buffer_size()?];
    d.read_image(&mut buf).ok()?;
    let pixels = if d.has_alpha() {
        buf.as_chunks::<4>().0.iter().flat_map(|&[r, g, b, a]| [b, g, r, a]).collect()
    } else {
        buf.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [b, g, r, 255]).collect()
    };
    Thumb::new(w, h, pixels)
}

/// `img` cut to the shape of `w × h` (its middle) and scaled to `w × h`
/// or, if smaller than that, to the largest size with that shape it has.
/// The renderer stretches it over the window either way.
pub fn cover(img: &Thumb, w: u32, h: u32) -> Thumb {
    if w == 0 || h == 0 {
        return img.clone();
    }
    let aspect = w as f32 / h as f32;
    let (iw, ih) = (img.width, img.height);
    let (cw, ch) = if iw as f32 / ih as f32 > aspect {
        (((ih as f32 * aspect).round() as u32).clamp(1, iw), ih)
    } else {
        (iw, ((iw as f32 / aspect).round() as u32).clamp(1, ih))
    };
    let (x0, y0) = ((iw - cw) / 2, (ih - ch) / 2);
    let mut cut = Vec::with_capacity(cw as usize * ch as usize * 4);
    for y in y0..y0 + ch {
        let start = (y as usize * iw as usize + x0 as usize) * 4;
        cut.extend_from_slice(&img.pixels[start..start + cw as usize * 4]);
    }
    let (dw, dh) = thumbs::fit(cw, ch, w, h);
    if (dw, dh) == (cw, ch) {
        Thumb { width: cw, height: ch, pixels: cut }
    } else {
        thumbs::shrink(cw, ch, &cut, dw, dh)
    }
}

/// Pictures directly in each of `dirs` (not below), each folder's sorted
/// by name, at most `max` in all.
pub fn pictures(dirs: &[PathBuf], max: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for d in dirs {
        let Ok(entries) = fs::read_dir(d) else { continue };
        let mut found: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file() && is_picture(p)).collect();
        found.sort();
        out.extend(found);
    }
    out.dedup();
    out.truncate(max);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `w × h` BGRA, each pixel's blue its column and green its row.
    fn ramp(w: u32, h: u32) -> Thumb {
        let mut p = Vec::new();
        for y in 0..h {
            for x in 0..w {
                p.extend([x as u8, y as u8, 0, 255]);
            }
        }
        Thumb::new(w, h, p).unwrap()
    }

    #[test]
    fn cover_cuts_the_middle_and_keeps_the_shape() {
        // Too wide for a square: the middle 40 columns.
        let c = cover(&ramp(80, 40), 40, 40);
        assert_eq!((c.width, c.height), (40, 40));
        assert_eq!(c.pixels[0], 20, "starts at column 20");
        // Too tall for 2:1: the middle rows, then shrunk to the window.
        let c = cover(&ramp(40, 80), 20, 10);
        assert_eq!((c.width, c.height), (20, 10));
        // Smaller than the window: cut but not enlarged.
        let c = cover(&ramp(30, 30), 300, 100);
        assert_eq!((c.width, c.height), (30, 10));
        assert_eq!(c.pixels[1], 10, "starts at row 10");
    }

    #[test]
    fn decodes_each_format_and_refuses_others() {
        let img = ramp(6, 4);
        let jpeg = img.encode().unwrap();
        let back = decode(&jpeg).unwrap();
        assert_eq!((back.width, back.height), (6, 4));
        // A 1×1 PNG, RGB, a red pixel.
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0xC9, 0xFE, 0x92, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
            0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let p = decode(png).unwrap();
        assert_eq!((p.width, p.height), (1, 1));
        assert_eq!(p.pixels, [0, 0, 255, 255], "red, as BGRA");
        assert!(decode(b"GIF89a....").is_none());
        assert!(decode(b"RIFF\0\0\0\0WEBPjunk").is_none());
    }

    #[test]
    fn lists_pictures_and_loads_them() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.JPG"), ramp(8, 8).encode().unwrap()).unwrap();
        fs::write(dir.path().join("a.webp"), b"not really").unwrap();
        fs::write(dir.path().join("notes.txt"), b"hi").unwrap();
        fs::create_dir(dir.path().join("sub.png")).unwrap();
        let found = pictures(&[dir.path().to_path_buf(), dir.path().join("missing")], 10);
        assert_eq!(found, vec![dir.path().join("a.webp"), dir.path().join("b.JPG")]);
        assert_eq!(pictures(&[dir.path().to_path_buf()], 1).len(), 1);
        assert_eq!(load(&dir.path().join("b.JPG")).map(|t| (t.width, t.height)), Ok((8, 8)));
        assert!(load(&dir.path().join("a.webp")).is_err());
        assert!(load(&dir.path().join("nope.png")).is_err());
    }
}
