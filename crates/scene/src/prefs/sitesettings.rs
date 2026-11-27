//! Per-site exceptions to the JavaScript, images and cookies settings: a
//! site can have each turned on or off whatever the default
//! (`settings::Settings`; cookies are on by default). A site can also start
//! in Passthrough (all keys to the page; off by default). Kept under the same
//! site names as page zoom (`pagezoom::site_of`). Saved by
//! `storage::sitesettings`; a second set, [`SiteSettings::over`], holds the
//! rules made for this session only (`:site-session`, `tJ`…), which win
//! over the saved ones and are never saved.

use std::collections::BTreeMap;

use crate::pagezoom::site_of;

/// What one site overrides; `None` follows the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SiteRule {
    pub javascript: Option<bool>,
    pub images: Option<bool>,
    pub cookies: Option<bool>,
    pub passthrough: Option<bool>,
}

impl SiteRule {
    pub fn is_empty(&self) -> bool {
        Content::ALL.iter().all(|&w| self.get(w).is_none())
    }

    fn get(&self, what: Content) -> Option<bool> {
        match what {
            Content::JavaScript => self.javascript,
            Content::Images => self.images,
            Content::Cookies => self.cookies,
            Content::Passthrough => self.passthrough,
        }
    }

    fn slot(&mut self, what: Content) -> &mut Option<bool> {
        match what {
            Content::JavaScript => &mut self.javascript,
            Content::Images => &mut self.images,
            Content::Cookies => &mut self.cookies,
            Content::Passthrough => &mut self.passthrough,
        }
    }
}

/// Which of a site's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Content {
    JavaScript,
    Images,
    Cookies,
    /// A card of the site starts in Passthrough when it is given the keys.
    Passthrough,
}

impl Content {
    pub const ALL: [Content; 4] = [Content::JavaScript, Content::Images, Content::Cookies, Content::Passthrough];

    pub fn name(self) -> &'static str {
        match self {
            Content::JavaScript => "JavaScript",
            Content::Images => "Images",
            Content::Cookies => "Cookies",
            Content::Passthrough => "Passthrough",
        }
    }

    /// Its word on the command line (`:site javascript off`).
    pub fn id(self) -> &'static str {
        match self {
            Content::JavaScript => "javascript",
            Content::Images => "images",
            Content::Cookies => "cookies",
            Content::Passthrough => "passthrough",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteSettings {
    pub sites: BTreeMap<String, SiteRule>,
}

impl SiteSettings {
    /// The site's own choice for `what`, if it has one.
    pub fn rule(&self, url: &str, what: Content) -> Option<bool> {
        self.sites.get(&site_of(url))?.get(what)
    }

    /// Whether `what` is on for the page at `url`, given the `default`.
    pub fn allows(&self, url: &str, what: Content, default: bool) -> bool {
        self.rule(url, what).unwrap_or(default)
    }

    /// Turns `what` on or off for `url`'s site. A choice equal to `default`
    /// removes the exception (so the site follows the default from then on).
    /// Returns whether anything changed.
    pub fn set(&mut self, url: &str, what: Content, on: bool, default: bool) -> bool {
        self.put(url, what, (on != default).then_some(on))
    }

    /// Sets `url`'s site's own choice for `what` as given (`None`: none).
    /// Returns whether anything changed.
    pub fn put(&mut self, url: &str, what: Content, rule: Option<bool>) -> bool {
        let site = site_of(url);
        if site.is_empty() {
            return false;
        }
        let before = self.sites.get(&site).copied().unwrap_or_default();
        let mut r = before;
        *r.slot(what) = rule;
        if r == before {
            return false;
        }
        if r.is_empty() {
            self.sites.remove(&site);
        } else {
            self.sites.insert(site, r);
        }
        true
    }

    /// These rules over `base`: a site's choice here wins, else `base`'s.
    pub fn over(&self, base: &SiteSettings) -> SiteSettings {
        let mut out = base.clone();
        for (site, r) in &self.sites {
            let e = out.sites.entry(site.clone()).or_default();
            for what in Content::ALL {
                if let Some(v) = r.get(what) {
                    *e.slot(what) = Some(v);
                }
            }
        }
        out
    }

    /// The sites whose own choice for `what` is `on`.
    pub fn sites_with(&self, what: Content, on: bool) -> impl Iterator<Item = &str> {
        self.sites.iter().filter(move |(_, r)| r.get(what) == Some(on)).map(|(s, _)| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exceptions_cover_the_default_per_site() {
        let mut s = SiteSettings::default();
        assert!(s.allows("https://a.com/x", Content::JavaScript, true));
        assert!(!s.allows("https://a.com/x", Content::JavaScript, false));
        assert!(s.set("https://www.a.com/", Content::JavaScript, false, true));
        assert!(!s.allows("https://a.com/other", Content::JavaScript, true));
        assert!(s.allows("https://a.com/other", Content::Images, true), "images untouched");
        assert!(s.allows("https://b.com/", Content::JavaScript, true), "other sites untouched");
        assert!(!s.set("https://a.com/", Content::JavaScript, false, true), "no change");
        // Back to the default: the exception goes.
        assert!(s.set("https://a.com/", Content::JavaScript, true, true));
        assert!(s.sites.is_empty());
        // An exception that turns something on while the default is off.
        assert!(s.set("https://c.com/", Content::Images, true, false));
        assert_eq!(s.rule("https://c.com/", Content::Images), Some(true));
        assert!(s.allows("https://c.com/", Content::Images, false));
        assert!(!s.set("nonsense", Content::Images, false, true));
        assert!(s.set("https://c.com/", Content::Cookies, false, true));
        assert_eq!(s.sites_with(Content::Cookies, false).collect::<Vec<_>>(), ["c.com"]);
    }

    #[test]
    fn session_rules_win_over_saved_ones() {
        let mut saved = SiteSettings::default();
        saved.set("https://a.com/", Content::JavaScript, false, true);
        saved.set("https://a.com/", Content::Images, false, true);
        let mut session = SiteSettings::default();
        // A session rule equal to the default still overrides a saved one.
        assert!(session.put("https://a.com/", Content::JavaScript, Some(true)));
        session.put("https://b.com/", Content::Cookies, Some(false));
        let all = session.over(&saved);
        assert_eq!(all.rule("https://a.com/", Content::JavaScript), Some(true));
        assert_eq!(all.rule("https://a.com/", Content::Images), Some(false), "saved kept");
        assert_eq!(all.rule("https://b.com/", Content::Cookies), Some(false));
        assert!(session.put("https://a.com/", Content::JavaScript, None));
        assert_eq!(session.sites.len(), 1);
    }
}
