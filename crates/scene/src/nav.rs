//! Keyboard moves between cards: from one card to its nearest neighbour in a
//! direction (h/j/k/l or the arrow keys). Our own design.
//!
//! A card counts as being in a direction when its centre lies past the current
//! card's centre that way. Among those, the one with the smallest gap along the
//! direction wins, with sideways distance weighing three times as much, so a
//! card straight ahead beats a nearer one far off to the side. Cards side by
//! side in the other axis (overlapping ranges) have no sideways distance.

use crate::{NodeId, Rect, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// How much more sideways distance costs than distance ahead.
const SIDEWAYS: f32 = 3.0;

/// The card to move to from `from` in `dir`, among the cards on the board.
/// `None` when nothing lies that way (or `from` doesn't exist).
pub fn neighbour(scene: &Scene, from: NodeId, dir: Dir) -> Option<NodeId> {
    let a = scene.node(from)?.frame();
    scene
        .nodes()
        .filter(|n| n.id != from)
        .filter_map(|n| score(a, n.frame(), dir).map(|s| (s, n.id)))
        .min_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)))
        .map(|(_, id)| id)
}

/// Cost of moving from `a` to `b` in `dir`; `None` if `b` isn't that way.
fn score(a: Rect, b: Rect, dir: Dir) -> Option<f32> {
    let (ca, cb) = (a.center(), b.center());
    // Distance ahead, gap ahead (0 when the cards overlap that way), and the
    // sideways gap between the two cards' ranges.
    let (ahead, gap, side) = match dir {
        Dir::Right => (cb.x - ca.x, b.x - a.right(), range_gap(a.y, a.bottom(), b.y, b.bottom())),
        Dir::Left => (ca.x - cb.x, a.x - b.right(), range_gap(a.y, a.bottom(), b.y, b.bottom())),
        Dir::Down => (cb.y - ca.y, b.y - a.bottom(), range_gap(a.x, a.right(), b.x, b.right())),
        Dir::Up => (ca.y - cb.y, a.y - b.bottom(), range_gap(a.x, a.right(), b.x, b.right())),
    };
    (ahead > 0.5).then(|| gap.max(0.0) + SIDEWAYS * side + ahead * 1e-3)
}

/// Distance between the ranges `[a0, a1]` and `[b0, b1]`; 0 if they overlap.
fn range_gap(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    (b0 - a1).max(a0 - b1).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3×2 grid of 400×300 cards, 100 apart, plus one far off to the right
    /// and low.
    ///
    /// ```text
    /// a b c
    /// d e f        g (far right, below)
    /// ```
    fn grid() -> (Scene, Vec<NodeId>) {
        let mut s = Scene::new();
        let mut ids = Vec::new();
        for row in 0..2 {
            for col in 0..3 {
                let r = Rect::new(col as f32 * 500.0, row as f32 * 430.0, 400.0, 300.0);
                ids.push(s.add_page(r, format!("{row}{col}")));
            }
        }
        ids.push(s.add_page(Rect::new(3000.0, 2000.0, 400.0, 300.0), "g"));
        (s, ids)
    }

    #[test]
    fn moves_to_the_card_next_to_it() {
        let (s, v) = grid();
        let [a, b, c, d, e, f, _g] = v[..] else { unreachable!() };
        assert_eq!(neighbour(&s, a, Dir::Right), Some(b));
        assert_eq!(neighbour(&s, b, Dir::Right), Some(c));
        assert_eq!(neighbour(&s, a, Dir::Down), Some(d));
        assert_eq!(neighbour(&s, e, Dir::Up), Some(b));
        assert_eq!(neighbour(&s, e, Dir::Left), Some(d));
        assert_eq!(neighbour(&s, a, Dir::Left), None);
        assert_eq!(neighbour(&s, a, Dir::Up), None);
        // Straight below beats g, which is further down but far to the side.
        assert_eq!(neighbour(&s, c, Dir::Down), Some(f));
    }

    #[test]
    fn reaches_a_card_off_to_the_side_when_nothing_is_ahead() {
        let (s, v) = grid();
        let (f, g) = (v[5], v[6]);
        assert_eq!(neighbour(&s, f, Dir::Right), Some(g));
        assert_eq!(neighbour(&s, f, Dir::Down), Some(g));
        assert_eq!(neighbour(&s, g, Dir::Left), Some(f));
        assert_eq!(neighbour(&s, g, Dir::Up), Some(f));
    }

    #[test]
    fn folded_cards_are_skipped_and_missing_cards_go_nowhere() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        let c = s.add_page(Rect::new(0.0, 1000.0, 400.0, 300.0), "c");
        s.fold(a).unwrap(); // b is folded away behind a
        assert_eq!(neighbour(&s, a, Dir::Right), None);
        assert_eq!(neighbour(&s, a, Dir::Down), Some(c));
        assert!(s.is_hidden(b));
        assert_eq!(neighbour(&s, NodeId(99), Dir::Down), None);
    }
}
