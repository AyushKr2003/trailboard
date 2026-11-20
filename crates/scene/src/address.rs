//! Moving about by address (M12 step 12): up the path (`gu`), to the
//! site's root (`gU`), the last number in the address up or down
//! (`Ctrl+a` / `Ctrl+x`), the address without tracking parts (`yp`), and
//! picking a page's "next" / "previous" link (`]]` / `[[`). Plain string
//! work on the address as the page reports it; our own rules.

/// The parts of an address: `scheme://authority`, the path, and what
/// follows it (`?query#fragment`, with its mark), as written.
struct Parts<'a> {
    origin: &'a str,
    path: &'a str,
    query: &'a str,
    fragment: &'a str,
}

/// Splits a hierarchical address (`http`, `https`, `ftp`, `file`…); `None`
/// for `data:`, `about:`, `view-source:` and the like.
fn parts(url: &str) -> Option<Parts<'_>> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return None;
    }
    let auth_len = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let origin = &url[..scheme.len() + 3 + auth_len];
    let after = &url[origin.len()..];
    let (before_frag, fragment) = match after.find('#') {
        Some(i) => (&after[..i], &after[i..]),
        None => (after, ""),
    };
    let (path, query) = match before_frag.find('?') {
        Some(i) => (&before_frag[..i], &before_frag[i..]),
        None => (before_frag, ""),
    };
    Some(Parts { origin, path, query, fragment })
}

/// The site an address is on: its host, without a port or a login
/// (`news.example.com`). `None` for addresses that have none.
pub fn site(url: &str) -> Option<&str> {
    let p = parts(url)?;
    let auth = p.origin.split_once("://")?.1;
    let host = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
    let host = match host.strip_prefix('[') {
        // An IPv6 address keeps its brackets.
        Some(v6) => &host[..v6.find(']').map_or(host.len(), |i| i + 2)],
        None => host.split(':').next().unwrap_or(host),
    };
    (!host.is_empty()).then_some(host)
}

/// `n` steps up the path (`gu`, `3gu`): the query and fragment go first
/// (that is one step), then a segment at a time
/// (`/a/b/c` → `/a/b/` → `/a/`). `None` when already at the root.
pub fn up(url: &str, n: u32) -> Option<String> {
    let p = parts(url)?;
    let mut path = p.path.to_string();
    let mut steps = n.max(1);
    if !p.query.is_empty() || !p.fragment.is_empty() {
        steps -= 1;
    }
    for _ in 0..steps {
        let trimmed = path.trim_end_matches('/');
        if trimmed.is_empty() {
            break;
        }
        path = match trimmed.rfind('/') {
            Some(i) => trimmed[..=i].to_string(),
            None => "/".to_string(),
        };
    }
    if path.is_empty() {
        path = "/".into();
    }
    let out = format!("{}{path}", p.origin);
    (out != url && out != format!("{url}/")).then_some(out)
}

/// The site's root (`gU`): `https://example.com/`. `None` when already there.
pub fn root(url: &str) -> Option<String> {
    let p = parts(url)?;
    let out = format!("{}/", p.origin);
    (out != url && p.origin != url).then_some(out)
}

/// The last number in the address after the site, moved by `delta`
/// (`Ctrl+a` / `Ctrl+x`, a count moves further): `page/7` → `page/8`. Zeros
/// in front keep the width (`007` → `008`); it stops at 0. `None` when
/// there is no number, or it can't move.
pub fn step_number(url: &str, delta: i64) -> Option<String> {
    let p = parts(url)?;
    let start = p.origin.len();
    let tail = &url[start..];
    let bytes = tail.as_bytes();
    let end = bytes.iter().rposition(u8::is_ascii_digit)? + 1;
    let begin = bytes[..end].iter().rposition(|b| !b.is_ascii_digit()).map_or(0, |i| i + 1);
    let digits = &tail[begin..end];
    // Very long digit runs (ids, hashes) are still numbers; keep to u128.
    let value: u128 = digits.parse().ok()?;
    let moved = if delta >= 0 {
        value.checked_add(delta as u128)?
    } else {
        value.saturating_sub(delta.unsigned_abs() as u128)
    };
    if moved == value {
        return None;
    }
    let text = if digits.starts_with('0') && digits.len() > 1 {
        format!("{moved:0width$}", width = digits.len())
    } else {
        moved.to_string()
    };
    Some(format!("{}{}{text}{}", p.origin, &tail[..begin], &tail[end..]))
}

