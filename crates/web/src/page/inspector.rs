//! The page inspector in a card. Chromium only draws its own inspector
//! window on screen, so a card instead loads the inspector's pages (bundled
//! with Chromium, `devtools://`) like any other page and connects them to
//! the inspected page through us, with no network port:
//!
//! - The inspector's pages talk the DevTools protocol over a WebSocket named
//!   in their address (`?ws=`). [`SHIM`] stands in for `WebSocket` there:
//!   what the inspector sends comes to us as a console message starting
//!   with [`PREFIX`] ([`Bridge::on_frontend_message`]).
//! - We send it to the inspected page with `send_dev_tools_message`, under
//!   an id of our own so it can't be taken for one of the app's own
//!   commands (zoom, colour scheme, scriptlets), and put the inspector's id
//!   back on the answer.
//! - Answers and events from the inspected page come to a DevTools observer
//!   and go to the inspector's page as a script call.
//! - Chromium gives `devtools://` pages its own inspector host, whose menus
//!   are native windows an off-screen page never shows; [`SHIM`] puts the
//!   inspector in hosted mode, which draws them in the page.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use cef::*;

use crate::js_string;

/// The inspector's pages, told to connect to [`SHIM`]'s stand-in socket.
pub const FRONTEND_URL: &str = "devtools://devtools/bundled/devtools_app.html?ws=trailboard.invalid/inspector";

/// Prefix of the console message carrying one protocol message from the
/// inspector's pages.
pub const PREFIX: &str = "\u{1}trailboard-inspector\u{1}";

/// Run in the inspector's page as it starts loading: a `WebSocket` that,
/// for our address only, sends with a console message and receives from
/// [`deliver_script`]. Every other address gets the real one.
pub const SHIM: &str = r#"(() => {
  const key = Symbol.for('trailboard.inspector');
  if (window[key]) return;
  const Real = window.WebSocket;
  const prefix = '\u0001trailboard-inspector\u0001';
  const say = console.debug.bind(console);
  class Line extends EventTarget {
    constructor(url) {
      super();
      this.url = url;
      this.protocol = '';
      this.extensions = '';
      this.binaryType = 'blob';
      this.bufferedAmount = 0;
      this.readyState = 0;
      this.onopen = this.onmessage = this.onerror = this.onclose = null;
      window[key] = this;
      setTimeout(() => { this.readyState = 1; this.fire(new Event('open')); });
    }
    send(m) { if (this.readyState === 1) say(prefix + m); }
    close() {
      if (this.readyState >= 2) return;
      this.readyState = 3;
      this.fire(new CloseEvent('close', { wasClean: true, code: 1000 }));
    }
    fire(ev) {
      const h = this['on' + ev.type];
      if (typeof h === 'function') h.call(this, ev);
      this.dispatchEvent(ev);
    }
    receive(list) {
      for (const data of list) this.fire(new MessageEvent('message', { data }));
    }
  }
  function WebSocket(url, protocols) {
    if (String(url).startsWith('ws://trailboard.invalid/')) return new Line(String(url));
    return protocols === undefined ? new Real(url) : new Real(url, protocols);
  }
  WebSocket.prototype = Real.prototype;
  Object.assign(WebSocket, { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 });
  window.WebSocket = WebSocket;
  window[key] = null;
  // Chromium gives these pages its own host for the inspector, which
  // would show menus as native windows we never see: hosted mode draws
  // them in the page instead.
  let host;
  Object.defineProperty(window, 'InspectorFrontendHost', {
    configurable: true,
    get() { return host; },
    set(h) {
      if (h && typeof h === 'object') h.isHostedMode = () => true;
      host = h;
    },
  });
})();"#;

/// The script that hands `messages` to the inspector's page.
fn deliver_script(messages: &[String]) -> String {
    let mut s = String::from("window[Symbol.for('trailboard.inspector')]?.receive([");
    for (i, m) in messages.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&js_string(m));
    }
    s.push_str("])");
    s
}

