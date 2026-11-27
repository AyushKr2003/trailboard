//! YouTube ads. They come from the same servers as the videos, so network
//! rules can't stop them; this works inside the page instead. Our own
//! design, from how YouTube's public web client loads a video:
//!
//! - The ads a video plays are listed in its player response (the
//!   `ytInitialPlayerResponse` global on a first load, the
//!   `/youtubei/v1/player` reply when the page moves to another video).
//!   [`SCRIPT`] drops the ad keys ([`AD_KEYS`]) from it before the player
//!   reads it: a trap on the global, and wrappers around `JSON.parse` and
//!   `fetch`.
//! - When an ad plays anyway (the script ran late, or the ad is stitched
//!   into the video), it is skipped: the skip button clicked, or the ad
//!   muted, sped up and run to its end; the sound and speed come back after.
//! - Ad slots in the feeds, the masthead ad and the "ad blockers are not
//!   allowed" dialog are hidden, and a video that dialog paused plays on.
//!
//! It runs in YouTube frames (the site, and its embeds on other sites)
//! under the same switches as the rest of blocking. Each outcome is
//! reported in a console message ([`PREFIX`]) for the log.

use crate::js_string;

/// Prefix of the console message with an outcome: `pruned`, `skipped` or `dialog`.
pub const PREFIX: &str = "\u{1}trailboard-youtube\u{1}";

/// The keys of a player response that list its ads.
const AD_KEYS: [&str; 4] = ["adPlacements", "adSlots", "playerAds", "adBreakHeartbeatParams"];

/// Elements hidden on YouTube: ad slots in the feeds and beside the video,
/// the masthead ad, and the dialog telling the viewer to turn blocking off.
const HIDE: &[&str] = &[
    "#masthead-ad",
    "#player-ads",
    "ytd-ad-slot-renderer",
    "ytd-in-feed-ad-layout-renderer",
    "ytd-banner-promo-renderer",
    "ytd-promoted-sparkles-web-renderer",
    "ytd-promoted-video-renderer",
    "ytd-display-ad-renderer",
    "ytd-statement-banner-renderer",
    "ytd-rich-item-renderer:has(ytd-ad-slot-renderer)",
    "ytd-rich-section-renderer:has(ytd-statement-banner-renderer)",
    "ytm-promoted-sparkles-web-renderer",
    "ad-slot-renderer",
    "ytd-enforcement-message-view-model",
    "tp-yt-paper-dialog:has(ytd-enforcement-message-view-model)",
    ".ytp-ad-overlay-container",
];

