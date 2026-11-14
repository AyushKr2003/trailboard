//! Undo/redo for board edits: opening, closing, moving and resizing cards.
//!
//! Each stack holds the edit that reverses an action. Applying an edit to the
//! scene returns its own reverse, which goes on the other stack, so undo and
//! redo share one code path. Page navigation, titles and stacking order are not
//! board edits and are not recorded.

use crate::{Edge, Node, NodeId, Rect, Scene};

/// Most actions kept for undo.
pub const LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// Give a card this content rect.
    SetRect { id: NodeId, rect: Rect },
    /// Put a removed card back, with its trail edges.
    Insert { node: Node, edges: Vec<Edge> },
    /// Remove a card.
    Remove(NodeId),
}

/// What an undo or redo did, so the app can start or stop the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added(NodeId),
    Removed(NodeId),
    Moved(NodeId),
}

impl Edit {
    /// Applies the edit; returns its reverse and what changed, or `None` if it
    /// no longer applies (its card is gone or its id is taken).
    fn apply(self, scene: &mut Scene) -> Option<(Edit, Change)> {
        match self {
            Edit::SetRect { id, rect } => {
                let n = scene.node_mut(id)?;
                let old = std::mem::replace(&mut n.rect, rect);
                Some((Edit::SetRect { id, rect: old }, Change::Moved(id)))
            }
            Edit::Insert { node, edges } => {
                let id = node.id;
                scene.insert(node, &edges).then_some((Edit::Remove(id), Change::Added(id)))
            }
            Edit::Remove(id) => {
                let (node, edges) = scene.remove(id)?;
                Some((Edit::Insert { node, edges }, Change::Removed(id)))
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// A card was opened.
    pub fn opened(&mut self, id: NodeId) {
        self.push(Edit::Remove(id));
    }

    /// A card was closed; `node` and `edges` are what [`Scene::remove`] returned.
    pub fn closed(&mut self, node: Node, edges: Vec<Edge>) {
        self.push(Edit::Insert { node, edges });
    }

    /// A card was moved or resized from `before`. No-op if it didn't change.
    pub fn moved(&mut self, scene: &Scene, id: NodeId, before: Rect) {
        if scene.node(id).is_some_and(|n| n.rect != before) {
            self.push(Edit::SetRect { id, rect: before });
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, scene: &mut Scene) -> Option<Change> {
        Self::step(&mut self.undo, &mut self.redo, scene)
    }

    pub fn redo(&mut self, scene: &mut Scene) -> Option<Change> {
        Self::step(&mut self.redo, &mut self.undo, scene)
    }

    fn push(&mut self, e: Edit) {
        self.redo.clear();
        self.undo.push(e);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    /// Applies the newest edit that still applies; stale ones are dropped.
    fn step(from: &mut Vec<Edit>, to: &mut Vec<Edit>, scene: &mut Scene) -> Option<Change> {
        while let Some(e) = from.pop() {
            if let Some((reverse, change)) = e.apply(scene) {
                to.push(reverse);
                return Some(change);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec2;

    fn board() -> (Scene, History, NodeId) {
        let mut s = Scene::new();
        let mut h = History::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "https://a");
        h.opened(a);
        (s, h, a)
    }

    #[test]
    fn undo_redo_open() {
        let (mut s, mut h, a) = board();
        let b = s.spawn_beside(a, "https://b").unwrap();
        h.opened(b);
        assert_eq!(h.undo(&mut s), Some(Change::Removed(b)));
        assert!(s.node(b).is_none() && s.edges().is_empty());
        assert_eq!(h.redo(&mut s), Some(Change::Added(b)));
        assert_eq!(s.node(b).unwrap().url, "https://b");
        assert_eq!(s.edges(), &[Edge { from: a, to: b }]);
    }

    #[test]
    fn undo_close_restores_card_and_trail() {
        let (mut s, mut h, a) = board();
        let b = s.spawn_beside(a, "https://b").unwrap();
        h.opened(b);
        let rect_b = s.node(b).unwrap().rect;
        let (node, edges) = s.remove(b).unwrap();
        h.closed(node, edges);
        assert_eq!(h.undo(&mut s), Some(Change::Added(b)));
        assert_eq!(s.node(b).unwrap().rect, rect_b);
        assert_eq!(s.edges().len(), 1);
        assert_eq!(h.redo(&mut s), Some(Change::Removed(b)));
        assert!(s.node(b).is_none());
    }

    #[test]
    fn undo_move_and_resize() {
        let (mut s, mut h, a) = board();
        let before = s.node(a).unwrap().rect;
        s.node_mut(a).unwrap().rect = Rect::new(500.0, 50.0, 600.0, 300.0);
        h.moved(&s, a, before);
        assert_eq!(h.undo(&mut s), Some(Change::Moved(a)));
        assert_eq!(s.node(a).unwrap().rect, before);
        h.redo(&mut s);
        assert_eq!(s.node(a).unwrap().rect.x, 500.0);
    }

    #[test]
    fn unchanged_move_is_not_recorded() {
        let (s, mut h, a) = board();
        let before = s.node(a).unwrap().rect;
        let mut h2 = History::new();
        h2.moved(&s, a, before);
        assert!(!h2.can_undo());
        h.moved(&s, a, before);
        assert_eq!(h.undo.len(), 1);
    }

    #[test]
    fn new_action_clears_redo() {
        let (mut s, mut h, a) = board();
        h.undo(&mut s);
        assert!(h.can_redo());
        let b = s.add_page(Rect::default(), "b");
        h.opened(b);
        assert!(!h.can_redo());
        assert!(s.node(a).is_none());
    }

    #[test]
    fn full_round_trip_restores_scene() {
        let (mut s, mut h, a) = board();
        let b = s.spawn_beside(a, "b").unwrap();
        h.opened(b);
        let c = s.spawn_beside(b, "c").unwrap();
        h.opened(c);
        let before = s.node(a).unwrap().rect;
        s.node_mut(a).unwrap().rect.x += 100.0;
        h.moved(&s, a, before);
        let (n, e) = s.remove(b).unwrap();
        h.closed(n, e);
        let snapshot = |s: &Scene| (s.nodes().map(|n| (n.id, n.rect)).collect::<Vec<_>>(), s.edges().to_vec());
        let end = snapshot(&s);
        while h.undo(&mut s).is_some() {}
        assert!(s.is_empty());
        while h.redo(&mut s).is_some() {}
        assert_eq!(snapshot(&s), end);
        assert!(s.topmost_at(Vec2::new(150.0, 10.0)).is_some());
    }

    #[test]
    fn stale_edit_is_skipped() {
        let (mut s, mut h, a) = board();
        let b = s.add_page(Rect::default(), "b");
        h.opened(b);
        // Something outside history removed `b`; undo skips to the next edit.
        s.remove(b);
        assert_eq!(h.undo(&mut s), Some(Change::Removed(a)));
        assert!(!h.can_undo());
    }

    #[test]
    fn history_is_bounded() {
        let mut s = Scene::new();
        let mut h = History::new();
        for _ in 0..LIMIT + 10 {
            let id = s.add_page(Rect::default(), "x");
            h.opened(id);
        }
        assert_eq!(h.undo.len(), LIMIT);
    }
}
