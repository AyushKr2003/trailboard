//! Browsing settings Chromium keeps as profile preferences: languages,
//! spelling, third-party cookies, proxy. [`apply`] sets them on the profile
//! and on every private cookie jar made afterwards ([`crate::Jar`]); changes
//! take effect for new requests at once, without restarting pages.
//!
//! The language list also goes to Chromium's network stack, which reads it
//! only when a context starts: [`crate::start`] for the profile, and each
//! private jar as it is made. A change reaches the profile's pages after a
//! restart.
//!
//! Two settings act on requests instead (checked on Chromium's IO thread,
//! so behind atomics and a lock): `DNT`/`Sec-GPC` headers
//! ([`set_do_not_track`]) and images ([`set_images`]).

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use cef::*;

/// How pages reach the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proxy {
    /// The desktop's proxy settings.
    System,
    Direct,
    /// `host:port`, or `scheme://host:port` (`http`, `https`, `socks4`, `socks5`).
    Fixed(String),
}

/// The preferences [`apply`] sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefs {
    /// `Accept-Language`, e.g. `hi-IN,hi,en-US,en`.
    pub accept_languages: String,
    pub spellcheck: bool,
    /// Dictionary for the spell checker, e.g. `en-US`.
    pub dictionary: String,
    pub block_third_party_cookies: bool,
    pub proxy: Proxy,
}

thread_local! {
    static CURRENT: RefCell<Option<Prefs>> = const { RefCell::new(None) };
    /// Chromium has made the profile's context; until then preferences
    /// can't be set and wait in `CURRENT`.
    static READY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Called once Chromium's profile context exists: sets the preferences
/// [`apply`] was given before it did.
pub(crate) fn context_ready() {
    READY.set(true);
    if let Some(p) = CURRENT.with_borrow(|c| c.clone()) {
        apply(&p);
    }
}

static DO_NOT_TRACK: AtomicBool = AtomicBool::new(false);

/// The language list for contexts made from now on (`hi-IN,hi,en-US,en`).
pub(crate) fn accept_language() -> String {
    CURRENT.with_borrow(|c| c.as_ref().map(|p| p.accept_languages.clone()).unwrap_or_default())
}

type ImagesOn = Box<dyn Fn(&str) -> bool + Send + Sync>;
static IMAGES: RwLock<Option<ImagesOn>> = RwLock::new(None);

/// Sends `DNT: 1` and `Sec-GPC: 1` with every request of a web page, or stops.
pub fn set_do_not_track(on: bool) {
    DO_NOT_TRACK.store(on, Ordering::Relaxed);
}

pub(crate) fn do_not_track() -> bool {
    DO_NOT_TRACK.load(Ordering::Relaxed)
}

/// Sets the test for whether a page at a URL loads images.
pub fn set_images(on: impl Fn(&str) -> bool + Send + Sync + 'static) {
    *IMAGES.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(on));
}

pub(crate) fn images_on(page: &str) -> bool {
    IMAGES.read().unwrap_or_else(|e| e.into_inner()).as_ref().is_none_or(|on| on(page))
}

/// Deletes every cookie in the profile (not in private jars, which go when
/// the app quits). Returns false if Chromium has no cookie store yet.
pub fn clear_cookies() -> bool {
    let Some(manager) = cookie_manager_get_global_manager(None) else { return false };
    manager.delete_cookies(None, None, None) != 0
}

/// Sets `prefs` on the profile (and remembers them for private jars).
pub fn apply(prefs: &Prefs) {
    CURRENT.with_borrow_mut(|c| *c = Some(prefs.clone()));
    if !READY.get() {
        return; // `context_ready` sets them
    }
    match request_context_get_global_context() {
        Some(ctx) => set_all(&ctx, prefs),
        None => log::warn!("prefs: no global request context"),
    }
}

/// Sets the current preferences on a newly made context.
pub(crate) fn apply_to(ctx: &RequestContext) {
    if let Some(p) = CURRENT.with_borrow(|c| c.clone()) {
        set_all(ctx, &p);
    }
}

fn set_all(ctx: &RequestContext, p: &Prefs) {
    set(ctx, "intl.accept_languages", string(&p.accept_languages));
    set(ctx, "browser.enable_spellchecking", boolean(p.spellcheck));
    set(ctx, "spellcheck.dictionaries", strings(&[p.dictionary.as_str()]));
    set(ctx, "spellcheck.dictionary", string(&p.dictionary));
    // 1: block third-party cookies everywhere; 0: allow them.
    set(ctx, "profile.cookie_controls_mode", int(if p.block_third_party_cookies { 1 } else { 0 }));
    set(ctx, "profile.block_third_party_cookies", boolean(p.block_third_party_cookies));
    set(ctx, "proxy", proxy(&p.proxy));
}

fn set(ctx: &RequestContext, name: &str, value: Option<Value>) {
    let Some(mut value) = value else { return };
    let name = CefString::from(name);
    if ctx.can_set_preference(Some(&name)) == 0 {
        log::debug!("prefs: {name} can't be set here");
        return;
    }
    // Not `default()`: an empty one is passed as a null pointer, which CEF refuses.
    let mut error = CefString::from("");
    if ctx.set_preference(Some(&name), Some(&mut value), Some(&mut error)) == 0 {
        log::warn!("prefs: setting {name}: {error}");
    }
}

fn boolean(b: bool) -> Option<Value> {
    let v = value_create()?;
    v.set_bool(b as ::std::os::raw::c_int);
    Some(v)
}

fn int(i: i32) -> Option<Value> {
    let v = value_create()?;
    v.set_int(i);
    Some(v)
}

fn string(s: &str) -> Option<Value> {
    let v = value_create()?;
    v.set_string(Some(&CefString::from(s)));
    Some(v)
}

fn strings(items: &[&str]) -> Option<Value> {
    let mut list = list_value_create()?;
    for (i, s) in items.iter().enumerate() {
        list.set_string(i, Some(&CefString::from(*s)));
    }
    let v = value_create()?;
    v.set_list(Some(&mut list));
    Some(v)
}

/// Chromium's `proxy` preference dictionary.
fn proxy(p: &Proxy) -> Option<Value> {
    let mut dict = dictionary_value_create()?;
    let (mode, server) = proxy_fields(p);
    dict.set_string(Some(&CefString::from("mode")), Some(&CefString::from(mode)));
    if let Some(server) = server {
        dict.set_string(Some(&CefString::from("server")), Some(&CefString::from(server.as_str())));
    }
    let v = value_create()?;
    v.set_dictionary(Some(&mut dict));
    Some(v)
}

/// The `mode` (and `server`) of the `proxy` preference.
fn proxy_fields(p: &Proxy) -> (&'static str, Option<String>) {
    match p {
        Proxy::System => ("system", None),
        Proxy::Direct => ("direct", None),
        Proxy::Fixed(s) if s.trim().is_empty() => ("system", None),
        Proxy::Fixed(s) => ("fixed_servers", Some(s.trim().to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_modes() {
        assert_eq!(proxy_fields(&Proxy::System), ("system", None));
        assert_eq!(proxy_fields(&Proxy::Direct), ("direct", None));
        assert_eq!(proxy_fields(&Proxy::Fixed(" socks5://h:1080 ".into())), ("fixed_servers", Some("socks5://h:1080".into())));
        assert_eq!(proxy_fields(&Proxy::Fixed(String::new())), ("system", None), "no server: as the desktop says");
    }
}
