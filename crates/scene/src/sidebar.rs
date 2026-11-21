//! The trails sidebar (B): every card of the board as a list, one group per
//! trail with its cards as a tree, then the cards on no trail. Our own design.
//!
//! - A trail is a group when it has two or more cards, a name (like Save as
//!   Markdown, `export`) or a pin. Pinned groups come first, then the rest;
//!   in each part, groups that were moved (`move_group`) keep their place and
//!   the others follow in the order their first card was made. Inside a
//!   group, cards are in reading order (`Scene::trail_tree`), indented by
//!   depth.
//! - Pins and places are part of the trail's mark (`TrailMark`), so they are
//!   saved with the board, kept by every card of the trail (a split trail
//!   keeps them in both halves) and undone like a rename. Cards folded away in a stack are listed in their place, marked
//!   `folded`, so the list doubles as a way into a stack.
//! - Cards on no trail come last, under one "Other cards" group. With no
//!   trail on the board that group is all there is, so its header is left
//!   out and the cards are listed bare (and can't be collapsed).
//! - A group can be collapsed to its header row. Collapsing is keyed by the
//!   trail's first card, lives in this state only (not the board file), and
//!   is forgotten when that card stops being first.
//! - The highlight is a row key, not an index, so it survives the list being
//!   rebuilt (every frame); a highlight on a row that went away falls back to
//!   the nearest row by position.

use std::collections::BTreeSet;

use crate::{NodeId, Scene, TrailMark};

/// A group of rows: one trail (by its first card), or the cards on no trail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Trail(NodeId),
    Others,
}

/// One row of the sidebar.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// A group's header: the trail's name (else its first card's title), its
    /// mark and number of cards.
    Header { group: Group, label: String, mark: TrailMark, count: usize, collapsed: bool },
    /// A card, `depth` levels into its trail; `folded` if it is behind its stack's front.
    Card { id: NodeId, group: Group, depth: usize, folded: bool },
}

/// What a row stands for, stable across rebuilds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKey {
    Header(Group),
    Card(NodeId),
}

impl Row {
    pub fn key(&self) -> RowKey {
        match *self {
            Row::Header { group, .. } => RowKey::Header(group),
            Row::Card { id, .. } => RowKey::Card(id),
        }
    }

    pub fn group(&self) -> Group {
        match *self {
            Row::Header { group, .. } | Row::Card { group, .. } => group,
        }
    }
}

/// Header label of the cards on no trail.
pub const OTHERS: &str = "Other cards";

/// Whether `groups` is just the cards on no trail, listed without a header.
fn only_others(groups: &[(Group, Vec<(NodeId, usize)>)]) -> bool {
    matches!(groups, [(Group::Others, _)])
}

/// The board's groups, each as `(group, cards in reading order with depth)`,
/// in the sidebar's order.
pub fn groups(scene: &Scene) -> Vec<(Group, Vec<(NodeId, usize)>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut lone = Vec::new();
    for n in scene.all_nodes() {
        if seen.contains(&n.id) {
            continue;
        }
        let tree = scene.trail_tree(n.id);
        seen.extend(tree.iter().map(|&(id, _)| id));
        let mark = scene.trail_mark(n.id);
        if tree.len() == 1 && mark.name.is_none() && !mark.pinned {
            lone.push((n.id, 0));
        } else {
            out.push((mark, Group::Trail(tree[0].0), tree));
        }
    }
    // Stable: unplaced groups keep the order their first card was made.
    out.sort_by_key(|(mark, ..)| (!mark.pinned, mark.place.unwrap_or(u32::MAX)));
    let mut out: Vec<_> = out.into_iter().map(|(_, g, tree)| (g, tree)).collect();
    if !lone.is_empty() {
        out.push((Group::Others, lone));
    }
    out
}

/// First cards of the trails listed with `head`'s pin state, in order.
fn section(scene: &Scene, pinned: bool) -> Vec<NodeId> {
    groups(scene)
        .into_iter()
        .filter_map(|(g, _)| match g {
            Group::Trail(h) if scene.trail_mark(h).pinned == pinned => Some(h),
            _ => None,
        })
        .collect()
}

