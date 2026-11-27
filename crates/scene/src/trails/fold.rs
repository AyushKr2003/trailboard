//! Fold: a whole trail collapses into a stack of cards. Our own design
//! (Marble's `stacks` module was not decompiled):
//!
//! - A trail here is every card joined to the picked one by edges, either way.
//!   Its members are kept in reading order: from each root (a card with no edge
//!   into it from the trail) down its links, depth first.
//! - Only the stack's *front* card is on the board; the others are hidden (not
//!   drawn, hit, laid out or kept live). The picked card is the front, so the
//!   stack sits where the reader was looking.
//! - Every member remembers its place relative to the trail's first card. The
//!   front's position stands for the stack: moving it moves the stack, peeking
//!   puts the next card there, and unfolding lays the trail out again around
//!   the front exactly as it was before the fold (shifted if the stack moved).
//! - Folding a trail that already holds a stack (links opened from a stack's
//!   front) unfolds that stack first, so one trail is one stack.
//! - A trail whose cards have different colours (a coloured trail joined
//!   it, `join`) folds one colour part at a time: S folds the cards of the
//!   current card's colour, and the other parts stay on the board, their
//!   lines running from the stack (`Scene::shown_edges`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{NodeId, Scene, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub id: NodeId,
    /// Content position relative to the first member's, when the trail was folded.
    pub offset: Vec2,
}

/// A folded trail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    /// In reading order; at least two.
    pub members: Vec<Member>,
    /// The member shown on the board.
    pub front: NodeId,
    /// Where the stack sits (the front's position) relative to the first
    /// member's place: the offset of the card that was in front when folded.
    pub anchor: Vec2,
}

impl Stack {
    pub fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.members.iter().map(|m| m.id)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.members.iter().any(|m| m.id == id)
    }

    /// Index of the front card in `members`.
    pub fn front_index(&self) -> usize {
        self.members.iter().position(|m| m.id == self.front).unwrap_or(0)
    }

}

/// What [`Scene::fold`] did, so it can be undone exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct Folded {
    /// The new stack's members, in reading order, and its front.
    pub members: Vec<NodeId>,
    pub front: NodeId,
    /// Stacks inside the trail that were unfolded first.
    pub unfolded: Vec<Stack>,
}

impl Scene {
    pub fn stacks(&self) -> &[Stack] {
        &self.stacks
    }

    pub fn stack_of(&self, id: NodeId) -> Option<&Stack> {
        self.stacks.iter().find(|s| s.contains(id))
    }

    /// True for a card folded away behind its stack's front.
    pub fn is_hidden(&self, id: NodeId) -> bool {
        self.stack_of(id).is_some_and(|s| s.front != id)
    }

    /// Every card joined to `id` by edges (either way, hidden ones too), in
    /// reading order: roots first, each followed by its links depth first.
    /// Just `[id]` for a lone card; empty if `id` doesn't exist.
    pub fn trail_of(&self, id: NodeId) -> Vec<NodeId> {
        self.trail_tree(id).into_iter().map(|(n, _)| n).collect()
    }

    /// [`Scene::trail_of`] with each card's depth: 0 for a root (and for the
    /// first card reached in a cycle), else one more than the card it was
    /// opened from.
    pub fn trail_tree(&self, id: NodeId) -> Vec<(NodeId, usize)> {
        if self.node(id).is_none() {
            return Vec::new();
        }
        let live = |e: &&crate::Edge| e.from != e.to && self.node(e.from).is_some() && self.node(e.to).is_some();
        let mut set = BTreeSet::from([id]);
        let mut queue = VecDeque::from([id]);
        while let Some(n) = queue.pop_front() {
            for e in self.edges.iter().filter(live) {
                let other = if e.from == n { e.to } else if e.to == n { e.from } else { continue };
                if set.insert(other) {
                    queue.push_back(other);
                }
            }
        }
        // Children in the order their edges were made; first edge into a card wins.
        let mut children: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        let mut has_parent = BTreeSet::new();
        for e in self.edges.iter().filter(live).filter(|e| set.contains(&e.from)) {
            if has_parent.insert(e.to) {
                children.entry(e.from).or_default().push(e.to);
            }
        }
        let mut order = Vec::with_capacity(set.len());
        let mut seen = BTreeSet::new();
        // Roots first, then (for cycles) whatever is left, lowest id first.
        let starts: Vec<NodeId> =
            set.iter().copied().filter(|n| !has_parent.contains(n)).chain(set.iter().copied()).collect();
        for start in starts {
            let mut todo = vec![(start, 0)];
            while let Some((n, depth)) = todo.pop() {
                if !seen.insert(n) {
                    continue;
                }
                order.push((n, depth));
                if let Some(kids) = children.get(&n) {
                    todo.extend(kids.iter().rev().map(|&k| (k, depth + 1)));
                }
            }
        }
        order
    }

