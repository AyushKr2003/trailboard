//! A page's media: whether it plays (so the app can keep its card awake
//! outside the live cap), the media controls of M13 step 5, and why a
//! video can't play. Our own design.
//!
//! [`script`] runs in every frame of a web page as it loads. It follows the
//! frame's media elements (in the document, in open shadow roots it hears
//! from, and ones never put in the document, through `play()`) and reports
//! with a console message starting with [`PREFIX`] whenever the frame's
//! [`State`] changes, and `0` as the frame goes away. Media plays when it
//! is playing and is a video (muted or not) or audible audio.
//!
//! The console doesn't say which frame spoke, so each frame names itself
//! with a random token; [`Frames`] keeps one state per token and the page
//! plays while any of them does. A main-frame load starts it afresh.
//!
//! Controls go to one frame: [`Frames::target`] names the one that played
//! last, else the one that showed media last, and [`control`] (run in every
//! frame) acts only in the frame with that token. It answers with a line
//! after [`DONE`] (read by `scene::media::Status`). A frame that sees a
//! video fail for its format, or for missing DRM, says so after
//! [`TROUBLE`], once per kind.

use std::collections::HashMap;

/// Prefix of a media report.
pub const PREFIX: &str = "\u{1}trailboard-media\u{1}";
/// Prefix of a control's answer.
pub const DONE: &str = "\u{1}trailboard-media-done\u{1}";
/// Prefix of a "can't play" report: `format` or `drm`.
pub const TROUBLE: &str = "\u{1}trailboard-media-trouble\u{1}";

/// What a frame has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// No media (or it went away).
    None,
    Playing,
    /// Media is there (loaded, or paused), not playing.
    Present,
}

/// Reads a report's body (after [`PREFIX`]): the frame's token and state.
pub fn parse(body: &str) -> Option<(&str, State)> {
    let (token, state) = body.split_once(':')?;
    if !token_ok(token) {
        return None;
    }
    match state {
        "1" => Some((token, State::Playing)),
        "2" => Some((token, State::Present)),
        "0" => Some((token, State::None)),
        _ => None,
    }
}

