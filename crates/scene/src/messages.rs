//! The messages shown this session (M12 step 16): every toast, newest
//! last, for `:messages` to list. A message that repeats straight after
//! itself counts up instead of taking another row. Only kept while the app
//! runs; nothing is saved.

use std::collections::VecDeque;

/// How many messages are kept; older ones drop off.
pub const KEEP: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// A number for the row that stays the same while the log changes.
    pub id: u64,
    pub text: String,
    /// When it was last shown (UTC ms).
    pub at_ms: i64,
    /// How many times in a row it was shown.
    pub times: u32,
}

#[derive(Debug, Default)]
pub struct Messages {
    list: VecDeque<Message>,
    next: u64,
}

impl Messages {
    /// Logs `text`, shown at `at_ms`. Blank text isn't logged.
    pub fn push(&mut self, text: &str, at_ms: i64) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.list.back_mut()
            && last.text == text
        {
            last.times += 1;
            last.at_ms = at_ms;
            return;
        }
        if self.list.len() == KEEP {
            self.list.pop_front();
        }
        self.next += 1;
        self.list.push_back(Message { id: self.next, text: text.to_string(), at_ms, times: 1 });
    }

    /// Newest first.
    pub fn newest(&self) -> impl Iterator<Item = &Message> {
        self.list.iter().rev()
    }

    pub fn get(&self, id: u64) -> Option<&Message> {
        self.list.iter().find(|m| m.id == id)
    }

    /// Drops one message; false if it was gone.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.list.len();
        self.list.retain(|m| m.id != id);
        self.list.len() != before
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

/// How long until a waiting `:later` line runs, short: "in 40 s",
/// "in 3 min", "in 2 h 5 min".
pub fn wait_label(ms: u64) -> String {
    let s = ms.div_ceil(1000);
    match s {
        0..60 => format!("in {s} s"),
        60..3600 => format!("in {} min", s.div_ceil(60)),
        _ => {
            let m = s.div_ceil(60);
            match m % 60 {
                0 => format!("in {} h", m / 60),
                r => format!("in {} h {r} min", m / 60),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_newest_first_and_counts_repeats() {
        let mut m = Messages::default();
        m.push("Copied", 1);
        m.push("Copied", 2);
        m.push("  ", 3);
        m.push("No link here", 4);
        m.push("Copied", 5);
        let texts: Vec<_> = m.newest().map(|x| (x.text.as_str(), x.times, x.at_ms)).collect();
        assert_eq!(texts, [("Copied", 1, 5), ("No link here", 1, 4), ("Copied", 2, 2)]);
        let first = m.newest().last().unwrap().id;
        assert!(m.remove(first));
        assert!(!m.remove(first));
        assert_eq!(m.len(), 2);
        assert!(m.get(first).is_none());
        m.clear();
        assert!(m.is_empty());
    }

    #[test]
    fn keeps_the_last_ones() {
        let mut m = Messages::default();
        for i in 0..KEEP + 5 {
            m.push(&format!("m{i}"), i as i64);
        }
        assert_eq!(m.len(), KEEP);
        assert_eq!(m.newest().next().unwrap().text, format!("m{}", KEEP + 4));
        assert_eq!(m.newest().last().unwrap().text, "m5");
        // Ids stay unique as old rows drop off.
        let mut ids: Vec<_> = m.newest().map(|x| x.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), KEEP);
    }

    #[test]
    fn wait_labels() {
        assert_eq!(wait_label(0), "in 0 s");
        assert_eq!(wait_label(400), "in 1 s");
        assert_eq!(wait_label(59_000), "in 59 s");
        assert_eq!(wait_label(60_000), "in 1 min");
        assert_eq!(wait_label(61_000), "in 2 min");
        assert_eq!(wait_label(3_600_000), "in 1 h");
        assert_eq!(wait_label(3_900_000), "in 1 h 5 min");
    }
}