    /// S: folds the colour part of `id` (its trail's cards of its colour,
    /// [`Scene::colour_part`]; the whole trail when it has one colour) into a
    /// stack with `id` in front, so cards of another colour that joined the
    /// trail stay out (owner, 2026-11-25). Stacks holding any of those cards
    /// are unfolded first. `None` for a part of one card or a card that
    /// doesn't exist.
    pub fn fold(&mut self, id: NodeId) -> Option<Folded> {
        let trail = self.colour_part(id);
        if trail.len() < 2 {
            return None;
        }
        let fronts: Vec<NodeId> = self.stacks.iter().filter(|s| s.ids().any(|m| trail.contains(&m))).map(|s| s.front).collect();
        let unfolded: Vec<Stack> = fronts.into_iter().filter_map(|f| self.unfold(f)).collect();
        self.fold_members(&trail, id).then_some(Folded { members: trail, front: id, unfolded })
    }

    /// Folds exactly these cards (in this order) into a stack behind `front`,
    /// remembering their current places. Returns false, changing nothing, if
    /// fewer than two of them exist, any is already in a stack, or `front`
    /// isn't one of them.
    pub fn fold_members(&mut self, members: &[NodeId], front: NodeId) -> bool {
        let ids: Vec<NodeId> = members.iter().copied().filter(|id| self.node(*id).is_some()).collect();
        if ids.len() < 2 || !ids.contains(&front) || ids.iter().any(|id| self.stack_of(*id).is_some()) {
            return false;
        }
        let origin = self.nodes[&ids[0]].rect.pos();
        let members: Vec<Member> =
            ids.iter().map(|&id| Member { id, offset: self.nodes[&id].rect.pos() - origin }).collect();
        let anchor = self.nodes[&front].rect.pos() - origin;
        self.push_stack(Stack { members, front, anchor }, origin);
        true
    }

    /// Folds an unfolded `stack` again as it was (same front, same place
    /// relative to its members), for undo. Members closed since are left out.
    /// Returns false, changing nothing, if fewer than two remain or any is
    /// already in a stack.
    pub fn refold(&mut self, mut stack: Stack) -> bool {
        stack.members.retain(|m| self.nodes.contains_key(&m.id));
        if stack.members.len() < 2 || stack.members.iter().any(|m| self.stack_of(m.id).is_some()) {
            return false;
        }
        if !stack.contains(stack.front) {
            stack.front = stack.members[0].id;
        }
        let first = stack.members[0];
        let origin = self.nodes[&first.id].rect.pos() - first.offset;
        self.push_stack(stack, origin);
        true
    }

    /// Moves every member to the stack's place and records the stack.
    fn push_stack(&mut self, stack: Stack, origin: Vec2) {
        let at = origin + stack.anchor;
        for m in &stack.members {
            self.nodes.get_mut(&m.id).expect("member exists").rect.set_pos(at);
        }
        let front = stack.front;
        self.stacks.push(stack);
        self.bring_to_front(front);
    }

    /// The trail edges to draw: every edge, with an end folded away behind a
    /// stack's front moved to that front, so a line into a folded part
    /// runs from its stack. Edges inside one stack, and repeats, are left out.
    pub fn shown_edges(&self) -> Vec<crate::Edge> {
        let shown = |id: NodeId| self.stack_of(id).filter(|s| s.front != id).map_or(id, |s| s.front);
        let mut out: Vec<crate::Edge> = Vec::new();
        for e in &self.edges {
            if self.node(e.from).is_none() || self.node(e.to).is_none() {
                continue;
            }
            let e = crate::Edge::new(shown(e.from), shown(e.to));
            if e.from != e.to && !out.contains(&e) {
                out.push(e);
            }
        }
        out
    }

