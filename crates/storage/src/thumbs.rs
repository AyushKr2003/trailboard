//! Page thumbnails: a small JPEG of what a card showed, kept per page in the
//! history index (`pagetext::PageIndex::{set_thumb, thumb}`).
//!
//! Pixels are BGRA8, as CEF paints them and the compositor uploads them.
//! A frame is shrunk (area average) to fit [`MAX_W`] × [`MAX_H`], then
//! encoded at [`QUALITY`]; alpha is dropped (pages are opaque).

use jpeg_encoder::{ColorType, Encoder};
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

/// The largest thumbnail, in pixels.
pub const MAX_W: u32 = 480;
pub const MAX_H: u32 = 360;
/// JPEG quality, 1–100.
pub const QUALITY: u8 = 72;

/// A BGRA8 image.
#[derive(Clone, PartialEq, Eq)]
pub struct Thumb {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl std::fmt::Debug for Thumb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Thumb({}x{})", self.width, self.height)
    }
}

impl Thumb {
    /// `None` unless `pixels` holds `width × height` BGRA pixels.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Thumb> {
        (width > 0 && height > 0 && pixels.len() == width as usize * height as usize * 4)
            .then_some(Thumb { width, height, pixels })
    }

    /// A page frame shrunk to thumbnail size (never enlarged).
    /// `None` for an empty or short frame.
    pub fn from_frame(width: u32, height: u32, pixels: &[u8]) -> Option<Thumb> {
        if width == 0 || height == 0 || pixels.len() < width as usize * height as usize * 4 {
            return None;
        }
        let (w, h) = fit(width, height, MAX_W, MAX_H);
        Some(shrink(width, height, pixels, w, h))
    }

    /// The thumbnail as a JPEG.
    pub fn encode(&self) -> Option<Vec<u8>> {
        let (w, h) = (u16::try_from(self.width).ok()?, u16::try_from(self.height).ok()?);
        let mut out = Vec::new();
        Encoder::new(&mut out, QUALITY).encode(&self.pixels, w, h, ColorType::Bgra).ok()?;
        Some(out)
    }

    /// A JPEG back into pixels; `None` if it isn't one we can read or is
    /// bigger than a thumbnail.
    pub fn decode(jpeg: &[u8]) -> Option<Thumb> {
        let opts = DecoderOptions::default()
            .jpeg_set_out_colorspace(ColorSpace::BGRA)
            .set_max_width(MAX_W as usize * 2)
            .set_max_height(MAX_H as usize * 2);
        let mut d = JpegDecoder::new_with_options(ZCursor::new(jpeg), opts);
        let pixels = d.decode().ok()?;
        let (w, h) = d.dimensions()?;
        Thumb::new(w as u32, h as u32, pixels)
    }

    /// The part of this image with the shape `aspect` (width / height):
    /// the top when it is too tall (a page starts at its top), the middle
    /// when it is too wide.
    pub fn crop_to(&self, aspect: f32) -> Thumb {
        if !(aspect.is_finite() && aspect > 0.0) {
            return self.clone();
        }
        let (w, h) = (self.width, self.height);
        let mine = w as f32 / h as f32;
        let (cw, ch) = if mine > aspect {
            (((h as f32 * aspect).round() as u32).clamp(1, w), h)
        } else {
            (w, ((w as f32 / aspect).round() as u32).clamp(1, h))
        };
        if (cw, ch) == (w, h) {
            return self.clone();
        }
        let x0 = (w - cw) / 2;
        let mut pixels = Vec::with_capacity(cw as usize * ch as usize * 4);
        for y in 0..ch {
            let start = (y as usize * w as usize + x0 as usize) * 4;
            pixels.extend_from_slice(&self.pixels[start..start + cw as usize * 4]);
        }
        Thumb { width: cw, height: ch, pixels }
    }
}

/// The largest size with `w × h`'s shape inside `max_w × max_h`, at most `w × h`.
pub fn fit(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let s = (max_w as f64 / w as f64).min(max_h as f64 / h as f64).min(1.0);
    (((w as f64 * s).round() as u32).max(1), ((h as f64 * s).round() as u32).max(1))
}