/// Query keys that only say where a click came from.
const TRACKING: &[&str] = &[
    "fbclid", "gclid", "gclsrc", "dclid", "gbraid", "wbraid", "msclkid", "yclid", "twclid", "ttclid", "li_fat_id",
    "mc_cid", "mc_eid", "_hsenc", "_hsmi", "__hssc", "__hstc", "__hsfp", "hsctatracking", "igshid", "igsh", "mkt_tok",
    "oly_anon_id", "oly_enc_id", "vero_id", "wickedid", "_openstat", "ref_src", "ref_url", "s_cid", "srsltid", "si",
];

/// Whether query key `key` is a tracking one: those in [`TRACKING`], and
/// any starting `utm_`, `pk_` or `mtm_` (campaign tags).
fn tracking(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    TRACKING.contains(&k.as_str()) || ["utm_", "pk_", "mtm_"].iter().any(|p| k.starts_with(p))
}

/// The address without its tracking parts (`yp`): campaign tags and click
/// ids are dropped from the query; the rest, in order, and the fragment
/// stay. Addresses without a query come back as they are.
pub fn clean(url: &str) -> String {
    let Some(p) = parts(url) else { return url.to_string() };
    if p.query.is_empty() {
        return url.to_string();
    }
    let kept: Vec<&str> = p.query[1..]
        .split('&')
        .filter(|pair| !pair.is_empty() && !tracking(pair.split('=').next().unwrap_or(pair)))
        .collect();
    let query = if kept.is_empty() { String::new() } else { format!("?{}", kept.join("&")) };
    format!("{}{}{query}{}", p.origin, p.path, p.fragment)
}

/// The page's source, for a card beside it (`gf`).
pub fn view_source(url: &str) -> String {
    if url.starts_with("view-source:") { url.to_string() } else { format!("view-source:{url}") }
}

/// The address to keep for a card at `stored` whose page reports
/// `reported`: Chromium reports a source view (`view-source:…`) by the
/// address it shows the source of, so a source card keeps its prefix
/// (it would come back as the page itself after sleeping).
pub fn reported(stored: &str, reported: &str) -> String {
    if stored.starts_with("view-source:") && !reported.starts_with("view-source:") {
        view_source(reported)
    } else {
        reported.to_string()
    }
}

/// A link a page offers that might go to its next or previous page: its
/// `rel` (from `<a>` or `<link>`), its text (or label) and its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagerLink {
    pub rel: String,
    pub text: String,
    pub href: String,
}

/// Words that mean "next" or "previous" as a whole link text.
const NEXT_WORDS: &[&str] = &["next", "next page", "next ›", "next »", "older", "older posts", "older entries", "more"];
const PREV_WORDS: &[&str] =
    &["previous", "prev", "previous page", "‹ previous", "« previous", "‹ prev", "« prev", "newer", "newer posts", "newer entries"];
const NEXT_SIGNS: &[&str] = &["›", "»", "→", ">", ">>", "⟩", "❯"];
const PREV_SIGNS: &[&str] = &["‹", "«", "←", "<", "<<", "⟨", "❮"];

