//! Creating the app, and resetting a space's live state.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use scene::dock;
use scene::keys;
use scene::settings::Settings;
use scene::userkeys;
use scene::{Camera, History, motion, Scene, Selection, Vec2};
use storage::spaces::SpaceIndex;
use web::SpawnBeside;
use winit::keyboard::ModifiersState;

use crate::dev::script::Script;
use crate::page::clipboard::Clipboard;
use crate::ui::theme::Theme;
use super::{Anims, App, Drag, Hit, ICON_KEY, max_live_from_env, PageKeys, SiteIcon};

impl App {
    pub fn new(initial_urls: Vec<String>, data_dir: std::path::PathBuf, settings: Settings) -> Self {
        let live_env = max_live_from_env();
        log::info!("settings: {settings:?}");
        let mut zooms = storage::zoom::load_zooms(&data_dir);
        zooms.default = settings.default_zoom_factor();
        let zooms = Rc::new(RefCell::new(zooms));
        let sites = storage::sitesettings::load_sites(&data_dir);
        let start_page = storage::start::load_start(&data_dir);
        let recent_commands = storage::commands::load_recent(&data_dir);
        let places = storage::jumps::load(&data_dir);
        let saved_pages = storage::saved::load(&data_dir);
        let lookup = zooms.clone();
        web::set_page_zoom(move |url| lookup.borrow().get(url));
        let icons = storage::icons::load_icons(&data_dir)
            .into_iter()
            .enumerate()
            .map(|(i, (site, icon))| (site, SiteIcon { key: ICON_KEY | i as u64, serial: 1, icon, saved: true }))
            .collect();
        let data_dir_for_downloads = data_dir.clone();
        let page_index = match storage::pagetext::PageIndex::open(&data_dir) {
            Ok(i) => Some(i),
            Err(e) => {
                log::warn!("history is off: {e}");
                None
            }
        };
        let mut app = Self {
            renderer: None,
            scene: Scene::new(),
            camera: Camera::default(),
            camera_goal: None,
            opening_fit: None,
            page_scroll: Default::default(),
            board_pan: Default::default(),
            cards: HashMap::new(),
            inspectors: HashMap::new(),
            docks: HashMap::new(),
            dock_keys: false,
            dock_side: dock::Side::default(),
            max_live: live_env.unwrap_or(settings.live_pages),
            blocking: crate::system::blocking::Blocking::start(data_dir.clone(), settings.block_ads),
            settings,
            live_from_env: live_env.is_some(),
            used: HashMap::new(),
            media: HashMap::new(),
            media_asked: None,
            media_told: HashSet::new(),
            page_full: None,
            use_clock: 0,
            focused: None,
            alt_card: None,
            closed: Vec::new(),
            history: History::new(),
            selection: Selection::new(),
            group: Vec::new(),
            keys_in_page: false,
            keys: userkeys::Keys::default(),
            sheet_sections: Vec::new(),
            pending: keys::Pending::default(),
            page_keys: PageKeys::Page,
            prefix_asked: None,
            title_shown: "Trailboard".to_string(),
            caret: None,
            forwarded: HashSet::new(),
            omnibox: None,
            recent_commands,
            last_line: None,
            later: Vec::new(),
            later_ids: 0,
            messages: Default::default(),
            places,
            places_dirty: false,
            jumping: false,
            before: None,
            spot_ask: None,
            restore_due: HashMap::new(),
            saved_pages,
            menu: None,
            shields: Vec::new(),
            tags: Vec::new(),
            hints: false,
            sheet: Default::default(),
            sidebar_motion: Default::default(),
            link_hints: None,
            card_pick: None,
            scroll_targets: HashMap::new(),
            sections: HashMap::new(),
            section_ask: None,
            pager_ask: None,
            scroll_note: None,
            scroll_flash: None,
            arrange: None,
            read: None,
            read_morph: None,
            page_css: HashMap::new(),
            dock_css: HashMap::new(),
            sidebar: Default::default(),
            sidebar_open: false,
            sidebar_keys: false,
            sidebar_at: None,
            sidebar_hover: None,
            sidebar_press: None,
            sidebar_toggled: None,
            sidebar_peek: false,
            peek: Default::default(),
            sidebar_spot: None,
            menu_pressed: false,
            clipboard: Clipboard::new(),
            ime_allowed: false,
            ime_page: None,
            ime_area: None,
            spawn_beside: SpawnBeside::new(true),
            drag: Drag::None,
            cursor: Vec2::default(),
            mods: ModifiersState::empty(),
            last_click: None,
            click_count: 1,
            camera_changed: Instant::now(),
            page_scale: 1.0,
            initial_urls,
            started: Instant::now(),
            script: Script::from_env(),
            quit: false,
            frame_changed: true,
            event_seen: false,
            always_redraw: false,
            perf: None,
            data_dir,
            spaces: SpaceIndex::default(),
            persist: true,
            epoch: 0,
            sessions: HashMap::new(),
            jar: None,
            toast: None,
            sandbox: None,
            anim: Anims::default(),
            label_fades: crate::ui::chrome::Fades::default(),
            photo: crate::board::backdrop::Photo::default(),
            wallpaper: None,
            wallpaper_checked: None,
            photo_files: Vec::new(),
            confirm: None,
            dirty: false,
            last_save: Instant::now(),
            theme: Theme::default(),
            desktop: None,
            page_scheme: None,
            note_target: None,
            saved: HashMap::new(),
            zooms,
            icons,
            start_page,
            pointer_in: false,
            finding: None,
            found: None,
            last_find: String::new(),
            find_back: false,
            find_marks: None,
            menu_by_key: None,
            page_index,
            text_due: HashMap::new(),
            visits_cache: RefCell::new(None),
            preview: None,
            preview_serial: 0,
            thumb_tried: HashSet::new(),
            downloads: storage::downloads::load_downloads(&data_dir_for_downloads),
            download_ids: HashMap::new(),
            parked: Vec::new(),
            sites,
            sites_session: Default::default(),
            hud: crate::ui::hud::Hud::default(),
            hud_hover: None,
            hud_down: false,
            hud_open: None,
            card_size: Vec2::new(scene::CARD_W, scene::CARD_H),
            hover: Hit::Board,
            bars: Vec::new(),
            bar_buttons: Vec::new(),
            nudge: Vec::new(),
            still: {
                if let Some(k) = std::env::var("TRAILBOARD_SLOW_MOTION").ok().and_then(|v| v.parse::<f32>().ok()) {
                    motion::set_slow(k);
                }
                let still = crate::system::desktop::reduced_motion();
                crate::builtin::welcome::set_still(still);
                still
            },
            glides: motion::Glides::default(),
            ghosts: Vec::new(),
            edges_in: Vec::new(),
            edges_out: Vec::new(),
            gathers: Vec::new(),
            space_in: None,
            sidebar_last: None,
            sidebar_out: None,
            save_as: None,
            asks: Vec::new(),
            dialogs: Vec::new(),
            choosing: None,
            files_dir: None,
            editing: None,
            field_token: 0,
        };
        app.apply_browsing();
        app.apply_site_content();
        app.apply_permissions();
        app.load_keys(false);
        app
    }

    pub fn shutdown(&mut self) {
        self.save();
        self.stop_downloads();
        self.docks.clear();
        self.cards.clear(); // closes every browser before CEF shuts down
        self.parked.clear();
    }
}
