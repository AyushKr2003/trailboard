//! The CEF handlers of a page: paint, life span, requests, display, loads, downloads, menus, find, and the client that ties them together.

use std::cell::RefCell;
use std::rc::Rc as StdRc;
use std::sync::atomic::Ordering;

use cef::*;

use crate::popup::{self, IRect};
use crate::{APP_ACTION, caret, consent, cosmetic, Cursor, download, emit, field, focus, ICON_SIZE, inspector, links, media, menu, note_activity, NOTE_MESSAGE, NOTE_OPEN, PageKind, pager, PAINT_BYTES, paint_css, PaintFrame, PAINTS, PopupState, scroll, Shared, text, WebEvent, youtube};

wrap_render_handler! {
    pub(crate) struct PageRender {
        shared: StdRc<Shared>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let (w, h) = self.shared.view_px();
                rect.x = 0;
                rect.y = 0;
                rect.width = w.max(1);
                rect.height = h.max(1);
            }
        }

        fn screen_info(&self, _browser: Option<&mut Browser>, info: Option<&mut ScreenInfo>) -> ::std::os::raw::c_int {
            match info {
                Some(info) => {
                    // Scale comes from page zoom (see `Shared::scale`), so the "screen" is 1×.
                    info.device_scale_factor = 1.0;
                    let (w, h) = self.shared.view_px();
                    let r = Rect { x: 0, y: 0, width: w, height: h };
                    info.rect = r.clone();
                    info.available_rect = r;
                    1
                }
                None => 0,
            }
        }

        fn on_text_selection_changed(
            &self,
            _browser: Option<&mut Browser>,
            selected_text: Option<&CefString>,
            _selected_range: Option<&Range>,
        ) {
            let text = selected_text.map(|t| t.to_string()).unwrap_or_default();
            log::trace!("page {} selection {} chars", self.shared.key, text.chars().count());
            *self.shared.selection.borrow_mut() = text;
        }

        fn on_ime_composition_range_changed(
            &self,
            _browser: Option<&mut Browser>,
            _selected_range: Option<&Range>,
            character_bounds: Option<&[Rect]>,
        ) {
            let rects: Vec<IRect> = character_bounds
                .unwrap_or_default()
                .iter()
                .map(|r| IRect { x: r.x, y: r.y, w: r.width, h: r.height })
                .collect();
            *self.shared.ime_caret.borrow_mut() = popup::bounding(&rects);
        }

        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            let sh = &self.shared;
            log::debug!("page {} popup show={show}", sh.key);
            if show != 0 {
                sh.popup.borrow_mut().shown = true;
                if sh.view.borrow().is_none() {
                    let f = sh.frame.borrow();
                    *sh.view.borrow_mut() =
                        Some(PaintFrame { width: f.width, height: f.height, pixels: f.pixels.clone(), serial: 0, css: f.css, damage: Vec::new() });
                }
            } else {
                *sh.popup.borrow_mut() = PopupState::default();
                // Restore the page pixels the popup covered.
                if let Some(view) = sh.view.borrow_mut().take() {
                    let mut f = sh.frame.borrow_mut();
                    f.width = view.width;
                    f.height = view.height;
                    f.css = view.css;
                    f.pixels = view.pixels;
                    f.bump(None);
                    note_activity();
                }
            }
        }

        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&Rect>) {
            let Some(r) = rect else { return };
            log::debug!("page {} popup rect {},{} {}x{}", self.shared.key, r.x, r.y, r.width, r.height);
            self.shared.popup.borrow_mut().rect = IRect { x: r.x, y: r.y, w: r.width, h: r.height };
            self.shared.compose();
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let len = width as usize * height as usize * 4;
            // SAFETY: CEF guarantees `buffer` holds width*height BGRA pixels for this call.
            let src = unsafe { std::slice::from_raw_parts(buffer, len) };
            PAINTS.fetch_add(1, Ordering::Relaxed);
            let sh = &self.shared;
            log::trace!("page {} paint {width}x{height} popup={}", sh.key, type_ == PaintElementType::POPUP);
            if type_ != PaintElementType::POPUP && sh.frame.borrow().width != width as u32 {
                log::debug!("page {} paints at {width}x{height} (scale {})", sh.key, sh.scale.borrow());
            }
            let css = paint_css((width as u32, height as u32), &sh.recent.borrow());
            // Only what Chromium says it repainted changed, unless the size did.
            let dirty: Vec<IRect> =
                dirty_rects.unwrap_or_default().iter().map(|r| IRect { x: r.x, y: r.y, w: r.width, h: r.height }).collect();
            let store = |f: &mut PaintFrame| {
                let resized = (f.width, f.height) != (width as u32, height as u32);
                f.width = width as u32;
                f.height = height as u32;
                f.css = css;
                // The rest of the frame already holds these pixels.
                let n = popup::copy_dirty(&mut f.pixels, src, f.width, f.height, !resized, &dirty);
                PAINT_BYTES.fetch_add(n as u64, Ordering::Relaxed);
                f.bump(if resized { None } else { popup::bounding(&dirty) });
                note_activity();
            };
            if type_ == PaintElementType::POPUP {
                let mut p = sh.popup.borrow_mut();
                let same = (p.width, p.height) == (width as u32, height as u32);
                p.width = width as u32;
                p.height = height as u32;
                let n = popup::copy_dirty(&mut p.pixels, src, width as u32, height as u32, same, &dirty);
                PAINT_BYTES.fetch_add(n as u64, Ordering::Relaxed);
            } else {
                let mut view = sh.view.borrow_mut();
                match view.as_mut() {
                    Some(v) => store(v),
                    // No popup: write straight into the shown frame.
                    None => return store(&mut sh.frame.borrow_mut()),
                }
            }
            sh.compose();
        }
    }
}

