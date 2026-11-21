//! Caret and Visual mode in a page (M12 step 14). Our own design.
//!
//! [`start`] puts a caret in the main frame's text and keeps the mode's
//! state on the page (`window.__tbCaret`); [`run`] moves it, [`mode`]
//! starts or ends a selection, [`copy`] hands the selection to the app and
//! ends, [`STOP`] ends. The moves use the browser's own idea of a
//! character, word, line, paragraph and the page's ends
//! (`Selection.modify`), so they follow the page's writing and layout.
//!
//! The state is two points in the text: where a selection started (none
//! in Caret mode) and the caret. Each move starts from the caret alone and
//! then draws the selection again (by characters, or grown to whole lines),
//! so selections that grow and shrink never drift. The caret is drawn by a
//! small bar of ours over the page (a closed shadow root, no pointer
//! events), since the browser draws no caret in text you can't type in.
//! After a move the caret is scrolled into view, its scroll areas first.
//!
//! Answers are one console message starting with [`PREFIX`] (see
//! [`Report`]). Any page can write one, so the app only takes a copy it
//! asked for.

/// Prefix of a caret message.
pub const PREFIX: &str = "\u{1}trailboard-caret\u{1}";

/// An answer from the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// The mode started or changed: `caret`, `chars` or `lines`.
    Mode(String),
    /// There was no text in view to put a caret in.
    NoText,
    /// The page has no caret any more (it went to another document).
    Gone,
    /// The selection's text, to the clipboard (`primary`: the primary
    /// selection). The mode has ended.
    Copy { primary: bool, text: String },
    /// Nothing was selected to copy; the mode goes on.
    Empty,
}

/// Reads a message's body (after [`PREFIX`]).
pub fn parse(body: &str) -> Option<Report> {
    let (word, rest) = body.split_once('\n').unwrap_or((body, ""));
    Some(match word {
        "mode" => Report::Mode(rest.to_string()),
        "none" => Report::NoText,
        "gone" => Report::Gone,
        "empty" => Report::Empty,
        "copy" => Report::Copy { primary: false, text: rest.to_string() },
        "primary" => Report::Copy { primary: true, text: rest.to_string() },
        _ => return None,
    })
}

