//! Typing keys.

use super::*;

impl App {
    /// Insert mode's own keys (`keys::insert`): the field in your editor,
    /// the primary selection pasted.
    pub(super) fn insert_action(&mut self, action: KeyAction) {
        let Some(id) = self.focused.filter(|_| self.entered()) else { return };
        match action {
            KeyAction::PastePrimary => match self.clipboard.primary().filter(|t| !t.is_empty()) {
                Some(text) => {
                    if let Some(card) = self.cards.get(&id) {
                        card.page.paste_text(&text);
                    }
                }
                None => self.toast("Nothing in the primary selection"),
            },
            KeyAction::EditInEditor => {
                if let Some(e) = &self.editing
                    && e.child.is_some()
                {
                    return self.toast("A field is in your editor already · close it first");
                }
                if let Some(old) = self.editing.take()
                    && let Some(f) = old.file
                {
                    let _ = std::fs::remove_file(f);
                }
                let Some(card) = self.cards.get(&id) else { return };
                self.field_token += 1;
                let token = self.field_token;
                let Some(frame) = card.page.read_field(token) else { return };
                log::info!("editor: reading field {token} in frame {frame}");
                self.editing = Some(Editing {
                    node: id,
                    frame,
                    token,
                    original: String::new(),
                    file: None,
                    child: None,
                    since: Instant::now(),
                    written: None,
                    sent: false,
                });
            }
            _ => {}
        }
    }

    /// A page's answer about the field being edited.
    pub(super) fn field_report(&mut self, node: NodeId, report: web::field::Report) {
        use web::field::Report;
        let Some(e) = self.editing.as_mut().filter(|e| e.node == node) else { return };
        match report {
            Report::Text { token, text } if token == e.token && e.file.is_none() => self.start_editor(text),
            Report::NoField { token } if token == e.token && e.file.is_none() => {
                self.editing = None;
                self.toast("No text field has the keys here");
            }
            Report::Put { token } if token == e.token && e.sent => {
                if let Some(f) = self.editing.take().and_then(|e| e.file) {
                    let _ = std::fs::remove_file(f);
                }
                log::info!("editor: text put back in field {token}");
                self.toast("Text put back");
            }
            Report::Gone { token } if token == e.token && e.sent => {
                let file = self.editing.take().and_then(|e| e.file).map(|f| f.display().to_string()).unwrap_or_default();
                self.toast(format!("The field is gone · your text is in {file}"));
            }
            _ => {}
        }
    }

    /// The field's text came: it goes to a private file and the editor starts on it.
    pub(super) fn start_editor(&mut self, text: String) {
        use std::os::unix::process::CommandExt;
        let Some(e) = self.editing.as_mut() else { return };
        let file = crate::system::editor::file_for(e.token);
        if let Err(err) = crate::system::editor::write_private(&file, &text) {
            self.editing = None;
            return self.toast(format!("Can't write the text for your editor: {err}"));
        }
        let Some(words) = crate::system::editor::command(&file, |n| std::env::var(n).ok(), crate::system::editor::on_path) else {
            let _ = std::fs::remove_file(&file);
            self.editing = None;
            return self.toast("No terminal for your editor · set TRAILBOARD_EDITOR to a command");
        };
        let started = std::process::Command::new(&words[0])
            .args(&words[1..])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .process_group(0)
            .spawn();
        match started {
            Ok(child) => {
                log::info!("editor: started {words:?} (pid {})", child.id());
                e.written = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
                e.original = text;
                e.file = Some(file);
                e.child = Some(child);
                e.since = Instant::now();
                self.toast("Editing in your editor · save and quit to put the text back");
            }
            Err(err) => {
                log::warn!("editor: {words:?}: {err}");
                let _ = std::fs::remove_file(&file);
                self.editing = None;
                self.toast(format!("Can't start {}: {err}", words[0]));
            }
        }
    }

    /// Each frame: when the editor ends, or (if it handed the file to an
    /// editor already running and returned at once) when the file is saved,
    /// the text goes back into the field.
    pub(super) fn tend_editor(&mut self) {
        const ANSWER: Duration = Duration::from_secs(3);
        const HANDED_OFF: Duration = Duration::from_secs(2);
        let Some(e) = self.editing.as_mut() else { return };
        let Some(file) = e.file.clone() else {
            if e.since.elapsed() > ANSWER {
                self.editing = None;
                self.toast("The page didn't say what is in the field");
            }
            return;
        };
        if e.sent {
            if e.since.elapsed() > ANSWER {
                let e = self.editing.take().expect("checked");
                self.toast(format!("The page didn't take the text · it is in {}", file.display()));
                log::warn!("editor: no answer for field {}", e.token);
            }
            return;
        }
        let modified = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
        let done = match e.child.as_mut() {
            Some(c) => match c.try_wait() {
                Ok(None) => false,
                Ok(Some(status)) => {
                    e.child = None;
                    let quick = e.since.elapsed() < HANDED_OFF && modified == e.written;
                    log::info!("editor: exited ({status}){}", if quick { "; watching the file for a save" } else { "" });
                    !quick
                }
                Err(err) => {
                    log::warn!("editor: {err}");
                    e.child = None;
                    true
                }
            },
            None => modified != e.written,
        };
        if !done {
            return;
        }
        let text = std::fs::read_to_string(&file).map(|t| crate::system::editor::saved_text(&t)).unwrap_or_else(|_| e.original.clone());
        if text == e.original {
            let _ = std::fs::remove_file(&file);
            self.editing = None;
            return self.toast("The text is as it was");
        }
        let sent = self.cards.get(&e.node).is_some_and(|c| c.page.write_field(&e.frame, e.token, &text));
        if sent {
            e.sent = true;
            e.since = Instant::now();
        } else {
            self.editing = None;
            self.toast(format!("The card is gone · your text is in {}", file.display()));
        }
    }
}
