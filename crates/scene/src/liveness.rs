//! Which cards keep a live browser. Every live card is a full Chromium
//! renderer, so only a few stay live; the rest sleep and show the last frame
//! they painted until they are focused or come back into view.
//!
//! Ranking, best first: the focused card, then cards on screen, then cards off
//! screen; ties go to the card used most recently. The top `cap` cards should be
//! live. A sleeping card is woken only when it is focused or on screen and the
//! caller allows waking (e.g. not mid-pan); waking an off-screen card would load
//! a page nobody can see.

use crate::NodeId;

/// One card's state, as seen by [`plan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub id: NodeId,
    /// Has a live browser now.
    pub live: bool,
    /// Intersects the screen.
    pub visible: bool,
    pub focused: bool,
    /// Larger is more recent (created, focused or interacted with).
    pub last_used: u64,
}

/// What to change so that at most `cap` cards are live.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Plan {
    pub wake: Vec<NodeId>,
    pub sleep: Vec<NodeId>,
}

/// Decide which cards to wake and put to sleep. `cap` is at least 1.
/// With `allow_wake` false only the focused card may be woken.
pub fn plan(cards: &[Candidate], cap: usize, allow_wake: bool) -> Plan {
    let cap = cap.max(1);
    let mut ranked: Vec<&Candidate> = cards.iter().collect();
    // Focused first, then visible, then most recent. Ties keep cards that are already
    // live (no swapping one page for another), then go by id so the order is stable.
    ranked.sort_by_key(|c| (!c.focused, !c.visible, std::cmp::Reverse(c.last_used), !c.live, c.id));

    let mut out = Plan::default();
    let mut live_after = 0;
    for (rank, c) in ranked.iter().enumerate() {
        let wanted = rank < cap;
        if c.live {
            if wanted {
                live_after += 1;
            } else {
                out.sleep.push(c.id);
            }
        } else if wanted && (c.focused || (allow_wake && c.visible)) {
            out.wake.push(c.id);
            live_after += 1;
        }
    }
    debug_assert!(live_after <= cap);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: u64, live: bool, visible: bool, focused: bool, last_used: u64) -> Candidate {
        Candidate { id: NodeId(id), live, visible, focused, last_used }
    }

    #[test]
    fn under_cap_changes_nothing_off_screen() {
        let cards = [c(1, true, true, false, 1), c(2, false, false, false, 2)];
        assert_eq!(plan(&cards, 4, true), Plan::default());
    }

    #[test]
    fn wakes_visible_sleepers_when_allowed() {
        let cards = [c(1, false, true, false, 1), c(2, false, false, false, 2)];
        assert_eq!(plan(&cards, 4, true).wake, vec![NodeId(1)]);
        assert!(plan(&cards, 4, false).wake.is_empty());
    }

    #[test]
    fn focused_always_wakes_even_when_waking_is_paused() {
        let cards = [c(1, true, true, false, 9), c(2, false, false, true, 0)];
        let p = plan(&cards, 1, false);
        assert_eq!(p.wake, vec![NodeId(2)]);
        assert_eq!(p.sleep, vec![NodeId(1)]);
    }

    #[test]
    fn over_cap_sleeps_off_screen_least_recent_first() {
        let cards = [
            c(1, true, false, false, 1), // off screen, oldest → sleeps
            c(2, true, false, false, 5),
            c(3, true, true, false, 2),
            c(4, true, true, true, 3),
        ];
        let p = plan(&cards, 3, true);
        assert_eq!(p.sleep, vec![NodeId(1)]);
        assert!(p.wake.is_empty());
    }

    #[test]
    fn visible_beats_recent_off_screen() {
        let cards = [c(1, true, false, false, 100), c(2, false, true, false, 1)];
        let p = plan(&cards, 1, true);
        assert_eq!(p.wake, vec![NodeId(2)]);
        assert_eq!(p.sleep, vec![NodeId(1)]);
    }

    #[test]
    fn more_visible_than_cap_keeps_most_recent() {
        let cards = [c(1, true, true, false, 1), c(2, true, true, false, 3), c(3, false, true, false, 2)];
        let p = plan(&cards, 2, true);
        assert_eq!(p.wake, vec![NodeId(3)]);
        assert_eq!(p.sleep, vec![NodeId(1)]);
    }

    #[test]
    fn stable_when_nothing_changes() {
        let cards = [c(1, true, true, false, 1), c(2, false, true, false, 1), c(3, true, true, false, 1)];
        // 1 and 3 already fill the cap; an equally recent sleeper doesn't displace them.
        assert_eq!(plan(&cards, 2, true), Plan::default());
    }

    #[test]
    fn never_exceeds_cap_and_cap_zero_means_one() {
        let cards: Vec<_> = (0..10).map(|i| c(i, i % 2 == 0, i < 6, i == 7, i)).collect();
        for cap in 0..12 {
            let p = plan(&cards, cap, true);
            let live = cards.iter().filter(|c| c.live).count() + p.wake.len() - p.sleep.len();
            assert!(live <= cap.max(1), "cap {cap}: {live} live");
            assert!(p.wake.contains(&NodeId(7)), "focused card wakes");
        }
    }
}
