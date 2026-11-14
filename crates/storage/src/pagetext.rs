//! The history index, `$DATA/page-text.db`: every page shown in a card, with
//! its title and text, searchable by word (SQLite FTS5). Shared by all spaces.
//!
//! Schema (version 1, in `PRAGMA user_version`):
//! - `pages(id, url UNIQUE, title, seen_ms)`: one row per address, last seen time.
//! - `pages_text` (FTS5: `title`, `url`, `body`; `rowid` = `pages.id`), with
//!   the `unicode61` tokenizer folding diacritics, and 2- and 3-letter prefix
//!   indexes so typing a word's start is fast.
//!
//! Results rank by BM25 with the title counting 4×, the address 2×, the
//! text 1×; ties go to the most recently seen. Only the newest
//! [`MAX_PAGES`] pages are kept. Rules for what is kept live in
//! `scene::pagetext`. Thumbnails are not stored yet.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use scene::pagetext::{self, MAX_TEXT, MAX_TITLE};

pub const PAGETEXT_VERSION: i32 = 1;
/// The most pages kept; older ones go first.
pub const MAX_PAGES: i64 = 5000;
/// Pruning runs once this many pages beyond [`MAX_PAGES`] have built up.
const PRUNE_SLACK: i64 = 100;

pub fn pagetext_path(data_dir: &Path) -> PathBuf {
    data_dir.join("page-text.db")
}

/// A page found in history.
#[derive(Debug, Clone, PartialEq)]
pub struct Visit {
    pub url: String,
    pub title: String,
    pub seen_ms: i64,
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Sql(rusqlite::Error),
    /// The file was written by a newer build; it is left untouched.
    Newer(i32),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Sql(e) => write!(f, "{e}"),
            Error::Newer(v) => write!(f, "page-text.db version {v} is newer than this build supports"),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sql(e)
    }
}

pub struct PageIndex {
    db: Connection,
}

impl PageIndex {
    /// Opens (or creates) the index in `data_dir`.
    pub fn open(data_dir: &Path) -> Result<PageIndex, Error> {
        std::fs::create_dir_all(data_dir).map_err(Error::Io)?;
        let db = Connection::open(pagetext_path(data_dir))?;
        // WAL keeps a crash from tearing the file and makes small writes cheap.
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(db)
    }

