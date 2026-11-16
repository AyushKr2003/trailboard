//! Page context menu: what was right-clicked, and which items to offer.
//!
//! Pure logic, no CEF types. The app draws the menu and runs the chosen action.

/// What the user right-clicked in a page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuContext {
    /// Link under the pointer.
    pub link: Option<String>,
    /// Image under the pointer (its source URL).
    pub image: Option<String>,
    /// Selected text, if any.
    pub selection: String,
    /// The click landed in a text field or other editable content.
    pub editable: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_cut: bool,
    pub can_copy: bool,
    pub can_paste: bool,
    pub can_select_all: bool,
    pub can_back: bool,
    pub can_forward: bool,
    /// The page is a note: no navigation, and nothing to add to a note.
    pub note: bool,
    /// The note is a sticky note (set by the app; the page doesn't know).
    pub sticky: bool,
    /// Ad blocking on the page's site (set by the app for web pages while
    /// blocking is on in the settings).
    pub blocking: Option<Blocking>,
    /// The misspelt word right-clicked in a text field, if any.
    pub misspelled: String,
    /// Chromium's spelling suggestions for it.
    pub suggestions: Vec<String>,
    /// JavaScript and images on the page's site (set by the app for web pages).
    pub site: Option<SiteContent>,
}

/// Whether JavaScript and images are on for a page's site, for its menu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteContent {
    pub site: String,
    pub javascript: bool,
    pub images: bool,
}

/// Most spelling suggestions offered.
const MAX_SUGGESTIONS: usize = 5;

/// Ad blocking on the site of a page, for its menu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blocking {
    pub site: String,
    /// Off when the user allowed the site.
    pub on: bool,
    /// Requests blocked since the page loaded.
    pub blocked: u32,
    /// Page elements hidden by cosmetic rules since the page loaded.
    pub hidden: u32,
    /// A cookie banner on the page was answered with "reject".
    pub banner_rejected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    OpenBeside(String),
    CopyText(String),
    /// Save this URL to the downloads folder (with the page's cookies).
    Download(String),
    Search(String),
    /// Quote the selected text in a note, with a link back to the page.
    AddToNote(String),
    /// Save the note as a Markdown file.
    SaveMarkdown,
    /// Make the note a sticky note, or a sticky note a note card.
    ToggleSticky,
    /// Turn ad blocking off on the page's site, or back on.
    ToggleBlocking,
    /// Turn JavaScript on or off on the page's site (and reload).
    ToggleJavaScript,
    /// Turn images on or off on the page's site (and reload).
    ToggleImages,
    /// Put this word in place of the misspelt one.
    ReplaceMisspelling(String),
    /// Stop marking this word as misspelt.
    AddToDictionary(String),
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Back,
    Forward,
    Reload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    Action { label: String, action: MenuAction, enabled: bool },
    Separator,
}

impl MenuItem {
    fn new(label: impl Into<String>, action: MenuAction, enabled: bool) -> Self {
        MenuItem::Action { label: label.into(), action, enabled }
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self, MenuItem::Action { enabled: true, .. })
    }
}

/// A link or image Chromium can download (not `javascript:` or `mailto:`).
/// Notes never download (see `can_download`), so their menus disable it.
fn downloadable(url: &str) -> bool {
    ["http:", "https:", "data:", "blob:", "file:", "ftp:"].iter().any(|s| url.len() > s.len() && url[..s.len()].eq_ignore_ascii_case(s))
}

/// Longest stretch of selected text quoted in the "Search for" label.
const QUOTE_CHARS: usize = 24;

