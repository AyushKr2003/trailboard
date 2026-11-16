//! Scrolling inside pages, made to feel like a desktop browser. Our own design.
//!
//! Measured in a page (2026-11-14): Chromium animates every wheel event we
//! send it unless the event is marked precise, as a mouse wheel notch in
//! Chrome. So:
//!
//! - A wheel notch ([`Scroller::wheel`]) goes out at once as one event of
//!   [`NOTCH`] px; Chromium glides it, and adds up notches that come quickly.
//! - Touchpad movement ([`Scroller::touch`]) goes out at once, marked
//!   precise (no animation, it follows the fingers), multiplied by
//!   [`TOUCHPAD_GAIN`]: Wayland gives it in surface px, which scroll a page
//!   far too slowly 1:1 (Chromium's own Wayland port and GTK 3 multiply too).
//! - Lifting the fingers after a flick keeps the page going (a fling):
//!   it starts at the speed of the last 100 ms of movement and slows down
//!   smoothly, sent a part per frame ([`Scroller::step`]), precise. Any new
//!   scroll stops it.
//! - All of it is scaled by the user's *Scroll speed* setting.
//! - Chromium takes whole px; the fractions left over are carried on.
//! - The app's loop runs far more often than the screen; a fling goes out at
//!   most once per [`SEND_GAP`], one event a frame rather than a flood of
//!   1 px ones.

use std::collections::VecDeque;

use crate::Vec2;

/// Px one wheel notch scrolls at normal speed (Chromium animates it).
pub const NOTCH: f32 = 120.0;
/// Page px per touchpad surface px at normal speed.
pub const TOUCHPAD_GAIN: f32 = 4.0;
/// Time constant of a fling slowing down, in seconds; a fling goes
/// `speed × FLING_TAU` px in all.
pub const FLING_TAU: f32 = 0.33;
/// Slowest flick that flings, and the speed a fling stops at, in px/s.
pub const FLING_START: f32 = 250.0;
pub const FLING_STOP: f32 = 60.0;
/// Touchpad movement this recent sets a fling's speed.
pub const FLING_WINDOW: f32 = 0.1;
/// Shortest time between two scrolls sent by [`Scroller::step`] (a 60 Hz frame).
pub const SEND_GAP: f32 = 1.0 / 60.0;

/// Scrolling of one target (a page and the point the pointer was over);
/// `T` is whatever the caller sends scrolls to.
#[derive(Debug, Clone)]
pub struct Scroller<T> {
    target: Option<T>,
    /// Fling speed, px/s.
    velocity: Vec2,
    /// Recent touchpad movement: (time, px after gain).
    samples: VecDeque<(f32, Vec2)>,
    /// Seconds since this scroller was made (for `samples`).
    clock: f32,
    /// Fractions of px not sent yet.
    carry: Vec2,
    /// Seconds since `step` last sent something.
    since_sent: f32,
}

impl<T> Default for Scroller<T> {
    fn default() -> Self {
        Scroller {
            target: None,
            velocity: Vec2::default(),
            samples: VecDeque::new(),
            clock: 0.0,
            carry: Vec2::default(),
            since_sent: SEND_GAP,
        }
    }
}

fn len(v: Vec2) -> f32 {
    v.x.hypot(v.y)
}

impl<T: Copy + PartialEq> Scroller<T> {
    /// Stops a fling and forgets recent movement.
    pub fn stop(&mut self) {
        let clock = self.clock;
        *self = Scroller { clock, ..Scroller::default() };
    }

    /// A fling is going: call [`Scroller::step`] every frame.
    pub fn busy(&self) -> bool {
        self.target.is_some() && len(self.velocity) > 0.0
    }

    /// The target of the last scroll, if any.
    pub fn target(&self) -> Option<T> {
        self.target
    }

    /// Scrolling another target than the last starts afresh.
    fn retarget(&mut self, target: T) {
        if self.target != Some(target) {
            self.stop();
            self.target = Some(target);
        }
    }

