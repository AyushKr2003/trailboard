//! Jumps and marks (M12 step 9). Our own design; the keys follow common
//! vim-style habits (`Ctrl+o` / `Ctrl+i`, `m{a-z}`, `'{a-z}`, `''`).
//!
//! - [`Jumps`]: the cards visited in one space, oldest first, each once (a
//!   card visited again moves to the end). `Ctrl+o` / `Ctrl+i` walk it
//!   without changing it; visiting a card from anywhere else adds it at the
//!   end, as a new jump does.
//! - Marks: a lower-case letter is a card's own scroll spot ([`ScrollSpot`],
//!   kept per card); an upper-case one is a board spot ([`BoardSpot`]: a
//!   space, a card and the camera), one per letter across every space.
//! - [`Before`]: where the last jump left from, for `''`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::NodeId;
use crate::geom::Camera;

/// Cards one space's jump list keeps at most.
pub const MAX_JUMPS: usize = 100;

/// The cards visited in a space, oldest first, and where `Ctrl+o` /
/// `Ctrl+i` have got to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Jumps {
    cards: Vec<NodeId>,
    /// Index of the entry we are at (the last one after a visit).
    at: usize,
}

impl Jumps {
    /// A list read back from disk; repeats keep their last place and a bad
    /// `at` goes to the end.
    pub fn from_parts(cards: Vec<NodeId>, at: usize) -> Jumps {
        let mut out: Vec<NodeId> = Vec::new();
        for (i, c) in cards.iter().enumerate() {
            if !cards[i + 1..].contains(c) {
                out.push(*c);
            }
        }
        let cut = out.len().saturating_sub(MAX_JUMPS);
        out.drain(..cut);
        let at = if at < out.len() { at } else { out.len().saturating_sub(1) };
        Jumps { cards: out, at }
    }

    pub fn cards(&self) -> &[NodeId] {
        &self.cards
    }

    pub fn at(&self) -> usize {
        self.at
    }

    /// Card `id` became current by any way but the jump list: it goes to the end.
    pub fn visit(&mut self, id: NodeId) {
        self.cards.retain(|&c| c != id);
        self.cards.push(id);
        let cut = self.cards.len().saturating_sub(MAX_JUMPS);
        self.cards.drain(..cut);
        self.at = self.cards.len() - 1;
    }

    /// `Ctrl+o`: `n` cards back that are still `alive`, or `None` if there
    /// is none older. `current`: the card current now; if it isn't the one
    /// the list is at (it was closed), the first step goes to that one.
    pub fn back(&mut self, n: usize, current: Option<NodeId>, alive: impl Fn(NodeId) -> bool) -> Option<NodeId> {
        if self.cards.is_empty() {
            return None;
        }
        let mut i = if Some(self.cards[self.at]) == current { self.at } else { self.at + 1 };
        let mut moved = None;
        for _ in 0..n.max(1) {
            match (0..i).rev().find(|&j| alive(self.cards[j]) && Some(self.cards[j]) != current) {
                Some(j) => {
                    i = j;
                    moved = Some(j);
                }
                None => break,
            }
        }
        let j = moved?;
        self.at = j;
        Some(self.cards[j])
    }

    /// `Ctrl+i`: `n` cards forward that are still `alive`, or `None`.
    pub fn forward(&mut self, n: usize, current: Option<NodeId>, alive: impl Fn(NodeId) -> bool) -> Option<NodeId> {
        let mut i = self.at;
        let mut moved = None;
        for _ in 0..n.max(1) {
            match (i + 1..self.cards.len()).find(|&j| alive(self.cards[j]) && Some(self.cards[j]) != current) {
                Some(j) => {
                    i = j;
                    moved = Some(j);
                }
                None => break,
            }
        }
        let j = moved?;
        self.at = j;
        Some(self.cards[j])
    }

