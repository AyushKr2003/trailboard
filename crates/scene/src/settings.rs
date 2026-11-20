//! The user's settings (the settings page, Ctrl+,): what each can be, how a
//! value steps to the next, and how it reads. Saved by `storage::settings`.

/// Where the address bar and the palette send a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEngine {
    DuckDuckGo,
    Google,
    Bing,
    Brave,
    Startpage,
    Ecosia,
    Kagi,
    Mojeek,
    Oscobo,
    MetaGer,
    Gibiru,
    Swisscows,
    WolframAlpha,
    Qwant,
    Searx,
}

impl SearchEngine {
    pub const ALL: [SearchEngine; 15] = [
        SearchEngine::DuckDuckGo,
        SearchEngine::Google,
        SearchEngine::Bing,
        SearchEngine::Brave,
        SearchEngine::Startpage,
        SearchEngine::Ecosia,
        SearchEngine::Kagi,
        SearchEngine::Mojeek,
        SearchEngine::Oscobo,
        SearchEngine::MetaGer,
        SearchEngine::Gibiru,
        SearchEngine::Swisscows,
        SearchEngine::WolframAlpha,
        SearchEngine::Qwant,
        SearchEngine::Searx,
    ];

    /// Its name in the settings file.
    pub fn id(self) -> &'static str {
        match self {
            SearchEngine::DuckDuckGo => "duckduckgo",
            SearchEngine::Google => "google",
            SearchEngine::Bing => "bing",
            SearchEngine::Brave => "brave",
            SearchEngine::Startpage => "startpage",
            SearchEngine::Ecosia => "ecosia",
            SearchEngine::Kagi => "kagi",
            SearchEngine::Mojeek => "mojeek",
            SearchEngine::Oscobo => "oscobo",
            SearchEngine::MetaGer => "metager",
            SearchEngine::Gibiru => "gibiru",
            SearchEngine::Swisscows => "swisscows",
            SearchEngine::WolframAlpha => "wolframalpha",
            SearchEngine::Qwant => "qwant",
            SearchEngine::Searx => "searx",
        }
    }

    pub fn from_id(id: &str) -> Option<SearchEngine> {
        SearchEngine::ALL.into_iter().find(|e| e.id() == id)
    }

    /// Its name on screen.
    pub fn name(self) -> &'static str {
        match self {
            SearchEngine::DuckDuckGo => "DuckDuckGo",
            SearchEngine::Google => "Google",
            SearchEngine::Bing => "Bing",
            SearchEngine::Brave => "Brave Search",
            SearchEngine::Startpage => "Startpage",
            SearchEngine::Ecosia => "Ecosia",
            SearchEngine::Kagi => "Kagi",
            SearchEngine::Mojeek => "Mojeek",
            SearchEngine::Oscobo => "Oscobo",
            SearchEngine::MetaGer => "MetaGer",
            SearchEngine::Gibiru => "Gibiru",
            SearchEngine::Swisscows => "Swisscows",
            SearchEngine::WolframAlpha => "WolframAlpha",
            SearchEngine::Qwant => "Qwant",
            SearchEngine::Searx => "SearXNG",
        }
    }

    /// The results page for a query already encoded for a URL.
    pub fn url(self, encoded: &str) -> String {
        let base = match self {
            SearchEngine::DuckDuckGo => "https://duckduckgo.com/?q=",
            SearchEngine::Google => "https://www.google.com/search?q=",
            SearchEngine::Bing => "https://www.bing.com/search?q=",
            SearchEngine::Brave => "https://search.brave.com/search?q=",
            SearchEngine::Startpage => "https://www.startpage.com/do/search?q=",
            SearchEngine::Ecosia => "https://www.ecosia.org/search?q=",
            SearchEngine::Kagi => "https://kagi.com/search?q=",
            SearchEngine::Mojeek => "https://www.mojeek.com/search?q=",
            SearchEngine::Oscobo => "https://www.oscobo.com/search.php?q=",
            SearchEngine::MetaGer => "https://metager.org/meta/meta.ger3?eingabe=",
            SearchEngine::Gibiru => "https://gibiru.com/results.html?q=",
            SearchEngine::Swisscows => "https://swisscows.com/en/web?query=",
            SearchEngine::WolframAlpha => "https://www.wolframalpha.com/input?i=",
            SearchEngine::Qwant => "https://www.qwant.com/?q=",
            // SearXNG is run by many people; this is one public instance.
            SearchEngine::Searx => "https://searx.be/search?q=",
        };
        format!("{base}{encoded}")
    }
}

/// Whether pages are told the desktop is light or dark (`prefers-color-scheme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageColours {
    /// As the Omarchy theme is.
    Theme,
    Light,
    Dark,
}

impl PageColours {
    pub const ALL: [PageColours; 3] = [PageColours::Theme, PageColours::Light, PageColours::Dark];

    pub fn id(self) -> &'static str {
        match self {
            PageColours::Theme => "theme",
            PageColours::Light => "light",
            PageColours::Dark => "dark",
        }
    }

    pub fn from_id(id: &str) -> Option<PageColours> {
        PageColours::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            PageColours::Theme => "Follow the theme",
            PageColours::Light => "Light",
            PageColours::Dark => "Dark",
        }
    }

    /// Pages are dark, given whether the theme is light.
    pub fn dark(self, theme_light: bool) -> bool {
        match self {
            PageColours::Theme => !theme_light,
            PageColours::Light => false,
            PageColours::Dark => true,
        }
    }
}

/// How far a wheel notch or touchpad movement scrolls a page (`scroll`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollSpeed {
    Slow,
    Normal,
    Fast,
    Faster,
}

impl ScrollSpeed {
    pub const ALL: [ScrollSpeed; 4] = [ScrollSpeed::Slow, ScrollSpeed::Normal, ScrollSpeed::Fast, ScrollSpeed::Faster];

