//! Ad and tracker blocking (M8): which filter lists are used and when they are
//! fetched again, and the sites where the user turned blocking off. The
//! matching itself happens in `web::shield`; the lists and the allowed sites
//! are kept by `storage::privacy`.

use crate::icon;

/// A filter list Trailboard fetches and keeps up to date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterList {
    /// Its file name in the filters folder, without `.txt`.
    pub id: &'static str,
    pub url: &'static str,
}

/// EasyList (ads) and EasyPrivacy (trackers), the lists most blockers start from.
pub const LISTS: [FilterList; 2] = [
    FilterList { id: "easylist", url: "https://easylist.to/easylist/easylist.txt" },
    FilterList { id: "easyprivacy", url: "https://easylist.to/easylist/easyprivacy.txt" },
];

/// The user's own rules, never fetched: `filters/user.txt`, in the same syntax.
pub const USER_LIST: &str = "user";

/// A list older than this is fetched again (both lists say `Expires: 4 days`).
pub const REFRESH_SECS: u64 = 4 * 24 * 3600;

/// After a failed fetch, wait this long before trying again in the same run.
pub const RETRY_SECS: u64 = 3600;

/// Whether a list saved `age` seconds ago (`None`: not saved) should be fetched.
pub fn is_stale(age: Option<u64>) -> bool {
    age.is_none_or(|a| a >= REFRESH_SECS)
}

/// What a fetched list must look like to replace the saved one: an Adblock
/// Plus header and a reasonable number of lines. A captive portal's login
/// page or a truncated download is kept out.
pub fn looks_like_list(text: &str) -> bool {
    let head = text.trim_start_matches('\u{feff}').trim_start();
    head.starts_with("[Adblock") && text.lines().count() >= 100
}

/// Whether the entry `site` (as [`icon::site`] gives it) covers the page at
/// `url`: the same site, or a subdomain of it.
pub fn covers(site: &str, url: &str) -> bool {
    icon::site(url).is_some_and(|s| s == site || s.strip_suffix(site).is_some_and(|rest| rest.ends_with('.')))
}

/// The sites where blocking is off, each a site as [`icon::site`] gives it
/// (`example.com`, `localhost:8080`). A site covers its subdomains too.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllowedSites {
    pub sites: Vec<String>,
}

impl AllowedSites {
    /// The entry that covers `url`, if blocking is off there.
    pub fn covering(&self, url: &str) -> Option<&str> {
        self.sites.iter().map(String::as_str).find(|s| covers(s, url))
    }

    pub fn allows(&self, url: &str) -> bool {
        self.covering(url).is_some()
    }

    /// Turns blocking off for `url`'s site, or back on if it was off (by
    /// removing whichever entry covered it). Returns the site and whether
    /// blocking is now on there; `None` for pages that aren't web pages.
    pub fn toggle(&mut self, url: &str) -> Option<(String, bool)> {
        if let Some(entry) = self.covering(url).map(str::to_string) {
            self.sites.retain(|s| *s != entry);
            return Some((entry, true));
        }
        let site = icon::site(url)?;
        self.sites.push(site.clone());
        self.sites.sort();
        Some((site, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staleness() {
        assert!(is_stale(None));
        assert!(!is_stale(Some(0)));
        assert!(!is_stale(Some(REFRESH_SECS - 1)));
        assert!(is_stale(Some(REFRESH_SECS)));
    }

    #[test]
    fn list_sanity() {
        let good = format!("[Adblock Plus 2.0]\n! Title: EasyList\n{}", "||ads.example^\n".repeat(200));
        assert!(looks_like_list(&good));
        assert!(looks_like_list(&format!("\u{feff}{good}")));
        assert!(!looks_like_list("<html><body>Sign in to the Wi-Fi</body></html>"));
        assert!(!looks_like_list("[Adblock Plus 2.0]\n||ads.example^\n"), "too short: cut off");
    }

    #[test]
    fn allowed_sites_cover_subdomains_and_toggle() {
        let mut a = AllowedSites::default();
        assert!(!a.allows("https://news.example.com/x"));
        assert_eq!(a.toggle("https://www.example.com/page"), Some(("example.com".into(), false)));
        assert!(a.allows("https://example.com/"));
        assert!(a.allows("https://news.example.com/x"));
        assert!(!a.allows("https://badexample.com/"), "not a subdomain");
        assert!(!a.allows("https://example.com.evil.net/"));
        // From a subdomain, the entry that covers it is removed.
        assert_eq!(a.toggle("https://news.example.com/"), Some(("example.com".into(), true)));
        assert!(a.sites.is_empty());
        assert_eq!(a.toggle("http://localhost:8761/a.html"), Some(("localhost:8761".into(), false)));
        assert!(a.allows("http://localhost:8761/b"));
        assert!(!a.allows("http://localhost:9000/b"));
        assert_eq!(a.toggle("file:///home/a.html"), None);
        assert_eq!(a.toggle("about:blank"), None);
        assert!(!a.allows("file:///home/a.html"));
        assert!(covers("example.com", "https://a.b.example.com/"));
        assert!(!covers("example.com", "https://example.org/"));
    }
}
