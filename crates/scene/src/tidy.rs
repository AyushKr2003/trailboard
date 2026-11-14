//! Tidy: lay the board out again so trails read left to right and nothing
//! overlaps. Our own rule (Marble's `tools::tidy` was not decompiled):
//!
//! - Each trail (a tree of cards joined by edges) is drawn with its root on the
//!   left and each generation in its own column, as wide as the widest card of
//!   that generation in the trail. A card sits level with the top of the band
//!   its descendants fill, so a first child lines up with its source, as when
//!   it was opened beside it.
//! - Siblings keep their current top-to-bottom order; trails are stacked top to
//!   bottom in the order of their roots.
//! - Lone cards (no edges) go in rows below the trails, wrapping at the widest
//!   trail or at about √n cards, whichever is wider.
//! - The layout starts at the top-left corner of the board as it was. Card sizes
//!   never change.

use std::collections::{BTreeMap, BTreeSet};

use crate::placement::GAP;
use crate::{Node, NodeId, Rect, Scene, Vec2, HEADER_H};

/// Extra space between two trails, on top of [`GAP`].
pub const TRAIL_GAP: f32 = GAP;

/// New content rects for every card, in id order. Cards that already sit where
/// Tidy would put them are included unchanged.
pub fn tidy(scene: &Scene) -> Vec<(NodeId, Rect)> {
    let Some(bounds) = scene.bounds() else { return Vec::new() };
    let nodes: BTreeMap<NodeId, &Node> = scene.nodes().map(|n| (n.id, n)).collect();

    // One parent per card: the first edge into it from a card that exists.
    let mut parent: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    for e in scene.edges() {
        if e.from != e.to && nodes.contains_key(&e.from) && nodes.contains_key(&e.to) {
            parent.entry(e.to).or_insert(e.from);
        }
    }
    let mut children: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for (&child, &p) in &parent {
        children.entry(p).or_default().push(child);
    }
    for list in children.values_mut() {
        list.sort_by(|a, b| reading_order(nodes[a], nodes[b]));
    }

    // Roots: cards without a parent. A cycle has none, so its lowest id stands in.
    let mut roots: Vec<NodeId> = nodes.keys().copied().filter(|id| !parent.contains_key(id)).collect();
    let mut seen: BTreeSet<NodeId> = BTreeSet::new();
    let mut trees: Vec<Vec<NodeId>> = Vec::new(); // each tree in pre-order, root first
    let mut i = 0;
    loop {
        if i == roots.len() {
            match nodes.keys().find(|id| !seen.contains(id)) {
                Some(&id) => roots.push(id),
                None => break,
            }
        }
        let root = roots[i];
        i += 1;
        if seen.contains(&root) {
            continue;
        }
        let mut order = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            order.push(id);
            if let Some(kids) = children.get(&id) {
                stack.extend(kids.iter().rev().filter(|k| !seen.contains(k)));
            }
        }
        trees.push(order);
    }
    trees.sort_by(|a, b| reading_order(nodes[&a[0]], nodes[&b[0]]));

    let mut out: BTreeMap<NodeId, Rect> = BTreeMap::new();
    let origin = Vec2::new(bounds.x.round(), bounds.y.round());
    let mut y = origin.y;
    let mut widest = 0.0f32;
    let (trails, lone): (Vec<_>, Vec<_>) = trees.into_iter().partition(|t| t.len() > 1);
    for tree in &trails {
        let (w, h) = layout_tree(tree, &nodes, &parent, Vec2::new(origin.x, y), &mut out);
        widest = widest.max(w);
        y += h + GAP + TRAIL_GAP;
    }

    // Lone cards in rows.
    let lone: Vec<&Node> = lone.iter().map(|t| nodes[&t[0]]).collect();
    if !lone.is_empty() {
        let per_row = (lone.len() as f32).sqrt().ceil() as usize;
        let grid_w = lone.iter().take(per_row).map(|n| n.rect.w).sum::<f32>() + GAP * (per_row - 1) as f32;
        let limit = widest.max(grid_w);
        let (mut x, mut row_h) = (origin.x, 0.0f32);
        for n in lone {
            let f = n.frame();
            if x > origin.x && x - origin.x + f.w > limit + 0.5 {
                x = origin.x;
                y += row_h + GAP;
                row_h = 0.0;
            }
            out.insert(n.id, Rect::new(x, y + HEADER_H, n.rect.w, n.rect.h));
            x += f.w + GAP;
            row_h = row_h.max(f.h);
        }
    }
    out.into_iter().collect()
}