/// The id of an answer from Chromium, and where it ends. Chromium writes
/// the id first (`{"id":12,"result":…}`); events have none.
fn answer_id(msg: &str) -> Option<(i32, usize)> {
    let rest = msg.strip_prefix(r#"{"id":"#)?;
    let len = rest.bytes().take_while(u8::is_ascii_digit).count();
    let id = rest[..len].parse().ok()?;
    Some((id, r#"{"id":"#.len() + len))
}

/// `msg` with its id (ending at `end`, from [`answer_id`]) set to `id`.
fn with_answer_id(msg: &str, end: usize, id: i32) -> String {
    format!(r#"{{"id":{id}{}"#, &msg[end..])
}

/// A message from the inspector under our `id` instead of its own: its id
/// and method, and the message to send. `None` if it isn't a command.
fn retag(msg: &str, id: i32) -> Option<(i64, String, String)> {
    let mut v: serde_json::Value = serde_json::from_str(msg).ok()?;
    let obj = v.as_object_mut()?;
    let theirs = obj.get("id")?.as_i64()?;
    let method = obj.get("method")?.as_str()?.to_string();
    obj.insert("id".into(), id.into());
    Some((theirs, method, v.to_string()))
}

/// A console message one of the app's own scripts in the page sent us
/// (they all start `\u{1}trailboard-`): not the page's, so not shown.
fn is_app_message(msg: &str) -> bool {
    msg.starts_with(r#"{"method":"Runtime.consoleAPICalled""#)
        && (msg.contains(r#""value":"\u0001trailboard-"#) || msg.contains("\"value\":\"\u{1}trailboard-"))
}

/// The node an answer to `DOM.getNodeForLocation` names.
fn located_node(msg: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(msg).ok()?;
    v.get("result")?.get("backendNodeId")?.as_i64()
}

/// The event that makes the inspector show `node`, as if picked in the page.
fn inspect_event(node: i64) -> String {
    format!(r#"{{"method":"Overlay.inspectNodeRequested","params":{{"backendNodeId":{node}}}}}"#)
}

/// Domains the inspector may have turned on, turned off when it closes: a
/// page paused in the debugger goes on, and highlights go away. (The app's
/// own commands use `Page` and `Emulation`, which stay.)
const DOMAINS_OFF: &[&str] =
    &["Debugger", "Overlay", "DOM", "CSS", "Network", "Profiler", "HeapProfiler", "Log", "Audits", "Animation", "LayerTree", "Runtime"];

struct State {
    /// The page inspected.
    target: BrowserHost,
    /// The inspector's own page.
    frontend: Browser,
    /// Our message ids to the inspector's.
    ids: RefCell<HashMap<i32, i64>>,
    /// A point to inspect (view px of the inspected page) once the inspector is ready.
    pending: Cell<Option<(i32, i32)>>,
    /// The inspector has turned on its overlay (it can show a node now).
    ready: Cell<bool>,
    /// Our `DOM.getNodeForLocation`, until its answer comes.
    locating: Cell<Option<i32>>,
}

impl State {
    fn deliver(&self, messages: &[String]) {
        if let Some(f) = self.frontend.main_frame() {
            f.execute_java_script(Some(&CefString::from(deliver_script(messages).as_str())), None, 0);
        }
    }

    fn locate(&self) {
        let Some((x, y)) = self.pending.take() else { return };
        // The protocol's points are CSS px of the page; ours are view px.
        let zoom = 1.2f64.powf(self.target.zoom_level()).max(0.01);
        let (x, y) = ((x as f64 / zoom).round() as i64, (y as f64 / zoom).round() as i64);
        let params = format!(r#"{{"x":{x},"y":{y},"includeUserAgentShadowDOM":false,"ignorePointerEventsNone":true}}"#);
        self.locating.set(crate::send_dev_tools_id(&self.target, "DOM.getNodeForLocation", &params));
    }

    fn on_backend_message(&self, msg: &str) {
        let Some((id, end)) = answer_id(msg) else {
            if !is_app_message(msg) {
                self.deliver(&[msg.to_string()]); // an event
            }
            return;
        };
        if self.locating.get() == Some(id) {
            self.locating.set(None);
            match located_node(msg) {
                Some(node) => self.deliver(&[inspect_event(node)]),
                None => log::debug!("inspector: nothing to inspect there"),
            }
            return;
        }
        // Answers to the app's own commands aren't the inspector's.
        let Some(theirs) = self.ids.borrow_mut().remove(&id) else { return };
        let Ok(theirs) = i32::try_from(theirs) else { return };
        self.deliver(&[with_answer_id(msg, end, theirs)]);
    }
}

impl Drop for State {
    fn drop(&mut self) {
        for d in DOMAINS_OFF {
            crate::send_dev_tools(&self.target, &format!("{d}.disable"), "{}");
        }
    }
}

wrap_dev_tools_message_observer! {
    struct FromTarget {
        state: Rc<State>,
    }

    impl DevToolsMessageObserver {
        fn on_dev_tools_message(&self, _browser: Option<&mut Browser>, message: Option<&[u8]>) -> ::std::os::raw::c_int {
            if let Some(msg) = message.and_then(|m| std::str::from_utf8(m).ok()) {
                self.state.on_backend_message(msg);
            }
            0 // others (the app's scriptlets) still get their answers
        }
    }
}

/// An inspector page's line to the page it inspects. Dropping it stops the
/// messages and turns off what the inspector turned on.
pub struct Bridge {
    state: Rc<State>,
    _registration: Registration,
}

impl Bridge {
    /// Connects inspector page `frontend` to the page of `target`; `at`: a
    /// point (view px) to inspect once the inspector is ready.
    pub(crate) fn new(target: &BrowserHost, frontend: Browser, at: Option<(i32, i32)>) -> Option<Bridge> {
        let state = Rc::new(State {
            target: target.clone(),
            frontend,
            ids: RefCell::new(HashMap::new()),
            pending: Cell::new(at),
            ready: Cell::new(false),
            locating: Cell::new(None),
        });
        let mut observer = FromTarget::new(state.clone());
        let registration = target.add_dev_tools_message_observer(Some(&mut observer))?;
        Some(Bridge { state, _registration: registration })
    }

    /// One protocol message from the inspector's page, to the inspected page.
    pub(crate) fn on_frontend_message(&self, msg: &str) {
        let s = &self.state;
        let id = crate::next_devtools_id();
        let Some((theirs, method, out)) = retag(msg, id) else {
            log::debug!("inspector: dropped a message that isn't a command");
            return;
        };
        s.ids.borrow_mut().insert(id, theirs);
        if s.target.send_dev_tools_message(Some(out.as_bytes())) == 0 {
            s.ids.borrow_mut().remove(&id);
            log::warn!("inspector: couldn't send {method}");
            return;
        }
        if method == "Overlay.enable" {
            s.ready.set(true);
            s.locate();
        }
    }

    /// The inspected browser's identifier.
    pub(crate) fn target_id(&self) -> i32 {
        self.state.target.browser().map_or(0, |b| b.identifier())
    }

    /// Shows the element at `at` (view px of the inspected page).
    pub(crate) fn inspect(&self, at: (i32, i32)) {
        self.state.pending.set(Some(at));
        if self.state.ready.get() {
            self.state.locate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_told_from_events() {
        assert_eq!(answer_id(r#"{"id":12,"result":{}}"#), Some((12, 8)));
        assert_eq!(answer_id(r#"{"method":"DOM.documentUpdated","params":{}}"#), None);
        assert_eq!(answer_id(r#"{"id":"x"}"#), None);
    }

    #[test]
    fn answers_get_the_inspectors_id_back() {
        let msg = r#"{"id":907,"result":{"a":1},"sessionId":"S"}"#;
        let (id, end) = answer_id(msg).unwrap();
        assert_eq!(id, 907);
        assert_eq!(with_answer_id(msg, end, 3), r#"{"id":3,"result":{"a":1},"sessionId":"S"}"#);
    }

    #[test]
    fn commands_are_sent_under_our_id() {
        let (theirs, method, out) = retag(r#"{"id":5,"method":"DOM.enable","params":{},"sessionId":"S"}"#, 900).unwrap();
        assert_eq!((theirs, method.as_str()), (5, "DOM.enable"));
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["id"], 900);
        assert_eq!(v["sessionId"], "S");
        assert!(retag(r#"{"method":"x"}"#, 1).is_none());
        assert!(retag("not json", 1).is_none());
    }

    #[test]
    fn the_apps_own_console_messages_are_left_out() {
        let ours = r#"{"method":"Runtime.consoleAPICalled","params":{"type":"log","args":[{"type":"string","value":"\u0001trailboard-text\u0001file:///x"}]}}"#;
        assert!(is_app_message(ours));
        let raw = "{\"method\":\"Runtime.consoleAPICalled\",\"params\":{\"args\":[{\"type\":\"string\",\"value\":\"\u{1}trailboard-focus\u{1}x\"}]}}";
        assert!(is_app_message(raw));
        let theirs = r#"{"method":"Runtime.consoleAPICalled","params":{"type":"log","args":[{"type":"string","value":"page says hi"}]}}"#;
        assert!(!is_app_message(theirs));
        assert!(!is_app_message(r#"{"method":"DOM.documentUpdated","params":{}}"#));
    }

    #[test]
    fn a_located_node_becomes_an_inspect_event() {
        assert_eq!(located_node(r#"{"id":4,"result":{"backendNodeId":17,"frameId":"F"}}"#), Some(17));
        assert_eq!(located_node(r#"{"id":4,"error":{"code":-32000}}"#), None);
        assert_eq!(inspect_event(17), r#"{"method":"Overlay.inspectNodeRequested","params":{"backendNodeId":17}}"#);
    }

    #[test]
    fn messages_reach_the_page_as_strings() {
        let s = deliver_script(&[r#"{"a":"</script>"}"#.to_string(), "x\u{2028}".to_string()]);
        assert!(s.starts_with("window[Symbol.for('trailboard.inspector')]?.receive(["));
        assert!(!s.contains("</script>") && !s.contains('\u{2028}'));
        assert!(s.ends_with("])"));
    }

    #[test]
    fn the_shim_names_our_prefix_and_address() {
        assert!(SHIM.contains(r"\u0001trailboard-inspector\u0001"));
        assert_eq!(PREFIX, "\u{1}trailboard-inspector\u{1}");
        // Menus drawn in the page, not as native windows.
        assert!(SHIM.contains("isHostedMode = () => true"));
        let addr = FRONTEND_URL.split("?ws=").nth(1).unwrap();
        assert!(SHIM.contains(&format!("'ws://{}", addr.split('/').next().unwrap())));
    }
}
