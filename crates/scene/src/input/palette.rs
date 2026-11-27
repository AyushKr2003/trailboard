//! Fuzzy matching and ranking for the Ctrl+K palette.
//!
//! A query matches a text when its letters (spaces ignored, case ignored)
//! appear in the text in order. Among the ways to place them, the best one is
//! kept by a small dynamic programme: each matched letter scores, more at the
//! start of a word and more when it follows the previous match; a jump over
//! letters costs a fixed amount, and so does a late start (a little).

/// Score of one matched letter.
const MATCH: i32 = 1;
/// The letter starts a word (after a non-letter, or a capital after a small letter).
const WORD_START: i32 = 5;
/// The letter is the text's first.
const TEXT_START: i32 = 3;
/// The letter follows the previous match directly.
const CONSECUTIVE: i32 = 4;
/// Jumping over letters between two matches.
const GAP: i32 = 3;
/// Letters skipped before the first match cost one point per three, up to this.
const MAX_LEAD: i32 = 3;
/// Only this many characters of a text take part (titles and URLs can be long).
const MAX_TEXT: usize = 200;
/// A match that scores below this is too scattered to show.
const MIN_SCORE: i32 = 0;
/// A match on the second text (a URL, a note's body) ranks this much lower.
const ALT_PENALTY: i32 = 8;
/// The second text is not shown, so a match there must be nearly in one
/// piece: its letters span at most this many times the query's length.
const ALT_SPAN: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub score: i32,
    /// Char indices of the matched letters in the text, ascending.
    pub positions: Vec<usize>,
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// How `query` matches `text`, if every letter of it is found in order.
/// An empty query matches everything with score 0.
pub fn fuzzy(query: &str, text: &str) -> Option<Match> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).map(fold).collect();
    if q.is_empty() {
        return Some(Match { score: 0, positions: Vec::new() });
    }
    let raw: Vec<char> = text.chars().take(MAX_TEXT).collect();
    let t: Vec<char> = raw.iter().copied().map(fold).collect();
    let (n, m) = (q.len(), t.len());
    if n > m {
        return None;
    }
    // Quick reject: not a subsequence at all.
    let mut it = t.iter();
    if !q.iter().all(|c| it.any(|x| x == c)) {
        return None;
    }

    let bonus = |j: usize| {
        let mut b = MATCH;
        if j == 0 {
            b += WORD_START + TEXT_START;
        } else {
            let (prev, cur) = (raw[j - 1], raw[j]);
            if !prev.is_alphanumeric() || (prev.is_lowercase() && cur.is_uppercase()) {
                b += WORD_START;
            }
        }
        b
    };
    const NONE: i32 = i32::MIN / 2;
    // score[i * m + j]: best score with q[..=i] placed and q[i] on t[j].
    let mut score = vec![NONE; n * m];
    let mut from = vec![usize::MAX; n * m];
    for j in 0..m {
        if t[j] == q[0] {
            score[j] = bonus(j) - (j as i32 / 3).min(MAX_LEAD);
        }
    }
    for i in 1..n {
        // Best of row i-1 at positions < j-1 (a gap), with its index.
        let mut best: (i32, usize) = (NONE, usize::MAX);
        for j in 1..m {
            if j >= 2 && score[(i - 1) * m + j - 2] > best.0 {
                best = (score[(i - 1) * m + j - 2], j - 2);
            }
            if t[j] != q[i] {
                continue;
            }
            let adjacent = score[(i - 1) * m + j - 1];
            let (via, k) = if adjacent > NONE && adjacent + CONSECUTIVE >= best.0 - GAP {
                (adjacent + CONSECUTIVE, j - 1)
            } else {
                (best.0 - GAP, best.1)
            };
            if k != usize::MAX && via > NONE / 2 {
                score[i * m + j] = via + bonus(j);
                from[i * m + j] = k;
            }
        }
    }
    let last = (n - 1) * m;
    let (j, &s) = score[last..].iter().enumerate().max_by_key(|&(j, s)| (*s, std::cmp::Reverse(j)))?;
    if s <= NONE / 2 {
        return None;
    }
    let mut positions = vec![0; n];
    let mut j = j;
    for i in (0..n).rev() {
        positions[i] = j;
        j = from[i * m + j];
    }
    Some(Match { score: s, positions })
}

/// Something the palette can offer: the text it shows, and a second text it
/// is also found by (a URL, a note's body, other words for a command).
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub text: &'a str,
    pub alt: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    /// Index into the candidates.
    pub index: usize,
    pub score: i32,
    /// Matched letters in `text` (empty when it matched on `alt`).
    pub positions: Vec<usize>,
}

/// Candidates that match `query`, best first. Ties keep the given order (so
/// the caller lists what matters most first); an empty query keeps every
/// candidate in order.
pub fn rank(query: &str, candidates: &[Candidate]) -> Vec<Ranked> {
    let mut out: Vec<Ranked> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, c)| {
            let on_text = fuzzy(query, c.text).filter(|m| m.score >= MIN_SCORE);
            let on_alt = fuzzy(query, c.alt)
                .filter(|m| m.score >= MIN_SCORE && span(m) <= ALT_SPAN * m.positions.len())
                .map(|m| m.score - ALT_PENALTY);
            let ranked = match (on_text, on_alt) {
                (Some(m), alt) if alt.is_none_or(|a| m.score >= a) => Ranked { index, score: m.score, positions: m.positions },
                (_, Some(score)) => Ranked { index, score, positions: Vec::new() },
                _ => return None,
            };
            Some(ranked)
        })
        .collect();
    if !query.trim().is_empty() {
        out.sort_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));
    }
    out
}

