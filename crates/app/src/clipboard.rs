//! The desktop clipboard, bridged by the app.
//!
//! Off-screen Chromium has no Wayland surface or input serial, so it can neither
//! publish nor read the desktop selection. The app does it instead: on copy/cut
//! it puts the page's selected text on the desktop clipboard, and on paste it
//! hands desktop text to the page (see [`paste_plan`]).

/// How to paste into a page.
#[derive(Debug, PartialEq, Eq)]
pub enum Paste {
    /// Let Chromium paste from its own clipboard (keeps formatting): the desktop
    /// clipboard still holds what was last copied from a page.
    Native,
    /// Insert this text from the desktop clipboard.
    Text(String),
}

/// Decide how to paste, given the desktop clipboard's text and the text the app
/// last copied out of a page.
pub fn paste_plan(desktop: Option<String>, last_from_page: Option<&str>) -> Paste {
    match desktop {
        Some(t) if !t.is_empty() && Some(t.as_str()) != last_from_page => Paste::Text(t),
        _ => Paste::Native,
    }
}

pub struct Clipboard {
    inner: Option<arboard::Clipboard>,
    failed: bool,
    /// Text we last copied from a page (also on Chromium's own clipboard).
    last_from_page: Option<String>,
}

impl Clipboard {
    pub fn new() -> Self {
        Clipboard { inner: None, failed: false, last_from_page: None }
    }

    fn get(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.inner.is_none() && !self.failed {
            match arboard::Clipboard::new() {
                Ok(c) => self.inner = Some(c),
                Err(e) => {
                    log::warn!("clipboard unavailable: {e}");
                    self.failed = true;
                }
            }
        }
        self.inner.as_mut()
    }

    pub fn set_text(&mut self, text: &str) {
        if let Some(c) = self.get()
            && let Err(e) = c.set_text(text)
        {
            log::warn!("clipboard write failed: {e}");
        }
    }

    pub fn text(&mut self) -> Option<String> {
        self.get()?.get_text().ok()
    }

    /// Text was copied from a page: Chromium has it, and so does the desktop now.
    pub fn copied_from_page(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        self.set_text(&text);
        self.last_from_page = Some(text);
    }

    pub fn paste_plan(&mut self) -> Paste {
        let desktop = self.text();
        paste_plan(desktop, self.last_from_page.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_prefers_desktop_text_unless_it_came_from_a_page() {
        assert_eq!(paste_plan(Some("x".into()), None), Paste::Text("x".into()));
        assert_eq!(paste_plan(Some("x".into()), Some("y")), Paste::Text("x".into()));
        assert_eq!(paste_plan(Some("y".into()), Some("y")), Paste::Native);
        assert_eq!(paste_plan(None, Some("y")), Paste::Native);
        assert_eq!(paste_plan(Some(String::new()), None), Paste::Native);
    }
}
