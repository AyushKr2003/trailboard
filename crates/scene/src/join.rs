//! Join and leave: any card can be moved into another trail, or cut out of
//! its own, so a named, coloured trail can be a group of any cards. Our own
//! design (M15 in `docs/PLAN.md` has the decisions):
//!
//! - A trail is every card its edges connect, so a card that just gained a
//!   second parent would merge both whole trails. Joining therefore *moves*
//!   a card: its edge from its parent is cut, then it gets one from the
//!   target, as if opened from it. Its followers (the cards opened from it,
//!   and so on) come along; the rest of its old trail stays.
//! - Joining a card to itself or to one of its followers would make a loop
//!   and is refused.
//! - The joined card goes where a link opened from the target would, and its
//!   followers move by the same offset, keeping their shape.
//! - The merged trail takes the target trail's name, colour, pin and place,
//!   each where it has one, else the joining trail's (and a fresh colour if
//!   neither has one). A card that had a parent brings no mark (it left its
//!   old trail's); one that leaves loses its mark, with its followers, and
//!   if it has followers takes a fresh colour as a trail of its own.
//! - A card in a stack, or a target in one, is unfolded first.
//! - Everything one call does is one undo step ([`crate::History::relinked`]).

use std::collections::{BTreeSet, VecDeque};

use crate::{placement, Edge, NodeId, Rect, Scene, Stack, TrailMark};

/// Why a join or leave changed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelinkError {
    /// The target, or every card, isn't on the board.
    Missing,
    /// The target is a joining card or one of its followers.
    Loop,
    /// Every card is already joined to the target, or has no parent to leave.
    Unchanged,
}

/// What a join or leave did, so it can be undone exactly and shown.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Relinked {
    /// The cards that joined or left, in the order they did.
    pub cards: Vec<NodeId>,
    /// Every trail edge as it was before.
    pub edges: Vec<Edge>,
    /// The old mark of each card whose mark changed.
    pub marks: Vec<(NodeId, TrailMark)>,
    /// The old content rect of each card that moved (after any unfold).
    pub moved: Vec<(NodeId, Rect)>,
    /// Stacks unfolded first, as they were.
    pub unfolded: Vec<Stack>,
}

impl Scene {
    /// Every card's mark, to tell later what a join or leave changed.
    fn marks(&self) -> Vec<(NodeId, TrailMark)> {
        self.nodes.values().map(|n| (n.id, n.trail.clone())).collect()
    }

    /// Of `before` ([`Scene::marks`]), the cards whose mark changed since,
    /// with their old mark (to undo).
    fn changed_marks(&self, before: &[(NodeId, TrailMark)]) -> Vec<(NodeId, TrailMark)> {
        before.iter().filter(|(id, old)| self.nodes.get(id).is_some_and(|n| n.trail != *old)).cloned().collect()
    }

