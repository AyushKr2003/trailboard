//! The page inspector docked in its page's card: the card's content split
//! into the page, a divider and the inspector, side by side or one above
//! the other. Pure geometry; any units (CSS px, screen px), as long as the
//! divider's width is in the same ones.

use crate::Rect;

/// Where the inspector sits in its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Right,
    Bottom,
}

impl Side {
    /// The other side.
    pub fn flip(self) -> Side {
        match self {
            Side::Right => Side::Bottom,
            Side::Bottom => Side::Right,
        }
    }

    /// Parses a command argument (`right`, `bottom`).
    pub fn parse(s: &str) -> Option<Side> {
        match s {
            "right" => Some(Side::Right),
            "bottom" => Some(Side::Bottom),
            _ => None,
        }
    }
}

/// The inspector's share of the card when it opens.
pub const DEFAULT_SHARE: f32 = 0.45;
/// The least and most of the card the inspector takes when its divider is dragged.
pub const MIN_SHARE: f32 = 0.15;
pub const MAX_SHARE: f32 = 0.85;

/// A docked card's parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Split {
    pub page: Rect,
    pub divider: Rect,
    pub inspector: Rect,
}

/// `content` split with the inspector on `side`, taking `share` of it
/// (after the divider, `gap` wide). Sizes are whole units, so a page and
/// its picture line up.
pub fn split(content: Rect, side: Side, share: f32, gap: f32) -> Split {
    let share = share.clamp(MIN_SHARE, MAX_SHARE);
    match side {
        Side::Right => {
            let room = (content.w - gap).max(0.0);
            let insp = (room * share).round();
            let page = room - insp;
            Split {
                page: Rect::new(content.x, content.y, page, content.h),
                divider: Rect::new(content.x + page, content.y, gap, content.h),
                inspector: Rect::new(content.x + page + gap, content.y, insp, content.h),
            }
        }
        Side::Bottom => {
            let room = (content.h - gap).max(0.0);
            let insp = (room * share).round();
            let page = room - insp;
            Split {
                page: Rect::new(content.x, content.y, content.w, page),
                divider: Rect::new(content.x, content.y + page, content.w, gap),
                inspector: Rect::new(content.x, content.y + page + gap, content.w, insp),
            }
        }
    }
}

/// The share that puts the divider's middle at `(x, y)` (dragging it).
pub fn share_at(content: Rect, side: Side, x: f32, y: f32, gap: f32) -> f32 {
    let (along, room) = match side {
        Side::Right => (content.right() - x, content.w - gap),
        Side::Bottom => (content.bottom() - y, content.h - gap),
    };
    ((along - gap / 2.0) / room.max(1.0)).clamp(MIN_SHARE, MAX_SHARE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_splits_across() {
        let s = split(Rect::new(10.0, 20.0, 1006.0, 600.0), Side::Right, 0.5, 6.0);
        assert_eq!(s.page, Rect::new(10.0, 20.0, 500.0, 600.0));
        assert_eq!(s.divider, Rect::new(510.0, 20.0, 6.0, 600.0));
        assert_eq!(s.inspector, Rect::new(516.0, 20.0, 500.0, 600.0));
    }

    #[test]
    fn bottom_splits_down() {
        let s = split(Rect::new(0.0, 0.0, 800.0, 406.0), Side::Bottom, 0.25, 6.0);
        assert_eq!(s.page, Rect::new(0.0, 0.0, 800.0, 300.0));
        assert_eq!(s.divider, Rect::new(0.0, 300.0, 800.0, 6.0));
        assert_eq!(s.inspector, Rect::new(0.0, 306.0, 800.0, 100.0));
    }

    #[test]
    fn the_parts_fill_the_card() {
        for side in [Side::Right, Side::Bottom] {
            for share in [0.0, 0.33, 0.5, 0.71, 1.0] {
                let c = Rect::new(3.0, 4.0, 957.0, 538.0);
                let s = split(c, side, share, 6.0);
                match side {
                    Side::Right => assert_eq!(s.page.w + s.divider.w + s.inspector.w, c.w),
                    Side::Bottom => assert_eq!(s.page.h + s.divider.h + s.inspector.h, c.h),
                }
                assert!(s.page.w > 0.0 && s.page.h > 0.0 && s.inspector.w > 0.0 && s.inspector.h > 0.0);
            }
        }
    }

    #[test]
    fn dragging_the_divider_moves_the_share() {
        let c = Rect::new(0.0, 0.0, 1006.0, 606.0);
        let s = share_at(c, Side::Right, 503.0, 0.0, 6.0);
        assert!((s - 0.5).abs() < 1e-3);
        // Where the divider lands is where it was dragged to.
        let back = split(c, Side::Right, s, 6.0);
        assert!((back.divider.center().x - 503.0).abs() <= 1.0);
        assert_eq!(share_at(c, Side::Right, 1000.0, 0.0, 6.0), MIN_SHARE);
        assert_eq!(share_at(c, Side::Right, 0.0, 0.0, 6.0), MAX_SHARE);
        assert!((share_at(c, Side::Bottom, 0.0, 453.0, 6.0) - 0.25).abs() < 1e-3);
    }

    #[test]
    fn sides_flip_and_parse() {
        assert_eq!(Side::Right.flip(), Side::Bottom);
        assert_eq!(Side::Bottom.flip(), Side::Right);
        assert_eq!(Side::parse("bottom"), Some(Side::Bottom));
        assert_eq!(Side::parse("left"), None);
    }
}
