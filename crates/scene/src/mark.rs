//! Trail names and colours. Our own design.
//!
//! - A trail has no object of its own (it is whatever cards its edges join),
//!   so its mark is kept on every card of it. Setting a mark writes it to all
//!   of them; a card opened beside one takes its source's mark. So closing any
//!   card, even the first, keeps the name, and a trail split by closing a card
//!   in the middle leaves both halves with it.
//! - If cards of a trail disagree (a card put back by undo after a rename), the
//!   first card in reading order with a name gives the name, and likewise for
//!   the colour.
//! - Colours are names of desktop-theme colours, so they suit light and dark
//!   themes alike; no colour means the theme's accent, as before.

use serde::{Deserialize, Serialize};

use crate::{NodeId, Scene};

/// Colours a trail takes in turn with C: none (the accent), then these.
pub const COLOURS: [Option<&str>; 7] =
    [None, Some("blue"), Some("green"), Some("magenta"), Some("orange"), Some("cyan"), Some("red")];

/// Longest trail name kept, in characters.
pub const NAME_CHARS: usize = 60;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrailMark {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A theme colour name (see [`COLOURS`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// Pinned to the top of the trails sidebar (`sidebar::toggle_pin`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Place in the sidebar's order, once the trail was moved there
    /// (`sidebar::move_group`); trails without one follow, oldest first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<u32>,
}

impl TrailMark {
    /// No name and no colour: nothing to show on the board. (A pin or a
    /// place only shows in the sidebar.)
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.colour.is_none()
    }

    /// Nothing at all: not saved.
    pub fn is_unset(&self) -> bool {
        *self == TrailMark::default()
    }
}

/// A name as typed, made fit to keep: one line, trimmed, at most
/// [`NAME_CHARS`] characters; `None` if nothing is left.
pub fn clean_name(name: &str) -> Option<String> {
    let one_line: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = one_line.chars().take(NAME_CHARS).collect();
    let cut = cut.trim_end();
    (!cut.is_empty()).then(|| cut.to_string())
}

/// The colour after `colour` in [`COLOURS`], wrapping to none.
pub fn next_colour(colour: Option<&str>) -> Option<String> {
    let i = COLOURS.iter().position(|c| *c == colour).map_or(0, |i| i + 1);
    COLOURS[i % COLOURS.len()].map(str::to_string)
}

impl Scene {
    /// The mark of the trail through `id` (empty for no card).
    pub fn trail_mark(&self, id: NodeId) -> TrailMark {
        let trail = self.trail_of(id);
        let first = |f: fn(&TrailMark) -> &Option<String>| {
            trail.iter().find_map(|&n| self.node(n).and_then(|n| f(&n.trail).clone()))
        };
        let pinned = trail.iter().any(|&n| self.node(n).is_some_and(|n| n.trail.pinned));
        let place = trail.iter().find_map(|&n| self.node(n).and_then(|n| n.trail.place));
        TrailMark { name: first(|m| &m.name), colour: first(|m| &m.colour), pinned, place }
    }

    /// Gives every card of the trail through `id` this mark. Returns each
    /// card's old mark (to undo), or empty if nothing changed.
    pub fn set_trail_mark(&mut self, id: NodeId, mark: &TrailMark) -> Vec<(NodeId, TrailMark)> {
        let mut old = Vec::new();
        for n in self.trail_of(id) {
            let node = self.nodes.get_mut(&n).expect("trail cards exist");
            if node.trail != *mark {
                old.push((n, std::mem::replace(&mut node.trail, mark.clone())));
            }
        }
        old
    }

    /// Names the trail through `id` (an empty name clears it), keeping its
    /// colour. Returns the old marks, as [`Scene::set_trail_mark`].
    pub fn name_trail(&mut self, id: NodeId, name: &str) -> Vec<(NodeId, TrailMark)> {
        let mark = TrailMark { name: clean_name(name), ..self.trail_mark(id) };
        self.set_trail_mark(id, &mark)
    }

    /// Steps the trail through `id` to its next colour, keeping its name.
    /// Returns the old marks, as [`Scene::set_trail_mark`].
    pub fn cycle_trail_colour(&mut self, id: NodeId) -> Vec<(NodeId, TrailMark)> {
        let mark = self.trail_mark(id);
        let mark = TrailMark { colour: next_colour(mark.colour.as_deref()), ..mark };
        self.set_trail_mark(id, &mark)
    }

    /// Each marked trail's mark and the card its label goes on: the first
    /// card of the trail on the board (a folded trail's is its stack's front).
    pub fn trail_labels(&self) -> Vec<(NodeId, TrailMark)> {
        let mut seen = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for n in self.all_nodes() {
            if n.trail.is_empty() || seen.contains(&n.id) {
                continue;
            }
            let trail = self.trail_of(n.id);
            seen.extend(trail.iter().copied());
            if let Some(&head) = trail.iter().find(|&&id| !self.is_hidden(id)) {
                out.push((head, self.trail_mark(head)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{History, Rect};

    /// a → b → c, and lone d.
    fn board() -> (Scene, [NodeId; 4]) {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        let c = s.spawn_beside(b, "c").unwrap();
        let d = s.add_page(Rect::new(0.0, 2000.0, 400.0, 300.0), "d");
        (s, [a, b, c, d])
    }

    fn name(s: &Scene, id: NodeId) -> Option<String> {
        s.trail_mark(id).name
    }

    #[test]
    fn names_are_one_trimmed_line_cut_short() {
        assert_eq!(clean_name("  Trip \n ideas\t"), Some("Trip ideas".into()));
        assert_eq!(clean_name("   "), None);
        let long = "x".repeat(100);
        assert_eq!(clean_name(&long).unwrap().chars().count(), NAME_CHARS);
    }

    #[test]
    fn colours_cycle_back_to_none() {
        let mut c = None;
        for _ in 1..COLOURS.len() {
            c = next_colour(c.as_deref());
            assert!(c.is_some());
        }
        assert_eq!(next_colour(c.as_deref()), None);
        assert_eq!(next_colour(Some("not a colour")), None);
    }

    #[test]
    fn naming_marks_the_whole_trail_only() {
        let (mut s, [a, b, c, d]) = board();
        let old = s.name_trail(b, " Rust ");
        assert_eq!(old.len(), 3);
        for id in [a, b, c] {
            assert_eq!(s.node(id).unwrap().trail.name.as_deref(), Some("Rust"));
        }
        assert_eq!(name(&s, d), None);
        assert!(s.name_trail(c, "Rust").is_empty(), "no change");
        s.cycle_trail_colour(a);
        assert_eq!(s.trail_mark(c), TrailMark { name: Some("Rust".into()), colour: Some("blue".into()), ..Default::default() });
        s.name_trail(a, "");
        assert_eq!(s.trail_mark(c), TrailMark { name: None, colour: Some("blue".into()), ..Default::default() });
    }

    #[test]
    fn new_links_take_the_mark_and_closing_keeps_it() {
        let (mut s, [a, b, c, _]) = board();
        s.name_trail(a, "Rust");
        let e = s.spawn_beside(c, "e").unwrap();
        let n = s.note_beside(e, "x", 300.0, 200.0).unwrap();
        assert_eq!(name(&s, n), Some("Rust".into()));
        assert_eq!(s.node(n).unwrap().trail.name.as_deref(), Some("Rust"));
        s.remove(a);
        assert_eq!(name(&s, b), Some("Rust".into()), "the first card closed");
        s.remove(c);
        assert_eq!(name(&s, b), Some("Rust".into()));
        assert_eq!(name(&s, e), Some("Rust".into()), "both halves keep it");
    }

    #[test]
    fn first_card_in_reading_order_wins_a_disagreement() {
        let (mut s, [a, _, c, _]) = board();
        s.name_trail(a, "Old");
        let (node, edges) = s.remove(c).unwrap();
        s.name_trail(a, "New");
        assert!(s.insert(node, &edges));
        assert_eq!(s.node(c).unwrap().trail.name.as_deref(), Some("Old"));
        assert_eq!(name(&s, c), Some("New".into()));
    }

    #[test]
    fn labels_go_on_the_first_card_on_the_board() {
        let (mut s, [a, b, _, d]) = board();
        assert!(s.trail_labels().is_empty());
        s.name_trail(b, "Rust");
        s.cycle_trail_colour(d);
        let labels = s.trail_labels();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].0, a);
        assert_eq!(labels[0].1.name.as_deref(), Some("Rust"));
        assert_eq!(labels[1], (d, TrailMark { name: None, colour: Some("blue".into()), ..Default::default() }));
        s.fold(b).unwrap();
        assert_eq!(s.trail_labels()[0].0, b, "the stack's front");
    }

    #[test]
    fn renaming_is_one_undo_step() {
        let (mut s, [a, _, c, _]) = board();
        let mut h = History::new();
        let old = s.name_trail(a, "Rust");
        h.marked(old);
        let old = s.cycle_trail_colour(c);
        h.marked(old);
        h.undo(&mut s);
        assert_eq!(s.trail_mark(a), TrailMark { name: Some("Rust".into()), colour: None, ..Default::default() });
        h.undo(&mut s);
        assert!(s.trail_mark(c).is_empty());
        h.redo(&mut s);
        assert_eq!(name(&s, c), Some("Rust".into()));
        h.marked(Vec::new());
        assert!(h.can_undo() && h.can_redo(), "no change is not a step");
    }

    #[test]
    fn marks_round_trip_through_json_and_old_cards_have_none() {
        let (mut s, [a, ..]) = board();
        s.name_trail(a, "Rust");
        let json = serde_json::to_string(&s).unwrap();
        let back: Scene = serde_json::from_str(&json).unwrap();
        assert_eq!(back.trail_mark(a).name.as_deref(), Some("Rust"));
        let plain = r#"{"id":1,"rect":{"x":0,"y":0,"w":1,"h":1},"url":"u","title":"t","z":1}"#;
        let n: crate::Node = serde_json::from_str(plain).unwrap();
        assert!(n.trail.is_empty());
        assert!(!serde_json::to_string(&n).unwrap().contains("trail"));
    }
}
