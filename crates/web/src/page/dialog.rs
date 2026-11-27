//! A page's own dialogs, answered from the app (M12 step 15). Our own design.
//!
//! `alert`, `confirm`, `prompt` and the "leave this page?" question
//! (`beforeunload`) arrive as [`WebEvent::DialogAsked`]; the page waits
//! until the app calls [`answer`]. A file chooser (`<input type=file>`,
//! `showOpenFilePicker`) arrives as [`WebEvent::FilesAsked`] and waits for
//! [`choose`]. When Chromium drops a dialog itself (the page navigated or
//! closed) the app hears [`WebEvent::DialogWithdrawn`].
//!
//! Everything here runs on the main thread.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use cef::*;

use crate::{emit, WebEvent};

/// Which dialog a page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `alert`: a message and OK.
    Alert,
    /// `confirm`: OK or Cancel.
    Confirm,
    /// `prompt`: a line of text, or Cancel.
    Prompt,
    /// `beforeunload`: leave (or reload) the page, or stay.
    Leave { reload: bool },
}

/// What a file chooser picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    /// One file that exists.
    Open,
    /// One or more files that exist.
    OpenMany,
    /// A folder.
    Folder,
    /// Where to save (the file may not exist yet).
    Save,
}

enum Waiting {
    Js { page: u64, callback: JsdialogCallback },
    Files { callback: FileDialogCallback },
}

