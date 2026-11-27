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
    /// Fetched again once this old (seconds).
    pub refresh: u64,
}

/// The lists uBlock Origin starts with, which is what makes sites work
/// there: EasyList (ads), EasyPrivacy (trackers) and Peter Lowe's list (ad
/// servers); uBlock's own lists (ads and the "turn off your ad blocker"
/// walls, using stand-ins and scriptlets, see `web::resources`; badware;
/// privacy; *Unbreak*, exceptions for what the other lists break; *Quick
/// fixes*, updated within hours); and the EasyList Cookie List (cookie
/// banners), published on fanboy.co.nz.
pub const LISTS: [FilterList; 9] = [
    FilterList { id: "easylist", url: "https://easylist.to/easylist/easylist.txt", refresh: REFRESH_SECS },
    FilterList { id: "easyprivacy", url: "https://easylist.to/easylist/easyprivacy.txt", refresh: REFRESH_SECS },
    FilterList { id: "easylist-cookie", url: "https://secure.fanboy.co.nz/fanboy-cookiemonster.txt", refresh: REFRESH_SECS },
    FilterList { id: "ublock-filters", url: "https://ublockorigin.github.io/uAssets/filters/filters.txt", refresh: REFRESH_SECS },
    FilterList { id: "ublock-badware", url: "https://ublockorigin.github.io/uAssets/filters/badware.txt", refresh: REFRESH_SECS },
    FilterList { id: "ublock-privacy", url: "https://ublockorigin.github.io/uAssets/filters/privacy.txt", refresh: REFRESH_SECS },
    FilterList { id: "ublock-unbreak", url: "https://ublockorigin.github.io/uAssets/filters/unbreak.txt", refresh: REFRESH_SECS },
    FilterList { id: "ublock-quick-fixes", url: "https://ublockorigin.github.io/uAssets/filters/quick-fixes.txt", refresh: QUICK_REFRESH_SECS },
    FilterList {
        id: "peter-lowe",
        url: "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=adblockplus&showintro=1&mimetype=plaintext",
        refresh: REFRESH_SECS,
    },
];

/// The user's own rules, never fetched: `filters/user.txt`, in the same syntax.
pub const USER_LIST: &str = "user";

/// A list older than this is fetched again (most say `Expires: 4 days` or `5 days`).
pub const REFRESH_SECS: u64 = 4 * 24 * 3600;

/// uBlock's *Quick fixes* says `Expires: 8 hours`; a day is plenty here.
pub const QUICK_REFRESH_SECS: u64 = 24 * 3600;

/// After a failed fetch, wait this long before trying again in the same run.
pub const RETRY_SECS: u64 = 3600;

/// Whether a list saved `age` seconds ago (`None`: not saved) should be
/// fetched, for a list fetched again every `refresh` seconds.
pub fn is_stale(age: Option<u64>, refresh: u64) -> bool {
    age.is_none_or(|a| a >= refresh)
}

/// What a fetched list must look like to replace the saved one: an Adblock
/// Plus header (or uBlock's `! Title:`) and a reasonable number of lines. A
/// captive portal's login page or a truncated download is kept out.
pub fn looks_like_list(text: &str) -> bool {
    let head = text.trim_start_matches('\u{feff}').trim_start();
    (head.starts_with("[Adblock") || head.starts_with("! Title:")) && text.lines().count() >= 100
}

/// What a fetched part of a list (an `!#include`) must look like: filter
/// syntax, not a web page. Parts may be short.
pub fn looks_like_part(text: &str) -> bool {
    let head = text.trim_start_matches('\u{feff}').trim_start();
    !head.starts_with('<') && (head.is_empty() || head.starts_with('!') || head.starts_with('['))
}

/// The parts a list pulls in with `!#include name` (uBlock's lists are split
/// by year), outside `!#if` blocks that don't apply: plain file names
/// beside it only, nothing with a path.
pub fn includes(text: &str) -> Vec<String> {
    let mut kept = String::new();
    keep_applicable(text, &mut kept, None);
    kept.lines().filter_map(include_name).map(str::to_string).collect()
}

/// The part an `!#include` line names, if it is a plain file name.
fn include_name(line: &str) -> Option<&str> {
    let n = line.trim().strip_prefix("!#include ")?.trim();
    let plain = n.len() <= 64 && n.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && !n.starts_with('.');
    (!n.is_empty() && plain).then_some(n)
}

/// Where the part `name` of the list at `list_url` is: beside it.
pub fn include_url(list_url: &str, name: &str) -> String {
    let base = list_url.split(['?', '#']).next().unwrap_or(list_url);
    let dir = base.rfind('/').map_or(base, |i| &base[..=i]);
    format!("{dir}{name}")
}

/// The file id (as in [`FilterList::id`]) the part `name` of list `list` is saved under.
pub fn include_id(list: &str, name: &str) -> String {
    format!("{list}--{}", name.strip_suffix(".txt").unwrap_or(name))
}