/// Lays out one trail (pre-order, root first) with its top-left frame corner at
/// `at`. Returns the trail's width and height.
fn layout_tree(
    tree: &[NodeId],
    nodes: &BTreeMap<NodeId, &Node>,
    parent: &BTreeMap<NodeId, NodeId>,
    at: Vec2,
    out: &mut BTreeMap<NodeId, Rect>,
) -> (f32, f32) {
    // Depth of each card, and column widths per depth.
    let mut depth: BTreeMap<NodeId, usize> = BTreeMap::new();
    let mut col_w: Vec<f32> = Vec::new();
    for &id in tree {
        let d = match parent.get(&id) {
            Some(p) if id != tree[0] => depth[p] + 1,
            _ => 0,
        };
        depth.insert(id, d);
        if col_w.len() <= d {
            col_w.push(0.0);
        }
        col_w[d] = col_w[d].max(nodes[&id].rect.w);
    }
    let mut col_x = Vec::with_capacity(col_w.len());
    let mut x = at.x;
    for w in &col_w {
        col_x.push(x);
        x += w + GAP;
    }
    let width = x - GAP - at.x;

    // Height of each card's band: its frame or its children's bands, whichever is taller.
    // Post-order is pre-order reversed with children after parents, so walk it backwards.
    let mut kids: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for &id in &tree[1..] {
        kids.entry(parent[&id]).or_default().push(id);
    }
    let mut band: BTreeMap<NodeId, f32> = BTreeMap::new();
    for &id in tree.iter().rev() {
        let below = kids.get(&id).map_or(0.0, |k| k.iter().map(|c| band[c]).sum::<f32>() + GAP * (k.len() - 1) as f32);
        band.insert(id, nodes[&id].frame().h.max(below));
    }

    // Place top-down: each card at the top of its band, children stacked from there.
    let mut top: BTreeMap<NodeId, f32> = BTreeMap::from([(tree[0], at.y)]);
    for &id in tree {
        let n = nodes[&id];
        let y = top[&id];
        out.insert(id, Rect::new(col_x[depth[&id]], y + HEADER_H, n.rect.w, n.rect.h));
        let mut cy = y;
        for c in kids.get(&id).into_iter().flatten() {
            top.insert(*c, cy);
            cy += band[c] + GAP;
        }
    }
    (width, band[&tree[0]])
}

