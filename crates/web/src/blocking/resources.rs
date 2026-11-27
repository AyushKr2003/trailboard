//! The resources filter rules name: stand-ins served in place of a blocked
//! request (`$redirect=noopjs`, `$redirect=google-ima.js`) and scriptlets run
//! in a page (`##+js(set-constant, …)`). Without them a rule that names one
//! still blocks, but the page sees a script fail to load, which is exactly
//! what "turn off your ad blocker" walls look for.
//!
//! All of them are our own ([`SURROGATES`], [`SCRIPTLETS`] and the small
//! files beside them), written to the names and arguments the common filter
//! lists use. Each `//@ name alias …` line in the scripts starts a resource;
//! a `+name` token on it is a helper the resource needs.

use adblock::resources::{MimeType, Resource, ResourceType};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

/// Stand-in scripts (also usable as scriptlets).
const SURROGATES: &str = include_str!("surrogates/surrogates.js");
/// Scriptlets and their helpers.
const SCRIPTLETS: &str = include_str!("surrogates/scriptlets.js");

/// Stand-ins that aren't scripts: (name, aliases, type, body).
const FILES: &[(&str, &[&str], MimeKind, &[u8])] = &[
    ("noop.txt", &["nooptext", "empty"], MimeKind::Text, b""),
    ("noop.html", &["noopframe"], MimeKind::Html, b"<!DOCTYPE html>\n<html><head></head><body></body></html>\n"),
    ("noop.css", &["noopcss"], MimeKind::Css, b""),
    ("noop.json", &["noopjson"], MimeKind::Json, b"{}"),
    ("noop-vast2.xml", &["noopvast-2.0"], MimeKind::Xml, b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><VAST version=\"2.0\"></VAST>"),
    ("noop-vast3.xml", &["noopvast-3.0"], MimeKind::Xml, b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><VAST version=\"3.0\"></VAST>"),
    ("noop-vast4.xml", &["noopvast-4.0"], MimeKind::Xml, b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><VAST version=\"4.0\"></VAST>"),
    (
        "noop-vmap1.xml",
        &["noop-vmap1.0.xml", "noopvmap-1.0"],
        MimeKind::Xml,
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><vmap:VMAP xmlns:vmap=\"http://www.iab.net/videosuite/vmap\" version=\"1.0\"></vmap:VMAP>",
    ),
    ("1x1.gif", &["1x1-transparent.gif"], MimeKind::Gif, include_bytes!("surrogates/1x1.gif")),
    ("2x2.png", &["2x2-transparent.png"], MimeKind::Png, include_bytes!("surrogates/2x2.png")),
    ("3x2.png", &["3x2-transparent.png"], MimeKind::Png, include_bytes!("surrogates/3x2.png")),
    ("32x32.png", &["32x32-transparent.png"], MimeKind::Png, include_bytes!("surrogates/32x32.png")),
    ("noop-0.1s.mp3", &["noopmp3-0.1s", "abp-resource:blank-mp3"], MimeKind::Mp3, include_bytes!("surrogates/noop-0.1s.mp3")),
    ("noop-0.5s.mp3", &[], MimeKind::Mp3, include_bytes!("surrogates/noop-0.5s.mp3")),
    ("noop-1s.mp4", &["noopmp4-1s", "abp-resource:blank-mp4"], MimeKind::Mp4, include_bytes!("surrogates/noop-1s.mp4")),
];

#[derive(Clone, Copy)]
enum MimeKind {
    Text,
    Html,
    Css,
    Json,
    Xml,
    Gif,
    Png,
    Mp3,
    Mp4,
}

impl MimeKind {
    fn mime(self) -> MimeType {
        match self {
            MimeKind::Text => MimeType::TextPlain,
            MimeKind::Html => MimeType::TextHtml,
            MimeKind::Css => MimeType::TextCss,
            MimeKind::Json => MimeType::ApplicationJson,
            MimeKind::Xml => MimeType::TextXml,
            MimeKind::Gif => MimeType::ImageGif,
            MimeKind::Png => MimeType::ImagePng,
            MimeKind::Mp3 => MimeType::AudioMp3,
            MimeKind::Mp4 => MimeType::VideoMp4,
        }
    }
}