    pub fn id(self) -> &'static str {
        match self {
            ScrollSpeed::Slow => "slow",
            ScrollSpeed::Normal => "normal",
            ScrollSpeed::Fast => "fast",
            ScrollSpeed::Faster => "faster",
        }
    }

    pub fn from_id(id: &str) -> Option<ScrollSpeed> {
        ScrollSpeed::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            ScrollSpeed::Slow => "Slow",
            ScrollSpeed::Normal => "Normal",
            ScrollSpeed::Fast => "Fast",
            ScrollSpeed::Faster => "Faster",
        }
    }

    /// Multiplier of `scroll::NOTCH` and `scroll::TOUCHPAD_GAIN`.
    pub fn factor(self) -> f32 {
        match self {
            ScrollSpeed::Slow => 0.6,
            ScrollSpeed::Normal => 1.0,
            ScrollSpeed::Fast => 1.5,
            ScrollSpeed::Faster => 2.2,
        }
    }
}

/// What Ctrl+T (*New card*) opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCard {
    /// The floating bar, to type an address or a search.
    AddressBar,
    /// A start page card: the search box and engine picker.
    StartPage,
}

impl NewCard {
    pub const ALL: [NewCard; 2] = [NewCard::AddressBar, NewCard::StartPage];

    pub fn id(self) -> &'static str {
        match self {
            NewCard::AddressBar => "address_bar",
            NewCard::StartPage => "start_page",
        }
    }

    pub fn from_id(id: &str) -> Option<NewCard> {
        NewCard::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            NewCard::AddressBar => "Address bar",
            NewCard::StartPage => "Start page",
        }
    }
}

/// What a site gets when it asks for a permission (camera, location, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// The app asks each time (Enter allows, Esc blocks), once per site and session.
    Ask,
    Allow,
    Block,
}

impl Permission {
    pub const ALL: [Permission; 3] = [Permission::Ask, Permission::Allow, Permission::Block];

    pub fn id(self) -> &'static str {
        match self {
            Permission::Ask => "ask",
            Permission::Allow => "allow",
            Permission::Block => "block",
        }
    }

    pub fn from_id(id: &str) -> Option<Permission> {
        Permission::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Permission::Ask => "Ask",
            Permission::Allow => "Allow",
            Permission::Block => "Block",
        }
    }
}

/// Where downloads are saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadTo {
    /// The desktop's Downloads folder (`XDG_DOWNLOAD_DIR`).
    Downloads,
    /// The desktop folder (`XDG_DESKTOP_DIR`).
    Desktop,
    /// A save dialog for each file.
    Ask,
}

impl DownloadTo {
    pub const ALL: [DownloadTo; 3] = [DownloadTo::Downloads, DownloadTo::Desktop, DownloadTo::Ask];

    pub fn id(self) -> &'static str {
        match self {
            DownloadTo::Downloads => "downloads",
            DownloadTo::Desktop => "desktop",
            DownloadTo::Ask => "ask",
        }
    }

    pub fn from_id(id: &str) -> Option<DownloadTo> {
        DownloadTo::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            DownloadTo::Downloads => "Downloads folder",
            DownloadTo::Desktop => "Desktop",
            DownloadTo::Ask => "Ask each time",
        }
    }
}

/// How pages reach the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proxy {
    /// As the desktop says (`http_proxy`, GNOME/KDE settings).
    System,
    /// No proxy.
    Direct,
    /// The server in `Settings::proxy_server`.
    Custom,
}

impl Proxy {
    pub const ALL: [Proxy; 3] = [Proxy::System, Proxy::Direct, Proxy::Custom];

    pub fn id(self) -> &'static str {
        match self {
            Proxy::System => "system",
            Proxy::Direct => "direct",
            Proxy::Custom => "custom",
        }
    }

    pub fn from_id(id: &str) -> Option<Proxy> {
        Proxy::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Proxy::System => "System",
            Proxy::Direct => "None",
            Proxy::Custom => "Custom",
        }
    }
}

/// How round corners are: cards, panels, menus and the app's own pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corners {
    /// Trailboard's own look: [`ROUNDED_RADIUS`] px.
    Rounded,
    /// Hyprland's window rounding (sharp on Omarchy's default).
    Hyprland,
    /// The radius in `Settings::corner_radius`.
    Custom,
}

/// The radius of [`Corners::Rounded`], in logical px.
pub const ROUNDED_RADIUS: f32 = 10.0;
/// The largest radius *Custom* takes.
pub const MAX_CORNER_RADIUS: u8 = 24;

impl Corners {
    pub const ALL: [Corners; 3] = [Corners::Rounded, Corners::Hyprland, Corners::Custom];

    pub fn id(self) -> &'static str {
        match self {
            Corners::Rounded => "rounded",
            Corners::Hyprland => "hyprland",
            Corners::Custom => "custom",
        }
    }

    pub fn from_id(id: &str) -> Option<Corners> {
        Corners::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Corners::Rounded => "Rounded",
            Corners::Hyprland => "Follow Hyprland",
            Corners::Custom => "Custom",
        }
    }
}

/// What the board shows behind the cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backdrop {
    /// A soft two-colour gradient from the space's tint, with fine grain and faint dots.
    Mood,
    /// A picture: the space's own, else the desktop's wallpaper; the mood until one loads.
    Photo,
    /// The space's flat colour with dots.
    Plain,
}

impl Backdrop {
    pub const ALL: [Backdrop; 3] = [Backdrop::Mood, Backdrop::Photo, Backdrop::Plain];