wrap_life_span_handler! {
    pub(crate) struct PageLife {
        shared: StdRc<Shared>,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            if let Some(url) = target_url.map(|u| u.to_string()).filter(|u| !u.is_empty()) {
                emit(WebEvent::OpenBeside { source: self.shared.key, url });
            }
            1 // cancel the native popup; the app opens a card instead
        }
    }
}

wrap_request_handler! {
    pub(crate) struct PageRequests {
        shared: StdRc<Shared>,
        spawn_beside: StdRc<RefCell<bool>>,
        // `resource_request_handler` runs on the IO thread: it may only touch
        // this (thread-safe) handler, never `shared`.
        guard: Option<ResourceRequestHandler>,
    }

    impl RequestHandler {
        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _is_navigation: ::std::os::raw::c_int,
            _is_download: ::std::os::raw::c_int,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut ::std::os::raw::c_int>,
        ) -> Option<ResourceRequestHandler> {
            self.guard.clone()
        }

        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: ::std::os::raw::c_int,
            is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // JavaScript on or off as the site it goes to has it, before it loads.
            if frame.as_ref().is_some_and(|f| f.is_main() != 0)
                && let (Some(host), Some(r)) = (browser.and_then(|b| b.host()), request.as_ref())
            {
                let url = CefString::from(&r.url()).to_string();
                self.shared.apply_javascript(&host, &url);
                // The site's scriptlets, in place before its first script runs.
                if self.shared.kind == PageKind::Web {
                    self.shared.early.before_navigation(&host, &url);
                }
            }
            // "Open links beside the page": a user-clicked link in the main frame
            // becomes a new card next to this one instead of replacing it.
            log::debug!(
                "before_browse page={} gesture={} redirect={} tt={:?} url={:?}",
                self.shared.key,
                user_gesture,
                is_redirect,
                request.as_ref().map(|r| r.transition_type().get_raw()),
                request.as_ref().map(|r| CefString::from(&r.url()).to_string().chars().take(200).collect::<String>())
            );
            if matches!(self.shared.kind, PageKind::Note | PageKind::App) {
                // A note stays a note (an app page, itself): only its own page loads in it; a link
                // the user follows opens beside, anything else is dropped.
                let url = request.as_ref().map(|r| CefString::from(&r.url()).to_string()).unwrap_or_default();
                if url == self.shared.initial_url {
                    return 0;
                }
                if user_gesture != 0 && !url.is_empty() && !url.starts_with("javascript:") {
                    emit(WebEvent::OpenBeside { source: self.shared.key, url });
                }
                return 1;
            }
            if !*self.spawn_beside.borrow() || user_gesture == 0 || is_redirect != 0 {
                return 0;
            }
            let (Some(frame), Some(request)) = (frame, request) else { return 0 };
            if frame.is_main() == 0 {
                return 0;
            }
            let tt = request.transition_type();
            let source = tt.get_raw() & TransitionType::SOURCE_MASK.get_raw();
            if source != TransitionType::LINK.get_raw() {
                return 0;
            }
            let url = CefString::from(&request.url()).to_string();
            if url.is_empty() || url == self.shared.initial_url || url.starts_with("javascript:") {
                return 0;
            }
            emit(WebEvent::OpenBeside { source: self.shared.key, url });
            1
        }

        fn on_open_urlfrom_tab(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            target_url: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // Ctrl/middle-click: always a new card beside.
            if let Some(url) = target_url.map(|u| u.to_string()).filter(|u| !u.is_empty()) {
                emit(WebEvent::OpenBeside { source: self.shared.key, url });
            }
            1
        }
    }
}

