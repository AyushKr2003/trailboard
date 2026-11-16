//! Page zoom (Ctrl +/−/0 inside a page): the zoom steps, and the site a zoom
//! belongs to. Like desktop browsers, a zoom belongs to a site, not to one
//! card, so every card on that site shares it (Chromium keeps zoom per host
//! anyway, so per-card zooms of one site would fight).

use std::collections::BTreeMap;

/// Zoom factors Ctrl +/− step through, as in desktop browsers.
pub const STEPS: [f64; 17] =
    [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0];

/// The next step up (`dir > 0`) or down from `factor`. A factor between
/// steps goes to the neighbouring step in that direction; the ends stay put.
pub fn step(factor: f64, dir: i32) -> f64 {
    const EPS: f64 = 1e-6;
    if dir > 0 {
        STEPS.iter().copied().find(|&s| s > factor + EPS).unwrap_or(STEPS[STEPS.len() - 1])
    } else if dir < 0 {
        STEPS.iter().rev().copied().find(|&s| s < factor - EPS).unwrap_or(STEPS[0])
    } else {
        factor
    }
}

/// "110%".
pub fn label(factor: f64) -> String {
    format!("{}%", (factor * 100.0).round() as i64)
}

/// The site a URL's zoom is kept under: its host (with port) lowercased, a
/// leading `www.` dropped, or the scheme for URLs without one (`file`, `data`, `about`).
pub fn site_of(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once(':') else { return String::new() };
    let scheme = scheme.to_ascii_lowercase();
    let Some(rest) = rest.strip_prefix("//") else { return scheme };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("").to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host.is_empty() { scheme } else { host.to_string() }
}

/// Zoom per site. Sites at the default zoom are not kept.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteZooms {
    /// The zooms that are saved.
    pub sites: BTreeMap<String, f64>,
    /// While a private space is shown: its own zooms, over `sites`. Changes
    /// go here (the default too, to override a saved zoom) and are never saved.
    pub private: Option<BTreeMap<String, f64>>,
    /// The zoom of sites without one of their own (the *Default page zoom*
    /// setting). Not saved here.
    pub default: f64,
}

impl Default for SiteZooms {
    fn default() -> Self {
        SiteZooms { sites: BTreeMap::new(), private: None, default: 1.0 }
    }
}

impl SiteZooms {
    pub fn get(&self, url: &str) -> f64 {
        let site = site_of(url);
        self.private.as_ref().and_then(|p| p.get(&site)).or_else(|| self.sites.get(&site)).copied().unwrap_or(self.default)
    }

    /// Sets the zoom of `url`'s site; returns whether it changed.
    pub fn set(&mut self, url: &str, factor: f64) -> bool {
        let site = site_of(url);
        let factor = factor.clamp(STEPS[0], STEPS[STEPS.len() - 1]);
        if (self.get(url) - factor).abs() < 1e-6 {
            return false;
        }
        if let Some(p) = self.private.as_mut() {
            p.insert(site, factor);
        } else if (factor - self.default).abs() < 1e-6 {
            self.sites.remove(&site);
        } else {
            self.sites.insert(site, factor);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_up_and_down() {
        assert_eq!(step(1.0, 1), 1.1);
        assert_eq!(step(1.0, -1), 0.9);
        assert_eq!(step(5.0, 1), 5.0);
        assert_eq!(step(0.25, -1), 0.25);
        // Between steps: the neighbour in that direction.
        assert_eq!(step(1.2, 1), 1.25);
        assert_eq!(step(1.2, -1), 1.1);
        assert_eq!(step(1.2, 0), 1.2);
        assert_eq!(label(1.1), "110%");
        assert_eq!(label(0.33), "33%");
    }

    #[test]
    fn sites() {
        assert_eq!(site_of("https://www.Example.com/a?b#c"), "example.com");
        assert_eq!(site_of("http://user:pw@localhost:8080/x"), "localhost:8080");
        assert_eq!(site_of("https://docs.rs"), "docs.rs");
        assert_eq!(site_of("file:///home/a.html"), "file");
        assert_eq!(site_of("about:blank"), "about");
        assert_eq!(site_of("data:text/html,hi"), "data");
        assert_eq!(site_of("nonsense"), "");
    }

    #[test]
    fn zooms_are_per_site_and_100_is_not_kept() {
        let mut z = SiteZooms::default();
        assert_eq!(z.get("https://a.com/x"), 1.0);
        assert!(z.set("https://a.com/x", 1.25));
        assert_eq!(z.get("https://www.a.com/other"), 1.25);
        assert_eq!(z.get("https://b.com/"), 1.0);
        assert!(!z.set("https://a.com/", 1.25));
        assert!(z.set("https://a.com/", 1.0));
        assert!(z.sites.is_empty());
        z.set("https://a.com/", 99.0);
        assert_eq!(z.get("https://a.com/"), 5.0);
    }

    #[test]
    fn sites_without_a_zoom_take_the_default() {
        let mut z = SiteZooms { default: 1.25, ..Default::default() };
        assert_eq!(z.get("https://a.com/"), 1.25);
        // 100% differs from the default now, so it is kept.
        assert!(z.set("https://a.com/", 1.0));
        assert_eq!(z.sites["a.com"], 1.0);
        assert_eq!(z.get("https://b.com/"), 1.25);
        // Back to the default: not kept.
        assert!(z.set("https://a.com/", 1.25));
        assert!(z.sites.is_empty());
    }

    #[test]
    fn private_zooms_cover_saved_ones_and_are_not_saved() {
        let mut z = SiteZooms::default();
        z.set("https://a.com/", 1.25);
        z.private = Some(BTreeMap::new());
        assert_eq!(z.get("https://a.com/"), 1.25, "saved zooms still apply");
        assert!(z.set("https://a.com/", 1.0));
        assert!(z.set("https://b.com/", 1.5));
        assert_eq!(z.get("https://a.com/"), 1.0);
        assert_eq!(z.get("https://b.com/"), 1.5);
        assert_eq!(z.sites.len(), 1, "nothing new to save");
        assert_eq!(z.sites["a.com"], 1.25);
        z.private = None;
        assert_eq!(z.get("https://a.com/"), 1.25);
        assert_eq!(z.get("https://b.com/"), 1.0);
    }
}