/// Gives every trail of `order` its index as its place. Old marks, to undo.
fn set_places(scene: &mut Scene, order: &[NodeId]) -> Vec<(NodeId, TrailMark)> {
    let mut old = Vec::new();
    for (i, &h) in order.iter().enumerate() {
        let mark = TrailMark { place: Some(i as u32), ..scene.trail_mark(h) };
        old.extend(scene.set_trail_mark(h, &mark));
    }
    old
}

/// Where the trail of `head` is among the pinned or the other trails, and
/// how many those are. `None` if `head` heads no listed trail.
pub fn position(scene: &Scene, head: NodeId) -> Option<(usize, usize)> {
    let s = section(scene, scene.trail_mark(head).pinned);
    Some((s.iter().position(|&h| h == head)?, s.len()))
}

/// Moves the trail headed by `head` to index `to` among the trails of its
/// part (pinned or not; clamped), numbering that part's places afresh.
/// Returns the old marks (for `History::marked`), empty if it didn't move.
pub fn move_group(scene: &mut Scene, head: NodeId, to: usize) -> Vec<(NodeId, TrailMark)> {
    let mut order = section(scene, scene.trail_mark(head).pinned);
    let Some(from) = order.iter().position(|&h| h == head) else { return Vec::new() };
    let to = to.min(order.len() - 1);
    if to == from {
        return Vec::new();
    }
    order.remove(from);
    order.insert(to, head);
    set_places(scene, &order)
}

/// Dragging the header of `head`'s trail over a row of `over`: the index
/// among its part's trails it would move to, if `over` is a trail of the same
/// part (pinned or not).
pub fn drop_to(scene: &Scene, head: NodeId, over: Group) -> Option<usize> {
    let Group::Trail(t) = over else { return None };
    let pinned = scene.trail_mark(head).pinned;
    (scene.trail_mark(t).pinned == pinned).then(|| section(scene, pinned).iter().position(|&h| h == t))?
}

/// The trail a row belongs to, by its first card (`None` for the cards on no trail).
pub fn trail_head(row: &Row) -> Option<NodeId> {
    match row.group() {
        Group::Trail(h) => Some(h),
        Group::Others => None,
    }
}

/// Pins the trail through card `id` (a card on no trail becomes a group of
/// its own), or unpins it. It goes last in its new part, whose places are
/// numbered afresh. Returns the old marks and whether it is pinned now.
pub fn toggle_pin(scene: &mut Scene, id: NodeId) -> (Vec<(NodeId, TrailMark)>, bool) {
    if scene.node(id).is_none() {
        return (Vec::new(), false);
    }
    let mark = scene.trail_mark(id);
    let pinned = !mark.pinned;
    let mut old = scene.set_trail_mark(id, &TrailMark { pinned, place: None, ..mark });
    let mut order = section(scene, pinned);
    if let Some(head) = groups(scene).into_iter().find(|(_, cards)| cards.iter().any(|&(c, _)| c == id)).map(|(g, _)| g)
        && let Group::Trail(head) = head
    {
        order.retain(|&h| h != head);
        order.push(head);
    }
    // Keep each card's first old mark only, so undo goes back all the way.
    for (n, m) in set_places(scene, &order) {
        if !old.iter().any(|(o, _)| *o == n) {
            old.push((n, m));
        }
    }
    (old, pinned)
}

/// Sidebar state that outlives a frame: collapsed groups, highlight, scroll.
#[derive(Debug, Clone, Default)]
pub struct Sidebar {
    pub collapsed: BTreeSet<Group>,
    /// Highlighted row while the sidebar has the keys.
    pub sel: Option<RowKey>,
    /// Index of the first row shown.
    pub scroll: usize,
}