wrap_display_handler! {
    pub(crate) struct PageDisplay {
        shared: StdRc<Shared>,
    }

    impl DisplayHandler {
        fn on_address_change(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if frame.is_some_and(|f| f.is_main() != 0)
                && self.shared.kind == PageKind::Web
                && let Some(url) = url {
                    *self.shared.url.borrow_mut() = url.to_string();
                    if let Some(h) = browser.and_then(|b| b.host()) {
                        self.shared.apply_zoom(&h);
                    }
                    emit(WebEvent::UrlChanged { page: self.shared.key, url: url.to_string() });
                }
        }

        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: ::std::os::raw::c_ulong,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            emit(WebEvent::CursorChanged { page: self.shared.key, cursor: Cursor::from_cef(type_) });
            1
        }

        fn on_favicon_urlchange(&self, browser: Option<&mut Browser>, icon_urls: Option<&mut CefStringList>) {
            if self.shared.kind != PageKind::Web {
                return;
            }
            // Chromium lists the page's icons best first, ending with /favicon.ico.
            let Some(first) = icon_urls.and_then(|l| {
                let raw: *mut cef::sys::_cef_string_list_t = l.into();
                CefStringList::from(raw).into_iter().next()
            }) else {
                return;
            };
            if *self.shared.icon_url.borrow() == first {
                return;
            }
            *self.shared.icon_url.borrow_mut() = first.clone();
            let Some(host) = browser.and_then(|b| b.host()) else { return };
            let page_url = self.shared.url.borrow().clone();
            let mut done = IconDone::new(self.shared.key, page_url);
            host.download_image(Some(&CefString::from(first.as_str())), 1, ICON_SIZE, 0, Some(&mut done));
        }

        fn on_fullscreen_mode_change(&self, _browser: Option<&mut Browser>, fullscreen: ::std::os::raw::c_int) {
            let on = fullscreen != 0;
            self.shared.fullscreen.set(on);
            emit(WebEvent::Fullscreen { page: self.shared.key, on });
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if self.shared.kind == PageKind::Web
                && let Some(title) = title {
                    emit(WebEvent::TitleChanged { page: self.shared.key, title: title.to_string() });
                }
        }

        fn on_console_message(
            &self,
            browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(message) = message.map(|m| m.to_string()) else { return 0 };
            if let Some(body) = message.strip_prefix(cosmetic::PREFIX) {
                if self.shared.kind == PageKind::Web
                    && let Some(browser) = browser
                {
                    cosmetic::answer(&self.shared.hide, browser, body);
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(cosmetic::COUNT_PREFIX) {
                self.shared.hide.on_count(body);
                return 1;
            }
            if let Some(body) = message.strip_prefix(consent::PREFIX) {
                if let Some((platform, what)) = consent::parse(body) {
                    let url = self.shared.url.borrow();
                    log::info!("consent: {platform} {what} on {}", url.chars().take(100).collect::<String>());
                    if what == "rejected" {
                        self.shared.banner_rejected.set(true);
                    }
                }
                return 1;
            }
            if let Some(what) = message.strip_prefix(youtube::PREFIX) {
                let url = self.shared.url.borrow();
                log::info!("youtube: {what} on {}", url.chars().take(100).collect::<String>());
                return 1;
            }
            if let Some(body) = message.strip_prefix(media::PREFIX) {
                if self.shared.kind == PageKind::Web
                    && let Some((token, state)) = media::parse(body)
                    && let Some(playing) = self.shared.media.borrow_mut().report(token, state)
                {
                    emit(WebEvent::Media { page: self.shared.key, playing });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(media::DONE) {
                if self.shared.kind == PageKind::Web && body.len() <= 120 {
                    emit(WebEvent::MediaDone { page: self.shared.key, answer: body.to_string() });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(media::TROUBLE) {
                if self.shared.kind == PageKind::Web && matches!(body, "format" | "drm") {
                    emit(WebEvent::MediaTrouble { page: self.shared.key, kind: body.to_string() });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(focus::PREFIX) {
                if let Some(report) = focus::parse(body) {
                    emit(WebEvent::Focus { page: self.shared.key, report });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(scroll::PREFIX) {
                if let Some(report) = scroll::parse(body) {
                    emit(WebEvent::Scroll { page: self.shared.key, report });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(caret::PREFIX) {
                log::trace!("caret: {}", body.chars().take(80).collect::<String>().replace('\n', " | "));
                if let Some(report) = caret::parse(body) {
                    emit(WebEvent::Caret { page: self.shared.key, report });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(field::PREFIX) {
                if let Some(report) = field::parse(body) {
                    emit(WebEvent::Field { page: self.shared.key, report });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(pager::PREFIX) {
                emit(WebEvent::Pager { page: self.shared.key, links: pager::parse(body) });
                return 1;
            }
            if let Some(body) = message.strip_prefix(links::PREFIX) {
                if let Some((width, links)) = links::parse(body) {
                    emit(WebEvent::Links { page: self.shared.key, width, links });
                }
                return 1;
            }
            if let Some(body) = message.strip_prefix(text::PREFIX) {
                if self.shared.kind == PageKind::Web
                    && let Some((url, title, body)) = text::parse(body)
                    && *self.shared.url.borrow() == url
                {
                    emit(WebEvent::PageText {
                        page: self.shared.key,
                        url: url.to_string(),
                        title: title.to_string(),
                        text: body.to_string(),
                    });
                }
                return 1;
            }
            if self.shared.kind == PageKind::Inspector {
                let Some(body) = message.strip_prefix(inspector::PREFIX) else { return 0 };
                if let Some(b) = self.shared.bridge.borrow().as_ref() {
                    b.on_frontend_message(body);
                }
                return 1;
            }
            if self.shared.kind == PageKind::App {
                if let Some(action) = message.strip_prefix(APP_ACTION) {
                    emit(WebEvent::AppAction { page: self.shared.key, action: action.to_string() });
                    return 1;
                }
                return 0;
            }
            if self.shared.kind != PageKind::Note {
                return 0;
            }
            if let Some(text) = message.strip_prefix(NOTE_MESSAGE) {
                emit(WebEvent::NoteText { page: self.shared.key, text: text.to_string() });
                return 1; // not worth logging every keystroke
            }
            if let Some(url) = message.strip_prefix(NOTE_OPEN).filter(|u| !u.is_empty()) {
                emit(WebEvent::OpenBeside { source: self.shared.key, url: url.to_string() });
                return 1;
            }
            0
        }
    }
}

wrap_download_image_callback! {
    pub(crate) struct IconDone {
        key: u64,
        page_url: String,
    }

    impl DownloadImageCallback {
        fn on_download_image_finished(&self, image_url: Option<&CefString>, status: ::std::os::raw::c_int, image: Option<&mut Image>) {
            let url = image_url.map(|u| u.to_string()).unwrap_or_default();
            let Some(image) = image else {
                log::debug!("page {}: no icon at {url:?} (HTTP {status})", self.key);
                return;
            };
            let (mut w, mut h) = (0, 0);
            let Some(bin) = image.as_bitmap(1.0, ColorType::BGRA_8888, AlphaType::POSTMULTIPLIED, Some(&mut w), Some(&mut h)) else {
                return;
            };
            let (width, height) = (w.max(0) as u32, h.max(0) as u32);
            let mut pixels = vec![0u8; bin.size()];
            bin.data(Some(&mut pixels), 0);
            if width == 0 || height == 0 || pixels.len() != (width * height * 4) as usize {
                return;
            }
            log::debug!("page {}: icon {width}x{height} from {url:?}", self.key);
            emit(WebEvent::Favicon { page: self.key, page_url: self.page_url.clone(), width, height, pixels });
        }
    }
}

wrap_load_handler! {
    pub(crate) struct PageLoad {
        shared: StdRc<Shared>,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            browser: Option<&mut Browser>,
            is_loading: ::std::os::raw::c_int,
            can_go_back: ::std::os::raw::c_int,
            can_go_forward: ::std::os::raw::c_int,
        ) {
            if let Some(h) = browser.and_then(|b| b.host()) {
                self.shared.apply_zoom(&h);
            }
            emit(WebEvent::LoadingChanged { page: self.shared.key, loading: is_loading != 0, can_back: can_go_back != 0, can_forward: can_go_forward != 0 });
        }

        fn on_load_start(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, _transition_type: TransitionType) {
            let Some(frame) = frame else { return };
            if frame.is_main() != 0 && self.shared.kind == PageKind::Inspector {
                frame.execute_java_script(Some(&CefString::from(inspector::SHIM)), None, 0);
            }
            if frame.is_main() != 0 {
                self.shared.committed.set(true);
                self.shared.banner_rejected.set(false);
                let cleared = self.shared.media.borrow_mut().clear();
                if let Some(playing) = cleared {
                    emit(WebEvent::Media { page: self.shared.key, playing });
                }
                // Chromium drops a zoom set before the page commits. Web pages
                // get it again when their address changes; app pages (the
                // start page) would otherwise wait until they finish loading,
                // laid out at the wrong size until then.
                if let Some(h) = browser.as_ref().and_then(|b| b.host()) {
                    self.shared.apply_zoom(&h);
                }
            }
            // Focus reports for Insert mode, in every frame of pages you browse
            // or use (notes take their keys another way).
            if self.shared.kind != PageKind::Note {
                frame.execute_java_script(Some(&CefString::from(focus::script().as_str())), None, 0);
            }
            // Media reports, so a card playing something stays awake.
            if self.shared.kind == PageKind::Web {
                frame.execute_java_script(Some(&CefString::from(media::script().as_str())), None, 0);
            }
            // Link hints inside frames: an idle listener in each frame below
            // the main one answers the main frame's hint script.
            if self.shared.kind == PageKind::Web && frame.is_main() == 0 {
                frame.execute_java_script(Some(&CefString::from(links::frame_script())), None, 0);
            }
            if self.shared.kind == PageKind::Web
                && let Some(browser) = browser
            {
                cosmetic::on_load_start(&self.shared.hide, browser, frame);
            }
        }
    }
}

wrap_download_handler! {
    pub(crate) struct PageDownloads {
        shared: StdRc<Shared>,
    }

    impl DownloadHandler {
        fn can_download(&self, _browser: Option<&mut Browser>, _url: Option<&CefString>, _method: Option<&CefString>) -> ::std::os::raw::c_int {
            // Notes never download; web pages may (the app picks the folder).
            (self.shared.kind == PageKind::Web) as ::std::os::raw::c_int
        }

        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> ::std::os::raw::c_int {
            let (Some(item), Some(callback)) = (download_item, callback) else { return 0 };
            let id = item.id();
            let url = CefString::from(&item.url()).to_string();
            let suggested = suggested_name.map(|n| n.to_string()).unwrap_or_default();
            download::asked(id, callback.clone());
            let blank = !self.shared.committed.get();
            log::info!("page {}: download {id} of {url:?} as {suggested:?} (blank page: {blank})", self.shared.key);
            emit(WebEvent::DownloadAsked { page: self.shared.key, id, url, suggested, blank });
            1 // the app answers with download::save_to
        }

        fn on_download_updated(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            callback: Option<&mut DownloadItemCallback>,
        ) {
            let Some(item) = download_item else { return };
            if let Some(update) = download::updated(item, callback) {
                emit(update);
            }
        }
    }
}

wrap_context_menu_handler! {
    pub(crate) struct PageMenu {
        shared: StdRc<Shared>,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            // An empty model makes CEF skip its own menu; the app draws one instead.
            if let Some(m) = model {
                m.clear();
            }
            // The inspector draws its own menus in the page.
            if self.shared.kind == PageKind::Inspector {
                return;
            }
            let Some(p) = params else { return };
            let edit = sys::cef_context_menu_edit_state_flags_t::from(p.edit_state_flags()).0;
            let has = |f: sys::cef_context_menu_edit_state_flags_t| edit & f.0 != 0;
            let non_empty = |s: String| Some(s).filter(|s| !s.is_empty());
            let image = if p.media_type() == ContextMenuMediaType::IMAGE {
                non_empty(CefString::from(&p.source_url()).to_string())
            } else {
                None
            };
            let context = menu::MenuContext {
                link: non_empty(CefString::from(&p.link_url()).to_string()),
                image,
                selection: CefString::from(&p.selection_text()).to_string(),
                editable: p.is_editable() != 0,
                can_undo: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_UNDO),
                can_redo: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_REDO),
                can_cut: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_CUT),
                can_copy: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_COPY),
                can_paste: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_PASTE),
                can_select_all: has(sys::cef_context_menu_edit_state_flags_t::CM_EDITFLAG_CAN_SELECT_ALL),
                can_back: browser.as_ref().is_some_and(|b| b.can_go_back() != 0),
                can_forward: browser.as_ref().is_some_and(|b| b.can_go_forward() != 0),
                note: self.shared.kind == PageKind::Note,
                sticky: false,
                blocking: None,
                misspelled: CefString::from(&p.misspelled_word()).to_string(),
                suggestions: {
                    let mut list = CefStringList::new();
                    p.dictionary_suggestions(Some(&mut list));
                    list.into_iter().collect()
                },
                site: None,
                inspect: None,
            };
            // Params are view px; the app works in CSS px.
            let s = *self.shared.scale.borrow();
            let (x, y) = (p.xcoord() as f32 / s, p.ycoord() as f32 / s);
            let context = menu::MenuContext {
                inspect: (self.shared.kind == PageKind::Web).then_some((x.round() as i32, y.round() as i32)),
                ..context
            };
            log::debug!("page {} context menu at {x:.0},{y:.0}: {context:?}", self.shared.key);
            emit(WebEvent::ContextMenu { page: self.shared.key, x, y, context });
        }
    }
}

wrap_find_handler! {
    pub(crate) struct PageFind {
        shared: StdRc<Shared>,
    }

    impl FindHandler {
        fn on_find_result(
            &self,
            _browser: Option<&mut Browser>,
            _identifier: ::std::os::raw::c_int,
            count: ::std::os::raw::c_int,
            _selection_rect: Option<&Rect>,
            active_match_ordinal: ::std::os::raw::c_int,
            final_update: ::std::os::raw::c_int,
        ) {
            emit(WebEvent::FindResult {
                page: self.shared.key,
                count,
                active: active_match_ordinal,
                done: final_update != 0,
            });
        }
    }
}

// One handler per CEF callback group makes `PageClient::new` take eleven.
#[allow(clippy::too_many_arguments)]
mod client {
use super::*;
wrap_client! {
    pub(crate) struct PageClient {
        render: RenderHandler,
        life: LifeSpanHandler,
        requests: RequestHandler,
        display: DisplayHandler,
        load: LoadHandler,
        menu: ContextMenuHandler,
        find: FindHandler,
        downloads: DownloadHandler,
        permissions: PermissionHandler,
        dialogs: JsdialogHandler,
        files: DialogHandler,
    }

    impl Client {
        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(self.permissions.clone())
        }
        fn jsdialog_handler(&self) -> Option<JsdialogHandler> {
            Some(self.dialogs.clone())
        }
        fn dialog_handler(&self) -> Option<DialogHandler> {
            Some(self.files.clone())
        }
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render.clone())
        }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life.clone())
        }
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(self.requests.clone())
        }
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }
        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load.clone())
        }
        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(self.menu.clone())
        }
        fn find_handler(&self) -> Option<FindHandler> {
            Some(self.find.clone())
        }
        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(self.downloads.clone())
        }
    }
}
}
pub(crate) use client::PageClient;