    pub fn id(self) -> &'static str {
        match self {
            Backdrop::Mood => "mood",
            Backdrop::Photo => "photo",
            Backdrop::Plain => "plain",
        }
    }

    pub fn from_id(id: &str) -> Option<Backdrop> {
        Backdrop::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Backdrop::Mood => "Colour mood",
            Backdrop::Photo => "Photo",
            Backdrop::Plain => "Plain",
        }
    }
}

/// The languages pages are asked for, as (id, name). `system` follows `$LANG`.
pub const LANGUAGES: [(&str, &str); 22] = [
    ("system", "System"),
    ("en-US", "English (US)"),
    ("en-GB", "English (UK)"),
    ("en-IN", "English (India)"),
    ("hi-IN", "Hindi"),
    ("bn-IN", "Bengali"),
    ("de-DE", "German"),
    ("fr-FR", "French"),
    ("es-ES", "Spanish"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("pt-PT", "Portuguese (Portugal)"),
    ("it-IT", "Italian"),
    ("nl-NL", "Dutch"),
    ("pl-PL", "Polish"),
    ("sv-SE", "Swedish"),
    ("ru-RU", "Russian"),
    ("uk-UA", "Ukrainian"),
    ("tr-TR", "Turkish"),
    ("ja-JP", "Japanese"),
    ("ko-KR", "Korean"),
    ("zh-CN", "Chinese (Simplified)"),
    ("zh-TW", "Chinese (Traditional)"),
];

/// The language id `id` names, if it is one of [`LANGUAGES`].
pub fn language_from_id(id: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|(i, _)| *i == id).map(|(i, _)| *i)
}

fn language_name(id: &str) -> &'static str {
    LANGUAGES.iter().find(|(i, _)| *i == id).map_or("System", |(_, n)| n)
}

/// A BCP 47 tag from a POSIX locale (`$LANG`): `en_IN.UTF-8` → `en-IN`.
/// `C`, `POSIX` or nothing give `en-US`.
pub fn locale_tag(posix: &str) -> String {
    let base = posix.split(['.', '@']).next().unwrap_or("").trim();
    if base.is_empty() || base == "C" || base == "POSIX" {
        return "en-US".into();
    }
    base.replace('_', "-")
}

/// The language tag `language` stands for (`system`: from `posix`, `$LANG`).
pub fn language_tag(language: &str, posix: &str) -> String {
    if language == "system" { locale_tag(posix) } else { language.to_string() }
}

/// The `Accept-Language` list for language tag `tag`: it, its base language,
/// then English (so a site without the language still answers in one most
/// people read). `hi-IN` → `hi-IN,hi,en-US,en`.
pub fn accept_languages(tag: &str) -> String {
    let base = tag.split('-').next().unwrap_or(tag);
    let mut list: Vec<&str> = vec![tag];
    for t in [base, "en-US", "en"] {
        if !list.iter().any(|l| l.eq_ignore_ascii_case(t)) {
            list.push(t);
        }
    }
    list.join(",")
}

/// The choices for a page zoom when a site has none of its own, in percent.
pub const DEFAULT_ZOOMS: [u16; 11] = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];

/// The choices for how many cards keep a live page.
pub const LIVE_PAGES: [usize; 8] = [2, 3, 4, 6, 8, 10, 12, 16];
pub const DEFAULT_LIVE_PAGES: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub search: SearchEngine,
    /// Cards that keep a live browser; the rest sleep (`liveness`).
    pub live_pages: usize,
    /// Pages opened are kept in history (text and thumbnail).
    pub history: bool,
    pub page_colours: PageColours,
    pub scroll_speed: ScrollSpeed,
    /// Block ads and trackers (`privacy`, `web::shield`).
    pub block_ads: bool,
    pub new_card: NewCard,
    /// Page zoom (percent) on sites without one of their own (Ctrl +/−).
    pub default_zoom: u16,
    /// Language pages are asked for: an id of [`LANGUAGES`].
    pub language: &'static str,
    /// Underline misspelt words (in the page's language) and suggest fixes.
    pub spellcheck: bool,
    /// Refuse cookies set by other sites than the page's.
    pub block_third_party_cookies: bool,
    /// Send `DNT: 1` and `Sec-GPC: 1` with every request.
    pub do_not_track: bool,
    /// Defaults for sites without their own rule (`sitesettings`).
    pub javascript: bool,
    pub images: bool,
    pub camera: Permission,
    pub microphone: Permission,
    pub location: Permission,
    pub notifications: Permission,
    pub download_to: DownloadTo,
    pub proxy: Proxy,
    /// `host:port` (or `scheme://host:port`) for [`Proxy::Custom`].
    pub proxy_server: String,
    /// The panel at the board's top left (sidebar button, the space's name).
    pub space_panel: bool,
    pub corners: Corners,
    pub backdrop: Backdrop,
    /// Radius in logical px for [`Corners::Custom`] (0 to [`MAX_CORNER_RADIUS`]).
    pub corner_radius: u8,
    /// Search keywords (`gh foo`), in the order the settings page lists them.
    pub keywords: Vec<crate::search::Keyword>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            search: SearchEngine::DuckDuckGo,
            live_pages: DEFAULT_LIVE_PAGES,
            history: true,
            page_colours: PageColours::Theme,
            scroll_speed: ScrollSpeed::Normal,
            block_ads: true,
            new_card: NewCard::AddressBar,
            default_zoom: 100,
            language: "system",
            spellcheck: true,
            block_third_party_cookies: false,
            do_not_track: false,
            javascript: true,
            images: true,
            camera: Permission::Ask,
            microphone: Permission::Ask,
            location: Permission::Ask,
            notifications: Permission::Ask,
            download_to: DownloadTo::Downloads,
            proxy: Proxy::System,
            proxy_server: String::new(),
            space_panel: true,
            corners: Corners::Rounded,
            backdrop: Backdrop::Mood,
            corner_radius: ROUNDED_RADIUS as u8,
            keywords: crate::search::defaults(),
        }
    }
}

