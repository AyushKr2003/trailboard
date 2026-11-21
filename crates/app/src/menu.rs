//! Menu overlays: the page context menu (items from `web::menu`), a stack's
//! outline (its members, to jump to one), and the menus of the board's own
//! controls (the spaces, and *More* on the dock). Layout, hit testing and
//! keyboard movement.

use scene::{NodeId, Rect, Vec2};
use storage::spaces::SpaceId;
use web::menu::{MenuAction, MenuItem};

use crate::icons::Icon;
use crate::palette::Command;

/// The card a menu of the board's own controls belongs to: none.
pub const NO_CARD: NodeId = NodeId(u64::MAX);

/// Longest page title shown in a stack's outline, in characters.
pub const OUTLINE_CHARS: usize = 60;

/// A row of a menu overlay.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// A page context menu item.
    Page(MenuItem),
    /// A stack member; `current` is the one in front.
    Member { id: NodeId, label: String, current: bool },
    /// Unfold the stack.
    Unfold,
    /// A board command, with the keys that also run it.
    Command { cmd: Command, label: &'static str, keys: &'static str },
    /// Go to a space; `current` is the one showing.
    Space { id: SpaceId, label: String, current: bool },
}

impl Entry {
    pub fn is_enabled(&self) -> bool {
        match self {
            Entry::Page(i) => i.is_enabled(),
            Entry::Member { .. } | Entry::Unfold | Entry::Command { .. } | Entry::Space { .. } => true,
        }
    }

    pub fn is_separator(&self) -> bool {
        matches!(self, Entry::Page(MenuItem::Separator))
    }

    /// The row's text; "-" for a separator.
    pub fn label(&self) -> &str {
        match self {
            Entry::Page(MenuItem::Action { label, .. }) | Entry::Member { label, .. } | Entry::Space { label, .. } => label,
            Entry::Command { label, .. } => label,
            Entry::Page(MenuItem::Separator) => "-",
            Entry::Unfold => "Unfold trail",
        }
    }

    /// The keys shown at the row's right end: a command's, or the page
    /// keys an editing or navigation item also has.
    pub fn keys(&self) -> &str {
        match self {
            Entry::Command { keys, .. } => keys,
            Entry::Page(MenuItem::Action { action, .. }) => match action {
                MenuAction::Undo => "Ctrl+Z",
                MenuAction::Redo => "Ctrl+Shift+Z",
                MenuAction::Cut => "Ctrl+X",
                MenuAction::Copy => "Ctrl+C",
                MenuAction::Paste => "Ctrl+V",
                MenuAction::SelectAll => "Ctrl+A",
                MenuAction::Back => "Alt+←",
                MenuAction::Forward => "Alt+→",
                MenuAction::Reload => "Ctrl+R",
                MenuAction::Inspect(..) => "F12",
                _ => "",
            },
            _ => "",
        }
    }

    /// The icon in the row's lead column, if it has one. Rows without one
    /// (stack members, spaces, spelling fixes) keep the column for the tick.
    pub fn icon(&self) -> Option<Icon> {
        match self {
            Entry::Command { cmd, .. } => Some(crate::palette::command_icon(*cmd)),
            Entry::Unfold => Some(Icon::Expand),
            Entry::Page(MenuItem::Action { action, label, .. }) => Some(match action {
                MenuAction::OpenBeside(_) => Icon::Trail,
                MenuAction::CopyText(_) if label.contains("link") => Icon::Link,
                MenuAction::CopyText(_) | MenuAction::Copy => Icon::Copy,
                MenuAction::Download(_) | MenuAction::SaveMarkdown => Icon::Download,
                MenuAction::Search(_) => Icon::Search,
                MenuAction::AddToNote(_) => Icon::Note,
                MenuAction::ToggleSticky => Icon::Sticky,
                MenuAction::ToggleBlocking => Icon::Shield,
                MenuAction::ToggleJavaScript => Icon::Code,
                MenuAction::ToggleImages => Icon::Picture,
                MenuAction::AddToDictionary(_) => Icon::Plus,
                MenuAction::ReplaceMisspelling(_) => return None,
                MenuAction::Undo => Icon::Undo,
                MenuAction::Redo => Icon::Redo,
                MenuAction::Cut => Icon::Cut,
                MenuAction::Paste => Icon::Paste,
                MenuAction::SelectAll => Icon::Fit,
                MenuAction::Back => Icon::Back,
                MenuAction::Forward => Icon::Forward,
                MenuAction::Reload => Icon::Reload,
                MenuAction::Inspect(..) => Icon::Code,
            }),
            _ => None,
        }
    }

