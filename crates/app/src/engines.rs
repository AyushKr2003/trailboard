//! What the start page and the search engine guide say about each search
//! engine: a group for the picker, a one-line pitch, a short description,
//! who it suits, a few tags, its icon on the web and a colour and letters
//! for the badge shown while the icon loads (or when it can't).
//!
//! The guide (`engines.html`) is one of the app's own pages, kept on the
//! board as `scene::note::ENGINES_URL`, built like the welcome page.

use scene::settings::SearchEngine;

use crate::note::percent_encode;
use crate::theme::Theme;

const PAGE: &str = include_str!("engines.html");

/// Where an engine sits in the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Private by design: no profile of you.
    Private,
    /// The big general engines, which track and personalise.
    Popular,
    /// Answers and calculations rather than web pages.
    Answers,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Private, Group::Popular, Group::Answers];

    fn id(self) -> &'static str {
        match self {
            Group::Private => "private",
            Group::Popular => "popular",
            Group::Answers => "answers",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Group::Private => "Privacy first",
            Group::Popular => "Most popular",
            Group::Answers => "Answers",
        }
    }
}

pub struct Info {
    pub group: Group,
    /// One line under the name.
    pub pitch: &'static str,
    /// Two or three sentences: what it is and how it works.
    pub about: &'static str,
    /// Who it suits.
    pub best_for: &'static str,
    pub tags: &'static [&'static str],
    /// Badge colour, `#rrggbb`.
    pub colour: &'static str,
    /// Badge letters.
    pub mark: &'static str,
    /// Its home, shown on the guide.
    pub site: &'static str,
    /// Its own icon, loaded from the web when a page shows it; the page
    /// falls back to the monogram badge when it can't load.
    pub icon: &'static str,
}