impl Settings {
    /// What typed text opens, with this engine and these keywords (`search::resolve`).
    pub fn resolve(&self, input: &str) -> Option<crate::search::Resolved> {
        crate::search::resolve(input, self.search, &self.keywords)
    }

    /// The address typed text opens (see [`Settings::resolve`]).
    pub fn resolve_url(&self, input: &str) -> Option<String> {
        self.resolve(input).map(crate::search::Resolved::into_url)
    }

    /// The page zoom factor for sites without their own.
    /// The corner radius in use (logical px), given Hyprland's rounding.
    pub fn corner_radius(&self, hyprland: f32) -> f32 {
        match self.corners {
            Corners::Rounded => ROUNDED_RADIUS,
            Corners::Hyprland => hyprland.max(0.0),
            Corners::Custom => f32::from(self.corner_radius.min(MAX_CORNER_RADIUS)),
        }
    }

    pub fn default_zoom_factor(&self) -> f64 {
        f64::from(self.default_zoom) / 100.0
    }

    /// What a site asking for `kind` gets.
    pub fn permission(&self, kind: Key) -> Option<Permission> {
        match kind {
            Key::Camera => Some(self.camera),
            Key::Microphone => Some(self.microphone),
            Key::Location => Some(self.location),
            Key::Notifications => Some(self.notifications),
            _ => None,
        }
    }
}

/// One setting, a row of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Search,
    NewCard,
    LivePages,
    History,
    PageColours,
    ScrollSpeed,
    BlockAds,
    DefaultZoom,
    Language,
    Spellcheck,
    ThirdPartyCookies,
    DoNotTrack,
    JavaScript,
    Images,
    Camera,
    Microphone,
    Location,
    Notifications,
    DownloadTo,
    Proxy,
    SpacePanel,
    Corners,
    Backdrop,
}

impl Key {
    /// In the panel's order.
    pub const ALL: [Key; 23] = [
        Key::Search,
        Key::NewCard,
        Key::DefaultZoom,
        Key::Language,
        Key::Spellcheck,
        Key::LivePages,
        Key::History,
        Key::BlockAds,
        Key::ThirdPartyCookies,
        Key::DoNotTrack,
        Key::JavaScript,
        Key::Images,
        Key::Camera,
        Key::Microphone,
        Key::Location,
        Key::Notifications,
        Key::DownloadTo,
        Key::Proxy,
        Key::SpacePanel,
        Key::Corners,
        Key::Backdrop,
        Key::PageColours,
        Key::ScrollSpeed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Key::Search => "Search engine",
            Key::NewCard => "New card (Ctrl+T) opens",
            Key::LivePages => "Pages kept awake",
            Key::History => "Keep history",
            Key::PageColours => "Page colours",
            Key::ScrollSpeed => "Scroll speed",
            Key::BlockAds => "Block ads and trackers",
            Key::DefaultZoom => "Default page zoom",
            Key::Language => "Page language",
            Key::Spellcheck => "Check spelling",
            Key::ThirdPartyCookies => "Block third-party cookies",
            Key::DoNotTrack => "Ask sites not to track",
            Key::JavaScript => "JavaScript",
            Key::Images => "Images",
            Key::Camera => "Camera",
            Key::Microphone => "Microphone",
            Key::Location => "Location",
            Key::Notifications => "Notifications",
            Key::DownloadTo => "Save downloads to",
            Key::Proxy => "Proxy",
            Key::SpacePanel => "Show the space panel",
            Key::Corners => "Corners",
            Key::Backdrop => "Board background",
        }
    }

    /// Other words it is found by, and every value it can take.
    pub fn words(self) -> String {
        let (words, values): (&str, Vec<&str>) = match self {
            Key::Search => ("web search address bar", SearchEngine::ALL.iter().map(|e| e.name()).collect()),
            Key::NewCard => ("ctrl+t new tab card page home floating bar", NewCard::ALL.iter().map(|c| c.name()).collect()),
            Key::LivePages => ("live cards memory sleep tabs browsers", Vec::new()),
            Key::History => ("record visited pages privacy pause", vec!["on", "off"]),
            Key::PageColours => ("light dark mode colour color scheme", PageColours::ALL.iter().map(|c| c.name()).collect()),
            Key::ScrollSpeed => ("wheel touchpad mouse smooth", ScrollSpeed::ALL.iter().map(|c| c.name()).collect()),
            Key::BlockAds => ("adblock privacy easylist tracking content blocker", vec!["on", "off"]),
            Key::DefaultZoom => ("page zoom size scale text bigger smaller percent", Vec::new()),
            Key::Language => ("languages accept-language locale translate", LANGUAGES.iter().map(|(_, n)| *n).collect()),
            Key::Spellcheck => ("spell checker spelling dictionary typos", vec!["on", "off"]),
            Key::ThirdPartyCookies => ("cookies privacy tracking site data", vec!["on", "off"]),
            Key::DoNotTrack => ("do not track dnt gpc global privacy control", vec!["on", "off"]),
            Key::JavaScript => ("javascript js scripts site settings", vec!["on", "off"]),
            Key::Images => ("images pictures site settings", vec!["on", "off"]),
            Key::Camera | Key::Microphone | Key::Location | Key::Notifications => {
                ("site permissions privacy webcam mic geolocation", Permission::ALL.iter().map(|p| p.name()).collect())
            }
            Key::DownloadTo => ("downloads folder save location directory", DownloadTo::ALL.iter().map(|d| d.name()).collect()),
            Key::Proxy => ("proxy server network socks http", Proxy::ALL.iter().map(|p| p.name()).collect()),
            Key::SpacePanel => ("space name top left corner sidebar button chrome hide", vec!["on", "off"]),
            Key::Backdrop => ("board background wallpaper photo picture image gradient colour color mood grain plain", Backdrop::ALL.iter().map(|c| c.name()).collect()),
            Key::Corners => ("rounded round corner radius sharp square look hyprland", Corners::ALL.iter().map(|c| c.name()).collect()),
        };
        let mut s = words.to_string();
        for v in values {
            s.push(' ');
            s.push_str(v);
        }
        s
    }
}

