//! What can be clicked in a page, for link hints: links, and buttons, fields
//! and other controls, or only some kinds of them (a [`Filter`], for the `;`
//! hint kinds). [`script`] runs in the page's main frame, gets the targets of
//! frames inside it (from other sites too) from [`frame_script`]'s listeners,
//! and answers with one console message starting with [`PREFIX`]: the page's
//! layout width, then one
//! line per target (`x,y,w,h` of the visible part in the page's CSS px, a tab,
//! its kind letter, a tab, and for a link or image its absolute URL). Our own design.

/// Prefix of the console message with a page's targets.
pub const PREFIX: &str = "\u{1}trailboard-links\u{1}";

/// The most targets one message reports.
pub const MAX_LINKS: usize = 500;

/// Which targets to label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// Links, controls and fields (`f`).
    All,
    /// Links a card can open.
    Links,
    /// Images with an address a card can open (not tiny ones).
    Images,
    /// Fields you type in.
    Fields,
    /// Links and images.
    Downloads,
    /// Everything [`Filter::All`] labels, and whatever shows a pointer
    /// cursor (things made clickable by a page's scripts).
    Clickable,
}

impl Filter {
    /// The selector group and the kind letters kept, as the script spells them.
    fn js(self) -> (&'static str, &'static str) {
        match self {
            Filter::All => ("all", "oct"),
            Filter::Links => ("links", "o"),
            Filter::Images => ("images", "i"),
            Filter::Fields => ("all", "t"),
            Filter::Downloads => ("downloads", "oi"),
            Filter::Clickable => ("clickable", "oct"),
        }
    }
}

/// How many levels of frames inside frames are asked for their targets.
pub const FRAME_DEPTH: u32 = 3;

/// How long a frame at depth `d` (counted down from [`FRAME_DEPTH`]) waits
/// for its own frames' answers, in ms: 150 at the top, less below, so each
/// frame answers before the one above stops waiting.
const WAIT_PER_LEVEL_MS: u32 = 50;

/// The script that collects `filter`'s targets in the main frame and, from
/// each visible frame, the targets that frame's listener ([`frame_script`])
/// sends back. One console message carries them all.
pub fn script(filter: Filter) -> String {
    let (group, keep) = filter.js();
    let helper = helper();
    format!(
        "(() => {{\n{helper}\ngather('{group}', '{keep}', {MAX_LINKS}, {FRAME_DEPTH}).then(list => console.log('\\u0001trailboard-links\\u0001' + innerWidth + '\\n' + list.map(line).join('\\n')));\n}})();"
    )
}

/// The listener added to every frame below the main one as it loads: asked
/// by its parent frame (with a one-time token), it collects its own targets
/// the same way, asks its own frames in turn, and answers its parent with
/// them measured from its own top-left corner. Idle until asked.
pub fn frame_script() -> &'static str {
    static SCRIPT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        let listen = LISTEN.replace("%LIMIT%", &MAX_LINKS.to_string()).replace("%DEPTH%", &(FRAME_DEPTH - 1).to_string());
        format!("(() => {{\n{}\n{listen}\n}})();", helper())
    });
    &SCRIPT
}

fn helper() -> String {
    HELPER.replacen("%WAIT%", &WAIT_PER_LEVEL_MS.to_string(), 1)
}

