//! Downloads. Chromium asks where to save each one ([`crate::WebEvent::DownloadAsked`]);
//! the app answers with [`save_to`], then hears its progress
//! ([`crate::WebEvent::DownloadUpdated`]) and may [`cancel`] it.
//!
//! Updates come through the client of the page that started the download,
//! so that page must stay alive (not closed) until the download ends, or
//! the download goes on unheard.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use cef::*;

use crate::WebEvent;

/// Where a download stands, as Chromium reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Running,
    Done,
    Cancelled,
    /// Interrupted: network, disk or server trouble.
    Failed,
}

thread_local! {
    /// Downloads waiting for the app to say where they go.
    static ASKED: RefCell<HashMap<u32, BeforeDownloadCallback>> = RefCell::new(HashMap::new());
    /// Running downloads' controls, for [`cancel`].
    static RUNNING: RefCell<HashMap<u32, DownloadItemCallback>> = RefCell::new(HashMap::new());
    /// Cancelled before their controls arrived.
    static CANCEL: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
}

pub(crate) fn asked(id: u32, callback: BeforeDownloadCallback) {
    ASKED.with_borrow_mut(|m| m.insert(id, callback));
}

/// Saves download `id` at `path` (without asking). Returns false if Chromium
/// isn't waiting for an answer about it.
pub fn save_to(id: u32, path: &Path) -> bool {
    let Some(cb) = ASKED.with_borrow_mut(|m| m.remove(&id)) else { return false };
    cb.cont(Some(&CefString::from(path.to_string_lossy().as_ref())), 0);
    true
}

/// Cancels running download `id`. One that has no controls yet (just
/// answered) is cancelled with its first update.
pub fn cancel(id: u32) {
    match RUNNING.with_borrow(|m| m.get(&id).cloned()) {
        Some(cb) => cb.cancel(),
        None => {
            CANCEL.with_borrow_mut(|s| s.insert(id));
        }
    }
}

pub(crate) fn updated(item: &mut DownloadItem, callback: Option<&mut DownloadItemCallback>) -> Option<WebEvent> {
    let id = item.id();
    let state = if item.is_complete() != 0 {
        Progress::Done
    } else if item.is_canceled() != 0 {
        Progress::Cancelled
    } else if item.is_interrupted() != 0 {
        Progress::Failed
    } else {
        Progress::Running
    };
    if state == Progress::Running {
        if let Some(cb) = callback {
            if CANCEL.with_borrow_mut(|s| s.remove(&id)) {
                cb.cancel();
            }
            RUNNING.with_borrow_mut(|m| m.insert(id, cb.clone()));
        }
    } else {
        RUNNING.with_borrow_mut(|m| m.remove(&id));
        CANCEL.with_borrow_mut(|s| s.remove(&id));
        ASKED.with_borrow_mut(|m| m.remove(&id));
    }
    log::debug!(
        "download {id}: {state:?} {}/{} bytes, path {:?}",
        item.received_bytes(),
        item.total_bytes(),
        CefString::from(&item.full_path()).to_string()
    );
    Some(WebEvent::DownloadUpdated { id, received: item.received_bytes(), total: item.total_bytes(), state })
}
