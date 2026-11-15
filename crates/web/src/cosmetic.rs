//! Hiding page elements with the filter lists' cosmetic rules (ad slots,
//! cookie banners), on the main thread. Our own design, modelled on how
//! content blockers use the `adblock` engine:
//!
//! - When a frame starts loading a web page, [`on_load_start`] asks the
//!   engine for the rules that apply to the frame's site (plus the generic
//!   selectors that aren't a plain class or id) and runs [`SCRIPT`] in it,
//!   which adopts them as a stylesheet the page can't see in its DOM.
//! - Generic rules keyed by a class or id (most of EasyList's) are too many
//!   to send whole. The script watches the frame for class and id names and
//!   reports new ones in a console message ([`PREFIX`]); [`answer`] looks
//!   them up and hands back only the selectors they could match.
//!
//! Procedural rules (`:has-text()`, `:upward()`, …) and scriptlets are not
//! applied; `:style()` and `:remove()` rules that are plain CSS are (a
//! `:remove()` element is hidden rather than removed).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use adblock::cosmetic_filter_cache::ProceduralOrActionFilter;
use cef::*;

use crate::{js_string, shield};

/// Prefix of the console message with a frame's new class and id names:
/// the frame's identifier, a newline, the classes (space-separated), a
/// newline, the ids (space-separated).
pub const PREFIX: &str = "\u{1}trailboard-hide\u{1}";

/// The most class and id names one frame reports in all, so a page that
/// generates names endlessly can't keep the lookups going.
const MAX_NAMES: usize = 20_000;

/// Runs in each frame of a web page as it starts loading. `__CSS__` is the
/// first stylesheet, `__FRAME__` the frame's identifier, `__GENERIC__`
/// whether to report class and id names (not on `$generichide` pages).
const SCRIPT: &str = r#"(() => {
  const key = Symbol.for('trailboard.hide');
  if (window[key]) return;
  const sheet = new CSSStyleSheet();
  let css = __CSS__;
  sheet.replaceSync(css);
  const adopt = () => {
    const d = document;
    if (!d.adoptedStyleSheets.includes(sheet)) d.adoptedStyleSheets = [...d.adoptedStyleSheets, sheet];
  };
  adopt();
  window[key] = (more) => { css += more; sheet.replaceSync(css); adopt(); };
  if (!__GENERIC__) return;
  const seenC = new Set(), seenI = new Set();
  let newC = [], newI = [], added = [], changed = [], timer = 0;
  const note = (e) => {
    if (e.nodeType !== 1 || seenC.size + seenI.size > __MAX__) return;
    const id = e.id;
    if (id && !seenI.has(id) && !/\s/.test(id)) { seenI.add(id); newI.push(id); }
    const cl = e.classList;
    if (cl) for (const c of cl) if (!seenC.has(c)) { seenC.add(c); newC.push(c); }
  };
  const flush = () => {
    timer = 0;
    for (const n of added) {
      note(n);
      if (n.querySelectorAll) for (const e of n.querySelectorAll('[class],[id]')) note(e);
    }
    for (const n of changed) note(n);
    added = []; changed = [];
    if (!newC.length && !newI.length) return;
    console.log(__PREFIX__ + __FRAME__ + '\n' + newC.join(' ') + '\n' + newI.join(' '));
    newC = []; newI = [];
  };
  const soon = () => { if (!timer) timer = setTimeout(flush, 30); };
  new MutationObserver((records) => {
    for (const r of records) {
      if (r.type === 'attributes') changed.push(r.target);
      else for (const n of r.addedNodes) if (n.nodeType === 1) added.push(n);
    }
    soon();
  }).observe(document, { childList: true, subtree: true, attributes: true, attributeFilter: ['class', 'id'] });
  if (document.documentElement) { added.push(document.documentElement); soon(); }
})();"#;

/// Each frame's exceptions (selectors its site unhides), for [`answer`].
#[derive(Default)]
pub(crate) struct Frames(RefCell<HashMap<String, HashSet<String>>>);

