//! One live web page (`Page`) and what the app can ask of it; cookie jars; small helpers.

use crate::*;

/// One live web page, rendered off screen.
pub struct Page {
    browser: Browser,
    shared: StdRc<Shared>,
    /// Requests [`shield`] stopped since the main frame last navigated (written on the IO thread).
    blocked: std::sync::Arc<shield::Blocked>,
}

/// Shared toggle for "open links beside the page" (Marble's default behaviour).
#[derive(Clone)]
pub struct SpawnBeside(StdRc<RefCell<bool>>);

impl SpawnBeside {
    pub fn new(on: bool) -> Self {
        Self(StdRc::new(RefCell::new(on)))
    }
    pub fn set(&self, on: bool) {
        *self.0.borrow_mut() = on;
    }
    pub fn get(&self) -> bool {
        *self.0.borrow()
    }
}

/// A cookie jar and cache of their own, kept in memory only, for the pages of
/// a private space: nothing they store reaches the profile on disk, and it is
/// gone when the last page using it and this handle are dropped.
#[derive(Clone)]
pub struct Jar(RequestContext);

impl Jar {
    pub fn in_memory() -> Option<Jar> {
        // An empty cache path makes an "incognito" context.
        let settings = RequestContextSettings {
            persist_session_cookies: 0,
            accept_language_list: CefString::from(prefs::accept_language().as_str()),
            ..Default::default()
        };
        let ctx = request_context_create_context(Some(&settings), None)?;
        prefs::apply_to(&ctx);
        Some(Jar(ctx))
    }

    /// Sets the current browsing preferences ([`prefs::apply`]) on this jar.
    pub fn refresh_prefs(&self) {
        prefs::apply_to(&self.0);
    }
}

impl Page {
    /// The state and handlers of a new page `key` of `kind` at `url`.
    fn parts(key: u64, url: &str, kind: PageKind, size: (i32, i32), scale: f32, spawn_beside: &SpawnBeside) -> (StdRc<Shared>, std::sync::Arc<shield::Blocked>, Client) {
        let shared = StdRc::new(Shared {
            key,
            size: RefCell::new(size),
            scale: RefCell::new(scale),
            recent: RefCell::new(vec![(size, scale)]),
            frame: RefCell::new(PaintFrame::default()),
            view: RefCell::new(None),
            popup: RefCell::new(PopupState::default()),
            selection: RefCell::new(String::new()),
            ime_caret: RefCell::new(None),
            initial_url: url.to_string(),
            url: RefCell::new(url.to_string()),
            icon_url: RefCell::new(String::new()),
            kind,
            committed: std::cell::Cell::new(false),
            hide: Default::default(),
            early: Default::default(),
            banner_rejected: std::cell::Cell::new(false),
            media: Default::default(),
            fullscreen: Default::default(),
            hold_focus: std::cell::Cell::new(false),
            script_off: std::cell::Cell::new(false),
            bridge: RefCell::new(None),
        });
        let blocked = std::sync::Arc::new(shield::Blocked::default());
        let client = PageClient::new(
            PageRender::new(shared.clone()),
            PageLife::new(shared.clone()),
            PageRequests::new(shared.clone(), spawn_beside.0.clone(), (kind == PageKind::Web).then(|| shield::guard(blocked.clone()))),
            PageDisplay::new(shared.clone()),
            PageLoad::new(shared.clone()),
            PageMenu::new(shared.clone()),
            PageFind::new(shared.clone()),
            PageDownloads::new(shared.clone()),
            permission::handler(key),
            dialog::js_handler(key),
            dialog::file_handler(key),
        );
        (shared, blocked, client)
    }

    fn browser_settings() -> BrowserSettings {
        BrowserSettings {
            windowless_frame_rate: 60,
            background_color: 0xffff_ffff,
            ..Default::default()
        }
    }

