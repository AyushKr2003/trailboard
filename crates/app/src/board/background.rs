//! Board background.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use scene::pagezoom;
use scene::settings::{self as prefs, Settings};
use scene::{NodeId, placement};

use crate::app::{App, PageKeys, system_locale};
use crate::ui::omnibox::{Omnibox, Target};

impl App {
    /// The picture the board shows now, if *Board background* is *Photo*:
    /// the space's own, else the desktop's wallpaper.
    pub(crate) fn board_photo(&self) -> Option<PathBuf> {
        if self.settings.backdrop != prefs::Backdrop::Photo {
            return None;
        }
        self.spaces.active_entry().photo.as_ref().map(PathBuf::from).or_else(|| self.wallpaper.clone())
    }

    /// Looks the desktop's wallpaper up again (it changes without the theme
    /// changing), at most every two seconds unless `now`.
    pub(crate) fn check_wallpaper(&mut self, now: bool) {
        if !now && self.wallpaper_checked.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return;
        }
        self.wallpaper_checked = Some(Instant::now());
        let w = crate::board::backdrop::desktop_wallpaper();
        if w != self.wallpaper {
            log::info!("desktop wallpaper: {w:?}");
            self.wallpaper = w;
        }
    }

    /// *Board background* changed in the settings.
    pub(crate) fn backdrop_changed(&mut self) {
        self.check_wallpaper(true);
        self.photo.retry();
        if self.settings.backdrop == prefs::Backdrop::Photo && self.board_photo().is_none() {
            self.toast("No desktop wallpaper found · pick a photo from the space's menu");
        }
    }

    /// *Background photo…*: the list of pictures to show behind this space.
    pub(crate) fn open_photo_list(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.check_wallpaper(true);
        self.photo_files = storage::photo::pictures(&crate::board::backdrop::picture_dirs(), 400);
        self.omnibox = Some(Omnibox::new("", Target::Photo));
        log::info!("photo list open: {} pictures, wallpaper {:?}", self.photo_files.len(), self.wallpaper);
    }

    /// The space's background becomes `path` (`None`: the desktop's
    /// wallpaper), and the board shows photos if it didn't.
    pub(crate) fn set_space_photo(&mut self, path: Option<String>) {
        let id = self.spaces.active;
        let name = path.as_deref().map_or_else(
            || "desktop wallpaper".to_string(),
            |p| Path::new(p).file_stem().map_or_else(|| p.to_string(), |s| s.to_string_lossy().to_string()),
        );
        self.spaces.set_photo(id, path);
        self.save_index();
        self.photo.retry();
        if self.settings.backdrop != prefs::Backdrop::Photo {
            self.choose_setting(prefs::Key::Backdrop, prefs::Backdrop::Photo.id());
        }
        log::info!("space {} photo {:?}", id.0, self.spaces.active_entry().photo);
        self.toast(format!("Background: {name}"));
    }

    /// The list of sites where ads are allowed, from the palette.
    pub(crate) fn open_allowed_sites(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::AllowedSites));
        log::info!("allowed sites open: {:?}", self.blocking.allowed_sites());
    }

    /// Blocks ads on `site` again, from the allowed-sites list (which stays
    /// open, its highlight kept in range).
    pub(crate) fn block_site_again(&mut self, site: &str) {
        if !self.blocking.block_again(site) {
            return;
        }
        self.reload_site(site, true);
        self.toast(format!("Blocking ads on {site} again"));
        let len = self.omnibox.as_ref().map_or(0, |ob| self.list_rows(ob).len());
        if let Some(ob) = self.omnibox.as_mut() {
            ob.sel = ob.sel.min(len.saturating_sub(1));
        }
    }

    /// Ctrl+,: the settings page. Go to it if it is on the board (unfolding
    /// it if needed), else add one in the middle of the view.
    pub(crate) fn open_settings(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.omnibox = None;
        self.finish_arrange(true);
        if let Some(id) = self.scene.settings_card() {
            log::info!("settings page {} already on the board", id.0);
            self.go_to_card(id);
            return;
        }
        let rect = placement::free_near(&self.scene, self.camera.center, self.card_size.x, self.card_size.y);
        let id = self.scene.add_settings(rect);
        self.create_page(id);
        self.history.opened(id);
        self.dirty = true;
        self.focus(Some(id));
        self.ensure_visible(id);
        log::info!("settings page {} added", id.0);
    }

    /// The settings page's data: the settings as in force, where the number
    /// of live pages comes from, and the sandbox.
    pub(crate) fn settings_data(&self) -> String {
        let shown = Settings {
            live_pages: self.max_live,
            ..self.settings.clone()
        };
        let mut notes = Vec::new();
        if self.live_from_env {
            notes.push((prefs::Key::LivePages, "TRAILBOARD_MAX_LIVE".to_string()));
        }
        let facts: Vec<(&str, String, String)> = self
            .sandbox
            .iter()
            .map(|s| ("Sandbox", s.summary().to_string(), s.explain().to_string()))
            .collect();
        crate::builtin::settings::data(&shown, &notes, &facts)
    }

    /// Shows live settings pages the settings as they are now.
    pub(crate) fn settings_changed(&self) {
        let script = crate::builtin::settings::data_script(&self.settings_data());
        for (id, card) in &self.cards {
            let ours = self
                .scene
                .node(*id)
                .is_some_and(|n| !n.is_note() && n.url == scene::note::SETTINGS_URL);
            if ours && card.page.kind() == web::PageKind::App {
                card.page.run_script(&script);
            }
        }
    }

    /// A message from the settings page (card `id`). Returns whether it was one of its.
    pub(crate) fn settings_page_action(&mut self, id: NodeId, action: &str) -> bool {
        use crate::builtin::settings as sp;
        if !self
            .scene
            .node(id)
            .is_some_and(|n| !n.is_note() && n.url == scene::note::SETTINGS_URL)
        {
            return false;
        }
        if let Some(msg) = action.strip_prefix(sp::SET) {
            match sp::parse_set(msg) {
                Some((key, value)) => self.choose_setting(key, value),
                None => log::warn!("settings page: unknown setting {msg:?}"),
            }
        } else if let Some(server) = action.strip_prefix(sp::PROXY) {
            self.set_proxy_server(server);
        } else if let Some(px) = action.strip_prefix(sp::CORNER_RADIUS) {
            self.set_corner_radius(px);
        } else if let Some(msg) = action.strip_prefix(sp::KEYWORD_SET) {
            match sp::parse_keyword(msg) {
                Some((key, name, url)) => self.set_keyword(key, name, url),
                None => log::warn!("settings page: bad keyword {msg:?}"),
            }
        } else if let Some(key) = action.strip_prefix(sp::KEYWORD_REMOVE) {
            self.remove_keyword(key);
        } else if action == sp::KEYWORD_RESET {
            self.settings.keywords = scene::search::defaults();
            log::info!("setting keywords = defaults");
            self.toast("Search keywords back to the defaults");
            self.save_settings();
        } else if let Some(name) = action.strip_prefix(sp::RUN) {
            match sp::command(name) {
                Some(c) => {
                    self.keys_in_page = false; // the board's own UI takes over
                    self.run_command(c);
                }
                None => log::warn!("settings page: unknown command {name:?}"),
            }
        } else if action == crate::builtin::start::GUIDE {
            self.show_engines(id);
        } else {
            return false;
        }
        true
    }

    /// Sets `key` to the choice `value` (`Settings::choices`), applies it at
    /// once and saves it.
    pub(crate) fn choose_setting(&mut self, key: prefs::Key, value: &str) {
        let mut s = self.settings.clone();
        if key == prefs::Key::LivePages {
            s.live_pages = self.max_live; // compare with what is in force
        }
        if !s.choose(key, value) {
            self.settings_changed(); // the page shows what is really set
            return;
        }
        let before = std::mem::replace(&mut self.settings, s);
        log::info!("setting {key:?} = {}", self.settings.value(key));
        match key {
            prefs::Key::Search => self.engine_changed(),
            prefs::Key::NewCard | prefs::Key::ScrollSpeed | prefs::Key::DownloadTo | prefs::Key::SpacePanel | prefs::Key::ModeLabel => {}
            prefs::Key::Corners => self.set_theme(self.theme.clone()),
            prefs::Key::Backdrop => self.backdrop_changed(),
            prefs::Key::LivePages => {
                self.max_live = self.settings.live_pages;
                self.live_from_env = false;
            }
            prefs::Key::History => {
                if !self.settings.history {
                    self.text_due.clear();
                }
            }
            prefs::Key::PageColours => self.apply_page_scheme(),
            prefs::Key::BlockAds => web::shield::set_enabled(self.settings.block_ads),
            prefs::Key::DefaultZoom => {
                self.zooms.borrow_mut().default = self.settings.default_zoom_factor();
                for c in self.cards.values() {
                    c.page.refresh_zoom();
                }
            }
            prefs::Key::Spellcheck | prefs::Key::ThirdPartyCookies | prefs::Key::DoNotTrack => self.apply_browsing(),
            prefs::Key::Language => {
                // Spelling follows at once; Chromium's network stack only
                // reads the languages sites are told when it starts.
                self.apply_browsing();
                let name = self.settings.value(key);
                self.toast(format!("Page language: {name} · sites see it after a restart"));
            }
            prefs::Key::Proxy => {
                self.apply_browsing();
                if self.settings.proxy == prefs::Proxy::Custom
                    && self.settings.proxy_server.is_empty()
                {
                    self.toast("Type the proxy server under Proxy");
                }
            }
            prefs::Key::JavaScript | prefs::Key::Images => self.site_content_changed(&before),
            prefs::Key::Camera | prefs::Key::Microphone | prefs::Key::Location | prefs::Key::Notifications => {
                self.apply_permissions();
            }
        }
        self.save_settings();
    }

    /// The settings as in force (the number of live pages may come from
    /// `TRAILBOARD_MAX_LIVE`).
    pub(crate) fn settings_in_force(&self) -> Settings {
        Settings { live_pages: self.max_live, ..self.settings.clone() }
    }

    /// `:set name [value]`: with a value, sets it; without, shows it.
    pub(crate) fn run_set(&mut self, arg: &str) {
        let (name, value) = arg.split_once(char::is_whitespace).map_or((arg, ""), |(n, v)| (n, v.trim()));
        let setting = match scene::setcmd::find(name) {
            Ok(x) => x,
            Err(e) => return self.toast(format!("set: {e}")),
        };
        if value.is_empty() && setting != scene::setcmd::Setting::ProxyServer {
            let shown = setting.shown(&self.settings_in_force());
            log::info!("set: {shown}");
            return self.toast(shown);
        }
        match scene::setcmd::set(&self.settings_in_force(), setting, value) {
            Ok(change) => self.apply_setting_change(setting, change),
            Err(e) => self.toast(e),
        }
    }

    /// `:toggle name [values…]`.
    pub(crate) fn run_toggle(&mut self, arg: &str) {
        let mut words = arg.split_whitespace();
        let setting = match scene::setcmd::find(words.next().unwrap_or("")) {
            Ok(x) => x,
            Err(e) => return self.toast(format!("toggle: {e}")),
        };
        let values: Vec<&str> = words.collect();
        match scene::setcmd::toggle(&self.settings_in_force(), setting, &values) {
            Ok(change) => self.apply_setting_change(setting, change),
            Err(e) => self.toast(e),
        }
    }

    /// Makes a change from the command line or a `t` key, as the settings
    /// page would, and says what the setting is now.
    pub(crate) fn apply_setting_change(&mut self, setting: scene::setcmd::Setting, change: scene::setcmd::Change) {
        use scene::setcmd::Change;
        log::info!("setting change: {change:?}");
        match change {
            Change::Choose(key, id) => {
                self.choose_setting(key, &id);
                if key == prefs::Key::Language {
                    return; // `choose_setting` says it, with when sites see it
                }
            }
            Change::ProxyServer(server) => {
                self.set_proxy_server(&server);
                if !server.is_empty() && self.settings.proxy != prefs::Proxy::Custom {
                    self.choose_setting(prefs::Key::Proxy, prefs::Proxy::Custom.id());
                }
                return; // `set_proxy_server` says it
            }
            Change::CornerRadius(px) => {
                self.set_corner_radius(&px.to_string());
                if self.settings.corners != prefs::Corners::Custom {
                    self.choose_setting(prefs::Key::Corners, prefs::Corners::Custom.id());
                }
            }
            Change::DefaultKeywords => {
                self.settings.keywords = scene::search::defaults();
                log::info!("setting keywords = defaults");
                self.save_settings();
            }
        }
        self.toast(setting.shown(&self.settings_in_force()));
    }

    pub(crate) fn save_settings(&mut self) {
        if let Err(e) = storage::settings::save_settings(&self.data_dir, &self.settings) {
            log::warn!("settings: saving: {e}");
            self.toast("Couldn't save settings");
        }
        self.settings_changed();
    }

    /// Hands the language, spelling, cookie, tracking and proxy settings to
    /// Chromium (the profile and every private space's cookie jar).
    pub(crate) fn apply_browsing(&self) {
        let s = &self.settings;
        let tag = prefs::language_tag(s.language, &system_locale());
        let proxy = match s.proxy {
            prefs::Proxy::System => web::prefs::Proxy::System,
            prefs::Proxy::Direct => web::prefs::Proxy::Direct,
            prefs::Proxy::Custom => web::prefs::Proxy::Fixed(s.proxy_server.clone()),
        };
        let p = web::prefs::Prefs {
            accept_languages: prefs::accept_languages(&tag),
            spellcheck: s.spellcheck,
            dictionary: tag,
            block_third_party_cookies: s.block_third_party_cookies,
            proxy,
            cookies_blocked: self.cookie_blocks(),
        };
        log::info!("browsing prefs: {p:?}, do not track {}", s.do_not_track);
        web::prefs::apply(&p);
        for jar in self.jar.iter().chain(self.sessions.values().filter_map(|s| s.jar.as_ref())) {
            jar.refresh_prefs();
        }
        web::prefs::set_do_not_track(s.do_not_track);
    }

    /// Each site's own JavaScript, images and cookies: this session's rules
    /// over the saved ones.
    pub(crate) fn site_rules(&self) -> scene::sitesettings::SiteSettings {
        self.sites_session.over(&self.sites)
    }

    /// The sites whose cookies are blocked, for Chromium: real hosts only
    /// (`file:` pages have no cookies to block).
    pub(crate) fn cookie_blocks(&self) -> Vec<String> {
        use scene::sitesettings::Content;
        let rules = self.site_rules();
        rules
            .sites_with(Content::Cookies, false)
            .filter(|s| s.contains('.') || s.contains(':') || *s == "localhost")
            .map(str::to_string)
            .collect()
    }

    /// Hands JavaScript and images (the defaults and each site's own) to the pages.
    pub(crate) fn apply_site_content(&self) {
        use scene::sitesettings::Content;
        let (js, images) = (self.settings.javascript, self.settings.images);
        let sites = self.site_rules();
        web::set_javascript(move |url| sites.allows(url, Content::JavaScript, js));
        let sites = self.site_rules();
        web::prefs::set_images(move |url| sites.allows(url, Content::Images, images));
    }

    /// After JavaScript or images changed (the setting, from `before`, or a
    /// site's own): applies it and reloads the live pages it changes.
    pub(crate) fn site_content_changed(&mut self, before: &Settings) {
        let old_sites = self.site_rules();
        self.site_content_changed_from(&old_sites, before);
    }

    /// `old_sites`: the site rules in force before (`site_rules`).
    pub(crate) fn site_content_changed_from(&mut self, old_sites: &scene::sitesettings::SiteSettings, before: &Settings) {
        use scene::sitesettings::Content;
        self.apply_site_content();
        let rules = self.site_rules();
        let cookies_changed = old_sites.sites_with(Content::Cookies, false).ne(rules.sites_with(Content::Cookies, false));
        if cookies_changed {
            self.apply_browsing(); // before the reloads, so they see it
        }
        let mut reloaded = 0;
        for (id, c) in &self.cards {
            if c.page.kind() != web::PageKind::Web {
                continue;
            }
            let Some(url) = self.scene.node(*id).map(|n| n.url.clone()) else { continue };
            let was = |w, d| old_sites.allows(&url, w, d);
            let now = |w, d| rules.allows(&url, w, d);
            if was(Content::JavaScript, before.javascript) != now(Content::JavaScript, self.settings.javascript) {
                c.page.refresh_javascript(); // reloads
                reloaded += 1;
            } else if was(Content::Images, before.images) != now(Content::Images, self.settings.images)
                || was(Content::Cookies, true) != now(Content::Cookies, true)
            {
                c.page.reload();
                reloaded += 1;
            }
        }
        log::info!("site content: {reloaded} page(s) reloaded");
    }

    /// Hands the camera, microphone, location and notification settings to
    /// the pages. Answers given this session are forgotten, so a change
    /// applies to sites already asked.
    pub(crate) fn apply_permissions(&self) {
        use web::permission::{Decision, Kind};
        let s = &self.settings;
        let decide = |p: prefs::Permission| match p {
            prefs::Permission::Ask => Decision::Ask,
            prefs::Permission::Allow => Decision::Allow,
            prefs::Permission::Block => Decision::Block,
        };
        let (camera, mic, location, notes) = (decide(s.camera), decide(s.microphone), decide(s.location), decide(s.notifications));
        web::permission::set_policy(move |k| match k {
            Kind::Camera => camera,
            Kind::Microphone => mic,
            Kind::Location => location,
            Kind::Notifications => notes,
            Kind::Other => Decision::Block,
        });
        web::permission::forget_answers();
    }

    /// Page menu: JavaScript or images on or off for card `id`'s site.
    pub(crate) fn toggle_site_content(&mut self, id: NodeId, what: scene::sitesettings::Content) {
        self.switch_site(Some(id), scene::setcmd::SiteSwitch::Content(what), None, false);
    }

    /// `:site`, `:site-session`, `tj`…: JavaScript, images, cookies or ads
    /// on (`Some(true)`), off, or flipped (`None`) for card `id`'s site,
    /// saved or for this session only; its live pages reload.
    pub(crate) fn switch_site(&mut self, id: Option<NodeId>, what: scene::setcmd::SiteSwitch, on: Option<bool>, session: bool) {
        use scene::setcmd::SiteSwitch;
        use scene::sitesettings::Content;
        let Some(url) = id.and_then(|id| self.scene.node(id)).filter(|n| !n.is_note() && !n.url.is_empty()).map(|n| n.url.clone()) else {
            return self.toast("No page here");
        };
        if id.and_then(|id| self.cards.get(&id)).is_some_and(|c| c.page.kind() != web::PageKind::Web) {
            return self.toast("Only web pages have site settings");
        }
        let what = match what {
            SiteSwitch::Ads => return self.switch_site_ads(&url, on, session),
            SiteSwitch::Content(c) => c,
        };
        let default = match what {
            Content::JavaScript => self.settings.javascript,
            Content::Images => self.settings.images,
            Content::Cookies => true,
            Content::Passthrough => false,
        };
        let old = self.site_rules();
        let want = on.unwrap_or(!old.allows(&url, what, default));
        let site = pagezoom::site_of(&url);
        let changed = if session {
            self.sites_session.put(&url, what, Some(want))
        } else {
            // A saved choice replaces this session's for the site.
            let a = self.sites_session.put(&url, what, None);
            let b = self.sites.set(&url, what, want, default);
            if b && self.persist
                && let Err(e) = storage::sitesettings::save_sites(&self.data_dir, &self.sites)
            {
                log::error!("saving site settings: {e}");
            }
            a || b
        };
        let state = if want { "on" } else { "off" };
        if !changed || self.site_rules() == old {
            return self.toast(format!("{} already {state} for {site}", what.name()));
        }
        log::info!("site {site}: {} {state}{}", what.id(), if session { " (session)" } else { "" });
        let until = if session { " until you quit" } else { "" };
        if what == Content::Passthrough {
            // The card in use switches now; nothing reloads.
            if self.entered() && self.focused == id {
                self.set_page_keys(if want { PageKeys::Passthrough } else { PageKeys::Page });
            }
            let starts = if want { "starts" } else { "no longer starts" };
            return self.toast(format!("{site} {starts} in Passthrough{until}"));
        }
        let before = self.settings.clone();
        self.site_content_changed_from(&old, &before);
        self.toast(format!("{} {state} for {site}{until}", what.name()));
    }

    /// Ads shown (`Some(true)`: blocking off), blocked, or flipped on
    /// `url`'s site; for this session only if `session` (or in a private space).
    pub(crate) fn switch_site_ads(&mut self, url: &str, on: Option<bool>, session: bool) {
        if !self.settings.block_ads {
            return self.toast("Blocking ads is off in Settings (Ctrl+,)");
        }
        let shown = self.blocking.allows(url);
        if on.is_some_and(|want| want == shown) {
            let site = scene::icon::site(url).unwrap_or_default();
            return self.toast(format!("Ads already {} on {site}", if shown { "allowed" } else { "blocked" }));
        }
        let private = self.is_private() || session;
        let Some((site, on)) = self.blocking.toggle(url, private) else {
            return self.toast("Ads are only blocked on web pages");
        };
        self.reload_site(&site, on);
        self.toast(match (on, private) {
            (true, _) => format!("Blocking ads on {site} again"),
            (false, false) => format!("Ads allowed on {site}"),
            (false, true) => format!("Ads allowed on {site} until you quit"),
        });
    }

    /// The slider under *Corners: Custom* let go at `text` px.
    pub(crate) fn set_corner_radius(&mut self, text: &str) {
        let Some(px) = text.trim().parse::<u8>().ok().filter(|&r| r <= prefs::MAX_CORNER_RADIUS) else {
            log::warn!("settings page: bad corner radius {text:?}");
            return;
        };
        self.settings.corner_radius = px;
        log::info!("setting Corners = {}", self.settings.value(prefs::Key::Corners));
        self.set_theme(self.theme.clone());
        self.save_settings();
    }

    /// Enter in the proxy field: the server for *Proxy: Custom*.
    /// Adds a search keyword, or changes the one with that keyword.
    pub(crate) fn set_keyword(&mut self, key: &str, name: &str, url: &str) {
        match scene::search::check(key, name, url) {
            Ok(k) => {
                log::info!("setting keyword {} = {} ({})", k.key, k.url, k.name);
                self.toast(format!("“{} …” searches {}", k.key, k.name));
                scene::search::put(&mut self.settings.keywords, k);
                self.save_settings();
            }
            Err(e) => self.toast(e),
        }
    }

    pub(crate) fn remove_keyword(&mut self, key: &str) {
        let before = self.settings.keywords.len();
        self.settings.keywords.retain(|k| k.key != key);
        if self.settings.keywords.len() != before {
            log::info!("setting keyword {key} removed");
            self.save_settings();
        }
    }

    pub(crate) fn set_proxy_server(&mut self, text: &str) {
        let server = text.trim().to_string();
        if server.is_empty() {
            self.settings.proxy = prefs::Proxy::System;
            self.toast("No proxy server given · Proxy follows the system");
        } else {
            self.toast(format!("Proxy: {server}"));
        }
        self.settings.proxy_server = server;
        log::info!("setting Proxy = {}", self.settings.value(prefs::Key::Proxy));
        self.apply_browsing();
        self.save_settings();
    }
}
