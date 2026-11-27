//! What the start page shows: the user's pinned sites in named groups (each
//! drawn as a small card, like the board's), in the user's order, and which
//! of its parts are on (greeting, clock, recent pages, groups; the greeting's
//! words are the page's, by its own clock). The page
//! edits a copy and sends it back whole; [`StartPage::cleaned`] is what the
//! app keeps of it, so a page can't store junk.

/// Groups a start page holds at most.
pub const MAX_GROUPS: usize = 8;
/// Sites a group holds at most.
pub const MAX_PER_GROUP: usize = 12;
/// Longest title, group name or user name kept, in characters.
pub const MAX_TITLE: usize = 60;
/// What the first group is called when pinning makes one.
pub const FIRST_GROUP: &str = "Pinned";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// What the link says; empty means a name made from the site's host.
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub pins: Vec<Pin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPage {
    pub groups: Vec<Group>,
    /// "Good evening" (and [`name`](Self::name)) under the clock.
    pub show_greeting: bool,
    /// The user's name for the greeting; empty for none.
    pub name: String,
    /// The time above the search field.
    pub show_clock: bool,
    /// The clock counts 0–23 instead of 1–12 with AM/PM.
    pub hour24: bool,
    /// The clock shows seconds too, and ticks each second.
    pub show_seconds: bool,
    /// The pinned groups.
    pub show_pins: bool,
    /// The last pages opened, as small cards.
    pub show_recent: bool,
}

impl Default for StartPage {
    fn default() -> Self {
        StartPage {
            groups: Vec::new(),
            show_greeting: true,
            name: String::new(),
            show_clock: true,
            hour24: false,
            show_seconds: false,
            show_pins: true,
            show_recent: true,
        }
    }
}

/// Whether `url` is something a pin may open: a web address.
pub fn pinnable(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|r| !r.is_empty() && !r.starts_with('/') && !r.chars().any(char::is_whitespace))
}

fn cut(s: &str) -> String {
    s.trim().chars().take(MAX_TITLE).collect()
}

impl StartPage {
    /// The same, keeping only web addresses, each once on the whole page,
    /// at most [`MAX_PER_GROUP`] per group and [`MAX_GROUPS`] groups, with
    /// names trimmed and cut to [`MAX_TITLE`]. Unnamed groups are called
    /// "Group"; empty groups stay (the user just made them).
    pub fn cleaned(mut self) -> Self {
        let mut seen = std::collections::HashSet::new();
        self.name = cut(&self.name);
        self.groups = self
            .groups
            .into_iter()
            .take(MAX_GROUPS)
            .map(|g| {
                let name = cut(&g.name);
                let pins = g
                    .pins
                    .into_iter()
                    .map(|p| Pin { title: cut(&p.title), url: p.url.trim().to_string() })
                    .filter(|p| pinnable(&p.url) && seen.insert(p.url.clone()))
                    .take(MAX_PER_GROUP)
                    .collect();
                Group { name: if name.is_empty() { "Group".into() } else { name }, pins }
            })
            .collect();
        self
    }

    pub fn pins(&self) -> impl Iterator<Item = &Pin> {
        self.groups.iter().flat_map(|g| &g.pins)
    }

    pub fn is_pinned(&self, url: &str) -> bool {
        self.pins().any(|p| p.url == url)
    }

    /// Pins `url` at the end of the first group with room, making
    /// [`FIRST_GROUP`] if there is none. Returns whether it was added.
    pub fn pin(&mut self, url: &str, title: &str) -> bool {
        if !pinnable(url) || self.is_pinned(url) {
            return false;
        }
        let pin = Pin { title: cut(title), url: url.to_string() };
        if let Some(g) = self.groups.iter_mut().find(|g| g.pins.len() < MAX_PER_GROUP) {
            g.pins.push(pin);
        } else if self.groups.len() < MAX_GROUPS {
            self.groups.push(Group { name: FIRST_GROUP.into(), pins: vec![pin] });
        } else {
            return false;
        }
        true
    }

    /// Takes `url` off, wherever it is. Returns whether it was there.
    pub fn unpin(&mut self, url: &str) -> bool {
        let before = self.pins().count();
        for g in &mut self.groups {
            g.pins.retain(|p| p.url != url);
        }
        self.pins().count() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(url: &str) -> Pin {
        Pin { title: String::new(), url: url.into() }
    }

    #[test]
    fn only_web_addresses_can_be_pinned() {
        for ok in ["https://a.org", "http://localhost:8080/x"] {
            assert!(pinnable(ok), "{ok}");
        }
        for bad in ["", "https://", "https:///x", "javascript:alert(1)", "file:///etc", "a.org", "https://a b.org"] {
            assert!(!pinnable(bad), "{bad}");
        }
    }

    #[test]
    fn cleaning_drops_junk_duplicates_and_extras() {
        let mut pins = vec![
            Pin { title: "  A  ".into(), url: " https://a.org/ ".into() },
            pin("https://a.org/"),
            pin("javascript:x"),
            Pin { title: "x".repeat(200), url: "https://b.org/".into() },
        ];
        pins.extend((0..40).map(|i| pin(&format!("https://{i}.org/"))));
        let groups = vec![
            Group { name: " Work ".into(), pins },
            Group { name: "".into(), pins: vec![pin("https://a.org/"), pin("https://c.org/")] },
        ];
        let mut many = groups.clone();
        many.extend((0..20).map(|_| Group { name: "g".into(), pins: vec![] }));
        let s = StartPage { groups: many, name: "  Sam ".into(), ..StartPage::default() }.cleaned();
        assert_eq!(s.groups.len(), MAX_GROUPS);
        assert_eq!(s.name, "Sam");
        let work = &s.groups[0];
        assert_eq!(work.name, "Work");
        assert_eq!(work.pins.len(), MAX_PER_GROUP);
        assert_eq!(work.pins[0], Pin { title: "A".into(), url: "https://a.org/".into() });
        assert_eq!(work.pins[1].title.chars().count(), MAX_TITLE);
        // a.org is already in Work: once on the whole page.
        assert_eq!(s.groups[1].name, "Group");
        assert_eq!(s.groups[1].pins, vec![pin("https://c.org/")]);
    }

    #[test]
    fn pin_and_unpin() {
        let mut s = StartPage::default();
        assert!(s.pin("https://a.org/", " A "));
        assert_eq!(s.groups[0].name, FIRST_GROUP);
        assert!(!s.pin("https://a.org/", "again"));
        assert!(!s.pin("about:blank", ""));
        assert!(s.is_pinned("https://a.org/") && s.groups[0].pins[0].title == "A");
        assert!(s.unpin("https://a.org/"));
        assert!(!s.unpin("https://a.org/"));
        assert_eq!(s.groups.len(), 1, "the group stays");
        for i in 0..MAX_PER_GROUP + 2 {
            assert!(s.pin(&format!("https://{i}.org/"), ""));
        }
        assert_eq!(s.groups.len(), 2, "a full group spills into a new one");
        assert_eq!(s.groups[1].pins.len(), 2);
    }
}