/// One `//@` section of a script file.
#[derive(Debug, PartialEq)]
struct Section<'a> {
    names: Vec<String>,
    deps: Vec<String>,
    body: &'a str,
}

/// Splits a script file into its `//@` sections (text before the first is a
/// header comment). With `add_js`, names get `.js` if they lack it (a
/// scriptlet is looked up that way); helper names (`.fn`) are kept as they
/// are, and so are stand-in names, which redirect rules give whole.
fn sections(text: &str, add_js: bool) -> Vec<Section<'_>> {
    let mut out = Vec::new();
    let mut starts: Vec<(usize, usize)> = Vec::new(); // (line start, body start)
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with("//@ ") {
            starts.push((pos, pos + line.len()));
        }
        pos += line.len();
    }
    for (i, &(head, body)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map_or(text.len(), |s| s.0);
        let mut names = Vec::new();
        let mut deps = Vec::new();
        for tok in text[head + 4..body].split_whitespace() {
            match tok.strip_prefix('+') {
                Some(dep) => deps.push(dep.to_string()),
                None if !add_js || tok.ends_with(".js") || tok.ends_with(".fn") => names.push(tok.to_string()),
                None => names.push(format!("{tok}.js")),
            }
        }
        out.push(Section { names, deps, body: text[body..end].trim() });
    }
    out
}

fn resource(name: &str, aliases: Vec<String>, mime: MimeType, body: &[u8], deps: Vec<String>) -> Resource {
    Resource {
        name: name.to_string(),
        aliases,
        kind: ResourceType::Mime(mime),
        content: STANDARD.encode(body),
        dependencies: deps,
        permission: Default::default(),
    }
}

/// Every resource, for `Engine::use_resources`.
pub(crate) fn all() -> Vec<Resource> {
    let mut out = Vec::new();
    for s in sections(SURROGATES, false).into_iter().chain(sections(SCRIPTLETS, true)) {
        let Some((name, aliases)) = s.names.split_first() else { continue };
        let mime = if name.ends_with(".fn") { MimeType::FnJavascript } else { MimeType::ApplicationJavascript };
        out.push(resource(name, aliases.to_vec(), mime, s.body.as_bytes(), s.deps));
    }
    for (name, aliases, kind, body) in FILES {
        out.push(resource(name, aliases.iter().map(|a| a.to_string()).collect(), kind.mime(), body, Vec::new()));
    }
    out
}