    /// Unfolds the stack holding `id`: every member goes back to its place
    /// relative to the front. Returns the stack as it was.
    pub fn unfold(&mut self, id: NodeId) -> Option<Stack> {
        let i = self.stacks.iter().position(|s| s.contains(id))?;
        let stack = self.stacks.remove(i);
        let at = self.nodes[&stack.front].rect.pos() - stack.anchor;
        for m in &stack.members {
            if let Some(n) = self.nodes.get_mut(&m.id) {
                n.rect.set_pos(at + m.offset);
            }
        }
        Some(stack)
    }

    /// Peek: shows the member `step` places after the front (wrapping; negative
    /// goes back) in its place. Returns the new front.
    pub fn peek(&mut self, id: NodeId, step: isize) -> Option<NodeId> {
        let i = self.stacks.iter().position(|s| s.contains(id))?;
        let s = &self.stacks[i];
        let len = s.members.len() as isize;
        let next = s.members[(s.front_index() as isize + step).rem_euclid(len) as usize].id;
        self.set_front(i, next);
        Some(next)
    }

    /// Outline: brings the member `id` to its stack's front, in the front's
    /// place. Returns false for a card in no stack; true (changing nothing) if
    /// it is already the front.
    pub fn show(&mut self, id: NodeId) -> bool {
        let Some(i) = self.stacks.iter().position(|s| s.contains(id)) else { return false };
        if self.stacks[i].front != id {
            self.set_front(i, id);
        }
        true
    }

    fn set_front(&mut self, stack: usize, id: NodeId) {
        let old = self.stacks[stack].front;
        if let Some(at) = self.nodes.get(&old).map(|n| n.rect.pos())
            && let Some(n) = self.nodes.get_mut(&id)
        {
            n.rect.set_pos(at);
        }
        self.stacks[stack].front = id;
        self.bring_to_front(id);
    }