    /// The row is ticked: the stack member in front, the space showing.
    pub fn is_current(&self) -> bool {
        matches!(self, Entry::Member { current: true, .. } | Entry::Space { current: true, .. })
    }

    /// A command's row, labelled and keyed as in the palette.
    pub fn command(cmd: Command) -> Entry {
        let info = crate::palette::COMMANDS.iter().find(|i| i.cmd == cmd).expect("every command is listed");
        Entry::Command { cmd, label: info.label, keys: info.keys }
    }

    pub fn separator() -> Entry {
        Entry::Page(MenuItem::Separator)
    }
}

/// A page title for one line: trimmed, cut to `max` characters with "…", or
/// "Untitled" if empty.
pub fn cut(title: &str, max: usize) -> String {
    let title = title.trim();
    if title.chars().count() > max {
        format!("{}…", title.chars().take(max - 1).collect::<String>().trim_end())
    } else if title.is_empty() {
        "Untitled".to_string()
    } else {
        title.to_string()
    }
}

/// A stack's outline: `(id, title or URL)` of each member in reading order,
/// numbered, with long titles cut short, then a separator and Unfold.
pub fn outline(members: &[(NodeId, &str)], front: NodeId) -> Vec<Entry> {
    let mut v: Vec<Entry> = members
        .iter()
        .enumerate()
        .map(|(i, &(id, title))| {
            let title = cut(title, OUTLINE_CHARS);
            Entry::Member { id, label: format!("{}  {title}", i + 1), current: id == front }
        })
        .collect();
    v.push(Entry::Page(MenuItem::Separator));
    v.push(Entry::Unfold);
    v
}

/// Sizes in logical px; multiplied by the window scale.
pub const ITEM_H: f32 = 32.0;
pub const SEP_H: f32 = 9.0;
pub const PAD: f32 = 4.0;
/// From a row's left edge to the middle of its icon (or tick).
pub const ICON_X: f32 = 16.0;
/// From the menu's edge to a row's text: room for the icon column.
pub const TEXT_PAD: f32 = 40.0;
pub const FONT: f32 = 13.0;
pub const MIN_W: f32 = 200.0;

pub struct ContextMenu {
    /// The card whose page was right-clicked, or the stack's front for an outline.
    pub page: NodeId,
    pub items: Vec<Entry>,
    /// Outer box, screen px.
    pub rect: Rect,
    /// One rect per item (separators included), screen px.
    pub rows: Vec<Rect>,
    pub hover: Option<usize>,
}

impl ContextMenu {
    /// Lay out `items` at `anchor` (screen px) with the given widest label width.
    pub fn new(page: NodeId, items: Vec<Entry>, anchor: Vec2, label_w: f32, viewport: Vec2, scale: f32) -> Self {
        let (rect, rows) = layout(&items, anchor, label_w, viewport, scale);
        // An outline starts on the member in front.
        let hover = items.iter().position(|e| matches!(e, Entry::Member { current: true, .. }));
        ContextMenu { page, items, rect, rows, hover }
    }

    /// Index of the enabled item at `p`.
    pub fn item_at(&self, p: Vec2) -> Option<usize> {
        self.rows.iter().position(|r| r.contains(p)).filter(|&i| self.items[i].is_enabled())
    }

    /// Move the highlight by `step` (±1) over enabled items, wrapping around.
    pub fn step(&mut self, step: i32) {
        let n = self.items.len() as i32;
        if n == 0 {
            return;
        }
        let mut i = match self.hover {
            Some(h) => h as i32,
            None if step > 0 => -1,
            None => n,
        };
        for _ in 0..n {
            i = (i + step).rem_euclid(n);
            if self.items[i as usize].is_enabled() {
                self.hover = Some(i as usize);
                return;
            }
        }
    }
}

