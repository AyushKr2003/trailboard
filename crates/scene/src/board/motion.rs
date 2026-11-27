//! Quiet motion on the board: cards glide to new places (Tidy), new cards
//! grow in from where they came from, closed cards shrink and fade, a
//! trail's cards slide into their stack as it folds, each becoming one of
//! the blank sheets drawn behind its front, and back out of those sheets as
//! it unfolds, and a space switch slides the board in. Everything here is pure: where a card
//! is drawn and how strongly, given when its motion started. The model
//! keeps the card's real rect throughout; only the drawing moves.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::{NodeId, Rect, Vec2};

/// How long a card takes to glide or grow into place.
pub const GLIDE: Duration = Duration::from_millis(220);
/// How long a trail's cards take to slide into their stack (fold) or out
/// to their places (unfold): longer than a glide, as they travel further.
pub const FOLD: Duration = Duration::from_millis(340);
/// How long a closed card takes to shrink away.
pub const CLOSE: Duration = Duration::from_millis(160);
/// How long a space switch takes: the tint cross-fades, the board slides in.
pub const SPACE: Duration = Duration::from_millis(240);
/// How far a new space's board slides in from, in screen px (× UI scale).
pub const SPACE_SLIDE: f32 = 32.0;
/// How long the sidebar takes to slide out.
pub const SIDEBAR_OUT: Duration = Duration::from_millis(150);

/// Cubic ease-out over 0..1, clamped.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease-in-out over 0..1, clamped: starts and ends gently, for
/// motions that cross much of the view (fold, unfold).
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// Every motion lasts this many times longer (bits of an `f32`; 1.0 is
/// 0x3f80_0000). Set from `TRAILBOARD_SLOW_MOTION` to watch or test them.
static SLOW: AtomicU32 = AtomicU32::new(0x3f80_0000);

/// Makes every motion `k` times slower (1 is normal).
pub fn set_slow(k: f32) {
    SLOW.store(k.clamp(0.1, 100.0).to_bits(), Ordering::Relaxed);
}

/// Raw progress (0..1, not eased) of something that started at `start`
/// and lasts `len`, at `now`.
pub fn progress(start: Instant, len: Duration, now: Instant) -> f32 {
    let len = len.as_secs_f32() * f32::from_bits(SLOW.load(Ordering::Relaxed));
    (now.saturating_duration_since(start).as_secs_f32() / len.max(1e-6)).clamp(0.0, 1.0)
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Rect::new(l(a.x, b.x), l(a.y, b.y), l(a.w, b.w), l(a.h, b.h))
}

/// `r` scaled by `k` about the point at fraction `(fx, fy)` of it (0.5, 0.5
/// is its middle; 0, 0.5 the middle of its left edge).
pub fn scale_about(r: Rect, k: f32, fx: f32, fy: f32) -> Rect {
    let p = Vec2::new(r.x + r.w * fx, r.y + r.h * fy);
    Rect::new(p.x - r.w * k * fx, p.y - r.h * k * fy, r.w * k, r.h * k)
}

/// Where a card opened beside `src` starts growing from: a little smaller,
/// pinned on the side that faces its source and pulled toward it, so it
/// seems to come out of the page that opened it.
pub fn open_from(src: Rect, card: Rect) -> Rect {
    let right = card.center().x >= src.center().x;
    let from = scale_about(card, 0.85, if right { 0.0 } else { 1.0 }, 0.0);
    let gap = if right { card.x - src.right() } else { src.x - card.right() };
    let pull = gap.clamp(0.0, 48.0) * if right { -1.0 } else { 1.0 };
    from.translate(Vec2::new(pull, 0.0))
}

/// Where a card with no source starts growing from: a little smaller, in
/// its own middle.
pub fn grow_from(card: Rect) -> Rect {
    scale_about(card, 0.92, 0.5, 0.5)
}

/// How many cards a stack shows behind its front, as blank sheets.
pub const STACK_SHOWN: usize = 3;

/// The sheet of the `j`-th card behind a stack's front (0: the next one):
/// 1, 2 or 3; cards further back share the last.
pub fn slot_behind(j: usize) -> usize {
    (j + 1).min(STACK_SHOWN)
}

/// Where sheet `slot` of a stack is: the front's rect moved `step` down and
/// right per slot (world units; the drawing keeps `step` a few px on screen).
pub fn stack_slot(front: Rect, slot: usize, step: f32) -> Rect {
    front.translate(Vec2::new(step * slot as f32, step * slot as f32))
}

/// How much of a page shows on a card `e` (eased, 0..1) of the way into its
/// stack: all of it at first, then fading out over the second half, so the
/// card arrives as a blank sheet.
fn page_folding(e: f32) -> f32 {
    1.0 - ease_out(((e - 0.4) / 0.6).clamp(0.0, 1.0))
}