/// Shared by [`script`] and [`frame_script`]: `collect` finds this frame's
/// targets that can be seen (on screen, not hidden, not disabled, not
/// covered at their middle) in document order, with the visible frames
/// among them as places to fill; `gather` asks those frames, shifts their
/// answers by where each frame's content box sits, trims them to that box
/// and drops spots something covers. A link with a URL a card can open is
/// `o`; a field you type in (text inputs, text areas, selects, editable
/// content) is `t`; an image (when asked for) is `i`; anything else
/// clickable is `c`.
const HELPER: &str = r#"
  const ASK = '\u0001trailboard-hints-ask', ANSWER = '\u0001trailboard-hints-answer';
  const WAIT = %WAIT%;
  const links = 'a[href], area[href]';
  const controls = links + ', button, input:not([type=hidden]), select, textarea, summary, label, ' +
    '[contenteditable=""], [contenteditable=true], [onclick], [tabindex]:not([tabindex="-1"]), ' +
    '[role=button], [role=link], [role=checkbox], [role=radio], [role=switch], [role=tab], ' +
    '[role=menuitem], [role=option], [role=combobox], [role=textbox], [role=treeitem]';
  const url = /^(https?|file):/i;
  const typed = /^(text|search|email|url|tel|password|number|date|datetime-local|month|week|time)$/i;
  const num = v => parseFloat(v) || 0;
  // Where a frame element's page shows: its content box (inside the border
  // and padding), in this frame's viewport px.
  const contentBox = f => {
    const r = f.getBoundingClientRect(), cs = getComputedStyle(f);
    const pl = num(cs.paddingLeft), pt = num(cs.paddingTop), pr = num(cs.paddingRight), pb = num(cs.paddingBottom);
    const x = r.left + f.clientLeft + pl, y = r.top + f.clientTop + pt;
    return { x, y, w: Math.max(0, f.clientWidth - pl - pr), h: Math.max(0, f.clientHeight - pt - pb) };
  };
  // A frame's answer moved into this frame: shifted by the content box,
  // trimmed to it and to the viewport (W×H); tiny leftovers go.
  const place = (list, box, W, H) => {
    const out = [];
    if (!Array.isArray(list)) return out;
    const cx0 = Math.max(box.x, 0), cy0 = Math.max(box.y, 0);
    const cx1 = Math.min(box.x + box.w, W), cy1 = Math.min(box.y + box.h, H);
    for (const t of list) {
      if (!Array.isArray(t) || t.length < 5) continue;
      const [x, y, w, h] = t.map(Number);
      if (![x, y, w, h].every(Number.isFinite)) continue;
      const x0 = Math.max(box.x + x, cx0), y0 = Math.max(box.y + y, cy0);
      const x1 = Math.min(box.x + x + w, cx1), y1 = Math.min(box.y + y + h, cy1);
      if (x1 - x0 < 2 || y1 - y0 < 2) continue;
      out.push([x0, y0, x1 - x0, y1 - y0, String(t[4]).slice(0, 1), String(t[5] || '').replace(/[\t\r\n]/g, '')]);
    }
    return out;
  };
  const line = t => t.slice(0, 4).map(v => Math.round(v)).join(',') + '\t' + t[4] + '\t' + t[5];
  const collect = (GROUP, KEEP, LIMIT, DEPTH) => {
    const W = innerWidth, H = innerHeight, out = [];
    const sel = (GROUP === 'links' ? links : GROUP === 'images' ? 'img' : GROUP === 'downloads' ? links + ', img' :
      GROUP === 'clickable' ? '*' : controls.replace(', label,', ',')) + ', iframe, frame';
    // Clickable: the controls, and elements on screen whose own cursor is a
    // pointer where their parent's isn't (the outermost of a clickable block).
    const pointer = e => {
      if (e.matches(controls)) return true;
      const r = e.getBoundingClientRect();
      if (r.bottom < 0 || r.top > H || r.right < 0 || r.left > W || r.width < 4 || r.height < 4) return false;
      if (getComputedStyle(e).cursor !== 'pointer') return false;
      const p = e.parentElement;
      return !p || getComputedStyle(p).cursor !== 'pointer';
    };
    const seen = new Set();
    let count = 0;
    for (const e of document.querySelectorAll(sel)) {
      if (count >= LIMIT) break;
      const tag = e.tagName;
      if (tag === 'IFRAME' || tag === 'FRAME') {
        if (DEPTH < 1 || !e.contentWindow || e.closest('[aria-hidden=true], [inert]')) continue;
        const b = contentBox(e);
        if (b.w < 2 || b.h < 2 || b.x + b.w <= 0 || b.y + b.h <= 0 || b.x >= W || b.y >= H) continue;
        out.push({ frame: e, box: b });
        continue;
      }
      if (GROUP === 'clickable' && !pointer(e)) continue;
      if (e.disabled || e.closest('[aria-hidden=true], [inert]')) continue;
      // A control inside a link is the link's.
      let p = e.parentElement, inside = false;
      for (; p; p = p.parentElement) if (seen.has(p)) { inside = true; break; }
      if (inside) continue;
      let kind = 'c', href = '';
      if ((tag === 'A' || tag === 'AREA') && typeof e.href === 'string' && url.test(e.href)) {
        kind = 'o'; href = e.href;
      } else if (tag === 'IMG' && KEEP.includes('i')) {
        const src = e.currentSrc || e.src || '';
        if (!url.test(src) || e.width < 16 || e.height < 16) continue;
        kind = 'i'; href = src;
      } else if (tag === 'TEXTAREA' || tag === 'SELECT' || e.isContentEditable ||
                 (tag === 'INPUT' && typed.test(e.type || 'text')) ||
                 /^(textbox|combobox)$/.test(e.getAttribute('role') || '')) {
        kind = 't';
      }
      if (!KEEP.includes(kind)) continue;
      for (const r of e.getClientRects()) {
        const x0 = Math.max(r.left, 0), y0 = Math.max(r.top, 0);
        const x1 = Math.min(r.right, W), y1 = Math.min(r.bottom, H);
        if (x1 - x0 < 2 || y1 - y0 < 2) continue;
        const hit = document.elementFromPoint((x0 + x1) / 2, (y0 + y1) / 2);
        const label = tag === 'INPUT' && e.labels && [...e.labels].some(l => l === hit || l.contains(hit));
        if (!hit || !(e === hit || e.contains(hit) || label || (tag === 'AREA' && hit.tagName === 'IMG'))) continue;
        out.push([x0, y0, x1 - x0, y1 - y0, kind, href]);
        seen.add(e);
        count++;
        break;
      }
    }
    return out;
  };
  const token = () => {
    const a = new Uint32Array(4);
    crypto.getRandomValues(a);
    return [...a].map(n => n.toString(36)).join('');
  };
  // Ask one frame for its targets; [] if it doesn't answer in time.
  const ask = (f, GROUP, KEEP, LIMIT, DEPTH) => new Promise(done => {
    const w = f.contentWindow, t = token();
    let timer = 0;
    const on = ev => {
      const d = ev.data;
      if (ev.source !== w || !d || d.k !== ANSWER || d.t !== t) return;
      finish(d.list);
    };
    const finish = list => { removeEventListener('message', on); clearTimeout(timer); done(list); };
    addEventListener('message', on);
    timer = setTimeout(() => finish([]), WAIT * (DEPTH + 1));
    try { w.postMessage({ k: ASK, t, g: GROUP, keep: KEEP, limit: LIMIT, depth: DEPTH }, '*'); } catch (_) { finish([]); }
  });
  const gather = async (GROUP, KEEP, LIMIT, DEPTH) => {
    const items = collect(GROUP, KEEP, LIMIT, DEPTH);
    if (!items.some(i => i.frame)) return items.slice(0, LIMIT);
    const answers = await Promise.all(items.map(i => i.frame ? ask(i.frame, GROUP, KEEP, LIMIT, DEPTH - 1) : null));
    const W = innerWidth, H = innerHeight, out = [];
    items.forEach((i, n) => {
      if (!i.frame) { out.push(i); return; }
      // Measured again: the page may have moved while the frames answered.
      for (const t of place(answers[n], contentBox(i.frame), W, H)) {
        if (document.elementFromPoint(t[0] + t[2] / 2, t[1] + t[3] / 2) === i.frame) out.push(t);
      }
    });
    return out.slice(0, LIMIT);
  };