/// Runs in a YouTube frame as it starts loading. `__KEYS__` is [`AD_KEYS`]
/// as a JS array, `__CSS__` the stylesheet hiding [`HIDE`].
const SCRIPT: &str = r#"(() => {
  const key = Symbol.for('trailboard.youtube');
  if (window[key]) return;
  window[key] = true;
  const KEYS = __KEYS__;
  const said = new Set();
  const report = (what) => { if (!said.has(what)) { said.add(what); console.log(__PREFIX__ + what); } };

  // Drops the ad keys from a player response, or from one nested where the
  // watch page keeps it. True if there were any.
  const prune = (o, depth = 0) => {
    if (!o || typeof o !== 'object' || depth > 3) return false;
    let hit = false;
    for (const k of KEYS) if (Object.prototype.hasOwnProperty.call(o, k)) { try { delete o[k]; hit = true; } catch (_) {} }
    if (Array.isArray(o)) { for (const x of o) hit = prune(x, depth + 1) || hit; }
    else if (o.playerResponse) hit = prune(o.playerResponse, depth + 1) || hit;
    if (hit) report('pruned');
    return hit;
  };
  const playerUrl = (u) => /\/youtubei\/v1\/(player|get_watch)\b/.test(u);

  // The first video's response, set by an inline script.
  for (const name of ['ytInitialPlayerResponse', 'playerResponse']) {
    let value = window[name];
    prune(value);
    try {
      Object.defineProperty(window, name, {
        configurable: true,
        get: () => value,
        set: (v) => { prune(v); value = v; },
      });
    } catch (_) {}
  }

  // Responses read as text and parsed.
  const parse = JSON.parse;
  JSON.parse = function (...args) {
    const out = parse.apply(this, args);
    if (out && typeof out === 'object' && (out.playerResponse || KEYS.some((k) => k in out))) prune(out);
    return out;
  };

  // Responses read with fetch's json().
  const fetch0 = window.fetch;
  if (fetch0) {
    window.fetch = function (input, init) {
      const url = typeof input === 'string' ? input : (input && input.url) || String(input);
      const p = fetch0.apply(this, arguments);
      if (!playerUrl(url)) return p;
      return p.then(async (resp) => {
        try {
          const o = await resp.clone().json();
          if (!prune(o)) return resp;
          const headers = new Headers(resp.headers);
          headers.delete('content-length');
          return new Response(JSON.stringify(o), { status: resp.status, statusText: resp.statusText, headers });
        } catch (_) { return resp; }
      });
    };
  }

  const sheet = new CSSStyleSheet();
  sheet.replaceSync(__CSS__);
  const adopt = () => {
    if (!document.adoptedStyleSheets.includes(sheet)) document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
  };
  adopt();

  // An ad that plays anyway: click skip, or mute it and run it to its end.
  let saved = null;
  const tick = () => {
    adopt();
    const player = document.querySelector('#movie_player, .html5-video-player');
    const video = player && player.querySelector('video');
    const ad = player && (player.classList.contains('ad-showing') || player.classList.contains('ad-interrupting'));
    if (ad && video) {
      if (!saved) saved = { muted: video.muted, rate: video.playbackRate };
      const skip = player.querySelector('.ytp-skip-ad-button, .ytp-ad-skip-button, .ytp-ad-skip-button-modern, .ytp-ad-skip-button-slot button');
      if (skip) skip.click();
      video.muted = true;
      try { video.playbackRate = 16; } catch (_) {}
      if (isFinite(video.duration) && video.duration > 0 && video.currentTime < video.duration - 0.1) video.currentTime = video.duration;
      report('skipped');
    } else if (saved && video) {
      video.muted = saved.muted;
      video.playbackRate = saved.rate;
      saved = null;
    }
    // The "ad blockers are not allowed" dialog: take it away and play on.
    const nag = document.querySelector('ytd-enforcement-message-view-model');
    if (nag) {
      const dialog = nag.closest('tp-yt-paper-dialog');
      (dialog || nag).remove();
      for (const b of document.querySelectorAll('tp-yt-iron-overlay-backdrop')) b.remove();
      if (video && video.paused) video.play().catch(() => {});
      report('dialog');
    }
  };
  setInterval(tick, 250);
})();"#;

/// Whether `url` is a YouTube page (the site, its mobile and music sites,
/// or an embed).
pub(crate) fn applies(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else { return false };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("").to_ascii_lowercase();
    let host = host.trim_end_matches('.');
    ["youtube.com", "youtube-nocookie.com"]
        .iter()
        .any(|d| host == *d || host.strip_suffix(d).is_some_and(|rest| rest.ends_with('.')))
}

/// The script, filled in.
pub(crate) fn script() -> String {
    let keys = format!("[{}]", AD_KEYS.iter().map(|k| js_string(k)).collect::<Vec<_>>().join(","));
    let css = crate::cosmetic::stylesheet(HIDE.iter().copied(), std::iter::empty());
    SCRIPT.replace("__KEYS__", &keys).replace("__PREFIX__", &js_string(PREFIX)).replace("__CSS__", &js_string(&css))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_to_youtube_and_its_embeds_only() {
        assert!(applies("https://www.youtube.com/watch?v=x"));
        assert!(applies("https://youtube.com/"));
        assert!(applies("https://m.youtube.com/watch?v=x"));
        assert!(applies("https://music.youtube.com/"));
        assert!(applies("https://www.youtube-nocookie.com/embed/x"));
        assert!(applies("HTTPS://WWW.YOUTUBE.COM:443/"));
        assert!(!applies("https://notyoutube.com/"));
        assert!(!applies("https://youtube.com.evil.example/"));
        assert!(!applies("https://evil.example/?u=https://youtube.com/"));
        assert!(!applies("https://youtube.com@evil.example/"));
        assert!(!applies("data:text/html,youtube.com"));
    }

    #[test]
    fn script_is_filled_in() {
        let s = script();
        assert!(!s.contains("__"), "a placeholder left");
        assert!(s.contains("\"adPlacements\""));
        assert!(s.contains("ytd-ad-slot-renderer{display:none!important}"));
    }

    #[test]
    fn every_hidden_selector_survives_the_stylesheet() {
        let css = crate::cosmetic::stylesheet(HIDE.iter().copied(), std::iter::empty());
        assert_eq!(css.lines().count(), HIDE.len());
    }
}
