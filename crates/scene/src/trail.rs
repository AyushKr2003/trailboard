//! Next/Previous on Trail: step along a trail's cards in reading order (the
//! order of [`Scene::trail_of`]: roots, then each card's links depth first),
//! whether the trail is spread out or folded into a stack. Our own design.
//!
//! - Stepping stops at either end instead of wrapping, so the reader knows the
//!   trail is done (stacks wrap when peeked; a trail is read through once).
//! - A card folded away behind its stack's front is brought to the front in
//!   the front's place, as peeking does. Like peeking, this is not an edit.

use crate::{NodeId, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailStep {
    /// Now at `to`, the `index`th (from 0) of the trail's `len` cards.
    Moved { to: NodeId, index: usize, len: usize },
    /// Already at the first (stepping back) or last (forward) card.
    End { first: bool, len: usize },
    /// The card has no trail (or doesn't exist).
    Lone,
}

impl Scene {
    /// Steps `step` cards along the trail of `id` (negative goes back) and,
    /// if the card reached is folded away, shows it at its stack's front.
    pub fn step_trail(&mut self, id: NodeId, step: isize) -> TrailStep {
        let trail = self.trail_of(id);
        let len = trail.len();
        if len < 2 {
            return TrailStep::Lone;
        }
        let at = trail.iter().position(|&n| n == id).expect("a card is in its own trail") as isize;
        let next = at + step;
        if next < 0 || next >= len as isize {
            return TrailStep::End { first: step < 0, len };
        }
        let to = trail[next as usize];
        if self.is_hidden(to) {
            self.show(to);
        }
        TrailStep::Moved { to, index: next as usize, len }
    }
}

#[cfg(test)]
mod tests {
    use super::TrailStep;
    use crate::{Rect, Scene};

    /// a → b, a → c, c → d, plus lone e.
    fn board() -> (Scene, [crate::NodeId; 5]) {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        let c = s.spawn_beside(a, "c").unwrap();
        let d = s.spawn_beside(c, "d").unwrap();
        let e = s.add_page(Rect::new(0.0, 2000.0, 400.0, 300.0), "e");
        (s, [a, b, c, d, e])
    }

    #[test]
    fn steps_in_reading_order_and_stops_at_the_ends() {
        let (mut s, [a, b, c, d, _]) = board();
        assert_eq!(s.step_trail(a, 1), TrailStep::Moved { to: b, index: 1, len: 4 });
        assert_eq!(s.step_trail(b, 1), TrailStep::Moved { to: c, index: 2, len: 4 });
        assert_eq!(s.step_trail(c, 1), TrailStep::Moved { to: d, index: 3, len: 4 });
        assert_eq!(s.step_trail(d, 1), TrailStep::End { first: false, len: 4 });
        assert_eq!(s.step_trail(d, -1), TrailStep::Moved { to: c, index: 2, len: 4 });
        assert_eq!(s.step_trail(a, -1), TrailStep::End { first: true, len: 4 });
    }

    #[test]
    fn a_lone_card_has_no_trail() {
        let (mut s, [.., e]) = board();
        assert_eq!(s.step_trail(e, 1), TrailStep::Lone);
        assert_eq!(s.step_trail(crate::NodeId(999), 1), TrailStep::Lone);
    }

    #[test]
    fn stepping_through_a_stack_brings_each_card_to_the_front_in_place() {
        let (mut s, [a, b, c, d, _]) = board();
        let at = s.node(c).unwrap().rect.pos();
        s.fold(c).unwrap();
        assert_eq!(s.step_trail(c, 1), TrailStep::Moved { to: d, index: 3, len: 4 });
        assert_eq!(s.stack_of(d).unwrap().front, d);
        assert!(!s.is_hidden(d) && s.is_hidden(c));
        assert_eq!(s.node(d).unwrap().rect.pos(), at);
        assert_eq!(s.step_trail(d, -3), TrailStep::Moved { to: a, index: 0, len: 4 });
        assert_eq!(s.stack_of(a).unwrap().front, a);
        assert_eq!(s.node(a).unwrap().rect.pos(), at);
        assert!(s.is_hidden(b));
    }
}
