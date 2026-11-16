//! Site permissions: camera, microphone, location, notifications. Chromium
//! asks the page's client ([`Permissions`]); the app's policy ([`set_policy`])
//! answers at once, or says to ask the user, in which case the request waits
//! ([`WebEvent::PermissionAsked`]) until the app calls [`answer`]. A site's
//! answer is remembered for the session ([`remember`]), per kind.
//!
//! Everything here runs on the main thread.

use std::cell::RefCell;
use std::collections::HashMap;

use cef::*;

use crate::{WebEvent, emit};

/// What a site asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Camera,
    Microphone,
    Location,
    Notifications,
    /// Anything else (screen capture, MIDI, clipboard, …): always refused.
    Other,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Camera => "camera",
            Kind::Microphone => "microphone",
            Kind::Location => "location",
            Kind::Notifications => "notifications",
            Kind::Other => "something else",
        }
    }
}

/// The policy's answer for one kind on one site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Block,
    Ask,
}

type Policy = Box<dyn Fn(Kind) -> Decision>;

/// A request waiting for the user.
enum Waiting {
    /// Camera and microphone (`getUserMedia`): the bits allowed already, the
    /// bits asked about.
    Media { callback: MediaAccessCallback, allowed: u32, asked: u32 },
    Prompt { callback: PermissionPromptCallback },
}

thread_local! {
    static POLICY: RefCell<Option<Policy>> = const { RefCell::new(None) };
    /// Answers the user gave this session, by (site origin, kind).
    static ANSWERS: RefCell<HashMap<(String, Kind), bool>> = RefCell::new(HashMap::new());
    static WAITING: RefCell<HashMap<u64, Waiting>> = RefCell::new(HashMap::new());
    static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

/// Sets what each kind gets when the user hasn't answered for the site.
pub fn set_policy(policy: impl Fn(Kind) -> Decision + 'static) {
    POLICY.with_borrow_mut(|p| *p = Some(Box::new(policy)));
}

/// Remembers the user's answer for `origin` and `kinds` for this session.
pub fn remember(origin: &str, kinds: &[Kind], allow: bool) {
    ANSWERS.with_borrow_mut(|a| {
        for k in kinds {
            a.insert((origin.to_string(), *k), allow);
        }
    });
}

/// Forgets every answer (after the settings change, so they apply again).
pub fn forget_answers() {
    ANSWERS.with_borrow_mut(|a| a.clear());
}

fn decide(origin: &str, kind: Kind) -> Decision {
    if kind == Kind::Other {
        return Decision::Block;
    }
    if let Some(allow) = ANSWERS.with_borrow(|a| a.get(&(origin.to_string(), kind)).copied()) {
        return if allow { Decision::Allow } else { Decision::Block };
    }
    POLICY.with_borrow(|p| p.as_ref().map_or(Decision::Block, |p| p(kind)))
}

/// Answers request `id` (from [`WebEvent::PermissionAsked`]). Returns false if
/// it no longer waits (the page went away or Chromium withdrew it).
pub fn answer(id: u64, allow: bool) -> bool {
    let Some(w) = WAITING.with_borrow_mut(|m| m.remove(&id)) else { return false };
    match w {
        Waiting::Media { callback, allowed, asked } => {
            let bits = if allow { allowed | asked } else { allowed };
            if bits == 0 { callback.cancel() } else { callback.cont(bits) }
        }
        Waiting::Prompt { callback } => {
            let result = if allow { PermissionRequestResult::ACCEPT } else { PermissionRequestResult::DENY };
            callback.cont(result);
        }
    }
    true
}

fn next_id() -> u64 {
    let id = NEXT.get();
    NEXT.set(id + 1);
    id
}

/// Which of the media bits `bits` are which kind.
fn media_kinds(bits: u32) -> Vec<(u32, Kind)> {
    use sys::cef_media_access_permission_types_t as M;
    let audio = M::CEF_MEDIA_PERMISSION_DEVICE_AUDIO_CAPTURE as u32;
    let video = M::CEF_MEDIA_PERMISSION_DEVICE_VIDEO_CAPTURE as u32;
    let mut out = Vec::new();
    for bit in (0..32).map(|i| 1u32 << i).filter(|b| bits & b != 0) {
        let kind = if bit == audio {
            Kind::Microphone
        } else if bit == video {
            Kind::Camera
        } else {
            Kind::Other
        };
        out.push((bit, kind));
    }
    out
}

/// The kinds in a permission prompt's `bits`.
fn prompt_kinds(bits: u32) -> Vec<Kind> {
    use sys::cef_permission_request_types_t as P;
    let mut out = Vec::new();
    for bit in (0..32).map(|i| 1u32 << i).filter(|b| bits & b != 0) {
        let kind = if bit == P::CEF_PERMISSION_TYPE_CAMERA_STREAM as u32 {
            Kind::Camera
        } else if bit == P::CEF_PERMISSION_TYPE_MIC_STREAM as u32 {
            Kind::Microphone
        } else if bit == P::CEF_PERMISSION_TYPE_GEOLOCATION as u32 {
            Kind::Location
        } else if bit == P::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32 {
            Kind::Notifications
        } else {
            Kind::Other
        };
        if !out.contains(&kind) {
            out.push(kind);
        }
    }
    out
}

/// How a group of decisions comes out: refused if any is, granted if all
/// are, else the user is asked (about the ones not decided).
fn overall(decisions: &[Decision]) -> Decision {
    if decisions.contains(&Decision::Block) {
        Decision::Block
    } else if decisions.iter().all(|d| *d == Decision::Allow) {
        Decision::Allow
    } else {
        Decision::Ask
    }
}

wrap_permission_handler! {
    pub(crate) struct Permissions {
        page: u64,
    }

    impl PermissionHandler {
        fn on_request_media_access_permission(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut MediaAccessCallback>,
        ) -> ::std::os::raw::c_int {
            let Some(callback) = callback else { return 0 };
            let origin = requesting_origin.map(|o| o.to_string()).unwrap_or_default();
            let (mut allowed, mut asked, mut ask_kinds) = (0u32, 0u32, Vec::new());
            for (bit, kind) in media_kinds(requested_permissions) {
                match decide(&origin, kind) {
                    Decision::Allow => allowed |= bit,
                    Decision::Block => {}
                    Decision::Ask => {
                        asked |= bit;
                        if !ask_kinds.contains(&kind) {
                            ask_kinds.push(kind);
                        }
                    }
                }
            }
            log::info!("page {}: {origin} asks for media {requested_permissions:#x}: allowed {allowed:#x}, asking {asked:#x}", self.page);
            if asked == 0 {
                if allowed == 0 { callback.cancel() } else { callback.cont(allowed) }
                return 1;
            }
            let id = next_id();
            WAITING.with_borrow_mut(|m| m.insert(id, Waiting::Media { callback: callback.clone(), allowed, asked }));
            emit(WebEvent::PermissionAsked { page: self.page, id, origin, kinds: ask_kinds });
            1
        }

        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            prompt_id: u64,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> ::std::os::raw::c_int {
            let Some(callback) = callback else { return 0 };
            let origin = requesting_origin.map(|o| o.to_string()).unwrap_or_default();
            let kinds = prompt_kinds(requested_permissions);
            let decisions: Vec<Decision> = kinds.iter().map(|k| decide(&origin, *k)).collect();
            let verdict = overall(&decisions);
            log::info!("page {}: {origin} asks for {kinds:?} (prompt {prompt_id}): {verdict:?}", self.page);
            match verdict {
                Decision::Allow => callback.cont(PermissionRequestResult::ACCEPT),
                Decision::Block => callback.cont(PermissionRequestResult::DENY),
                Decision::Ask => {
                    let ask: Vec<Kind> = kinds.iter().zip(&decisions).filter(|(_, d)| **d == Decision::Ask).map(|(k, _)| *k).collect();
                    WAITING.with_borrow_mut(|m| m.insert(prompt_id | PROMPT_BIT, Waiting::Prompt { callback: callback.clone() }));
                    emit(WebEvent::PermissionAsked { page: self.page, id: prompt_id | PROMPT_BIT, origin, kinds: ask });
                }
            }
            1
        }

        fn on_dismiss_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            prompt_id: u64,
            _result: PermissionRequestResult,
        ) {
            let id = prompt_id | PROMPT_BIT;
            if WAITING.with_borrow_mut(|m| m.remove(&id)).is_some() {
                emit(WebEvent::PermissionWithdrawn { id });
            }
        }
    }
}