thread_local! {
    static WAITING: RefCell<HashMap<u64, Waiting>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

fn next_id() -> u64 {
    let id = NEXT.get();
    NEXT.set(id + 1);
    id
}

/// Answers page dialog `id`: `ok` is OK / Leave (false: Cancel / Stay);
/// `text` is a prompt's answer. Returns false if it no longer waits.
pub fn answer(id: u64, ok: bool, text: &str) -> bool {
    let Some(w) = WAITING.with_borrow_mut(|m| m.remove(&id)) else { return false };
    match w {
        Waiting::Js { callback, .. } => callback.cont(ok as i32, Some(&CefString::from(text))),
        Waiting::Files { callback } => callback.cancel(),
    }
    true
}

/// Answers file chooser `id` with `paths` (none: cancelled). Returns false
/// if it no longer waits.
pub fn choose(id: u64, paths: &[std::path::PathBuf]) -> bool {
    let Some(w) = WAITING.with_borrow_mut(|m| m.remove(&id)) else { return false };
    match w {
        Waiting::Files { callback } if !paths.is_empty() => {
            let mut list = CefStringList::new();
            for p in paths {
                list.append(&p.to_string_lossy());
            }
            callback.cont(Some(&mut list));
        }
        Waiting::Files { callback } => callback.cancel(),
        Waiting::Js { callback, .. } => callback.cont(0, None),
    }
    true
}

/// Drops page `page`'s waiting dialog (Chromium reset it), telling the app.
fn withdraw(page: u64) {
    let ids: Vec<u64> = WAITING.with_borrow_mut(|m| {
        let ids: Vec<u64> = m.iter().filter(|(_, w)| matches!(w, Waiting::Js { page: p, .. } if *p == page)).map(|(id, _)| *id).collect();
        for id in &ids {
            m.remove(id);
        }
        ids
    });
    for id in ids {
        emit(WebEvent::DialogWithdrawn { id });
    }
}

fn text(s: Option<&CefString>) -> String {
    s.map(|s| s.to_string()).unwrap_or_default()
}

fn list(l: Option<&mut CefStringList>) -> Vec<String> {
    l.map(|l| {
        let raw: *mut cef::sys::_cef_string_list_t = l.into();
        CefStringList::from(raw).into_iter().filter(|s| !s.is_empty()).collect()
    })
    .unwrap_or_default()
}

wrap_jsdialog_handler! {
    pub(crate) struct PageDialogs {
        page: u64,
    }

    impl JsdialogHandler {
        fn on_jsdialog(
            &self,
            _browser: Option<&mut Browser>,
            origin_url: Option<&CefString>,
            dialog_type: JsdialogType,
            message_text: Option<&CefString>,
            default_prompt_text: Option<&CefString>,
            callback: Option<&mut JsdialogCallback>,
            _suppress_message: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let Some(callback) = callback else {
                log::warn!("page {}: a dialog came without a callback", self.page);
                return 0;
            };
            use sys::cef_jsdialog_type_t as T;
            let kind = match dialog_type.as_ref() {
                T::JSDIALOGTYPE_CONFIRM => Kind::Confirm,
                T::JSDIALOGTYPE_PROMPT => Kind::Prompt,
                _ => Kind::Alert,
            };
            let id = next_id();
            log::info!("page {}: {kind:?} dialog {id}", self.page);
            WAITING.with_borrow_mut(|m| m.insert(id, Waiting::Js { page: self.page, callback: callback.clone() }));
            emit(WebEvent::DialogAsked {
                page: self.page,
                id,
                kind,
                origin: text(origin_url),
                message: text(message_text),
                default: text(default_prompt_text),
            });
            1
        }

        fn on_before_unload_dialog(
            &self,
            _browser: Option<&mut Browser>,
            _message_text: Option<&CefString>,
            is_reload: ::std::os::raw::c_int,
            callback: Option<&mut JsdialogCallback>,
        ) -> ::std::os::raw::c_int {
            let Some(callback) = callback else {
                log::warn!("page {}: a dialog came without a callback", self.page);
                return 0;
            };
            let id = next_id();
            log::info!("page {}: leave dialog {id}", self.page);
            WAITING.with_borrow_mut(|m| m.insert(id, Waiting::Js { page: self.page, callback: callback.clone() }));
            // Chromium no longer shows a page's own words here, so neither do we.
            emit(WebEvent::DialogAsked {
                page: self.page,
                id,
                kind: Kind::Leave { reload: is_reload != 0 },
                origin: String::new(),
                message: String::new(),
                default: String::new(),
            });
            1
        }

        fn on_reset_dialog_state(&self, _browser: Option<&mut Browser>) {
            withdraw(self.page);
        }
    }
}

wrap_dialog_handler! {
    pub(crate) struct FileDialogs {
        page: u64,
    }

    impl DialogHandler {
        fn on_file_dialog(
            &self,
            _browser: Option<&mut Browser>,
            mode: FileDialogMode,
            title: Option<&CefString>,
            default_file_path: Option<&CefString>,
            accept_filters: Option<&mut CefStringList>,
            _accept_extensions: Option<&mut CefStringList>,
            _accept_descriptions: Option<&mut CefStringList>,
            callback: Option<&mut FileDialogCallback>,
        ) -> ::std::os::raw::c_int {
            let Some(callback) = callback else {
                log::warn!("page {}: a dialog came without a callback", self.page);
                return 0;
            };
            use sys::cef_file_dialog_mode_t as M;
            let mode = match mode.as_ref() {
                M::FILE_DIALOG_OPEN_MULTIPLE => FileMode::OpenMany,
                M::FILE_DIALOG_OPEN_FOLDER => FileMode::Folder,
                M::FILE_DIALOG_SAVE => FileMode::Save,
                _ => FileMode::Open,
            };
            let id = next_id();
            let accept = list(accept_filters);
            log::info!("page {}: file chooser {id} ({mode:?}, accepts {accept:?})", self.page);
            WAITING.with_borrow_mut(|m| m.insert(id, Waiting::Files { callback: callback.clone() }));
            emit(WebEvent::FilesAsked { page: self.page, id, mode, title: text(title), default: text(default_file_path), accept });
            1
        }
    }
}

pub(crate) fn js_handler(page: u64) -> JsdialogHandler {
    PageDialogs::new(page)
}

pub(crate) fn file_handler(page: u64) -> DialogHandler {
    FileDialogs::new(page)
}

/// A dialog's message as the app shows it: runs of blank lines become one,
/// each line trimmed at its end, at most `max` chars (ending in `…`).
pub fn tidy_message(message: &str, max: usize) -> String {
    let mut out = String::new();
    let mut blank = false;
    for line in message.trim().lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank = true;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank { "\n\n" } else { "\n" });
        }
        blank = false;
        out.push_str(line);
    }
    if out.chars().count() > max {
        out = out.chars().take(max.saturating_sub(1)).collect::<String>().trim_end().to_string();
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_tidied_and_capped() {
        assert_eq!(tidy_message("  Hello  \n\n\n\nworld \n", 100), "Hello\n\nworld");
        assert_eq!(tidy_message("a\nb", 100), "a\nb");
        assert_eq!(tidy_message("abcdef", 4), "abc…");
        assert_eq!(tidy_message("", 10), "");
    }
}