/// The helpers, installed once per document.
const SETUP: &str = r#"
if (!window.__tbCaret) {
  const P = '\u0001trailboard-caret\u0001';
  const say = m => console.log(P + m);
  const S = { on: false, mode: 'caret', anchor: null, focus: null, goalX: null, host: null, bar: null, color: '#888' };
  const sel = () => document.getSelection();
  const pt = () => { const s = sel(); return s && s.focusNode ? { n: s.focusNode, o: s.focusOffset } : null; };
  const at = p => { try { sel().collapse(p.n, p.o); } catch (e) {} };
  const go = (dir, by) => { try { sel().modify('move', dir, by); } catch (e) {} };
  // < 0 when a is before b, 0 when they are the same point.
  const cmp = (a, b) => {
    if (a.n === b.n && a.o === b.o) return 0;
    try { const r = document.createRange(); r.setStart(a.n, a.o); return -r.comparePoint(b.n, b.o); } catch (e) { return 0; }
  };
  const box = q => ({ left: q.left, right: q.right, top: q.top, bottom: q.bottom, height: q.bottom - q.top });
  // Where a point is drawn: a collapsed range's box, else the character
  // after it (its left edge) or before it (its right edge), else its element.
  const rectAt = p => {
    if (!p || !p.n.isConnected) return null;
    const r = document.createRange();
    try { r.setStart(p.n, p.o); r.collapse(true); } catch (e) { return null; }
    let rs = r.getClientRects();
    if (rs.length && rs[0].height > 0) return box(rs[0]);
    if (p.n.nodeType === 3) {
      if (p.o < p.n.length) {
        r.setEnd(p.n, p.o + 1); rs = r.getClientRects();
        if (rs.length) { const q = box(rs[0]); q.right = q.left; return q; }
      }
      if (p.o > 0) {
        r.setStart(p.n, p.o - 1); r.setEnd(p.n, p.o); rs = r.getClientRects();
        if (rs.length) { const q = box(rs[rs.length - 1]); q.left = q.right; return q; }
      }
    }
    let e = p.n.nodeType === 1 ? (p.n.childNodes[p.o] || p.n) : p.n.parentElement;
    if (e && e.nodeType !== 1) e = e.parentElement;
    if (!e) return null;
    const q = box(e.getBoundingClientRect());
    q.bottom = q.top + Math.min(q.height, 20); q.height = q.bottom - q.top; q.right = q.left;
    return q;
  };
  // Text right before (or after) a point that isn't a space.
  const wordy = (p, before) => !!p && p.n.nodeType === 3 &&
    (before ? p.o > 0 && /\S/.test(p.n.data[p.o - 1]) : p.o < p.n.length && /\S/.test(p.n.data[p.o]));
  // A word step that moves: a character when the word step sticks. False
  // at the page's end.
  const wordStep = dir => {
    const q = pt(); go(dir, 'word');
    if (!cmp(q, pt())) go(dir, 'character');
    return cmp(q, pt()) !== 0;
  };
  const inView = q => !!q && q.bottom > 0 && q.top < innerHeight && q.right >= 0 && q.left <= innerWidth;
  // The first text at the top of the view, at its line's start.
  const top = () => {
    for (let y = 6; y < innerHeight; y += 12) {
      for (const fx of [0.04, 0.12, 0.25, 0.4, 0.6]) {
        const c = document.caretRangeFromPoint(innerWidth * fx, y);
        if (c && c.startContainer.nodeType === 3 && /\S/.test(c.startContainer.data)) {
          at({ n: c.startContainer, o: c.startOffset });
          go('backward', 'lineboundary');
          return pt();
        }
      }
    }
    return null;
  };
  const moves = {
    left: () => go('backward', 'character'),
    right: () => go('forward', 'character'),
    down: () => go('forward', 'line'),
    up: () => go('backward', 'line'),
    // A word's end / start: the browser's word steps also stop at line
    // breaks and block edges (and can stick at a block's end), so they
    // step on until text is beside them.
    end: () => {
      for (let i = 0; i < 8; i++) if (!wordStep('forward') || wordy(pt(), true)) break;
    },
    back: () => {
      for (let i = 0; i < 8; i++) if (!wordStep('backward') || wordy(pt(), false)) break;
    },
    // To the next word's start: on to a word's end and back to its start,
    // until that start is ahead of where the caret was.
    word: () => {
      const o = pt(); let p = o;
      for (let i = 0; i < 8; i++) {
        at(p);
        if (!wordStep('forward')) break;
        const e = pt();
        go('backward', 'word');
        if (cmp(o, pt()) < 0 && wordy(pt(), false)) return;
        p = e;
      }
      at(p);
    },
    home: () => go('backward', 'lineboundary'),
    eol: () => go('forward', 'lineboundary'),
    top: () => go('backward', 'documentboundary'),
    bottom: () => go('forward', 'documentboundary'),
    para: () => {
      const o = pt(); go('forward', 'paragraphboundary');
      if (!cmp(o, pt())) { go('forward', 'character'); go('forward', 'paragraphboundary'); }
    },
    'para-back': () => {
      const o = pt(); go('backward', 'paragraphboundary');
      if (!cmp(o, pt())) { go('backward', 'character'); go('backward', 'paragraphboundary'); }
    },
  };
  // The page's selection from the state.
  const draw = () => {
    const s = sel();
    if (!S.focus || !s) return;
    try {
      if (!S.anchor || S.mode === 'caret') s.collapse(S.focus.n, S.focus.o);
      else if (S.mode === 'chars') s.setBaseAndExtent(S.anchor.n, S.anchor.o, S.focus.n, S.focus.o);
      else {
        const fwd = cmp(S.anchor, S.focus) <= 0;
        at(S.anchor); go(fwd ? 'backward' : 'forward', 'lineboundary'); const a = pt();
        at(S.focus); go(fwd ? 'forward' : 'backward', 'lineboundary'); const f = pt();
        s.setBaseAndExtent(a.n, a.o, f.n, f.o);
      }
    } catch (e) {}
    S.drawn = pt();
  };
  const mark = () => {
    if (!S.host || !S.host.isConnected) {
      const host = document.createElement('trailboard-caret');
      host.style.cssText = 'all:initial;position:fixed;left:0;top:0;width:0;height:0;z-index:2147483647;pointer-events:none';
      const root = host.attachShadow({ mode: 'closed' });
      const bar = document.createElement('div');
      root.appendChild(bar);
      document.documentElement.appendChild(host);
      S.host = host; S.bar = bar;
    }
    const q = rectAt(S.focus);
    const b = S.bar.style;
    b.cssText = 'position:fixed;pointer-events:none;width:2px;border-radius:1px;box-shadow:0 0 0 1px rgba(0,0,0,.35);background:' + S.color;
    if (!q) { b.display = 'none'; return; }
    const h = Math.max(q.height, 12);
    b.left = (q.left - 1) + 'px'; b.top = q.top + 'px'; b.height = h + 'px';
  };
  // Brings the caret into view: its scroll areas, then the page.
  const reveal = () => {
    const m = 48;
    let e = S.focus.n.nodeType === 1 ? S.focus.n : S.focus.n.parentElement;
    for (; e && e !== document.body && e !== document.documentElement; e = e.parentElement) {
      const cs = getComputedStyle(e);
      const q = rectAt(S.focus);
      if (!q) return;
      if (e.scrollHeight > e.clientHeight + 1 && /(auto|scroll|overlay)/.test(cs.overflowY)) {
        const b = e.getBoundingClientRect();
        if (q.top < b.top + 8) e.scrollTop -= b.top + 8 - q.top;
        else if (q.bottom > b.bottom - 8) e.scrollTop += q.bottom - b.bottom + 8;
      }
      if (e.scrollWidth > e.clientWidth + 1 && /(auto|scroll|overlay)/.test(cs.overflowX)) {
        const b = e.getBoundingClientRect();
        if (q.left < b.left + 8) e.scrollLeft -= b.left + 8 - q.left;
        else if (q.right > b.right - 8) e.scrollLeft += q.right - b.right + 8;
      }
    }
    const q = rectAt(S.focus);
    if (!q) return;
    let dy = 0, dx = 0;
    if (q.top < m) dy = q.top - m; else if (q.bottom > innerHeight - m) dy = q.bottom - innerHeight + m;
    if (q.left < 16) dx = q.left - 16; else if (q.right > innerWidth - 16) dx = q.right - innerWidth + 16;
    if (dx || dy) window.scrollBy({ left: dx, top: dy, behavior: 'instant' });
  };
  const end = () => {
    S.on = false; S.anchor = null; S.focus = null;
    if (S.host) S.host.remove();
    S.host = null;
  };
  Object.defineProperty(window, '__tbCaret', { value: {
    start(want, color, found) {
      S.color = color;
      const s = sel();
      let p = null;
      S.anchor = null; S.goalX = null;
      if (s && s.rangeCount && s.focusNode) {
        const r = s.getRangeAt(0);
        const q = r.getBoundingClientRect();
        if (inView(q) || inView(rectAt(pt()))) {
          if (!s.isCollapsed && found && s.toString().toLowerCase() === found.toLowerCase()) {
            p = { n: r.startContainer, o: r.startOffset };
          } else if (!s.isCollapsed) {
            S.anchor = { n: s.anchorNode, o: s.anchorOffset }; p = pt();
            if (want === 'caret') want = 'chars';
          } else p = pt();
        }
      }
      if (!p) p = top();
      if (!p) { end(); return say('none'); }
      S.focus = p; S.on = true;
      if (want !== 'caret' && !S.anchor) S.anchor = { n: p.n, o: p.o };
      S.mode = want;
      draw(); reveal(); mark();
      say('mode\n' + S.mode);
    },
    run(op, n) {
      if (!S.on) return say('gone');
      // A click moved the caret: go on from there.
      const now = pt();
      if (now && S.drawn && S.mode === 'caret' && sel().isCollapsed && cmp(now, S.drawn)) S.focus = now;
      if (!S.focus.n.isConnected) { const p = top(); if (!p) { end(); return say('none'); } S.focus = p; }
      const f = moves[op];
      if (!f) return;
      const vertical = op === 'down' || op === 'up';
      if (!vertical) S.goalX = null;
      else if (S.goalX === null) { const q = rectAt(S.focus); if (q) S.goalX = q.left; }
      for (let i = 0; i < n; i++) {
        at(S.focus); f();
        const p = pt();
        if (p) S.focus = p;
      }
      // Up and down keep to the column they started in.
      if (vertical && S.goalX !== null) {
        reveal();
        const q = rectAt(S.focus);
        const c = q && document.caretRangeFromPoint(S.goalX, (q.top + q.bottom) / 2);
        if (c && c.startContainer.nodeType === 3) {
          const p = { n: c.startContainer, o: c.startOffset };
          const q2 = rectAt(p);
          if (q2 && Math.abs(q2.top - q.top) < Math.max(q.height, 1) / 2) S.focus = p;
        }
      }
      draw(); reveal(); mark();
    },
    mode(m) {
      if (!S.on) return say('gone');
      if (m === 'caret') S.anchor = null;
      else if (!S.anchor) S.anchor = { n: S.focus.n, o: S.focus.o };
      S.mode = m;
      draw(); mark();
      say('mode\n' + m);
    },
    swap() {
      if (!S.on) return say('gone');
      if (!S.anchor) return;
      const a = S.anchor; S.anchor = S.focus; S.focus = a; S.goalX = null;
      draw(); reveal(); mark();
    },
    copy(primary) {
      if (!S.on) return say('gone');
      const t = String(sel() || '');
      if (!t) return say('empty');
      try { sel().removeAllRanges(); } catch (e) {}
      end();
      say((primary ? 'primary' : 'copy') + '\n' + t);
    },
    stop() {
      if (!S.on) return;
      try { if (S.mode !== 'caret' || sel().isCollapsed) sel().removeAllRanges(); } catch (e) {}
      end();
    },
  } });
}
"#;