/// A card folding into its stack `t` (0..1, raw) of the way: its rect,
/// sliding from `from` (its place) to `slot` (its sheet behind the front,
/// [`stack_slot`]), and how much of its page shows over the blank sheet.
pub fn folding(from: Rect, slot: Rect, t: f32) -> (Rect, f32) {
    let e = ease_in_out(t);
    (lerp_rect(from, slot, e), page_folding(e))
}

/// A closed card `t` (0..1, raw) of the way out: its rect, shrinking
/// toward its middle, and its strength, fading.
pub fn closing(card: Rect, t: f32) -> (Rect, f32) {
    let e = ease_out(t);
    (scale_about(card, 1.0 - 0.08 * e, 0.5, 0.5), 1.0 - e)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Glide {
    from: Rect,
    start: Instant,
    /// Fades in as it moves (a new card), else stays solid (a moved one).
    fade: bool,
    len: Duration,
    /// Comes out of a stack's sheet (unfold): eased in and out, drawn over
    /// a blank sheet while its page fades in; else eased out.
    unfold: bool,
}

/// Cards on their way somewhere, by id.
#[derive(Debug, Default)]
pub struct Glides {
    items: HashMap<NodeId, Glide>,
}

impl Glides {
    /// Card `id` starts moving from `from` (world units) to wherever it is.
    /// A card already moving starts again from `from`.
    pub fn start(&mut self, id: NodeId, from: Rect, fade: bool, now: Instant) {
        self.items.insert(id, Glide { from, start: now, fade, len: GLIDE, unfold: false });
    }

    /// Card `id` slides out of its stack's sheet at `from` ([`stack_slot`])
    /// to its place over [`FOLD`], eased in and out: a blank sheet whose
    /// page fades in on the way (the reverse of [`folding`]).
    pub fn unfold(&mut self, id: NodeId, from: Rect, now: Instant) {
        self.items.insert(id, Glide { from, start: now, fade: true, len: FOLD, unfold: true });
    }

    /// Whether card `id` is coming out of a stack now: draw a blank sheet
    /// under it (where [`Glides::at`] puts it) as its page fades in.
    pub fn unfolding(&self, id: NodeId, now: Instant) -> bool {
        self.items.get(&id).is_some_and(|g| g.unfold && progress(g.start, g.len, now) < 1.0)
    }

    /// Where card `id`, really at `to`, is drawn now and how strongly; `None`
    /// once it has arrived (draw it at `to`, solid).
    pub fn at(&self, id: NodeId, to: Rect, now: Instant) -> Option<(Rect, f32)> {
        let g = self.items.get(&id)?;
        let t = progress(g.start, g.len, now);
        if t >= 1.0 {
            return None;
        }
        if g.unfold {
            // The fold run backwards: ease_in_out(1 - t) = 1 - ease_in_out(t).
            let e = ease_in_out(t);
            return Some((lerp_rect(g.from, to, e), page_folding(1.0 - e)));
        }
        let e = ease_out(t);
        Some((lerp_rect(g.from, to, e), if g.fade { e.max(0.0) } else { 1.0 }))
    }

    /// Some card is still moving: keep drawing frames.
    pub fn busy(&self, now: Instant) -> bool {
        self.items.values().any(|g| progress(g.start, g.len, now) < 1.0)
    }

    /// Forgets cards that have arrived.
    pub fn settle(&mut self, now: Instant) {
        self.items.retain(|_, g| progress(g.start, g.len, now) < 1.0);
    }

    /// Card `id` stops wherever it was going (it is drawn where it is).
    pub fn forget(&mut self, id: NodeId) {
        self.items.remove(&id);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glide_runs_from_its_start_to_the_card_and_ends() {
        let t0 = Instant::now();
        let mut g = Glides::default();
        let (from, to) = (Rect::new(0.0, 0.0, 100.0, 50.0), Rect::new(200.0, 100.0, 100.0, 50.0));
        g.start(NodeId(1), from, false, t0);
        assert_eq!(g.at(NodeId(1), to, t0), Some((from, 1.0)));
        let (mid, a) = g.at(NodeId(1), to, t0 + GLIDE / 2).unwrap();
        assert!(mid.x > 100.0 && mid.x < 200.0, "eased out: past halfway at half time ({mid:?})");
        assert_eq!(a, 1.0);
        assert!(g.busy(t0 + GLIDE / 2));
        assert_eq!(g.at(NodeId(1), to, t0 + GLIDE), None);
        assert!(!g.busy(t0 + GLIDE));
        g.settle(t0 + GLIDE);
        assert!(g.items.is_empty());
        assert_eq!(g.at(NodeId(2), to, t0), None, "never moved");
    }

    #[test]
    fn a_new_card_fades_in_as_it_grows() {
        let t0 = Instant::now();
        let mut g = Glides::default();
        let card = Rect::new(0.0, 0.0, 100.0, 100.0);
        g.start(NodeId(1), grow_from(card), true, t0);
        let (r, a) = g.at(NodeId(1), card, t0).unwrap();
        assert_eq!(a, 0.0);
        assert!((r.center().x - 50.0).abs() < 1e-3 && r.w < 100.0);
        let (_, a) = g.at(NodeId(1), card, t0 + GLIDE / 2).unwrap();
        assert!(a > 0.5 && a < 1.0);
    }

    #[test]
    fn a_card_opened_beside_grows_out_of_its_source() {
        let src = Rect::new(0.0, 0.0, 400.0, 300.0);
        let right = Rect::new(500.0, 0.0, 400.0, 300.0);
        let f = open_from(src, right);
        assert!(f.w < right.w);
        assert_eq!(f.x, right.x - 48.0, "pulled toward the source, at most 48");
        assert_eq!(f.y, right.y);
        // Opened on the left: pinned on its right side.
        let left = Rect::new(-450.0, 0.0, 400.0, 300.0);
        let f = open_from(src, left);
        assert!((f.right() - (left.right() + 48.0)).abs() < 1e-3, "{f:?}");
    }

    #[test]
    fn ease_in_out_is_gentle_at_both_ends() {
        assert_eq!((ease_in_out(0.0), ease_in_out(0.5), ease_in_out(1.0)), (0.0, 0.5, 1.0));
        assert!(ease_in_out(0.1) < 0.1 && ease_in_out(0.9) > 0.9);
        let mut last = 0.0;
        for i in 1..=100 {
            let v = ease_in_out(i as f32 / 100.0);
            assert!(v >= last, "never goes back");
            last = v;
        }
    }

    #[test]
    fn cards_behind_a_front_take_its_three_sheets() {
        assert_eq!((0..6).map(slot_behind).collect::<Vec<_>>(), vec![1, 2, 3, 3, 3, 3]);
        let front = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(stack_slot(front, 2, 4.0), Rect::new(18.0, 28.0, 100.0, 50.0));
    }

    #[test]
    fn a_folding_card_slides_onto_its_sheet_and_arrives_blank() {
        let (place, sheet) = (Rect::new(300.0, 0.0, 100.0, 50.0), Rect::new(12.0, 12.0, 120.0, 60.0));
        assert_eq!(folding(place, sheet, 0.0), (place, 1.0));
        let (r, a) = folding(place, sheet, 0.3);
        assert!(r.x < 300.0 && a == 1.0, "moving, page still all there: {r:?} {a}");
        let (mid, a) = folding(place, sheet, 0.5);
        assert!((mid.x - 156.0).abs() < 1e-3 && a > 0.3 && a < 1.0, "{mid:?} {a}");
        assert_eq!(folding(place, sheet, 1.0), (sheet, 0.0));
    }

    #[test]
    fn an_unfolding_card_is_the_fold_backwards() {
        let t0 = Instant::now();
        let mut g = Glides::default();
        let (sheet, place) = (Rect::new(12.0, 12.0, 100.0, 50.0), Rect::new(300.0, 0.0, 100.0, 50.0));
        g.unfold(NodeId(1), sheet, t0);
        assert!(g.unfolding(NodeId(1), t0) && !g.unfolding(NodeId(2), t0));
        assert_eq!(g.at(NodeId(1), place, t0), Some((sheet, 0.0)), "starts as the blank sheet");
        for i in 1..10 {
            let t = i as f32 / 10.0;
            let (r, a) = g.at(NodeId(1), place, t0 + FOLD.mul_f32(t)).unwrap();
            let (r2, a2) = folding(place, sheet, 1.0 - t);
            assert!((r.x - r2.x).abs() < 0.5 && (a - a2).abs() < 0.02, "t={t}: {r:?} {a} vs {r2:?} {a2}");
        }
        assert!(g.at(NodeId(1), place, t0 + GLIDE).is_some(), "lasts longer than a glide");
        assert_eq!(g.at(NodeId(1), place, t0 + FOLD), None);
        assert!(!g.unfolding(NodeId(1), t0 + FOLD));
    }

    #[test]
    fn a_closed_card_shrinks_a_little_and_fades() {
        let card = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(closing(card, 0.0), (card, 1.0));
        let (r, a) = closing(card, 1.0);
        assert_eq!(a, 0.0);
        assert!((r.w - 92.0).abs() < 1e-3 && (r.center().x - 50.0).abs() < 1e-3);
    }
}