/// How many characters a match covers, first matched letter to last.
fn span(m: &Match) -> usize {
    match (m.positions.first(), m.positions.last()) {
        (Some(a), Some(b)) => b - a + 1,
        _ => 0,
    }
}

/// The first row to show so that row `sel` is visible, moving the window
/// `scroll` as little as possible; `shown` rows fit, out of `len`.
pub fn keep_visible(sel: usize, scroll: usize, shown: usize, len: usize) -> usize {
    let shown = shown.max(1);
    let max = len.saturating_sub(shown);
    let scroll = if sel < scroll {
        sel
    } else if sel >= scroll + shown {
        sel + 1 - shown
    } else {
        scroll
    };
    scroll.min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(q: &str, t: &str) -> i32 {
        fuzzy(q, t).map(|m| m.score).unwrap_or(i32::MIN)
    }

    #[test]
    fn subsequence_or_nothing() {
        assert!(fuzzy("tdy", "Tidy trails").is_some());
        assert!(fuzzy("ydt", "Tidy").is_none());
        assert!(fuzzy("tidyx", "Tidy").is_none());
        assert_eq!(fuzzy("", "anything").map(|m| m.score), Some(0));
        assert_eq!(fuzzy("  ", "anything").map(|m| m.score), Some(0));
    }

    #[test]
    fn case_and_spaces_ignored() {
        let m = fuzzy("TIDY tr", "Tidy trails").expect("match");
        assert_eq!(m.positions, vec![0, 1, 2, 3, 5, 6]);
        assert_eq!(fuzzy("ÉCOLE", "école").map(|m| m.positions), Some(vec![0, 1, 2, 3, 4]));
    }

    #[test]
    fn prefers_word_starts_and_runs() {
        // "fit" at the start beats "fit" inside a word.
        assert!(score("fit", "Fit everything") > score("fit", "Outfitter"));
        // A run beats scattered letters.
        assert!(score("new", "New note") > score("new", "Name the trail, edit, wait"));
        // Initials find words: "rc" → Reopen closed card.
        let m = fuzzy("rcc", "Reopen closed card").expect("match");
        assert_eq!(m.positions, vec![0, 7, 14]);
        // camelCase counts as a word start.
        let m = fuzzy("gh", "GitHub").expect("match");
        assert_eq!(m.positions, vec![0, 3]);
    }

    #[test]
    fn best_placement_not_first() {
        // Greedy would take the first 'n' of "Open"; the word start is better.
        let m = fuzzy("on", "Open note").expect("match");
        assert_eq!(m.positions, vec![0, 5]);
        let m = fuzzy("note", "nothing to see: note").expect("match");
        assert_eq!(m.positions, vec![16, 17, 18, 19]);
    }

    #[test]
    fn rank_orders_and_filters() {
        let c = |text: &'static str, alt: &'static str| Candidate { text, alt };
        let cands = [
            c("Wikipedia: Trail", "en.wikipedia.org/wiki/Trail"),
            c("Tidy trails", "arrange layout"),
            c("Trails sidebar", "list tree"),
            c("Quit", "exit close"),
            c("Something", "zzz"),
        ];
        let r = rank("trail", &cands);
        let order: Vec<usize> = r.iter().map(|r| r.index).collect();
        // A match at the very start first, then the earlier word start.
        assert_eq!(order, vec![2, 1, 0]);
        // Found by its other words: no highlighted letters, ranked below text matches.
        let r = rank("exit", &cands);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].index, 3);
        assert!(r[0].positions.is_empty());
        // Empty query: every candidate, in the given order.
        let r = rank("", &cands);
        assert_eq!(r.iter().map(|r| r.index).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
        // Scattered single letters deep inside a long URL are dropped.
        assert!(rank("qz", &[c("Page", "example.com/aaaaaaaaq/bbbbbbbbbz")]).is_empty());
        // Letters strewn over a hidden URL don't count, even at word starts.
        assert!(rank("help", &[c("P5", "home/shadow/.cache/trailboard-e2e-fold/pages/p5.html")]).is_empty());
        assert_eq!(rank("pages", &[c("P5", "home/shadow/pages/p5.html")]).len(), 1);
    }

    #[test]
    fn text_match_beats_alt_match() {
        let cands = [
            Candidate { text: "Page about notes", alt: "" },
            Candidate { text: "Something else", alt: "notes" },
        ];
        let r = rank("notes", &cands);
        assert_eq!(r[0].index, 0);
        assert_eq!(r[1].index, 1);
    }

    #[test]
    fn long_texts_are_cut() {
        let long = format!("{}needle", "x".repeat(300));
        assert!(fuzzy("needle", &long).is_none());
    }

    #[test]
    fn keeps_selection_visible() {
        assert_eq!(keep_visible(0, 0, 5, 20), 0);
        assert_eq!(keep_visible(4, 0, 5, 20), 0);
        assert_eq!(keep_visible(5, 0, 5, 20), 1);
        assert_eq!(keep_visible(3, 6, 5, 20), 3);
        assert_eq!(keep_visible(19, 0, 5, 20), 15);
        // Never scrolled past the end, even if the list got shorter.
        assert_eq!(keep_visible(2, 10, 5, 8), 2);
        assert_eq!(keep_visible(7, 10, 5, 8), 3);
        assert_eq!(keep_visible(0, 4, 5, 3), 0);
    }
}