/// Whether `url` is a web page (the only frames that get rules).
fn is_web(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// The main frame's address in `browser`.
fn main_url(browser: &mut Browser) -> String {
    browser.main_frame().map(|f| CefString::from(&f.url()).to_string()).unwrap_or_default()
}

/// Called when `frame` of a web page starts loading: hides what the rules
/// say to on its site, and starts watching it for generic rules.
pub(crate) fn on_load_start(frames: &Frames, browser: &mut Browser, frame: &mut Frame) {
    let id = CefString::from(&frame.identifier()).to_string();
    if frame.is_main() != 0 {
        frames.0.borrow_mut().clear();
    }
    let url = CefString::from(&frame.url()).to_string();
    if !is_web(&url) || id.contains('\n') {
        return;
    }
    let page = if frame.is_main() != 0 { url.clone() } else { main_url(browser) };
    let Some(rules) = shield::rules_for(&page) else { return };
    let res = rules.url_cosmetic_resources(&url);
    let styled = res.procedural_actions.iter().filter_map(|json| serde_json::from_str::<ProceduralOrActionFilter>(json).ok()?.as_css());
    let css = stylesheet(res.hide_selectors.iter().map(String::as_str), styled);
    log::debug!(
        "hide: {} rules in frame {id} ({}){}",
        res.hide_selectors.len(),
        url.chars().take(100).collect::<String>(),
        if res.generichide { ", no generic rules" } else { "" }
    );
    let generic = !res.generichide;
    frames.0.borrow_mut().insert(id.clone(), res.exceptions);
    let script = SCRIPT
        .replace("__CSS__", &js_string(&css))
        .replace("__FRAME__", &js_string(&id))
        .replace("__PREFIX__", &js_string(PREFIX))
        .replace("__MAX__", &MAX_NAMES.to_string())
        .replace("__GENERIC__", if generic { "true" } else { "false" });
    frame.execute_java_script(Some(&CefString::from(script.as_str())), Some(&CefString::from(url.as_str())), 0);
}

/// Splits a message body (after [`PREFIX`]) into the frame, its classes and its ids.
fn parse(body: &str) -> Option<(&str, Vec<&str>, Vec<&str>)> {
    let mut lines = body.splitn(3, '\n');
    let frame = lines.next().filter(|f| !f.is_empty())?;
    let classes = lines.next()?.split(' ').filter(|c| !c.is_empty()).collect();
    let ids = lines.next().unwrap_or("").split(' ').filter(|i| !i.is_empty()).collect();
    Some((frame, classes, ids))
}

/// Answers a frame's report of new class and id names with the generic
/// rules that could match them.
pub(crate) fn answer(frames: &Frames, browser: &mut Browser, body: &str) {
    let Some((id, classes, ids)) = parse(body) else { return };
    let frames = frames.0.borrow();
    let Some(exceptions) = frames.get(id) else { return }; // a frame from before the page navigated
    let Some(frame) = browser.frame_by_identifier(Some(&CefString::from(id))) else { return };
    let Some(rules) = shield::rules_for(&main_url(browser)) else { return };
    let selectors = rules.hidden_class_id_selectors(&classes, &ids, exceptions);
    if selectors.is_empty() {
        return;
    }
    log::debug!("hide: {} generic rules for {} classes, {} ids in frame {id}", selectors.len(), classes.len(), ids.len());
    let css = stylesheet(selectors.iter().map(String::as_str), std::iter::empty());
    let script = format!("window[Symbol.for('trailboard.hide')]?.({})", js_string(&css));
    frame.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
}

/// A stylesheet hiding the elements `hide` selects and giving those
/// `styled` selects their style: one rule per selector, so a selector
/// Chromium can't parse drops only its own rule. Selectors and styles that
/// could spill into the next rule are left out.
pub(crate) fn stylesheet<'a>(hide: impl IntoIterator<Item = &'a str>, styled: impl IntoIterator<Item = (String, String)>) -> String {
    let mut css = String::new();
    for sel in hide.into_iter().filter(|s| contained(s)) {
        css.push_str(sel);
        css.push_str("{display:none!important}\n");
    }
    for (sel, style) in styled {
        if contained(&sel) && contained(&style) && style_ok(&style) {
            css.push_str(&format!("{sel}{{{style}}}\n"));
        }
    }
    css
}

/// Whether a style is a list of declarations with no way out of its rule
/// and nothing that loads a resource.
fn style_ok(style: &str) -> bool {
    let lower = style.to_ascii_lowercase();
    !style.contains(['{', '}', '@']) && !lower.contains("url(") && !lower.contains("image-set(") && !lower.contains("expression(")
}