/// Top to bottom, then left to right, then oldest first.
fn reading_order(a: &Node, b: &Node) -> std::cmp::Ordering {
    a.rect.y.total_cmp(&b.rect.y).then(a.rect.x.total_cmp(&b.rect.x)).then(a.id.cmp(&b.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Edge;

    fn apply(s: &mut Scene, rects: &[(NodeId, Rect)]) {
        for (id, r) in rects {
            s.node_mut(*id).unwrap().rect = *r;
        }
    }

    fn no_overlaps(s: &Scene) {
        let frames: Vec<_> = s.nodes().map(|n| (n.id, n.frame())).collect();
        for (i, (a, fa)) in frames.iter().enumerate() {
            for (b, fb) in &frames[i + 1..] {
                assert!(!fa.inflate(GAP / 2.0 - 1.0).intersects(fb), "{a:?} overlaps {b:?}: {fa:?} {fb:?}");
            }
        }
    }

    #[test]
    fn empty_board() {
        assert!(tidy(&Scene::new()).is_empty());
    }

    #[test]
    fn trail_goes_left_to_right_in_columns() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(500.0, 900.0, 800.0, 600.0), "a");
        // Scatter the children, wide one first in reading order.
        let b = s.add_page(Rect::new(-3000.0, 100.0, 1000.0, 400.0), "b");
        let c = s.add_page(Rect::new(4000.0, 2000.0, 600.0, 300.0), "c");
        let d = s.add_page(Rect::new(0.0, 5000.0, 300.0, 300.0), "d");
        s.insert_edge(Edge { from: a, to: b });
        s.insert_edge(Edge { from: a, to: c });
        s.insert_edge(Edge { from: c, to: d });
        let before_bounds = s.bounds().unwrap();
        let r = tidy(&s);
        apply(&mut s, &r);
        no_overlaps(&s);
        let n = |id| s.node(id).unwrap().rect;
        // Origin is the old top-left corner.
        assert_eq!(n(a).x, before_bounds.x.round());
        assert_eq!(n(a).y - HEADER_H, before_bounds.y.round());
        // Columns: root, then children, then grandchildren after the widest child.
        assert_eq!(n(b).x, n(a).right() + GAP);
        assert_eq!(n(c).x, n(b).x);
        assert_eq!(n(d).x, n(b).x + 1000.0 + GAP);
        // First child level with its source; siblings keep their order.
        assert_eq!(n(b).y, n(a).y);
        assert!(n(c).y > n(b).bottom());
        assert_eq!(n(d).y, n(c).y);
        // Sizes unchanged.
        assert_eq!((n(b).w, n(b).h), (1000.0, 400.0));
    }

    #[test]
    fn subtrees_do_not_collide() {
        // a → b, c; b has three tall children, so c must go below them.
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        let c = s.spawn_beside(a, "c").unwrap();
        for _ in 0..3 {
            s.spawn_beside(b, "x").unwrap();
        }
        s.spawn_beside(c, "y").unwrap();
        // Shuffle everything so the result can't be the spawn layout by accident.
        let ids: Vec<NodeId> = s.nodes().map(|n| n.id).collect();
        for (k, id) in ids.iter().enumerate() {
            let r = &mut s.node_mut(*id).unwrap().rect;
            r.x = (k as f32 * 37.0) % 200.0;
            r.y = k as f32 * 10.0 + if *id == c { 1000.0 } else { 0.0 };
        }
        let r = tidy(&s);
        apply(&mut s, &r);
        no_overlaps(&s);
        let n = |id| s.node(id).unwrap().rect;
        assert!(n(c).y > n(b).y);
    }

    #[test]
    fn trails_stack_and_lone_cards_wrap_below() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 400.0, 300.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        let p = s.add_page(Rect::new(0.0, -2000.0, 400.0, 300.0), "p");
        let q = s.spawn_beside(p, "q").unwrap();
        let lone: Vec<NodeId> = (0..5).map(|k| s.add_page(Rect::new(k as f32 * 3000.0, 9000.0, 400.0, 300.0), "l")).collect();
        let r = tidy(&s);
        apply(&mut s, &r);
        no_overlaps(&s);
        let n = |id| s.node(id).unwrap().rect;
        // p's trail was higher up, so it comes first.
        assert!(n(p).y < n(a).y);
        assert_eq!(n(a).x, n(p).x);
        assert_eq!(n(b).y, n(a).y);
        assert_eq!(n(q).y, n(p).y);
        // Lone cards are below both trails, in rows of ⌈√5⌉ = 3 (wider than the
        // two-card trails), keeping their left-to-right order.
        let trails_bottom = n(a).bottom().max(n(b).bottom());
        assert!(lone.iter().all(|id| n(*id).y - HEADER_H > trails_bottom));
        assert_eq!(n(lone[0]).y, n(lone[2]).y);
        assert!(n(lone[0]).x < n(lone[1]).x && n(lone[1]).x < n(lone[2]).x);
        assert!(n(lone[3]).y > n(lone[0]).bottom());
        assert_eq!(n(lone[3]).x, n(lone[0]).x);
    }

    #[test]
    fn idempotent() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(10.0, 10.0, 400.0, 300.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        s.spawn_beside(b, "c").unwrap();
        s.spawn_beside(a, "d").unwrap();
        s.add_page(Rect::new(-500.0, 2000.0, 200.0, 200.0), "e");
        let first = tidy(&s);
        apply(&mut s, &first);
        assert_eq!(tidy(&s), first);
    }

    #[test]
    fn cycles_and_dangling_edges_still_place_every_card() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 100.0, 100.0), "a");
        let b = s.add_page(Rect::new(0.0, 0.0, 100.0, 100.0), "b");
        let c = s.add_page(Rect::new(0.0, 0.0, 100.0, 100.0), "c");
        s.insert_edge(Edge { from: a, to: b });
        s.insert_edge(Edge { from: b, to: a });
        s.insert_edge(Edge { from: c, to: c });
        s.insert_edge(Edge { from: NodeId(99), to: c });
        let r = tidy(&s);
        assert_eq!(r.len(), 3);
        apply(&mut s, &r);
        no_overlaps(&s);
    }
}
