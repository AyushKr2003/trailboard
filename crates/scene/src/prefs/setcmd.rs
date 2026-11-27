//! Settings from the command line (M12 step 11): `:set name value`,
//! `:set name` (shows it), `:reset name` and `:toggle name [values…]`, and
//! the per-site switches of `:site` / `:site-session`. Names are the
//! settings file's, with dashes (`page-colours`); values are a choice's id
//! or name (`dark`, `DuckDuckGo`), or a unique start of one. The app turns
//! a [`Change`] into the same call the settings page makes.

use crate::settings::{Key, Settings, MAX_CORNER_RADIUS, ROUNDED_RADIUS};
use crate::sitesettings::Content;

/// A setting `:set` reaches: a row of the settings page, or one of the
/// values typed under a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Key(Key),
    /// The server for *Proxy: Custom*.
    ProxyServer,
    /// The radius for *Corners: Custom*.
    CornerRadius,
    /// The search keywords (shown and reset only; the settings page edits them).
    Keywords,
}

/// Every setting, in the settings page's order.
pub fn all() -> Vec<Setting> {
    let mut out = Vec::new();
    for k in Key::ALL {
        out.push(Setting::Key(k));
        match k {
            Key::Search => out.push(Setting::Keywords),
            Key::Proxy => out.push(Setting::ProxyServer),
            Key::Corners => out.push(Setting::CornerRadius),
            _ => {}
        }
    }
    out
}

impl Setting {
    /// Its name on the command line.
    pub fn name(self) -> String {
        match self {
            Setting::Key(k) => k.id().replace('_', "-"),
            Setting::ProxyServer => "proxy-server".into(),
            Setting::CornerRadius => "corner-radius".into(),
            Setting::Keywords => "keywords".into(),
        }
    }

    /// Its name on the settings page.
    pub fn label(self) -> &'static str {
        match self {
            Setting::Key(k) => k.label(),
            Setting::ProxyServer => "Proxy server",
            Setting::CornerRadius => "Corner radius",
            Setting::Keywords => "Search keywords",
        }
    }

    /// What it is set to, as shown.
    pub fn value(self, s: &Settings) -> String {
        match self {
            Setting::Key(k) => s.value(k),
            Setting::ProxyServer if s.proxy_server.is_empty() => "none".into(),
            Setting::ProxyServer => s.proxy_server.clone(),
            Setting::CornerRadius => format!("{} px", s.corner_radius),
            Setting::Keywords => match s.keywords.len() {
                1 => "1 keyword".into(),
                n => format!("{n} keywords"),
            },
        }
    }

    /// `Page colours: Dark`, for a toast.
    pub fn shown(self, s: &Settings) -> String {
        format!("{}: {}", self.label(), self.value(s))
    }
}

/// What to do to the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Set a row to the choice with this id (`Settings::choose`).
    Choose(Key, String),
    ProxyServer(String),
    /// The radius, and *Corners* to *Custom*.
    CornerRadius(u8),
    /// The search keywords back to the defaults.
    DefaultKeywords,
}

fn norm(name: &str) -> String {
    name.trim().to_lowercase().replace('_', "-")
}

/// The setting `name` stands for: a name exactly (case ignored, `_` or
/// `-`), else the one name it starts.
pub fn find(name: &str) -> Result<Setting, String> {
    let n = norm(name);
    if n.is_empty() {
        return Err("Name a setting".into());
    }
    let every = all();
    if let Some(s) = every.iter().find(|s| s.name() == n) {
        return Ok(*s);
    }
    let starts: Vec<&Setting> = every.iter().filter(|s| s.name().starts_with(&n)).collect();
    match starts.as_slice() {
        [one] => Ok(**one),
        [] => Err(format!("No setting “{}”", name.trim())),
        many => Err(format!("Could be {}", many.iter().map(|s| s.name()).collect::<Vec<_>>().join(", "))),
    }
}

