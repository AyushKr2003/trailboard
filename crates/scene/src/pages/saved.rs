//! Saved pages (M12 step 9): pages kept under a name, like bookmarks, to
//! open again from the command line (`:saved`, `b` / `B`). Our own design.
//!
//! - A name is one line, at most [`crate::mark::NAME_CHARS`] characters, and
//!   is unique ignoring case: saving under a name that exists replaces its
//!   address.
//! - With no name given, the page's title is used (else its host), and a
//!   page saved already under any name is left as it is.
//! - Newest first.

use serde::{Deserialize, Serialize};

use crate::mark::clean_name;
use crate::palette::{self, Candidate};

/// Saved pages kept at most.
pub const MAX_SAVED: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPage {
    pub name: String,
    pub url: String,
    /// When it was saved, ms since the epoch (UTC).
    #[serde(default)]
    pub added: i64,
}

/// What [`Saved::save`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Added(String),
    /// The name was taken: its address is now this page's.
    Replaced(String),
    /// No name was given and the page is saved already, under this one.
    Already(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    pages: Vec<SavedPage>,
}

/// `https://www.example.com/a` → `example.com` (a name of last resort).
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.strip_prefix("www.").unwrap_or(host)
}

impl Saved {
    /// Pages read back from disk: bad names and repeats (the later ones) dropped.
    pub fn from_pages(pages: Vec<SavedPage>) -> Saved {
        let mut s = Saved::default();
        for p in pages {
            match clean_name(&p.name) {
                Some(name) if !p.url.trim().is_empty() && s.index(&name).is_none() && s.pages.len() < MAX_SAVED => {
                    s.pages.push(SavedPage { name, url: p.url.trim().to_string(), added: p.added })
                }
                _ => {}
            }
        }
        s
    }

    /// Newest first.
    pub fn pages(&self) -> &[SavedPage] {
        &self.pages
    }

    fn index(&self, name: &str) -> Option<usize> {
        let name = name.trim().to_lowercase();
        self.pages.iter().position(|p| p.name.to_lowercase() == name)
    }

    /// Saves `url` under `name`, else under `title`, else its host. Err if
    /// there is no address, or no name can be made.
    pub fn save(&mut self, name: Option<&str>, url: &str, title: &str, now: i64) -> Result<Outcome, String> {
        let url = url.trim();
        if url.is_empty() {
            return Err("This card has no page to save".into());
        }
        let given = name.and_then(clean_name);
        if given.is_none()
            && let Some(p) = self.pages.iter().find(|p| p.url == url)
        {
            return Ok(Outcome::Already(p.name.clone()));
        }
        let Some(name) = given.or_else(|| clean_name(title)).or_else(|| clean_name(host(url))) else {
            return Err("Give the saved page a name".into());
        };
        let page = SavedPage { name: name.clone(), url: url.to_string(), added: now };
        match self.index(&name) {
            Some(i) => {
                self.pages.remove(i);
                self.pages.insert(0, page);
                Ok(Outcome::Replaced(name))
            }
            None => {
                self.pages.insert(0, page);
                self.pages.truncate(MAX_SAVED);
                Ok(Outcome::Added(name))
            }
        }
    }

    /// The page `typed` names: that name (case ignored), else the best
    /// fuzzy match.
    pub fn find(&self, typed: &str) -> Option<&SavedPage> {
        if let Some(i) = self.index(typed) {
            return Some(&self.pages[i]);
        }
        let cands: Vec<Candidate> = self.pages.iter().map(|p| Candidate { text: &p.name, alt: &p.url }).collect();
        palette::rank(typed, &cands).first().map(|r| &self.pages[r.index])
    }

    /// Forgets the page called `name` exactly (case ignored).
    pub fn remove(&mut self, name: &str) -> Option<SavedPage> {
        self.index(name).map(|i| self.pages.remove(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_what_is_given() {
        let mut s = Saved::default();
        assert_eq!(s.save(Some("  Rust\n docs "), "https://doc.rust-lang.org/", "The Rust docs", 1), Ok(Outcome::Added("Rust docs".into())));
        assert_eq!(s.save(None, "https://example.com/a", "Example A", 2), Ok(Outcome::Added("Example A".into())));
        assert_eq!(s.save(None, "https://www.untitled.org/x", "  ", 3), Ok(Outcome::Added("untitled.org".into())));
        // Saved already, with no name asked for.
        assert_eq!(s.save(None, "https://example.com/a", "Other", 4), Ok(Outcome::Already("Example A".into())));
        assert_eq!(s.pages().iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["untitled.org", "Example A", "Rust docs"]);
        assert!(s.save(None, " ", "t", 5).is_err());
    }

    #[test]
    fn a_name_is_unique_ignoring_case() {
        let mut s = Saved::default();
        s.save(Some("Docs"), "https://a.org", "", 1).unwrap();
        s.save(Some("other"), "https://c.org", "", 2).unwrap();
        assert_eq!(s.save(Some("docs"), "https://b.org", "", 3), Ok(Outcome::Replaced("docs".into())));
        assert_eq!(s.pages().len(), 2);
        assert_eq!(s.pages()[0].url, "https://b.org", "newest first");
        // The same page may be saved under a second name on purpose.
        assert_eq!(s.save(Some("Again"), "https://b.org", "", 4), Ok(Outcome::Added("Again".into())));
    }

    #[test]
    fn find_and_remove() {
        let mut s = Saved::default();
        s.save(Some("Rust docs"), "https://doc.rust-lang.org/", "", 1).unwrap();
        s.save(Some("Arch wiki"), "https://wiki.archlinux.org/", "", 2).unwrap();
        assert_eq!(s.find("RUST DOCS").map(|p| p.url.as_str()), Some("https://doc.rust-lang.org/"));
        assert_eq!(s.find("arch").map(|p| p.name.as_str()), Some("Arch wiki"));
        assert_eq!(s.find("zzzz"), None);
        assert_eq!(s.remove("arch"), None, "only the exact name");
        assert!(s.remove("arch WIKI").is_some());
        assert_eq!(s.pages().len(), 1);
    }

    #[test]
    fn read_back_drops_bad_rows() {
        let p = |name: &str, url: &str| SavedPage { name: name.into(), url: url.into(), added: 0 };
        let s = Saved::from_pages(vec![p("A", "https://a"), p(" ", "https://b"), p("a", "https://c"), p("C", "")]);
        assert_eq!(s.pages(), &[p("A", "https://a")]);
    }
}