/// Next (`dir` > 0) or previous in `all`, wrapping round.
fn cycle<T: Copy + PartialEq>(all: &[T], now: T, dir: i32) -> T {
    let i = all.iter().position(|&v| v == now).unwrap_or(0) as i32;
    let n = all.len() as i32;
    all[(i + dir.signum()).rem_euclid(n) as usize]
}

impl Settings {
    /// What `key` is set to, as shown.
    pub fn value(&self, key: Key) -> String {
        match key {
            Key::Search => self.search.name().to_string(),
            Key::NewCard => self.new_card.name().to_string(),
            Key::LivePages => self.live_pages.to_string(),
            Key::History => on_off(self.history),
            Key::PageColours => self.page_colours.name().to_string(),
            Key::ScrollSpeed => self.scroll_speed.name().to_string(),
            Key::BlockAds => on_off(self.block_ads),
            Key::DefaultZoom => format!("{}%", self.default_zoom),
            Key::Language => language_name(self.language).to_string(),
            Key::Spellcheck => on_off(self.spellcheck),
            Key::SpacePanel => on_off(self.space_panel),
            Key::ThirdPartyCookies => on_off(self.block_third_party_cookies),
            Key::DoNotTrack => on_off(self.do_not_track),
            Key::JavaScript => on_off(self.javascript),
            Key::Images => on_off(self.images),
            Key::Camera => self.camera.name().to_string(),
            Key::Microphone => self.microphone.name().to_string(),
            Key::Location => self.location.name().to_string(),
            Key::Notifications => self.notifications.name().to_string(),
            Key::DownloadTo => self.download_to.name().to_string(),
            Key::Corners => match self.corners {
                Corners::Custom => format!("Custom · {} px", self.corner_radius),
                c => c.name().to_string(),
            },
            Key::Backdrop => self.backdrop.name().to_string(),
            Key::Proxy => match self.proxy {
                Proxy::Custom if !self.proxy_server.is_empty() => format!("Custom · {}", self.proxy_server),
                p => p.name().to_string(),
            },
        }
    }

    /// Moves `key` to its next (`dir` > 0) or previous value. Choices wrap
    /// round; the number of live pages stops at its ends, and a number
    /// between two choices (from `TRAILBOARD_MAX_LIVE`) goes to the nearer
    /// one that way. Returns whether anything changed.
    pub fn step(&mut self, key: Key, dir: i32) -> bool {
        let before = self.clone();
        match key {
            Key::Search => self.search = cycle(&SearchEngine::ALL, self.search, dir),
            Key::NewCard => self.new_card = cycle(&NewCard::ALL, self.new_card, dir),
            Key::PageColours => self.page_colours = cycle(&PageColours::ALL, self.page_colours, dir),
            Key::ScrollSpeed => self.scroll_speed = cycle(&ScrollSpeed::ALL, self.scroll_speed, dir),
            Key::History => self.history = !self.history,
            Key::BlockAds => self.block_ads = !self.block_ads,
            Key::Spellcheck => self.spellcheck = !self.spellcheck,
            Key::SpacePanel => self.space_panel = !self.space_panel,
            Key::ThirdPartyCookies => self.block_third_party_cookies = !self.block_third_party_cookies,
            Key::DoNotTrack => self.do_not_track = !self.do_not_track,
            Key::JavaScript => self.javascript = !self.javascript,
            Key::Images => self.images = !self.images,
            Key::Camera => self.camera = cycle(&Permission::ALL, self.camera, dir),
            Key::Microphone => self.microphone = cycle(&Permission::ALL, self.microphone, dir),
            Key::Location => self.location = cycle(&Permission::ALL, self.location, dir),
            Key::Notifications => self.notifications = cycle(&Permission::ALL, self.notifications, dir),
            Key::DownloadTo => self.download_to = cycle(&DownloadTo::ALL, self.download_to, dir),
            Key::Proxy => self.proxy = cycle(&Proxy::ALL, self.proxy, dir),
            Key::Corners => self.corners = cycle(&Corners::ALL, self.corners, dir),
            Key::Backdrop => self.backdrop = cycle(&Backdrop::ALL, self.backdrop, dir),
            Key::Language => {
                let ids: Vec<&'static str> = LANGUAGES.iter().map(|(i, _)| *i).collect();
                self.language = cycle(&ids, self.language, dir);
            }
            Key::DefaultZoom => {
                // Stops at the ends, like the number of live pages.
                let z = self.default_zoom;
                let next = if dir > 0 {
                    DEFAULT_ZOOMS.iter().copied().find(|&v| v > z)
                } else {
                    DEFAULT_ZOOMS.iter().rev().copied().find(|&v| v < z)
                };
                if let Some(v) = next {
                    self.default_zoom = v;
                }
            }
            Key::LivePages => {
                let n = self.live_pages;
                let next = if dir > 0 {
                    LIVE_PAGES.iter().copied().find(|&v| v > n)
                } else {
                    LIVE_PAGES.iter().rev().copied().find(|&v| v < n)
                };
                if let Some(v) = next {
                    self.live_pages = v;
                }
            }
        }
        *self != before
    }
}