/// `src` (BGRA, `sw × sh`) shrunk to `dw × dh` by averaging each output
/// pixel's area.
fn shrink(sw: u32, sh: u32, src: &[u8], dw: u32, dh: u32) -> Thumb {
    let (sw, sh, dw, dh) = (sw as usize, sh as usize, dw as usize, dh as usize);
    let mut out = Vec::with_capacity(dw * dh * 4);
    for y in 0..dh {
        let (y0, y1) = (y * sh / dh, ((y + 1) * sh / dh).max(y * sh / dh + 1));
        for x in 0..dw {
            let (x0, x1) = (x * sw / dw, ((x + 1) * sw / dw).max(x * sw / dw + 1));
            let mut sum = [0u32; 4];
            for row in src[y0 * sw * 4..y1 * sw * 4].chunks_exact(sw * 4) {
                for px in row[x0 * 4..x1 * 4].as_chunks::<4>().0 {
                    for (s, &v) in sum.iter_mut().zip(px) {
                        *s += v as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            out.extend(sum.iter().map(|s| ((s + n / 2) / n) as u8));
        }
    }
    Thumb { width: dw as u32, height: dh as u32, pixels: out }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Left half blue, right half red (BGRA), opaque.
    fn halves(w: u32, h: u32) -> Vec<u8> {
        let mut p = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                p.extend(if x < w / 2 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
            }
        }
        p
    }

    #[test]
    fn fit_keeps_shape_and_never_enlarges() {
        assert_eq!(fit(1600, 1200, 480, 360), (480, 360));
        assert_eq!(fit(1600, 600, 480, 360), (480, 180));
        assert_eq!(fit(600, 1800, 480, 360), (120, 360));
        assert_eq!(fit(200, 100, 480, 360), (200, 100));
        assert_eq!(fit(10_000, 1, 480, 360), (480, 1));
    }

    #[test]
    fn frames_shrink_by_averaging() {
        let t = Thumb::from_frame(960, 720, &halves(960, 720)).unwrap();
        assert_eq!((t.width, t.height), (480, 360));
        assert_eq!(&t.pixels[0..4], &[255, 0, 0, 255]);
        let last = t.pixels.len() - 4;
        assert_eq!(&t.pixels[last..], &[0, 0, 255, 255]);
        // A 2×2 block of black and white averages to grey.
        let checker = [0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255];
        let s = shrink(2, 2, &checker, 1, 1);
        assert_eq!(s.pixels, [128, 128, 128, 255]);
        assert!(Thumb::from_frame(0, 10, &[]).is_none());
        assert!(Thumb::from_frame(10, 10, &[0; 12]).is_none(), "short frame");
    }

    #[test]
    fn encode_decode_round_trip() {
        let t = Thumb::from_frame(800, 600, &halves(800, 600)).unwrap();
        let jpeg = t.encode().unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
        assert!(jpeg.len() < 40_000, "{} bytes", jpeg.len());
        let back = Thumb::decode(&jpeg).unwrap();
        assert_eq!((back.width, back.height), (t.width, t.height));
        // Lossy, but the colours stay put away from the edge.
        let px = |x: usize, y: usize| &back.pixels[(y * back.width as usize + x) * 4..][..3];
        assert!(px(20, 20)[0] > 230 && px(20, 20)[2] < 25, "{:?}", px(20, 20));
        assert!(px(460, 20)[2] > 230 && px(460, 20)[0] < 25, "{:?}", px(460, 20));
        assert!(Thumb::decode(b"not a jpeg").is_none());
        assert!(Thumb::decode(&jpeg[..jpeg.len() / 3]).is_none_or(|t| t.width == back.width));
    }

    #[test]
    fn crop_keeps_the_top_or_the_middle() {
        // 4×4 with row index in blue and column index in green.
        let mut p = Vec::new();
        for y in 0..4u8 {
            for x in 0..4u8 {
                p.extend([y, x, 0, 255]);
            }
        }
        let t = Thumb::new(4, 4, p).unwrap();
        let wide = t.crop_to(2.0);
        assert_eq!((wide.width, wide.height), (4, 2));
        assert_eq!(wide.pixels[(4 + 3) * 4], 1, "rows 0 and 1 kept");
        let tall = t.crop_to(0.5);
        assert_eq!((tall.width, tall.height), (2, 4));
        assert_eq!(tall.pixels[1], 1, "columns 1 and 2 kept");
        assert_eq!(t.crop_to(1.0), t);
        assert_eq!(t.crop_to(f32::NAN), t);
        assert_eq!(t.crop_to(100.0).height, 1);
    }
}
