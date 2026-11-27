//! Which cards keep a live browser. Every live card is a full Chromium
//! renderer, so only a few stay live; the rest sleep and show the last frame
//! they painted until they are focused or come back into view.
//!
//! Ranking, best first: the focused card, then cards on screen, then cards off
//! screen; ties go to the card used most recently. The top `cap` cards should be
//! live. A sleeping card is woken only when it is focused or on screen and the
//! caller allows waking (e.g. not mid-pan); waking an off-screen card would load
//! a page nobody can see.
//!
//! Some cards sit outside the cap altogether ([`Candidate::exempt`]): one set
//! to [`Awake::Keep`] or [`Awake::Active`], and one playing media (see
//! [`media_keeps`]). They are never put to sleep, don't take a slot, and an
//! exempt card that is asleep wakes once waking is allowed. The cap only
//! counts the other cards.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::NodeId;

/// How long a card that stopped playing media stays outside the cap, so a
/// pause to read or to answer something doesn't lose its place.
pub const MEDIA_GRACE: Duration = Duration::from_secs(10 * 60);

/// A card's awake mode, chosen with the button on its title bar.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Awake {
    /// The live cap decides (and media playing keeps it awake).
    #[default]
    Auto,
    /// Never put to sleep; hidden off screen like any page, so it costs little.
    Keep,
    /// Never put to sleep, never hidden, and keeps its focus: it runs as if
    /// it were the card on screen.
    Active,
}

impl Awake {
    pub fn is_auto(&self) -> bool {
        *self == Awake::Auto
    }

    /// The next mode, as the button cycles: Auto → Keep → Active → Auto.
    pub fn next(self) -> Awake {
        match self {
            Awake::Auto => Awake::Keep,
            Awake::Keep => Awake::Active,
            Awake::Active => Awake::Auto,
        }
    }

    /// The name used in commands and logs.
    pub fn name(self) -> &'static str {
        match self {
            Awake::Auto => "auto",
            Awake::Keep => "keep",
            Awake::Active => "active",
        }
    }

    /// Reads a name (`auto`, `keep` / `awake`, `active`).
    pub fn parse(s: &str) -> Option<Awake> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "off" => Some(Awake::Auto),
            "keep" | "awake" | "keep-awake" => Some(Awake::Keep),
            "active" | "always-active" => Some(Awake::Active),
            _ => None,
        }
    }
}

/// Whether media keeps a card outside the cap: it is playing now, or
/// stopped less than [`MEDIA_GRACE`] ago (`stopped_for`; `None`: never played).
pub fn media_keeps(playing: bool, stopped_for: Option<Duration>) -> bool {
    playing || stopped_for.is_some_and(|d| d < MEDIA_GRACE)
}

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
    /// Outside the cap: kept awake by its mode or by media.
    pub exempt: bool,
}

/// What to change so that at most `cap` cards that aren't exempt are live.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Plan {
    pub wake: Vec<NodeId>,
    pub sleep: Vec<NodeId>,
}

/// Decide which cards to wake and put to sleep. `cap` is at least 1.
/// With `allow_wake` false only the focused card may be woken.
pub fn plan(cards: &[Candidate], cap: usize, allow_wake: bool) -> Plan {
    let cap = cap.max(1);
    let mut out = Plan::default();
    // Exempt cards stay live, and wake when waking is allowed (or on focus).
    for c in cards.iter().filter(|c| c.exempt && !c.live && (allow_wake || c.focused)) {
        out.wake.push(c.id);
    }
    let mut ranked: Vec<&Candidate> = cards.iter().filter(|c| !c.exempt).collect();
    // Focused first, then visible, then most recent. Ties keep cards that are already
    // live (no swapping one page for another), then go by id so the order is stable.
    ranked.sort_by_key(|c| (!c.focused, !c.visible, std::cmp::Reverse(c.last_used), !c.live, c.id));

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
        Candidate { id: NodeId(id), live, visible, focused, last_used, exempt: false }
    }

    fn kept(id: u64, live: bool, visible: bool, last_used: u64) -> Candidate {
        Candidate { exempt: true, ..c(id, live, visible, false, last_used) }
    }

    #[test]
    fn exempt_cards_never_sleep_and_take_no_slot() {
        // Cap 2: two old kept cards off screen, three plain ones.
        let cards = [
            kept(1, true, false, 0),
            kept(2, true, false, 1),
            c(3, true, true, false, 5),
            c(4, true, true, false, 4),
            c(5, true, false, false, 3),
        ];
        let p = plan(&cards, 2, true);
        assert_eq!(p.sleep, vec![NodeId(5)], "only the plain card beyond the cap sleeps");
        assert!(p.wake.is_empty());
    }

    #[test]
    fn plain_cards_still_get_the_whole_cap() {
        let mut cards: Vec<_> = (0..5).map(|i| kept(i, true, false, i)).collect();
        cards.extend((10..14).map(|i| c(i, false, true, false, i)));
        let p = plan(&cards, 3, true);
        assert_eq!(p.wake.len(), 3);
        assert!(p.sleep.is_empty());
    }

    #[test]
    fn sleeping_exempt_cards_wake_when_allowed_even_off_screen() {
        let cards = [kept(1, false, false, 0), c(2, true, true, true, 1)];
        assert_eq!(plan(&cards, 1, true).wake, vec![NodeId(1)]);
        assert!(plan(&cards, 1, false).wake.is_empty(), "not mid-pan");
        let focused = [Candidate { focused: true, ..kept(1, false, false, 0) }];
        assert_eq!(plan(&focused, 1, false).wake, vec![NodeId(1)]);
    }

    #[test]
    fn a_focused_exempt_card_leaves_the_slot_to_others() {
        let cards = [Candidate { focused: true, ..kept(1, true, true, 9) }, c(2, true, true, false, 1)];
        assert_eq!(plan(&cards, 1, true), Plan::default());
    }

    #[test]
    fn media_grace() {
        assert!(media_keeps(true, None));
        assert!(!media_keeps(false, None));
        assert!(media_keeps(false, Some(Duration::from_secs(60))));
        assert!(!media_keeps(false, Some(MEDIA_GRACE)));
    }

    #[test]
    fn awake_cycles_and_reads_names() {
        assert_eq!(Awake::Auto.next(), Awake::Keep);
        assert_eq!(Awake::Keep.next(), Awake::Active);
        assert_eq!(Awake::Active.next(), Awake::Auto);
        for a in [Awake::Auto, Awake::Keep, Awake::Active] {
            assert_eq!(Awake::parse(a.name()), Some(a));
        }
        assert_eq!(Awake::parse(" Awake "), Some(Awake::Keep));
        assert_eq!(Awake::parse("sometimes"), None);
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
