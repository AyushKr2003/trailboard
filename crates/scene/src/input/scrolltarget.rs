//! Scroll targets (M12 step 3): the order of a page's scroll sections and
//! moving between them. Our own design.
//!
//! A page reports its sections (`web::scroll`) as a tree: the page itself at
//! the root, the panes in it, the lists in those. This module puts them in
//! reading order (outer before inner; among neighbours, top to bottom, then
//! left to right), steps through that order (`]s` / `[s`), steps out to the
//! enclosing section (`S`), names each for the mode pill (`page`, `2`,
//! `2.1`), and places `s`'s labels so a nested section's label doesn't hide
//! its parent's. The automatic pick and chaining at an edge need the page's
//! live state, so the page script does those.

use crate::geom::{Rect, Vec2};
use std::collections::HashMap;

/// One section: its number in the page, the section around it (`None`
/// for the page, or a section whose parent isn't known), and where it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    pub id: u32,
    pub parent: Option<u32>,
    pub rect: Rect,
}

/// Neighbours whose tops are this close count as one row (left to right).
const ROW: f32 = 24.0;

fn children(sections: &[Section]) -> (Vec<usize>, HashMap<u32, Vec<usize>>) {
    let known: std::collections::HashSet<u32> = sections.iter().map(|s| s.id).collect();
    let mut roots = Vec::new();
    let mut kids: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, s) in sections.iter().enumerate() {
        match s.parent.filter(|p| known.contains(p) && *p != s.id) {
            Some(p) => kids.entry(p).or_default().push(i),
            None => roots.push(i),
        }
    }
    let by_reading = |v: &mut Vec<usize>| {
        v.sort_by(|&a, &b| {
            let (ra, rb) = (sections[a].rect, sections[b].rect);
            let (ba, bb) = ((ra.y / ROW).floor(), (rb.y / ROW).floor());
            ba.total_cmp(&bb).then(ra.x.total_cmp(&rb.x)).then(sections[a].id.cmp(&sections[b].id))
        })
    };
    by_reading(&mut roots);
    kids.values_mut().for_each(by_reading);
    (roots, kids)
}

/// Every section's id in reading order: outer before inner, neighbours top
/// to bottom and left to right. A section in a loop of parents is left out.
pub fn order(sections: &[Section]) -> Vec<u32> {
    let (roots, kids) = children(sections);
    let mut out = Vec::with_capacity(sections.len());
    let mut stack: Vec<usize> = roots.into_iter().rev().collect();
    while let Some(i) = stack.pop() {
        let id = sections[i].id;
        if out.contains(&id) {
            continue;
        }
        out.push(id);
        if let Some(k) = kids.get(&id) {
            stack.extend(k.iter().rev());
        }
    }
    out
}

/// The next (`forward`) or previous section after `from` in reading order,
/// going round at the ends. From a section not in the list: the first (or
/// last). `None` when there are no sections.
pub fn step(sections: &[Section], from: Option<u32>, forward: bool) -> Option<u32> {
    let o = order(sections);
    if o.is_empty() {
        return None;
    }
    let n = o.len();
    Some(match from.and_then(|f| o.iter().position(|&x| x == f)) {
        Some(i) if forward => o[(i + 1) % n],
        Some(i) => o[(i + n - 1) % n],
        None if forward => o[0],
        None => o[n - 1],
    })
}

/// The section around `id` (`S`), `None` at the outermost one.
pub fn outer(sections: &[Section], id: u32) -> Option<u32> {
    let s = sections.iter().find(|s| s.id == id)?;
    let p = s.parent?;
    sections.iter().any(|x| x.id == p).then_some(p)
}

/// A name for each section, for the mode pill: the outermost ones are
/// `page` (when there is one root) or numbered; inside a section they are
/// numbered from 1 in reading order, joined with dots (`2.1`).
pub fn names(sections: &[Section]) -> HashMap<u32, String> {
    let (roots, kids) = children(sections);
    let mut out = HashMap::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    if roots.len() == 1 {
        stack.push((roots[0], String::new()));
    } else {
        stack.extend(roots.iter().enumerate().map(|(n, &i)| (i, (n + 1).to_string())));
    }
    while let Some((i, name)) = stack.pop() {
        let id = sections[i].id;
        if out.contains_key(&id) {
            continue;
        }
        if let Some(k) = kids.get(&id) {
            for (n, &c) in k.iter().enumerate() {
                let child = if name.is_empty() { (n + 1).to_string() } else { format!("{name}.{}", n + 1) };
                stack.push((c, child));
            }
        }
        out.insert(id, if name.is_empty() { "page".to_string() } else { name });
    }
    out
}

