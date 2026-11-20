//! What typed text opens: an address, a search with the chosen engine, or a
//! search with a keyword's site (`gh foo` searches GitHub for “foo”).
//!
//! A keyword is a short word, then a space, then the query. Addresses are
//! recognised first, so a keyword never hides one (`w3.org` stays an
//! address); a keyword alone (`gh`) or with nothing after it is searched for
//! with the default engine like any other word. Keywords are matched
//! ignoring case. The table is the user's (Settings → Search), saved by
//! `storage::settings`; [`defaults`] is what a new or older settings file
//! starts with.

use crate::settings::SearchEngine;

/// One keyword: `key` then a space and a query opens `url` with the query
/// in place of `{}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    /// What is typed (lower case, see [`valid_key`]).
    pub key: String,
    /// The site's name, as the address bar says it (*Search GitHub for “foo”*).
    pub name: String,
    /// The results page, with `{}` where the query goes (see [`valid_url`]).
    pub url: String,
}

impl Keyword {
    pub fn new(key: &str, name: &str, url: &str) -> Keyword {
        Keyword { key: key.to_string(), name: name.to_string(), url: url.to_string() }
    }

    /// The results page for `query` (not yet encoded).
    pub fn url_for(&self, query: &str) -> String {
        fill(&self.url, query)
    }
}

/// The placeholder for the query in a keyword's address.
pub const PLACEHOLDER: &str = "{}";
/// The longest keyword.
pub const MAX_KEY: usize = 16;

/// The keywords a new settings file has: the common engines and a few
/// sites people search often.
pub fn defaults() -> Vec<Keyword> {
    let engine = |key: &str, e: SearchEngine| Keyword::new(key, e.name(), &e.url(PLACEHOLDER));
    vec![
        engine("d", SearchEngine::DuckDuckGo),
        engine("g", SearchEngine::Google),
        engine("b", SearchEngine::Bing),
        engine("br", SearchEngine::Brave),
        engine("sp", SearchEngine::Startpage),
        engine("k", SearchEngine::Kagi),
        engine("wa", SearchEngine::WolframAlpha),
        Keyword::new("w", "Wikipedia", "https://en.wikipedia.org/w/index.php?search={}"),
        Keyword::new("gh", "GitHub", "https://github.com/search?q={}"),
        Keyword::new("yt", "YouTube", "https://www.youtube.com/results?search_query={}"),
        Keyword::new("map", "OpenStreetMap", "https://www.openstreetmap.org/search?query={}"),
        Keyword::new("mdn", "MDN", "https://developer.mozilla.org/en-US/search?q={}"),
        Keyword::new("rs", "Docs.rs", "https://docs.rs/releases/search?query={}"),
        Keyword::new("cr", "crates.io", "https://crates.io/search?q={}"),
        Keyword::new("aw", "ArchWiki", "https://wiki.archlinux.org/index.php?search={}"),
        Keyword::new("aur", "AUR", "https://aur.archlinux.org/packages?K={}"),
    ]
}

/// A keyword is 1 to [`MAX_KEY`] letters, digits, `-` or `_` (no dot, so
/// it can't look like an address).
pub fn valid_key(key: &str) -> bool {
    let n = key.chars().count();
    (1..=MAX_KEY).contains(&n) && key.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// A keyword's address is an http(s) address with one `{}`.
pub fn valid_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://")) && url.matches(PLACEHOLDER).count() == 1 && !url.contains(char::is_whitespace)
}

/// Why `key`, `name` and `url` can't be a keyword, if they can't.
pub fn check(key: &str, name: &str, url: &str) -> Result<Keyword, &'static str> {
    let key = key.trim().to_lowercase();
    let url = url.trim();
    if !valid_key(&key) {
        return Err("A keyword is one short word: letters, digits, - or _");
    }
    if !valid_url(url) {
        return Err("The address needs http:// or https:// and one {} where the search goes");
    }
    let name = name.trim();
    let name = if name.is_empty() { host(url) } else { name };
    Ok(Keyword::new(&key, name, url))
}

