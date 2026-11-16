//! Per-site exceptions to the JavaScript and images settings: a site can have
//! either turned on or off whatever the default (`settings::Settings`). Kept
//! under the same site names as page zoom (`pagezoom::site_of`). Saved by
//! `storage::sitesettings`.

use std::collections::BTreeMap;

use crate::pagezoom::site_of;

/// What one site overrides; `None` follows the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SiteRule {
    pub javascript: Option<bool>,
    pub images: Option<bool>,
}

impl SiteRule {
    fn is_empty(&self) -> bool {
        self.javascript.is_none() && self.images.is_none()
    }
}

/// Which of a site's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    JavaScript,
    Images,
}

impl Content {
    pub fn name(self) -> &'static str {
        match self {
            Content::JavaScript => "JavaScript",
            Content::Images => "Images",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteSettings {
    pub sites: BTreeMap<String, SiteRule>,
}

impl SiteSettings {
    fn slot(rule: &mut SiteRule, what: Content) -> &mut Option<bool> {
        match what {
            Content::JavaScript => &mut rule.javascript,
            Content::Images => &mut rule.images,
        }
    }

    /// The site's own choice for `what`, if it has one.
    pub fn rule(&self, url: &str, what: Content) -> Option<bool> {
        let r = self.sites.get(&site_of(url))?;
        match what {
            Content::JavaScript => r.javascript,
            Content::Images => r.images,
        }
    }

    /// Whether `what` is on for the page at `url`, given the `default`.
    pub fn allows(&self, url: &str, what: Content, default: bool) -> bool {
        self.rule(url, what).unwrap_or(default)
    }

    /// Turns `what` on or off for `url`'s site. A choice equal to `default`
    /// removes the exception (so the site follows the default from then on).
    /// Returns whether anything changed.
    pub fn set(&mut self, url: &str, what: Content, on: bool, default: bool) -> bool {
        let site = site_of(url);
        if site.is_empty() {
            return false;
        }
        let before = self.sites.get(&site).copied().unwrap_or_default();
        let mut rule = before;
        *Self::slot(&mut rule, what) = (on != default).then_some(on);
        if rule == before {
            return false;
        }
        if rule.is_empty() {
            self.sites.remove(&site);
        } else {
            self.sites.insert(site, rule);
        }
        true
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
    }
}