    /// An index that lives in memory only (tests, or when the file can't be opened).
    pub fn in_memory() -> Result<PageIndex, Error> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> Result<PageIndex, Error> {
        let version: i32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > PAGETEXT_VERSION {
            return Err(Error::Newer(version));
        }
        if version < 1 {
            db.execute_batch(
                "BEGIN;
                 CREATE TABLE IF NOT EXISTS pages(
                     id INTEGER PRIMARY KEY,
                     url TEXT NOT NULL UNIQUE,
                     title TEXT NOT NULL,
                     seen_ms INTEGER NOT NULL);
                 CREATE INDEX IF NOT EXISTS pages_seen ON pages(seen_ms);
                 CREATE VIRTUAL TABLE IF NOT EXISTS pages_text USING fts5(
                     title, url, body,
                     tokenize = 'unicode61 remove_diacritics 2',
                     prefix = '2 3');
                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }
        Ok(PageIndex { db })
    }

    /// Records that `url` was seen at `now_ms` with this title and text,
    /// replacing what was kept for it. Pages `scene::pagetext::keeps` rejects are
    /// ignored. Returns whether it was recorded.
    pub fn record(&mut self, url: &str, title: &str, text: &str, now_ms: i64) -> Result<bool, Error> {
        if !pagetext::keeps(url) {
            return Ok(false);
        }
        let title = pagetext::clamp(title.trim(), MAX_TITLE);
        let text = pagetext::clamp(text, MAX_TEXT);
        let tx = self.db.transaction()?;
        let id: i64 = tx.query_row(
            "INSERT INTO pages(url, title, seen_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(url) DO UPDATE SET title = excluded.title, seen_ms = excluded.seen_ms
             RETURNING id",
            params![url, title, now_ms],
            |r| r.get(0),
        )?;
        tx.execute("DELETE FROM pages_text WHERE rowid = ?1", [id])?;
        tx.execute(
            "INSERT INTO pages_text(rowid, title, url, body) VALUES (?1, ?2, ?3, ?4)",
            params![id, title, url, text],
        )?;
        tx.commit()?;
        self.prune(MAX_PAGES, PRUNE_SLACK)?;
        Ok(true)
    }

    /// Keeps the newest `keep` pages once more than `keep + slack` are kept.
    fn prune(&mut self, keep: i64, slack: i64) -> Result<(), Error> {
        let count: i64 = self.db.query_row("SELECT count(*) FROM pages", [], |r| r.get(0))?;
        if count <= keep + slack {
            return Ok(());
        }
        let tx = self.db.transaction()?;
        let newest = "SELECT id FROM pages ORDER BY seen_ms DESC, id DESC LIMIT ?1";
        tx.execute(&format!("DELETE FROM pages_text WHERE rowid NOT IN ({newest})"), [keep])?;
        let gone = tx.execute(&format!("DELETE FROM pages WHERE id NOT IN ({newest})"), [keep])?;
        log::info!("history: forgot the {gone} oldest pages, keeping {keep}");
        tx.commit()?;
        Ok(())
    }

    /// Pages containing every word of `query` (word starts count), best first.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Visit>, Error> {
        let Some(q) = pagetext::fts_query(query) else { return Ok(Vec::new()) };
        let mut st = self.db.prepare_cached(
            "SELECT p.url, p.title, p.seen_ms FROM pages_text t JOIN pages p ON p.id = t.rowid
             WHERE pages_text MATCH ?1
             ORDER BY bm25(pages_text, 4.0, 2.0, 1.0), p.seen_ms DESC
             LIMIT ?2",
        )?;
        let rows = st.query_map(params![q, limit as i64], |r| {
            Ok(Visit { url: r.get(0)?, title: r.get(1)?, seen_ms: r.get(2)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The most recently seen pages, newest first.
    pub fn recent(&self, limit: usize) -> Result<Vec<Visit>, Error> {
        let mut st = self.db.prepare_cached("SELECT url, title, seen_ms FROM pages ORDER BY seen_ms DESC, id DESC LIMIT ?1")?;
        let rows = st.query_map([limit as i64], |r| Ok(Visit { url: r.get(0)?, title: r.get(1)?, seen_ms: r.get(2)? }))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn len(&self) -> Result<usize, Error> {
        Ok(self.db.query_row("SELECT count(*) FROM pages", [], |r| r.get::<_, i64>(0))? as usize)
    }

    pub fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx() -> PageIndex {
        PageIndex::in_memory().unwrap()
    }

    fn urls(v: &[Visit]) -> Vec<&str> {
        v.iter().map(|v| v.url.as_str()).collect()
    }

    #[test]
    fn finds_by_title_text_and_word_starts() {
        let mut i = idx();
        i.record("https://a.test/", "Learning Rust", "ownership and borrowing", 1).unwrap();
        i.record("https://b.test/", "Cooking", "a recipe for rustic bread", 2).unwrap();
        assert_eq!(urls(&i.search("borrow", 10).unwrap()), ["https://a.test/"]);
        // Both contain a word starting with "rust"; the title match ranks first.
        assert_eq!(urls(&i.search("rust", 10).unwrap()), ["https://a.test/", "https://b.test/"]);
        // Every word must be there.
        assert_eq!(urls(&i.search("rust bread", 10).unwrap()), ["https://b.test/"]);
        assert!(i.search("ownershipx", 10).unwrap().is_empty());
        // Case and diacritics fold; syntax characters are just separators.
        i.record("https://c.test/", "Café Crème", "", 3).unwrap();
        assert_eq!(urls(&i.search("CAFE cre", 10).unwrap()), ["https://c.test/"]);
        assert!(i.search("\"*(:", 10).unwrap().is_empty());
        // An address finds its page first (its words also start other words).
        assert_eq!(i.search("b.test", 10).unwrap()[0].url, "https://b.test/");
    }

    #[test]
    fn same_url_is_one_page_with_the_latest_text() {
        let mut i = idx();
        i.record("https://a.test/", "Old", "first words", 1).unwrap();
        i.record("https://a.test/", "New", "second words", 5).unwrap();
        assert_eq!(i.len().unwrap(), 1);
        assert!(i.search("first", 10).unwrap().is_empty());
        let v = i.search("second", 10).unwrap();
        assert_eq!(v, vec![Visit { url: "https://a.test/".into(), title: "New".into(), seen_ms: 5 }]);
    }

    #[test]
    fn ties_go_to_the_most_recent_and_limit_holds() {
        let mut i = idx();
        for (n, t) in [(1, 10), (2, 30), (3, 20)] {
            i.record(&format!("https://{n}.test/"), "Same", "", t).unwrap();
        }
        assert_eq!(urls(&i.search("same", 2).unwrap()), ["https://2.test/", "https://3.test/"]);
        assert_eq!(urls(&i.recent(10).unwrap()), ["https://2.test/", "https://3.test/", "https://1.test/"]);
    }

    #[test]
    fn skips_pages_that_are_not_kept() {
        let mut i = idx();
        assert!(!i.record("data:text/html,hi", "x", "hi", 1).unwrap());
        assert!(!i.record("chrome-error://chromewebdata/", "x", "hi", 1).unwrap());
        assert!(i.is_empty().unwrap());
    }

    #[test]
    fn keeps_only_the_newest_pages() {
        let mut i = idx();
        for n in 0..12 {
            i.record(&format!("https://{n}.test/"), "Page", "", n).unwrap();
            i.prune(5, 2).unwrap();
        }
        // Pruned back to 5 whenever it passed 5 + 2.
        let n = i.len().unwrap();
        assert!((5..=7).contains(&n), "{n}");
        let recent = i.recent(1).unwrap();
        assert_eq!(recent[0].url, "https://11.test/");
        // The oldest are gone from the text index too.
        assert!(i.search("0", 10).unwrap().is_empty());
        assert_eq!(i.search("page", 100).unwrap().len(), n);
    }

    #[test]
    fn long_text_is_cut_and_reopening_keeps_pages() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut i = PageIndex::open(dir.path()).unwrap();
            let long = "word ".repeat(MAX_TEXT) + "tailword";
            i.record("https://a.test/", "A", &long, 1).unwrap();
        }
        let i = PageIndex::open(dir.path()).unwrap();
        assert_eq!(i.len().unwrap(), 1);
        assert_eq!(i.search("word", 5).unwrap().len(), 1);
        assert!(i.search("tailword", 5).unwrap().is_empty());
    }

    #[test]
    fn newer_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        {
            let db = Connection::open(pagetext_path(dir.path())).unwrap();
            db.pragma_update(None, "user_version", 9).unwrap();
        }
        assert!(matches!(PageIndex::open(dir.path()), Err(Error::Newer(9))));
    }
}