/// `url`'s host, for a keyword with no name.
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.strip_prefix("www.").unwrap_or(host)
}

/// `list` with `k` added, replacing a keyword with the same key (in place).
pub fn put(list: &mut Vec<Keyword>, k: Keyword) {
    match list.iter_mut().find(|o| o.key == k.key) {
        Some(o) => *o = k,
        None => list.push(k),
    }
}

/// What some typed text opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// An address (with a scheme, added if it was a bare host).
    Address(String),
    /// A search: `site` is the engine's or keyword's name, `query` what is
    /// searched for, `keyword` the keyword used (none: the default engine).
    Search { url: String, site: String, query: String, keyword: Option<String> },
}

impl Resolved {
    pub fn url(&self) -> &str {
        match self {
            Resolved::Address(u) | Resolved::Search { url: u, .. } => u,
        }
    }

    pub fn into_url(self) -> String {
        match self {
            Resolved::Address(u) | Resolved::Search { url: u, .. } => u,
        }
    }

    /// What Enter does, as a list row says it (*Search GitHub for “foo”*).
    pub fn label(&self) -> String {
        match self {
            Resolved::Address(_) => String::new(),
            Resolved::Search { site, query, .. } => format!("Search {site} for “{query}”"),
        }
    }
}

/// Turn what the user typed into an address: keep addresses, add a scheme
/// to bare hosts, search with a keyword's site when the text starts with
/// one and a space, and send everything else to `engine`.
pub fn resolve(input: &str, engine: SearchEngine, keywords: &[Keyword]) -> Option<Resolved> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if is_address(s) {
        return Some(Resolved::Address(s.to_string()));
    }
    if looks_like_host(s) {
        return Some(Resolved::Address(format!("https://{s}")));
    }
    if let Some((word, rest)) = s.split_once(char::is_whitespace) {
        let query = rest.trim();
        if !query.is_empty()
            && let Some(k) = keywords.iter().find(|k| k.key.eq_ignore_ascii_case(word))
        {
            return Some(Resolved::Search { url: k.url_for(query), site: k.name.clone(), query: query.to_string(), keyword: Some(k.key.clone()) });
        }
    }
    Some(Resolved::Search { url: engine.url(&encode_query(s)), site: engine.name().to_string(), query: s.to_string(), keyword: None })
}

/// Text that is already an address (it has a scheme we open).
fn is_address(s: &str) -> bool {
    ["http://", "https://", "file://", "about:", "chrome://", "data:"].iter().any(|p| s.starts_with(p))
}

/// One word with a dot (not at its end), or `localhost`.
fn looks_like_host(s: &str) -> bool {
    !s.contains(char::is_whitespace) && (s.contains('.') || s.starts_with("localhost")) && !s.ends_with('.')
}

/// `template` with `query` (encoded) in place of `{}`; with no `{}`, after it.
pub fn fill(template: &str, query: &str) -> String {
    let q = encode_query(query.trim());
    if template.contains(PLACEHOLDER) {
        template.replacen(PLACEHOLDER, &q, 1)
    } else {
        format!("{template}{q}")
    }
}

