//! Ad and tracker blocking: Brave's `adblock` engine with the filter lists the
//! app hands over ([`load_rules`]), asked about every request a page makes.
//!
//! Unlike the rest of this crate, requests are checked on Chromium's IO
//! thread, so the state here is behind locks and atomics, not `Rc`/`RefCell`:
//! the rules (swapped whole when a list changes, built on a thread of their
//! own), the on/off switch, and the app's test for sites where blocking is
//! off. Pages keep the addresses blocked since they last navigated
//! ([`Blocked`]); Chromium may ask twice for one (its preload scanner, then
//! the parser), so they are counted once.
//!
//! The same rules hide page elements (cosmetic rules), on the main thread:
//! see [`crate::cosmetic`].
//!
//! A rule may name a stand-in for what it blocks (`$redirect=noopjs`): the
//! request is then answered here with that harmless body ([`Stand`]),
//! instead of failing, so a page checking that its ad script loaded finds
//! one (see [`crate::resources`]).
//!
//! [`fetch`] downloads a URL with Chromium's network stack, for the lists.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use adblock::Engine as Rules;
use adblock::lists::{FilterSet, ParseOptions, RuleTypes};
use cef::*;

use crate::{WebEvent, emit};

/// Whether blocking is off on the page at a URL (a site the user allowed).
type Allowed = Box<dyn Fn(&str) -> bool + Send + Sync>;

static RULES: RwLock<Option<Arc<Rules>>> = RwLock::new(None);
static ON: AtomicBool = AtomicBool::new(true);
static ALLOWED: RwLock<Option<Allowed>> = RwLock::new(None);
/// Bumped by each [`load_rules`], so a slow, older build doesn't replace a newer one.
static BUILD: AtomicU64 = AtomicU64::new(0);

/// The addresses blocked on one page since its main frame last navigated.
#[derive(Default)]
pub(crate) struct Blocked(Mutex<HashSet<String>>);

impl Blocked {
    fn set(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn count(&self) -> u32 {
        self.set().len() as u32
    }
}

/// Turns blocking on or off everywhere (the setting).
pub fn set_enabled(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// Sets the test for pages where blocking is off, given the page's URL.
pub fn set_allowed(allowed: impl Fn(&str) -> bool + Send + Sync + 'static) {
    *ALLOWED.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(allowed));
}

/// Builds the rules from `lists` (name, text) on a thread of its own and
/// uses them once built; until then the previous rules (if any) stay. The
/// first rules are built right here instead, so the pages that open at
/// start (and their scriptlets) don't load before blocking is in place. No
/// lists: nothing is blocked.
pub fn load_rules(lists: Vec<(String, String)>) {
    let build = BUILD.fetch_add(1, Ordering::SeqCst) + 1;
    if lists.is_empty() {
        *RULES.write().unwrap_or_else(|e| e.into_inner()) = None;
        return;
    }
    if RULES.read().unwrap_or_else(|e| e.into_inner()).is_none() {
        let start = Instant::now();
        let rules = build_rules(lists);
        *RULES.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(rules));
        log::info!("blocking: first rules ready in {} ms", start.elapsed().as_millis());
        return;
    }
    let spawned = std::thread::Builder::new().name("adblock rules".into()).spawn(move || {
        let start = Instant::now();
        let names: Vec<String> = lists.iter().map(|(n, t)| format!("{n} ({} lines)", t.lines().count())).collect();
        let rules = build_rules(lists);
        if BUILD.load(Ordering::SeqCst) != build {
            return; // a newer build was asked for meanwhile
        }
        *RULES.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(rules));
        log::info!("blocking: rules from {} ready in {} ms", names.join(", "), start.elapsed().as_millis());
    });
    if let Err(e) = spawned {
        log::warn!("blocking: can't start building rules: {e}");
    }
}

pub(crate) fn build_rules(lists: Vec<(String, String)>) -> Rules {
    let mut set = FilterSet::new(false);
    // Network rules for requests, cosmetic rules for hiding page elements.
    let opts = ParseOptions { rule_types: RuleTypes::All, ..Default::default() };
    for (_, text) in lists {
        set.add_filter_list(text, opts);
    }
    let mut rules = Rules::new_with_filter_set(set);
    rules.use_resources(crate::resources::all());
    rules
}

