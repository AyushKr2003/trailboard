//! Site icons in title bars, and the name label a card gets when the board
//! is zoomed out too far to read its title.

use crate::geom::{Rect, Vec2};
use crate::pagezoom::site_of;

/// Title bars smaller than this (px) draw no title: the card under the
/// pointer, or else the current card, gets a [`name_label`] instead. One
/// threshold for both, so a title never shows twice.
pub const READABLE_TITLE: f32 = 6.0;

/// The site an icon is kept under: the page's site ([`site_of`]) for web
/// pages with a host, `None` for `file:`, `data:`, `about:` and the like
/// (they would all share one icon).
pub fn site(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once(':')?;
    let web = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
    if !web || !rest.starts_with("//") {
        return None;
    }
    let s = site_of(url);
    (!s.is_empty() && !s.eq_ignore_ascii_case(scheme)).then_some(s)
}

/// Where a `size` name label goes for a card drawn at `frame` (screen px):
/// under the card, left-aligned with it, `gap` below; above it when there is
/// no room below. Always kept inside the `viewport`, `margin` from its edges.
pub fn name_label(frame: Rect, size: Vec2, viewport: Vec2, gap: f32, margin: f32) -> Rect {
    let below = frame.bottom() + gap;
    let above = frame.y - gap - size.y;
    let y = if below + size.y <= viewport.y - margin || above < margin { below } else { above };
    let max_x = (viewport.x - margin - size.x).max(margin);
    let max_y = (viewport.y - margin - size.y).max(margin);
    Rect::new(frame.x.clamp(margin, max_x), y.clamp(margin, max_y), size.x, size.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_hosts_have_icon_sites() {
        assert_eq!(site("https://www.Example.com/a?b").as_deref(), Some("example.com"));
        assert_eq!(site("http://localhost:8080/").as_deref(), Some("localhost:8080"));
        assert_eq!(site("file:///home/x/page.html"), None);
        assert_eq!(site("data:text/html,hi"), None);
        assert_eq!(site("about:blank"), None);
        assert_eq!(site("https:///nohost"), None);
        assert_eq!(site("chrome://version"), None);
        assert_eq!(site(""), None);
    }

    #[test]
    fn label_goes_under_the_card_inside_the_view() {
        let vp = Vec2::new(800.0, 600.0);
        let size = Vec2::new(200.0, 24.0);
        // Room below: under the card, left-aligned.
        let r = name_label(Rect::new(100.0, 100.0, 60.0, 40.0), size, vp, 6.0, 8.0);
        assert_eq!((r.x, r.y), (100.0, 146.0));
        // At the bottom edge: above the card.
        let r = name_label(Rect::new(100.0, 560.0, 60.0, 30.0), size, vp, 6.0, 8.0);
        assert_eq!(r.y, 560.0 - 6.0 - 24.0);
        // Near the right edge: pulled left to fit.
        let r = name_label(Rect::new(750.0, 100.0, 40.0, 40.0), size, vp, 6.0, 8.0);
        assert_eq!(r.x, 800.0 - 8.0 - 200.0);
        // A card taller than the view: kept on screen.
        let r = name_label(Rect::new(-50.0, -100.0, 60.0, 900.0), size, vp, 6.0, 8.0);
        assert_eq!((r.x, r.y), (8.0, 600.0 - 8.0 - 24.0));
    }
}