impl Sidebar {
    /// The rows to show, collapsed groups as their header only. Forgets
    /// collapsed trails that no longer exist. With no trail, just the cards.
    pub fn rows(&mut self, scene: &Scene) -> Vec<Row> {
        let groups = groups(scene);
        if only_others(&groups) {
            // No header to expand it from, so it can't stay collapsed.
            self.collapsed.clear();
            let (group, cards) = groups.into_iter().next().unwrap();
            return cards.into_iter().map(|(id, depth)| Row::Card { id, group, depth, folded: scene.is_hidden(id) }).collect();
        }
        self.collapsed.retain(|g| groups.iter().any(|(h, _)| h == g));
        let mut rows = Vec::new();
        for (group, cards) in groups {
            let collapsed = self.collapsed.contains(&group);
            let (label, mark) = match group {
                Group::Trail(head) => {
                    let mark = scene.trail_mark(head);
                    let title = scene.node(head).map(|n| n.title.trim()).filter(|t| !t.is_empty());
                    let title = title.or_else(|| scene.node(head).map(|n| n.url.as_str())).unwrap_or_default();
                    (mark.name.clone().unwrap_or_else(|| title.to_string()), mark)
                }
                Group::Others => (OTHERS.to_string(), TrailMark::default()),
            };
            rows.push(Row::Header { group, label, mark, count: cards.len(), collapsed });
            if !collapsed {
                rows.extend(cards.into_iter().map(|(id, depth)| Row::Card { id, group, depth, folded: scene.is_hidden(id) }));
            }
        }
        rows
    }

    /// Index of the highlighted row, if it is shown.
    pub fn index(&self, rows: &[Row]) -> Option<usize> {
        let sel = self.sel?;
        rows.iter().position(|r| r.key() == sel)
    }

    /// Highlights the row of card `id`, or its group's header when the group
    /// is collapsed. False if the card isn't listed.
    pub fn select_card(&mut self, scene: &Scene, id: NodeId) -> bool {
        let groups = groups(scene);
        let bare = only_others(&groups);
        let Some((group, _)) = groups.into_iter().find(|(_, cards)| cards.iter().any(|&(c, _)| c == id)) else {
            return false;
        };
        self.sel = Some(if !bare && self.collapsed.contains(&group) { RowKey::Header(group) } else { RowKey::Card(id) });
        true
    }

    /// Moves the highlight `step` rows (clamped at the ends). With no
    /// highlight, starts at the first row (or the last, going up).
    /// `old_index` is where the highlight was, for a row that went away.
    pub fn step(&mut self, rows: &[Row], step: isize, old_index: Option<usize>) -> Option<RowKey> {
        if rows.is_empty() {
            self.sel = None;
            return None;
        }
        let last = rows.len() as isize - 1;
        let to = match self.index(rows).or(old_index.map(|i| i.min(rows.len() - 1))) {
            Some(i) => (i as isize + step).clamp(0, last),
            None if step >= 0 => 0,
            None => last,
        };
        self.sel = Some(rows[to as usize].key());
        self.sel
    }

    /// h: collapses the highlighted row's group and highlights its header.
    /// False if it was collapsed already (or nothing is highlighted).
    pub fn collapse(&mut self, rows: &[Row]) -> bool {
        let Some(row) = self.index(rows).map(|i| &rows[i]) else { return false };
        let group = row.group();
        if !rows.iter().any(|r| r.key() == RowKey::Header(group)) {
            return false; // listed bare, with no header to fold into
        }
        self.sel = Some(RowKey::Header(group));
        self.collapsed.insert(group)
    }

    /// l: expands the highlighted header's group. False if it wasn't a
    /// collapsed header.
    pub fn expand(&mut self, rows: &[Row]) -> bool {
        match self.index(rows).map(|i| &rows[i]) {
            Some(&Row::Header { group, collapsed: true, .. }) => self.collapsed.remove(&group),
            _ => false,
        }
    }

    /// Collapses or expands `group`.
    pub fn toggle(&mut self, group: Group) {
        if !self.collapsed.remove(&group) {
            self.collapsed.insert(group);
        }
    }