/// The choice id `text` stands for in `key`'s choices: an id or a name
/// exactly (case ignored), for a switch also yes/no/true/false/1/0, for a
/// number one in range (`125%`); else the one id or name it starts.
pub fn value_id(s: &Settings, key: Key, text: &str) -> Result<String, String> {
    let t = text.trim().to_lowercase();
    if key.is_switch() {
        match t.as_str() {
            "on" | "yes" | "true" | "1" => return Ok("on".into()),
            "off" | "no" | "false" | "0" => return Ok("off".into()),
            _ => {}
        }
    }
    if matches!(key, Key::DefaultZoom | Key::LivePages) {
        let n = t.trim_end_matches('%').trim();
        if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
            let mut probe = s.clone();
            probe.choose(key, n);
            let id = n.trim_start_matches('0').to_string();
            if probe.choice(key) == id {
                return Ok(id);
            }
            let range = if key == Key::DefaultZoom { "25–500%" } else { "1–64" };
            return Err(format!("{}: {} isn't {range}", key.label(), text.trim()));
        }
    }
    let choices = s.choices(key);
    if let Some((id, _)) = choices.iter().find(|(id, name)| id.to_lowercase() == t || name.to_lowercase() == t) {
        return Ok(id.clone());
    }
    let mut starts: Vec<&String> = choices
        .iter()
        .filter(|(id, name)| !t.is_empty() && (id.to_lowercase().starts_with(&t) || name.to_lowercase().starts_with(&t)))
        .map(|(id, _)| id)
        .collect();
    starts.dedup();
    match starts.as_slice() {
        [one] => Ok((*one).clone()),
        _ => {
            let ids: Vec<&str> = choices.iter().map(|(id, _)| id.as_str()).collect();
            let list = if ids.len() <= 8 { format!("one of {}", ids.join(", ")) } else { "a choice (Tab lists them)".into() };
            Err(format!("{}: “{}” isn't {list}", key.label(), text.trim()))
        }
    }
}

/// `:set setting text`.
pub fn set(s: &Settings, setting: Setting, text: &str) -> Result<Change, String> {
    match setting {
        Setting::Key(k) => value_id(s, k, text).map(|id| Change::Choose(k, id)),
        Setting::ProxyServer => Ok(Change::ProxyServer(text.trim().to_string())),
        Setting::CornerRadius => match text.trim().trim_end_matches("px").trim().parse::<u8>() {
            Ok(px) if px <= MAX_CORNER_RADIUS => Ok(Change::CornerRadius(px)),
            _ => Err(format!("Corner radius: “{}” isn't 0–{MAX_CORNER_RADIUS} px", text.trim())),
        },
        Setting::Keywords => Err("Search keywords are edited in Settings (Ctrl+,); :reset keywords restores the defaults".into()),
    }
}

/// `:reset setting`: back to the default.
pub fn reset(setting: Setting) -> Change {
    let d = Settings::default();
    match setting {
        Setting::Key(k) => Change::Choose(k, d.choice(k)),
        Setting::ProxyServer => Change::ProxyServer(String::new()),
        Setting::CornerRadius => Change::CornerRadius(ROUNDED_RADIUS as u8),
        Setting::Keywords => Change::DefaultKeywords,
    }
}

/// `:toggle setting [values…]`. With no values a switch flips and a list
/// goes to its next choice; with one value it goes between that and the
/// default; with several, to the one after the current (the first if the
/// current isn't among them).
pub fn toggle(s: &Settings, setting: Setting, values: &[&str]) -> Result<Change, String> {
    let Setting::Key(key) = setting else {
        return Err(format!("{} can't be toggled; use :set", setting.label()));
    };
    let now = s.choice(key);
    let next = match values {
        [] => {
            let mut probe = s.clone();
            probe.step(key, 1);
            probe.choice(key)
        }
        [one] => {
            let id = value_id(s, key, one)?;
            if id == now { Settings::default().choice(key) } else { id }
        }
        many => {
            let ids = many.iter().map(|v| value_id(s, key, v)).collect::<Result<Vec<_>, _>>()?;
            match ids.iter().position(|id| *id == now) {
                Some(i) => ids[(i + 1) % ids.len()].clone(),
                None => ids[0].clone(),
            }
        }
    };
    Ok(Change::Choose(key, next))
}

/// `td`: pages dark if they are light now, else light, whatever
/// *Page colours* says (`theme_light`: whether the theme is light).
pub fn dark_pages(s: &Settings, theme_light: bool) -> Change {
    let dark = s.page_colours.dark(theme_light);
    Change::Choose(Key::PageColours, if dark { "light" } else { "dark" }.into())
}

/// The command line's rows for setting names: (name, what it is now).
pub fn name_rows(s: &Settings) -> Vec<(String, String)> {
    all().into_iter().map(|x| (x.name(), x.shown(s))).collect()
}

/// The command line's rows for `setting`'s values: (id, name), the current
/// one saying so.
pub fn value_rows(s: &Settings, setting: Setting) -> Vec<(String, String)> {
    let Setting::Key(k) = setting else { return Vec::new() };
    let now = s.choice(k);
    s.choices(k)
        .into_iter()
        .map(|(id, name)| {
            let detail = if id == now { format!("{name} · now") } else { name };
            (id, detail)
        })
        .collect()
}