impl Key {
    /// Its name in the settings page's messages.
    pub fn id(self) -> &'static str {
        match self {
            Key::Search => "search",
            Key::NewCard => "new_card",
            Key::LivePages => "live_pages",
            Key::History => "history",
            Key::PageColours => "page_colours",
            Key::ScrollSpeed => "scroll_speed",
            Key::BlockAds => "block_ads",
            Key::DefaultZoom => "default_zoom",
            Key::Language => "language",
            Key::Spellcheck => "spellcheck",
            Key::ThirdPartyCookies => "third_party_cookies",
            Key::DoNotTrack => "do_not_track",
            Key::JavaScript => "javascript",
            Key::Images => "images",
            Key::Camera => "camera",
            Key::Microphone => "microphone",
            Key::Location => "location",
            Key::Notifications => "notifications",
            Key::DownloadTo => "download_to",
            Key::Proxy => "proxy",
            Key::SpacePanel => "space_panel",
            Key::Corners => "corners",
            Key::Backdrop => "board_background",
        }
    }

    pub fn from_id(id: &str) -> Option<Key> {
        Key::ALL.into_iter().find(|k| k.id() == id)
    }

    /// Whether it is on or off (a switch rather than a list of choices).
    pub fn is_switch(self) -> bool {
        matches!(
            self,
            Key::History
                | Key::BlockAds
                | Key::Spellcheck
                | Key::ThirdPartyCookies
                | Key::DoNotTrack
                | Key::JavaScript
                | Key::Images
                | Key::SpacePanel
        )
    }
}

fn ids<T: Copy>(all: &[T], id: fn(T) -> &'static str, name: fn(T) -> &'static str) -> Vec<(String, String)> {
    all.iter().map(|&v| (id(v).to_string(), name(v).to_string())).collect()
}

impl Settings {
    /// Every value `key` can take, as (id, name), in order. A number set
    /// outside the usual choices (by `TRAILBOARD_MAX_LIVE` or by hand) is
    /// among them, so it can be shown.
    pub fn choices(&self, key: Key) -> Vec<(String, String)> {
        if key.is_switch() {
            return vec![("on".into(), "On".into()), ("off".into(), "Off".into())];
        }
        match key {
            Key::Search => ids(&SearchEngine::ALL, SearchEngine::id, SearchEngine::name),
            Key::NewCard => ids(&NewCard::ALL, NewCard::id, NewCard::name),
            Key::PageColours => ids(&PageColours::ALL, PageColours::id, PageColours::name),
            Key::ScrollSpeed => ids(&ScrollSpeed::ALL, ScrollSpeed::id, ScrollSpeed::name),
            Key::Camera | Key::Microphone | Key::Location | Key::Notifications => ids(&Permission::ALL, Permission::id, Permission::name),
            Key::DownloadTo => ids(&DownloadTo::ALL, DownloadTo::id, DownloadTo::name),
            Key::Proxy => ids(&Proxy::ALL, Proxy::id, Proxy::name),
            Key::Corners => ids(&Corners::ALL, Corners::id, Corners::name),
            Key::Backdrop => ids(&Backdrop::ALL, Backdrop::id, Backdrop::name),
            Key::Language => LANGUAGES.iter().map(|(i, n)| (i.to_string(), n.to_string())).collect(),
            Key::DefaultZoom => {
                let mut v: Vec<u16> = DEFAULT_ZOOMS.to_vec();
                if !v.contains(&self.default_zoom) {
                    v.push(self.default_zoom);
                    v.sort_unstable();
                }
                v.into_iter().map(|z| (z.to_string(), format!("{z}%"))).collect()
            }
            Key::LivePages => {
                let mut v: Vec<usize> = LIVE_PAGES.to_vec();
                if !v.contains(&self.live_pages) {
                    v.push(self.live_pages);
                    v.sort_unstable();
                }
                v.into_iter().map(|n| (n.to_string(), n.to_string())).collect()
            }
            _ => Vec::new(),
        }
    }

    /// The id (in [`Settings::choices`]) of what `key` is set to.
    pub fn choice(&self, key: Key) -> String {
        let on = |b: bool| if b { "on" } else { "off" }.to_string();
        match key {
            Key::Search => self.search.id().into(),
            Key::NewCard => self.new_card.id().into(),
            Key::LivePages => self.live_pages.to_string(),
            Key::History => on(self.history),
            Key::PageColours => self.page_colours.id().into(),
            Key::ScrollSpeed => self.scroll_speed.id().into(),
            Key::BlockAds => on(self.block_ads),
            Key::DefaultZoom => self.default_zoom.to_string(),
            Key::Language => self.language.into(),
            Key::Spellcheck => on(self.spellcheck),
            Key::ThirdPartyCookies => on(self.block_third_party_cookies),
            Key::DoNotTrack => on(self.do_not_track),
            Key::JavaScript => on(self.javascript),
            Key::Images => on(self.images),
            Key::Camera => self.camera.id().into(),
            Key::Microphone => self.microphone.id().into(),
            Key::Location => self.location.id().into(),
            Key::Notifications => self.notifications.id().into(),
            Key::DownloadTo => self.download_to.id().into(),
            Key::Proxy => self.proxy.id().into(),
            Key::SpacePanel => on(self.space_panel),
            Key::Corners => self.corners.id().into(),
            Key::Backdrop => self.backdrop.id().into(),
        }
    }

