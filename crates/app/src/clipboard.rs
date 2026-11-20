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

    /// Puts `text` in the primary selection (what a middle click pastes).
    /// Under Wayland `wl-copy` does it when it is installed (arboard's
    /// data-control path can't reach the primary selection on every
    /// compositor); otherwise arboard.
    pub fn set_primary(&mut self, text: &str) {
        use arboard::{LinuxClipboardKind, SetExtLinux};
        if let Some(prog) = wayland_tool("wl-copy") {
            match wl_copy_primary(&prog, text) {
                Ok(()) => {
                    log::info!("primary selection set with {}", prog.display());
                    return;
                }
                Err(e) => log::warn!("wl-copy --primary failed: {e}"),
            }
        }
        if let Some(c) = self.get()
            && let Err(e) = c.set().clipboard(LinuxClipboardKind::Primary).text(text)
        {
            log::warn!("primary selection write failed: {e}");
        }
    }

    /// The primary selection's text (`wl-paste` under Wayland when it is
    /// installed, else arboard).
    pub fn primary(&mut self) -> Option<String> {
        use arboard::{GetExtLinux, LinuxClipboardKind};
        if let Some(prog) = wayland_tool("wl-paste") {
            match std::process::Command::new(prog)
                .args(["--primary", "--no-newline", "--type", "text"])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
            {
                Ok(out) if out.status.success() => return String::from_utf8(out.stdout).ok(),
                // Nothing selected anywhere.
                Ok(_) => return None,
                Err(e) => log::warn!("wl-paste --primary failed: {e}"),
            }
        }
        match self.get()?.get().clipboard(LinuxClipboardKind::Primary).text() {
            Ok(t) => Some(t),
            Err(e) => {
                log::info!("primary selection: {e}");
                None
            }
        }
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

/// `name` on `PATH`, when running under Wayland.
fn wayland_tool(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("WAYLAND_DISPLAY")?;
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

/// `wl-copy --primary` with `text` on its standard input; it forks to keep
/// serving the selection, so this returns once it has read the text.
fn wl_copy_primary(prog: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut child = std::process::Command::new(prog)
        .args(["--primary", "--type", "text/plain;charset=utf-8"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    child.stdin.take().expect("piped").write_all(text.as_bytes())?;
    let status = child.wait()?;
    if status.success() { Ok(()) } else { Err(std::io::Error::other(format!("exited with {status}"))) }
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
