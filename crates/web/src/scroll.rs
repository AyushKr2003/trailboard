//! Scroll sections (M12 step 3): the parts of a page that scroll on their
//! own (a sidebar, a list, the main pane, a carousel, a wide table, a frame),
//! found and scrolled from a page script. Our own design.
//!
//! Each command ([`list`], [`by`], [`to`], [`pick`]) runs in the page's main
//! frame and answers with one console message starting with [`PREFIX`]. The
//! script keeps a numbered registry of the sections it has seen in the
//! document (`0` is always the page itself), so the app can name a section
//! by number in its next command; a number whose element has gone is
//! answered as if no target were set (the automatic pick) and says so.
//!
//! What is done in the page, because it needs the page's live state:
//! - the automatic pick: the innermost section at the middle of the view (a
//!   modal, the main pane), else the largest one in view, else the page;
//! - chaining: at a section's edge a scroll goes on to the enclosing section
//!   that scrolls that way, as a wheel does, unless the section says
//!   `overscroll-behavior: contain` (or `none`);
//! - a target that is out of view is brought back into view first, and one
//!   that something else covers (a modal over it) gives way to the
//!   automatic pick while it is covered;
//! - smooth scrolls that follow each other quickly add up (each starts from
//!   where the last one is heading, not from where the page is mid-glide).
//!
//! Frames from another site can't be scrolled by script: the answer asks the
//! app for a real wheel event at a point inside the frame. So does a page
//! where nothing scrolls up and down by script (custom scrollers).

/// Prefix of a section message.
pub const PREFIX: &str = "\u{1}trailboard-scroll\u{1}";

/// The most sections one list reports.
pub const MAX_SECTIONS: usize = 200;

/// The page's own section number.
pub const PAGE: u32 = 0;

/// One section in view: its number, the section around it (`None` for the
/// page itself), the part of it that is in view (page CSS px), whether it
/// scrolls sideways / up and down, whether it is a frame from another site
/// (which scrolls, but only by wheel), and how far down it is (0–100).
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub id: u32,
    pub parent: Option<u32>,
    pub rect: [f32; 4],
    pub x: bool,
    pub y: bool,
    pub frame: bool,
    pub pct: u8,
}

/// What a scroll command ended with, besides scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// It scrolled.
    Moved,
    /// Nothing moved: the section and those around it are at that edge.
    Edge,
    /// The target was out of view and was brought back into view.
    Shown,
    /// Nothing here scrolls sideways.
    NoSideways,
}

/// One answer from the page.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// The sections in view ([`list`]), the one the automatic pick would
    /// choose now, and the page's layout width (CSS px) to scale rects by.
    List { width: f32, auto: u32, sections: Vec<Section> },
    /// A scroll ([`by`], [`to`]) moved section `scrolled` (in view at
    /// `rect`, now `pct` % along the axis it moved on). `reset`: the target
    /// asked for was gone, so the automatic pick was used.
    Did { width: f32, scrolled: u32, rect: [f32; 4], pct: u8, note: Note, reset: bool },
    /// Scroll by a real wheel event at `at` (page CSS px; `delta` positive
    /// is up / left, CSS px), for frames from another site and custom
    /// scrollers. `jump`: at once, not gliding.
    Wheel { width: f32, at: (f32, f32), delta: (i32, i32), jump: bool },
    /// The innermost section under a click ([`pick`]); [`PAGE`] when the
    /// click was on no section of its own.
    Picked { id: u32 },
    /// Where the target is scrolled ([`spot`]): the section that scrolls
    /// (a [`Report::Spot`] path, `""` for the page) and how far, CSS px.
    Spot { path: String, x: f32, y: f32 },
}

/// A section's path as [`Report::Spot`] gives it: child numbers from the
/// document's root, joined by dots. Anything else is refused, since it goes
/// back into a script.
pub fn is_path(path: &str) -> bool {
    path.is_empty() || (path.len() <= 400 && path.split('.').all(|n| !n.is_empty() && n.len() <= 5 && n.bytes().all(|b| b.is_ascii_digit())))
}

