//! The user's settings (the settings panel, Ctrl+,): what each can be, how a
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

/// The choices for how many cards keep a live page.
pub const LIVE_PAGES: [usize; 8] = [2, 3, 4, 6, 8, 10, 12, 16];
pub const DEFAULT_LIVE_PAGES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl Key {
    /// In the panel's order.
    pub const ALL: [Key; 7] = [Key::Search, Key::NewCard, Key::LivePages, Key::History, Key::BlockAds, Key::PageColours, Key::ScrollSpeed];

    pub fn label(self) -> &'static str {
        match self {
            Key::Search => "Search engine",
            Key::NewCard => "New card (Ctrl+T) opens",
            Key::LivePages => "Pages kept awake",
            Key::History => "Keep history",
            Key::PageColours => "Page colours",
            Key::ScrollSpeed => "Scroll speed",
            Key::BlockAds => "Block ads and trackers",
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
            Key::History => if self.history { "On" } else { "Off" }.to_string(),
            Key::PageColours => self.page_colours.name().to_string(),
            Key::ScrollSpeed => self.scroll_speed.name().to_string(),
            Key::BlockAds => if self.block_ads { "On" } else { "Off" }.to_string(),
        }
    }

    /// Moves `key` to its next (`dir` > 0) or previous value. Choices wrap
    /// round; the number of live pages stops at its ends, and a number
    /// between two choices (from `TRAILBOARD_MAX_LIVE`) goes to the nearer
    /// one that way. Returns whether anything changed.
    pub fn step(&mut self, key: Key, dir: i32) -> bool {
        let before = *self;
        match key {
            Key::Search => self.search = cycle(&SearchEngine::ALL, self.search, dir),
            Key::NewCard => self.new_card = cycle(&NewCard::ALL, self.new_card, dir),
            Key::PageColours => self.page_colours = cycle(&PageColours::ALL, self.page_colours, dir),
            Key::ScrollSpeed => self.scroll_speed = cycle(&ScrollSpeed::ALL, self.scroll_speed, dir),
            Key::History => self.history = !self.history,
            Key::BlockAds => self.block_ads = !self.block_ads,
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

#[cfg(test)]
mod tests {
    use super::*;

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