/// What `:site` / `:site-session` and the `t` keys switch for a site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SiteSwitch {
    Content(Content),
    /// Blocking ads and trackers there (`privacy::AllowedSites`).
    Ads,
}

impl SiteSwitch {
    pub const ALL: [SiteSwitch; 5] = [
        SiteSwitch::Content(Content::JavaScript),
        SiteSwitch::Content(Content::Images),
        SiteSwitch::Content(Content::Cookies),
        SiteSwitch::Ads,
        SiteSwitch::Content(Content::Passthrough),
    ];

    pub fn id(self) -> &'static str {
        match self {
            SiteSwitch::Content(c) => c.id(),
            SiteSwitch::Ads => "ads",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SiteSwitch::Content(c) => c.name(),
            SiteSwitch::Ads => "Ads",
        }
    }
}

/// The words `:site` completes: the switches, then on / off.
pub const SITE_WORDS: [&str; 5] = ["javascript", "images", "cookies", "ads", "passthrough"];

/// Reads `:site`'s argument: a switch (a unique start is enough) and
/// optionally `on` / `off` (else it flips). For ads, `on` means ads are
/// shown, that is blocking is off there.
pub fn parse_site(arg: &str) -> Result<(SiteSwitch, Option<bool>), String> {
    let mut words = arg.split_whitespace();
    let mut what = words.next().unwrap_or("").to_lowercase();
    if what == "js" {
        what = "javascript".into();
    }
    let mut found = SiteSwitch::ALL.into_iter().filter(|s| !what.is_empty() && s.id().starts_with(&what));
    let switch = match (found.next(), found.next()) {
        (Some(s), None) => s,
        _ => return Err(format!("site: “{what}” isn't one of {}", SITE_WORDS.join(", "))),
    };
    let on = match words.next().map(str::to_lowercase).as_deref() {
        None => None,
        Some("on" | "yes" | "true" | "1") => Some(true),
        Some("off" | "no" | "false" | "0") => Some(false),
        Some(other) => return Err(format!("site: “{other}” isn't on or off")),
    };
    if let Some(extra) = words.next() {
        return Err(format!("site: “{extra}” after on/off"));
    }
    Ok((switch, on))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_found_by_name_or_start() {
        assert_eq!(find("page-colours"), Ok(Setting::Key(Key::PageColours)));
        assert_eq!(find("PAGE_COLOURS"), Ok(Setting::Key(Key::PageColours)));
        assert_eq!(find("pag"), Ok(Setting::Key(Key::PageColours)));
        assert_eq!(find("proxy"), Ok(Setting::Key(Key::Proxy)), "exact beats proxy-server");
        assert_eq!(find("proxy-s"), Ok(Setting::ProxyServer));
        assert_eq!(find("board-background"), Ok(Setting::Key(Key::Backdrop)));
        assert!(find("s").unwrap_err().starts_with("Could be search, "));
        assert_eq!(find("nope"), Err("No setting “nope”".into()));
        assert!(find(" ").is_err());
        // Every name is unique and found by itself.
        for s in all() {
            assert_eq!(find(&s.name()), Ok(s), "{}", s.name());
        }
    }

    #[test]
    fn values_by_id_name_start_or_number() {
        let s = Settings::default();
        assert_eq!(set(&s, Setting::Key(Key::PageColours), "dark"), Ok(Change::Choose(Key::PageColours, "dark".into())));
        assert_eq!(value_id(&s, Key::PageColours, "Follow the theme"), Ok("theme".into()));
        assert_eq!(value_id(&s, Key::PageColours, "fol"), Ok("theme".into()), "a start of a name");
        assert_eq!(value_id(&s, Key::Search, "duck"), Ok("duckduckgo".into()));
        assert_eq!(value_id(&s, Key::Search, "brave search"), Ok("brave".into()));
        assert!(value_id(&s, Key::Search, "m").unwrap_err().contains("Tab lists them"), "mojeek, metager");
        assert_eq!(value_id(&s, Key::History, "no"), Ok("off".into()));
        assert_eq!(value_id(&s, Key::History, "ON"), Ok("on".into()));
        assert_eq!(value_id(&s, Key::DefaultZoom, "125%"), Ok("125".into()));
        assert_eq!(value_id(&s, Key::DefaultZoom, "130"), Ok("130".into()), "between the choices is fine");
        assert_eq!(value_id(&s, Key::DefaultZoom, "900"), Err("Default page zoom: 900 isn't 25–500%".into()));
        assert_eq!(value_id(&s, Key::LivePages, "3"), Ok("3".into()));
        assert_eq!(value_id(&s, Key::Language, "en-gb"), Ok("en-GB".into()));
        assert_eq!(
            value_id(&s, Key::PageColours, "pink"),
            Err("Page colours: “pink” isn't one of theme, light, dark".into())
        );
        assert_eq!(set(&s, Setting::CornerRadius, "12px"), Ok(Change::CornerRadius(12)));
        assert!(set(&s, Setting::CornerRadius, "99").is_err());
        assert_eq!(set(&s, Setting::ProxyServer, " h:1 "), Ok(Change::ProxyServer("h:1".into())));
        assert!(set(&s, Setting::Keywords, "x").is_err());
    }

    #[test]
    fn reset_and_shown() {
        assert_eq!(reset(Setting::Key(Key::Search)), Change::Choose(Key::Search, "duckduckgo".into()));
        assert_eq!(reset(Setting::Key(Key::LivePages)), Change::Choose(Key::LivePages, "6".into()));
        assert_eq!(reset(Setting::CornerRadius), Change::CornerRadius(10));
        assert_eq!(reset(Setting::Keywords), Change::DefaultKeywords);
        let s = Settings::default();
        assert_eq!(Setting::Key(Key::PageColours).shown(&s), "Page colours: Follow the theme");
        assert_eq!(Setting::ProxyServer.shown(&s), "Proxy server: none");
        assert_eq!(Setting::Keywords.shown(&s), "Search keywords: 16 keywords");
    }

    #[test]
    fn toggles() {
        let mut s = Settings::default();
        let pc = Setting::Key(Key::PageColours);
        assert_eq!(toggle(&s, Setting::Key(Key::History), &[]), Ok(Change::Choose(Key::History, "off".into())));
        assert_eq!(toggle(&s, pc, &[]), Ok(Change::Choose(Key::PageColours, "light".into())), "the next choice");
        // One value: it, then the default again.
        assert_eq!(toggle(&s, pc, &["dark"]), Ok(Change::Choose(Key::PageColours, "dark".into())));
        s.choose(Key::PageColours, "dark");
        assert_eq!(toggle(&s, pc, &["dark"]), Ok(Change::Choose(Key::PageColours, "theme".into())));
        // Several: round them; the first if the current isn't one.
        assert_eq!(toggle(&s, pc, &["light", "dark"]), Ok(Change::Choose(Key::PageColours, "light".into())));
        s.choose(Key::PageColours, "theme");
        assert_eq!(toggle(&s, pc, &["light", "dark"]), Ok(Change::Choose(Key::PageColours, "light".into())));
        assert!(toggle(&s, pc, &["light", "pink"]).is_err());
        assert!(toggle(&s, Setting::ProxyServer, &[]).is_err());
    }

    #[test]
    fn dark_pages_flips_what_shows() {
        let mut s = Settings::default();
        // Following a dark theme, pages are dark: go light.
        assert_eq!(dark_pages(&s, false), Change::Choose(Key::PageColours, "light".into()));
        assert_eq!(dark_pages(&s, true), Change::Choose(Key::PageColours, "dark".into()));
        s.choose(Key::PageColours, "light");
        assert_eq!(dark_pages(&s, false), Change::Choose(Key::PageColours, "dark".into()));
    }

    #[test]
    fn rows() {
        let s = Settings::default();
        let names = name_rows(&s);
        assert_eq!(names.len(), all().len());
        assert!(names.contains(&("page-colours".into(), "Page colours: Follow the theme".into())));
        let v = value_rows(&s, Setting::Key(Key::PageColours));
        assert_eq!(v[0], ("theme".into(), "Follow the theme · now".into()));
        assert_eq!(v[2], ("dark".into(), "Dark".into()));
        assert!(value_rows(&s, Setting::ProxyServer).is_empty());
    }

    #[test]
    fn site_arguments() {
        assert_eq!(parse_site("javascript"), Ok((SiteSwitch::Content(Content::JavaScript), None)));
        assert_eq!(parse_site("j off"), Ok((SiteSwitch::Content(Content::JavaScript), Some(false))));
        assert_eq!(parse_site("COOKIES on"), Ok((SiteSwitch::Content(Content::Cookies), Some(true))));
        assert_eq!(parse_site("a"), Ok((SiteSwitch::Ads, None)));
        assert_eq!(parse_site("p on"), Ok((SiteSwitch::Content(Content::Passthrough), Some(true))));
        assert_eq!(parse_site("js on"), Ok((SiteSwitch::Content(Content::JavaScript), Some(true))));
        assert!(parse_site("").is_err());
        assert!(parse_site("x").is_err());
        assert!(parse_site("images maybe").is_err());
        assert!(parse_site("images on now").is_err());
    }
}