fn nums<const N: usize>(s: &str) -> Option<[f32; N]> {
    let v: Vec<f32> = s.split(',').map(|n| n.trim().parse().ok()).collect::<Option<_>>()?;
    let a: [f32; N] = v.try_into().ok()?;
    a.iter().all(|n| n.is_finite()).then_some(a)
}

fn pct(s: &str) -> Option<u8> {
    s.trim().parse::<u8>().ok().map(|p| p.min(100))
}

/// Reads a message's body (after [`PREFIX`]); `None` if it is malformed.
pub fn parse(body: &str) -> Option<Report> {
    let mut lines = body.split('\n');
    let head: Vec<&str> = lines.next()?.split('\t').collect();
    let width: f32 = head.get(1)?.trim().parse().ok().filter(|w: &f32| *w > 0.0 && w.is_finite())?;
    match head[0] {
        "L" => {
            let auto = head.get(2)?.trim().parse().ok()?;
            let sections = lines
                .filter_map(|line| {
                    let f: Vec<&str> = line.split('\t').collect();
                    if f.len() < 5 {
                        return None;
                    }
                    let id: u32 = f[0].trim().parse().ok()?;
                    let parent: i64 = f[1].trim().parse().ok()?;
                    let rect = nums::<4>(f[2])?;
                    if rect[2] <= 0.0 || rect[3] <= 0.0 {
                        return None;
                    }
                    let axes = f[3].trim();
                    let frame = axes == "f";
                    let parent = (parent >= 0 && id != PAGE).then_some(parent as u32).filter(|p| *p != id);
                    if id != PAGE && parent.is_none() {
                        return None;
                    }
                    Some(Section {
                        id,
                        parent,
                        rect,
                        x: frame || axes.contains('x'),
                        y: frame || axes.contains('y'),
                        frame,
                        pct: pct(f[4])?,
                    })
                })
                .take(MAX_SECTIONS)
                .collect();
            Some(Report::List { width, auto, sections })
        }
        "D" if head.len() >= 7 => {
            let note = match head[5].trim() {
                "m" => Note::Moved,
                "e" => Note::Edge,
                "o" => Note::Shown,
                "n" => Note::NoSideways,
                _ => return None,
            };
            Some(Report::Did {
                width,
                scrolled: head[2].trim().parse().ok()?,
                rect: nums::<4>(head[3])?,
                pct: pct(head[4])?,
                note,
                reset: head[6].trim() == "1",
            })
        }
        "W" if head.len() >= 5 => {
            let at = nums::<2>(head[2])?;
            let d = nums::<2>(head[3])?;
            Some(Report::Wheel { width, at: (at[0], at[1]), delta: (d[0] as i32, d[1] as i32), jump: head[4].trim() == "1" })
        }
        "P" => Some(Report::Picked { id: head.get(2)?.trim().parse().ok()? }),
        "S" if head.len() >= 5 => {
            let path = head[2].trim().to_string();
            let x: f32 = head[3].trim().parse().ok().filter(|v: &f32| v.is_finite())?;
            let y: f32 = head[4].trim().parse().ok().filter(|v: &f32| v.is_finite())?;
            is_path(&path).then_some(Report::Spot { path, x: x.max(0.0), y: y.max(0.0) })
        }
        _ => None,
    }
}

/// Which way a scroll goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// How far one step of [`by`] goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// About three lines of text (60 CSS px).
    Line,
    /// Half the section's height (or width).
    Half,
}

