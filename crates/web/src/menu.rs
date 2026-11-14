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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    OpenBeside(String),
    CopyText(String),
    Search(String),
    /// Quote the selected text in a note, with a link back to the page.
    AddToNote(String),
    /// Save the note as a Markdown file.
    SaveMarkdown,
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
        ]);
    }
    if let Some(img) = ctx.image.as_ref().filter(|l| !l.is_empty()) {
        sections.push(vec![
            MenuItem::new("Open image beside", MenuAction::OpenBeside(img.clone()), true),
            MenuItem::new("Copy image address", MenuAction::CopyText(img.clone()), true),
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
        sections.push(vec![MenuItem::new("Save as Markdown", MenuAction::SaveMarkdown, true)]);
    }
    if sections.is_empty() {
        sections.push(vec![
            MenuItem::new("Back", MenuAction::Back, ctx.can_back),
            MenuItem::new("Forward", MenuAction::Forward, ctx.can_forward),
            MenuItem::new("Reload", MenuAction::Reload, true),
        ]);
        sections.push(vec![MenuItem::new("Select all", MenuAction::SelectAll, true)]);
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
    fn link_and_selection() {
        let ctx = MenuContext {
            link: Some("https://a.example/".into()),
            selection: "  some\nselected   text that is rather long indeed ".into(),
            ..Default::default()
        };
        let items = items(&ctx);
        assert_eq!(
            labels(&items),
            ["Open link beside", "Copy link address", "-", "Copy", "Search for “some selected text that…”", "Add to note"]
        );
        assert_eq!(items[1], MenuItem::new("Copy link address", MenuAction::CopyText("https://a.example/".into()), true));
        let MenuItem::Action { action, .. } = &items[4] else { panic!() };
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
        assert_eq!(labels(&items(&ctx)), ["Select all", "-", "Save as Markdown"]);
        let ctx = MenuContext { note: true, selection: "words".into(), ..Default::default() };
        assert_eq!(labels(&items(&ctx)), ["Copy", "Search for “words”", "-", "Save as Markdown"]);
    }

    #[test]
    fn image_link() {
        let ctx = MenuContext { link: Some("https://l/".into()), image: Some("https://i/x.png".into()), ..Default::default() };
        assert_eq!(
            labels(&items(&ctx)),
            ["Open link beside", "Copy link address", "-", "Open image beside", "Copy image address"]
        );
    }
}