/// Set on the ids of Chromium's prompts, so they never meet media requests' ids.
const PROMPT_BIT: u64 = 1 << 63;

pub(crate) fn handler(page: u64) -> PermissionHandler {
    Permissions::new(page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_wins_and_all_must_allow() {
        use Decision::*;
        assert_eq!(overall(&[Allow, Allow]), Allow);
        assert_eq!(overall(&[Allow, Ask]), Ask);
        assert_eq!(overall(&[Ask, Block]), Block);
        assert_eq!(overall(&[]), Allow);
    }

    #[test]
    fn answers_cover_the_policy_for_their_site() {
        set_policy(|k| if k == Kind::Camera { Decision::Ask } else { Decision::Block });
        assert_eq!(decide("https://a.com", Kind::Camera), Decision::Ask);
        assert_eq!(decide("https://a.com", Kind::Location), Decision::Block);
        assert_eq!(decide("https://a.com", Kind::Other), Decision::Block);
        remember("https://a.com", &[Kind::Camera], true);
        assert_eq!(decide("https://a.com", Kind::Camera), Decision::Allow);
        assert_eq!(decide("https://b.com", Kind::Camera), Decision::Ask);
        remember("https://a.com", &[Kind::Location], true);
        assert_eq!(decide("https://a.com", Kind::Location), Decision::Allow, "the user's answer wins");
        forget_answers();
        assert_eq!(decide("https://a.com", Kind::Camera), Decision::Ask);
    }
}