    /// Takes a removed card out of its stack: the next member comes to the
    /// front in its place, and a stack left with one card is dissolved.
    pub(crate) fn forget_in_stacks(&mut self, id: NodeId, at: Vec2) {
        let Some(i) = self.stacks.iter().position(|s| s.contains(id)) else { return };
        let s = &mut self.stacks[i];
        let k = s.members.iter().position(|m| m.id == id).expect("member");
        s.members.remove(k);
        if s.members.len() < 2 {
            self.stacks.remove(i);
            return;
        }
        if s.front == id {
            let next = s.members[k.min(s.members.len() - 1)].id;
            s.front = next;
            if let Some(n) = self.nodes.get_mut(&next) {
                n.rect.set_pos(at);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Rect, Scene, Vec2};

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

    fn places(s: &Scene) -> Vec<(crate::NodeId, Rect)> {
        s.all_nodes().map(|n| (n.id, n.rect)).collect()
    }

    #[test]
    fn trail_is_in_reading_order() {
        let (s, [a, b, c, d, e]) = board();
        assert_eq!(s.trail_of(d), vec![a, b, c, d]);
        assert_eq!(s.trail_of(e), vec![e]);
    }

    #[test]
    fn fold_hides_all_but_the_front_where_it_was() {
        let (mut s, [a, b, c, d, e]) = board();
        let c_at = s.node(c).unwrap().rect;
        let f = s.fold(c).unwrap();
        assert_eq!((f.members.clone(), f.front), (vec![a, b, c, d], c));
        assert!(!s.is_hidden(c) && s.is_hidden(a) && s.is_hidden(b) && s.is_hidden(d) && !s.is_hidden(e));
        assert_eq!(s.node(c).unwrap().rect, c_at);
        let visible: Vec<_> = s.nodes().map(|n| n.id).collect();
        assert_eq!(visible, vec![c, e]);
        assert!(s.visible_edges().next().is_none());
        assert_eq!(s.topmost_at(Vec2::new(c_at.x + 1.0, c_at.y + 1.0)), Some(c));
        assert!(s.fold(e).is_none(), "a lone card has no trail");
    }

    #[test]
    fn a_trail_of_two_colours_folds_one_colour_at_a_time() {
        let (mut s, [a, b, c, d, e]) = board();
        // e → x (a trail of its own, green) joins d keeping its colour: the
        // trail is blue then green.
        let x = s.spawn_beside(e, "x").unwrap();
        s.join_keeping(e, d).unwrap();
        assert_eq!(s.trail_of(a), vec![a, b, c, d, e, x]);
        let blue = s.fold(b).unwrap();
        assert_eq!(blue.members, vec![a, b, c, d]);
        assert!(!s.is_hidden(e) && !s.is_hidden(x), "the green part stays out");
        // The line d → e now runs from the stack's front.
        assert!(s.shown_edges().contains(&crate::Edge::new(b, e)));
        assert!(s.shown_edges().iter().all(|e| !s.is_hidden(e.from) && !s.is_hidden(e.to)));
        // The green part folds on its own, leaving the blue stack as it is.
        let green = s.fold(x).unwrap();
        assert_eq!((green.members, green.unfolded.len()), (vec![e, x], 0));
        assert_eq!(s.stacks().len(), 2);
        assert_eq!(s.shown_edges(), vec![crate::Edge::new(b, x)]);
        // Unfolding one leaves the other.
        s.unfold(b).unwrap();
        assert_eq!(s.stacks().len(), 1);
        assert!(s.is_hidden(e));
    }

    #[test]
    fn unfold_restores_places_shifted_with_the_stack() {
        let (mut s, [_, b, c, _, _]) = board();
        let before = places(&s);
        s.fold(b).unwrap();
        s.unfold(b).unwrap();
        assert_eq!(places(&s), before);
        assert!(s.stacks().is_empty());

        // Move the stack, peek, then unfold: everything shifts by the move.
        s.fold(b).unwrap();
        let moved = Vec2::new(500.0, -100.0);
        let r = s.node(b).unwrap().rect;
        s.node_mut(b).unwrap().rect.set_pos(r.pos() + moved);
        assert_eq!(s.peek(b, 1), Some(c));
        assert_eq!(s.node(c).unwrap().rect.pos(), r.pos() + moved);
        s.unfold(c).unwrap();
        for ((id, now), (_, was)) in places(&s).into_iter().zip(before) {
            if id.0 <= 4 {
                assert_eq!(now.pos(), was.pos() + moved, "card {id:?}");
            }
        }
    }

    #[test]
    fn peek_wraps_both_ways() {
        let (mut s, [a, b, c, d, _]) = board();
        s.fold(a).unwrap();
        assert_eq!(s.peek(a, -1), Some(d));
        assert_eq!(s.peek(d, 1), Some(a));
        assert_eq!(s.peek(a, 2), Some(c));
        assert_eq!(s.stack_of(b).unwrap().front, c);
    }

    #[test]
    fn show_brings_any_member_forward_in_place() {
        let (mut s, [a, b, c, d, e]) = board();
        s.fold(b).unwrap();
        let at = s.node(b).unwrap().rect.pos();
        assert!(s.show(d));
        assert_eq!(s.stack_of(a).unwrap().front, d);
        assert_eq!(s.node(d).unwrap().rect.pos(), at);
        assert!(s.is_hidden(b) && !s.is_hidden(d));
        assert_eq!(s.topmost_at(at + Vec2::new(1.0, 1.0)), Some(d));
        assert!(s.show(d), "already in front");
        assert_eq!(s.stack_of(c).unwrap().front, d);
        assert!(!s.show(e), "not in a stack");
    }

    #[test]
    fn folding_again_takes_in_an_older_stack() {
        let (mut s, [a, b, c, d, _]) = board();
        s.fold(a).unwrap();
        let x = s.spawn_beside(a, "x").unwrap();
        let f = s.fold(x).unwrap();
        assert_eq!(f.members, vec![a, b, c, d, x]);
        assert_eq!(f.unfolded.len(), 1);
        assert_eq!(s.stacks().len(), 1);
    }

    #[test]
    fn removing_members_moves_the_front_and_dissolves() {
        let (mut s, [a, b, c, _, _]) = board();
        // Just a and b in a stack.
        assert!(s.fold_members(&[a, b], a));
        let at = s.node(a).unwrap().rect.pos();
        s.remove(a);
        assert!(s.stacks().is_empty(), "one card left is no stack");
        assert_eq!(s.node(b).unwrap().rect.pos(), at);
        assert!(!s.fold_members(&[b, c], a), "front must be a member");
    }

    #[test]
    fn stacks_round_trip_through_json() {
        let (mut s, [_, b, ..]) = board();
        s.fold(b).unwrap();
        let back: Scene = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.stacks(), s.stacks());
        // Boards from before stacks still load.
        let mut v = serde_json::to_value(&s).unwrap();
        v.as_object_mut().unwrap().remove("stacks");
        let old: Scene = serde_json::from_value(v).unwrap();
        assert!(old.stacks().is_empty());
    }
}
