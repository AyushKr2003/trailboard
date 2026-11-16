//! Multi-select: a set of cards picked with a selection band (Shift+drag on the
//! board) or Shift+click, which then move or close together.
//!
//! Selection is session state, separate from keyboard focus, and is not saved.

use std::collections::BTreeSet;

use crate::{NodeId, Rect, Scene, Vec2};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Selection {
    ids: BTreeSet<NodeId>,
}

impl Selection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.ids.contains(&id)
    }

    /// Selected ids in ascending order.
    pub fn ids(&self) -> Vec<NodeId> {
        self.ids.iter().copied().collect()
    }

    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// Adds `id` if absent, removes it if present.
    pub fn toggle(&mut self, id: NodeId) {
        if !self.ids.remove(&id) {
            self.ids.insert(id);
        }
    }

    pub fn extend(&mut self, ids: impl IntoIterator<Item = NodeId>) {
        self.ids.extend(ids);
    }

    pub fn remove(&mut self, id: NodeId) {
        self.ids.remove(&id);
    }

    /// Drops ids whose card has left the scene (closed, undone).
    pub fn prune(&mut self, scene: &Scene) {
        self.ids.retain(|id| scene.node(*id).is_some());
    }

    /// Selects every card on the board.
    pub fn select_all(&mut self, scene: &Scene) {
        self.ids = scene.nodes().map(|n| n.id).collect();
    }
}

/// Cards whose frame (title bar + content) touches `band`, in id order.
/// A band with no area (a click) selects nothing.
pub fn in_band(scene: &Scene, band: Rect) -> Vec<NodeId> {
    if band.w <= 0.0 || band.h <= 0.0 {
        return Vec::new();
    }
    scene.nodes().filter(|n| n.frame().intersects(&band)).map(|n| n.id).collect()
}

/// Content rects of the cards a group drag will move, taken when the drag
/// starts: every selected card if `grabbed` is selected, else just `grabbed`.
pub fn drag_group(scene: &Scene, sel: &Selection, grabbed: NodeId) -> Vec<(NodeId, Rect)> {
    let ids = if sel.contains(grabbed) { sel.ids() } else { vec![grabbed] };
    ids.into_iter().filter_map(|id| scene.node(id).map(|n| (id, n.rect))).collect()
}

/// Puts each card of a group drag at its start rect moved by `d` (world units).
pub fn move_group(scene: &mut Scene, group: &[(NodeId, Rect)], d: Vec2) {
    for (id, start) in group {
        if let Some(n) = scene.node_mut(*id) {
            n.rect = start.translate(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> (Scene, NodeId, NodeId, NodeId) {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 100.0, 100.0), "a");
        let b = s.add_page(Rect::new(300.0, 0.0, 100.0, 100.0), "b");
        let c = s.add_page(Rect::new(0.0, 400.0, 100.0, 100.0), "c");
        (s, a, b, c)
    }

    #[test]
    fn band_selects_touching_frames() {
        let (s, a, b, _) = board();
        // Touches a's right edge and b's left edge.
        let band = Rect::from_corners(Vec2::new(90.0, 20.0), Vec2::new(310.0, 25.0));
        assert_eq!(in_band(&s, band), vec![a, b]);
        assert!(in_band(&s, Rect::new(150.0, 150.0, 50.0, 50.0)).is_empty());
        assert!(in_band(&s, Rect::new(50.0, 50.0, 0.0, 0.0)).is_empty());
    }

    #[test]
    fn toggle_extend_prune() {
        let (mut s, a, b, c) = board();
        let mut sel = Selection::new();
        sel.extend([a, b]);
        sel.toggle(b);
        sel.toggle(c);
        assert_eq!(sel.ids(), vec![a, c]);
        s.remove(a);
        sel.prune(&s);
        assert_eq!(sel.ids(), vec![c]);
        sel.select_all(&s);
        assert_eq!(sel.len(), 2);
    }

    #[test]
    fn group_drag_moves_selection_or_single_card() {
        let (mut s, a, b, c) = board();
        let mut sel = Selection::new();
        sel.extend([a, b]);
        let g = drag_group(&s, &sel, b);
        assert_eq!(g.len(), 2);
        move_group(&mut s, &g, Vec2::new(10.0, -5.0));
        assert_eq!(s.node(a).unwrap().rect, Rect::new(10.0, -5.0, 100.0, 100.0));
        assert_eq!(s.node(b).unwrap().rect.x, 310.0);
        assert_eq!(s.node(c).unwrap().rect.y, 400.0);
        // Grabbing an unselected card moves only that card.
        assert_eq!(drag_group(&s, &sel, c), vec![(c, s.node(c).unwrap().rect)]);
    }
}
