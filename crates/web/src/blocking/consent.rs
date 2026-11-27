//! Answering cookie-consent banners with "reject" on known consent
//! platforms. Our own rules, written from each platform's public page
//! markup (the ids and classes its banner shows on any site using it), not
//! taken from other blockers' lists.
//!
//! [`script`] runs in each frame of a web page beside the cosmetic one
//! (same switches: the setting, the per-site toggle). It watches the frame
//! for a platform's banner for [`WATCH_MS`]; when one is shown (counting
//! one our hiding rules hide), it clicks that platform's reject button,
//! once. If the page is left unscrollable afterwards with the banner gone
//! or hidden, it lets `html` and `body` scroll again. Each outcome is
//! reported in a console message ([`PREFIX`]) for the log.

use crate::js_string;

/// Prefix of the console message with an outcome: the platform's name, a
/// newline, what happened (`rejected`, `no-button`, `shown` for a banner
/// only watched, `unlocked`).
pub const PREFIX: &str = "\u{1}trailboard-consent\u{1}";

/// How long a frame is watched for a banner after it starts loading.
const WATCH_MS: u32 = 30_000;

/// One consent platform's banner.
#[derive(Debug)]
pub(crate) struct Platform {
    /// For the log.
    pub name: &'static str,
    /// The banner, present only while the platform asks.
    pub banner: &'static str,
    /// Its reject button(s), searched in the same root as the banner. None
    /// when the choice is made elsewhere (in a frame of its own) and the
    /// banner is only watched, to let the page scroll once it goes.
    pub reject: Option<&'static str>,
    /// The button's text must read as a refusal ([`REFUSAL`]): for
    /// platforms whose buttons carry no role in their markup.
    pub by_text: bool,
    /// Only in frames whose address matches this (JS regex source).
    pub frame: Option<&'static str>,
    /// The element whose open shadow root holds the banner.
    pub shadow: Option<&'static str>,
}

const fn p(name: &'static str, banner: &'static str, reject: &'static str) -> Platform {
    Platform { name, banner, reject: Some(reject), by_text: false, frame: None, shadow: None }
}

/// The platforms we answer, most common first.
pub(crate) const PLATFORMS: &[Platform] = &[
    p("OneTrust", "#onetrust-banner-sdk", "#onetrust-reject-all-handler"),
    p("Didomi", "#didomi-notice", "#didomi-notice-disagree-button"),
    p("Cookiebot", "#CybotCookiebotDialog", "#CybotCookiebotDialogBodyButtonDecline"),
    // The message lives in its own frame (privacy-mgmt.com or a site's own
    // host); its buttons carry their choice type: 13 is "reject all".
    Platform {
        frame: Some(r"[?&]message_id="),
        ..p("Sourcepoint", "button.sp_choice_type_13, button.sp_choice_type_REJECT_ALL", "button.sp_choice_type_13, button.sp_choice_type_REJECT_ALL")
    },
    // …and the page holding that frame, which it may leave locked.
    Platform { reject: None, ..p("Sourcepoint page", "[id^=\"sp_message_container_\"]", "") },
    Platform { by_text: true, ..p("Quantcast", "#qc-cmp2-ui", ".qc-cmp2-summary-buttons button") },
    Platform { shadow: Some("#usercentrics-root"), ..p("Usercentrics", "[data-testid=\"uc-deny-all-button\"]", "[data-testid=\"uc-deny-all-button\"]") },
    p("TrustArc", "#truste-consent-track", "#truste-consent-required"),
    p("Funding Choices", ".fc-consent-root", ".fc-cta-do-not-consent"),
    p("consentmanager", "#cmpbox", ".cmpboxbtnno"),
    p("CookieYes", ".cky-consent-container", ".cky-btn-reject"),
    p("Complianz", ".cmplz-cookiebanner", ".cmplz-deny"),
    p("iubenda", "#iubenda-cs-banner", ".iubenda-cs-reject-btn"),
    p("Osano", ".osano-cm-dialog", ".osano-cm-denyAll"),
    p("Klaro", ".klaro .cookie-notice", ".klaro .cn-decline"),
    p("Cookie Notice", "#cookie-notice", "#cn-refuse-cookie"),
];

/// What a refusal button says, for platforms answered by text: the whole
/// label (case-insensitive, trimmed), in the languages consent banners are
/// most often shown in.
const REFUSAL: &str = r"^(reject( all)?|decline( all)?|deny( all)?|disagree|i do not agree|do not consent|refuse( all)?|(use )?(only )?(strictly )?(necessary|essential)( cookies)?( only)?|alle ablehnen|ablehnen|nur notwendige( cookies)?|tout refuser|refuser|continuer sans accepter|rechazar( todo)?|rifiuta( tutto)?|alles weigeren|weigeren|odrzuć( wszystkie)?|rejeitar( tudo)?|avvisa alla|afvis alle|hylkää kaikki)$";

