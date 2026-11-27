//! Links: a card joined to a card of another trail without the two trails
//! merging (owner, 2026-11-25: "keep this card connected to both"). Our own
//! design (M15, "Links"):
//!
//! - A link is kept apart from trail edges, so nothing that works out a
//!   trail sees it: trails, parents, join, leave, Fold, Tidy and the
//!   sidebar's tree are as if it weren't there. Each trail keeps its own
//!   name, colours, sidebar group and stack.
//! - It is drawn as a dashed line from the card linked to, in that card's
//!   colour; an end folded away in a stack is drawn at the stack's front,
//!   so a folded trail keeps its line to the card.
//! - Linking leaves the card where it is. A link is refused between cards
//!   of one trail (their trail line joins them already) and twice between
//!   the same two cards (either way round).
//! - Closing either card takes its links; undo brings them back.
//! - Each link or unlink is one undo step ([`crate::History::linked`]).

use crate::{Edge, NodeId, Scene};

/// Why a link or unlink changed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkError {
    /// The target, or every card, isn't on the board.
    Missing,
    /// Every card is in the target's trail already.
    SameTrail,
    /// Every card is linked to the target already, or (unlink) isn't.
    Unchanged,
}

impl Scene {
    /// Every link, as `Edge`s with `link` set.
    pub fn links(&self) -> &[Edge] {
        &self.links
    }

    /// Whether `a` and `b` are linked, either way round.
    pub fn is_linked(&self, a: NodeId, b: NodeId) -> bool {
        self.links.iter().any(|e| (e.from == a && e.to == b) || (e.from == b && e.to == a))
    }

    /// The cards linked to `id`, in the order they were linked.
    pub fn link_ends(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        for e in &self.links {
            let other = if e.from == id { e.to } else if e.to == id { e.from } else { continue };
            if other != id && self.node(other).is_some() && !out.contains(&other) {
                out.push(other);
            }
        }
        out
    }

    /// Links each of `cards` to `target` (a line from `target` to it). Cards
    /// in `target`'s trail, already linked to it, or missing are left out.
    /// Returns the links as they were (to undo).
    pub fn link_many(&mut self, cards: &[NodeId], target: NodeId) -> Result<Vec<Edge>, LinkError> {
        if self.node(target).is_none() {
            return Err(LinkError::Missing);
        }
        let cards: Vec<NodeId> = cards.iter().copied().filter(|&c| c != target && self.node(c).is_some()).collect();
        if cards.is_empty() {
            return Err(LinkError::Missing);
        }
        let trail = self.trail_of(target);
        let others: Vec<NodeId> = cards.iter().copied().filter(|c| !trail.contains(c)).collect();
        if others.is_empty() {
            return Err(LinkError::SameTrail);
        }
        let new: Vec<NodeId> = others.into_iter().filter(|&c| !self.is_linked(c, target)).collect();
        if new.is_empty() {
            return Err(LinkError::Unchanged);
        }
        let old = self.links.clone();
        self.links.extend(new.into_iter().map(|c| Edge::link(target, c)));
        Ok(old)
    }

    /// Removes the link between `a` and `b`. Returns the links as they were.
    pub fn unlink(&mut self, a: NodeId, b: NodeId) -> Result<Vec<Edge>, LinkError> {
        if !self.is_linked(a, b) {
            return Err(LinkError::Unchanged);
        }
        let old = self.links.clone();
        self.links.retain(|e| !((e.from == a && e.to == b) || (e.from == b && e.to == a)));
        Ok(old)
    }