/// Link text as compared: lower case, spaces squeezed, arrows and dots at
/// the ends kept apart from the words (`Next »` → `next »`).
fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// How well link `l` says "next" (`next`) or "previous": a `rel` is best,
/// then the whole text being one of the words, then the text starting
/// with one (`Next chapter`), then a lone arrow. 0: not at all.
fn score(l: &PagerLink, next: bool) -> u8 {
    let rels = if next { &["next"][..] } else { &["prev", "previous"][..] };
    if l.rel.split_whitespace().any(|r| rels.iter().any(|w| r.eq_ignore_ascii_case(w))) {
        return 4;
    }
    let t = norm(&l.text);
    if t.is_empty() || t.chars().count() > 30 {
        return 0;
    }
    let (words, signs) = if next { (NEXT_WORDS, NEXT_SIGNS) } else { (PREV_WORDS, PREV_SIGNS) };
    let bare = t.trim_matches(|c: char| !c.is_alphanumeric()).trim();
    if words.contains(&t.as_str()) || (!bare.is_empty() && words.contains(&bare)) {
        return 3;
    }
    let first = bare.split([' ', ':', '-']).next().unwrap_or("");
    if !first.is_empty() && words.iter().any(|w| !w.contains(' ') && *w == first && *w != "more") {
        return 2;
    }
    if signs.contains(&t.as_str()) {
        return 1;
    }
    0
}

