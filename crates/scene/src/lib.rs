//! Board model: cards on an infinite canvas, the edges (trails) between them,
//! and the camera that views them. Pure data — no GPU or browser types — so it
//! can be unit-tested and serialized.

pub mod arrange;
pub mod export;
pub mod fold;
pub mod geom;
pub mod hint;
pub mod history;
pub mod liveness;
pub mod mark;
pub mod nav;
pub mod note;
pub mod palette;
pub mod placement;
pub mod read;
pub mod selection;
pub mod sidebar;
pub mod tidy;
pub mod trail;

use std::collections::BTreeMap;

pub use fold::{Folded, Member, Stack};
pub use geom::{Camera, Rect, Vec2};
pub use history::{Change, Edit, History};
pub use mark::TrailMark;
pub use selection::Selection;
pub use trail::TrailStep;
use serde::{Deserialize, Serialize};

/// Height of a card's title bar, in world units, drawn above the page content.
pub const HEADER_H: f32 = 30.0;

/// Height of a sticky note's grab strip, its only chrome (`note`).
pub const STICKY_GRIP_H: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// Page content area in world units. The title bar sits directly above it.
    pub rect: Rect,
    pub url: String,
    pub title: String,
    /// Stacking order; larger draws on top.
    pub z: u64,
    /// Markdown text of a note card (`note`); `None` for a web page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Name and colour of the trail this card is on (`mark`), kept on every card of it.
    #[serde(default, skip_serializing_if = "TrailMark::is_empty")]
    pub trail: TrailMark,
    /// A note shown as a sticky note: the name of its theme colour (`note`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticky: Option<String>,
}

impl Node {
    /// Height of the chrome above the content: a title bar, or a sticky note's strip.
    pub fn header_h(&self) -> f32 {
        if self.is_sticky() { STICKY_GRIP_H } else { HEADER_H }
    }

    /// Content rect plus title bar: the area the card occupies on the board.
    pub fn frame(&self) -> Rect {
        let h = self.header_h();
        Rect::new(self.rect.x, self.rect.y - h, self.rect.w, self.rect.h + h)
    }

    pub fn header(&self) -> Rect {
        let h = self.header_h();
        Rect::new(self.rect.x, self.rect.y - h, self.rect.w, h)
    }
}