    /// The links to draw: an end folded away in a stack is moved to the
    /// stack's front; a link whose ends then meet, and repeats, are left out.
    pub fn shown_links(&self) -> Vec<Edge> {
        let shown = |id: NodeId| self.stack_of(id).filter(|s| s.front != id).map_or(id, |s| s.front);
        let mut out: Vec<Edge> = Vec::new();
        for e in &self.links {
            if self.node(e.from).is_none() || self.node(e.to).is_none() {
                continue;
            }
            let e = Edge::link(shown(e.from), shown(e.to));
            if e.from != e.to && !out.contains(&e) {
                out.push(e);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{History, Rect};

    /// github → pulls → fillip, and google → reddit.
    fn board() -> (Scene, [NodeId; 5]) {
        let mut s = Scene::new();
        let github = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "github");
        let pulls = s.spawn_beside(github, "pulls").unwrap();
        let fillip = s.spawn_beside(pulls, "fillip").unwrap();
        let google = s.add_page(Rect::new(0.0, 1000.0, 400.0, 300.0), "google");
        let reddit = s.spawn_beside(google, "reddit").unwrap();
        (s, [github, pulls, fillip, google, reddit])
    }

    #[test]
    fn a_link_joins_two_trails_without_merging_them() {
        let (mut s, [github, pulls, fillip, google, reddit]) = board();
        let at = s.node(reddit).unwrap().rect;
        let mut h = History::new();
        let old = s.link_many(&[reddit], pulls).unwrap();
        h.linked(old);
        assert_eq!(s.links(), &[Edge::link(pulls, reddit)]);
        assert_eq!(s.trail_of(github), vec![github, pulls, fillip], "trails stay apart");
        assert_eq!(s.trail_of(reddit), vec![google, reddit]);
        assert_eq!(s.parent(reddit), Some(google));
        assert_eq!(s.node(reddit).unwrap().rect, at, "nothing moves");
        assert_eq!(s.link_ends(reddit), vec![pulls]);
        assert_eq!(s.link_ends(pulls), vec![reddit]);
        assert!(s.is_linked(pulls, reddit) && s.is_linked(reddit, pulls));
        // Refused: twice, either way round; inside one trail; missing.
        assert_eq!(s.link_many(&[reddit], pulls), Err(LinkError::Unchanged));
        assert_eq!(s.link_many(&[pulls], reddit), Err(LinkError::Unchanged));
        assert_eq!(s.link_many(&[fillip], github), Err(LinkError::SameTrail));
        assert_eq!(s.link_many(&[NodeId(99)], github), Err(LinkError::Missing));
        // One undo step each way.
        h.undo(&mut s);
        assert!(s.links().is_empty());
        h.redo(&mut s);
        assert_eq!(s.links().len(), 1);
        h.linked(s.unlink(reddit, pulls).unwrap());
        assert!(s.links().is_empty());
        assert_eq!(s.unlink(reddit, pulls), Err(LinkError::Unchanged));
        h.undo(&mut s);
        assert_eq!(s.links().len(), 1);
    }

    #[test]
    fn a_folded_trail_keeps_its_link_from_the_stack() {
        let (mut s, [github, pulls, fillip, _, reddit]) = board();
        s.link_many(&[reddit], pulls).unwrap();
        s.fold(fillip).unwrap();
        assert_eq!(s.shown_links(), vec![Edge::link(fillip, reddit)]);
        s.unfold(fillip).unwrap();
        assert_eq!(s.shown_links(), vec![Edge::link(pulls, reddit)]);
        // Folding the trail is unchanged by the link: three cards, not five.
        assert_eq!(s.fold(github).unwrap().members.len(), 3);
    }

    #[test]
    fn closing_a_card_takes_its_links_and_undo_brings_them_back() {
        let (mut s, [_, pulls, _, google, reddit]) = board();
        s.link_many(&[reddit], pulls).unwrap();
        let mut h = History::new();
        let (node, edges) = s.remove(reddit).unwrap();
        assert!(s.links().is_empty());
        assert_eq!(s.trail_of(google), vec![google]);
        h.closed(node, edges);
        h.undo(&mut s);
        assert_eq!(s.links(), &[Edge::link(pulls, reddit)]);
        assert_eq!(s.parent(reddit), Some(google));
    }

    #[test]
    fn links_are_saved() {
        let (mut s, [_, pulls, _, _, reddit]) = board();
        s.link_many(&[reddit], pulls).unwrap();
        let back: Scene = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.links(), &[Edge::link(pulls, reddit)]);
        assert_eq!(back.edges().len(), 3);
        assert!(serde_json::to_string(&back.edges()).unwrap().find("link").is_none(), "trail edges carry no flag");
    }
}