/// Which of `links` goes to the next page (`next`) or the previous one,
/// from a page at `current`: the best [`score`], the first of those in
/// the page. Links back to `current`, or that aren't web or file
/// addresses, never count.
pub fn pick_pager(links: &[PagerLink], next: bool, current: &str) -> Option<usize> {
    let openable = |h: &str| ["http:", "https:", "file:"].iter().any(|s| h.get(..s.len()).is_some_and(|p| p.eq_ignore_ascii_case(s)));
    let same = |h: &str| h.trim_end_matches('#') == current.trim_end_matches('#');
    links
        .iter()
        .enumerate()
        .filter(|(_, l)| openable(&l.href) && !same(&l.href))
        .map(|(i, l)| (i, score(l, next)))
        .filter(|&(_, s)| s > 0)
        // The highest score; among equals the first (max_by_key keeps the last).
        .max_by_key(|&(i, s)| (s, std::cmp::Reverse(i)))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites() {
        assert_eq!(site("https://news.example.com/a?b"), Some("news.example.com"));
        assert_eq!(site("http://user:pw@example.com:8080/x"), Some("example.com"));
        assert_eq!(site("http://[::1]:8000/"), Some("[::1]"));
        assert_eq!(site("file:///tmp/a.html"), None);
        assert_eq!(site("about:blank"), None);
    }

    #[test]
    fn going_up() {
        assert_eq!(up("https://e.com/a/b/c", 1).as_deref(), Some("https://e.com/a/b/"));
        assert_eq!(up("https://e.com/a/b/c/", 1).as_deref(), Some("https://e.com/a/b/"));
        assert_eq!(up("https://e.com/a/b/c", 2).as_deref(), Some("https://e.com/a/"));
        assert_eq!(up("https://e.com/a/b/c", 9).as_deref(), Some("https://e.com/"));
        // The query and fragment are the first step.
        assert_eq!(up("https://e.com/a/b?x=1#y", 1).as_deref(), Some("https://e.com/a/b"));
        assert_eq!(up("https://e.com/a/b?x=1", 2).as_deref(), Some("https://e.com/a/"));
        assert_eq!(up("https://e.com/", 1), None);
        assert_eq!(up("https://e.com", 1), None);
        assert_eq!(up("file:///home/me/doc.html", 1).as_deref(), Some("file:///home/me/"));
        assert_eq!(up("data:text/html,hi", 1), None);
    }

    #[test]
    fn the_root() {
        assert_eq!(root("https://e.com:8443/a/b?c#d").as_deref(), Some("https://e.com:8443/"));
        assert_eq!(root("https://e.com/"), None);
        assert_eq!(root("https://e.com"), None);
        assert_eq!(root("about:blank"), None);
    }

    #[test]
    fn numbers_step() {
        assert_eq!(step_number("https://e.com/page/7", 1).as_deref(), Some("https://e.com/page/8"));
        assert_eq!(step_number("https://e.com/page/7", -3).as_deref(), Some("https://e.com/page/4"));
        assert_eq!(step_number("https://e.com/page/2", -5).as_deref(), Some("https://e.com/page/0"));
        assert_eq!(step_number("https://e.com/page/0", -1), None);
        // The last number, in the query too; the site's digits never.
        assert_eq!(step_number("https://web2.e.com/a1/b?p=9#top", 1).as_deref(), Some("https://web2.e.com/a1/b?p=10#top"));
        assert_eq!(step_number("https://web2.e.com/about", 1), None);
        // Width is kept for zero-padded numbers.
        assert_eq!(step_number("http://e.com/img007.jpg", 1).as_deref(), Some("http://e.com/img008.jpg"));
        assert_eq!(step_number("http://e.com/img099.jpg", 1).as_deref(), Some("http://e.com/img100.jpg"));
        assert_eq!(step_number("http://e.com/img010.jpg", -1).as_deref(), Some("http://e.com/img009.jpg"));
        assert_eq!(step_number("http://e.com/9", 10).as_deref(), Some("http://e.com/19"));
    }

    #[test]
    fn tracking_goes() {
        assert_eq!(clean("https://e.com/a?utm_source=x&id=3&fbclid=abc#s"), "https://e.com/a?id=3#s");
        assert_eq!(clean("https://e.com/a?UTM_Medium=x&gclid=1"), "https://e.com/a");
        assert_eq!(clean("https://e.com/a?q=utm_source"), "https://e.com/a?q=utm_source");
        assert_eq!(clean("https://e.com/a"), "https://e.com/a");
        assert_eq!(clean("https://e.com/a?&&x=1&"), "https://e.com/a?x=1");
        assert_eq!(clean("about:blank"), "about:blank");
    }

    #[test]
    fn source() {
        assert_eq!(view_source("https://e.com/"), "view-source:https://e.com/");
        assert_eq!(view_source("view-source:https://e.com/"), "view-source:https://e.com/");
        assert_eq!(reported("view-source:https://e.com/", "https://e.com/"), "view-source:https://e.com/");
        assert_eq!(reported("view-source:https://e.com/", "https://e.com/b"), "view-source:https://e.com/b");
        assert_eq!(reported("https://e.com/", "https://e.com/b"), "https://e.com/b");
    }

    fn l(rel: &str, text: &str, href: &str) -> PagerLink {
        PagerLink { rel: rel.into(), text: text.into(), href: href.into() }
    }

    #[test]
    fn pager_prefers_rel_then_words_then_arrows() {
        let here = "https://e.com/list/2";
        let links = [
            l("", "»", "https://e.com/list/9"),
            l("", "Next chapter", "https://e.com/ch/3"),
            l("", "  Next  › ", "https://e.com/list/3"),
            l("next", "", "https://e.com/list/3?rel"),
            l("", "Previous", "https://e.com/list/1"),
            l("", "‹", "https://e.com/list/0"),
        ];
        assert_eq!(pick_pager(&links, true, here), Some(3));
        assert_eq!(pick_pager(&links[..3], true, here), Some(2));
        assert_eq!(pick_pager(&links[..2], true, here), Some(1));
        assert_eq!(pick_pager(&links[..1], true, here), Some(0));
        assert_eq!(pick_pager(&links, false, here), Some(4));
        assert_eq!(pick_pager(&links[5..], false, here), Some(0));
        assert_eq!(pick_pager(&[l("prev", "", "https://e.com/1")], false, here), Some(0));
        assert_eq!(pick_pager(&[l("noopener previous", "", "https://e.com/1")], false, here), Some(0));
    }

    #[test]
    fn pager_skips_what_cannot_be_the_link() {
        let here = "https://e.com/a";
        assert_eq!(pick_pager(&[l("", "Next", "javascript:void(0)")], true, here), None);
        assert_eq!(pick_pager(&[l("", "Next", "https://e.com/a#")], true, here), None);
        assert_eq!(pick_pager(&[l("", "Nextcloud setup guide for beginners and more", "https://x/")], true, here), None);
        assert_eq!(pick_pager(&[l("", "Read more", "https://x/")], true, here), None);
        assert_eq!(pick_pager(&[l("", "Newsletter", "https://x/")], false, here), None);
        assert_eq!(pick_pager(&[], true, here), None);
        // The first of equals.
        assert_eq!(pick_pager(&[l("", "Next", "https://e.com/b"), l("", "next", "https://e.com/c")], true, here), Some(0));
    }
}