/// An edge for [`to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// The helpers every command starts with, installed once per document.
const SETUP: &str = r#"
if (!window.__tbScroll) {
  const P = '\u0001trailboard-scroll\u0001';
  // SLACK: an overflow this small (a border, a rounding) isn't scrolling.
  const LINE = 60, PAD = 1, SLACK = 8, GLIDE = 300;
  const ids = new WeakMap(), els = new Map(), pend = new WeakMap();
  let next = 1;
  const page = () => document.scrollingElement || document.documentElement;
  const idOf = e => {
    if (e === page()) return 0;
    let i = ids.get(e);
    if (!i) { i = next++; ids.set(e, i); els.set(i, new WeakRef(e)); }
    return i;
  };
  const el = id => {
    if (id === 0) return page();
    const r = els.get(id), e = r && r.deref();
    return e && e.isConnected ? e : null;
  };
  const isFrame = e => e.tagName === 'IFRAME' || e.tagName === 'FRAME';
  // A frame's window if a script may scroll it (same site), else null;
  // undefined for an ordinary element.
  const win = e => {
    if (e === page()) return window;
    if (!isFrame(e)) return undefined;
    try { const w = e.contentWindow; if (w && w.document && w.document.documentElement) return w; } catch (_) {}
    return null;
  };
  const hidden = (doc, a) => {
    const h = doc.documentElement, b = doc.body;
    const o = n => n ? getComputedStyle(n)['overflow' + a] : 'visible';
    const ho = o(h);
    return ho === 'hidden' || ho === 'clip' || (ho === 'visible' && (o(b) === 'hidden' || o(b) === 'clip'));
  };
  // [left, top, max left, max top, view width, view height], or null.
  const pos = e => {
    const w = win(e);
    if (w === null) return null;
    if (w) {
      const s = w.document.scrollingElement || w.document.documentElement;
      return [w.scrollX, w.scrollY, s.scrollWidth - s.clientWidth, s.scrollHeight - s.clientHeight, s.clientWidth, s.clientHeight];
    }
    return [e.scrollLeft, e.scrollTop, e.scrollWidth - e.clientWidth, e.scrollHeight - e.clientHeight, e.clientWidth, e.clientHeight];
  };
  // Which ways it scrolls: 'x', 'y', 'xy', '' or 'f' (a frame only a wheel reaches).
  const axes = e => {
    const w = win(e);
    if (w === null) return 'f';
    const p = pos(e);
    if (w) {
      return (p[2] > SLACK && !hidden(w.document, 'X') ? 'x' : '') + (p[3] > SLACK && !hidden(w.document, 'Y') ? 'y' : '');
    }
    const cs = getComputedStyle(e), ok = o => o === 'auto' || o === 'scroll' || o === 'overlay';
    return (p[2] > SLACK && ok(cs.overflowX) ? 'x' : '') + (p[3] > SLACK && ok(cs.overflowY) ? 'y' : '');
  };
  const up = e => e.assignedSlot || e.parentElement || (e.parentNode && e.parentNode.host) || null;
  // The section around e (the page for any top-level one; null for the page).
  const outer = e => {
    if (e === page()) return null;
    for (let p = up(e); p; p = up(p)) {
      if (p === document.documentElement || p === document.body) break;
      if (!isFrame(p) && axes(p)) return p;
    }
    return page();
  };
  // The part of e that is in view (inside the view and the sections
  // around it), or null.
  const seen = e => {
    if (e === page()) return [0, 0, innerWidth, innerHeight];
    const r = e.getBoundingClientRect();
    let x0 = Math.max(r.left, 0), y0 = Math.max(r.top, 0), x1 = Math.min(r.right, innerWidth), y1 = Math.min(r.bottom, innerHeight);
    for (let c = outer(e); c && c !== page() && x1 - x0 >= 2 && y1 - y0 >= 2; c = outer(c)) {
      const q = c.getBoundingClientRect();
      x0 = Math.max(x0, q.left); y0 = Math.max(y0, q.top); x1 = Math.min(x1, q.right); y1 = Math.min(y1, q.bottom);
    }
    if (x1 - x0 < 2 || y1 - y0 < 2) return null;
    return [x0, y0, x1 - x0, y1 - y0];
  };
  const deepAt = (x, y) => {
    let e = document.elementFromPoint(x, y);
    while (e && e.shadowRoot) { const d = e.shadowRoot.elementFromPoint(x, y); if (!d || d === e) break; e = d; }
    return e;
  };
  // The innermost section at a point (frames count), else the page.
  const at = (x, y) => {
    for (let e = deepAt(x, y); e; e = up(e)) {
      if (e === document.documentElement || e === document.body) break;
      if (isFrame(e) || axes(e)) return e;
    }
    return page();
  };
  const sections = () => {
    const out = [], walk = root => {
      for (const e of root.querySelectorAll('*')) {
        if (out.length >= 200) return;
        if (e.shadowRoot) walk(e.shadowRoot);
        const t = e.tagName;
        if (t === 'INPUT' || t === 'TEXTAREA' || t === 'SELECT' || e === document.documentElement || e === document.body) continue;
        if (!isFrame(e) && e.scrollHeight <= e.clientHeight + SLACK && e.scrollWidth <= e.clientWidth + SLACK) continue;
        const r = seen(e);
        if (!r || r[2] < 40 || r[3] < 24) continue;
        // Frames scroll on their own too.
        const a = axes(e);
        if (!a || getComputedStyle(e).visibility === 'hidden') continue;
        out.push([e, r, a]);
      }
    };
    walk(document);
    return out;
  };
  const auto = () => {
    const m = at(innerWidth / 2, innerHeight / 2);
    if (m !== page() || /y/.test(axes(m))) return m;
    let best = null, area = 0;
    for (const [e, r] of sections()) if (r[2] * r[3] > area) { best = e; area = r[2] * r[3]; }
    return best || page();
  };
  const pct = (e, ax) => {
    const p = pos(e);
    if (!p) return 0;
    const [c, m] = ax === 'x' ? [p[0], p[2]] : [p[1], p[3]];
    return m > PAD ? Math.round(Math.min(Math.max(c / m, 0), 1) * 100) : 0;
  };
  const r4 = r => r.map(v => Math.round(v)).join(',');
  const say = s => console.log(P + s);
  const inside = (h, e) => { for (let c = h; c; c = up(c)) if (c === e) return true; return false; };
  // A point inside e that e itself gets (not a sticky bar over it), for a wheel.
  const spot = e => {
    const r = seen(e) || [0, 0, innerWidth, innerHeight];
    for (const [fx, fy] of [[.5, .5], [.5, .3], [.5, .7], [.3, .5], [.7, .5], [.5, .15], [.5, .85]]) {
      const x = r[0] + r[2] * fx, y = r[1] + r[3] * fy, h = deepAt(x, y);
      if (h && inside(h, e)) return [x, y];
    }
    return [r[0] + r[2] / 2, r[1] + r[3] / 2];
  };
  // Something else (a modal, a popup) is over the middle of e's part in view.
  const covered = e => {
    if (e === page()) return false;
    const r = seen(e);
    if (!r) return false;
    const h = deepAt(r[0] + r[2] / 2, r[1] + r[3] / 2);
    return !!h && !inside(h, e);
  };
  const wheel = (e, dx, dy, jump) => {
    const [x, y] = spot(e);
    say('W\t' + innerWidth + '\t' + Math.round(x) + ',' + Math.round(y) + '\t' + Math.round(dx) + ',' + Math.round(dy) + '\t' + (jump ? 1 : 0));
  };
  const did = (e, ax, note, reset) =>
    say('D\t' + innerWidth + '\t' + idOf(e) + '\t' + r4(seen(e) || [0, 0, 1, 1]) + '\t' + pct(e, ax) + '\t' + note + '\t' + (reset ? 1 : 0));
  // The target (or the automatic pick), and whether a target asked for was
  // gone. A target under a modal or popup gives way to the automatic pick
  // (the modal) until it is uncovered.
  const target = id => {
    if (id < 0) return [auto(), false];
    const e = el(id);
    if (!e) return [auto(), true];
    return [covered(e) ? auto() : e, false];
  };
  // e or the nearest section around it that scrolls along ax.
  const along = (e, ax) => {
    for (let c = e; c; c = outer(c)) { const a = axes(c); if (a === 'f' || a.includes(ax)) return c; }
    return null;
  };
  const contained = (e, ax) => {
    if (e === page() || isFrame(e)) return false;
    const o = getComputedStyle(e)[ax === 'x' ? 'overscrollBehaviorX' : 'overscrollBehaviorY'];
    return o === 'contain' || o === 'none';
  };
  // A section's child numbers from the root (null inside a shadow root).
  const pathOf = e => {
    const p = [];
    for (let c = e; c && c !== document.documentElement; c = c.parentElement) {
      const q = c.parentElement;
      if (!q) return null;
      p.unshift(Array.prototype.indexOf.call(q.children, c));
    }
    return p.join('.');
  };
  const byPath = s => {
    let e = document.documentElement;
    for (const i of s.split('.')) e = e && e.children[+i];
    return e || null;
  };
  const go = (e, l, t, smooth) => {
    const w = win(e);
    (w || e).scrollTo({ left: l, top: t, behavior: smooth ? 'smooth' : 'instant' });
    if (smooth) pend.set(e, { l, t, until: performance.now() + GLIDE }); else pend.delete(e);
  };
  window.__tbScroll = {
    list() {
      const a = auto(), rows = [];
      const pa = axes(page());
      rows.push('0\t-1\t' + r4([0, 0, innerWidth, innerHeight]) + '\t' + (pa || '-') + '\t' + pct(page(), 'y'));
      for (const [e, r, ax] of sections()) {
        const o = outer(e);
        rows.push(idOf(e) + '\t' + idOf(o) + '\t' + r4(r) + '\t' + ax + '\t' + pct(e, ax === 'x' ? 'x' : 'y'));
      }
      say('L\t' + innerWidth + '\t' + idOf(a) + '\n' + rows.join('\n'));
    },
    by(id, ax, n, half, smooth) {
      const [t, reset] = target(id);
      const s = along(t, ax);
      if (!s) {
        if (ax === 'x') { did(t, ax, 'n', reset); return; }
        // Nothing scrolls by script: perhaps a custom scroller, which a wheel reaches.
        wheel(t, 0, -n * (half ? innerHeight / 2 : LINE), false);
        return;
      }
      if (s !== page()) {
        const r = s.getBoundingClientRect(), v = seen(s);
        // Less than a quarter of what could show: bring it back first.
        if ((v ? v[2] * v[3] : 0) < 0.25 * Math.min(r.width * r.height, innerWidth * innerHeight)) {
          s.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: smooth ? 'smooth' : 'instant' });
          did(s, ax, 'o', reset);
          return;
        }
      }
      for (let c = s; c; ) {
        const p = pos(c);
        if (!p) {
          const d = n * (half ? (ax === 'x' ? (seen(c) || [0, 0, innerWidth])[2] : (seen(c) || [0, 0, 0, innerHeight])[3]) / 2 : LINE);
          wheel(c, ax === 'x' ? -d : 0, ax === 'y' ? -d : 0, false);
          return;
        }
        const pd = pend.get(c), live = pd && performance.now() < pd.until;
        const l = live ? pd.l : p[0], t0 = live ? pd.t : p[1];
        const step = n * (half ? (ax === 'x' ? p[4] : p[5]) / 2 : LINE);
        const cur = ax === 'x' ? l : t0, max = ax === 'x' ? p[2] : p[3];
        if ((step > 0 && cur < max - PAD) || (step < 0 && cur > PAD)) {
          const v = Math.min(Math.max(cur + step, 0), max);
          go(c, ax === 'x' ? v : l, ax === 'y' ? v : t0, smooth);
          say('D\t' + innerWidth + '\t' + idOf(c) + '\t' + r4(seen(c) || [0, 0, 1, 1]) + '\t' +
              (max > PAD ? Math.round(v / max * 100) : 0) + '\tm\t' + (reset ? 1 : 0));
          return;
        }
        if (contained(c, ax)) break;
        let o = outer(c);
        while (o) { const a = axes(o); if (a === 'f' || a.includes(ax)) break; o = outer(o); }
        c = o;
      }
      did(s, ax, 'e', reset);
    },
    to(id, ax, end) {
      const [t, reset] = target(id);
      const s = along(t, ax);
      if (!s) {
        if (ax === 'x') { did(t, ax, 'n', reset); return; }
        wheel(t, 0, end ? -1e7 : 1e7, true);
        return;
      }
      const p = pos(s);
      if (!p) { wheel(s, ax === 'x' ? (end ? -1e7 : 1e7) : 0, ax === 'y' ? (end ? -1e7 : 1e7) : 0, true); return; }
      const v = end ? (ax === 'x' ? p[2] : p[3]) : 0;
      go(s, ax === 'x' ? v : p[0], ax === 'y' ? v : p[1], false);
      did(s, ax, 'm', reset);
    },
    pick(x, y) { say('P\t' + innerWidth + '\t' + idOf(at(x, y))); },
    // Where the target is scrolled, for a mark: its path and position.
    spot(id) {
      const [t] = target(id);
      let s = along(t, 'y') || along(t, 'x') || page(), path = pathOf(s);
      if (path === null || !pos(s)) { s = page(); path = ''; }
      const p = pos(s) || [0, 0];
      say('S\t' + innerWidth + '\t' + path + '\t' + Math.round(p[0]) + '\t' + Math.round(p[1]));
    },
    // Back to a spot: the section at the path if it still scrolls, else the page.
    restore(path, l, t) {
      let e = path ? byPath(path) : page();
      if (!e || (e !== page() && !axes(e))) e = page();
      go(e, l, t, false);
      did(e, 'y', 'm', false);
    },
  };
}
"#;