    /// Creates a page identified by `key` (the app's card id) at `size` CSS
    /// pixels. With a `jar`, it keeps cookies and cache there instead of the profile.
    pub fn new(
        key: u64,
        url: &str,
        kind: PageKind,
        size: (i32, i32),
        scale: f32,
        spawn_beside: &SpawnBeside,
        jar: Option<&Jar>,
    ) -> Option<Page> {
        let (shared, blocked, mut client) = Self::parts(key, url, kind, size, scale, spawn_beside);
        let window_info = WindowInfo { windowless_rendering_enabled: 1, ..Default::default() };
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from(url)),
            Some(&Self::browser_settings()),
            None,
            jar.map(|j| j.0.clone()).as_mut(),
        )?;
        if let Some(h) = browser.host() {
            shared.apply_zoom(&h);
            shared.apply_javascript(&h, url);
            if kind == PageKind::Web {
                shared.early.watch(&h);
                // The media script also runs before each document's own
                // scripts (in the main frame and frames in its process), so
                // it sees a DRM request made at once. Load start still
                // installs it in other frames; it installs once.
                let params = serde_json::json!({ "source": media::script() }).to_string();
                send_dev_tools(&h, "Page.addScriptToEvaluateOnNewDocument", &params);
            }
        }
        let page = Page { browser, shared, blocked };
        page.apply_color_scheme();
        Some(page)
    }

    /// Opens Chromium's inspector for this page as a page of its own, `key`,
    /// rendered off screen like any other at `size` CSS px: the inspector's
    /// own pages, talking to this page through [`inspector::Bridge`]. `at`:
    /// a point in this page (CSS px) whose element it opens on.
    pub fn open_inspector(&self, key: u64, size: (i32, i32), scale: f32, at: Option<(f32, f32)>, spawn_beside: &SpawnBeside) -> Option<Page> {
        let target = self.host()?;
        let page = Page::new(key, inspector::FRONTEND_URL, PageKind::Inspector, size, scale, spawn_beside, None)?;
        let bridge = inspector::Bridge::new(&target, page.browser.clone(), at.map(|p| self.view_point(p)))?;
        *page.shared.bridge.borrow_mut() = Some(bridge);
        log::info!("page {}: inspector {key} open", self.shared.key);
        Some(page)
    }

    /// Points `inspector`, this page's open inspector, at the element at `at` (CSS px).
    pub fn inspect_at(&self, inspector: &Page, at: (f32, f32)) {
        if let Some(b) = inspector.shared.bridge.borrow().as_ref() {
            b.inspect(self.view_point(at));
        }
    }

    /// This page is an inspector of `page`'s browser (still the same one).
    pub fn inspects(&self, page: &Page) -> bool {
        self.shared.bridge.borrow().as_ref().is_some_and(|b| b.target_id() == page.browser.identifier())
    }

    /// A point in CSS px as Chromium's view px.
    fn view_point(&self, (x, y): (f32, f32)) -> (i32, i32) {
        let s = *self.shared.scale.borrow();
        ((x * s).round() as i32, (y * s).round() as i32)
    }

    /// Sends the colour scheme set with `set_color_scheme` to this page.
    pub fn apply_color_scheme(&self) {
        let (Some(dark), Some(host)) = (DARK.get(), self.browser.host()) else { return };
        let scheme = if dark { "dark" } else { "light" };
        let params = format!(r#"{{"features":[{{"name":"prefers-color-scheme","value":"{scheme}"}}]}}"#);
        if !send_dev_tools(&host, "Emulation.setEmulatedMedia", &params) {
            log::warn!("page {}: couldn't set colour scheme", self.shared.key);
        }
    }

    /// Asks [`set_javascript`] again for the page's address; if the answer
    /// changed, applies it and reloads (scripts that ran can't be taken back,
    /// and ones that didn't need a new load to run).
    pub fn refresh_javascript(&self) {
        let Some(host) = self.browser.host() else { return };
        let was = self.shared.script_off.get();
        let url = self.shared.url.borrow().clone();
        self.shared.apply_javascript(&host, &url);
        if self.shared.script_off.get() != was {
            self.browser.reload();
        }
    }

    /// Puts `word` in place of the misspelt word the menu was opened on.
    pub fn replace_misspelling(&self, word: &str) {
        if let Some(h) = self.browser.host() {
            h.replace_misspelling(Some(&CefString::from(word)));
        }
    }

    /// Stops marking `word` as misspelt.
    pub fn add_to_dictionary(&self, word: &str) {
        if let Some(h) = self.browser.host() {
            h.add_word_to_dictionary(Some(&CefString::from(word)));
        }
    }

    /// How many addresses were blocked since the page last navigated.
    pub fn blocked(&self) -> u32 {
        self.blocked.count()
    }

    /// How many page elements the filter lists' cosmetic rules hide (ad
    /// slots, cookie banners), in all its frames, since it last navigated.
    pub fn hidden(&self) -> u32 {
        self.shared.hide.hidden()
    }

    /// Whether Trailboard clicked a cookie banner's reject button on this
    /// page (in any of its frames) since it last navigated.
    pub fn banner_rejected(&self) -> bool {
        self.shared.banner_rejected.get()
    }

    pub fn key(&self) -> u64 {
        self.shared.key
    }

    pub fn frame(&self) -> std::cell::Ref<'_, PaintFrame> {
        self.shared.frame.borrow()
    }

    fn host(&self) -> Option<BrowserHost> {
        self.browser.host()
    }

    /// Resize the page viewport (CSS px) and/or change its raster scale (pixels per CSS px).
    pub fn resize(&self, size: (i32, i32), scale: f32) {
        let changed_size = *self.shared.size.borrow() != size;
        let changed_scale = (*self.shared.scale.borrow() - scale).abs() > f32::EPSILON;
        *self.shared.size.borrow_mut() = size;
        *self.shared.scale.borrow_mut() = scale;
        if changed_size || changed_scale {
            let mut recent = self.shared.recent.borrow_mut();
            recent.insert(0, (size, scale));
            recent.truncate(4);
        }
        if let Some(h) = self.host() {
            if changed_scale {
                self.shared.apply_zoom(&h);
            }
            if changed_size || changed_scale {
                h.was_resized();
            }
        }
    }

    /// The page zoom Chromium has, without the raster scale: the user's zoom once applied.
    pub fn zoom_factor(&self) -> f64 {
        let level = self.host().map_or(0.0, |h| h.zoom_level());
        1.2f64.powf(level) / *self.shared.scale.borrow() as f64
    }

    /// Chromium's default zoom as a factor over the raster scale (its default
    /// stays 100%, so this is 1 / scale).
    pub fn default_zoom_factor(&self) -> f64 {
        let level = self.host().map_or(0.0, |h| h.default_zoom_level());
        1.2f64.powf(level) / *self.shared.scale.borrow() as f64
    }

    /// Applies a changed user zoom ([`set_page_zoom`]); does nothing if it is unchanged.
    pub fn refresh_zoom(&self) {
        if let Some(h) = self.host() {
            self.shared.apply_zoom(&h);
        }
    }

    /// Finds `text` in the page (case-insensitive): a new search with
    /// `next == false`, otherwise the next (or previous) match of the same text.
    /// Results arrive as [`WebEvent::FindResult`].
    pub fn find(&self, text: &str, forward: bool, next: bool) {
        if let Some(h) = self.host() {
            if text.is_empty() {
                h.stop_finding(1);
            } else {
                h.find(Some(&CefString::from(text)), forward as _, 0, next as _);
            }
        }
    }

    /// Ends find in page, leaving the active match selected.
    pub fn stop_finding(&self) {
        if let Some(h) = self.host() {
            h.stop_finding(0);
        }
    }

    /// Ends find in page and clears its marks, the active match's selection
    /// included (Esc after a search). A search the find bar already ended
    /// left that match as the page's selection, so it is dropped too.
    pub fn clear_find(&self) {
        if let Some(h) = self.host() {
            h.stop_finding(1);
        }
        self.run_script("window.getSelection && window.getSelection().removeAllRanges();");
    }

    pub fn set_hidden(&self, hidden: bool) {
        if let Some(h) = self.host() {
            h.was_hidden(hidden as _);
        }
    }

    /// Gives the page its focus or takes it away. While the page holds its
    /// focus ([`Page::hold_focus`]) taking it away does nothing.
    pub fn set_focus(&self, focus: bool) {
        if !focus && self.shared.hold_focus.get() {
            return;
        }
        if let Some(h) = self.host() {
            h.set_focus(focus as _);
        }
    }

    /// Keeps the page focused (`on`) whichever card has the keys, as an
    /// *Always active* card does. Turning it off leaves the focus as it is.
    pub fn hold_focus(&self, on: bool) {
        self.shared.hold_focus.set(on);
        if on && let Some(h) = self.host() {
            h.set_focus(1);
        }
    }

    /// How often the page paints, frames a second (60 normally).
    pub fn set_frame_rate(&self, fps: i32) {
        if let Some(h) = self.host() {
            h.set_windowless_frame_rate(fps.clamp(1, 60));
        }
    }

    /// Whether any of the page's frames plays media ([`WebEvent::Media`]).
    pub fn playing(&self) -> bool {
        self.shared.media.borrow().playing()
    }

    pub fn set_muted(&self, muted: bool) {
        if let Some(h) = self.host() {
            h.set_audio_muted(muted as _);
        }
    }

    pub fn muted(&self) -> bool {
        self.host().is_some_and(|h| h.is_audio_muted() != 0)
    }

    /// Runs a media control (`op` with its number, see [`media::control`])
    /// in the frame whose media played or showed last. False when no frame
    /// has media. The answer comes as [`WebEvent::MediaDone`].
    ///
    /// The main frame gets it through the DevTools protocol as if from a
    /// click, so playing with sound and full screen are allowed there;
    /// other frames through plain script (a page may refuse those).
    pub fn media_control(&self, op: &str, value: f64, rates: &[u16]) -> bool {
        let Some(target) = self.shared.media.borrow().target().map(str::to_string) else { return false };
        let Some(script) = media::control(&target, op, value, rates) else { return false };
        let Some(host) = self.host() else { return false };
        let params = format!(r#"{{"expression":{},"userGesture":true}}"#, js_string(&script));
        let main_sent = send_dev_tools(&host, "Runtime.evaluate", &params);
        let main_id = self.browser.main_frame().map(|f| CefString::from(&f.identifier()).to_string());
        let mut ids = CefStringList::new();
        self.browser.frame_identifiers(Some(&mut ids));
        for id in ids.into_iter() {
            if Some(&id) == main_id.as_ref() && main_sent {
                continue;
            }
            if let Some(f) = self.browser.frame_by_identifier(Some(&CefString::from(id.as_str()))) {
                f.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
            }
        }
        true
    }

    /// The page is in full screen.
    pub fn fullscreen(&self) -> bool {
        self.shared.fullscreen.get()
    }

    /// Takes the page out of full screen.
    pub fn exit_fullscreen(&self) {
        if let Some(h) = self.host() {
            h.exit_fullscreen(1);
        }
    }

    pub fn load(&self, url: &str) {
        if let Some(f) = self.browser.main_frame() {
            f.load_url(Some(&CefString::from(url)));
        }
    }

    /// Downloads `url` with this page's cookies (the menu's *Download link*).
    /// It arrives as [`WebEvent::DownloadAsked`] like any other download.
    pub fn download(&self, url: &str) {
        if let Some(h) = self.host() {
            h.start_download(Some(&CefString::from(url)));
        }
    }

    pub fn back(&self) {
        self.browser.go_back();
    }

    /// Stops loading (`Ctrl+s`).
    pub fn stop(&self) {
        self.browser.stop_load();
    }

    pub fn forward(&self) {
        self.browser.go_forward();
    }

    pub fn reload(&self) {
        self.browser.reload();
    }

    /// Reloads without any cache, so every request goes to the network
    /// again (and past [`shield`]); a plain reload takes scripts and images
    /// from Blink's memory cache without asking.
    pub fn reload_fresh(&self) {
        self.browser.reload_ignore_cache();
    }

    pub fn mouse_move(&self, x: i32, y: i32, modifiers: u32, leave: bool) {
        let (x, y) = self.shared.widget_point(x, y);
        if let Some(h) = self.host() {
            h.send_mouse_move_event(Some(&MouseEvent { x, y, modifiers }), leave as _);
        }
    }

    pub fn mouse_button(&self, x: i32, y: i32, modifiers: u32, button: MouseButton, up: bool, clicks: i32) {
        let ty = match button {
            MouseButton::Left => MouseButtonType::LEFT,
            MouseButton::Middle => MouseButtonType::MIDDLE,
            MouseButton::Right => MouseButtonType::RIGHT,
        };
        let (x, y) = self.shared.widget_point(x, y);
        if let Some(h) = self.host() {
            h.send_mouse_click_event(Some(&MouseEvent { x, y, modifiers }), ty, up as _, clicks);
        }
    }

    /// Scrolls the page at view px (`x`, `y`) by view px. `precise` marks
    /// touchpad-like movement, which Chromium applies as is; otherwise it
    /// animates the scroll as for a mouse wheel notch.
    pub fn mouse_wheel(&self, x: i32, y: i32, modifiers: u32, dx: i32, dy: i32, precise: bool) {
        let (x, y) = self.shared.widget_point(x, y);
        let modifiers = if precise { modifiers | PRECISE_SCROLL } else { modifiers };
        // Deltas are view pixels; scale them so a notch scrolls the same CSS distance.
        let s = *self.shared.scale.borrow();
        let (dx, dy) = ((dx as f32 * s).round() as i32, (dy as f32 * s).round() as i32);
        if let Some(h) = self.host() {
            h.send_mouse_wheel_event(Some(&MouseEvent { x, y, modifiers }), dx, dy);
        }
    }

    /// Text selected in the page right now (empty if none).
    pub fn selected_text(&self) -> String {
        self.shared.selection.borrow().clone()
    }

    /// Show `text` as the IME's in-progress composition. `cursor` is the caret or
    /// selection inside it as byte offsets, as winit reports them.
    pub fn ime_set_composition(&self, text: &str, cursor: Option<(usize, usize)>) {
        let Some(h) = self.host() else { return };
        let len = utf16_len(text);
        let underline = CompositionUnderline {
            size: std::mem::size_of::<sys::cef_composition_underline_t>(),
            range: Range { from: 0, to: len },
            color: 0xff00_0000,
            background_color: 0,
            thick: 0,
            ..Default::default()
        };
        let sel = match cursor {
            Some((a, b)) => Range { from: utf16_len(&text[..a.min(text.len())]), to: utf16_len(&text[..b.min(text.len())]) },
            None => Range { from: len, to: len },
        };
        h.ime_set_composition(Some(&CefString::from(text)), Some(&[underline]), Some(&INVALID_RANGE), Some(&sel));
    }

    /// Insert the IME's final text, replacing any composition.
    pub fn ime_commit(&self, text: &str) {
        if let Some(h) = self.host() {
            h.ime_commit_text(Some(&CefString::from(text)), Some(&INVALID_RANGE), 0);
        }
    }

    pub fn ime_cancel(&self) {
        if let Some(h) = self.host() {
            h.ime_cancel_composition();
        }
    }

    /// Where the IME composition or caret is, in CSS px within the page.
    pub fn ime_caret(&self) -> Option<(f32, f32, f32, f32)> {
        let r = (*self.shared.ime_caret.borrow())?;
        let s = *self.shared.scale.borrow();
        Some((r.x as f32 / s, r.y as f32 / s, r.w as f32 / s, r.h as f32 / s))
    }

    fn edit_frame(&self) -> Option<Frame> {
        self.browser.focused_frame().or_else(|| self.browser.main_frame())
    }

    pub fn edit(&self, cmd: EditCommand) {
        let Some(f) = self.edit_frame() else { return };
        match cmd {
            EditCommand::Undo => f.undo(),
            EditCommand::Redo => f.redo(),
            EditCommand::Cut => f.cut(),
            EditCommand::Copy => f.copy(),
            EditCommand::Paste => f.paste(),
            EditCommand::SelectAll => f.select_all(),
        }
    }

    /// Paste `text` (from the system clipboard) into the focused field: the page
    /// gets a `paste` event first and can handle it; otherwise the text is inserted.
    pub fn paste_text(&self, text: &str) {
        let Some(f) = self.edit_frame() else { return };
        let script = format!(
            "(function(t){{var e=document.activeElement||document.body;var d=new DataTransfer();\
             d.setData('text/plain',t);var ev=new ClipboardEvent('paste',{{clipboardData:d,bubbles:true,\
             cancelable:true,composed:true}});if(e.dispatchEvent(ev))document.execCommand('insertText',false,t);}})({})",
            js_string(text)
        );
        f.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
    }

    /// Sends the focused text field's text as a [`WebEvent::Field`] report
    /// under `token`; returns the frame it runs in, for [`Page::write_field`].
    pub fn read_field(&self, token: u64) -> Option<String> {
        let f = self.edit_frame()?;
        f.execute_java_script(Some(&CefString::from(field::read(token).as_str())), None, 0);
        Some(CefString::from(&f.identifier()).to_string())
    }

    /// Puts `text` into the field [`Page::read_field`] read under `token` in `frame`.
    pub fn write_field(&self, frame: &str, token: u64, text: &str) -> bool {
        let Some(f) = self.browser.frame_by_identifier(Some(&CefString::from(frame))) else { return false };
        f.execute_java_script(Some(&CefString::from(field::write(token, text).as_str())), None, 0);
        true
    }

    /// Runs `script` in the page's main frame.
    pub fn run_script(&self, script: &str) {
        if let Some(f) = self.browser.main_frame() {
            f.execute_java_script(Some(&CefString::from(script)), None, 0);
        }
    }

    /// Focus the first visible text field (`gi`); the answer arrives as a
    /// [`WebEvent::Focus`] (a user's focus, or [`focus::Report::NoField`]).
    pub fn focus_first_field(&self) {
        self.run_script(&focus::focus_first_field());
    }

    /// Take the focus out of the focused field, in whichever frame has it.
    pub fn blur_field(&self) {
        if let Some(f) = self.edit_frame() {
            f.execute_java_script(Some(&CefString::from(focus::BLUR)), None, 0);
        }
    }

    /// Ask for the targets of `filter` visible in the page; they arrive as
    /// [`WebEvent::Links`].
    pub fn request_links(&self, filter: links::Filter) {
        self.run_script(&links::script(filter));
    }

    /// Ask for the page's next / previous links; they arrive as [`WebEvent::Pager`].
    pub fn request_pager(&self) {
        self.run_script(pager::SCRIPT);
    }

    /// Run a scroll section command ([`scroll::list`], [`scroll::by`]…); the
    /// answer arrives as [`WebEvent::Scroll`].
    pub fn scroll_command(&self, script: &str) {
        self.run_script(script);
    }

    /// Ask for the page's address, title and text; they arrive as [`WebEvent::PageText`].
    pub fn request_text(&self) {
        if self.shared.kind == PageKind::Web {
            self.run_script(text::SCRIPT);
        }
    }

    pub fn kind(&self) -> PageKind {
        self.shared.kind
    }

    pub fn send_key(&self, ev: &keys::Key) {
        if let Some(h) = self.host() {
            for e in ev.to_cef() {
                h.send_key_event(Some(&e));
            }
        }
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        // The bridge holds this page's browser: let it go first.
        self.shared.bridge.borrow_mut().take();
        if let Some(h) = self.host() {
            h.close_browser(1);
        }
    }
}

/// CEF's "no range" value.
pub(crate) const INVALID_RANGE: Range = Range { from: u32::MAX, to: u32::MAX };

pub(crate) fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// `s` as a JavaScript string literal.
/// `s` as a JavaScript string literal, safe inside a `<script>` element.
pub fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Line terminators that end a JS string, and anything that could close a <script>.
            '\u{2028}' | '\u{2029}' | '<' | '>' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// CEF `EVENTFLAG_*` bits.
pub mod flags {
    pub const SHIFT: u32 = 1 << 1;
    pub const CONTROL: u32 = 1 << 2;
    pub const ALT: u32 = 1 << 3;
    pub const LEFT_MOUSE: u32 = 1 << 4;
    pub const MIDDLE_MOUSE: u32 = 1 << 5;
    pub const RIGHT_MOUSE: u32 = 1 << 6;
}
