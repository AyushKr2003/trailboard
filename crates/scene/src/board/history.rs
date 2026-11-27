//! Undo/redo for board edits: opening, closing, moving and resizing cards,
//! folding or unfolding trails, naming or colouring them, and joining cards
//! to trails or cutting them out. Peeking through a stack is not an edit.
//!
//! Each stack holds the edit that reverses an action. Applying an edit to the
//! scene returns its own reverse, which goes on the other stack, so undo and
//! redo share one code path. Page navigation, titles and stacking order are not
//! board edits and are not recorded.

use crate::{Edge, Folded, Node, NodeId, Rect, Relinked, Scene, Stack, TrailMark};

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
    /// Fold an unfolded stack again, as it was.
    Refold(Stack),
    /// Unfold the stack holding this card.
    Unfold(NodeId),
    /// Give these cards these trail marks (names and colours).
    SetMarks(Vec<(NodeId, TrailMark)>),
    /// Make a note a sticky note in this colour, or (`None`) a note card again.
    SetSticky { id: NodeId, colour: Option<String> },
    /// Make the trail edges exactly these (a join or leave).
    SetEdges(Vec<Edge>),
    /// Make the links exactly these (`link`).
    SetLinks(Vec<Edge>),
    /// Several edits undone and redone as one step, applied in order.
    Batch(Vec<Edit>),
}

/// What an undo or redo did, so the app can start or stop the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added(NodeId),
    Removed(NodeId),
    Moved(NodeId),
    /// A note became a sticky note, changed colour, or became a note card again.
    Restyled(NodeId),
}