/// The `adblock` name for a Chromium resource type.
fn request_type(t: ResourceType) -> &'static str {
    match t {
        ResourceType::MAIN_FRAME => "document",
        ResourceType::SUB_FRAME => "subdocument",
        ResourceType::STYLESHEET => "stylesheet",
        ResourceType::SCRIPT | ResourceType::WORKER | ResourceType::SHARED_WORKER | ResourceType::SERVICE_WORKER => "script",
        ResourceType::IMAGE | ResourceType::FAVICON => "image",
        ResourceType::FONT_RESOURCE => "font",
        ResourceType::OBJECT | ResourceType::PLUGIN_RESOURCE => "object",
        ResourceType::MEDIA => "media",
        ResourceType::XHR => "xmlhttprequest",
        ResourceType::PING => "ping",
        ResourceType::CSP_REPORT => "csp_report",
        _ => "other",
    }
}

/// What happens to a request.
#[derive(Debug, PartialEq)]
enum Verdict {
    Allow,
    Block,
    /// Blocked, and answered with this stand-in (type, body).
    Stand(String, Vec<u8>),
}

/// What `rules` do to a request for `url` of type `kind` made by the page at `page`.
fn verdict(rules: &Rules, url: &str, page: &str, kind: &str) -> Verdict {
    let web = |u: &str| ["http://", "https://", "ws://", "wss://"].iter().any(|s| u.len() > s.len() && u[..s.len()].eq_ignore_ascii_case(s));
    if !web(url) || kind == "document" {
        return Verdict::Allow; // a page itself, or data:, blob:, chrome: and the like
    }
    let Ok(req) = adblock::request::Request::new(url, page, kind, "GET") else { return Verdict::Allow };
    let result = rules.check_network_request(&req);
    if !result.should_block() {
        return Verdict::Allow;
    }
    match result.redirect.as_deref().and_then(crate::resources::decode_data_url) {
        Some((mime, body)) => Verdict::Stand(mime, body),
        None => Verdict::Block,
    }
}

#[cfg(test)]
fn blocks(rules: &Rules, url: &str, page: &str, kind: &str) -> bool {
    verdict(rules, url, page, kind) != Verdict::Allow
}

/// Whether blocking is on for the page at `page`: on in the settings and
/// not a site the user allowed (lists loaded or not).
pub(crate) fn active_for(page: &str) -> bool {
    ON.load(Ordering::Relaxed) && !ALLOWED.read().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|allowed| allowed(page))
}

/// The rules in force on the page at `page`: none while blocking is off,
/// on a site the user allowed, or before any list is loaded.
pub(crate) fn rules_for(page: &str) -> Option<Arc<Rules>> {
    if !active_for(page) {
        return None;
    }
    RULES.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// What happens to a request, with the current rules and switches.
fn check(url: &str, page: &str, kind: &str) -> Verdict {
    rules_for(page).map_or(Verdict::Allow, |r| verdict(&r, url, page, kind))
}

/// A stand-in's type and body.
type StandIn = (String, Arc<[u8]>);

// Checks each request of one page (on the IO thread), so it holds only
// thread-safe state.
wrap_resource_request_handler! {
    pub(crate) struct Guard {
        blocked: Arc<Blocked>,
        // Stand-ins decided in `on_before_resource_load`, by request
        // identifier, for `resource_handler` to serve.
        stands: Arc<Mutex<HashMap<u64, StandIn>>>,
    }

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let Some(request) = request else { return ReturnValue::CONTINUE };
            if crate::prefs::do_not_track() {
                for name in ["DNT", "Sec-GPC"] {
                    request.set_header_by_name(Some(&CefString::from(name)), Some(&CefString::from("1")), 1);
                }
            }
            let kind = request_type(request.resource_type());
            if kind == "document" {
                self.blocked.set().clear(); // a new page in the main frame
                return ReturnValue::CONTINUE;
            }
            let url = CefString::from(&request.url()).to_string();
            // The page the request is for: the main frame's address, or
            // before that is known, whatever referred the request.
            let mut page = browser.and_then(|b| b.main_frame()).map(|f| CefString::from(&f.url()).to_string()).unwrap_or_default();
            if page.is_empty() || page == "about:blank" {
                page = CefString::from(&request.referrer_url()).to_string();
            }
            // Images turned off for the page's site (not counted as blocked ads).
            if kind == "image" && !crate::prefs::images_on(&page) {
                return ReturnValue::CANCEL;
            }
            let verdict = check(&url, &page, kind);
            if verdict == Verdict::Allow {
                return ReturnValue::CONTINUE;
            }
            let first = self.blocked.set().insert(url.clone());
            log::debug!(
                "blocked {kind} {}{}{} on {}",
                url.chars().take(200).collect::<String>(),
                if matches!(verdict, Verdict::Stand(..)) { " (stand-in served)" } else { "" },
                if first { "" } else { " (again)" },
                page.chars().take(100).collect::<String>()
            );
            match verdict {
                Verdict::Stand(mime, body) => {
                    self.stands.lock().unwrap_or_else(|e| e.into_inner()).insert(request.identifier(), (mime, body.into()));
                    ReturnValue::CONTINUE
                }
                _ => ReturnValue::CANCEL,
            }
        }

        fn resource_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let id = request?.identifier();
            let (mime, body) = self.stands.lock().unwrap_or_else(|e| e.into_inner()).remove(&id)?;
            Some(Stand::new(mime, body, Default::default(), Default::default()))
        }
    }
}