/// `s` encoded for a query string: spaces as `+`, everything but letters,
/// digits and `-_.~` as `%XX`.
pub fn encode_query(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DDG: SearchEngine = SearchEngine::DuckDuckGo;

    fn url(input: &str) -> Option<String> {
        resolve(input, DDG, &defaults()).map(Resolved::into_url)
    }

    #[test]
    fn addresses_and_plain_searches() {
        assert_eq!(url("example.com").as_deref(), Some("https://example.com"));
        assert_eq!(url("http://a.b/c").as_deref(), Some("http://a.b/c"));
        assert_eq!(url("rust lang").as_deref(), Some("https://duckduckgo.com/?q=rust+lang"));
        assert_eq!(url("c++").as_deref(), Some("https://duckduckgo.com/?q=c%2B%2B"));
        assert_eq!(resolve("rust lang", SearchEngine::Google, &[]).map(Resolved::into_url).as_deref(), Some("https://www.google.com/search?q=rust+lang"));
        assert_eq!(url("  "), None);
    }

    #[test]
    fn keywords() {
        assert_eq!(url("g rust").as_deref(), Some("https://www.google.com/search?q=rust"));
        assert_eq!(url("gh  foo bar ").as_deref(), Some("https://github.com/search?q=foo+bar"));
        assert_eq!(url("w trail").as_deref(), Some("https://en.wikipedia.org/w/index.php?search=trail"));
        assert_eq!(url("GH foo").as_deref(), Some("https://github.com/search?q=foo"), "case is ignored");
        let r = resolve("gh foo", DDG, &defaults()).unwrap();
        assert_eq!(r.label(), "Search GitHub for “foo”");
        assert!(matches!(r, Resolved::Search { keyword: Some(ref k), .. } if k == "gh"));
        // The default engine says its own name.
        assert_eq!(resolve("rust", DDG, &defaults()).unwrap().label(), "Search DuckDuckGo for “rust”");
    }

    #[test]
    fn keyword_needs_a_space_and_a_query_and_an_address_wins() {
        // Alone, or without a query: a plain search for the word.
        assert_eq!(url("gh").as_deref(), Some("https://duckduckgo.com/?q=gh"));
        assert_eq!(url("gh ").as_deref(), Some("https://duckduckgo.com/?q=gh"));
        // Glued to the query: not the keyword.
        assert_eq!(url("ghfoo").as_deref(), Some("https://duckduckgo.com/?q=ghfoo"));
        // Addresses are never keyword searches.
        assert_eq!(url("w3.org").as_deref(), Some("https://w3.org"));
        assert_eq!(url("g.co").as_deref(), Some("https://g.co"));
        assert_eq!(url("https://g rust").as_deref(), Some("https://g rust"));
        // Not a keyword: the whole text is searched.
        assert_eq!(url("zz top").as_deref(), Some("https://duckduckgo.com/?q=zz+top"));
        // An empty table: no keywords at all.
        assert_eq!(resolve("g rust", DDG, &[]).map(Resolved::into_url).as_deref(), Some("https://duckduckgo.com/?q=g+rust"));
    }

    #[test]
    fn filling_and_checking() {
        assert_eq!(fill("https://x.org/s?q={}&l=en", "a b&c"), "https://x.org/s?q=a+b%26c&l=en");
        assert_eq!(fill("https://x.org/s?q=", "a"), "https://x.org/s?q=a");
        assert!(valid_key("gh") && valid_key("my-wiki") && valid_key("ü"));
        assert!(!valid_key("") && !valid_key("a.b") && !valid_key("a b") && !valid_key(&"x".repeat(17)));
        assert!(valid_url("https://x.org/?q={}"));
        assert!(!valid_url("x.org/?q={}") && !valid_url("https://x.org/?q=") && !valid_url("https://x.org/{}/{}"));
        assert_eq!(check(" GH ", "", "https://www.github.com/search?q={}").unwrap(), Keyword::new("gh", "github.com", "https://www.github.com/search?q={}"));
        assert!(check("a b", "X", "https://x.org/?q={}").is_err());
        assert!(check("x", "X", "https://x.org/").is_err());
        let mut list = defaults();
        let n = list.len();
        put(&mut list, Keyword::new("gh", "Code", "https://c.example/?q={}"));
        assert_eq!(list.len(), n);
        assert_eq!(list.iter().find(|k| k.key == "gh").unwrap().name, "Code");
        put(&mut list, Keyword::new("new", "New", "https://n.example/?q={}"));
        assert_eq!(list.len(), n + 1);
        // Every default is valid and unique.
        let d = defaults();
        assert!(d.iter().all(|k| valid_key(&k.key) && valid_url(&k.url)));
        let mut keys: Vec<_> = d.iter().map(|k| &k.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), d.len());
    }
}
