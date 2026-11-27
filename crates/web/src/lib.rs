//! Chromium (CEF) in off-screen-rendering mode. Each card owns one windowless
//! browser whose frames arrive as BGRA pixel buffers; the app uploads them to
//! GPU textures. All CEF callbacks run on the main thread because we drive CEF
//! with `external_message_pump` + [`pump`], so plain `Rc`/`RefCell` are enough.
//! The one exception is request blocking ([`shield`]), asked on Chromium's IO thread.

pub mod caret;
pub mod consent;
pub mod cosmetic;
pub mod dialog;
pub mod download;
pub mod field;
pub mod focus;
pub mod inspector;
pub mod keys;
pub mod links;
pub mod media;
pub mod menu;
pub mod pager;
pub mod permission;
pub mod popup;
pub mod prefs;
pub mod render_node;
pub(crate) mod resources;
pub mod sandbox;
pub mod scroll;
pub mod shield;
pub mod text;
pub mod youtube;

use cef::{args::Args, *};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc as StdRc;
use std::sync::atomic::{AtomicU64, Ordering};

use popup::IRect;

mod events;
mod runtime;
mod engine;
mod frame;
mod shared;
mod handlers;
mod page;

pub use events::*;
pub use runtime::*;
pub use engine::*;
pub use frame::*;
pub use shared::*;
use handlers::*;
pub use page::*;

#[cfg(test)]
mod tests {
    use super::{extra_switches, IRect, PaintFrame, DAMAGE_KEPT};

    #[test]
    fn extra_switches_reads_names_and_values() {
        let s = |n: &str, v: Option<&str>| (n.to_string(), v.map(str::to_string));
        assert_eq!(
            extra_switches("  --disable-gpu-compositing\t--use-gl=angle disable-gpu -- --=x"),
            vec![s("disable-gpu-compositing", None), s("use-gl", Some("angle")), s("disable-gpu", None)]
        );
        assert!(extra_switches("").is_empty());
    }

    fn r(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect { x, y, w, h }
    }

    fn frame() -> PaintFrame {
        PaintFrame { width: 100, height: 50, ..Default::default() }
    }

    #[test]
    fn changed_since_unions_the_paints_after_a_serial() {
        let mut f = frame();
        f.bump(Some(r(10, 10, 2, 16))); // 1: a caret
        f.bump(Some(r(40, 5, 10, 10))); // 2
        f.bump(Some(r(0, 30, 5, 5))); // 3
        assert_eq!(f.changed_since(3), Some(IRect::default()), "nothing new");
        assert_eq!(f.changed_since(2), Some(r(0, 30, 5, 5)));
        assert_eq!(f.changed_since(1), Some(r(0, 5, 50, 30)));
        assert_eq!(f.changed_since(0), Some(r(0, 5, 50, 30)));
    }

    #[test]
    fn changed_since_is_clamped_to_the_frame() {
        let mut f = frame();
        f.bump(Some(r(-5, 40, 200, 30)));
        assert_eq!(f.changed_since(0), Some(r(0, 40, 100, 10)));
    }

    #[test]
    fn changed_since_doesnt_know_after_a_whole_frame_change_or_when_too_far_behind() {
        let mut f = frame();
        f.bump(Some(r(1, 1, 1, 1)));
        f.bump(None); // a popup, a resize
        f.bump(Some(r(1, 1, 1, 1)));
        assert_eq!(f.changed_since(0), None);
        assert_eq!(f.changed_since(1), None);
        assert_eq!(f.changed_since(2), Some(r(1, 1, 1, 1)));
        for _ in 0..DAMAGE_KEPT {
            f.bump(Some(r(1, 1, 1, 1)));
        }
        assert_eq!(f.changed_since(2), None, "serial 3 is forgotten");
        assert_eq!(f.changed_since(3), Some(r(1, 1, 1, 1)));
    }

    /// The engine the tests load is the one at `CEF_PATH`, which the build
    /// already checked is this version (`cef-dll-sys` reads its archive.json).
    #[test]
    fn the_loaded_engine_reports_its_version() {
        assert_eq!(super::loaded_engine_version(), super::built_engine_version());
        assert!(super::built_engine_version().0 > 100);
    }

    #[test]
    fn a_paint_is_matched_to_the_view_size_it_was_laid_out_at() {
        use super::paint_css;
        // Card 1024×720 at 0.5, then read at 1440×866 at 2.
        let recent = [((1440, 866), 2.0), ((1024, 720), 0.5)];
        assert_eq!(paint_css((2880, 1732), &recent), (1440.0, 866.0));
        // The old paint, still showing after the resize.
        assert_eq!(paint_css((512, 360), &recent), (1024.0, 720.0));
        // Rounding is tolerated.
        assert_eq!(paint_css((513, 360), &recent), (1024.0, 720.0));
        // Unknown: the pixels over the newest scale.
        assert_eq!(paint_css((800, 600), &recent), (400.0, 300.0));
        assert_eq!(paint_css((10, 10), &[]), (10.0, 10.0));
    }

    use super::{js_string, utf16_len, zoom_level_for};

    #[test]
    fn js_strings_are_escaped() {
        assert_eq!(js_string("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
        assert_eq!(js_string("</script>\u{2028}"), r#""\u003c/script\u003e\u2028""#);
        assert_eq!(js_string("\u{1}é😀"), "\"\\u0001é😀\"");
    }

    #[test]
    fn utf16_lengths() {
        assert_eq!(utf16_len("aé"), 2);
        assert_eq!(utf16_len("😀"), 2);
    }

    #[test]
    fn zoom_level_matches_chromium_factor() {
        for s in [0.5f32, 1.0, 1.25, 2.0] {
            let factor = 1.2f64.powf(zoom_level_for(s));
            assert!((factor - s as f64).abs() < 1e-9, "{s}: {factor}");
        }
        assert_eq!(zoom_level_for(1.0), 0.0);
    }
}