    /// Drops cards that are gone for good, keeping `at` on the same entry
    /// (or the one before it).
    pub fn prune(&mut self, keep: impl Fn(NodeId) -> bool) {
        let current = self.cards.get(self.at).copied();
        let before = self.cards[..self.at.min(self.cards.len())].iter().filter(|&&c| keep(c)).count();
        self.cards.retain(|&c| keep(c));
        self.at = match current {
            Some(c) if keep(c) => before,
            _ => before.saturating_sub(1),
        }
        .min(self.cards.len().saturating_sub(1));
    }
}

/// A letter that marks a card's scroll spot (`a`–`z`).
pub fn is_card_mark(c: char) -> bool {
    c.is_ascii_lowercase()
}

/// A letter that marks a board spot (`A`–`Z`).
pub fn is_board_mark(c: char) -> bool {
    c.is_ascii_uppercase()
}

/// Where a page was scrolled: its address, which scroll section (a path
/// of child numbers from the document's root, `""` for the page itself;
/// see `web::scroll`) and how far it was scrolled, CSS px.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollSpot {
    pub url: String,
    #[serde(default)]
    pub path: String,
    pub x: f32,
    pub y: f32,
}

/// The same page: equal addresses, the part after `#` aside.
pub fn same_page(a: &str, b: &str) -> bool {
    let cut = |u: &str| u.split('#').next().unwrap_or(u).to_string();
    cut(a) == cut(b)
}

/// A place on a board: the space (by its id), the card current there (if
/// any) and the camera.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoardSpot {
    pub space: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<NodeId>,
    pub camera: Camera,
}

/// Where the last jump left from (`''` goes back there, and that jump's
/// start becomes the new one).
#[derive(Debug, Clone, PartialEq)]
pub enum Before {
    Board(BoardSpot),
    Scroll(NodeId, ScrollSpot),
}

/// One space's jump list and its cards' marks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpacePlaces {
    pub jumps: Jumps,
    pub marks: BTreeMap<NodeId, BTreeMap<char, ScrollSpot>>,
}

/// Every space's jumps and marks, and the board marks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Places {
    pub spaces: BTreeMap<u64, SpacePlaces>,
    pub board: BTreeMap<char, BoardSpot>,
}

impl Places {
    pub fn space(&mut self, space: u64) -> &mut SpacePlaces {
        self.spaces.entry(space).or_default()
    }

    /// Sets mark `c` of card `card` (a lower-case letter).
    pub fn set_card_mark(&mut self, space: u64, card: NodeId, c: char, spot: ScrollSpot) {
        self.space(space).marks.entry(card).or_default().insert(c, spot);
    }

    pub fn card_mark(&self, space: u64, card: NodeId, c: char) -> Option<&ScrollSpot> {
        self.spaces.get(&space)?.marks.get(&card)?.get(&c)
    }

    /// A space was deleted: its jumps and marks go, and board marks in it.
    pub fn forget_space(&mut self, space: u64) {
        self.spaces.remove(&space);
        self.board.retain(|_, s| s.space != space);
    }

    /// Drops a space's jumps and marks of cards that are gone for good.
    pub fn prune(&mut self, space: u64, keep: impl Fn(NodeId) -> bool) {
        let s = self.space(space);
        s.jumps.prune(&keep);
        s.marks.retain(|id, m| keep(*id) && !m.is_empty());
    }

    /// The marks of one card, as `a b c`, for a message.
    pub fn letters(&self, space: u64, card: NodeId) -> String {
        let card: Vec<char> = self.spaces.get(&space).and_then(|s| s.marks.get(&card)).map(|m| m.keys().copied().collect()).unwrap_or_default();
        card.iter().chain(self.board.keys()).map(|c| c.to_string()).collect::<Vec<_>>().join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[u64]) -> Vec<NodeId> {
        v.iter().map(|&i| NodeId(i)).collect()
    }

    #[test]
    fn visits_move_to_the_end() {
        let mut j = Jumps::default();
        for i in [1, 2, 3, 2] {
            j.visit(NodeId(i));
        }
        assert_eq!(j.cards(), ids(&[1, 3, 2]));
        assert_eq!(j.at(), 2);
    }