/// Runs in a frame as it starts loading. `__RULES__` is [`PLATFORMS`],
/// `__REFUSAL__` the text test, `__WATCH__` how long to watch.
const SCRIPT: &str = r#"(() => {
  const key = Symbol.for('trailboard.consent');
  if (window[key]) return;
  window[key] = true;
  const rules = __RULES__.filter((r) => !r.f || new RegExp(r.f).test(location.href));
  if (!rules.length) return;
  const refusal = new RegExp(__REFUSAL__, 'i');
  const report = (name, what) => console.log(__PREFIX__ + name + '\n' + what);
  // Whether `e` is shown, counting what our hiding rules hide.
  const shown = (e, ours) => {
    const sheet = ours && window[Symbol.for('trailboard.hide')]?.sheet;
    if (sheet) sheet.disabled = true;
    try { return e.checkVisibility({ visibilityProperty: true }); }
    finally { if (sheet) sheet.disabled = false; }
  };
  const root = (r) => {
    if (!r.s) return document;
    const host = document.querySelector(r.s);
    return host && host.shadowRoot;
  };
  const button = (at, r) => {
    for (const e of at.querySelectorAll(r.r)) {
      if (e.disabled) continue;
      if (r.t && !refusal.test(e.textContent.replace(/\s+/g, ' ').trim())) continue;
      return e;
    }
  };
  // Lets html and body scroll if the page locked them for a banner that is now gone or hidden.
  const unlock = (r, banner) => {
    if (banner.isConnected && shown(banner, false)) return;
    let did = false;
    for (const e of [document.documentElement, document.body]) {
      if (!e) continue;
      const s = getComputedStyle(e);
      if (s.overflowY === 'hidden' || s.overflow === 'hidden') { e.style.setProperty('overflow', 'auto', 'important'); did = true; }
    }
    if (did) report(r.n, 'unlocked');
  };
  let done = false, timer = 0, observer = null;
  const check = () => {
    timer = 0;
    if (done) return;
    for (const r of rules) {
      const at = root(r);
      const banner = at && at.querySelector(r.b);
      if (!banner || !shown(banner, true)) continue;
      done = true;
      if (observer) observer.disconnect();
      const b = r.r && button(at, r);
      if (b) { b.click(); report(r.n, 'rejected'); } else report(r.n, r.r ? 'no-button' : 'shown');
      for (const ms of [800, 2500]) setTimeout(() => unlock(r, banner), ms);
      return;
    }
  };
  const soon = () => { if (!timer && !done) timer = setTimeout(check, 250); };
  observer = new MutationObserver(soon);
  observer.observe(document, { childList: true, subtree: true });
  for (const ms of [500, 1500, 4000, 10000]) setTimeout(soon, ms);
  setTimeout(() => { done = true; observer.disconnect(); }, __WATCH__);
  soon();
})();"#;

/// The script to run in a frame of a web page as it starts loading.
pub(crate) fn script() -> String {
    let rules: Vec<_> = PLATFORMS
        .iter()
        .map(|p| {
            let mut r = serde_json::json!({ "n": p.name, "b": p.banner });
            if let Some(reject) = p.reject {
                r["r"] = reject.into();
            }
            if p.by_text {
                r["t"] = true.into();
            }
            if let Some(f) = p.frame {
                r["f"] = f.into();
            }
            if let Some(s) = p.shadow {
                r["s"] = s.into();
            }
            r
        })
        .collect();
    let rules = serde_json::Value::from(rules).to_string();
    SCRIPT
        .replace("__RULES__", &rules)
        .replace("__REFUSAL__", &js_string(REFUSAL))
        .replace("__PREFIX__", &js_string(PREFIX))
        .replace("__WATCH__", &WATCH_MS.to_string())
}

/// Splits a message body (after [`PREFIX`]) into the platform and what happened.
pub(crate) fn parse(body: &str) -> Option<(&str, &str)> {
    let (name, what) = body.split_once('\n')?;
    PLATFORMS.iter().any(|p| p.name == name).then_some((name, what))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platforms_are_well_formed() {
        let mut names = std::collections::HashSet::new();
        for p in PLATFORMS {
            assert!(names.insert(p.name), "{} twice", p.name);
            for sel in [p.banner].into_iter().chain(p.reject).chain(p.shadow) {
                assert!(crate::cosmetic::contained(sel), "{}: {sel}", p.name);
                assert!(!sel.contains('\n'));
            }
            assert!(!p.name.contains('\n'));
        }
    }

    #[test]
    fn script_has_no_placeholders_left() {
        let s = script();
        assert!(!s.contains("__"), "{}", &s[s.find("__").unwrap()..][..40]);
        assert!(s.contains("\"n\":\"OneTrust\""));
        assert!(s.contains("\"t\":true"), "Quantcast answers by text");
        assert!(!s.contains("\"t\":false") && !s.contains(":null"), "defaults left out");
    }

    #[test]
    fn refusal_reads_whole_labels() {
        let re = regex::RegexBuilder::new(REFUSAL).case_insensitive(true).build().unwrap();
        for yes in ["Reject all", "DECLINE", "Necessary only", "Only necessary cookies", "Use necessary cookies only", "Alle ablehnen", "Tout refuser", "Disagree", "Strictly necessary only"] {
            assert!(re.is_match(yes), "{yes}");
        }
        for no in ["Accept all", "More options", "Agree", "Reject all and subscribe", "Manage", "I accept"] {
            assert!(!re.is_match(no), "{no}");
        }
    }

    #[test]
    fn parses_reports() {
        assert_eq!(parse("OneTrust\nrejected"), Some(("OneTrust", "rejected")));
        assert_eq!(parse("Nobody\nrejected"), None, "only our platforms");
        assert_eq!(parse("OneTrust"), None);
    }
}
