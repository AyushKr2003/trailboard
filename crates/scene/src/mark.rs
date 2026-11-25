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
//! - Every trail gets a colour of its own when it forms (owner, 2026-11-25:
//!   "so we can tell them apart"): the colour the board's trails use least,
//!   the earliest in [`COLOURS`] on a tie ([`Scene::fresh_trail_colour`]).
//!   C still changes it, the accent included. A single card shows no colour
//!   of its own; it shows once the card has a trail.

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
    /// The mark of the trail through `id` (empty for no card): the trail's
    /// name, pin and place, and the colour of `id`'s own colour part.
    pub fn trail_mark(&self, id: NodeId) -> TrailMark {
        let trail = self.trail_of(id);
        let name = trail.iter().find_map(|&n| self.node(n).and_then(|n| n.trail.name.clone()));
        let pinned = trail.iter().any(|&n| self.node(n).is_some_and(|n| n.trail.pinned));
        let place = trail.iter().find_map(|&n| self.node(n).and_then(|n| n.trail.place));
        let colour = self.node(id).and_then(|n| n.trail.colour.clone());
        TrailMark { name, colour, pinned, place }
    }

    /// The cards of the trail through `id` that share its colour (its
    /// colour part), in reading order. Cards that joined from another
    /// coloured trail keep their colour, so a trail can have several parts.
    pub fn colour_part(&self, id: NodeId) -> Vec<NodeId> {
        let Some(colour) = self.node(id).map(|n| n.trail.colour.clone()) else { return Vec::new() };
        self.trail_of(id).into_iter().filter(|n| self.nodes[n].trail.colour == colour).collect()
    }

    /// Gives every card of the trail through `id` this mark's name, pin and
    /// place, and the cards of `id`'s colour part its colour (other parts
    /// keep theirs). Returns each card's old mark (to undo), or empty if
    /// nothing changed.
    pub fn set_trail_mark(&mut self, id: NodeId, mark: &TrailMark) -> Vec<(NodeId, TrailMark)> {
        let part: std::collections::BTreeSet<NodeId> = self.colour_part(id).into_iter().collect();
        let mut old = Vec::new();
        for n in self.trail_of(id) {
            let node = self.nodes.get_mut(&n).expect("trail cards exist");
            let colour = if part.contains(&n) { mark.colour.clone() } else { node.trail.colour.clone() };
            let new = TrailMark { colour, ..mark.clone() };
            if node.trail != new {
                old.push((n, std::mem::replace(&mut node.trail, new)));
            }
        }
        old
    }

    /// Names the trail through `id` (an empty name clears it), keeping its
    /// colours. Returns the old marks, as [`Scene::set_trail_mark`].
    pub fn name_trail(&mut self, id: NodeId, name: &str) -> Vec<(NodeId, TrailMark)> {
        let mark = TrailMark { name: clean_name(name), ..self.trail_mark(id) };
        self.set_trail_mark(id, &mark)
    }

    /// Steps the colour part of `id` to its next colour, keeping the
    /// trail's name. Returns the old marks, as [`Scene::set_trail_mark`].
    pub fn cycle_trail_colour(&mut self, id: NodeId) -> Vec<(NodeId, TrailMark)> {
        let mark = self.trail_mark(id);
        let mark = TrailMark { colour: next_colour(mark.colour.as_deref()), ..mark };
        self.set_trail_mark(id, &mark)
    }

    /// The colour for a trail (or part) that is forming: of the colours in
    /// [`COLOURS`] (not the accent), the one the board's trails of two or
    /// more cards use least (each colour part counts); the earliest on a tie.
    pub fn fresh_trail_colour(&self) -> String {
        let mut used: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        let mut seen = std::collections::BTreeSet::new();
        for n in self.all_nodes() {
            if seen.contains(&n.id) {
                continue;
            }
            let trail = self.trail_of(n.id);
            seen.extend(trail.iter().copied());
            if trail.len() > 1 {
                let colours: std::collections::BTreeSet<String> = trail.iter().filter_map(|&t| self.nodes[&t].trail.colour.clone()).collect();
                for c in colours {
                    *used.entry(c).or_default() += 1;
                }
            }
        }
        let count = |c: &str| used.get(c).copied().unwrap_or(0);
        let colours = COLOURS.iter().flatten();
        let least = colours.clone().map(|c| count(c)).min().unwrap_or(0);
        colours.copied().find(|c| count(c) == least).unwrap_or("blue").to_string()
    }

    /// Gives the trail through `id` a fresh colour if it has two or more
    /// cards and none of them has one yet. Returns the old marks, as
    /// [`Scene::set_trail_mark`].
    pub fn colour_new_trail(&mut self, id: NodeId) -> Vec<(NodeId, TrailMark)> {
        let trail = self.trail_of(id);
        if trail.len() < 2 || trail.iter().any(|t| self.nodes[t].trail.colour.is_some()) {
            return Vec::new();
        }
        let colour = self.fresh_trail_colour();
        let mark = self.trail_mark(id);
        self.set_trail_mark(id, &TrailMark { colour: Some(colour), ..mark })
    }

    /// Gives every trail of two or more cards without a colour one of its
    /// own, oldest trail first (a board saved before trails were coloured
    /// as they formed). Returns how many were coloured.
    pub fn colour_trails(&mut self) -> usize {
        let heads: Vec<NodeId> = {
            let mut seen = std::collections::BTreeSet::new();
            let mut out = Vec::new();
            for n in self.all_nodes() {
                if seen.insert(n.id) {
                    let trail = self.trail_of(n.id);
                    seen.extend(trail.iter().copied());
                    out.push(trail[0]);
                }
            }
            out
        };
        heads.into_iter().filter(|&h| !self.colour_new_trail(h).is_empty()).count()
    }

    /// The labels above trails: each marked trail's mark on its first card
    /// on the board (a folded trail's is its stack's front), and a colour
    /// dot on the first card of each further colour part. A single card
    /// with only a colour shows none: the colour is its trail's, and it has
    /// none yet.
    pub fn trail_labels(&self) -> Vec<(NodeId, TrailMark)> {
        let mut seen = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for n in self.all_nodes() {
            if seen.contains(&n.id) {
                continue;
            }
            let trail = self.trail_of(n.id);
            seen.extend(trail.iter().copied());
            let shown: Vec<NodeId> = trail.iter().copied().filter(|&id| !self.is_hidden(id)).collect();
            let Some(&head) = shown.first() else { continue };
            let mark = self.trail_mark(head);
            if !mark.is_empty() && !(trail.len() == 1 && mark.name.is_none()) {
                out.push((head, mark.clone()));
            }
            let mut colours = std::collections::BTreeSet::from([mark.colour.clone()]);
            for &id in &shown[1..] {
                let c = self.nodes[&id].trail.colour.clone();
                if c.is_some() && colours.insert(c.clone()) {
                    out.push((id, TrailMark { colour: c, ..TrailMark::default() }));
                }
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
        assert_eq!(s.trail_mark(c).colour.as_deref(), Some("blue"), "a colour of its own when it formed");
        s.cycle_trail_colour(a);
        assert_eq!(s.trail_mark(c), TrailMark { name: Some("Rust".into()), colour: Some("green".into()), ..Default::default() });
        s.name_trail(a, "");
        assert_eq!(s.trail_mark(c), TrailMark { name: None, colour: Some("green".into()), ..Default::default() });
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
        assert_eq!(s.trail_labels(), vec![(a, TrailMark { colour: Some("blue".into()), ..Default::default() })]);
        s.name_trail(b, "Rust");
        // A lone card's colour shows only once it has a trail; a name shows.
        s.cycle_trail_colour(d);
        assert_eq!(s.trail_labels().len(), 1);
        s.name_trail(d, "Alone");
        let labels = s.trail_labels();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].0, a);
        assert_eq!(labels[0].1.name.as_deref(), Some("Rust"));
        assert_eq!(labels[1], (d, TrailMark { name: Some("Alone".into()), colour: Some("blue".into()), ..Default::default() }));
        s.fold(b).unwrap();
        assert_eq!(s.trail_labels()[0].0, b, "the stack's front");
    }

    #[test]
    fn each_new_trail_takes_the_colour_used_least() {
        let mut s = Scene::new();
        let mut heads = Vec::new();
        for i in 0..8 {
            let h = s.add_page(Rect::new(0.0, 1000.0 * i as f32, 400.0, 300.0), "h");
            s.spawn_beside(h, "x").unwrap();
            heads.push(h);
        }
        let colours: Vec<String> = heads.iter().map(|&h| s.trail_mark(h).colour.unwrap()).collect();
        // Six trails, six colours; then round again, least used first.
        assert_eq!(colours, ["blue", "green", "magenta", "orange", "cyan", "red", "blue", "green"]);
        // A trail recoloured by hand counts as its new colour.
        s.cycle_trail_colour(heads[0]); // blue → green
        assert_eq!(s.fresh_trail_colour(), "blue");
        // Opening more on a trail keeps its colour.
        s.spawn_beside(heads[1], "y").unwrap();
        assert_eq!(s.trail_mark(heads[1]).colour.as_deref(), Some("green"));
    }

    #[test]
    fn an_older_board_gets_a_colour_per_trail_once() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.add_page(Rect::new(500.0, 0.0, 400.0, 300.0), "b");
        let c = s.add_page(Rect::new(0.0, 1000.0, 400.0, 300.0), "c");
        let d = s.add_page(Rect::new(500.0, 1000.0, 400.0, 300.0), "d");
        let lone = s.add_page(Rect::new(0.0, 2000.0, 400.0, 300.0), "e");
        s.insert_edge(crate::Edge { from: a, to: b });
        s.insert_edge(crate::Edge { from: c, to: d });
        assert_eq!(s.colour_trails(), 2);
        assert_eq!(s.trail_mark(b).colour.as_deref(), Some("blue"));
        assert_eq!(s.trail_mark(d).colour.as_deref(), Some("green"));
        assert!(s.trail_mark(lone).is_unset(), "a lone card has no trail to colour");
        assert_eq!(s.colour_trails(), 0, "already coloured");
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
        assert_eq!(s.trail_mark(a), TrailMark { name: Some("Rust".into()), colour: Some("blue".into()), ..Default::default() });
        h.undo(&mut s);
        assert_eq!(s.trail_mark(c), TrailMark { colour: Some("blue".into()), ..Default::default() });
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
