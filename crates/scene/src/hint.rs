//! Link hints: short labels for the links on a page, so one can be followed
//! by typing its label (`f` on the board). Our own design.
//!
//! Labels are drawn from a small alphabet of home-row letters and are
//! prefix-free (no label starts another), so a label is followed as soon as
//! it is typed in full, with no Enter. As many labels as possible are one
//! letter shorter than the rest; the short ones go to the first links.

/// Letters labels are made of: the home row, which needs no reaching.
pub const ALPHABET: &str = "asdfghjkl";

/// `count` distinct, prefix-free labels from `alphabet`, shortest first.
/// Empty for `count == 0` or an alphabet of fewer than two letters.
pub fn labels(count: usize, alphabet: &str) -> Vec<String> {
    let letters: Vec<char> = alphabet.chars().collect();
    if count == 0 || letters.len() < 2 {
        return Vec::new();
    }
    // The leaves of a tree with `letters.len()` children per node: splitting
    // the first (shortest) leaf adds `k - 1` leaves, until there are enough.
    let mut leaves: std::collections::VecDeque<String> = letters.iter().map(|c| c.to_string()).collect();
    while leaves.len() < count {
        let parent = leaves.pop_front().expect("never empty");
        leaves.extend(letters.iter().map(|c| format!("{parent}{c}")));
    }
    let mut out: Vec<String> = leaves.into_iter().take(count).collect();
    out.sort_by_key(String::len); // stable: alphabet order within a length
    out
}

/// What a typed letter did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    /// The label of link `n` is now typed in full.
    Follow(usize),
    /// Some labels still start with what is typed.
    Narrowed,
    /// No label starts with it; the letter was not kept.
    NoMatch,
}

/// The labels of one page's links and what has been typed of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    pub labels: Vec<String>,
    pub typed: String,
}

impl Hints {
    /// Labels for `count` links.
    pub fn new(count: usize) -> Self {
        Self { labels: labels(count, ALPHABET), typed: String::new() }
    }

    /// Type one letter (case is ignored).
    pub fn type_char(&mut self, c: char) -> Typed {
        let next: String = format!("{}{}", self.typed, c.to_lowercase());
        if let Some(i) = self.labels.iter().position(|l| *l == next) {
            self.typed = next;
            return Typed::Follow(i);
        }
        if self.labels.iter().any(|l| l.starts_with(&next)) {
            self.typed = next;
            Typed::Narrowed
        } else {
            Typed::NoMatch
        }
    }

    /// Take back the last letter typed. False when nothing was typed.
    pub fn backspace(&mut self) -> bool {
        self.typed.pop().is_some()
    }

    /// Whether link `n`'s label is still possible.
    pub fn shown(&self, n: usize) -> bool {
        self.labels.get(n).is_some_and(|l| l.starts_with(&self.typed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefix_free(ls: &[String]) -> bool {
        ls.iter().enumerate().all(|(i, a)| ls.iter().enumerate().all(|(j, b)| i == j || !b.starts_with(a.as_str())))
    }

    #[test]
    fn few_links_get_one_letter() {
        assert_eq!(labels(3, ALPHABET), ["a", "s", "d"]);
        assert_eq!(labels(9, ALPHABET).len(), 9);
        assert!(labels(9, ALPHABET).iter().all(|l| l.len() == 1));
    }

    #[test]
    fn more_links_mix_one_and_two_letters() {
        let ls = labels(20, ALPHABET);
        assert_eq!(ls.len(), 20);
        assert!(prefix_free(&ls));
        // Splitting two letters gives 7 + 18 = 25 ≥ 20 leaves: 7 short, 13 long.
        assert_eq!(ls.iter().filter(|l| l.len() == 1).count(), 7);
        assert!(ls.windows(2).all(|w| w[0].len() <= w[1].len()), "shortest first");
    }

    #[test]
    fn many_links_stay_distinct_and_prefix_free() {
        for n in [1, 2, 10, 81, 82, 300, 500] {
            let ls = labels(n, ALPHABET);
            assert_eq!(ls.len(), n);
            let unique: std::collections::HashSet<_> = ls.iter().collect();
            assert_eq!(unique.len(), n);
            assert!(prefix_free(&ls), "{n}");
            assert!(ls.iter().all(|l| l.len() <= 3), "{n}");
        }
    }

    #[test]
    fn nothing_to_label() {
        assert!(labels(0, ALPHABET).is_empty());
        assert!(labels(5, "a").is_empty());
    }

    #[test]
    fn typing_narrows_then_follows() {
        let mut h = Hints::new(20);
        let long = h.labels.iter().position(|l| l.len() == 2).unwrap();
        let label = h.labels[long].clone();
        let mut cs = label.chars();
        assert_eq!(h.type_char(cs.next().unwrap().to_ascii_uppercase()), Typed::Narrowed);
        assert!(h.shown(long));
        assert!(!h.shown(0), "a one-letter label is no longer possible");
        assert_eq!(h.type_char(cs.next().unwrap()), Typed::Follow(long));
    }

    #[test]
    fn a_wrong_letter_is_dropped_and_backspace_undoes() {
        let mut h = Hints::new(3);
        assert_eq!(h.type_char('z'), Typed::NoMatch);
        assert_eq!(h.typed, "");
        assert!(!h.backspace());
        let mut h = Hints::new(20);
        let first_long = h.labels.iter().find(|l| l.len() == 2).unwrap().clone();
        let c = first_long.chars().next().unwrap();
        assert_eq!(h.type_char(c), Typed::Narrowed);
        assert_eq!(h.type_char('z'), Typed::NoMatch);
        assert_eq!(h.typed, c.to_string());
        assert!(h.backspace());
        assert!((0..20).all(|i| h.shown(i)));
    }

    #[test]
    fn a_one_letter_label_follows_at_once() {
        let mut h = Hints::new(2);
        assert_eq!(h.type_char('s'), Typed::Follow(1));
    }
}