/// A trail edge: `to` was opened from a link on `from`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Scene {
    nodes: BTreeMap<NodeId, Node>,
    edges: Vec<Edge>,
    next_id: u64,
    next_z: u64,
    /// Folded trails (`fold`). Absent in boards saved before stacks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stacks: Vec<Stack>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_page(&mut self, rect: Rect, url: impl Into<String>) -> NodeId {
        self.next_id += 1;
        self.next_z += 1;
        let id = NodeId(self.next_id);
        let url = url.into();
        self.nodes.insert(
            id,
            Node { id, rect, title: url.clone(), url, z: self.next_z, note: None, trail: TrailMark::default(), sticky: None },
        );
        id
    }

    /// Opens `url` as a new card beside `from`, connected by a trail edge.
    pub fn spawn_beside(&mut self, from: NodeId, url: impl Into<String>) -> Option<NodeId> {
        let src = self.nodes.get(&from)?.clone();
        let rect = placement::beside(self, &src);
        let id = self.add_page(rect, url);
        self.nodes.get_mut(&id).expect("just added").trail = src.trail;
        self.edges.push(Edge { from, to: id });
        Some(id)
    }

    /// Removes a card and its trail edges, returning both so the card can be restored.
    pub fn remove(&mut self, id: NodeId) -> Option<(Node, Vec<Edge>)> {
        let node = self.nodes.remove(&id)?;
        self.forget_in_stacks(id, node.rect.pos());
        let (gone, kept) = self.edges.drain(..).partition(|e| e.from == id || e.to == id);
        self.edges = kept;
        Some((node, gone))
    }

    /// Puts back a card removed earlier, under its old id and on top of the
    /// stack, with those of its `edges` whose other end still exists.
    /// Returns false (and changes nothing) if the id is already in use.
    pub fn insert(&mut self, mut node: Node, edges: &[Edge]) -> bool {
        let id = node.id;
        if self.nodes.contains_key(&id) {
            return false;
        }
        self.next_id = self.next_id.max(id.0);
        self.next_z += 1;
        node.z = self.next_z;
        self.nodes.insert(id, node);
        for e in edges {
            let touches = e.from == id || e.to == id;
            if touches && self.nodes.contains_key(&e.from) && self.nodes.contains_key(&e.to) && !self.edges.contains(e) {
                self.edges.push(*e);
            }
        }
        true
    }

    /// Adds an edge as is, even a dangling or looping one (as a hand-edited file could hold).
    #[cfg(test)]
    pub(crate) fn insert_edge(&mut self, e: Edge) {
        self.edges.push(e);
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    /// Cards on the board, in id order: every card but those folded away in a stack.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values().filter(|n| !self.is_hidden(n.id))
    }

    /// Every card, folded away or not.
    pub fn all_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Every trail edge, including those between folded cards.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Edges between two cards on the board (neither folded away).
    pub fn visible_edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(|e| !self.is_hidden(e.from) && !self.is_hidden(e.to))
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Nodes ordered back to front.
    pub fn draw_order(&self) -> Vec<&Node> {
        let mut v: Vec<&Node> = self.nodes().collect();
        v.sort_by_key(|n| n.z);
        v
    }

    pub fn bring_to_front(&mut self, id: NodeId) {
        self.next_z += 1;
        let z = self.next_z;
        if let Some(n) = self.nodes.get_mut(&id) {
            n.z = z;
        }
    }

    /// Topmost card whose frame (title bar + content) contains `p`.
    pub fn topmost_at(&self, p: Vec2) -> Option<NodeId> {
        self.nodes()
            .filter(|n| n.frame().contains(p))
            .max_by_key(|n| n.z)
            .map(|n| n.id)
    }

    /// Bounding box of every card frame, if any.
    pub fn bounds(&self) -> Option<Rect> {
        self.nodes().map(Node::frame).reduce(|a, b| a.union(&b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_beside_adds_edge_and_does_not_overlap() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 800.0, 600.0), "https://a");
        let b = s.spawn_beside(a, "https://b").unwrap();
        let c = s.spawn_beside(a, "https://c").unwrap();
        assert_eq!(s.edges().len(), 2);
        let fa = s.node(a).unwrap().frame();
        let fb = s.node(b).unwrap().frame();
        let fc = s.node(c).unwrap().frame();
        assert!(!fa.intersects(&fb));
        assert!(!fb.intersects(&fc));
        assert!(fb.x > fa.right());
    }

    #[test]
    fn topmost_prefers_higher_z() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 100.0, 100.0), "a");
        let b = s.add_page(Rect::new(50.0, 50.0, 100.0, 100.0), "b");
        assert_eq!(s.topmost_at(Vec2::new(75.0, 75.0)), Some(b));
        s.bring_to_front(a);
        assert_eq!(s.topmost_at(Vec2::new(75.0, 75.0)), Some(a));
        assert_eq!(s.topmost_at(Vec2::new(500.0, 500.0)), None);
    }

    #[test]
    fn remove_drops_edges() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 100.0, 100.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        s.remove(b);
        assert!(s.edges().is_empty());
    }

    #[test]
    fn insert_restores_node_and_live_edges_only() {
        let mut s = Scene::new();
        let a = s.add_page(Rect::new(0.0, 0.0, 100.0, 100.0), "a");
        let b = s.spawn_beside(a, "b").unwrap();
        let c = s.spawn_beside(b, "c").unwrap();
        let (node_b, edges_b) = s.remove(b).unwrap();
        assert_eq!(edges_b.len(), 2);
        s.remove(c);
        assert!(s.insert(node_b.clone(), &edges_b));
        assert_eq!(s.edges(), &[Edge { from: a, to: b }]);
        assert!(!s.insert(node_b, &edges_b), "id already in use");
        // Restored card is on top, and fresh ids never collide with it.
        assert_eq!(s.topmost_at(Vec2::new(200.0, 50.0)), Some(b));
        let d = s.add_page(Rect::default(), "d");
        assert!(d.0 > c.0);
    }
}
