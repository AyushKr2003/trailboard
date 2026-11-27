//! The way a trail's line runs between two cards: straight across out of
//! the source's side, straight up or down halfway between the cards, and
//! straight across into the target, turning at right angles. Each turn is
//! rounded with the radius the cards' corners use (the *Corners* setting,
//! at the zoom shown), so a trail looks made of the same stuff as the
//! cards; with square corners the turns are square too. Owner's decision,
//! 2026-11-21 (before, a trail was an S-curve). Our own design.

use crate::Vec2;

/// Points along a rounded turn (more look no smoother at any size we draw).
const ARC_STEPS: usize = 6;

/// The line from `from` to `to` (screen or board px, any units alike) as
/// points to join: across, along, across, each turn a quarter circle of
/// `radius`, made smaller where the line has less room than that. On one
/// level the line is straight.
pub fn elbow(from: Vec2, to: Vec2, radius: f32) -> Vec<Vec2> {
    let dy = to.y - from.y;
    if dy.abs() < 0.5 {
        return vec![from, to];
    }
    let mid = (from.x + to.x) / 2.0;
    let sx = if to.x >= from.x { 1.0 } else { -1.0 };
    let sy = dy.signum();
    // Room for each turn: half the way across, half the way down.
    let r = radius.max(0.0).min((mid - from.x).abs()).min(dy.abs() / 2.0);
    let mut out = vec![from];
    // First turn: from across to along. Its centre sits inside the corner.
    let c1 = Vec2::new(mid - sx * r, from.y + sy * r);
    for i in 0..=ARC_STEPS {
        let a = i as f32 / ARC_STEPS as f32 * std::f32::consts::FRAC_PI_2;
        out.push(Vec2::new(c1.x + sx * r * a.sin(), c1.y - sy * r * a.cos()));
    }
    // Second turn: from along to across, into the target.
    let c2 = Vec2::new(mid + sx * r, to.y - sy * r);
    for i in 0..=ARC_STEPS {
        let a = i as f32 / ARC_STEPS as f32 * std::f32::consts::FRAC_PI_2;
        out.push(Vec2::new(c2.x - sx * r * a.cos(), c2.y + sy * r * a.sin()));
    }
    out.push(to);
    // A turn of no radius is one point, not seven.
    out.dedup_by(|a, b| (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4);
    out
}

/// The first `t` (0 to 1) of `path` by length: the line as far as it has
/// drawn in (a trail just joined grows from its source). The whole path for
/// `t` ≥ 1; just its start for `t` ≤ 0.
pub fn part(path: &[Vec2], t: f32) -> Vec<Vec2> {
    let Some(&first) = path.first() else { return Vec::new() };
    if t >= 1.0 {
        return path.to_vec();
    }
    let len = |a: Vec2, b: Vec2| (b.x - a.x).hypot(b.y - a.y);
    let total: f32 = path.windows(2).map(|w| len(w[0], w[1])).sum();
    let mut left = total * t.max(0.0);
    let mut out = vec![first];
    for w in path.windows(2) {
        let l = len(w[0], w[1]);
        if l >= left {
            let k = if l > 0.0 { left / l } else { 0.0 };
            out.push(Vec2::new(w[0].x + (w[1].x - w[0].x) * k, w[0].y + (w[1].y - w[0].y) * k));
            return out;
        }
        left -= l;
        out.push(w[1]);
    }
    out
}

/// `path` cut into dashes `dash` long with `gap` between them, measured
/// along the path (so a dash can run round a turn as two pieces): the
/// segments to draw, in order. A link between trails is drawn so.
pub fn dashes(path: &[Vec2], dash: f32, gap: f32) -> Vec<(Vec2, Vec2)> {
    let mut out = Vec::new();
    if dash <= 0.0 {
        return out;
    }
    let period = dash + gap.max(0.0);
    let mut along = 0.0_f32; // distance from the path's start to the segment's start
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b.x - a.x).hypot(b.y - a.y);
        if len <= 0.0 {
            continue;
        }
        let at = |t: f32| Vec2::new(a.x + (b.x - a.x) * t / len, a.y + (b.y - a.y) * t / len);
        // Walk the dashes that overlap this segment.
        let mut start = (along / period).floor() * period;
        while start < along + len {
            let (s0, s1) = ((start - along).max(0.0), (start + dash - along).min(len));
            if s1 > s0 {
                out.push((at(s0), at(s1)));
            }
            start += period;
        }
        along += len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashes_run_along_the_path_and_round_turns() {
        let p = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0)];
        let d = dashes(&p, 4.0, 2.0);
        // 0–4, 6–10, then 12–16 is 2–6 down the second run, 18–20 is 8–10.
        assert_eq!(d.len(), 4, "{d:?}");
        assert!(close(d[0].0, Vec2::new(0.0, 0.0)) && close(d[0].1, Vec2::new(4.0, 0.0)));
        assert!(close(d[1].0, Vec2::new(6.0, 0.0)) && close(d[1].1, Vec2::new(10.0, 0.0)));
        assert!(close(d[2].0, Vec2::new(10.0, 2.0)) && close(d[2].1, Vec2::new(10.0, 6.0)));
        assert!(close(d[3].0, Vec2::new(10.0, 8.0)) && close(d[3].1, Vec2::new(10.0, 10.0)));
        assert!(dashes(&p, 0.0, 2.0).is_empty());
    }

    #[test]
    fn part_draws_the_line_in_by_length() {
        let p = elbow(Vec2::new(0.0, 0.0), Vec2::new(100.0, 60.0), 0.0); // 50 + 60 + 50 = 160 long
        assert_eq!(part(&p, 1.0), p);
        assert_eq!(part(&p, 0.0), vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0)]);
        let q = part(&p, 0.25); // 40 along the first run
        assert_eq!(q.len(), 2);
        assert!(close(q[1], Vec2::new(40.0, 0.0)), "{q:?}");
        let q = part(&p, 0.5); // 80: 30 down the middle run
        assert_eq!(q.len(), 3);
        assert!(close(q[1], Vec2::new(50.0, 0.0)) && close(q[2], Vec2::new(50.0, 30.0)), "{q:?}");
        assert!(part(&[], 0.5).is_empty());
    }

    fn close(a: Vec2, b: Vec2) -> bool {
        (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
    }

    #[test]
    fn square_corners_turn_at_right_angles_halfway() {
        let p = elbow(Vec2::new(0.0, 0.0), Vec2::new(100.0, 60.0), 0.0);
        assert_eq!(p.len(), 4, "{p:?}");
        for (a, b) in [(p[0], (0.0, 0.0)), (p[1], (50.0, 0.0)), (p[2], (50.0, 60.0)), (p[3], (100.0, 60.0))] {
            assert!(close(a, Vec2::new(b.0, b.1)), "{p:?}");
        }
    }

    #[test]
    fn rounded_turns_keep_the_runs_straight_and_the_arcs_on_their_radius() {
        let r = 10.0;
        let p = elbow(Vec2::new(0.0, 0.0), Vec2::new(100.0, 60.0), r);
        assert!(close(p[0], Vec2::new(0.0, 0.0)) && close(*p.last().unwrap(), Vec2::new(100.0, 60.0)));
        // The first run is level until the turn starts r before halfway.
        assert!(close(p[1], Vec2::new(40.0, 0.0)), "{:?}", p[1]);
        // Every point of the first turn is r from its centre.
        let c1 = Vec2::new(40.0, 10.0);
        for q in &p[1..=7] {
            assert!(((q.x - c1.x).hypot(q.y - c1.y) - r).abs() < 1e-3, "{q:?}");
        }
        // The run down is straight at halfway.
        assert!(close(p[7], Vec2::new(50.0, 10.0)) && close(p[8], Vec2::new(50.0, 50.0)), "{:?}", &p[7..9]);
        assert!(close(p[14], Vec2::new(60.0, 60.0)), "{:?}", p[14]);
    }

    #[test]
    fn going_left_and_up_mirrors() {
        let p = elbow(Vec2::new(100.0, 60.0), Vec2::new(0.0, 0.0), 8.0);
        assert!(close(p[1], Vec2::new(58.0, 60.0)), "{:?}", p[1]);
        assert!(p.iter().all(|q| q.x <= 100.0 && q.x >= 0.0 && q.y <= 60.0 && q.y >= 0.0));
    }

    #[test]
    fn a_tight_space_shrinks_the_radius_and_a_level_one_is_straight() {
        // Only 6 down: each turn gets at most 3.
        let p = elbow(Vec2::new(0.0, 0.0), Vec2::new(100.0, 6.0), 20.0);
        assert!(close(p[1], Vec2::new(47.0, 0.0)), "{:?}", p[1]);
        assert!(p.iter().all(|q| q.y >= -1e-3 && q.y <= 6.0 + 1e-3));
        assert_eq!(elbow(Vec2::new(0.0, 5.0), Vec2::new(80.0, 5.2), 10.0).len(), 2);
    }
}