    /// Scrolls the least so that row `index` is among the `shown` rows, and
    /// so no space is left empty below the last row.
    pub fn scroll_to(&mut self, index: Option<usize>, len: usize, shown: usize) {
        let shown = shown.max(1);
        if let Some(i) = index {
            if i < self.scroll {
                self.scroll = i;
            } else if i >= self.scroll + shown {
                self.scroll = i + 1 - shown;
            }
        }
        self.scroll = self.scroll.min(len.saturating_sub(shown));
    }

    /// Wheel: scrolls by `lines` rows (negative goes up), within the list.
    pub fn scroll_by(&mut self, lines: isize, len: usize, shown: usize) {
        let max = len.saturating_sub(shown.max(1)) as isize;
        self.scroll = (self.scroll as isize + lines).clamp(0, max.max(0)) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    /// a → b, a → c, c → d; lone e; named lone f; lone g.
    fn board() -> (Scene, [NodeId; 7]) {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        let c = s.spawn_beside(a, "c").unwrap();
        let d = s.spawn_beside(c, "d").unwrap();
        let e = s.add_page(Rect::new(0.0, 2000.0, 400.0, 300.0), "e");
        let f = s.add_page(Rect::new(0.0, 3000.0, 400.0, 300.0), "f");
        s.name_trail(f, "Reading");
        let g = s.add_page(Rect::new(0.0, 4000.0, 400.0, 300.0), "g");
        (s, [a, b, c, d, e, f, g])
    }

    fn keys(rows: &[Row]) -> Vec<RowKey> {
        rows.iter().map(Row::key).collect()
    }

    #[test]
    fn lists_trails_as_trees_then_cards_on_no_trail() {
        let (mut s, [a, b, c, d, e, f, g]) = board();
        s.node_mut(a).unwrap().title = "  ".into();
        let rows = Sidebar::default().rows(&s);
        use RowKey::*;
        assert_eq!(
            keys(&rows),
            [Header(Group::Trail(a)), Card(a), Card(b), Card(c), Card(d), Header(Group::Trail(f)), Card(f), Header(Group::Others), Card(e), Card(g)]
        );
        let Row::Header { label, count, .. } = &rows[0] else { panic!() };
        assert_eq!((label.as_str(), *count), ("a", 4), "an empty title falls back to the URL");
        let Row::Header { label, mark, .. } = &rows[5] else { panic!() };
        assert_eq!((label.as_str(), mark.name.as_deref()), ("Reading", Some("Reading")));
        let Row::Header { label, count, .. } = &rows[7] else { panic!() };
        assert_eq!((label.as_str(), *count), (OTHERS, 2));
        let depths: Vec<usize> =
            rows[1..5].iter().map(|r| if let Row::Card { depth, .. } = r { *depth } else { 9 }).collect();
        assert_eq!(depths, [0, 1, 1, 2]);
    }

    #[test]
    fn folded_cards_stay_listed_and_marked() {
        let (mut s, [a, b, c, d, ..]) = board();
        s.fold(c).unwrap();
        let rows = Sidebar::default().rows(&s);
        let folded: Vec<(NodeId, bool)> =
            rows[1..5].iter().map(|r| if let Row::Card { id, folded, .. } = r { (*id, *folded) } else { panic!() }).collect();
        assert_eq!(folded, [(a, true), (b, true), (c, false), (d, true)]);
    }

    #[test]
    fn steps_clamp_and_survive_a_removed_row() {
        let (mut s, [a, b, ..]) = board();
        let mut sb = Sidebar::default();
        let rows = sb.rows(&s);
        assert_eq!(sb.step(&rows, 1, None), Some(RowKey::Header(Group::Trail(a))));
        assert_eq!(sb.step(&rows, -1, None), Some(RowKey::Header(Group::Trail(a))), "clamped at the top");
        sb.step(&rows, 2, None);
        assert_eq!(sb.sel, Some(RowKey::Card(b)));
        assert_eq!(sb.step(&rows, 99, None), Some(rows.last().unwrap().key()));
        sb.sel = Some(RowKey::Card(b));
        let at = sb.index(&rows);
        s.remove(b);
        let rows = sb.rows(&s);
        assert_eq!(sb.index(&rows), None);
        // From b's old place (2), one down: the row now at 3 (d, after c).
        assert_eq!(sb.step(&rows, 1, at), Some(rows[3].key()));
        assert_eq!(Sidebar::default().step(&[], 1, None), None);
    }

    #[test]
    fn collapse_and_expand_a_group() {
        let (s, [a, _, c, ..]) = board();
        let mut sb = Sidebar::default();
        assert!(sb.select_card(&s, c));
        let rows = sb.rows(&s);
        assert!(sb.collapse(&rows));
        assert_eq!(sb.sel, Some(RowKey::Header(Group::Trail(a))));
        let rows = sb.rows(&s);
        assert_eq!(rows.len(), 6, "trail a is down to its header");
        assert!(matches!(rows[0], Row::Header { collapsed: true, .. }));
        assert!(!sb.collapse(&rows), "already collapsed");
        // A card in a collapsed group selects its header.
        assert!(sb.select_card(&s, c));
        assert_eq!(sb.sel, Some(RowKey::Header(Group::Trail(a))));
        assert!(sb.expand(&rows));
        let rows = sb.rows(&s);
        assert_eq!(rows.len(), 10);
        assert!(!sb.expand(&rows), "already expanded");
        sb.toggle(Group::Others);
        assert_eq!(sb.rows(&s).len(), 8);
        assert!(!sb.select_card(&s, NodeId(99)));
    }

    #[test]
    fn with_no_trail_the_cards_are_listed_without_a_header() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.add_page(Rect::new(0.0, 2000.0, 400.0, 300.0), "b");
        let mut sb = Sidebar::default();
        sb.toggle(Group::Others);
        let rows = sb.rows(&s);
        assert_eq!(keys(&rows), [RowKey::Card(a), RowKey::Card(b)], "no header, and not collapsed");
        assert!(sb.collapsed.is_empty());
        assert!(sb.select_card(&s, b));
        assert_eq!(sb.sel, Some(RowKey::Card(b)));
        assert!(!sb.collapse(&rows), "no header to collapse into");
        assert_eq!(sb.sel, Some(RowKey::Card(b)));
        // A trail brings the groups, and the header, back.
        let c = s.spawn_beside(a, "c").unwrap();
        assert_eq!(
            keys(&sb.rows(&s)),
            [RowKey::Header(Group::Trail(a)), RowKey::Card(a), RowKey::Card(c), RowKey::Header(Group::Others), RowKey::Card(b)]
        );
        assert!(Sidebar::default().rows(&Scene::new()).is_empty());
    }

