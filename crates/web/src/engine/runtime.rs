//! When to pump CEF, waking the app, paint counters, and the process-wide lookups pages consult (colour scheme, JavaScript, zoom, DevTools ids).

use std::cell::RefCell;
use std::rc::Rc as StdRc;
use std::sync::atomic::{AtomicU64, Ordering};

use cef::*;

use crate::{EVENTS, WebEvent};

/// The longest CEF goes unpumped even when it asked for nothing: a safety
/// net, as in CEF's own external-pump sample, for work it might not
/// schedule. CEF does schedule its work (`on_schedule_message_pump_work`),
/// so this needn't be short: at 33 ms it woke an idle app 30 times a second.
pub const MAX_PUMP_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

/// When CEF next wants [`Engine::pump`], in nanoseconds since [`epoch`];
/// `u64::MAX` when it asked for nothing. Written from any thread.
pub(crate) static PUMP_DUE: AtomicU64 = AtomicU64::new(0);
/// Bumped by every event and every paint, so the app can tell a pump that
/// changed something from one that didn't.
pub(crate) static ACTIVITY: AtomicU64 = AtomicU64::new(0);
pub(crate) static WAKER: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

pub(crate) fn epoch() -> std::time::Instant {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(std::time::Instant::now)
}

pub(crate) fn nanos_at(t: std::time::Instant) -> u64 {
    t.saturating_duration_since(epoch()).as_nanos().min(u64::MAX as u128 - 1) as u64
}

/// Gives `web` a way to wake the app's event loop from any thread (CEF asks
/// for pump work from its own threads). Set once, before CEF starts.
pub fn set_waker(wake: Box<dyn Fn() + Send + Sync>) {
    let _ = WAKER.set(wake);
}

/// Wakes the app's event loop (from any thread) and counts as activity, so
/// the loop draws: call after handing the main thread something through a
/// channel.
pub fn wake() {
    note_activity();
    wake_loop();
}

/// Wakes the loop without asking for a frame (CEF wants pumping).
fn wake_loop() {
    if let Some(w) = WAKER.get() {
        w();
    }
}

/// CEF wants pumping in `delay_ms` (0 or less: now). Any thread.
pub(crate) fn schedule_pump(delay_ms: i64) {
    let due = std::time::Instant::now() + std::time::Duration::from_millis(delay_ms.max(0) as u64);
    PUMP_DUE.fetch_min(nanos_at(due), Ordering::AcqRel);
    wake_loop();
}

/// Page paints received from CEF, and the bytes `on_paint` copied for them,
/// since the start (for `TRAILBOARD_PERF`).
pub(crate) static PAINTS: AtomicU64 = AtomicU64::new(0);
pub(crate) static PAINT_BYTES: AtomicU64 = AtomicU64::new(0);

/// Paints CEF delivered and bytes copied out of its buffers, since the start.
pub fn paint_stats() -> (u64, u64) {
    (PAINTS.load(Ordering::Relaxed), PAINT_BYTES.load(Ordering::Relaxed))
}

pub(crate) fn note_activity() {
    ACTIVITY.fetch_add(1, Ordering::Relaxed);
}

/// A count that grows whenever CEF sent an event, a page painted or
/// [`wake`] was called.
pub fn activity() -> u64 {
    ACTIVITY.load(Ordering::Relaxed)
}

pub fn drain_events() -> Vec<WebEvent> {
    EVENTS.with_borrow_mut(std::mem::take)
}

thread_local! {
    /// Colour scheme pages should report (`None`: leave Chromium's own choice).
    pub(crate) static DARK: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static DEVTOOLS_ID: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

/// Makes pages report a light or dark `prefers-color-scheme`, following the
/// desktop theme. New pages take it when created; call `Page::apply_color_scheme`
/// on live pages after changing it. Windowless (Alloy-style) browsers ignore
/// the request context's Chrome colour scheme, so this goes through DevTools
/// media emulation per page, which also survives navigation.
pub fn set_color_scheme(dark: bool) {
    DARK.set(Some(dark));
}

pub(crate) type ScriptLookup = StdRc<dyn Fn(&str) -> bool>;

thread_local! {
    /// Whether a page at a URL runs JavaScript (see [`set_javascript`]).
    static JAVASCRIPT: RefCell<Option<ScriptLookup>> = const { RefCell::new(None) };
}

/// Sets how web pages find whether they run JavaScript, from their URL.
/// Pages ask before each navigation of their main frame; after changing
/// what it answers, call [`Page::refresh_javascript`] on live pages.
pub fn set_javascript(lookup: impl Fn(&str) -> bool + 'static) {
    JAVASCRIPT.with_borrow_mut(|j| *j = Some(StdRc::new(lookup)));
}

pub(crate) fn javascript_on(url: &str) -> bool {
    let lookup = JAVASCRIPT.with_borrow(|j| j.clone());
    lookup.is_none_or(|f| f(url))
}

/// Sends one DevTools protocol command to a page (its answer is ignored).
pub(crate) fn send_dev_tools(host: &BrowserHost, method: &str, params: &str) -> bool {
    send_dev_tools_id(host, method, params).is_some()
}

/// A DevTools message id no other message of ours uses.
pub(crate) fn next_devtools_id() -> i32 {
    let id = DEVTOOLS_ID.get() + 1;
    DEVTOOLS_ID.set(id);
    id
}

/// [`send_dev_tools`], giving the message's id (its result carries it).
pub(crate) fn send_dev_tools_id(host: &BrowserHost, method: &str, params: &str) -> Option<i32> {
    let id = next_devtools_id();
    let msg = format!(r#"{{"id":{id},"method":"{method}","params":{params}}}"#);
    (host.send_dev_tools_message(Some(msg.as_bytes())) != 0).then_some(id)
}

pub(crate) type ZoomLookup = StdRc<dyn Fn(&str) -> f64>;

thread_local! {
    /// The user's page zoom for a URL (see [`set_page_zoom`]).
    static PAGE_ZOOM: RefCell<Option<ZoomLookup>> = const { RefCell::new(None) };
}

/// Sets how web pages find their user zoom (Ctrl +/−) from their URL: a factor,
/// 1.0 for none. Pages ask on every navigation; after changing what it answers,
/// call [`Page::refresh_zoom`] on live pages. Notes are never zoomed.
pub fn set_page_zoom(lookup: impl Fn(&str) -> f64 + 'static) {
    PAGE_ZOOM.with_borrow_mut(|z| *z = Some(StdRc::new(lookup)));
}

pub(crate) fn page_zoom(url: &str) -> f64 {
    let f = PAGE_ZOOM.with_borrow(|z| z.clone());
    f.map_or(1.0, |f| f(url)).clamp(0.05, 20.0)
}