fn token_ok(token: &str) -> bool {
    !token.is_empty() && token.len() <= 32 && token.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The states of a page's frames, each with when it last changed.
#[derive(Debug, Default)]
pub struct Frames {
    frames: HashMap<String, (State, u64)>,
    clock: u64,
}

/// At most this many frames are followed; a page with more is odd, and the
/// rest are ignored.
const MAX_FRAMES: usize = 64;

impl Frames {
    /// Takes one frame's report. Returns whether the page plays, if that changed.
    pub fn report(&mut self, token: &str, state: State) -> Option<bool> {
        let before = self.playing();
        if state == State::None {
            self.frames.remove(token);
        } else if self.frames.len() < MAX_FRAMES || self.frames.contains_key(token) {
            self.clock += 1;
            self.frames.insert(token.to_string(), (state, self.clock));
        }
        let after = self.playing();
        (before != after).then_some(after)
    }

    /// Forgets every frame (the main frame started a new page). Returns
    /// `Some(false)` if the page was playing.
    pub fn clear(&mut self) -> Option<bool> {
        let was = self.playing();
        self.frames.clear();
        was.then_some(false)
    }

    pub fn playing(&self) -> bool {
        self.frames.values().any(|(s, _)| *s == State::Playing)
    }

    /// The frame controls go to: the one that started playing last, else
    /// the one whose media changed last.
    pub fn target(&self) -> Option<&str> {
        let latest = |want: State| {
            self.frames.iter().filter(|(_, (s, _))| *s == want).max_by_key(|(_, (_, at))| *at).map(|(t, _)| t.as_str())
        };
        latest(State::Playing).or_else(|| latest(State::Present))
    }
}

/// The script that runs `op` (with its number `value`) in the frame named
/// `target`; other frames ignore it. `rates` are the steps of `faster` /
/// `slower`, in percent.
pub fn control(target: &str, op: &str, value: f64, rates: &[u16]) -> Option<String> {
    if !token_ok(target) || op.is_empty() || !op.bytes().all(|b| b.is_ascii_lowercase()) || !value.is_finite() {
        return None;
    }
    let rates: Vec<String> = rates.iter().map(u16::to_string).collect();
    Some(format!("window.__tbMediaCtl && window.__tbMediaCtl('{target}', '{op}', {value}, [{}]);", rates.join(",")))
}

/// Installs the media reports and controls in a frame (once per document).
pub fn script() -> String {
    format!(
        r#"(() => {{
  if (window.__tbMedia) return;
  Object.defineProperty(window, '__tbMedia', {{ value: true }});
  const P = '{PREFIX}', D = '{DONE}', T = '{TROUBLE}';
  const token = Math.random().toString(36).slice(2, 12) || 'f';
  const seen = new Set();
  let last = '0';
  const plays = m => !m.paused && !m.ended && (m.tagName === 'VIDEO' || (!m.muted && m.volume > 0));
  const has = m => m.isConnected && !!(m.currentSrc || m.src || m.srcObject || m.readyState > 0);
  const check = () => {{
    let s = '0';
    for (const m of seen) {{
      if (!m.isConnected && m.paused) {{ seen.delete(m); continue; }}
      if (plays(m)) {{ s = '1'; break; }}
      if (has(m)) s = '2';
    }}
    if (s !== last) {{ last = s; console.log(P + token + ':' + s); }}
  }};
  // Why a video can't play, once per kind.
  const told = new Set();
  const trouble = k => {{ if (!told.has(k)) {{ told.add(k); console.log(T + k); }} }};
  let drmFailed = false;
  const wanted = m => m.isConnected && (m.autoplay || m.controls || m.__tbAsked ||
    (m.tagName === 'VIDEO' && m.getBoundingClientRect().width >= 120));
  // Played, it should get going; stuck while DRM was refused, that's why.
  const watch = m => {{
    if (!drmFailed || m.tagName !== 'VIDEO') return;
    setTimeout(() => {{ if (drmFailed && m.isConnected && m.readyState < 3 && !m.error) trouble('drm'); }}, 5000);
  }};
  const kinds = ['play', 'playing', 'pause', 'ended', 'emptied', 'volumechange', 'abort', 'error', 'loadstart', 'loadedmetadata'];
  const follow = m => {{
    if (!(m instanceof HTMLMediaElement) || seen.has(m)) return;
    seen.add(m);
    for (const k of kinds) m.addEventListener(k, check);
  }};
  // Media events don't bubble, but they pass the window on the way down.
  for (const k of kinds) addEventListener(k, e => {{ follow(e.target); check(); }}, true);
  addEventListener('play', e => {{ if (e.target instanceof HTMLMediaElement) watch(e.target); }}, true);
  addEventListener('error', e => {{
    const m = e.target;
    if (!(m instanceof HTMLMediaElement) || !m.error || !wanted(m)) return;
    if (m.error.code === 3 || m.error.code === 4) trouble(drmFailed || m.mediaKeys ? 'drm' : 'format');
  }}, true);
  // Encrypted media with no keys a few seconds on: no DRM here.
  addEventListener('encrypted', e => {{
    const m = e.target;
    if (m instanceof HTMLMediaElement) setTimeout(() => {{ if (!m.mediaKeys && m.isConnected) trouble('drm'); }}, 3000);
  }}, true);
  const access = navigator.requestMediaKeySystemAccess;
  if (access) navigator.requestMediaKeySystemAccess = function (system) {{
    const r = access.apply(this, arguments);
    if (!/clearkey/i.test(String(system))) r.catch(() => {{ drmFailed = true; }});
    return r;
  }};
  // Elements never put in the document (`new Audio()`) only say so here.
  const play = HTMLMediaElement.prototype.play;
  HTMLMediaElement.prototype.play = function () {{
    follow(this);
    this.__tbAsked = true;
    const r = play.apply(this, arguments);
    setTimeout(check, 0);
    watch(this);
    return r;
  }};
  // A frame going away (or into the back/forward cache) stops counting.
  addEventListener('pagehide', () => {{ if (last !== '0') {{ last = '0'; console.log(P + token + ':0'); }} }}, true);
  addEventListener('pageshow', e => {{ if (e.persisted) check(); }}, true);

  // Controls: the media this frame would act on.
  const shown = m => {{
    const r = m.getBoundingClientRect();
    const w = Math.max(0, Math.min(r.right, innerWidth) - Math.max(r.left, 0));
    const h = Math.max(0, Math.min(r.bottom, innerHeight) - Math.max(r.top, 0));
    return Math.min(w * h, 1e8);
  }};
  const pick = () => {{
    for (const m of document.querySelectorAll('video, audio')) follow(m);
    let best = null, score = -1;
    for (const m of seen) {{
      if (!has(m) && m.paused) continue;
      let s = (plays(m) ? 4e9 : 0) + (m.paused ? 0 : 2e9) + (m.currentTime > 0 ? 1e9 : 0);
      s += m.tagName === 'VIDEO' ? shown(m) : 1;
      if (s > score) {{ score = s; best = m; }}
    }}
    return best;
  }};
  const full = m => {{ const f = document.fullscreenElement; return !!f && (f === m || f.contains(m)); }};
  const end = m => isFinite(m.duration) ? m.duration
    : (m.seekable && m.seekable.length ? m.seekable.end(m.seekable.length - 1) : Infinity);
  const answer = (op, m) => console.log(D + [op, m.paused ? 'r' : 'p', (m.currentTime || 0).toFixed(2),
    isFinite(m.duration) ? m.duration.toFixed(2) : 'inf', m.playbackRate, m.loop ? 1 : 0, full(m) ? 1 : 0].join(' '));
  // The top page puts the frame that asked (through any frames between) in full screen.
  addEventListener('message', e => {{
    const d = e.data;
    if (!d || typeof d !== 'object' || typeof d.__tbFull !== 'string') return;
    if (window.top !== window) {{ parent.postMessage(d, '*'); return; }}
    const out = on => console.log(D + 'full r 0 inf 1 0 ' + (on ? 1 : 0));
    if (document.fullscreenElement) {{ document.exitFullscreen().then(() => out(false), () => out(false)); return; }}
    const f = [...document.querySelectorAll('iframe')].find(f => f.contentWindow === e.source);
    if (!f || !document.fullscreenEnabled) {{ console.log(D + 'nofull'); return; }}
    f.requestFullscreen().then(() => out(true), () => console.log(D + 'nofull'));
  }});
  Object.defineProperty(window, '__tbMediaCtl', {{ value: (target, op, v, rates) => {{
    if (target !== token) return;
    const m = pick();
    if (!m) {{ console.log(D + 'none'); return; }}
    const say = () => answer(op, m);
    if (op === 'toggle' || op === 'play' || op === 'pause') {{
      if (op === 'pause' || (op === 'toggle' && !m.paused)) {{ m.pause(); say(); return; }}
      const fail = e => console.log(D + (e && e.name === 'NotAllowedError' ? 'blocked' : 'none'));
      // A frame without a click of its own may play once the top page has
      // had one (it runs this at the same time) and lends it on.
      const again = e => e && e.name === 'NotAllowedError' && window.top !== window
        ? new Promise(r => setTimeout(r, 150)).then(() => m.play()).then(say, fail) : fail(e);
      const r = m.play();
      if (r && r.then) r.then(say, again);
      else say();
      return;
    }}
    if (op === 'full') {{
      if (document.fullscreenElement) {{ document.exitFullscreen().then(say, say); return; }}
      // A frame without a click of its own asks the top page, which has one.
      const up = () => window.top !== window ? parent.postMessage({{ __tbFull: token }}, '*') : console.log(D + 'nofull');
      if (m.tagName !== 'VIDEO' || !document.fullscreenEnabled || !m.requestFullscreen) {{ up(); return; }}
      // Shown whole: a site's own fit (cover) would crop it to the window.
      const fit = [m.style.getPropertyValue('object-fit'), m.style.getPropertyPriority('object-fit')];
      const unfit = () => m.style.setProperty('object-fit', fit[0], fit[1]);
      m.style.setProperty('object-fit', 'contain', 'important');
      const back = () => {{ if (!full(m)) {{ unfit(); document.removeEventListener('fullscreenchange', back); }} }};
      m.requestFullscreen().then(() => {{ document.addEventListener('fullscreenchange', back); say(); }}, e => {{ unfit(); up(e); }});
      return;
    }}
    if (op === 'seek') m.currentTime = Math.max(0, Math.min(m.currentTime + v, end(m)));
    else if (op === 'seekto') m.currentTime = Math.max(0, Math.min(v, end(m)));
    else if (op === 'seekpct' && isFinite(m.duration)) m.currentTime = m.duration * v / 100;
    else if (op === 'loop') m.loop = !m.loop;
    else if (op === 'rate') {{
      let r = v;
      if (v < 0) {{
        const now = Math.round(m.playbackRate * 100);
        r = (v === -1 ? (rates.find(x => x > now) ?? rates[rates.length - 1])
          : ([...rates].reverse().find(x => x < now) ?? rates[0])) / 100;
      }}
      m.playbackRate = r;
    }}
    say();
  }} }});
}})();"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_parse() {
        assert_eq!(parse("ab12:1"), Some(("ab12", State::Playing)));
        assert_eq!(parse("ab12:2"), Some(("ab12", State::Present)));
        assert_eq!(parse("ab12:0"), Some(("ab12", State::None)));
        assert_eq!(parse("ab12:3"), None);
        assert_eq!(parse(":1"), None);
        assert_eq!(parse("a b:1"), None);
        assert_eq!(parse("ab12"), None);
    }

    #[test]
    fn a_page_plays_while_any_frame_does() {
        let mut f = Frames::default();
        assert_eq!(f.report("main", State::None), None);
        assert_eq!(f.report("main", State::Playing), Some(true));
        assert_eq!(f.report("embed", State::Playing), None);
        assert_eq!(f.report("main", State::Present), None, "the embed still plays");
        assert_eq!(f.report("embed", State::None), Some(false));
        assert!(!f.playing());
    }

    #[test]
    fn a_new_page_forgets_old_frames() {
        let mut f = Frames::default();
        f.report("old", State::Playing);
        assert_eq!(f.clear(), Some(false));
        assert_eq!(f.clear(), None);
        assert!(!f.playing());
        assert_eq!(f.target(), None);
    }

    #[test]
    fn frames_are_capped() {
        let mut f = Frames::default();
        for i in 0..MAX_FRAMES + 10 {
            f.report(&format!("f{i}"), State::Playing);
        }
        assert_eq!(f.frames.len(), MAX_FRAMES);
        assert_eq!(f.report("f0", State::None), None);
    }

    #[test]
    fn controls_go_to_the_frame_playing_else_the_last_with_media() {
        let mut f = Frames::default();
        assert_eq!(f.target(), None);
        f.report("main", State::Present);
        f.report("embed", State::Present);
        assert_eq!(f.target(), Some("embed"), "the last to show media");
        f.report("main", State::Playing);
        f.report("ad", State::Present);
        assert_eq!(f.target(), Some("main"), "playing beats present");
        f.report("embed", State::Playing);
        assert_eq!(f.target(), Some("embed"), "the last to start");
        f.report("embed", State::Present);
        assert_eq!(f.target(), Some("main"));
        f.report("main", State::None);
        assert_eq!(f.target(), Some("embed"), "paused after ad changed");
    }

    #[test]
    fn control_scripts() {
        let s = control("abc", "seek", -10.0, &[50, 100]).unwrap();
        assert_eq!(s, "window.__tbMediaCtl && window.__tbMediaCtl('abc', 'seek', -10, [50,100]);");
        assert!(control("a'b", "seek", 1.0, &[]).is_none());
        assert!(control("abc", "Seek()", 1.0, &[]).is_none());
        assert!(control("abc", "seek", f64::NAN, &[]).is_none());
    }

    #[test]
    fn script_is_whole() {
        let s = script();
        assert!(s.contains(PREFIX) && s.contains(DONE) && s.contains(TROUBLE));
        assert_eq!(s.matches('{').count(), s.matches('}').count());
        assert_eq!(s.matches('(').count(), s.matches(')').count());
    }
}