/// Items for `ctx`, grouped into sections separated by `Separator`s.
/// Order: link, image, editing or selection, then page navigation when nothing
/// more specific was clicked.
pub fn items(ctx: &MenuContext) -> Vec<MenuItem> {
    let mut sections: Vec<Vec<MenuItem>> = Vec::new();
    if let Some(link) = ctx.link.as_ref().filter(|l| !l.is_empty()) {
        sections.push(vec![
            MenuItem::new("Open link beside", MenuAction::OpenBeside(link.clone()), true),
            MenuItem::new("Copy link address", MenuAction::CopyText(link.clone()), true),
            MenuItem::new("Download link", MenuAction::Download(link.clone()), downloadable(link) && !ctx.note),
        ]);
    }
    if let Some(img) = ctx.image.as_ref().filter(|l| !l.is_empty()) {
        sections.push(vec![
            MenuItem::new("Open image beside", MenuAction::OpenBeside(img.clone()), true),
            MenuItem::new("Copy image address", MenuAction::CopyText(img.clone()), true),
            MenuItem::new("Save image", MenuAction::Download(img.clone()), downloadable(img) && !ctx.note),
        ]);
    }
    let selection = ctx.selection.trim();
    if ctx.editable && !ctx.misspelled.is_empty() {
        let mut fixes: Vec<MenuItem> = ctx
            .suggestions
            .iter()
            .take(MAX_SUGGESTIONS)
            .map(|s| MenuItem::new(s.clone(), MenuAction::ReplaceMisspelling(s.clone()), true))
            .collect();
        if fixes.is_empty() {
            fixes.push(MenuItem::new("No spelling suggestions", MenuAction::ReplaceMisspelling(String::new()), false));
        }
        fixes.push(MenuItem::new(
            format!("Add “{}” to dictionary", quote(&ctx.misspelled)),
            MenuAction::AddToDictionary(ctx.misspelled.clone()),
            true,
        ));
        sections.push(fixes);
    }
    if ctx.editable {
        sections.push(vec![
            MenuItem::new("Undo", MenuAction::Undo, ctx.can_undo),
            MenuItem::new("Redo", MenuAction::Redo, ctx.can_redo),
        ]);
        sections.push(vec![
            MenuItem::new("Cut", MenuAction::Cut, ctx.can_cut),
            MenuItem::new("Copy", MenuAction::Copy, ctx.can_copy),
            MenuItem::new("Paste", MenuAction::Paste, ctx.can_paste),
            MenuItem::new("Select all", MenuAction::SelectAll, ctx.can_select_all),
        ]);
    } else if !selection.is_empty() {
        sections.push(vec![
            MenuItem::new("Copy", MenuAction::Copy, true),
            MenuItem::new(format!("Search for “{}”", quote(selection)), MenuAction::Search(selection.to_string()), true),
        ]);
        if !ctx.note {
            sections.last_mut().expect("just pushed").push(MenuItem::new(
                "Add to note",
                MenuAction::AddToNote(ctx.selection.clone()),
                true,
            ));
        }
    }
    if sections.is_empty() && ctx.note {
        sections.push(vec![MenuItem::new("Select all", MenuAction::SelectAll, true)]);
    }
    if ctx.note {
        let toggle = if ctx.sticky { "Make note card" } else { "Make sticky note" };
        sections.push(vec![
            MenuItem::new("Save as Markdown", MenuAction::SaveMarkdown, true),
            MenuItem::new(toggle, MenuAction::ToggleSticky, true),
        ]);
    }
    if sections.is_empty() {
        sections.push(vec![
            MenuItem::new("Back", MenuAction::Back, ctx.can_back),
            MenuItem::new("Forward", MenuAction::Forward, ctx.can_forward),
            MenuItem::new("Reload", MenuAction::Reload, true),
        ]);
        sections.push(vec![MenuItem::new("Select all", MenuAction::SelectAll, true)]);
        if let Some(s) = &ctx.site {
            let word = |on: bool| if on { "Turn off" } else { "Turn on" };
            sections.push(vec![
                MenuItem::new(format!("{} JavaScript on {}", word(s.javascript), s.site), MenuAction::ToggleJavaScript, true),
                MenuItem::new(format!("{} images on {}", word(s.images), s.site), MenuAction::ToggleImages, true),
            ]);
        }
        if let Some(b) = &ctx.blocking {
            sections.push(blocking_items(b));
        }
    }
    let mut out = Vec::new();
    for (i, s) in sections.into_iter().enumerate() {
        if i > 0 {
            out.push(MenuItem::Separator);
        }
        out.extend(s);
    }
    out
}

/// The ad-blocking entries for a page's site: what the page menu ends with,
/// and all the title bar's shield offers.
pub fn blocking_items(b: &Blocking) -> Vec<MenuItem> {
    let label = if b.on {
        let mut label = format!("Allow ads on {}", b.site);
        if b.blocked > 0 {
            label.push_str(&format!(" · {} blocked", b.blocked));
        }
        if b.hidden > 0 {
            label.push_str(&format!(" · {} hidden", b.hidden));
        }
        label
    } else {
        format!("Block ads on {}", b.site)
    };
    let mut section = Vec::new();
    if b.on && b.banner_rejected {
        // Only telling: nothing to do with it.
        section.push(MenuItem::new("Cookie banner rejected", MenuAction::ToggleBlocking, false));
    }
    section.push(MenuItem::new(label, MenuAction::ToggleBlocking, true));
    section
}

