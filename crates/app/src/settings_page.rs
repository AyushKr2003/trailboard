//! The settings page (Ctrl+,): one of the app's own pages, kept on the board
//! as `scene::note::SETTINGS_URL` and built like the search engine guide.
//! Settings are grouped in sections with a short note under each; switches
//! turn on and off, short lists are buttons side by side and long ones a
//! drop-down. A box at the top filters them.
//!
//! The page's messages (after `web::APP_ACTION`) are [`SET`] `key=value`
//! (`scene::settings::Key::id` and an id of `Settings::choices`),
//! [`PROXY`] and the server typed, [`RUN`] and a name of [`command`], and
//! `start::GUIDE` for the search engine guide. The app sends the page its
//! data again with [`data_script`] whenever a setting changes.

use scene::settings::{Key, Settings};

use crate::note::percent_encode;
use crate::palette::Command;
use crate::theme::Theme;

const PAGE: &str = include_str!("settings.html");

/// Before `key=value`: a setting changed on the page.
pub const SET: &str = "set:";
/// Before the proxy server typed for *Proxy: Custom*.
pub const PROXY: &str = "proxy-server:";
/// Before the name of a board command the page has a button for.
pub const RUN: &str = "run:";

/// A section of the page: its id, name, the settings in it and buttons
/// (label, [`command`] name) under them.
struct Section {
    id: &'static str,
    name: &'static str,
    keys: &'static [Key],
    buttons: &'static [(&'static str, &'static str)],
}

const SECTIONS: &[Section] = &[
    Section {
        id: "search",
        name: "Search and new cards",
        keys: &[Key::Search, Key::NewCard],
        buttons: &[],
    },
    Section {
        id: "appearance",
        name: "Appearance",
        keys: &[Key::PageColours, Key::DefaultZoom, Key::ScrollSpeed],
        buttons: &[("Keyboard shortcuts", "shortcuts")],
    },
    Section {
        id: "privacy",
        name: "Privacy",
        keys: &[Key::BlockAds, Key::ThirdPartyCookies, Key::DoNotTrack, Key::History],
        buttons: &[("Sites with ads allowed", "allowed-sites"), ("History", "history"), ("Clear history…", "clear-history"), ("Clear cookies…", "clear-cookies")],
    },
    Section {
        id: "content",
        name: "Site content",
        keys: &[Key::JavaScript, Key::Images],
        buttons: &[],
    },
    Section {
        id: "permissions",
        name: "Permissions",
        keys: &[Key::Camera, Key::Microphone, Key::Location, Key::Notifications],
        buttons: &[],
    },
    Section {
        id: "languages",
        name: "Languages",
        keys: &[Key::Language, Key::Spellcheck],
        buttons: &[],
    },
    Section {
        id: "downloads",
        name: "Downloads",
        keys: &[Key::DownloadTo],
        buttons: &[("Show downloads", "downloads")],
    },
    Section {
        id: "system",
        name: "Performance and network",
        keys: &[Key::LivePages, Key::Proxy],
        buttons: &[],
    },
];

/// The board command a page button names.
pub fn command(name: &str) -> Option<Command> {
    Some(match name {
        "shortcuts" => Command::Shortcuts,
        "allowed-sites" => Command::AllowedSites,
        "history" => Command::History,
        "clear-history" => Command::ClearHistory,
        "clear-cookies" => Command::ClearCookies,
        "downloads" => Command::Downloads,
        _ => return None,
    })
}

/// The line under a setting: what it does.
fn help(key: Key) -> &'static str {
    match key {
        Key::Search => "Where the address bar, the palette and start pages send a search.",
        Key::NewCard => "What Ctrl+T opens: the floating address bar, or an empty card with a search box.",
        Key::PageColours => "Whether pages are told to use their light or dark look.",
        Key::DefaultZoom => "Page zoom on sites you haven't zoomed yourself (Ctrl + and Ctrl −).",
        Key::ScrollSpeed => "How far a wheel notch or a touchpad swipe scrolls a page.",
        Key::BlockAds => "Stop ads and trackers loading. You can allow a site from its page menu.",
        Key::ThirdPartyCookies => "Refuse cookies set by other sites than the one you're on. Some sign-ins may break.",
        Key::DoNotTrack => "Send Do Not Track and Global Privacy Control with every request.",
        Key::History => "Remember the pages you open, with their text, so you can find them again.",
        Key::JavaScript => "Let sites run scripts. Change it for one site from its page menu.",
        Key::Images => "Show pictures on pages. Change it for one site from its page menu.",
        Key::Camera => "What a site gets when it asks to use your camera.",
        Key::Microphone => "What a site gets when it asks to use your microphone.",
        Key::Location => "What a site gets when it asks where you are.",
        Key::Notifications => "What a site gets when it asks to show notifications.",
        Key::Language => "The language sites are asked for, and the dictionary for spelling. Sites see a change after a restart.",
        Key::Spellcheck => "Underline misspelt words as you type and suggest fixes.",
        Key::DownloadTo => "Where files you download are saved.",
        Key::LivePages => "Cards that keep a running page. The rest sleep and wake when you go to them; fewer saves memory.",
        Key::Proxy => "How pages reach the internet: as the system says, directly, or through a server you give.",
    }
}