pub fn info(e: SearchEngine) -> Info {
    use Group::*;
    use SearchEngine as E;
    match e {
        E::DuckDuckGo => Info {
            group: Private,
            pitch: "Private everyday search",
            about: "Doesn't track you or keep a profile of your searches. Results come mostly from Bing plus its own crawler, with instant answers and !bangs that jump straight into other sites' search.",
            best_for: "Anyone who wants a private default that just works.",
            tags: &["No tracking", "!bangs"],
            colour: "#de5833",
            mark: "D",
            site: "duckduckgo.com",
            icon: "https://duckduckgo.com/static-assets/favicons/DDG-iOS-icon_152x152.png",
        },
        E::Brave => Info {
            group: Private,
            pitch: "Its own independent index",
            about: "Brave runs its own web index instead of renting results from Google or Bing, and doesn't profile you. Goggles let you re-rank results with your own rules.",
            best_for: "People who want private search that doesn't depend on Big Tech.",
            tags: &["No tracking", "Own index"],
            colour: "#fb542b",
            mark: "B",
            site: "search.brave.com",
            icon: "https://www.google.com/s2/favicons?domain=search.brave.com&sz=128",
        },
        E::Startpage => Info {
            group: Private,
            pitch: "Google results, without Google",
            about: "Sends your query to Google on your behalf, so you get Google-quality results without Google seeing who you are. Anonymous View opens result pages through its proxy.",
            best_for: "Google fans who don't want to be tracked.",
            tags: &["No tracking", "Google results", "Proxy view"],
            colour: "#6573ff",
            mark: "S",
            site: "startpage.com",
            icon: "https://www.google.com/s2/favicons?domain=startpage.com&sz=128",
        },
        E::Mojeek => Info {
            group: Private,
            pitch: "A truly independent crawler",
            about: "A UK engine with its own crawler and index built since 2004, no tracking and no personalisation. The index is smaller, so you see a different, unfiltered slice of the web.",
            best_for: "Curious searchers who want results no one else ranks for them.",
            tags: &["No tracking", "Own index"],
            colour: "#7a5cff",
            mark: "M",
            site: "mojeek.com",
            icon: "https://www.mojeek.com/apple-touch-icon.png",
        },
        E::Qwant => Info {
            group: Private,
            pitch: "European search, European privacy",
            about: "A French engine under EU privacy law that doesn't track you or sell your data. Results mix Bing with a growing European index.",
            best_for: "People who want a European alternative.",
            tags: &["No tracking", "EU"],
            colour: "#5c7cfa",
            mark: "Q",
            site: "qwant.com",
            icon: "https://www.qwant.com/apple-touch-icon.png",
        },
        E::Swisscows => Info {
            group: Private,
            pitch: "Family-safe, Swiss-hosted",
            about: "Runs on its own servers in Switzerland, stores nothing about you, and always filters out adult and violent content.",
            best_for: "Families, schools and shared computers.",
            tags: &["No tracking", "Family safe", "Switzerland"],
            colour: "#e5242e",
            mark: "Sw",
            site: "swisscows.com",
            icon: "https://swisscows.com/favicon-144.png",
        },
        E::MetaGer => Info {
            group: Private,
            pitch: "Open-source metasearch",
            about: "Run by a German non-profit, it asks several engines at once and blends their results. It's open source and can open results through an anonymising proxy. Heavy use may need a MetaGer key.",
            best_for: "Privacy purists who like non-profit, open tools.",
            tags: &["No tracking", "Metasearch", "Non-profit"],
            colour: "#f08c00",
            mark: "Mg",
            site: "metager.org",
            icon: "https://metager.org/img/favicon/favicon-114.png",
        },
        E::Searx => Info {
            group: Private,
            pitch: "Hackable metasearch",
            about: "Open-source metasearch that anyone can host. It gathers results from dozens of engines and passes on nothing about you. Trailboard uses a public instance (searx.be), which can be slow at busy times.",
            best_for: "Tinkerers and people who run their own services.",
            tags: &["No tracking", "Metasearch", "Open source"],
            colour: "#3050ff",
            mark: "Sx",
            site: "searx.be",
            icon: "https://searx.be/static/themes/simple/img/favicon.svg",
        },
        E::Oscobo => Info {
            group: Private,
            pitch: "Simple, private, British",
            about: "A small UK engine that doesn't track or profile you. Results come from Bing, presented without the personalisation.",
            best_for: "Anyone who wants a quiet, no-frills private search.",
            tags: &["No tracking"],
            colour: "#1c9a8a",
            mark: "O",
            site: "oscobo.com",
            icon: "https://www.oscobo.com/favicon.ico",
        },
        E::Gibiru => Info {
            group: Private,
            pitch: "Less filtered results",
            about: "Says it doesn't keep your searches and shows results with less filtering than the big engines.",
            best_for: "Researchers who want to see what's normally filtered out.",
            tags: &["No tracking", "Uncensored"],
            colour: "#2b8a3e",
            mark: "G",
            site: "gibiru.com",
            icon: "https://gibiru.com/assets/img/favicon.ico",
        },
        E::Ecosia => Info {
            group: Private,
            pitch: "Searches that plant trees",
            about: "A Berlin company that spends its profits on climate projects and tree planting. Results come from Bing and Google, and it doesn't sell your data.",
            best_for: "Climate-minded people who want their searches to help.",
            tags: &["Doesn't sell data", "Climate"],
            colour: "#2f9e44",
            mark: "E",
            site: "ecosia.org",
            icon: "https://www.ecosia.org/apple-touch-icon.png",
        },
        E::Kagi => Info {
            group: Private,
            pitch: "Paid, ad-free, yours to tune",
            about: "A subscription engine with no ads and no tracking. You can block, boost or pin sites so results fit you. Needs a Kagi account.",
            best_for: "Power users happy to pay for quality.",
            tags: &["No ads", "Paid", "Custom ranking"],
            colour: "#ffb319",
            mark: "K",
            site: "kagi.com",
            icon: "https://kagi.com/apple-touch-icon.png",
        },
        E::Google => Info {
            group: Popular,
            pitch: "The biggest index",
            about: "The largest web index and the strongest for local, recent and obscure results. In return it tracks your searches and personalises what you see.",
            best_for: "The widest coverage when privacy matters less.",
            tags: &["Largest index", "Tracks you"],
            colour: "#4285f4",
            mark: "G",
            site: "google.com",
            icon: "https://www.google.com/s2/favicons?domain=google.com&sz=128",
        },
        E::Bing => Info {
            group: Popular,
            pitch: "Microsoft's web search",
            about: "Microsoft's engine, strong on images and video and tied to Microsoft accounts. Many private engines are built on its results.",
            best_for: "People in the Microsoft world and image search.",
            tags: &["Own index", "Tracks you"],
            colour: "#0f8a7e",
            mark: "B",
            site: "bing.com",
            icon: "https://www.bing.com/apple-touch-icon.png",
        },
        E::WolframAlpha => Info {
            group: Answers,
            pitch: "Computes answers, not links",
            about: "A computational knowledge engine: maths, units, science, dates and data, worked out step by step. Not a web search, so it has no list of pages.",
            best_for: "Students, engineers and anyone with a calculation.",
            tags: &["Maths", "Facts", "No web results"],
            colour: "#f96932",
            mark: "W",
            site: "wolframalpha.com",
            icon: "https://www.google.com/s2/favicons?domain=wolframalpha.com&sz=128",
        },
    }
}