/// Single-line, shortened text for a menu label.
fn quote(s: &str) -> String {
    let flat: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= QUOTE_CHARS {
        return flat;
    }
    let cut: String = flat.chars().take(QUOTE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[MenuItem]) -> Vec<String> {
        items
            .iter()
            .map(|i| match i {
                MenuItem::Action { label, .. } => label.clone(),
                MenuItem::Separator => "-".into(),
            })
            .collect()
    }

    #[test]
    fn plain_page_offers_navigation() {
        let ctx = MenuContext { can_back: true, ..Default::default() };
        let items = items(&ctx);
        assert_eq!(labels(&items), ["Back", "Forward", "Reload", "-", "Select all"]);
        assert!(items[0].is_enabled());
        assert!(!items[1].is_enabled());
    }

    #[test]
    fn plain_page_offers_blocking_on_its_site() {
        let b = Blocking { site: "news.example".into(), on: true, blocked: 12, hidden: 0, banner_rejected: false };
        let ctx = MenuContext { blocking: Some(b.clone()), ..Default::default() };
        let got = items(&ctx);
        assert_eq!(labels(&got), ["Back", "Forward", "Reload", "-", "Select all", "-", "Allow ads on news.example · 12 blocked"]);
        let MenuItem::Action { action, .. } = &got[6] else { panic!() };
        assert_eq!(*action, MenuAction::ToggleBlocking);
        let ctx = MenuContext { blocking: Some(Blocking { blocked: 0, ..b.clone() }), ..Default::default() };
        assert_eq!(labels(&items(&ctx))[6], "Allow ads on news.example");
        // Elements hidden by cosmetic rules are counted beside the requests.
        let ctx = MenuContext { blocking: Some(Blocking { hidden: 4, ..b.clone() }), ..Default::default() };
        assert_eq!(labels(&items(&ctx))[6], "Allow ads on news.example · 12 blocked · 4 hidden");
        let ctx = MenuContext { blocking: Some(Blocking { blocked: 0, hidden: 1, ..b.clone() }), ..Default::default() };
        assert_eq!(labels(&items(&ctx))[6], "Allow ads on news.example · 1 hidden");
        let ctx = MenuContext { blocking: Some(Blocking { on: false, hidden: 4, ..b.clone() }), ..Default::default() };
        assert_eq!(labels(&items(&ctx))[6], "Block ads on news.example");
        let ctx = MenuContext { blocking: Some(Blocking { on: false, ..b.clone() }), ..Default::default() };
        assert_eq!(labels(&items(&ctx))[6], "Block ads on news.example");
        // A rejected cookie banner is told, above the toggle, while blocking is on.
        let ctx = MenuContext { blocking: Some(Blocking { banner_rejected: true, ..b.clone() }), ..Default::default() };
        let got = items(&ctx);
        assert_eq!(labels(&got)[6..], ["Cookie banner rejected", "Allow ads on news.example · 12 blocked"]);
        assert!(!got[6].is_enabled() && got[7].is_enabled());
        let ctx = MenuContext { blocking: Some(Blocking { banner_rejected: true, on: false, ..b.clone() }), ..Default::default() };
        assert_eq!(labels(&items(&ctx))[6..], ["Block ads on news.example"]);
        // Over a link, the menu is about the link.
        let ctx = MenuContext { blocking: Some(b), link: Some("https://a/".into()), ..Default::default() };
        assert!(!labels(&items(&ctx)).iter().any(|l| l.contains("ads")));
    }

    #[test]
    fn shield_offers_only_the_blocking_entries() {
        let b = Blocking { site: "news.example".into(), on: true, blocked: 3, hidden: 0, banner_rejected: true };
        let got = blocking_items(&b);
        assert_eq!(labels(&got), ["Cookie banner rejected", "Allow ads on news.example · 3 blocked"]);
        // The same entries the page menu ends with.
        let ctx = MenuContext { blocking: Some(b), ..Default::default() };
        assert_eq!(items(&ctx)[6..], got[..]);
    }

    #[test]
    fn link_and_selection() {
        let ctx = MenuContext {
            link: Some("https://a.example/".into()),
            selection: "  some\nselected   text that is rather long indeed ".into(),
            ..Default::default()
        };
        let items = items(&ctx);
        assert_eq!(
            labels(&items),
            ["Open link beside", "Copy link address", "Download link", "-", "Copy", "Search for “some selected text that…”", "Add to note"]
        );
        assert_eq!(items[1], MenuItem::new("Copy link address", MenuAction::CopyText("https://a.example/".into()), true));
        let MenuItem::Action { action, .. } = &items[5] else { panic!() };
        assert_eq!(*action, MenuAction::Search("some\nselected   text that is rather long indeed".into()));
    }

    #[test]
    fn editable_uses_edit_flags() {
        let ctx = MenuContext { editable: true, can_paste: true, can_select_all: true, ..Default::default() };
        let items = items(&ctx);
        assert_eq!(labels(&items), ["Undo", "Redo", "-", "Cut", "Copy", "Paste", "Select all"]);
        let enabled: Vec<bool> = items.iter().map(MenuItem::is_enabled).collect();
        assert_eq!(enabled, [false, false, false, false, false, true, true]);
    }

    #[test]
    fn misspelt_words_offer_fixes_first() {
        let ctx = MenuContext {
            editable: true,
            misspelled: "teh".into(),
            suggestions: ["the", "tea", "ten", "tech", "eh", "te"].map(String::from).to_vec(),
            ..Default::default()
        };
        let got = items(&ctx);
        assert_eq!(labels(&got)[..7], ["the", "tea", "ten", "tech", "eh", "Add “teh” to dictionary", "-"]);
        let MenuItem::Action { action, .. } = &got[0] else { panic!() };
        assert_eq!(*action, MenuAction::ReplaceMisspelling("the".into()));
        let MenuItem::Action { action, .. } = &got[5] else { panic!() };
        assert_eq!(*action, MenuAction::AddToDictionary("teh".into()));
        let ctx = MenuContext { editable: true, misspelled: "qzxv".into(), ..Default::default() };
        let got = items(&ctx);
        assert_eq!(labels(&got)[..2], ["No spelling suggestions", "Add “qzxv” to dictionary"]);
        assert!(!got[0].is_enabled());
        // Outside a text field there is nothing to fix.
        let ctx = MenuContext { misspelled: "teh".into(), ..Default::default() };
        assert_eq!(labels(&items(&ctx))[0], "Back");
    }

    #[test]
    fn plain_page_offers_javascript_and_images_on_its_site() {
        let s = SiteContent { site: "a.example".into(), javascript: true, images: false };
        let ctx = MenuContext { site: Some(s), ..Default::default() };
        let got = items(&ctx);
        assert_eq!(labels(&got)[5..], ["-", "Turn off JavaScript on a.example", "Turn on images on a.example"]);
        let MenuItem::Action { action, .. } = &got[7] else { panic!() };
        assert_eq!(*action, MenuAction::ToggleImages);
    }

    #[test]
    fn notes_offer_saving_but_no_navigation_or_add_to_note() {
        let ctx = MenuContext { note: true, ..Default::default() };
        assert_eq!(labels(&items(&ctx)), ["Select all", "-", "Save as Markdown", "Make sticky note"]);
        let ctx = MenuContext { note: true, selection: "words".into(), ..Default::default() };
        assert_eq!(labels(&items(&ctx)), ["Copy", "Search for “words”", "-", "Save as Markdown", "Make sticky note"]);
        let ctx = MenuContext { note: true, sticky: true, ..Default::default() };
        assert_eq!(labels(&items(&ctx)), ["Select all", "-", "Save as Markdown", "Make note card"]);
    }

    #[test]
    fn image_link() {
        let ctx = MenuContext { link: Some("https://l/".into()), image: Some("https://i/x.png".into()), ..Default::default() };
        assert_eq!(
            labels(&items(&ctx)),
            ["Open link beside", "Copy link address", "Download link", "-", "Open image beside", "Copy image address", "Save image"]
        );
        let ctx = MenuContext { link: Some("mailto:a@b".into()), ..Default::default() };
        assert!(!items(&ctx)[2].is_enabled(), "mailto: can't be downloaded");
        assert!(downloadable("HTTPS://x/y.zip") && !downloadable("javascript:void(0)") && !downloadable("https:"));
    }
}