impl Edit {
    /// Applies the edit; returns its reverse and what changed, or `None` if it
    /// no longer applies (its card is gone or its id is taken). A batch applies
    /// the parts that still apply and is stale only if none do.
    fn apply(self, scene: &mut Scene) -> Option<(Edit, Vec<Change>)> {
        match self {
            Edit::SetRect { id, rect } => {
                let n = scene.node_mut(id)?;
                let old = std::mem::replace(&mut n.rect, rect);
                Some((Edit::SetRect { id, rect: old }, vec![Change::Moved(id)]))
            }
            Edit::Insert { node, edges } => {
                let id = node.id;
                scene.insert(node, &edges).then(|| (Edit::Remove(id), vec![Change::Added(id)]))
            }
            Edit::Remove(id) => {
                let (node, edges) = scene.remove(id)?;
                Some((Edit::Insert { node, edges }, vec![Change::Removed(id)]))
            }
            Edit::SetMarks(marks) => {
                let mut old = Vec::new();
                for (id, mark) in marks {
                    if let Some(n) = scene.node_mut(id) {
                        old.push((id, std::mem::replace(&mut n.trail, mark)));
                    }
                }
                let first = old.first()?.0;
                Some((Edit::SetMarks(old), vec![Change::Moved(first)]))
            }
            Edit::SetEdges(edges) => {
                let old = std::mem::replace(&mut scene.edges, edges);
                let ends: std::collections::BTreeSet<NodeId> = old
                    .iter()
                    .filter(|e| !scene.edges.contains(e))
                    .chain(scene.edges.iter().filter(|e| !old.contains(e)))
                    .flat_map(|e| [e.from, e.to])
                    .filter(|&id| scene.node(id).is_some())
                    .collect();
                Some((Edit::SetEdges(old), ends.into_iter().map(Change::Moved).collect()))
            }
            Edit::SetLinks(links) => {
                let old = std::mem::replace(&mut scene.links, links);
                let ends: std::collections::BTreeSet<NodeId> = old
                    .iter()
                    .filter(|e| !scene.links.contains(e))
                    .chain(scene.links.iter().filter(|e| !old.contains(e)))
                    .flat_map(|e| [e.from, e.to])
                    .filter(|&id| scene.node(id).is_some())
                    .collect();
                Some((Edit::SetLinks(old), ends.into_iter().map(Change::Moved).collect()))
            }
            Edit::SetSticky { id, colour } => {
                let old = scene.set_sticky(id, colour)?;
                Some((Edit::SetSticky { id, colour: old }, vec![Change::Restyled(id)]))
            }
            Edit::Refold(stack) => {
                let front = stack.front;
                scene.refold(stack).then(|| {
                    let front = scene.stack_of(front).map_or(front, |s| s.front);
                    (Edit::Unfold(front), vec![Change::Moved(front)])
                })
            }
            Edit::Unfold(id) => {
                let stack = scene.unfold(id)?;
                let front = stack.front;
                Some((Edit::Refold(stack), vec![Change::Moved(front)]))
            }
            Edit::Batch(edits) => {
                let mut reverse = Vec::new();
                let mut changes = Vec::new();
                for e in edits {
                    if let Some((r, c)) = e.apply(scene) {
                        reverse.push(r);
                        changes.extend(c);
                    }
                }
                // Undo the parts in the opposite order (a card restored later may
                // carry the trail edge to one restored earlier).
                reverse.reverse();
                (!changes.is_empty()).then_some((Edit::Batch(reverse), changes))
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
        self.moved_many(scene, &[(id, before)]);
    }

    /// Several cards were moved together from their `before` rects: one step.
    pub fn moved_many(&mut self, scene: &Scene, before: &[(NodeId, Rect)]) {
        let edits: Vec<Edit> = before
            .iter()
            .filter(|(id, r)| scene.node(*id).is_some_and(|n| n.rect != *r))
            .map(|&(id, rect)| Edit::SetRect { id, rect })
            .collect();
        self.push_group(edits);
    }

    /// Several cards were closed together, in this order; each pair is what
    /// [`Scene::remove`] returned. One step.
    pub fn closed_many(&mut self, closed: Vec<(Node, Vec<Edge>)>) {
        // Restore in reverse: an edge between two closed cards went with the first.
        let edits = closed.into_iter().rev().map(|(node, edges)| Edit::Insert { node, edges }).collect();
        self.push_group(edits);
    }

    /// A trail was folded ([`Scene::fold`]). Undo unfolds it and folds again
    /// any stack the fold took in.
    pub fn folded(&mut self, f: &Folded) {
        let mut edits = vec![Edit::Unfold(f.front)];
        edits.extend(f.unfolded.iter().cloned().map(Edit::Refold));
        self.push_group(edits);
    }

    /// Cards joined a trail or left one; `r` is what [`Scene::join_many`] or
    /// [`Scene::leave_many`] returned. Undo puts back the edges, marks and
    /// places, then folds again any stack it unfolded.
    pub fn relinked(&mut self, r: &Relinked) {
        let mut edits = vec![Edit::SetEdges(r.edges.clone())];
        if !r.marks.is_empty() {
            edits.push(Edit::SetMarks(r.marks.clone()));
        }
        edits.extend(r.moved.iter().map(|&(id, rect)| Edit::SetRect { id, rect }));
        edits.extend(r.unfolded.iter().cloned().map(Edit::Refold));
        self.push_group(edits);
    }

    /// Cards were linked or unlinked; `old` is the links as they were
    /// ([`Scene::link_many`], [`Scene::unlink`]).
    pub fn linked(&mut self, old: Vec<Edge>) {
        self.push(Edit::SetLinks(old));
    }

    /// A stack was unfolded; `stack` is what [`Scene::unfold`] returned.
    pub fn unfolded(&mut self, stack: &Stack) {
        self.push(Edit::Refold(stack.clone()));
    }

    /// A trail was named or recoloured; `old` is what [`Scene::set_trail_mark`]
    /// returned. No-op if nothing changed.
    pub fn marked(&mut self, old: Vec<(NodeId, TrailMark)>) {
        if !old.is_empty() {
            self.push(Edit::SetMarks(old));
        }
    }

    /// A note was made sticky, recoloured or made a card again; `old` is what
    /// [`Scene::set_sticky`] returned. No-op for `None` (nothing changed).
    pub fn restyled(&mut self, id: NodeId, old: Option<Option<String>>) {
        if let Some(colour) = old {
            self.push(Edit::SetSticky { id, colour });
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undoes the newest action; returns what changed (empty if nothing to undo).
    pub fn undo(&mut self, scene: &mut Scene) -> Vec<Change> {
        Self::step(&mut self.undo, &mut self.redo, scene)
    }

    pub fn redo(&mut self, scene: &mut Scene) -> Vec<Change> {
        Self::step(&mut self.redo, &mut self.undo, scene)
    }

    /// Records one edit as is, several as a batch, none not at all.
    fn push_group(&mut self, mut edits: Vec<Edit>) {
        match edits.len() {
            0 => {}
            1 => self.push(edits.pop().expect("one edit")),
            _ => self.push(Edit::Batch(edits)),
        }
    }

    fn push(&mut self, e: Edit) {
        self.redo.clear();
        self.undo.push(e);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    /// Applies the newest edit that still applies; stale ones are dropped.
    fn step(from: &mut Vec<Edit>, to: &mut Vec<Edit>, scene: &mut Scene) -> Vec<Change> {
        while let Some(e) = from.pop() {
            if let Some((reverse, changes)) = e.apply(scene) {
                to.push(reverse);
                return changes;
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec2;

    #[test]
    fn sticky_changes_undo_and_redo() {
        let mut s = Scene::new();
        let mut h = History::new();
        let id = s.add_note(Rect::new(0.0, 0.0, 100.0, 100.0), "x");
        h.restyled(id, s.set_sticky(id, Some("yellow".into())));
        h.restyled(id, s.cycle_sticky_colour(id));
        h.restyled(id, s.set_sticky(id, Some("green".into()))); // already green: nothing recorded
        assert_eq!(s.node(id).unwrap().sticky.as_deref(), Some("green"));
        assert_eq!(h.undo(&mut s), vec![Change::Restyled(id)]);
        assert_eq!(s.node(id).unwrap().sticky.as_deref(), Some("yellow"));
        h.undo(&mut s);
        assert!(!s.node(id).unwrap().is_sticky());
        assert!(!h.can_undo());
        h.redo(&mut s);
        h.redo(&mut s);
        assert_eq!(s.node(id).unwrap().sticky.as_deref(), Some("green"));
    }

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
        assert_eq!(h.undo(&mut s), vec![Change::Removed(b)]);
        assert!(s.node(b).is_none() && s.edges().is_empty());
        assert_eq!(h.redo(&mut s), vec![Change::Added(b)]);
        assert_eq!(s.node(b).unwrap().url, "https://b");
        assert_eq!(s.edges(), &[Edge::new(a, b)]);
    }

    #[test]
    fn undo_close_restores_card_and_trail() {
        let (mut s, mut h, a) = board();
        let b = s.spawn_beside(a, "https://b").unwrap();
        h.opened(b);
        let rect_b = s.node(b).unwrap().rect;
        let (node, edges) = s.remove(b).unwrap();
        h.closed(node, edges);
        assert_eq!(h.undo(&mut s), vec![Change::Added(b)]);
        assert_eq!(s.node(b).unwrap().rect, rect_b);
        assert_eq!(s.edges().len(), 1);
        assert_eq!(h.redo(&mut s), vec![Change::Removed(b)]);
        assert!(s.node(b).is_none());
    }

    #[test]
    fn undo_move_and_resize() {
        let (mut s, mut h, a) = board();
        let before = s.node(a).unwrap().rect;
        s.node_mut(a).unwrap().rect = Rect::new(500.0, 50.0, 600.0, 300.0);
        h.moved(&s, a, before);
        assert_eq!(h.undo(&mut s), vec![Change::Moved(a)]);
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
        while !h.undo(&mut s).is_empty() {}
        assert!(s.is_empty());
        while !h.redo(&mut s).is_empty() {}
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
        assert_eq!(h.undo(&mut s), vec![Change::Removed(a)]);
        assert!(!h.can_undo());
    }

    #[test]
    fn group_move_is_one_step() {
        let (mut s, mut h, a) = board();
        let b = s.add_page(Rect::new(600.0, 0.0, 100.0, 100.0), "b");
        let c = s.add_page(Rect::new(900.0, 0.0, 100.0, 100.0), "c");
        let before: Vec<_> = [a, b, c].iter().map(|&id| (id, s.node(id).unwrap().rect)).collect();
        // a and b move, c stays put and is not recorded.
        for id in [a, b] {
            s.node_mut(id).unwrap().rect.y += 50.0;
        }
        h.moved_many(&s, &before);
        assert_eq!(h.undo.len(), 2, "open + one batch");
        assert_eq!(h.undo(&mut s), vec![Change::Moved(a), Change::Moved(b)]);
        assert_eq!(s.node(a).unwrap().rect.y, 0.0);
        assert_eq!(s.node(b).unwrap().rect.y, 0.0);
        h.redo(&mut s);
        assert_eq!(s.node(b).unwrap().rect.y, 50.0);
        // A single moved card is recorded as a plain edit.
        let mut h2 = History::new();
        let before = s.node(c).unwrap().rect;
        s.node_mut(c).unwrap().rect.x += 1.0;
        h2.moved_many(&s, &[(c, before)]);
        assert!(matches!(h2.undo.last(), Some(Edit::SetRect { .. })));
    }

    #[test]
    fn group_close_restores_edges_between_closed_cards() {
        let (mut s, mut h, a) = board();
        let b = s.spawn_beside(a, "b").unwrap();
        let c = s.spawn_beside(b, "c").unwrap();
        let closed: Vec<_> = [b, c].iter().map(|&id| s.remove(id).unwrap()).collect();
        h.closed_many(closed);
        assert!(s.edges().is_empty());
        let mut undone = h.undo(&mut s);
        undone.sort_by_key(|c| format!("{c:?}"));
        assert_eq!(undone, vec![Change::Added(b), Change::Added(c)]);
        let mut edges = s.edges().to_vec();
        edges.sort_by_key(|e| (e.from, e.to));
        assert_eq!(edges, vec![Edge::new(a, b), Edge::new(b, c)]);
        let redone = h.redo(&mut s);
        assert_eq!(redone.len(), 2);
        assert!(s.node(b).is_none() && s.node(c).is_none() && s.edges().is_empty());
    }

    #[test]
    fn batch_applies_what_still_applies() {
        let (mut s, mut h, a) = board();
        let b = s.add_page(Rect::new(600.0, 0.0, 100.0, 100.0), "b");
        let before = vec![(a, s.node(a).unwrap().rect), (b, s.node(b).unwrap().rect)];
        s.node_mut(a).unwrap().rect.x += 5.0;
        s.node_mut(b).unwrap().rect.x += 5.0;
        h.moved_many(&s, &before);
        s.remove(b);
        assert_eq!(h.undo(&mut s), vec![Change::Moved(a)]);
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

    #[test]
    fn fold_and_unfold_undo_and_redo() {
        let (mut s, mut h, a) = board();
        let b = s.spawn_beside(a, "b").unwrap();
        let snapshot = |s: &Scene| s.all_nodes().map(|n| (n.id, n.rect)).collect::<Vec<_>>();
        let before = snapshot(&s);
        let f = s.fold(b).unwrap();
        h.folded(&f);
        assert_eq!(h.undo(&mut s), vec![Change::Moved(b)]);
        assert!(s.stacks().is_empty());
        assert_eq!(snapshot(&s), before);
        h.redo(&mut s);
        assert!(s.is_hidden(a));
        let stack = s.unfold(a).unwrap();
        h.unfolded(&stack);
        h.undo(&mut s);
        assert_eq!(s.stack_of(a).map(|st| st.front), Some(b));

        // Peek, unfold, undo: the stack comes back exactly where it was.
        s.peek(b, 1);
        let peeked = snapshot(&s);
        let stack = s.unfold(a).unwrap();
        h.unfolded(&stack);
        h.undo(&mut s);
        assert_eq!(snapshot(&s), peeked);
        assert_eq!(s.stack_of(b).map(|st| st.front), Some(a));
    }
}
