//! What can be clicked in a page, for link hints: links, and buttons, fields
//! and other controls. [`SCRIPT`] runs in the page's main frame and answers
//! with one console message starting with [`PREFIX`]: the page's layout width,
//! then one line per target (`x,y,w,h` of the visible part in the page's CSS
//! px, a tab, its kind letter, a tab, and for a link its absolute URL). Our own design.

/// Prefix of the console message with a page's targets.
pub const PREFIX: &str = "\u{1}trailboard-links\u{1}";

/// The most targets one message reports.
pub const MAX_LINKS: usize = 500;

/// Collects targets that can be seen: on screen, not hidden, not disabled, not
/// covered by something else at their middle, in document order. A link with
/// a URL a card can open is `o`; a field you type in (text inputs, text areas,
/// selects, editable content) is `t`; anything else clickable is `c`.
pub const SCRIPT: &str = r#"(() => {
  const W = innerWidth, H = innerHeight, out = [];
  const sel = 'a[href], area[href], button, input:not([type=hidden]), select, textarea, summary, ' +
    '[contenteditable=""], [contenteditable=true], [onclick], [tabindex]:not([tabindex="-1"]), ' +
    '[role=button], [role=link], [role=checkbox], [role=radio], [role=switch], [role=tab], ' +
    '[role=menuitem], [role=option], [role=combobox], [role=textbox], [role=treeitem]';
  const typed = /^(text|search|email|url|tel|password|number|date|datetime-local|month|week|time)$/i;
  const seen = new Set();
  for (const e of document.querySelectorAll(sel)) {
    if (out.length >= 500) break;
    if (e.disabled || e.closest('[aria-hidden=true], [inert]')) continue;
    // A control inside a link is the link's.
    let p = e.parentElement, inside = false;
    for (; p; p = p.parentElement) if (seen.has(p)) { inside = true; break; }
    if (inside) continue;
    let kind = 'c', href = '';
    const tag = e.tagName;
    if ((tag === 'A' || tag === 'AREA') && typeof e.href === 'string' && /^(https?|file):/i.test(e.href)) {
      kind = 'o'; href = e.href;
    } else if (tag === 'TEXTAREA' || tag === 'SELECT' || e.isContentEditable ||
               (tag === 'INPUT' && typed.test(e.type || 'text')) ||
               /^(textbox|combobox)$/.test(e.getAttribute('role') || '')) {
      kind = 't';
    }
    for (const r of e.getClientRects()) {
      const x0 = Math.max(r.left, 0), y0 = Math.max(r.top, 0);
      const x1 = Math.min(r.right, W), y1 = Math.min(r.bottom, H);
      if (x1 - x0 < 2 || y1 - y0 < 2) continue;
      const hit = document.elementFromPoint((x0 + x1) / 2, (y0 + y1) / 2);
      const label = tag === 'INPUT' && e.labels && [...e.labels].some(l => l === hit || l.contains(hit));
      if (!hit || !(e === hit || e.contains(hit) || label || (tag === 'AREA' && hit.tagName === 'IMG'))) continue;
      out.push([x0, y0, x1 - x0, y1 - y0].map(v => Math.round(v)).join(',') + '\t' + kind + '\t' + href);
      seen.add(e);
      break;
    }
  }
  console.log('\u0001trailboard-links\u0001' + W + '\n' + out.join('\n'));
})();"#;

/// What choosing a target does.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// A link: open its URL beside the card.
    Open(String),
    /// Click it (buttons, checkboxes, tabs, links without a URL).
    Click,
    /// Click it and type in it (text fields, selects, editable content).
    Type,
}

/// One target: where its visible part is (CSS px of the page) and what it does.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub rect: [f32; 4],
    pub action: Action,
}

/// The page's layout width and its targets, from a message without [`PREFIX`].
/// Malformed lines are skipped; `None` when even the width is missing.
pub fn parse(body: &str) -> Option<(f32, Vec<Link>)> {
    let mut lines = body.split('\n');
    let width: f32 = lines.next()?.trim().parse().ok().filter(|w: &f32| *w > 0.0)?;
    let links = lines
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let nums = parts.next()?;
            let v: Vec<f32> = nums.split(',').map(|n| n.trim().parse().ok()).collect::<Option<_>>()?;
            let rect: [f32; 4] = v.try_into().ok()?;
            let action = match (parts.next()?.trim(), parts.next().unwrap_or("").trim()) {
                ("o", url) => {
                    let ok = ["http:", "https:", "file:"]
                        .iter()
                        .any(|s| url.get(..s.len()).is_some_and(|p| p.eq_ignore_ascii_case(s)));
                    if !ok {
                        return None;
                    }
                    Action::Open(url.to_string())
                }
                ("c", _) => Action::Click,
                ("t", _) => Action::Type,
                _ => return None,
            };
            (rect[2] > 0.0 && rect[3] > 0.0).then_some(Link { rect, action })
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
        let (w, links) = parse("1024\n10,20,30,12\to\thttps://a.example/x\n0,0,5,5\to\tfile:///tmp/p.html\n1,1,9,9\tc\t\n2,2,9,9\tt").unwrap();
        assert_eq!(w, 1024.0);
        assert_eq!(links.len(), 4);
        assert_eq!(links[0], Link { rect: [10.0, 20.0, 30.0, 12.0], action: Action::Open("https://a.example/x".into()) });
        assert_eq!(links[1].action, Action::Open("file:///tmp/p.html".into()));
        assert_eq!(links[2].action, Action::Click);
        assert_eq!(links[3].action, Action::Type);
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
        let (_, links) = parse(
            "800\n1,2,3\to\thttps://x\n1,2,3,4\to\tjavascript:alert(1)\n1,2,0,4\tc\nno tab\n1,2,3,4\tz\n1,2,3,4\to\tHTTPS://ok",
        )
        .unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].action, Action::Open("HTTPS://ok".into()));
    }

    #[test]
    fn script_uses_the_prefix() {
        let escaped = PREFIX.replace('\u{1}', "\\u0001");
        assert!(SCRIPT.contains(&escaped));
        assert!(SCRIPT.contains(&MAX_LINKS.to_string()));
    }
}