/// Whether a `!#if` token holds for Trailboard, as uBlock Origin defines
/// them: a Chromium browser with a user stylesheet, without HTML filtering
/// (it can't rewrite a page's source) and not on a phone.
fn env_holds(token: &str) -> bool {
    matches!(token, "env_chromium" | "cap_user_stylesheet" | "env_desktop")
}

/// Evaluates a `!#if` condition: tokens, `!`, `&&`, `||` and brackets.
fn condition_holds(expr: &str) -> bool {
    fn or(t: &[&str], i: &mut usize) -> bool {
        let mut v = and(t, i);
        while t.get(*i) == Some(&"||") {
            *i += 1;
            v |= and(t, i);
        }
        v
    }
    fn and(t: &[&str], i: &mut usize) -> bool {
        let mut v = not(t, i);
        while t.get(*i) == Some(&"&&") {
            *i += 1;
            v &= not(t, i);
        }
        v
    }
    fn not(t: &[&str], i: &mut usize) -> bool {
        match t.get(*i).copied() {
            Some("!") => {
                *i += 1;
                !not(t, i)
            }
            Some("(") => {
                *i += 1;
                let v = or(t, i);
                if t.get(*i) == Some(&")") {
                    *i += 1;
                }
                v
            }
            Some(tok) => {
                *i += 1;
                env_holds(tok)
            }
            None => false,
        }
    }
    let spaced = expr.replace("&&", " && ").replace("||", " || ").replace('!', " ! ").replace('(', " ( ").replace(')', " ) ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    or(&tokens, &mut 0)
}

/// A list ready for the engine: the `!#if` blocks that don't apply to
/// Trailboard dropped, and each `!#include` replaced by the part's text
/// (`part` gives it by name; a part not there yet is left out). Parts
/// don't include further parts.
pub fn preprocess(text: &str, part: PartText) -> String {
    let mut out = String::with_capacity(text.len());
    keep_applicable(text, &mut out, Some(part));
    out
}

/// Gives the text of a list's part by name (`None`: not there).
pub type PartText<'a> = &'a dyn Fn(&str) -> Option<String>;

fn keep_applicable(text: &str, out: &mut String, part: Option<PartText>) {
    // One entry per open `!#if`: whether its lines are kept.
    let mut open: Vec<bool> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(expr) = t.strip_prefix("!#if ") {
            let outer = open.last().copied().unwrap_or(true);
            open.push(outer && condition_holds(expr));
            continue;
        }
        if t == "!#else" {
            let outer = open.len() < 2 || open[open.len() - 2];
            if let Some(last) = open.last_mut() {
                *last = outer && !*last;
            }
            continue;
        }
        if t == "!#endif" {
            open.pop();
            continue;
        }
        if !open.last().copied().unwrap_or(true) {
            continue;
        }
        if let Some(part) = part
            && t.starts_with("!#include ")
        {
            if let Some(text) = include_name(t).and_then(part) {
                keep_applicable(&text, out, None);
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
}

/// How many rules a filter list holds: its lines that aren't blank, a
/// `!` comment or a `[Adblock …]` header.
pub fn count_rules(text: &str) -> usize {
    text.lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}'))
        .filter(|l| !l.is_empty() && !l.starts_with('!') && !l.starts_with('['))
        .count()
}

/// What the user is told after `filters/user.txt` changed and was read
/// again: `rules` in it now (`None`: the file is gone), and whether blocking
/// is `on` in the settings.
pub fn user_rules_message(rules: Option<usize>, on: bool) -> String {
    let what = match rules {
        None => "Your filter rules were removed".to_string(),
        Some(1) => "Your filter rules reloaded: 1 rule".to_string(),
        Some(n) => format!("Your filter rules reloaded: {n} rules"),
    };
    if on { what } else { format!("{what} (blocking is off in the settings)") }
}

/// Whether the entry `site` (as [`icon::site`] gives it) covers the page at
/// `url`: the same site, or a subdomain of it.
pub fn covers(site: &str, url: &str) -> bool {
    icon::site(url).is_some_and(|s| s == site || s.strip_suffix(site).is_some_and(|rest| rest.ends_with('.')))
}

/// The count on a card's title bar for `blocked` things the blocker stopped
/// on its page (requests blocked and elements hidden): nothing for none, and a capped figure so the mark stays small.
pub fn blocked_badge(blocked: u32) -> Option<String> {
    match blocked {
        0 => None,
        1..=99 => Some(blocked.to_string()),
        _ => Some("99+".into()),
    }
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

    /// Turns blocking back on for `site` (an entry as listed). Returns
    /// whether it was listed.
    pub fn remove(&mut self, site: &str) -> bool {
        let before = self.sites.len();
        self.sites.retain(|s| s != site);
        self.sites.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_rules_and_words_the_reload() {
        assert_eq!(count_rules(""), 0);
        let text = "\u{feff}[Adblock Plus 2.0]\n! Title: mine\n\n||ads.example^\n  example.com##.banner  \n!x\n@@||ok.example^\n";
        assert_eq!(count_rules(text), 3);
        assert_eq!(user_rules_message(Some(3), true), "Your filter rules reloaded: 3 rules");
        assert_eq!(user_rules_message(Some(1), true), "Your filter rules reloaded: 1 rule");
        assert_eq!(user_rules_message(Some(0), true), "Your filter rules reloaded: 0 rules");
        assert_eq!(user_rules_message(None, true), "Your filter rules were removed");
        assert_eq!(user_rules_message(Some(2), false), "Your filter rules reloaded: 2 rules (blocking is off in the settings)");
    }

    #[test]
    fn blocked_badge_text() {
        assert_eq!(blocked_badge(0), None);
        assert_eq!(blocked_badge(1).as_deref(), Some("1"));
        assert_eq!(blocked_badge(99).as_deref(), Some("99"));
        assert_eq!(blocked_badge(100).as_deref(), Some("99+"));
        assert_eq!(blocked_badge(u32::MAX).as_deref(), Some("99+"));
    }

    #[test]
    fn staleness() {
        assert!(is_stale(None, REFRESH_SECS));
        assert!(!is_stale(Some(0), REFRESH_SECS));
        assert!(!is_stale(Some(REFRESH_SECS - 1), REFRESH_SECS));
        assert!(is_stale(Some(REFRESH_SECS), REFRESH_SECS));
        assert!(is_stale(Some(QUICK_REFRESH_SECS), QUICK_REFRESH_SECS));
        let ids: std::collections::HashSet<_> = LISTS.iter().map(|l| l.id).collect();
        assert_eq!(ids.len(), LISTS.len(), "ids are unique");
        assert!(!ids.contains(USER_LIST));
        assert!(LISTS.iter().all(|l| !l.id.contains("--")), "`--` marks an included part");
        assert!(LISTS.iter().filter(|l| l.id.starts_with("ublock-")).all(|l| l.url.starts_with("https://ublockorigin.github.io/uAssets/filters/")));
    }

    #[test]
    fn includes_are_plain_names_beside_the_list() {
        let text = "! Title: uBlock filters\n!#include filters-2024.txt\n!#include ../etc/passwd\n!#include https://evil/x.txt\n!#include  filters-general.txt \n!#if env_mobile\n!#include filters-mobile.txt\n!#endif\n||a^";
        assert_eq!(includes(text), ["filters-2024.txt", "filters-general.txt"]);
        assert_eq!(
            include_url("https://ublockorigin.github.io/uAssets/filters/filters.txt", "filters-2024.txt"),
            "https://ublockorigin.github.io/uAssets/filters/filters-2024.txt"
        );
        assert_eq!(include_url("https://a.example/x/list.php?y=1/2", "p.txt"), "https://a.example/x/p.txt");
        assert_eq!(include_id("ublock-filters", "filters-2024.txt"), "ublock-filters--filters-2024");
    }

    #[test]
    fn preprocess_keeps_what_applies_and_splices_parts() {
        let text = "\
||always^
!#if env_firefox
||firefox^
!#else
||not-firefox^
!#endif
!#if !cap_html_filtering
||no-html^
!#if env_mobile
||mobile^
!#endif
!#endif
!#if cap_html_filtering
||html^
!#else
!#if env_chromium && !env_safari
||chromium^
!#endif
!#endif
!#include part.txt
!#include missing.txt
||last^
";
        let part = |name: &str| (name == "part.txt").then(|| "!#if env_safari\n||safari^\n!#endif\n||part^\n!#include part.txt\n".to_string());
        let out = preprocess(text, &part);
        let rules: Vec<&str> = out.lines().filter(|l| !l.starts_with('!')).collect();
        assert_eq!(rules, ["||always^", "||not-firefox^", "||no-html^", "||chromium^", "||part^", "||last^"]);
        assert!(condition_holds("(env_chromium || env_firefox) && !env_mobile"));
        assert!(!condition_holds("env_firefox"));
        assert!(!condition_holds(""));
    }

    #[test]
    fn parts_must_be_filter_text() {
        assert!(looks_like_part("! Title: x\n||a^"));
        assert!(looks_like_part(""));
        assert!(!looks_like_part("<!DOCTYPE html><title>404</title>"));
        let ubo = format!("! Title: uBlock filters\n{}", "||a.example^\n".repeat(200));
        assert!(looks_like_list(&ubo));
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
        assert!(!a.remove("example.com"), "not listed");
        assert!(a.remove("localhost:8761"));
        assert!(!a.allows("http://localhost:8761/b"));
        assert!(covers("example.com", "https://a.b.example.com/"));
        assert!(!covers("example.com", "https://example.org/"));
    }
}