    /// The card `id` was opened from: the first edge into it (as
    /// [`Scene::trail_tree`] reads it). `None` for a trail's first card.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.edges.iter().find(|e| e.to == id && e.from != id && self.node(e.from).is_some()).map(|e| e.from)
    }

    /// `id` and every card opened from it, and from those, and so on (edges
    /// followed forwards only; a loop is followed once).
    pub fn followers(&self, id: NodeId) -> Vec<NodeId> {
        if self.node(id).is_none() {
            return Vec::new();
        }
        let mut seen = BTreeSet::from([id]);
        let mut out = vec![id];
        let mut queue = VecDeque::from([id]);
        while let Some(n) = queue.pop_front() {
            for e in self.edges.iter().filter(|e| e.from == n && self.node(e.to).is_some()) {
                if seen.insert(e.to) {
                    out.push(e.to);
                    queue.push_back(e.to);
                }
            }
        }
        out
    }

    /// True if `ancestor` is on the parent chain above `id` (not `id` itself).
    fn is_below(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut seen = BTreeSet::from([id]);
        let mut at = id;
        while let Some(p) = self.parent(at) {
            if p == ancestor {
                return true;
            }
            if !seen.insert(p) {
                return false;
            }
            at = p;
        }
        false
    }

    /// Joins `card` to the trail of `target`, as if opened from it, taking
    /// that trail's colour (a merge). See [`Scene::join_many`].
    pub fn join(&mut self, card: NodeId, target: NodeId) -> Result<Relinked, RelinkError> {
        if card == target && self.node(card).is_some() {
            return Err(RelinkError::Loop);
        }
        self.join_many(&[card], target, false)
    }

    /// [`Scene::join`], but a coloured trail of two or more keeps its colour.
    pub fn join_keeping(&mut self, card: NodeId, target: NodeId) -> Result<Relinked, RelinkError> {
        if card == target && self.node(card).is_some() {
            return Err(RelinkError::Loop);
        }
        self.join_many(&[card], target, true)
    }

    /// Joins each of `cards` to `target`, as if opened from it, bringing its
    /// followers along: one undo step. The target itself, cards that don't
    /// exist and cards below another joining card (they come along anyway)
    /// are left out; cards already joined to `target` stay as they are. The
    /// rest join in reading order of where they are (top to bottom, then left
    /// to right), each placed beside the target in turn.
    ///
    /// Colours (owner, 2026-11-25: two kinds of join): with `keep` false
    /// (a merge) the joining cards take the target trail's colour, so the
    /// trail stays one colour; with `keep` true a joining part of two or
    /// more coloured cards keeps its colour, so the trail shows several
    /// (and S folds one at a time). A single card always takes the
    /// target's. A target on no trail yet takes the first joining trail's
    /// colour, else a fresh one.
    pub fn join_many(&mut self, cards: &[NodeId], target: NodeId, keep: bool) -> Result<Relinked, RelinkError> {
        if self.node(target).is_none() {
            return Err(RelinkError::Missing);
        }
        let all: BTreeSet<NodeId> = cards.iter().copied().filter(|&c| c != target && self.node(c).is_some()).collect();
        if all.is_empty() {
            return Err(RelinkError::Missing);
        }
        let mut cards: Vec<NodeId> = all.iter().copied().filter(|&c| !all.iter().any(|&a| self.is_below(c, a))).collect();
        if cards.iter().any(|&c| self.followers(c).contains(&target)) {
            return Err(RelinkError::Loop);
        }
        cards.retain(|&c| self.parent(c) != Some(target));
        if cards.is_empty() {
            return Err(RelinkError::Unchanged);
        }

        let mut r = Relinked { edges: self.edges.clone(), ..Relinked::default() };
        for &id in cards.iter().chain([&target]) {
            if let Some(stack) = self.unfold(id) {
                r.unfolded.push(stack);
            }
        }
        cards.sort_by(|a, b| {
            let (a, b) = (self.nodes[a].rect, self.nodes[b].rect);
            a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x))
        });

        let before = self.marks();
        // The merged name, pin and place, before any edge changes: the
        // target trail's, filled in from each joining trail's first card.
        let mut mark = self.trail_mark(target);
        for &c in &cards {
            if self.parent(c).is_none() {
                let m = self.trail_mark(c);
                mark.name = mark.name.or(m.name);
                mark.pinned |= m.pinned;
                mark.place = mark.place.or(m.place);
            }
        }
        // Colours: see above.
        let coloured = |s: &Self, c: NodeId| s.followers(c).len() > 1 && s.nodes[&c].trail.colour.is_some();
        if mark.colour.is_none() && self.trail_of(target).len() == 1 {
            let first = cards.iter().find(|&&c| coloured(self, c)).and_then(|c| self.nodes[c].trail.colour.clone());
            mark.colour = Some(first.unwrap_or_else(|| self.fresh_trail_colour()));
        }
        let takers: Vec<NodeId> = cards.iter().copied().filter(|&c| !(keep && coloured(self, c))).collect();
        for &c in &cards {
            self.edges.retain(|e| e.to != c || e.from == c);
            self.edges.push(Edge::new(target, c));
        }
        // Takers join the target's colour part, which then takes the mark's colour.
        let target_colour = self.nodes[&target].trail.colour.clone();
        for c in takers {
            for f in self.followers(c) {
                self.nodes.get_mut(&f).expect("followers exist").trail.colour = target_colour.clone();
            }
        }
        self.set_trail_mark(target, &mark);
        r.marks = self.changed_marks(&before);

        let mut skip: BTreeSet<NodeId> = cards.iter().flat_map(|&c| self.followers(c)).collect();
        for &c in &cards {
            let src = self.nodes[&target].clone();
            let rect = self.nodes[&c].rect;
            let to = placement::beside_skipping(self, &src, rect.w, rect.h, &skip);
            let d = to.pos() - rect.pos();
            for f in self.followers(c) {
                skip.remove(&f);
                let n = self.nodes.get_mut(&f).expect("followers exist");
                if d.x != 0.0 || d.y != 0.0 {
                    r.moved.push((f, n.rect));
                    n.rect = n.rect.translate(d);
                }
            }
        }
        r.cards = cards;
        Ok(r)
    }

    /// The cards joined to `id` by a trail line, either way: its parent(s)
    /// and the cards opened from it, in reading order. Empty for a lone card.
    pub fn neighbours(&self, id: NodeId) -> Vec<NodeId> {
        let next: BTreeSet<NodeId> = self
            .edges
            .iter()
            .filter_map(|e| if e.from == id { Some(e.to) } else if e.to == id { Some(e.from) } else { None })
            .filter(|&n| n != id && self.node(n).is_some())
            .collect();
        self.trail_of(id).into_iter().filter(|n| next.contains(n)).collect()
    }

    /// Cuts the trail line between `a` and `b` (`Shift+A`, owner
    /// 2026-11-25): the card on the far side of the line from the trail's
    /// start leaves, with the cards that follow it ([`Scene::leave_many`]).
    /// So cutting a card from its parent takes it out; cutting it from a
    /// card opened from it takes that one out. `Unchanged` if no line joins
    /// them.
    pub fn cut(&mut self, a: NodeId, b: NodeId) -> Result<Relinked, RelinkError> {
        if self.node(a).is_none() || self.node(b).is_none() {
            return Err(RelinkError::Missing);
        }
        let child = if self.edges.iter().any(|e| e.from == a && e.to == b) {
            b
        } else if self.edges.iter().any(|e| e.from == b && e.to == a) {
            a
        } else {
            return Err(RelinkError::Unchanged);
        };
        self.leave_many(&[child])
    }

    /// Cuts `card` out of its trail. See [`Scene::leave_many`].
    pub fn leave(&mut self, card: NodeId) -> Result<Relinked, RelinkError> {
        self.leave_many(&[card])
    }

    /// Cuts each of `cards` out of its trail: its edges from its parent go,
    /// so it and its followers make a trail of their own, with no name,
    /// colour, pin or place. Cards stay where they are (a stack holding one
    /// is unfolded first). A trail's first card has no parent to leave and is
    /// left out. One undo step.
    pub fn leave_many(&mut self, cards: &[NodeId]) -> Result<Relinked, RelinkError> {
        let all: BTreeSet<NodeId> = cards.iter().copied().filter(|&c| self.node(c).is_some()).collect();
        if all.is_empty() {
            return Err(RelinkError::Missing);
        }
        let cards: Vec<NodeId> = all.into_iter().filter(|&c| self.parent(c).is_some()).collect();
        if cards.is_empty() {
            return Err(RelinkError::Unchanged);
        }
        let mut r = Relinked { edges: self.edges.clone(), ..Relinked::default() };
        for &c in &cards {
            if let Some(stack) = self.unfold(c) {
                r.unfolded.push(stack);
            }
        }
        let before = self.marks();
        // Each leaving card with the colour of the card it was opened from.
        let parents: Vec<(NodeId, Option<String>)> =
            cards.iter().map(|&c| (c, self.parent(c).and_then(|p| self.nodes[&p].trail.colour.clone()))).collect();
        for &c in &cards {
            self.edges.retain(|e| e.to != c || e.from == c);
        }
        // The cards cut out lose the trail's name, pin and place. A part of
        // another colour keeps it (it joined in with it); the rest, if more
        // than one card, takes a fresh colour as a trail of its own.
        for (c, from) in parents {
            let fs = self.followers(c);
            let fresh = (fs.len() > 1 && fs.iter().any(|f| self.nodes[f].trail.colour == from)).then(|| self.fresh_trail_colour());
            for f in fs.iter() {
                let n = self.nodes.get_mut(f).expect("followers exist");
                let colour = if fs.len() == 1 { None } else if n.trail.colour != from { n.trail.colour.clone() } else { fresh.clone() };
                n.trail = TrailMark { colour, ..TrailMark::default() };
            }
        }
        r.marks = self.changed_marks(&before);
        r.cards = cards;
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{placement::GAP, History};

    fn page(s: &mut Scene, x: f32, y: f32, url: &str) -> NodeId {
        s.add_page(Rect::new(x, y, 400.0, 300.0), url)
    }

    /// a → b → c and a → d (named "Work", blue: the first trail's colour);
    /// e → f (green); lone g far away.
    fn board() -> (Scene, [NodeId; 7]) {
        let mut s = Scene::new();
        let a = page(&mut s, 0.0, 0.0, "a");
        let b = s.spawn_beside(a, "b").unwrap();
        let c = s.spawn_beside(b, "c").unwrap();
        let d = s.spawn_beside(a, "d").unwrap();
        s.name_trail(a, "Work");
        let e = page(&mut s, 0.0, 5000.0, "e");
        let f = s.spawn_beside(e, "f").unwrap();
        let g = page(&mut s, 0.0, 10000.0, "g");
        (s, [a, b, c, d, e, f, g])
    }

    /// What a join, leave or undo can change: rects, marks, edges, stacks.
    type Snap = (Vec<(NodeId, Rect, TrailMark)>, Vec<Edge>, Vec<Stack>);

    fn snap(s: &Scene) -> Snap {
        let nodes = s.all_nodes().map(|n| (n.id, n.rect, n.trail.clone())).collect();
        (nodes, s.edges().to_vec(), s.stacks().to_vec())
    }

    fn names(s: &Scene, ids: &[NodeId]) -> Vec<Option<String>> {
        ids.iter().map(|&i| s.node(i).unwrap().trail.name.clone()).collect()
    }

    #[test]
    fn a_lone_card_joins_beside_the_target_and_takes_its_mark() {
        let (mut s, [a, b, c, d, _, _, g]) = board();
        let r = s.join(g, d).unwrap();
        assert_eq!(r.cards, vec![g]);
        assert_eq!(s.parent(g), Some(d));
        assert_eq!(s.trail_of(a), vec![a, b, c, d, g]);
        assert_eq!(s.trail_mark(g).name.as_deref(), Some("Work"));
        assert_eq!(s.trail_mark(g).colour.as_deref(), Some("blue"));
        let (dr, gr) = (s.node(d).unwrap().rect, s.node(g).unwrap().rect);
        assert_eq!(gr.x, dr.right() + GAP);
        assert_eq!(r.moved, vec![(g, Rect::new(0.0, 10000.0, 400.0, 300.0))]);
    }

    #[test]
    fn a_card_with_followers_brings_them_keeping_their_shape() {
        let (mut s, [a, b, c, _, e, f, _]) = board();
        let gap = s.node(f).unwrap().rect.pos() - s.node(e).unwrap().rect.pos();
        s.join(e, c).unwrap();
        assert_eq!(s.trail_of(a).len(), 6);
        assert_eq!(s.followers(e), vec![e, f]);
        assert_eq!(s.node(f).unwrap().rect.pos() - s.node(e).unwrap().rect.pos(), gap);
        assert_eq!(s.parent(b), Some(a), "the target trail is untouched");
    }

    #[test]
    fn a_card_mid_trail_moves_out_with_its_followers_only() {
        let (mut s, [a, b, c, d, e, f, _]) = board();
        s.join_keeping(b, f).unwrap();
        assert_eq!(s.trail_of(a), vec![a, d]);
        assert_eq!(s.trail_of(e), vec![e, f, b, c]);
        // It left "Work" for e's trail: it brings no name, but b and c, two
        // coloured cards, keep their colour as a part of e's trail.
        assert_eq!(s.trail_mark(e), TrailMark { colour: Some("green".into()), ..TrailMark::default() });
        assert_eq!(names(&s, &[b, c]), vec![None, None]);
        assert_eq!(s.trail_mark(b).colour.as_deref(), Some("blue"));
        assert_eq!(s.colour_part(e), vec![e, f]);
        assert_eq!(s.colour_part(b), vec![b, c]);
        // …and "Work" keeps its own.
        assert_eq!(s.trail_mark(a).name.as_deref(), Some("Work"));
    }

    #[test]
    fn a_named_trail_onto_a_lone_card_keeps_its_name_and_the_target_wins_when_both_have_one() {
        let (mut s, [a, .., e, _, g]) = board();
        s.join(a, g).unwrap();
        assert_eq!(s.trail_mark(g).name.as_deref(), Some("Work"));
        assert!(s.trail_of(g).iter().all(|&n| s.node(n).unwrap().trail.name.as_deref() == Some("Work")));
        s.name_trail(e, "Home");
        s.join(g, e).unwrap();
        let m = s.trail_mark(e);
        assert_eq!(m.name.as_deref(), Some("Home"));
        assert_eq!(m.colour.as_deref(), Some("green"), "the target's colour wins");
    }

    #[test]
    fn two_lone_cards_make_a_trail_with_a_colour_of_its_own() {
        let (mut s, [.., g]) = board();
        let x = page(&mut s, 3000.0, 10000.0, "x");
        let mut h = History::new();
        let r = s.join(x, g).unwrap();
        assert_eq!(s.trail_mark(g).colour.as_deref(), Some("magenta"), "blue and green are taken");
        h.relinked(&r);
        h.undo(&mut s);
        assert!(s.trail_mark(g).is_unset() && s.trail_mark(x).is_unset());
        // A single card leaving has no trail to colour.
        s.join(x, g).unwrap();
        s.leave(x).unwrap();
        assert!(s.trail_mark(x).is_unset());
    }

    #[test]
    fn a_merge_gives_the_joining_trail_the_targets_colour() {
        let (mut s, [a, b, c, d, e, f, _]) = board();
        s.join(e, d).unwrap();
        assert_eq!(s.colour_part(a), vec![a, b, c, d, e, f], "one colour");
        assert_eq!(s.trail_mark(f).colour.as_deref(), Some("blue"));
        // The middle of a trail moving by merge takes the new trail's colour too.
        let (mut s, [_, b, c, _, e, f, _]) = board();
        s.join(b, f).unwrap();
        assert_eq!(s.colour_part(e), vec![e, f, b, c]);
        // A lone target takes the joining trail's colour, keeping or not.
        let (mut s, [a, .., g]) = board();
        s.join(a, g).unwrap();
        assert_eq!(s.trail_mark(g).colour.as_deref(), Some("blue"));
    }

    #[test]
    fn joined_trails_keep_their_colours_and_c_recolours_one_part() {
        let (mut s, [a, b, c, d, e, f, g]) = board();
        // e → f (green) joins "Work" (blue), keeping its colour: one trail, two colours.
        s.join_keeping(e, d).unwrap();
        assert_eq!(s.trail_of(a).len(), 6);
        assert_eq!(s.colour_part(a), vec![a, b, c, d]);
        assert_eq!(s.colour_part(f), vec![e, f]);
        assert_eq!(s.trail_mark(e).name.as_deref(), Some("Work"), "the name is the trail's");
        // A single card takes the colour of the card it joins, even keeping.
        s.join_keeping(g, e).unwrap();
        assert_eq!(s.trail_mark(g).colour.as_deref(), Some("green"));
        // C recolours one part only; naming keeps both.
        s.cycle_trail_colour(f);
        assert_eq!(s.trail_mark(e).colour.as_deref(), Some("magenta"));
        assert_eq!(s.trail_mark(b).colour.as_deref(), Some("blue"));
        s.name_trail(f, "Both");
        assert_eq!(s.trail_mark(a).colour.as_deref(), Some("blue"));
        assert_eq!(s.trail_mark(e).name.as_deref(), Some("Both"));
        // The board shows a dot on the green part's first card.
        assert!(s.trail_labels().iter().any(|(id, m)| *id == e && m.name.is_none() && m.colour.as_deref() == Some("magenta")));
        // Leaving takes the part out with its own colour.
        let mut s2 = s.clone();
        s2.leave(e).unwrap();
        assert_eq!(s2.trail_mark(e).colour.as_deref(), Some("magenta"));
        assert_eq!(s2.trail_mark(e).name, None);
        assert_eq!(s2.trail_mark(a).name.as_deref(), Some("Both"));
        // A part of the same colour as the card it leaves gets a fresh colour.
        s2.leave(b).unwrap();
        assert_eq!(s2.trail_mark(b).colour.as_deref(), Some("green"));
        assert_eq!(s2.colour_part(c), vec![b, c]);
    }

    #[test]
    fn loops_are_refused_and_nothing_changes() {
        let (mut s, [a, b, c, ..]) = board();
        let before = snap(&s);
        assert_eq!(s.join(a, c), Err(RelinkError::Loop));
        assert_eq!(s.join(b, b), Err(RelinkError::Loop));
        assert_eq!(s.join(b, a), Err(RelinkError::Unchanged), "already there");
        assert_eq!(s.join(NodeId(999), a), Err(RelinkError::Missing));
        assert_eq!(s.join(a, NodeId(999)), Err(RelinkError::Missing));
        assert_eq!(snap(&s), before);
    }

    #[test]
    fn a_card_moves_within_its_own_trail() {
        let (mut s, [a, b, c, d, ..]) = board();
        s.join(d, c).unwrap();
        assert_eq!(s.parent(d), Some(c));
        assert_eq!(s.trail_of(a), vec![a, b, c, d]);
        assert_eq!(s.trail_mark(d).name.as_deref(), Some("Work"));
    }

    #[test]
    fn leaving_cuts_the_card_and_its_followers_out_in_place() {
        let (mut s, [a, b, c, d, ..]) = board();
        let rects: Vec<Rect> = [b, c].iter().map(|&n| s.node(n).unwrap().rect).collect();
        let r = s.leave(b).unwrap();
        assert_eq!(r.cards, vec![b]);
        assert!(r.moved.is_empty());
        assert_eq!(s.trail_of(a), vec![a, d]);
        assert_eq!(s.trail_of(b), vec![b, c]);
        // A trail of its own, with no name and a colour of its own.
        assert_eq!(s.trail_mark(b), TrailMark { colour: Some("magenta".into()), ..TrailMark::default() });
        assert_eq!(s.trail_mark(a).name.as_deref(), Some("Work"));
        assert_eq!(s.trail_mark(a).colour.as_deref(), Some("blue"));
        assert_eq!(vec![s.node(b).unwrap().rect, s.node(c).unwrap().rect], rects);
        assert_eq!(s.leave(a), Err(RelinkError::Unchanged), "the first card has no parent");
        assert_eq!(s.leave(NodeId(999)), Err(RelinkError::Missing));
    }

    #[test]
    fn stacks_unfold_first_and_fold_again_on_undo() {
        let (mut s, [a, b, _, _, _, _, g]) = board();
        let mut h = History::new();
        let f = s.fold(a).unwrap();
        h.folded(&f);
        let folded = snap(&s);
        let r = s.join(g, b).unwrap();
        assert_eq!(r.unfolded.len(), 1);
        assert!(s.stacks().is_empty());
        assert_eq!(s.parent(g), Some(b));
        h.relinked(&r);
        h.undo(&mut s);
        assert_eq!(snap(&s), folded);
        h.redo(&mut s);
        assert!(s.stacks().is_empty());
        assert_eq!(s.parent(g), Some(b));
    }

    #[test]
    fn join_and_leave_undo_and_redo_as_one_step() {
        let (mut s, [a, b, _, d, e, _, g]) = board();
        let start = snap(&s);
        let mut h = History::new();
        let r = s.join(e, d).unwrap();
        h.relinked(&r);
        let joined = snap(&s);
        let r = s.leave(b).unwrap();
        h.relinked(&r);
        let left = snap(&s);
        h.undo(&mut s);
        assert_eq!(snap(&s), joined);
        h.undo(&mut s);
        assert_eq!(snap(&s), start);
        assert_eq!(s.trail_of(g), vec![g]);
        h.redo(&mut s);
        assert_eq!(snap(&s), joined);
        h.redo(&mut s);
        assert_eq!(snap(&s), left);
        assert_eq!(s.parent(a), None);
    }

    #[test]
    fn several_join_in_reading_order_skipping_the_target_and_cards_coming_along() {
        let (mut s, [a, b, c, d, e, f, g]) = board();
        let x = page(&mut s, 3000.0, 8000.0, "x");
        let start = snap(&s);
        // f comes along under e; g is lower than x; a is the target.
        let r = s.join_many(&[g, f, e, a, x], a, false).unwrap();
        assert_eq!(r.cards, vec![e, x, g]);
        assert_eq!(s.trail_of(a), vec![a, b, c, d, e, f, x, g]);
        for w in [e, x, g].windows(2) {
            let (p, q) = (s.node(w[0]).unwrap().rect, s.node(w[1]).unwrap().rect);
            assert!(q.y > p.y, "placed in turn, each below the last");
        }
        assert_eq!(s.parent(f), Some(e));
        let mut h = History::new();
        h.relinked(&r);
        h.undo(&mut s);
        assert_eq!(snap(&s), start);
        assert_eq!(s.join_many(&[a], a, false), Err(RelinkError::Missing));
    }

    #[test]
    fn the_sidebar_lists_a_joined_card_under_its_target_and_a_left_one_apart() {
        use crate::sidebar::{groups, Group};
        let (mut s, [a, b, c, d, e, f, g]) = board();
        s.join(g, b).unwrap();
        let gs = groups(&s);
        assert_eq!(gs[0], (Group::Trail(a), vec![(a, 0), (b, 1), (c, 2), (g, 2), (d, 1)]));
        assert_eq!(gs[1], (Group::Trail(e), vec![(e, 0), (f, 1)]));
        assert_eq!(gs.len(), 2, "g is no longer among the other cards");
        s.leave(g).unwrap();
        let gs = groups(&s);
        assert_eq!(gs[0].1.len(), 4);
        assert_eq!(gs.last().unwrap(), &(Group::Others, vec![(g, 0)]));
    }

    #[test]
    fn cutting_a_line_splits_the_trail_there() {
        // a → b → c and a → d.
        let (mut s, [a, b, c, d, e, f, g]) = board();
        assert_eq!(s.neighbours(b), vec![a, c]);
        assert_eq!(s.neighbours(a), vec![b, d]);
        assert_eq!(s.neighbours(f), vec![e]);
        assert!(s.neighbours(g).is_empty());
        // b cut from c (a card opened from it): c leaves.
        s.cut(b, c).unwrap();
        assert_eq!(s.trail_of(a), vec![a, b, d]);
        assert_eq!(s.trail_of(c), vec![c]);
        // b cut from a (its parent): b leaves; a keeps d.
        s.cut(b, a).unwrap();
        assert_eq!(s.trail_of(a), vec![a, d]);
        assert_eq!(s.neighbours(b), Vec::<NodeId>::new());
        // A trail's first card can cut its one line: the other card leaves.
        s.cut(e, f).unwrap();
        assert!(s.neighbours(e).is_empty());
        assert_eq!(s.cut(e, g), Err(RelinkError::Unchanged), "no line there");
    }

    #[test]
    fn leaving_several_cuts_each_one() {
        let (mut s, [a, b, c, d, ..]) = board();
        let r = s.leave_many(&[a, b, c]).unwrap();
        assert_eq!(r.cards, vec![b, c]);
        assert_eq!(s.trail_of(a), vec![a, d]);
        assert_eq!(s.trail_of(b), vec![b]);
        assert_eq!(s.trail_of(c), vec![c]);
    }
}