/// Whether `s` stays inside one rule: no braces or comments, quotes closed,
/// brackets balanced, no trailing escape, not an at-rule.
fn contained(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.starts_with('@') || s.contains("/*") || s.contains("*/") {
        return false;
    }
    let mut depth: Vec<char> = Vec::new();
    let mut quote: Option<char> = None;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (c, quote) {
            ('\\', _) => {
                if chars.next().is_none_or(|n| n == '\n') {
                    return false;
                }
            }
            ('\n', _) => return false,
            (q, Some(open)) if q == open => quote = None,
            (_, Some(_)) => {}
            ('"' | '\'', None) => quote = Some(c),
            ('{' | '}', None) => return false,
            ('(', None) => depth.push(')'),
            ('[', None) => depth.push(']'),
            (')' | ']', None) => {
                let open = depth.pop();
                if open != Some(c) {
                    return false;
                }
            }
            _ => {}
        }
    }
    quote.is_none() && depth.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_that_spill_are_left_out() {
        assert!(contained(".ad-banner"));
        assert!(contained("div[id^=\"ad-\"] > a[href*='{x}']"));
        assert!(contained("#cookie:not(.ok)"));
        assert!(contained("a[title=\"\\\"\"]"));
        assert!(!contained(".a{color:red}"));
        assert!(!contained("div[id=\"x"), "open quote");
        assert!(!contained("div[id=x"), "open bracket");
        assert!(!contained(":not(.a"), "open paren");
        assert!(!contained("a)"));
        assert!(!contained("a /* x"));
        assert!(!contained("@import 'x'"));
        assert!(!contained(".a\\"), "trailing escape");
        assert!(!contained("a\nb"));
        assert!(!contained("  "));
    }

    #[test]
    fn stylesheet_has_one_rule_per_selector() {
        let css = stylesheet([".ad", "#banner", ".bad{"], [("body".to_string(), "overflow: auto !important".to_string())]);
        assert_eq!(css, ".ad{display:none!important}\n#banner{display:none!important}\nbody{overflow: auto !important}\n");
        let css = stylesheet([], [("a".to_string(), "background: url(x)".to_string()), ("b".to_string(), "x:y}c{z:w".to_string())]);
        assert_eq!(css, "", "styles that load or escape are dropped");
    }

    #[test]
    fn parses_reports() {
        assert_eq!(parse("1-2\nad top\nbanner"), Some(("1-2", vec!["ad", "top"], vec!["banner"])));
        assert_eq!(parse("1-2\n\n"), Some(("1-2", vec![], vec![])));
        assert_eq!(parse("1-2\nad"), Some(("1-2", vec!["ad"], vec![])));
        assert_eq!(parse("\nad\n"), None);
        assert_eq!(parse("1-2"), None);
    }

    #[test]
    fn rules_from_lists_hide_by_site_class_and_id() {
        let rules = shield::build_rules(vec![(
            "test".into(),
            "##.ad-slot\n##div[data-ad]\nnews.site##.cookie-wall\nnews.site##body:style(overflow: auto !important)\nother.site#@#.ad-slot\n##+js(nope)".into(),
        )]);
        let res = rules.url_cosmetic_resources("https://news.site/story");
        assert!(res.hide_selectors.contains(".cookie-wall"));
        assert!(res.hide_selectors.contains("div[data-ad]"), "generic, not class or id");
        let styled: Vec<_> = res.procedural_actions.iter().filter_map(|j| serde_json::from_str::<ProceduralOrActionFilter>(j).ok()?.as_css()).collect();
        assert_eq!(styled, vec![("body".to_string(), "overflow: auto !important".to_string())]);
        assert_eq!(rules.hidden_class_id_selectors(["ad-slot", "text"], ["main"], &res.exceptions), vec![".ad-slot"]);
        let other = rules.url_cosmetic_resources("https://other.site/");
        assert!(rules.hidden_class_id_selectors(["ad-slot"], [] as [&str; 0], &other.exceptions).is_empty(), "unhidden there");
    }

    #[test]
    fn script_placeholders_are_all_known() {
        for p in ["__CSS__", "__FRAME__", "__PREFIX__", "__MAX__", "__GENERIC__"] {
            assert!(SCRIPT.contains(p), "{p}");
        }
        assert_eq!(SCRIPT.matches("__").count(), 10);
    }
}
