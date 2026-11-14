//! The links visible in a page, for link hints. [`SCRIPT`] runs in the page's
//! main frame and answers with one console message starting with [`PREFIX`]:
//! the page's layout width, then one line per link (`x,y,w,h` of the visible
//! part in the page's CSS px, a tab, its absolute URL). Our own design.

/// Prefix of the console message with a page's links.
pub const PREFIX: &str = "\u{1}trailboard-links\u{1}";

/// The most links one message reports.
pub const MAX_LINKS: usize = 500;

/// Collects links that can be seen: on screen, not hidden, not covered by
/// something else at their middle. Only links with a URL a card can open.
pub const SCRIPT: &str = r#"(() => {
  const W = innerWidth, H = innerHeight, out = [];
  for (const a of document.querySelectorAll('a[href], area[href]')) {
    if (out.length >= 500) break;
    const href = a.href;
    if (typeof href !== 'string' || !/^(https?|file):/i.test(href)) continue;
    for (const r of a.getClientRects()) {
      const x0 = Math.max(r.left, 0), y0 = Math.max(r.top, 0);
      const x1 = Math.min(r.right, W), y1 = Math.min(r.bottom, H);
      if (x1 - x0 < 2 || y1 - y0 < 2) continue;
      const hit = document.elementFromPoint((x0 + x1) / 2, (y0 + y1) / 2);
      if (!hit || !(a === hit || a.contains(hit) || (a.tagName === 'AREA' && hit.tagName === 'IMG'))) continue;
      out.push([x0, y0, x1 - x0, y1 - y0].map(v => Math.round(v)).join(',') + '\t' + href);
      break;
    }
  }
  console.log('\u0001trailboard-links\u0001' + W + '\n' + out.join('\n'));
})();"#;

/// One link: where its visible part is (CSS px of the page) and where it goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub rect: [f32; 4],
    pub url: String,
}

/// The page's layout width and its links, from a message without [`PREFIX`].
/// Malformed lines are skipped; `None` when even the width is missing.
pub fn parse(body: &str) -> Option<(f32, Vec<Link>)> {
    let mut lines = body.split('\n');
    let width: f32 = lines.next()?.trim().parse().ok().filter(|w: &f32| *w > 0.0)?;
    let links = lines
        .filter_map(|line| {
            let (nums, url) = line.split_once('\t')?;
            let v: Vec<f32> = nums.split(',').map(|n| n.trim().parse().ok()).collect::<Option<_>>()?;
            let rect: [f32; 4] = v.try_into().ok()?;
            let url = url.trim();
            let ok = ["http:", "https:", "file:"].iter().any(|s| url.get(..s.len()).is_some_and(|p| p.eq_ignore_ascii_case(s)));
            (ok && rect[2] > 0.0 && rect[3] > 0.0).then(|| Link { rect, url: url.to_string() })
        })
        .take(MAX_LINKS)
        .collect();
    Some((width, links))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_width_and_links() {
        let (w, links) = parse("1024\n10,20,30,12\thttps://a.example/x\n0,0,5,5\tfile:///tmp/p.html").unwrap();
        assert_eq!(w, 1024.0);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], Link { rect: [10.0, 20.0, 30.0, 12.0], url: "https://a.example/x".into() });
        assert_eq!(links[1].url, "file:///tmp/p.html");
    }

    #[test]
    fn no_links_is_fine() {
        assert_eq!(parse("800\n"), Some((800.0, vec![])));
        assert_eq!(parse("800"), Some((800.0, vec![])));
    }

    #[test]
    fn bad_input_is_skipped() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("wide\n1,2,3,4\thttps://x"), None);
        let (_, links) = parse("800\n1,2,3\thttps://x\n1,2,3,4\tjavascript:alert(1)\n1,2,0,4\thttps://x\nno tab\n1,2,3,4\tHTTPS://ok").unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "HTTPS://ok");
    }

    #[test]
    fn script_uses_the_prefix() {
        let escaped = PREFIX.replace('\u{1}', "\\u0001");
        assert!(SCRIPT.contains(&escaped));
        assert!(SCRIPT.contains(&MAX_LINKS.to_string()));
    }
}
