//! Whether a page is playing media, so the app can keep its card awake
//! outside the live cap. Our own design.
//!
//! [`script`] runs in every frame of a web page as it loads. It follows the
//! frame's media elements (in the document, in open shadow roots it hears
//! from, and ones never put in the document, through `play()`) and reports
//! with a console message starting with [`PREFIX`] whenever the frame's
//! answer changes, and `0` as the frame goes away. Media counts when it is
//! playing and is a video (muted or not) or audible audio.
//!
//! The console doesn't say which frame spoke, so each frame names itself
//! with a random token; [`Frames`] keeps one answer per token and the page
//! plays while any of them does. A main-frame load starts it afresh.

use std::collections::HashMap;

/// Prefix of a media report.
pub const PREFIX: &str = "\u{1}trailboard-media\u{1}";

/// Reads a report's body (after [`PREFIX`]): the frame's token and whether it plays.
pub fn parse(body: &str) -> Option<(&str, bool)> {
    let (token, state) = body.split_once(':')?;
    if token.is_empty() || token.len() > 32 || !token.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    match state {
        "1" => Some((token, true)),
        "0" => Some((token, false)),
        _ => None,
    }
}

/// The answers of a page's frames.
#[derive(Debug, Default)]
pub struct Frames(HashMap<String, bool>);

/// At most this many frames are followed; a page with more is odd, and the
/// rest are ignored.
const MAX_FRAMES: usize = 64;

impl Frames {
    /// Takes one frame's report. Returns the page's answer if it changed.
    pub fn report(&mut self, token: &str, playing: bool) -> Option<bool> {
        let before = self.playing();
        if playing {
            if self.0.len() < MAX_FRAMES || self.0.contains_key(token) {
                self.0.insert(token.to_string(), true);
            }
        } else {
            self.0.remove(token);
        }
        let after = self.playing();
        (before != after).then_some(after)
    }

    /// Forgets every frame (the main frame started a new page). Returns
    /// `Some(false)` if the page was playing.
    pub fn clear(&mut self) -> Option<bool> {
        let was = self.playing();
        self.0.clear();
        was.then_some(false)
    }

    pub fn playing(&self) -> bool {
        self.0.values().any(|&p| p)
    }
}

/// Installs the media reports in a frame (once per document).
pub fn script() -> String {
    format!(
        r#"(() => {{
  if (window.__tbMedia) return;
  Object.defineProperty(window, '__tbMedia', {{ value: true }});
  const P = '{PREFIX}';
  const token = Math.random().toString(36).slice(2, 12) || 'f';
  const seen = new Set();
  let last = '0';
  const plays = m => !m.paused && !m.ended && (m.tagName === 'VIDEO' || (!m.muted && m.volume > 0));
  const check = () => {{
    let on = false;
    for (const m of seen) {{
      if (!m.isConnected && m.paused) {{ seen.delete(m); continue; }}
      if (plays(m)) {{ on = true; break; }}
    }}
    const s = on ? '1' : '0';
    if (s !== last) {{ last = s; console.log(P + token + ':' + s); }}
  }};
  const kinds = ['play', 'playing', 'pause', 'ended', 'emptied', 'volumechange', 'abort', 'error'];
  const follow = m => {{
    if (!(m instanceof HTMLMediaElement) || seen.has(m)) return;
    seen.add(m);
    for (const k of kinds) m.addEventListener(k, check);
  }};
  // Media events don't bubble, but they pass the window on the way down.
  for (const k of kinds) addEventListener(k, e => {{ follow(e.target); check(); }}, true);
  // Elements never put in the document (`new Audio()`) only say so here.
  const play = HTMLMediaElement.prototype.play;
  HTMLMediaElement.prototype.play = function () {{
    follow(this);
    const r = play.apply(this, arguments);
    setTimeout(check, 0);
    return r;
  }};
  // A frame going away (or into the back/forward cache) stops counting.
  addEventListener('pagehide', () => {{ if (last !== '0') {{ last = '0'; console.log(P + token + ':0'); }} }}, true);
  addEventListener('pageshow', e => {{ if (e.persisted) check(); }}, true);
}})();"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_parse() {
        assert_eq!(parse("ab12:1"), Some(("ab12", true)));
        assert_eq!(parse("ab12:0"), Some(("ab12", false)));
        assert_eq!(parse("ab12:2"), None);
        assert_eq!(parse(":1"), None);
        assert_eq!(parse("a b:1"), None);
        assert_eq!(parse("ab12"), None);
    }

    #[test]
    fn a_page_plays_while_any_frame_does() {
        let mut f = Frames::default();
        assert_eq!(f.report("main", false), None);
        assert_eq!(f.report("main", true), Some(true));
        assert_eq!(f.report("embed", true), None);
        assert_eq!(f.report("main", false), None, "the embed still plays");
        assert_eq!(f.report("embed", false), Some(false));
        assert!(!f.playing());
    }

    #[test]
    fn a_new_page_forgets_old_frames() {
        let mut f = Frames::default();
        f.report("old", true);
        assert_eq!(f.clear(), Some(false));
        assert_eq!(f.clear(), None);
        assert!(!f.playing());
    }

    #[test]
    fn frames_are_capped() {
        let mut f = Frames::default();
        for i in 0..MAX_FRAMES + 10 {
            f.report(&format!("f{i}"), true);
        }
        assert_eq!(f.0.len(), MAX_FRAMES);
        assert_eq!(f.report("f0", false), None);
    }

    #[test]
    fn script_is_whole() {
        let s = script();
        assert!(s.contains(PREFIX));
        assert_eq!(s.matches('{').count(), s.matches('}').count());
    }
}