/// Menu box and row rects. The menu opens down and right of `anchor`, flipping
/// up or left when it would leave the viewport, and is clamped inside it.
pub fn layout(items: &[Entry], anchor: Vec2, label_w: f32, viewport: Vec2, scale: f32) -> (Rect, Vec<Rect>) {
    let w = (label_w + (TEXT_PAD + 16.0) * scale).max(MIN_W * scale);
    let content_h: f32 =
        items.iter().map(|i| if i.is_separator() { SEP_H } else { ITEM_H }).sum::<f32>() * scale;
    let h = content_h + 2.0 * PAD * scale;
    let mut x = anchor.x;
    let mut y = anchor.y;
    if x + w > viewport.x {
        x = anchor.x - w;
    }
    if y + h > viewport.y {
        y = anchor.y - h;
    }
    x = x.min(viewport.x - w).max(0.0);
    y = y.min(viewport.y - h).max(0.0);
    let mut rows = Vec::with_capacity(items.len());
    let mut top = y + PAD * scale;
    for i in items {
        let rh = if i.is_separator() { SEP_H } else { ITEM_H } * scale;
        rows.push(Rect::new(x + PAD * scale, top, w - 2.0 * PAD * scale, rh));
        top += rh;
    }
    (Rect::new(x, y, w, h), rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use web::menu::{items, MenuContext};

    fn sample() -> Vec<Entry> {
        // Back (disabled), Forward (disabled), Reload, -, Select all
        items(&MenuContext::default()).into_iter().map(Entry::Page).collect()
    }

    #[test]
    fn outline_numbers_members_and_starts_on_the_front() {
        let long = "x".repeat(100);
        let members = [(NodeId(4), "Alpha"), (NodeId(7), "  "), (NodeId(9), long.as_str())];
        let o = outline(&members, NodeId(7));
        let labels: Vec<&str> = o.iter().map(Entry::label).collect();
        assert_eq!(&labels[..2], &["1  Alpha", "2  Untitled"]);
        assert_eq!(labels[2].chars().count(), 3 + OUTLINE_CHARS);
        assert!(labels[2].ends_with('…'));
        assert_eq!(&labels[3..], &["-", "Unfold trail"]);
        let m = ContextMenu::new(NodeId(7), o, Vec2::new(0.0, 0.0), 50.0, Vec2::new(800.0, 600.0), 1.0);
        assert_eq!(m.hover, Some(1));
        assert!(m.items.iter().all(|e| e.is_enabled() || e.is_separator()));
    }

    #[test]
    fn opens_below_right_and_flips_at_edges() {
        let vp = Vec2::new(800.0, 600.0);
        let (r, rows) = layout(&sample(), Vec2::new(100.0, 100.0), 50.0, vp, 1.0);
        assert_eq!((r.x, r.y, r.w), (100.0, 100.0, MIN_W));
        assert_eq!(r.h, 4.0 * ITEM_H + SEP_H + 2.0 * PAD);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].y, 100.0 + PAD);
        assert_eq!(rows[3].h, SEP_H);
        let (r, _) = layout(&sample(), Vec2::new(790.0, 590.0), 50.0, vp, 1.0);
        assert_eq!((r.right(), r.bottom()), (790.0, 590.0));
        // Too tall to flip up: clamped to the top.
        let (r, _) = layout(&sample(), Vec2::new(10.0, 50.0), 50.0, Vec2::new(800.0, 120.0), 1.0);
        assert_eq!(r.y, 0.0);
    }

    #[test]
    fn hit_and_keyboard_skip_disabled_and_separators() {
        let vp = Vec2::new(800.0, 600.0);
        let mut m = ContextMenu::new(NodeId(1), sample(), Vec2::new(0.0, 0.0), 50.0, vp, 1.0);
        let center = |r: Rect| Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert_eq!(m.item_at(center(m.rows[0])), None, "Back is disabled");
        assert_eq!(m.item_at(center(m.rows[2])), Some(2));
        assert_eq!(m.item_at(center(m.rows[3])), None, "separator");
        assert_eq!(m.item_at(Vec2::new(500.0, 500.0)), None);
        m.step(1);
        assert_eq!(m.hover, Some(2));
        m.step(1);
        assert_eq!(m.hover, Some(4));
        m.step(1);
        assert_eq!(m.hover, Some(2), "wraps");
        m.hover = None;
        m.step(-1);
        assert_eq!(m.hover, Some(4));
    }
}