    #[test]
    fn back_and_forward() {
        let mut j = Jumps::default();
        for i in 1..=4 {
            j.visit(NodeId(i));
        }
        let all = |_| true;
        assert_eq!(j.back(1, Some(NodeId(4)), all), Some(NodeId(3)));
        assert_eq!(j.back(2, Some(NodeId(3)), all), Some(NodeId(1)));
        assert_eq!(j.back(1, Some(NodeId(1)), all), None, "nothing older");
        assert_eq!(j.at(), 0);
        assert_eq!(j.forward(1, Some(NodeId(1)), all), Some(NodeId(2)));
        assert_eq!(j.forward(9, Some(NodeId(2)), all), Some(NodeId(4)), "as far as it goes");
        assert_eq!(j.forward(1, Some(NodeId(4)), all), None);
        // A visit from the middle goes to the end; the list isn't cut.
        j.back(2, Some(NodeId(4)), all);
        j.visit(NodeId(5));
        assert_eq!(j.cards(), ids(&[1, 2, 3, 4, 5]));
    }

    #[test]
    fn closed_cards_are_skipped() {
        let mut j = Jumps::default();
        for i in 1..=4 {
            j.visit(NodeId(i));
        }
        let alive = |id: NodeId| id != NodeId(3);
        assert_eq!(j.back(1, Some(NodeId(4)), alive), Some(NodeId(2)));
        // The current card was closed (nothing is current): back goes to
        // the newest card still there.
        let mut j = Jumps::from_parts(ids(&[1, 2]), 1);
        assert_eq!(j.back(1, None, |_| true), Some(NodeId(2)));
        let mut j = Jumps::from_parts(ids(&[1, 2]), 1);
        assert_eq!(j.back(1, None, |id| id != NodeId(2)), Some(NodeId(1)));
        assert_eq!(Jumps::default().back(1, None, |_| true), None);
    }

    #[test]
    fn capped_and_read_back() {
        let mut j = Jumps::default();
        for i in 0..(MAX_JUMPS as u64 + 10) {
            j.visit(NodeId(i));
        }
        assert_eq!(j.cards().len(), MAX_JUMPS);
        assert_eq!(j.cards()[0], NodeId(10));
        let r = Jumps::from_parts(ids(&[1, 2, 1, 3]), 99);
        assert_eq!(r.cards(), ids(&[2, 1, 3]));
        assert_eq!(r.at(), 2);
        assert_eq!(Jumps::from_parts(Vec::new(), 4).at(), 0);
    }

    #[test]
    fn prune_keeps_the_place() {
        let mut j = Jumps::from_parts(ids(&[1, 2, 3, 4]), 2);
        j.prune(|id| id != NodeId(1));
        assert_eq!(j.cards(), ids(&[2, 3, 4]));
        assert_eq!(j.cards()[j.at()], NodeId(3));
        j.prune(|id| id != NodeId(3));
        assert_eq!(j.cards()[j.at()], NodeId(2), "the one before");
        j.prune(|_| false);
        assert!(j.cards().is_empty());
        assert_eq!(j.at(), 0);
    }

    #[test]
    fn marks() {
        assert!(is_card_mark('a') && !is_card_mark('A') && !is_card_mark('\''));
        assert!(is_board_mark('Z') && !is_board_mark('z') && !is_board_mark('1'));
        assert!(same_page("https://a.org/x#top", "https://a.org/x"));
        assert!(!same_page("https://a.org/x", "https://a.org/y"));
        let mut p = Places::default();
        let spot = ScrollSpot { url: "u".into(), path: String::new(), x: 0.0, y: 40.0 };
        p.set_card_mark(1, NodeId(5), 'a', spot.clone());
        assert_eq!(p.card_mark(1, NodeId(5), 'a'), Some(&spot));
        assert_eq!(p.card_mark(2, NodeId(5), 'a'), None, "marks are per space");
        p.board.insert('A', BoardSpot { space: 2, card: None, camera: Camera::default() });
        assert_eq!(p.letters(1, NodeId(5)), "a A");
        p.prune(1, |id| id != NodeId(5));
        assert_eq!(p.card_mark(1, NodeId(5), 'a'), None);
        p.forget_space(2);
        assert!(p.board.is_empty());
    }
}