    #[test]
    fn a_collapsed_trail_is_forgotten_when_it_goes() {
        let (mut s, [a, b, c, d, ..]) = board();
        let mut sb = Sidebar::default();
        sb.toggle(Group::Trail(a));
        for id in [a, b, c, d] {
            s.remove(id);
        }
        sb.rows(&s);
        assert!(sb.collapsed.is_empty());
    }

    fn heads(s: &Scene) -> Vec<Group> {
        groups(s).into_iter().map(|(g, _)| g).collect()
    }

    #[test]
    fn pinned_trails_come_first_and_a_pinned_lone_card_is_a_group() {
        use Group::*;
        let (mut s, [a, _, c, _, e, f, g]) = board();
        assert_eq!(heads(&s), [Trail(a), Trail(f), Others]);
        let (old, on) = toggle_pin(&mut s, g);
        assert!(on && old.len() == 1);
        assert_eq!(heads(&s), [Trail(g), Trail(a), Trail(f), Others]);
        // Pinning by any card of a trail pins the whole trail, after g.
        assert!(toggle_pin(&mut s, c).1);
        assert_eq!(heads(&s), [Trail(g), Trail(a), Trail(f), Others]);
        assert!(s.trail_mark(a).pinned);
        // A card opened beside a pinned one is on the pinned trail.
        let h = s.spawn_beside(g, "h").unwrap();
        assert!(s.trail_mark(h).pinned);
        assert_eq!(groups(&s)[0].1.len(), 2);
        // Unpinning the lone card's trail... it now has two cards, so stays a group.
        assert!(!toggle_pin(&mut s, h).1);
        assert_eq!(heads(&s), [Trail(a), Trail(f), Trail(g), Others]);
        s.remove(h);
        assert_eq!(heads(&s), [Trail(a), Trail(f), Others], "g is lone and plain again");
        let rows = Sidebar::default().rows(&s);
        assert!(matches!(&rows[0], Row::Header { mark, .. } if mark.pinned));
        assert!(toggle_pin(&mut s, NodeId(99)).0.is_empty());
        let _ = e;
    }