/// The engines, the groups and the one in use, as JSON for the pages.
pub fn data(current: SearchEngine) -> String {
    let q = web::js_string;
    let engines: Vec<String> = SearchEngine::ALL
        .iter()
        .map(|&e| {
            let i = info(e);
            let tags: Vec<String> = i.tags.iter().map(|t| q(t)).collect();
            format!(
                "{{\"id\":{},\"name\":{},\"group\":{},\"pitch\":{},\"about\":{},\"best\":{},\"tags\":[{}],\"colour\":{},\"mark\":{},\"site\":{},\"icon\":{}}}",
                q(e.id()),
                q(e.name()),
                q(i.group.id()),
                q(i.pitch),
                q(i.about),
                q(i.best_for),
                tags.join(","),
                q(i.colour),
                q(i.mark),
                q(i.site),
                q(i.icon)
            )
        })
        .collect();
    let groups: Vec<String> = Group::ALL.iter().map(|g| format!("{{\"id\":{},\"name\":{}}}", q(g.id()), q(g.name()))).collect();
    format!("{{\"engines\":[{}],\"groups\":[{}],\"current\":{}}}", engines.join(","), groups.join(","), q(current.id()))
}

/// Fills a page's placeholders: theme, message prefix and engine data.
pub fn fill(page: &str, th: &Theme, engine: SearchEngine) -> String {
    let html = page
        .replacen("__PREFIX__", &web::js_string(web::APP_ACTION), 1)
        .replacen("__THEME__", &crate::welcome::colors(th), 1)
        .replacen("__DATA__", &data(engine), 1);
    format!("data:text/html;charset=utf-8,{}", percent_encode(&html))
}

/// The search engine guide for theme `th`, with `engine` in use.
pub fn page_url(th: &Theme, engine: SearchEngine) -> String {
    fill(PAGE, th, engine)
}

/// Script that tells a live start page or guide which engine is in use.
pub fn engine_script(engine: SearchEngine) -> String {
    format!("window.tbEngine&&tbEngine({})", web::js_string(engine.id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_is_described() {
        for e in SearchEngine::ALL {
            let i = info(e);
            assert!(!i.pitch.is_empty() && !i.about.is_empty() && !i.best_for.is_empty(), "{e:?}");
            assert!(i.colour.len() == 7 && i.colour.starts_with('#'), "{e:?}");
            assert!(!i.mark.is_empty() && i.mark.chars().count() <= 2, "{e:?}");
            assert!(i.icon.starts_with("https://"), "{e:?}");
        }
    }

    #[test]
    fn the_data_names_every_engine_and_the_current_one() {
        let d = data(SearchEngine::Qwant);
        for e in SearchEngine::ALL {
            assert!(d.contains(&format!("\"id\":\"{}\"", e.id())), "{}", e.id());
        }
        assert!(d.ends_with("\"current\":\"qwant\"}"), "{d}");
        let url = page_url(&Theme::default(), SearchEngine::Qwant);
        assert!(!url.contains("__DATA__") && !url.contains("__THEME__") && !url.contains("__PREFIX__"));
    }
}