"#;

/// [`frame_script`]'s listener, after [`HELPER`]. Only the parent frame's
/// requests count; one listener per document.
const LISTEN: &str = r#"
  if (window[ASK]) return;
  Object.defineProperty(window, ASK, { value: true });
  const GROUPS = ['all', 'links', 'images', 'downloads', 'clickable'];
  addEventListener('message', ev => {
    const d = ev.data;
    if (parent === window || ev.source !== parent || !d || d.k !== ASK || typeof d.t !== 'string') return;
    if (!GROUPS.includes(d.g) || typeof d.keep !== 'string' || !/^[octi]{1,4}$/.test(d.keep)) return;
    const limit = Math.max(0, Math.min(%LIMIT%, d.limit | 0)), depth = Math.max(0, Math.min(%DEPTH%, d.depth | 0));
    gather(d.g, d.keep, limit, depth).then(list => parent.postMessage({ k: ANSWER, t: d.t, list }, '*'), () => {});
  });
"#;

/// What choosing a target does.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// A link: open its URL beside the card.
    Open(String),
    /// An image, by its address.
    Image(String),
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
            let openable = |url: &str| {
                ["http:", "https:", "file:"].iter().any(|s| url.get(..s.len()).is_some_and(|p| p.eq_ignore_ascii_case(s)))
            };
            let action = match (parts.next()?.trim(), parts.next().unwrap_or("").trim()) {
                ("o", url) if openable(url) => Action::Open(url.to_string()),
                ("i", url) if openable(url) => Action::Image(url.to_string()),
                ("o" | "i", _) => return None,
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
        let (w, links) =
            parse("1024\n10,20,30,12\to\thttps://a.example/x\n0,0,5,5\to\tfile:///tmp/p.html\n1,1,9,9\tc\t\n2,2,9,9\tt\n3,3,40,40\ti\thttps://a.example/p.png\n3,3,40,40\ti\tdata:image/png,x")
                .unwrap();
        assert_eq!(w, 1024.0);
        assert_eq!(links.len(), 5);
        assert_eq!(links[4].action, Action::Image("https://a.example/p.png".into()));
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
        let s = script(Filter::All);
        assert!(s.contains("'\\u0001trailboard-links\\u0001'"), "{s}");
        assert!(s.contains(&format!(", {MAX_LINKS}, {FRAME_DEPTH})")));
    }

    #[test]
    fn script_fills_in_the_filter() {
        for f in [Filter::All, Filter::Links, Filter::Images, Filter::Fields, Filter::Downloads, Filter::Clickable] {
            let s = script(f);
            assert!(!s.contains('%'), "{f:?}");
            let (group, keep) = f.js();
            assert!(s.contains(&format!("gather('{group}', '{keep}', ")), "{f:?}");
        }
    }

    #[test]
    fn frame_script_is_filled_in_and_listens_to_its_parent_only() {
        let s = frame_script();
        assert!(!s.contains('%'));
        assert!(s.contains("ev.source !== parent"));
        assert!(s.contains(&format!("Math.min({MAX_LINKS}, d.limit")));
        assert!(s.contains(&format!("Math.min({}, d.depth", FRAME_DEPTH - 1)));
        // The kinds the listener accepts are the ones the filters send.
        for f in [Filter::All, Filter::Links, Filter::Images, Filter::Fields, Filter::Downloads, Filter::Clickable] {
            let (group, keep) = f.js();
            assert!(s.contains(&format!("'{group}'")), "{f:?}");
            assert!(keep.chars().all(|c| "octi".contains(c)) && keep.len() <= 4, "{f:?}");
        }
    }
}