/// The page's data: sections with each setting's label, note, kind, choices
/// and value; the proxy server; and the facts it shows (label, value, more).
/// `notes` add a word after a setting's label (where its value comes from).
pub fn data(s: &Settings, notes: &[(Key, String)], facts: &[(&str, String, String)]) -> String {
    let q = web::js_string;
    let sections: Vec<String> = SECTIONS
        .iter()
        .map(|sec| {
            let items: Vec<String> = sec
                .keys
                .iter()
                .map(|&k| {
                    let choices = s.choices(k);
                    let kind = if k.is_switch() {
                        "switch"
                    } else if choices.len() <= 4 {
                        "segment"
                    } else {
                        "select"
                    };
                    let choices: Vec<String> = choices.iter().map(|(id, name)| format!("[{},{}]", q(id), q(name))).collect();
                    let note = notes.iter().find(|(nk, _)| *nk == k).map_or(String::new(), |(_, n)| n.clone());
                    format!(
                        "{{\"key\":{},\"label\":{},\"help\":{},\"kind\":{},\"choices\":[{}],\"value\":{},\"note\":{},\"words\":{}}}",
                        q(k.id()),
                        q(k.label()),
                        q(help(k)),
                        q(kind),
                        choices.join(","),
                        q(&s.choice(k)),
                        q(&note),
                        q(&k.words())
                    )
                })
                .collect();
            let buttons: Vec<String> = sec.buttons.iter().map(|(label, cmd)| format!("[{},{}]", q(label), q(cmd))).collect();
            format!("{{\"id\":{},\"name\":{},\"items\":[{}],\"buttons\":[{}]}}", q(sec.id), q(sec.name), items.join(","), buttons.join(","))
        })
        .collect();
    let facts: Vec<String> = facts.iter().map(|(l, v, more)| format!("[{},{},{}]", q(l), q(v), q(more))).collect();
    format!("{{\"sections\":[{}],\"proxyServer\":{},\"facts\":[{}]}}", sections.join(","), q(&s.proxy_server), facts.join(","))
}

/// The settings page for theme `th`, showing `data` (from [`data`]).
pub fn page_url(th: &Theme, data: &str) -> String {
    let html = PAGE
        .replacen("__PREFIX__", &web::js_string(web::APP_ACTION), 1)
        .replacen("__THEME__", &crate::welcome::colors(th), 1)
        .replacen("__DATA__", data, 1);
    format!("data:text/html;charset=utf-8,{}", percent_encode(&html))
}

/// Script that shows a live settings page new data.
pub fn data_script(data: &str) -> String {
    format!("window.tbSettings&&tbSettings({data})")
}

/// A `set:` message's setting and value.
pub fn parse_set(msg: &str) -> Option<(Key, &str)> {
    let (key, value) = msg.split_once('=')?;
    Some((Key::from_id(key)?, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_is_in_one_section() {
        for k in Key::ALL {
            let n = SECTIONS.iter().filter(|s| s.keys.contains(&k)).count();
            assert_eq!(n, 1, "{k:?}");
            assert!(!help(k).is_empty());
        }
        for sec in SECTIONS {
            for (_, cmd) in sec.buttons {
                assert!(command(cmd).is_some(), "{cmd}");
            }
        }
    }

    #[test]
    fn the_page_gets_its_data_and_sends_known_messages() {
        let s = Settings::default();
        let d = data(&s, &[(Key::LivePages, "TRAILBOARD_MAX_LIVE".into())], &[("Sandbox", "On".into(), "More".into())]);
        assert!(d.contains("\"key\":\"search\"") && d.contains("\"value\":\"duckduckgo\""), "{d}");
        assert!(d.contains("\"note\":\"TRAILBOARD_MAX_LIVE\""));
        assert!(d.contains("\"kind\":\"switch\"") && d.contains("\"kind\":\"segment\"") && d.contains("\"kind\":\"select\""));
        let url = page_url(&Theme::default(), &d);
        assert!(!url.contains("__DATA__") && !url.contains("__THEME__") && !url.contains("__PREFIX__"));
        for msg in PAGE.split("act(\"").skip(1).filter_map(|s| s.split('"').next()) {
            assert!([SET, PROXY, RUN, crate::start::GUIDE].contains(&msg), "unknown message {msg}");
        }
        let rounded = Theme { rounding: 10.0, ..Theme::default() };
        assert!(crate::welcome::colors(&rounded).contains("\"radius\":\"10px\""), "corners follow Hyprland");
        assert_eq!(parse_set("block_ads=off"), Some((Key::BlockAds, "off")));
        assert_eq!(parse_set("nope=off"), None);
        assert_eq!(parse_set("block_ads"), None);
    }
}
