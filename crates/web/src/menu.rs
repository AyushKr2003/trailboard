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
}

/// Ad blocking on the site of a page, for its menu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blocking {
    pub site: String,
    /// Off when the user allowed the site.
    pub on: bool,
    /// Requests blocked since the page loaded.
    pub blocked: u32,
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
        if let Some(b) = &ctx.blocking {
            let label = match (b.on, b.blocked) {
                (true, 0) => format!("Allow ads on {}", b.site),
                (true, n) => format!("Allow ads on {} · {n} blocked", b.site),
                (false, _) => format!("Block ads on {}", b.site),
            };
            let mut section = Vec::new();
            if b.on && b.banner_rejected {
                // Only telling: nothing to do with it.
                section.push(MenuItem::new("Cookie banner rejected", MenuAction::ToggleBlocking, false));
            }
            section.push(MenuItem::new(label, MenuAction::ToggleBlocking, true));
            sections.push(section);
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
        let b = Blocking { site: "news.example".into(), on: true, blocked: 12, banner_rejected: false };
        let ctx = MenuContext { blocking: Some(b.clone()), ..Default::default() };
        let got = items(&ctx);
        assert_eq!(labels(&got), ["Back", "Forward", "Reload", "-", "Select all", "-", "Allow ads on news.example · 12 blocked"]);
        let MenuItem::Action { action, .. } = &got[6] else { panic!() };
        assert_eq!(*action, MenuAction::ToggleBlocking);
        let ctx = MenuContext { blocking: Some(Blocking { blocked: 0, ..b.clone() }), ..Default::default() };
        assert_eq!(labels(&items(&ctx))[6], "Allow ads on news.example");
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