fn with_setup(call: &str) -> String {
    format!("(() => {{{SETUP}\n{call}\n}})();")
}

/// Starts in `want` (`caret`, or `lines` for `V`), drawing the caret in
/// `color` (a CSS colour). A selection in view is kept (a mouse's becomes
/// Visual), except a match of `found` (the last find), where the caret goes
/// to its start; else the caret goes to the start of the first line in
/// view. `wait_ms` lets a find that just ended turn its match into the
/// selection first.
pub fn start(want: &str, color: &str, found: &str, wait_ms: u32) -> String {
    let call = format!(
        "setTimeout(() => window.__tbCaret.start({}, {}, {}), {wait_ms});",
        crate::js_string(want),
        crate::js_string(color),
        crate::js_string(found)
    );
    with_setup(&call)
}

/// Moves `n` times by `op` (`scene::caret::Motion::op`).
pub fn run(op: &str, n: u32) -> String {
    let call = format!("window.__tbCaret.run({}, {n});", crate::js_string(op));
    with_setup(&call)
}

/// Into mode `m` (`caret`, `chars`, `lines`).
pub fn mode(m: &str) -> String {
    with_setup(&format!("window.__tbCaret.mode({});", crate::js_string(m)))
}

/// The selection's ends swap.
pub fn swap() -> String {
    with_setup("window.__tbCaret.swap();")
}