fn run(call: &str) -> String {
    format!("(() => {{{SETUP}\nwindow.__tbScroll.{call};\n}})();")
}

fn target(id: Option<u32>) -> i64 {
    id.map_or(-1, i64::from)
}

/// Report the sections in view as a [`Report::List`].
pub fn list() -> String {
    run("list()")
}

/// Scroll `target` (`None`: the automatic pick) `n` steps along `axis`
/// (positive: down / right), chaining at its edge. `smooth`: glide.
pub fn by(target_id: Option<u32>, axis: Axis, n: i32, unit: Unit, smooth: bool) -> String {
    let ax = if axis == Axis::X { "x" } else { "y" };
    run(&format!("by({}, '{ax}', {n}, {}, {smooth})", target(target_id), unit == Unit::Half))
}

/// Scroll `target` (`None`: the automatic pick) to an edge, at once; never chains.
pub fn to(target_id: Option<u32>, edge: Edge) -> String {
    let (ax, end) = match edge {
        Edge::Top => ("y", false),
        Edge::Bottom => ("y", true),
        Edge::Left => ("x", false),
        Edge::Right => ("x", true),
    };
    run(&format!("to({}, '{ax}', {end})", target(target_id)))
}

/// Report where `target` (`None`: the automatic pick) is scrolled, as a
/// [`Report::Spot`].
pub fn spot(target_id: Option<u32>) -> String {
    run(&format!("spot({})", target(target_id)))
}