    /// Wheel notches (`lines`, in winit's sign: positive y scrolls up) for
    /// `target`: the whole px to send now, as one (animated) wheel event.
    pub fn wheel(&mut self, target: T, lines: Vec2, speed: f32) -> Vec2 {
        self.retarget(target);
        self.velocity = Vec2::default();
        self.samples.clear();
        self.whole(lines * (NOTCH * speed))
    }

    /// Touchpad movement `px` for `target`: the whole px to send now
    /// (precise). `ended` is the fingers lifting (no movement then, usually),
    /// which starts a fling if they were moving fast enough.
    pub fn touch(&mut self, target: T, px: Vec2, speed: f32, ended: bool) -> Vec2 {
        self.retarget(target);
        self.velocity = Vec2::default();
        let d = px * (TOUCHPAD_GAIN * speed);
        if len(d) > 0.0 {
            self.samples.push_back((self.clock, d));
        }
        let since = self.clock - FLING_WINDOW;
        while self.samples.front().is_some_and(|&(t, _)| t < since) {
            self.samples.pop_front();
        }
        if ended {
            // Speed over the window (at least a frame long, so one big
            // sample doesn't fling at once).
            let first = self.samples.front().map_or(self.clock, |&(t, _)| t);
            let span = (self.clock - first).max(1.0 / 60.0);
            let sum = self.samples.iter().fold(Vec2::default(), |a, &(_, d)| a + d);
            let v = sum * (1.0 / span);
            if len(v) >= FLING_START {
                self.velocity = v;
                self.since_sent = SEND_GAP;
            }
            self.samples.clear();
        }
        self.whole(d)
    }

    /// Advances `dt` seconds; returns the target and whole px of the fling to
    /// send now (precise), if any.
    pub fn step(&mut self, dt: f32) -> Option<(T, Vec2)> {
        self.clock += dt;
        self.since_sent += dt;
        let target = self.target?;
        if len(self.velocity) == 0.0 || self.since_sent < SEND_GAP {
            return None;
        }
        let dt = std::mem::replace(&mut self.since_sent, 0.0);
        let decay = (-dt / FLING_TAU).exp();
        // Exact distance of the exponential slow-down over `dt`.
        let d = self.velocity * (FLING_TAU * (1.0 - decay));
        self.velocity = self.velocity * decay;
        if len(self.velocity) < FLING_STOP {
            self.velocity = Vec2::default();
        }
        let out = self.whole(d);
        (out != Vec2::default()).then_some((target, out))
    }

    /// Whole px of `d` plus what was carried; keeps the fraction.
    fn whole(&mut self, d: Vec2) -> Vec2 {
        let all = self.carry + d;
        let out = Vec2::new(all.x.trunc(), all.y.trunc());
        self.carry = all - out;
        out
    }
}

/// Board px one wheel notch pans at normal speed.
pub const BOARD_NOTCH: f32 = 120.0;
/// Board px per touchpad surface px at normal speed: more than 1:1, which
/// crawls (the same reason pages take [`TOUCHPAD_GAIN`]), less than a page,
/// since a board moves under the fingers rather than a page inside a card.
pub const BOARD_TOUCHPAD_GAIN: f32 = 2.5;
/// Time constant of a wheel notch's glide on the board, in seconds: it is
/// nearly all there after 3 × this (about 0.2 s, like a browser's smooth
/// scrolling).
pub const GLIDE_TAU: f32 = 0.07;

/// Panning the board, made to feel like scrolling a page (`Scroller`):
/// a wheel notch glides there, easing out, and notches that come quickly
/// add up; touchpad movement follows the fingers at once and flings when
/// they lift after a flick. All of it scaled by *Scroll speed*. The board
/// takes fractions, so nothing is rounded.
#[derive(Debug, Clone, Default)]
pub struct Pan {
    /// Of a wheel glide: px still to go.
    glide: Vec2,
    /// Touchpad movement and its fling (whole px, which is plenty).
    touch: Scroller<()>,
}

impl Pan {
    /// Stops a glide or a fling (the board was grabbed, or the camera
    /// is going somewhere).
    pub fn stop(&mut self) {
        self.glide = Vec2::default();
        self.touch.stop();
    }