/// A stand-in body from the engine's `data:` URL for a redirect: its type and bytes.
pub(crate) fn decode_data_url(url: &str) -> Option<(String, Vec<u8>)> {
    let rest = url.strip_prefix("data:")?;
    let (head, data) = rest.split_once(',')?;
    let mime = head.strip_suffix(";base64")?;
    Some((mime.to_string(), STANDARD.decode(data).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use adblock::Engine;
    use adblock::lists::{FilterSet, ParseOptions, RuleTypes};

    fn engine(rules: &str) -> Engine {
        let mut set = FilterSet::new(false);
        set.add_filter_list(rules.to_string(), ParseOptions { rule_types: RuleTypes::All, ..Default::default() });
        let mut e = Engine::new_with_filter_set(set);
        e.use_resources(all());
        e
    }

    #[test]
    fn sections_split_names_aliases_and_helpers() {
        let s = sections("// header\n//@ set-constant set +tb-util.fn\nfunction a() {}\n\n//@ tb-util.fn\nfunction b() {}\n", true);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].names, ["set-constant.js", "set.js"]);
        assert_eq!(s[0].deps, ["tb-util.fn"]);
        assert_eq!(s[0].body, "function a() {}");
        assert_eq!(s[1].names, ["tb-util.fn"]);
        assert_eq!(s[1].body, "function b() {}");
        assert_eq!(sections("//@ noopjs noop.js\n1", false)[0].names, ["noopjs", "noop.js"], "stand-in names kept");
    }

    #[test]
    fn every_resource_loads_and_names_are_unique() {
        let all = all();
        let mut seen = std::collections::HashSet::new();
        for r in &all {
            for n in std::iter::once(&r.name).chain(&r.aliases) {
                assert!(seen.insert(n.clone()), "{n} named twice");
            }
            for d in &r.dependencies {
                assert!(all.iter().any(|o| &o.name == d), "{}: missing helper {d}", r.name);
            }
        }
        let mut storage = adblock::resources::InMemoryResourceStorage::default();
        for r in all {
            let name = r.name.clone();
            storage.add_resource(r).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        }
    }

    #[test]
    fn scriptlets_start_with_their_function() {
        for s in sections(SCRIPTLETS, true).iter().filter(|s| !s.names[0].ends_with(".fn")) {
            assert!(s.body.starts_with("function tb"), "{:?} must start with its function", s.names[0]);
        }
    }

    #[test]
    fn the_names_lists_use_most_are_there() {
        let all = all();
        let has = |n: &str| all.iter().any(|r| r.name == n || r.aliases.iter().any(|a| a == n));
        for n in [
            "set.js", "aopr.js", "acs.js", "nowoif.js", "nostif.js", "rmnt.js", "aeld.js", "no-fetch-if.js", "aopw.js",
            "no-xhr-if.js", "aost.js", "nano-sib.js", "nosiif.js", "noeval-if.js", "json-prune.js", "nano-stb.js", "ra.js",
            "set-local-storage-item.js", "set-cookie.js", "nobab.js", "nofab.js", "popads-dummy.js", "window.open-defuser.js",
            "noopjs", "noop.js", "google-ima.js", "1x1.gif", "noopmp3-0.1s", "nooptext", "noopframe", "2x2.png",
            "noopmp4-1s", "googlesyndication_adsbygoogle.js", "googletagservices_gpt.js", "fuckadblock.js-3.2.0",
        ] {
            assert!(has(n), "{n} missing");
        }
    }

    #[test]
    fn redirect_rules_serve_a_stand_in() {
        let e = engine("||ads.example/gpt.js$script,redirect=googletagservices_gpt.js\n||ads.example/x.gif$image,redirect=1x1.gif\n||ads.example/missing.js$script,redirect=no-such-thing");
        let req = |u: &str, t: &str| adblock::request::Request::new(u, "https://news.site/", t, "GET").unwrap();
        let r = e.check_network_request(&req("https://ads.example/gpt.js", "script"));
        assert!(r.should_block());
        let (mime, body) = decode_data_url(r.redirect.as_deref().unwrap()).unwrap();
        assert_eq!(mime, "application/javascript");
        assert!(String::from_utf8(body).unwrap().contains("googletag"));
        let r = e.check_network_request(&req("https://ads.example/x.gif", "image"));
        assert_eq!(decode_data_url(r.redirect.as_deref().unwrap()).unwrap(), ("image/gif".into(), include_bytes!("surrogates/1x1.gif").to_vec()));
        let r = e.check_network_request(&req("https://ads.example/missing.js", "script"));
        assert!(r.should_block() && r.redirect.is_none(), "unknown stand-in: still blocked");
    }

    #[test]
    fn scriptlet_rules_inject_their_function_and_helpers() {
        let e = engine("news.site##+js(set, adBlockDetected, false)\nnews.site##+js(nostif, adblock, 2000)\nnews.site##+js(trusted-set-cookie, a, b)");
        let script = e.url_cosmetic_resources("https://news.site/story").injected_script;
        assert!(script.contains("function tbSetConstant("), "{script}");
        assert!(script.contains("function tbMatcher("), "helper included");
        assert_eq!(script.matches("function tbMatcher(").count(), 1, "helpers once");
        assert!(script.contains("tbSetConstant(\"adBlockDetected\", \"false\")"), "{script}");
        assert!(script.contains("tbNoSetTimeoutIf(\"adblock\", \"2000\")"), "{script}");
        assert!(!script.contains("trusted"), "unknown scriptlets are left out");
        assert!(e.url_cosmetic_resources("https://other.site/").injected_script.is_empty());
    }

    #[test]
    fn data_urls_decode() {
        assert_eq!(decode_data_url("data:text/plain;base64,aGk="), Some(("text/plain".into(), b"hi".to_vec())));
        assert_eq!(decode_data_url("data:text/plain,hi"), None);
        assert_eq!(decode_data_url("https://x/"), None);
    }
}