/// The selection's text to the app, and the mode ends.
pub fn copy(primary: bool) -> String {
    with_setup(&format!("window.__tbCaret.copy({primary});"))
}

/// Ends the mode: the caret goes, and the selection with it.
pub const STOP: &str = "window.__tbCaret && window.__tbCaret.stop();";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_parse() {
        assert_eq!(parse("mode\nchars"), Some(Report::Mode("chars".into())));
        assert_eq!(parse("none"), Some(Report::NoText));
        assert_eq!(parse("gone"), Some(Report::Gone));
        assert_eq!(parse("empty"), Some(Report::Empty));
        assert_eq!(parse("copy\nline 1\nline 2"), Some(Report::Copy { primary: false, text: "line 1\nline 2".into() }));
        assert_eq!(parse("primary\nx"), Some(Report::Copy { primary: true, text: "x".into() }));
        assert_eq!(parse("what"), None);
    }

    #[test]
    fn scripts_use_the_prefix_and_quote_their_words() {
        assert!(SETUP.contains(&PREFIX.replace('\u{1}', "\\u0001")));
        let s = start("caret", "#ffd60a", "it's \"x\"", 60);
        assert!(s.contains("start(\"caret\", \"#ffd60a\", \"it's \\\"x\\\"\")"), "{s}");
        assert!(s.contains("}, 60);") || s.contains(", 60);"));
        assert!(run("word", 3).contains("run(\"word\", 3)"));
        assert!(copy(true).contains("copy(true)"));
    }
}