pub(crate) fn guard(blocked: Arc<Blocked>) -> ResourceRequestHandler {
    Guard::new(blocked, Default::default())
}

// Answers one request with a stand-in body (on the IO thread). Pages fetch
// some with CORS, so the request's origin is let in.
wrap_resource_handler! {
    struct Stand {
        mime: String,
        body: Arc<[u8]>,
        sent: Arc<AtomicUsize>,
        origin: Arc<Mutex<String>>,
    }

    impl ResourceHandler {
        fn open(
            &self,
            request: Option<&mut Request>,
            handle_request: Option<&mut ::std::os::raw::c_int>,
            _callback: Option<&mut Callback>,
        ) -> ::std::os::raw::c_int {
            if let Some(r) = request {
                let origin = CefString::from(&r.header_by_name(Some(&CefString::from("Origin")))).to_string();
                *self.origin.lock().unwrap_or_else(|e| e.into_inner()) = origin;
            }
            if let Some(h) = handle_request {
                *h = 1;
            }
            1
        }

        fn response_headers(
            &self,
            response: Option<&mut Response>,
            response_length: Option<&mut i64>,
            _redirect_url: Option<&mut CefString>,
        ) {
            if let Some(r) = response {
                r.set_status(200);
                r.set_status_text(Some(&CefString::from("OK")));
                r.set_mime_type(Some(&CefString::from(self.mime.as_str())));
                let origin = self.origin.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let (allow, credentials) = if origin.is_empty() || origin == "null" { ("*".to_string(), false) } else { (origin, true) };
                r.set_header_by_name(Some(&CefString::from("Access-Control-Allow-Origin")), Some(&CefString::from(allow.as_str())), 1);
                if credentials {
                    r.set_header_by_name(Some(&CefString::from("Access-Control-Allow-Credentials")), Some(&CefString::from("true")), 1);
                }
                r.set_header_by_name(Some(&CefString::from("Cache-Control")), Some(&CefString::from("no-store")), 1);
            }
            if let Some(len) = response_length {
                *len = self.body.len() as i64;
            }
        }

        fn read(
            &self,
            data_out: *mut u8,
            bytes_to_read: ::std::os::raw::c_int,
            bytes_read: Option<&mut ::std::os::raw::c_int>,
            _callback: Option<&mut ResourceReadCallback>,
        ) -> ::std::os::raw::c_int {
            let start = self.sent.load(Ordering::Relaxed).min(self.body.len());
            let n = (self.body.len() - start).min(bytes_to_read.max(0) as usize);
            if let Some(out) = bytes_read {
                *out = n as ::std::os::raw::c_int;
            }
            if n == 0 || data_out.is_null() {
                return 0; // all sent
            }
            // SAFETY: CEF hands a writable buffer of `bytes_to_read` bytes at `data_out`.
            unsafe { std::ptr::copy_nonoverlapping(self.body[start..].as_ptr(), data_out, n) };
            self.sent.store(start + n, Ordering::Relaxed);
            1
        }

        fn cancel(&self) {}
    }
}

/// Longest body [`fetch`] keeps; the rest is dropped and the fetch fails.
const MAX_BODY: usize = 32 << 20;