/// Where each label goes, given the sections' rects in label order and the
/// size of a label: at its section's top-left, moved down (then right)
/// past any label placed before it, so a nested section's label sits beside
/// its parent's rather than on it. Kept inside `bounds`.
pub fn label_spots(rects: &[Rect], label: Vec2, bounds: Rect) -> Vec<Vec2> {
    let gap = 2.0;
    let mut placed: Vec<Rect> = Vec::with_capacity(rects.len());
    for r in rects {
        let clamp = |p: Vec2| {
            Vec2::new(
                p.x.min(bounds.right() - label.x).max(bounds.x),
                p.y.min(bounds.bottom() - label.y).max(bounds.y),
            )
        };
        let start = clamp(Vec2::new(r.x + gap, r.y + gap));
        let mut at = start;
        let free = |p: Vec2, placed: &[Rect]| {
            let b = Rect::new(p.x, p.y, label.x, label.y);
            !placed.iter().any(|q| q.intersects(&b))
        };
        // Down the section's left edge first, then along its top.
        let mut tries = 0;
        while !free(at, &placed) && tries < 8 {
            tries += 1;
            at = clamp(Vec2::new(start.x, start.y + tries as f32 * (label.y + gap)));
        }
        let mut tries = 0;
        while !free(at, &placed) && tries < 8 {
            tries += 1;
            at = clamp(Vec2::new(start.x + tries as f32 * (label.x + gap), start.y));
        }
        placed.push(Rect::new(at.x, at.y, label.x, label.y));
    }
    placed.into_iter().map(|r| r.pos()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: u32, parent: Option<u32>, x: f32, y: f32, w: f32, h: f32) -> Section {
        Section { id, parent, rect: Rect::new(x, y, w, h) }
    }

    /// A page with a sidebar (left), a main pane (right) holding a list and
    /// a wide table (below the list), and a carousel in the sidebar.
    fn page() -> Vec<Section> {
        vec![
            s(5, Some(2), 320.0, 400.0, 600.0, 200.0), // table, in main, lower
            s(0, None, 0.0, 0.0, 1280.0, 720.0), // page
            s(2, Some(0), 300.0, 0.0, 980.0, 720.0), // main pane
            s(1, Some(0), 0.0, 4.0, 300.0, 720.0),   // sidebar (same row as main)
            s(4, Some(2), 320.0, 40.0, 600.0, 300.0), // list, in main
            s(3, Some(1), 10.0, 300.0, 280.0, 120.0), // carousel, in sidebar
        ]
    }

    #[test]
    fn reading_order_is_outer_first_then_rows() {
        assert_eq!(order(&page()), vec![0, 1, 3, 2, 4, 5]);
    }

    #[test]
    fn steps_go_round() {
        let p = page();
        assert_eq!(step(&p, Some(0), true), Some(1));
        assert_eq!(step(&p, Some(3), true), Some(2));
        assert_eq!(step(&p, Some(5), true), Some(0));
        assert_eq!(step(&p, Some(0), false), Some(5));
        assert_eq!(step(&p, Some(99), true), Some(0));
        assert_eq!(step(&p, None, false), Some(5));
        assert_eq!(step(&[], Some(1), true), None);
    }

    #[test]
    fn stepping_out() {
        let p = page();
        assert_eq!(outer(&p, 4), Some(2));
        assert_eq!(outer(&p, 2), Some(0));
        assert_eq!(outer(&p, 0), None);
        assert_eq!(outer(&p, 42), None);
        // A parent that isn't listed is no step.
        assert_eq!(outer(&[s(7, Some(9), 0.0, 0.0, 9.0, 9.0)], 7), None);
    }

    #[test]
    fn names_follow_the_tree() {
        let n = names(&page());
        assert_eq!(n[&0], "page");
        assert_eq!(n[&1], "1");
        assert_eq!(n[&3], "1.1");
        assert_eq!(n[&2], "2");
        assert_eq!(n[&4], "2.1");
        assert_eq!(n[&5], "2.2");
    }

    #[test]
    fn orphans_and_loops_are_safe() {
        // No page: the roots are numbered; a parent loop doesn't hang.
        let v = vec![s(1, None, 0.0, 0.0, 10.0, 10.0), s(2, Some(77), 0.0, 50.0, 10.0, 10.0), s(3, Some(4), 0.0, 0.0, 5.0, 5.0), s(4, Some(3), 0.0, 0.0, 5.0, 5.0)];
        let o = order(&v);
        assert_eq!(o, vec![1, 2]);
        let n = names(&v);
        assert_eq!((n[&1].as_str(), n[&2].as_str()), ("1", "2"));
    }

    #[test]
    fn nested_labels_step_aside() {
        let bounds = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let label = Vec2::new(20.0, 16.0);
        // The page, a pane at the same corner, a list at the same corner too.
        let rects = [Rect::new(0.0, 0.0, 1000.0, 800.0), Rect::new(0.0, 0.0, 500.0, 800.0), Rect::new(0.0, 0.0, 400.0, 300.0), Rect::new(600.0, 100.0, 200.0, 200.0)];
        let spots = label_spots(&rects, label, bounds);
        assert_eq!(spots[0], Vec2::new(2.0, 2.0));
        assert_eq!(spots[1], Vec2::new(2.0, 20.0));
        assert_eq!(spots[2], Vec2::new(2.0, 38.0));
        assert_eq!(spots[3], Vec2::new(602.0, 102.0));
        // Kept inside the bounds.
        let edge = label_spots(&[Rect::new(990.0, 790.0, 50.0, 50.0)], label, bounds);
        assert_eq!(edge[0], Vec2::new(980.0, 784.0));
    }
}
