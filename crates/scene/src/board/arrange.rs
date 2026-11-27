//! Moving and resizing cards from the keyboard (M, then h/j/k/l). Our own design.
//!
//! An [`Arrange`] remembers where the cards were when it started, so Esc can put
//! them back and finishing records one undo step for the whole session. Moves
//! shift every card of the group (the selection, if the current card is in it);
//! resizing changes only the current card and keeps its top-left corner fixed,
//! as the resize grip at its bottom-right does: l / j grow it right / down,
//! h / k shrink it, never below `min`.

use crate::nav::Dir;
use crate::{NodeId, Rect, Scene, Vec2};

#[derive(Debug, Clone, PartialEq)]
pub struct Arrange {
    /// The card being arranged (the current card).
    pub id: NodeId,
    /// Every card that moves with it, with its content rect at the start.
    pub start: Vec<(NodeId, Rect)>,
}

impl Arrange {
    /// Starts arranging `id`, with the cards of `group` (from
    /// [`crate::selection::drag_group`]). `None` if `id` isn't on the board.
    pub fn new(scene: &Scene, id: NodeId, group: Vec<(NodeId, Rect)>) -> Option<Arrange> {
        scene.node(id)?;
        let mut start = group;
        if !start.iter().any(|(g, _)| *g == id) {
            start.push((id, scene.node(id)?.rect));
        }
        Some(Arrange { id, start })
    }

    /// Moves every card of the group `step` world units in `dir`.
    pub fn nudge(&self, scene: &mut Scene, dir: Dir, step: f32) {
        let d = offset(dir, step);
        for (id, _) in &self.start {
            if let Some(n) = scene.node_mut(*id) {
                n.rect = n.rect.translate(d);
            }
        }
    }

    /// Resizes the arranged card by `step` world units: Right/Down grow it,
    /// Left/Up shrink it, down to `min` on each side.
    pub fn resize(&self, scene: &mut Scene, dir: Dir, step: f32, min: f32) {
        if let Some(n) = scene.node_mut(self.id) {
            n.rect = grow(n.rect, dir, step, min);
        }
    }

    /// Puts every card back where it was at the start (Esc).
    pub fn cancel(&self, scene: &mut Scene) {
        for (id, r) in &self.start {
            if let Some(n) = scene.node_mut(*id) {
                n.rect = *r;
            }
        }
    }

    /// The arranged card is gone (closed or undone away).
    pub fn is_stale(&self, scene: &Scene) -> bool {
        scene.node(self.id).is_none()
    }
}

fn offset(dir: Dir, step: f32) -> Vec2 {
    match dir {
        Dir::Left => Vec2::new(-step, 0.0),
        Dir::Right => Vec2::new(step, 0.0),
        Dir::Up => Vec2::new(0.0, -step),
        Dir::Down => Vec2::new(0.0, step),
    }
}

/// `r` with its right or bottom edge moved by `step` in `dir`, top-left fixed,
/// no side smaller than `min` (a card already smaller than `min` never shrinks).
pub fn grow(r: Rect, dir: Dir, step: f32, min: f32) -> Rect {
    let shrink = |len: f32| (len - step).max(min.min(len));
    let (w, h) = match dir {
        Dir::Right => (r.w + step, r.h),
        Dir::Left => (shrink(r.w), r.h),
        Dir::Down => (r.w, r.h + step),
        Dir::Up => (r.w, shrink(r.h)),
    };
    Rect::new(r.x, r.y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{selection, History, Selection};

    fn board() -> (Scene, NodeId, NodeId, NodeId) {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.add_page(Rect::new(500.0, 0.0, 400.0, 300.0), "b");
        let c = s.add_page(Rect::new(0.0, 500.0, 400.0, 300.0), "c");
        (s, a, b, c)
    }

    #[test]
    fn nudges_the_card_and_cancel_puts_it_back() {
        let (mut s, a, b, _) = board();
        let g = selection::drag_group(&s, &Selection::new(), a);
        let ar = Arrange::new(&s, a, g).unwrap();
        ar.nudge(&mut s, Dir::Right, 40.0);
        ar.nudge(&mut s, Dir::Down, 40.0);
        ar.nudge(&mut s, Dir::Down, 8.0);
        ar.nudge(&mut s, Dir::Left, 10.0);
        ar.nudge(&mut s, Dir::Up, 0.0);
        assert_eq!(s.node(a).unwrap().rect, Rect::new(30.0, 48.0, 400.0, 300.0));
        assert_eq!(s.node(b).unwrap().rect.x, 500.0, "other cards stay");
        ar.cancel(&mut s);
        assert_eq!(s.node(a).unwrap().rect, Rect::new(0.0, 0.0, 400.0, 300.0));
    }

    #[test]
    fn moves_the_selection_with_a_selected_card_but_resizes_one() {
        let (mut s, a, b, c) = board();
        let mut sel = Selection::new();
        sel.extend([a, b]);
        let ar = Arrange::new(&s, b, selection::drag_group(&s, &sel, b)).unwrap();
        ar.nudge(&mut s, Dir::Up, 20.0);
        assert_eq!(s.node(a).unwrap().rect.y, -20.0);
        assert_eq!(s.node(b).unwrap().rect.y, -20.0);
        assert_eq!(s.node(c).unwrap().rect.y, 500.0);
        ar.resize(&mut s, Dir::Right, 50.0, 240.0);
        assert_eq!(s.node(b).unwrap().rect.w, 450.0);
        assert_eq!(s.node(a).unwrap().rect.w, 400.0);
    }

    #[test]
    fn resize_keeps_the_corner_and_the_minimum() {
        let r = Rect::new(10.0, 20.0, 300.0, 260.0);
        assert_eq!(grow(r, Dir::Right, 40.0, 240.0), Rect::new(10.0, 20.0, 340.0, 260.0));
        assert_eq!(grow(r, Dir::Down, 40.0, 240.0), Rect::new(10.0, 20.0, 300.0, 300.0));
        assert_eq!(grow(r, Dir::Left, 40.0, 240.0), Rect::new(10.0, 20.0, 260.0, 260.0));
        assert_eq!(grow(r, Dir::Up, 40.0, 240.0), Rect::new(10.0, 20.0, 300.0, 240.0));
        assert_eq!(grow(r, Dir::Left, 400.0, 240.0).w, 240.0);
        // A card already under the minimum doesn't shrink further, nor jump up.
        let small = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(grow(small, Dir::Left, 40.0, 240.0).w, 100.0);
    }

    #[test]
    fn a_whole_session_is_one_undo_step() {
        let (mut s, a, _, _) = board();
        let ar = Arrange::new(&s, a, Vec::new()).unwrap();
        assert_eq!(ar.start, vec![(a, Rect::new(0.0, 0.0, 400.0, 300.0))]);
        ar.nudge(&mut s, Dir::Right, 40.0);
        ar.resize(&mut s, Dir::Down, 40.0, 240.0);
        let mut h = History::new();
        h.moved_many(&s, &ar.start);
        h.undo(&mut s);
        assert_eq!(s.node(a).unwrap().rect, Rect::new(0.0, 0.0, 400.0, 300.0));
        assert!(h.undo(&mut s).is_empty(), "nothing else to undo");
        assert!(!ar.is_stale(&s));
        s.remove(a);
        assert!(ar.is_stale(&s));
        assert!(Arrange::new(&s, a, Vec::new()).is_none());
    }
}