    /// Sets `key` to the choice `id`. Returns whether anything changed; an
    /// unknown id changes nothing.
    pub fn choose(&mut self, key: Key, id: &str) -> bool {
        let before = self.clone();
        let flag = match id {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        };
        match key {
            Key::Search => SearchEngine::from_id(id).into_iter().for_each(|v| self.search = v),
            Key::NewCard => NewCard::from_id(id).into_iter().for_each(|v| self.new_card = v),
            Key::PageColours => PageColours::from_id(id).into_iter().for_each(|v| self.page_colours = v),
            Key::ScrollSpeed => ScrollSpeed::from_id(id).into_iter().for_each(|v| self.scroll_speed = v),
            Key::Camera => Permission::from_id(id).into_iter().for_each(|v| self.camera = v),
            Key::Microphone => Permission::from_id(id).into_iter().for_each(|v| self.microphone = v),
            Key::Location => Permission::from_id(id).into_iter().for_each(|v| self.location = v),
            Key::Notifications => Permission::from_id(id).into_iter().for_each(|v| self.notifications = v),
            Key::DownloadTo => DownloadTo::from_id(id).into_iter().for_each(|v| self.download_to = v),
            Key::Proxy => Proxy::from_id(id).into_iter().for_each(|v| self.proxy = v),
            Key::Corners => Corners::from_id(id).into_iter().for_each(|v| self.corners = v),
            Key::Backdrop => Backdrop::from_id(id).into_iter().for_each(|v| self.backdrop = v),
            Key::Language => language_from_id(id).into_iter().for_each(|v| self.language = v),
            Key::DefaultZoom => {
                if let Some(z) = id.parse::<u16>().ok().filter(|z| (25..=500).contains(z)) {
                    self.default_zoom = z;
                }
            }
            Key::LivePages => {
                if let Some(n) = id.parse::<usize>().ok().filter(|n| (1..=64).contains(n)) {
                    self.live_pages = n;
                }
            }
            Key::History => flag.into_iter().for_each(|v| self.history = v),
            Key::BlockAds => flag.into_iter().for_each(|v| self.block_ads = v),
            Key::Spellcheck => flag.into_iter().for_each(|v| self.spellcheck = v),
            Key::ThirdPartyCookies => flag.into_iter().for_each(|v| self.block_third_party_cookies = v),
            Key::DoNotTrack => flag.into_iter().for_each(|v| self.do_not_track = v),
            Key::JavaScript => flag.into_iter().for_each(|v| self.javascript = v),
            Key::Images => flag.into_iter().for_each(|v| self.images = v),
            Key::SpacePanel => flag.into_iter().for_each(|v| self.space_panel = v),
        }
        *self != before
    }
}