    #[test]
    fn moving_a_trail_keeps_to_its_part_and_undoes() {
        use Group::*;
        let (mut s, [a, b, _, _, e, f, g]) = board();
        let x = s.spawn_beside(e, "x").unwrap();
        // Trails a, e (e → x), f; g lone.
        assert_eq!(heads(&s), [Trail(a), Trail(e), Trail(f), Others]);
        assert_eq!(position(&s, e), Some((1, 3)));
        let old = move_group(&mut s, f, 0);
        assert_eq!(heads(&s), [Trail(f), Trail(a), Trail(e), Others]);
        assert_eq!(s.trail_mark(b).place, Some(1), "every card of the trail keeps the place");
        assert!(move_group(&mut s, f, 0).is_empty(), "already there");
        assert!(move_group(&mut s, g, 0).is_empty(), "g is on no trail");
        move_group(&mut s, f, 99);
        assert_eq!(heads(&s), [Trail(a), Trail(e), Trail(f), Others], "clamped to the end");
        // A pinned trail moves among the pinned only.
        toggle_pin(&mut s, f);
        toggle_pin(&mut s, x);
        assert_eq!(heads(&s), [Trail(f), Trail(e), Trail(a), Others]);
        move_group(&mut s, e, 0);
        assert_eq!(heads(&s), [Trail(e), Trail(f), Trail(a), Others]);
        assert_eq!(position(&s, a), Some((0, 1)));
        assert_eq!(drop_to(&s, f, Trail(e)), Some(0));
        assert_eq!(drop_to(&s, f, Trail(a)), None, "a is not pinned");
        assert_eq!(drop_to(&s, f, Others), None);
        // A new trail comes after the moved ones.
        let y = s.spawn_beside(g, "y").unwrap();
        assert_eq!(heads(&s), [Trail(e), Trail(f), Trail(a), Trail(g)]);
        let _ = y;
        // Undo puts the marks back as they were before the first move.
        let mut h = crate::History::default();
        h.marked(old);
        h.undo(&mut s);
        assert_eq!(s.trail_mark(a).place, None);
    }

    #[test]
    fn scrolling_keeps_the_highlight_shown() {
        let mut sb = Sidebar::default();
        sb.scroll_to(Some(12), 20, 5);
        assert_eq!(sb.scroll, 8);
        sb.scroll_to(Some(3), 20, 5);
        assert_eq!(sb.scroll, 3);
        sb.scroll_to(Some(5), 20, 5);
        assert_eq!(sb.scroll, 3, "already shown");
        sb.scroll = 18;
        sb.scroll_to(None, 20, 5);
        assert_eq!(sb.scroll, 15, "no empty space below the last row");
        sb.scroll_by(-20, 20, 5);
        assert_eq!(sb.scroll, 0);
        sb.scroll_by(40, 20, 5);
        assert_eq!(sb.scroll, 15);
        sb.scroll_by(1, 3, 5);
        assert_eq!(sb.scroll, 0, "everything fits");
    }
}