/// Scroll the section at `path` (else the page) to (`x`, `y`) CSS px, at
/// once; answers as [`by`] does. A bad path means the page.
pub fn restore(path: &str, x: f32, y: f32) -> String {
    let path = if is_path(path) { path } else { "" };
    let (x, y) = (if x.is_finite() { x.max(0.0) } else { 0.0 }, if y.is_finite() { y.max(0.0) } else { 0.0 });
    run(&format!("restore('{path}', {x}, {y})"))
}

/// Report the innermost section at (`x`, `y`), given in a view `width` CSS
/// px wide (the page's own width may differ when it is zoomed).
pub fn pick(x: f32, y: f32, width: f32) -> String {
    let w = width.max(1.0);
    run(&format!("pick({x} * innerWidth / {w}, {y} * innerWidth / {w})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots() {
        assert_eq!(parse("S\t1280\t1.0.3\t0\t420"), Some(Report::Spot { path: "1.0.3".into(), x: 0.0, y: 420.0 }));
        assert_eq!(parse("S\t1280\t\t5\t-2"), Some(Report::Spot { path: String::new(), x: 5.0, y: 0.0 }));
        assert_eq!(parse("S\t1280\t1');alert(1);('\t0\t0"), None, "a path is only numbers");
        assert!(is_path("") && is_path("1.22.3") && !is_path("1..2") && !is_path("a") && !is_path("1."));
        assert!(restore("x'); evil('", 0.0, f32::NAN).contains("restore('', 0, 0)"));
        assert!(restore("1.2", 3.0, 40.5).contains("restore('1.2', 3, 40.5)"));
        assert!(spot(None).contains("spot(-1)") && spot(Some(4)).contains("spot(4)"));
    }

    #[test]
    fn parses_a_list() {
        let body = "L\t1280\t3\n0\t-1\t0,0,1280,720\ty\t40\n3\t0\t0,60,300,660\ty\t0\n4\t3\t10,100,280,200\txy\t100\n5\t0\t400,100,600,400\tf\t0";
        let Some(Report::List { width, auto, sections }) = parse(body) else { panic!() };
        assert_eq!((width, auto), (1280.0, 3));
        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0], Section { id: 0, parent: None, rect: [0.0, 0.0, 1280.0, 720.0], x: false, y: true, frame: false, pct: 40 });
        assert_eq!(sections[2].parent, Some(3));
        assert!(sections[2].x && sections[2].y);
        assert!(sections[3].frame && sections[3].x && sections[3].y);
    }

    #[test]
    fn a_page_that_does_not_scroll_is_listed() {
        let Some(Report::List { sections, .. }) = parse("L\t800\t0\n0\t-1\t0,0,800,600\t-\t0") else { panic!() };
        assert!(!sections[0].x && !sections[0].y);
    }

    #[test]
    fn bad_lines_are_skipped() {
        let body = "L\t800\t0\n0\t-1\t0,0,800,600\ty\t0\n7\t-1\t0,0,10,10\ty\t0\n8\t0\t0,0,0,10\ty\t0\n9\t0\t1,2,3\ty\t0\n10\t10\t0,0,5,5\ty\t0\nnope";
        let Some(Report::List { sections, .. }) = parse(body) else { panic!() };
        assert_eq!(sections.iter().map(|s| s.id).collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn parses_the_answers() {
        assert_eq!(
            parse("D\t1000\t4\t1,2,300,400\t55\tm\t0"),
            Some(Report::Did { width: 1000.0, scrolled: 4, rect: [1.0, 2.0, 300.0, 400.0], pct: 55, note: Note::Moved, reset: false })
        );
        assert!(matches!(parse("D\t1000\t0\t0,0,1,1\t100\te\t1"), Some(Report::Did { note: Note::Edge, reset: true, .. })));
        assert!(matches!(parse("D\t1000\t0\t0,0,1,1\t0\tn\t0"), Some(Report::Did { note: Note::NoSideways, .. })));
        assert_eq!(parse("W\t900\t450,300\t0,-60\t0"), Some(Report::Wheel { width: 900.0, at: (450.0, 300.0), delta: (0, -60), jump: false }));
        assert_eq!(parse("P\t900\t6"), Some(Report::Picked { id: 6 }));
        for bad in ["", "L", "L\t0\t0", "D\t900\t1\t0,0,1,1\t5\tz\t0", "W\t900\tx\t0,0\t0", "Q\t900\t1", "P\t900"] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn commands_are_whole_scripts() {
        let escaped = PREFIX.replace('\u{1}', "\\u0001");
        for s in [list(), by(Some(3), Axis::Y, -2, Unit::Half, true), to(None, Edge::Right), pick(10.0, 20.0, 800.0)] {
            assert!(s.contains(&escaped) && s.contains("window.__tbScroll"));
            assert_eq!(s.matches('{').count(), s.matches('}').count(), "{s}");
            assert_eq!(s.matches('(').count(), s.matches(')').count());
        }
        assert!(by(None, Axis::X, 3, Unit::Line, false).contains("by(-1, 'x', 3, false, false)"));
        assert!(to(Some(2), Edge::Top).contains("to(2, 'y', false)"));
        assert!(SETUP.contains(&MAX_SECTIONS.to_string()));
    }
}