fn on_off(on: bool) -> String {
    if on { "On" } else { "Off" }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_radius_by_choice() {
        let mut s = Settings::default();
        assert_eq!(s.corner_radius(0.0), ROUNDED_RADIUS);
        s.corners = Corners::Hyprland;
        assert_eq!(s.corner_radius(8.0), 8.0);
        assert_eq!(s.corner_radius(-1.0), 0.0);
        s.corners = Corners::Custom;
        s.corner_radius = 4;
        assert_eq!(s.corner_radius(8.0), 4.0);
        assert_eq!(s.value(Key::Corners), "Custom · 4 px");
        s.corner_radius = 200;
        assert_eq!(s.corner_radius(0.0), f32::from(MAX_CORNER_RADIUS));
        assert!(s.choose(Key::Corners, "rounded"));
        assert_eq!(s.corners, Corners::Rounded);
        assert!(!s.choose(Key::Corners, "soft"));
    }

    #[test]
    fn choices_wrap_and_numbers_stop() {
        let mut s = Settings::default();
        assert!(s.step(Key::Search, -1));
        assert_eq!(s.search, SearchEngine::Searx);
        assert!(s.step(Key::Search, 1));
        assert_eq!(s.search, SearchEngine::DuckDuckGo);
        s.step(Key::PageColours, 1);
        assert_eq!(s.page_colours, PageColours::Light);
        assert!(s.step(Key::ScrollSpeed, 1));
        assert_eq!(s.scroll_speed, ScrollSpeed::Fast);
        s.step(Key::ScrollSpeed, -1);
        s.step(Key::ScrollSpeed, -1);
        s.step(Key::ScrollSpeed, -1);
        assert_eq!(s.scroll_speed, ScrollSpeed::Faster, "wraps");
        assert!(s.step(Key::History, 1));
        assert!(!s.history);
        assert!(s.step(Key::History, -1));
        assert!(s.history);
        assert!(s.block_ads);
        assert!(s.step(Key::BlockAds, 1));
        assert!(!s.block_ads);
        assert_eq!(s.value(Key::BlockAds), "Off");

        s.live_pages = 16;
        assert!(!s.step(Key::LivePages, 1));
        s.live_pages = 2;
        assert!(!s.step(Key::LivePages, -1));
        // Between two choices: the nearer one that way.
        s.live_pages = 5;
        s.step(Key::LivePages, 1);
        assert_eq!(s.live_pages, 6);
        s.live_pages = 5;
        s.step(Key::LivePages, -1);
        assert_eq!(s.live_pages, 4);
        s.live_pages = 40;
        s.step(Key::LivePages, -1);
        assert_eq!(s.live_pages, 16);
    }

    #[test]
    fn new_card_toggles_between_bar_and_start_page() {
        let mut s = Settings::default();
        assert_eq!(s.new_card, NewCard::AddressBar);
        assert!(s.step(Key::NewCard, 1));
        assert_eq!((s.new_card, s.value(Key::NewCard).as_str()), (NewCard::StartPage, "Start page"));
        assert!(s.step(Key::NewCard, 1));
        assert_eq!(s.new_card, NewCard::AddressBar);
        for c in NewCard::ALL {
            assert_eq!(NewCard::from_id(c.id()), Some(c));
        }
        assert!(Key::NewCard.words().contains("Start page"));
    }

    #[test]
    fn browsing_settings_step_and_read() {
        let mut s = Settings::default();
        assert_eq!(s.value(Key::DefaultZoom), "100%");
        assert!(s.step(Key::DefaultZoom, 1));
        assert_eq!(s.default_zoom_factor(), 1.1);
        s.default_zoom = 200;
        assert!(!s.step(Key::DefaultZoom, 1), "stops at the top");
        s.default_zoom = 50;
        assert!(!s.step(Key::DefaultZoom, -1), "and the bottom");
        // A value between choices (edited by hand) goes to the nearer one that way.
        s.default_zoom = 105;
        s.step(Key::DefaultZoom, -1);
        assert_eq!(s.default_zoom, 100);

        assert_eq!(s.value(Key::Language), "System");
        assert!(s.step(Key::Language, 1));
        assert_eq!((s.language, s.value(Key::Language).as_str()), ("en-US", "English (US)"));
        assert!(s.step(Key::Language, -1));
        assert!(s.step(Key::Language, -1));
        assert_eq!(s.language, "zh-TW", "wraps");

        assert_eq!(s.permission(Key::Camera), Some(Permission::Ask));
        assert!(s.step(Key::Camera, 1));
        assert_eq!(s.value(Key::Camera), "Allow");
        assert_eq!(s.permission(Key::Microphone), Some(Permission::Ask), "each on its own");
        assert_eq!(s.permission(Key::Images), None);

        assert!(s.step(Key::DownloadTo, -1));
        assert_eq!(s.download_to, DownloadTo::Ask);
        assert!(s.step(Key::Proxy, -1));
        assert_eq!(s.value(Key::Proxy), "Custom");
        s.proxy_server = "127.0.0.1:8080".into();
        assert_eq!(s.value(Key::Proxy), "Custom · 127.0.0.1:8080");

        for k in [Key::Spellcheck, Key::JavaScript, Key::Images] {
            assert_eq!(s.value(k), "On");
            assert!(s.step(k, 1));
            assert_eq!(s.value(k), "Off");
        }
        for k in [Key::ThirdPartyCookies, Key::DoNotTrack] {
            assert_eq!(s.value(k), "Off");
            assert!(s.step(k, 1));
            assert_eq!(s.value(k), "On");
        }
        assert!(Key::ALL.iter().all(|k| !k.label().is_empty()));
        assert!(Key::Camera.words().contains("Block"));
        assert!(Key::Language.words().contains("Hindi"));
    }

    #[test]
    fn every_setting_can_be_chosen_by_id() {
        for k in Key::ALL {
            assert_eq!(Key::from_id(k.id()), Some(k));
            let mut s = Settings::default();
            let choices = s.choices(k);
            assert!(choices.len() >= 2, "{k:?}");
            assert!(choices.iter().any(|(id, _)| *id == s.choice(k)), "{k:?}: current is a choice");
            for (id, _) in &choices {
                s.choose(k, id);
                assert_eq!(&s.choice(k), id, "{k:?}");
            }
            let before = s.clone();
            assert!(!s.choose(k, "no-such-value"), "{k:?}");
            assert_eq!(s, before);
        }
        let mut s = Settings::default();
        assert!(s.choose(Key::Search, "kagi"));
        assert_eq!(s.search, SearchEngine::Kagi);
        assert!(!s.choose(Key::Search, "kagi"), "unchanged");
        assert!(s.choose(Key::BlockAds, "off"));
        assert!(!s.block_ads);
        assert!(!s.choose(Key::LivePages, "0"), "out of range");
        // A number outside the usual choices is offered too.
        s.live_pages = 5;
        assert!(s.choices(Key::LivePages).iter().any(|(id, _)| id == "5"));
        assert!(Key::History.is_switch() && !Key::Proxy.is_switch());
    }

    #[test]
    fn languages_become_accept_language_lists() {
        assert_eq!(locale_tag("en_IN.UTF-8"), "en-IN");
        assert_eq!(locale_tag("de_DE@euro"), "de-DE");
        assert_eq!(locale_tag("C"), "en-US");
        assert_eq!(locale_tag(""), "en-US");
        assert_eq!(language_tag("system", "fr_FR.UTF-8"), "fr-FR");
        assert_eq!(language_tag("hi-IN", "fr_FR.UTF-8"), "hi-IN");
        assert_eq!(accept_languages("hi-IN"), "hi-IN,hi,en-US,en");
        assert_eq!(accept_languages("en-US"), "en-US,en");
        assert_eq!(accept_languages("en-GB"), "en-GB,en,en-US");
        assert_eq!(accept_languages("ja"), "ja,en-US,en");
        assert_eq!(language_from_id("pt-BR"), Some("pt-BR"));
        assert_eq!(language_from_id("xx"), None);
        for p in Permission::ALL {
            assert_eq!(Permission::from_id(p.id()), Some(p));
        }
        for d in DownloadTo::ALL {
            assert_eq!(DownloadTo::from_id(d.id()), Some(d));
        }
        for p in Proxy::ALL {
            assert_eq!(Proxy::from_id(p.id()), Some(p));
        }
    }

    #[test]
    fn ids_round_trip_and_values_read() {
        for e in SearchEngine::ALL {
            assert_eq!(SearchEngine::from_id(e.id()), Some(e));
            assert!(e.url("a+b").starts_with("https://") && e.url("a+b").ends_with("=a+b"));
        }
        for c in PageColours::ALL {
            assert_eq!(PageColours::from_id(c.id()), Some(c));
        }
        for c in ScrollSpeed::ALL {
            assert_eq!(ScrollSpeed::from_id(c.id()), Some(c));
        }
        assert_eq!(Settings::default().value(Key::ScrollSpeed), "Normal");
        assert_eq!(SearchEngine::from_id("altavista"), None);
        let s = Settings::default();
        assert_eq!(s.value(Key::Search), "DuckDuckGo");
        assert_eq!(s.value(Key::LivePages), "6");
        assert_eq!(s.value(Key::History), "On");
        assert!(Key::Search.words().contains("Startpage"));
        assert!(PageColours::Theme.dark(false) && !PageColours::Theme.dark(true));
        assert!(PageColours::Dark.dark(true) && !PageColours::Light.dark(false));
    }
}