thread_local! {
    /// Fetches in flight, kept alive until they complete.
    static FETCHES: RefCell<Vec<(u64, Urlrequest)>> = const { RefCell::new(Vec::new()) };
}
static FETCH_ID: AtomicU64 = AtomicU64::new(0);

wrap_urlrequest_client! {
    struct Fetcher {
        id: u64,
        tag: String,
        body: std::rc::Rc<RefCell<Vec<u8>>>,
        too_big: std::rc::Rc<std::cell::Cell<bool>>,
    }

    impl UrlrequestClient {
        fn on_download_data(&self, _request: Option<&mut Urlrequest>, data: *const u8, data_length: usize) {
            if data.is_null() || data_length == 0 {
                return;
            }
            let mut body = self.body.borrow_mut();
            if body.len() + data_length > MAX_BODY {
                self.too_big.set(true);
                return;
            }
            // SAFETY: CEF hands `data_length` readable bytes at `data` for this call.
            body.extend_from_slice(unsafe { std::slice::from_raw_parts(data, data_length) });
        }

        fn on_request_complete(&self, request: Option<&mut Urlrequest>) {
            let id = self.id;
            FETCHES.with_borrow_mut(|f| f.retain(|(i, _)| *i != id));
            let (ok, http) = request
                .map(|r| (r.request_status() == UrlrequestStatus::SUCCESS, r.response().map(|resp| resp.status()).unwrap_or(0)))
                .unwrap_or((false, 0));
            let ok = ok && http == 200 && !self.too_big.get();
            let body = if ok { std::mem::take(&mut *self.body.borrow_mut()) } else { Vec::new() };
            log::info!("fetch {}: {} (HTTP {http}, {} bytes)", self.tag, if ok { "done" } else { "failed" }, body.len());
            emit(WebEvent::Fetched { tag: self.tag.clone(), ok, body });
        }
    }
}

/// Downloads `url` (GET, through the profile's network stack and cache, not
/// tied to any page, so never blocked); answers with [`WebEvent::Fetched`]
/// carrying `tag`. Returns false if the request couldn't be made.
pub fn fetch(url: &str, tag: &str) -> bool {
    let Some(mut request) = request_create() else { return false };
    request.set_url(Some(&CefString::from(url)));
    request.set_method(Some(&CefString::from("GET")));
    let id = FETCH_ID.fetch_add(1, Ordering::Relaxed);
    let mut client = Fetcher::new(id, tag.to_string(), Default::default(), Default::default());
    match urlrequest_create(Some(&mut request), Some(&mut client), None) {
        Some(r) => {
            FETCHES.with_borrow_mut(|f| f.push((id, r)));
            log::info!("fetch {tag}: {url}");
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(text: &str) -> Rules {
        build_rules(vec![("test".into(), text.into())])
    }

    #[test]
    fn blocks_listed_requests_but_never_pages() {
        let r = rules("||ads.example^\n/banner/*$image\n@@||ads.example/ok.js\n||tracker.example^$third-party");
        let page = "https://news.site/story";
        assert!(blocks(&r, "https://ads.example/a.js", page, "script"));
        assert!(!blocks(&r, "https://ads.example/ok.js", page, "script"), "exception");
        assert!(blocks(&r, "https://cdn.site/banner/1.png", page, "image"));
        assert!(!blocks(&r, "https://cdn.site/banner/1.png", page, "script"), "image rule");
        assert!(blocks(&r, "https://tracker.example/p", page, "xmlhttprequest"));
        assert!(!blocks(&r, "https://tracker.example/p", "https://tracker.example/", "xmlhttprequest"), "first party");
        assert!(!blocks(&r, "https://ads.example/", page, "document"), "a page is never blocked");
        assert!(!blocks(&r, "data:text/plain,ads.example", page, "image"));
        assert!(!blocks(&r, "https://news.site/app.js", page, "script"));
        assert!(blocks(&r, "wss://ads.example/live", page, "websocket"));
    }

    #[test]
    fn resource_types_map_to_adblock_names() {
        assert_eq!(request_type(ResourceType::MAIN_FRAME), "document");
        assert_eq!(request_type(ResourceType::SUB_FRAME), "subdocument");
        assert_eq!(request_type(ResourceType::XHR), "xmlhttprequest");
        assert_eq!(request_type(ResourceType::FAVICON), "image");
        assert_eq!(request_type(ResourceType::PREFETCH), "other");
    }
}