    /// A glide or a fling is going: call [`Pan::step`] every frame.
    pub fn busy(&self) -> bool {
        len(self.glide) > 0.0 || self.touch.busy()
    }

    /// Wheel notches (`lines`, winit's sign): added to the glide.
    pub fn wheel(&mut self, lines: Vec2, speed: f32) {
        self.touch.stop();
        self.glide = self.glide + lines * (BOARD_NOTCH * speed);
    }

    /// Touchpad movement `px`: the px to pan now. `ended` is the fingers
    /// lifting, which may start a fling.
    pub fn touch(&mut self, px: Vec2, speed: f32, ended: bool) -> Vec2 {
        self.glide = Vec2::default();
        self.touch.touch((), px, speed * BOARD_TOUCHPAD_GAIN / TOUCHPAD_GAIN, ended)
    }

    /// Advances `dt` seconds; returns the px to pan now (glide and fling).
    /// Call it every frame, also while nothing moves: it times the touchpad
    /// movement a fling's speed comes from.
    pub fn step(&mut self, dt: f32) -> Vec2 {
        let mut out = Vec2::default();
        if len(self.glide) > 0.0 {
            let part = self.glide * (1.0 - (-dt / GLIDE_TAU).exp());
            let left = self.glide - part;
            // The last half px goes at once rather than creeping.
            if len(left) < 0.5 {
                out = self.glide;
                self.glide = Vec2::default();
            } else {
                out = part;
                self.glide = left;
            }
        }
        if let Some(((), d)) = self.touch.step(dt) {
            out = out + d;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs frames of `dt` while a fling goes; total sent, sends and time.
    fn run(s: &mut Scroller<u8>, dt: f32) -> (Vec2, usize, f32) {
        let (mut total, mut sends, mut t) = (Vec2::default(), 0, 0.0);
        while s.busy() {
            t += dt;
            if let Some((_, d)) = s.step(dt) {
                total = total + d;
                sends += 1;
            }
            assert!(t < 10.0, "never settles");
        }
        (total, sends, t)
    }

    /// Six touchpad moves of `px` a frame apart, then the fingers lift.
    fn flick(s: &mut Scroller<u8>, px: f32) -> f32 {
        let mut sent = 0.0;
        for _ in 0..6 {
            sent += s.touch(1, Vec2::new(0.0, px), 1.0, false).y;
            s.step(1.0 / 60.0);
        }
        sent + s.touch(1, Vec2::default(), 1.0, true).y
    }

    #[test]
    fn a_notch_goes_at_once_scaled_by_speed() {
        let mut s = Scroller::default();
        assert_eq!(s.wheel(1, Vec2::new(0.0, -1.0), 1.0), Vec2::new(0.0, -NOTCH));
        assert_eq!(s.wheel(1, Vec2::new(1.0, 0.0), 1.5), Vec2::new(NOTCH * 1.5, 0.0));
        assert!(!s.busy() && s.step(1.0 / 60.0).is_none());
    }

    #[test]
    fn touchpad_is_multiplied_and_fractions_carry() {
        let mut s = Scroller::default();
        let a = s.touch(1, Vec2::new(0.0, 0.3), 1.0, false);
        let b = s.touch(1, Vec2::new(0.0, 0.3), 1.0, false);
        let c = s.touch(1, Vec2::new(0.0, 0.4), 1.0, false);
        assert_eq!(a.y + b.y + c.y, 4.0, "1 surface px × 4");
        assert!(s.step(1.0 / 60.0).is_none(), "no fling while the fingers are down");
    }

    #[test]
    fn a_flick_flings_and_slows_down_but_a_slow_lift_does_not() {
        let mut s = Scroller::default();
        let sent = flick(&mut s, -10.0);
        assert_eq!(sent, -240.0);
        // 40 px a frame: 2400 px/s.
        assert!(s.busy());
        let (_, d1) = s.step(1.0 / 60.0).unwrap();
        let (_, d2) = s.step(1.0 / 60.0).unwrap();
        assert!(d1.y < 0.0 && d2.y > d1.y, "keeps going, slowing: {d1:?} {d2:?}");
        let (rest, _, t) = run(&mut s, 1.0 / 60.0);
        let total = d1.y + d2.y + rest.y;
        assert!(total < -600.0 && total > -900.0, "about speed × FLING_TAU: {total}");
        assert!(t < 2.0, "stops within 2 s: {t}");

        // Slow movement, then a pause before lifting: no fling.
        let mut s = Scroller::default();
        s.touch(1, Vec2::new(0.0, -1.0), 1.0, false);
        for _ in 0..12 {
            s.step(1.0 / 60.0);
        }
        s.touch(1, Vec2::default(), 1.0, true);
        assert!(!s.busy());
    }

    #[test]
    fn a_fast_loop_sends_a_fling_once_a_frame() {
        let mut fast = Scroller::default();
        flick(&mut fast, -10.0);
        let (total, sends, t) = run(&mut fast, 0.001);
        let mut slow = Scroller::default();
        flick(&mut slow, -10.0);
        let (total60, ..) = run(&mut slow, 1.0 / 60.0);
        assert!(sends <= (t / SEND_GAP) as usize + 1, "{sends} sends in {t} s");
        assert!((total.y - total60.y).abs() <= 25.0, "about the same distance: {total:?} {total60:?}");
    }

    #[test]
    fn new_input_or_another_target_stops_a_fling() {
        let mut s = Scroller::default();
        flick(&mut s, -10.0);
        s.wheel(1, Vec2::new(0.0, 1.0), 1.0);
        assert!(!s.busy(), "the wheel stops it");
        flick(&mut s, -10.0);
        s.touch(1, Vec2::new(0.0, 1.0), 1.0, false);
        assert!(!s.busy(), "fingers down again stop it");
        flick(&mut s, -10.0);
        s.wheel(2, Vec2::new(0.0, -1.0), 1.0);
        assert_eq!(s.target(), Some(2));
        assert!(!s.busy());
        flick(&mut s, -10.0);
        s.stop();
        assert!(s.step(1.0 / 60.0).is_none());
    }

    #[test]
    fn a_notch_glides_on_the_board_and_notches_add_up() {
        let mut p = Pan::default();
        p.wheel(Vec2::new(0.0, -1.0), 1.0);
        assert!(p.busy());
        let first = p.step(1.0 / 60.0).y;
        assert!(first < 0.0 && first > -BOARD_NOTCH / 2.0, "eases: {first}");
        let mut total = first;
        p.wheel(Vec2::new(0.0, -1.0), 1.0); // a second notch mid-glide
        let mut t = 0.0;
        while p.busy() {
            total += p.step(1.0 / 60.0).y;
            t += 1.0 / 60.0;
            assert!(t < 2.0, "never settles");
        }
        assert!((total + 2.0 * BOARD_NOTCH).abs() < 1e-3, "{total}");
        assert!(t < 0.6, "settles quickly: {t}");
        p.wheel(Vec2::new(0.0, 1.0), 1.5);
        assert!(p.step(1.0).y > 0.0);
        assert!(!p.busy() || p.step(1.0) == Vec2::default());
    }

    #[test]
    fn the_touchpad_pans_at_once_and_flings_on_the_board() {
        let mut p = Pan::default();
        let mut sent = 0.0;
        for _ in 0..6 {
            sent += p.touch(Vec2::new(0.0, -10.0), 1.0, false).y;
            p.step(1.0 / 60.0);
        }
        assert_eq!(sent, -150.0, "10 surface px × 2.5, six times");
        p.touch(Vec2::default(), 1.0, true);
        assert!(p.busy(), "a flick flings");
        let mut fling = 0.0;
        while p.busy() {
            fling += p.step(1.0 / 60.0).y;
        }
        assert!(fling < -100.0, "{fling}");
        // A wheel notch stops a fling; grabbing stops a glide.
        p.wheel(Vec2::new(0.0, 1.0), 1.0);
        p.stop();
        assert!(!p.busy());
    }
}
