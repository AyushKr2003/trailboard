//! Next / previous page links (`]]` / `[[`, M12 step 12). [`SCRIPT`] runs
//! in the page's main frame and answers with one console message starting
//! with [`PREFIX`]: one line per candidate, `rel`, a tab, its text, a tab,
//! its absolute address. `<link rel=next|prev>` comes first, then links
//! whose `rel` or short text could say next or previous; the app picks one
//! (`scene::address::pick_pager`). Our own design.

/// Prefix of the console message with a page's candidates.
pub const PREFIX: &str = "\u{1}trailboard-pager\u{1}";

/// The most candidates one message reports.
pub const MAX_LINKS: usize = 200;

/// Collects the candidates (see the module). Links must have an address;
/// text is the link's own, else its label or title, squeezed and cut to 40
/// characters; tabs and line breaks never reach the message.
pub const SCRIPT: &str = r#"(() => {
  const out = [];
  const clean = s => (s || '').replace(/\s+/g, ' ').trim().slice(0, 40);
  const add = (rel, text, href) => {
    if (out.length < 200 && href) out.push(clean(rel) + '\t' + clean(text) + '\t' + href.replace(/[\t\n\r]/g, ''));
  };
  for (const l of document.querySelectorAll('link[rel][href]'))
    if (/(^|\s)(next|prev|previous)(\s|$)/i.test(l.rel)) add(l.rel, '', l.href);
  const loose = /next|prev|older|newer|more|[›»→<>«‹←⟨⟩❮❯]/i;
  for (const a of document.querySelectorAll('a[href]')) {
    const rel = a.rel || '';
    const text = a.innerText || a.getAttribute('aria-label') || a.title || '';
    const hasRel = /(^|\s)(next|prev|previous)(\s|$)/i.test(rel);
    if (!hasRel && (text.length > 60 || !loose.test(text))) continue;
    if (!hasRel && !a.getClientRects().length) continue;
    add(rel, text, a.href);
  }
  console.log('\u0001trailboard-pager\u0001' + out.join('\n'));
})();"#;

/// One candidate as the page reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub rel: String,
    pub text: String,
    pub href: String,
}

/// The candidates, from a message without [`PREFIX`]; malformed lines are skipped.
pub fn parse(body: &str) -> Vec<Link> {
    body.split('\n')
        .filter_map(|line| {
            let mut p = line.splitn(3, '\t');
            let (rel, text, href) = (p.next()?, p.next()?, p.next()?.trim());
            (!href.is_empty()).then(|| Link { rel: rel.trim().to_string(), text: text.trim().to_string(), href: href.to_string() })
        })
        .take(MAX_LINKS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_candidates() {
        let got = parse("next\t\thttps://e.com/2\n\tOlder posts »\thttps://e.com/p/3\nbroken line\n\tx\t\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], Link { rel: "next".into(), text: String::new(), href: "https://e.com/2".into() });
        assert_eq!(got[1].text, "Older posts »");
        assert!(parse("").is_empty());
    }

    #[test]
    fn script_uses_the_prefix_and_limit() {
        assert!(SCRIPT.contains(&PREFIX.replace('\u{1}', "\\u0001")));
        assert!(SCRIPT.contains(&format!("out.length < {MAX_LINKS}")));
    }
}
