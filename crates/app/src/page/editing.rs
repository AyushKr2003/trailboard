//! Clipboard, IME, context menu.

use std::time::Duration;

use scene::keys;
use scene::{Node, NodeId, Rect, Vec2};
use web::EditCommand;
use web::menu::{MenuAction, MenuItem};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::Ime;

use crate::app::{App, BAR_GAP, BAR_H, menu_labels};
use crate::keyboard::keymap::KeyIn;
use crate::page::clipboard::Paste;
use crate::ui::menu::{self, ContextMenu, Entry};
use crate::ui::omnibox;
use crate::ui::palette::{self, Command};

impl App {
    /// Ctrl+C / Ctrl+X (`key`) or the menu's Copy / Cut (`None`) in the focused page.
    pub(crate) fn page_copy(&mut self, cut: bool, key: Option<&KeyIn>) {
        let Some(page) = self.focused_page() else { return };
        // Read the selection before a cut removes it.
        let text = page.selected_text();
        match key {
            Some(k) => page.send_key(&k.to_web(self.mods)),
            None => page.edit(if cut { EditCommand::Cut } else { EditCommand::Copy }),
        }
        log::debug!("copy from page: {} chars", text.chars().count());
        self.clipboard.copied_from_page(text);
    }

    /// Ctrl+W / Ctrl+U / Ctrl+H in a page's field: the editing keys
    /// Chromium knows (Ctrl+Backspace; Shift+Home then Backspace; Backspace),
    /// sent to the focused frame like typed keys, so they work in frames too.
    pub(crate) fn edit_page_field(&self, edit: keys::FieldEdit) {
        use web::keys::{Key, vk};
        let Some(p) = self.focused_page() else { return };
        let taps: &[(i32, u32)] = match edit {
            keys::FieldEdit::WordBack => &[(vk::BACK, web::flags::CONTROL)],
            keys::FieldEdit::LineStart => &[(vk::HOME, web::flags::SHIFT), (vk::BACK, 0)],
            keys::FieldEdit::CharBack => &[(vk::BACK, 0)],
        };
        for &(code, mods) in taps {
            for k in Key::tap(code, mods) {
                p.send_key(&k);
            }
        }
        log::debug!("page field edit: {edit:?}");
    }

    /// Ctrl+V (`key`) or the menu's Paste (`None`) in the focused page.
    pub(crate) fn page_paste(&mut self, key: Option<&KeyIn>) {
        let plan = self.clipboard.paste_plan();
        let Some(page) = self.focused_page() else { return };
        match plan {
            Paste::Text(t) => {
                log::debug!("paste desktop text: {} chars", t.chars().count());
                page.paste_text(&t);
            }
            Paste::Native => match key {
                Some(k) => page.send_key(&k.to_web(self.mods)),
                None => page.edit(EditCommand::Paste),
            },
        }
    }

    pub(crate) fn ime(&mut self, ime: Ime) {
        log::debug!("ime {ime:?}");
        if let Some(ob) = self.omnibox.as_mut() {
            // The address bar takes committed text only; it doesn't show compositions.
            if let Ime::Commit(t) = ime {
                ob.insert(&t);
                ob.sel = 0;
                ob.scroll = 0;
            }
            return;
        }
        let Some(id) = self.focused else { return };
        let Some(page) = self.cards.get(&id).map(|c| &c.page) else { return };
        match ime {
            Ime::Preedit(text, cursor) if !text.is_empty() => {
                page.ime_set_composition(&text, cursor);
                self.ime_page = Some(id);
            }
            Ime::Preedit(..) | Ime::Disabled => {
                if self.ime_page.take().is_some() {
                    page.ime_cancel();
                }
            }
            Ime::Commit(text) => {
                page.ime_commit(&text);
                self.ime_page = None;
            }
            Ime::Enabled => {}
        }
    }

    /// Enable IME while text can go somewhere, and keep the candidate window by the caret.
    pub(crate) fn sync_ime(&mut self) {
        // Only where text goes: an input method would take Page mode's keys.
        let page_keys = self.typing();
        // A test script feeds IME events itself; keep a running input method out of it.
        let allowed = (self.omnibox.is_some() || page_keys) && self.script.is_none();
        // A composition left in a page that lost focus is dropped.
        if let Some(id) = self.ime_page
            && (Some(id) != self.focused || !page_keys)
        {
            if let Some(c) = self.cards.get(&id) {
                c.page.ime_cancel();
            }
            self.ime_page = None;
        }
        let area = if self.omnibox.is_some() {
            let f = palette::field_rect(self.viewport(), self.ui());
            Some([f.x as i32, f.y as i32, f.w as i32, f.h as i32])
        } else if page_keys {
            self.focused.and_then(|id| {
                let (x, y, w, h) = self.cards.get(&id)?.page.ime_caret()?;
                let r = self.page_rect_to_screen(id, Rect::new(x, y, w.max(1.0), h.max(1.0)))?;
                Some([r.x as i32, r.y as i32, r.w.ceil() as i32, r.h.ceil() as i32])
            })
        } else {
            None
        };
        let Some(w) = self.window().cloned() else { return };
        if allowed != self.ime_allowed {
            log::debug!("ime allowed={allowed}");
            w.set_ime_allowed(allowed);
            self.ime_allowed = allowed;
            self.ime_area = None;
        }
        if allowed
            && let Some(a) = area
            && self.ime_area != Some(a)
        {
            log::debug!("ime area {a:?}");
            w.set_ime_cursor_area(PhysicalPosition::new(a[0], a[1]), PhysicalSize::new(a[2].max(1) as u32, a[3].max(1) as u32));
            self.ime_area = Some(a);
        }
    }

    pub(crate) fn open_menu(&mut self, id: NodeId, x: f32, y: f32, ctx: &web::menu::MenuContext) {
        let Some(at) = self.page_rect_to_screen(id, Rect::new(x, y, 0.0, 0.0)) else { return };
        let anchor = Vec2::new(at.x, at.y);
        let ctx = web::menu::MenuContext {
            sticky: self.scene.node(id).is_some_and(Node::is_sticky),
            blocking: self.blocking_state(id),
            site: self.site_content_state(id),
            ..ctx.clone()
        };
        let mut items: Vec<Entry> = web::menu::items(&ctx).into_iter().map(Entry::Page).collect();
        // The card's trail (M15), before *Inspect*, which stays last.
        let mut trail = vec![Entry::command(Command::JoinTrail), Entry::command(Command::JoinTrailKeep), Entry::command(Command::LinkCard)];
        if !self.scene.neighbours(id).is_empty() || !self.scene.link_ends(id).is_empty() {
            trail.push(Entry::command(Command::LeaveTrail));
        }
        let at = match items.iter().rposition(|e| matches!(e, Entry::Page(MenuItem::Action { action: web::menu::MenuAction::Inspect(..), .. }))) {
            Some(i) => i.saturating_sub(1), // its separator
            None => items.len(),
        };
        let lead = if at > 0 { vec![Entry::separator()] } else { Vec::new() };
        items.splice(at..at, lead.into_iter().chain(trail));
        let m = self.layout_menu(id, items, anchor);
        log::info!("context menu on card {}: {:?}", id.0, menu_labels(&m.items));
        self.menu = Some(m);
        self.menu_pressed = false;
        if self.menu_by_key.take().is_some_and(|t| t.elapsed() < Duration::from_secs(2))
            && let Some(m) = self.menu.as_mut()
        {
            m.step(1);
        }
    }

    pub(crate) fn layout_menu(&mut self, id: NodeId, items: Vec<Entry>, anchor: Vec2) -> ContextMenu {
        let scale = self.ui();
        let ksize = crate::ui::chrome::type_::CAPTION * scale;
        let label_w = match self.renderer.as_mut() {
            Some(r) => items
                .iter()
                .filter(|e| !e.is_separator())
                .map(|e| {
                    let keys = crate::ui::chrome::keycaps(e.keys()).unwrap_or_default();
                    let widths: Vec<f32> = keys.iter().map(|k| r.measure(k, ksize, false)).collect();
                    let kw = if keys.is_empty() { 0.0 } else { crate::ui::chrome::keycaps_width(&widths, ksize) + 24.0 * scale };
                    r.measure(e.label(), menu::FONT * scale, e.is_current()) + kw
                })
                .fold(0.0, f32::max),
            None => 0.0,
        };
        ContextMenu::new(id, items, anchor, label_w, self.viewport(), scale)
    }

    /// A press on one of the board's own controls.
    pub(crate) fn hud_press(&mut self, c: crate::ui::hud::Control) {
        use crate::ui::hud::Control;
        log::info!("control: {c:?}");
        self.keys_in_page = false;
        match c {
            Control::Space => {
                let items = self.space_menu_items();
                self.open_hud_menu(c, items);
            }
            _ => {
                if let Some(cmd) = c.command() {
                    self.run_command(cmd);
                }
            }
        }
    }

    /// Opens `items` as the menu of control `c`, under it. A second press
    /// on the control closes it.
    /// The spaces menu: every space, then making, renaming, tinting and deleting them.
    pub(crate) fn space_menu_items(&self) -> Vec<Entry> {
        let active = self.spaces.active;
        let mut items: Vec<Entry> = self
            .spaces
            .spaces
            .iter()
            .map(|e| Entry::Space { id: e.id, label: if e.private { format!("{} · private", e.name) } else { e.name.clone() }, current: e.id == active })
            .collect();
        items.push(Entry::separator());
        items.extend([Command::NewSpace, Command::NewPrivateSpace].map(Entry::command));
        items.push(Entry::separator());
        items.extend([Command::RenameSpace, Command::NextTint, Command::BackgroundPhoto, Command::PrivateSpace, Command::DeleteSpace].map(Entry::command));
        items
    }

    /// The sidebar's space line: the spaces menu under it.
    pub(crate) fn open_sidebar_space_menu(&mut self, at: Rect) {
        let anchor = Vec2::new(at.x + 6.0 * self.ui(), at.bottom() + 4.0 * self.ui());
        let m = self.layout_menu(menu::NO_CARD, self.space_menu_items(), anchor);
        log::info!("sidebar space menu: {:?}", menu_labels(&m.items));
        self.menu = Some(m);
        self.hud_open = None;
        self.menu_pressed = false;
    }

    pub(crate) fn open_hud_menu(&mut self, c: crate::ui::hud::Control, items: Vec<Entry>) {
        if self.hud_open == Some(c) && self.menu.is_some() {
            self.menu = None;
            self.hud_open = None;
            return;
        }
        let Some(at) = self.hud.rect(c) else { return };
        let anchor = Vec2::new(at.x, at.bottom() + 8.0 * self.ui());
        let m = self.layout_menu(menu::NO_CARD, items, anchor);
        log::info!("menu of {c:?}: {:?}", menu_labels(&m.items));
        self.menu = Some(m);
        self.hud_open = Some(c);
        self.menu_pressed = false;
    }

    /// A right-click on the empty board: what can be made and done there,
    /// each with its key.
    pub(crate) fn open_board_menu(&mut self, at: Vec2) {
        let groups: [&[Command]; 4] = [
            &[Command::NewCard, Command::NewNote, Command::NewSticky],
            &[Command::Tidy, Command::Fold, Command::Fit],
            &[Command::Reopen, Command::Undo, Command::Redo],
            &[Command::History, Command::Downloads, Command::Settings, Command::Shortcuts],
        ];
        let mut items: Vec<Entry> = Vec::new();
        for (i, g) in groups.iter().enumerate() {
            if i > 0 {
                items.push(Entry::separator());
            }
            items.extend(g.iter().copied().map(Entry::command));
        }
        let m = self.layout_menu(menu::NO_CARD, items, at);
        log::info!("board menu: {:?}", menu_labels(&m.items));
        self.menu = Some(m);
        self.hud_open = None;
        self.menu_pressed = false;
    }

    /// A click on a label's shield: the site's blocking entries (the
    /// end of the page menu), under the shield.
    pub(crate) fn open_shield_menu(&mut self, id: NodeId) {
        let Some(b) = self.blocking_state(id) else { return };
        let Some(at) = self.shields.iter().find(|(s, _)| *s == id).map(|(_, r)| *r) else { return };
        let items = web::menu::blocking_items(&b).into_iter().map(Entry::Page).collect();
        let anchor = Vec2::new(at.x, at.bottom() + 2.0 * self.ui());
        let m = self.layout_menu(id, items, anchor);
        log::info!("shield menu on card {}: {:?}", id.0, menu_labels(&m.items));
        self.menu = Some(m);
    }

    /// The count badge on a stack's front, screen px. Sized by digit count, so
    /// hit testing needs no text measuring.
    pub(crate) fn stack_badge(&self, n: &scene::Node) -> Option<Rect> {
        let len = self.scene.stack_of(n.id)?.members.len();
        let size = 11.0 * self.ui();
        let frame = self.to_screen_rect(n.frame());
        let h = (size * 1.7).round();
        let w = (size * (0.62 * len.to_string().len() as f32 + 1.0)).max(h);
        // At the start of the card's label.
        let ui = self.ui();
        Some(Rect::new(frame.x, (frame.y - BAR_GAP * ui - BAR_H * ui / 2.0 - h / 2.0).round(), w, h))
    }

    /// O, or a click on the count badge: list the stack's members under its
    /// badge, to bring one to the front (or unfold the trail).
    pub(crate) fn open_outline(&mut self, front: NodeId) {
        let Some(stack) = self.scene.stack_of(front) else { return };
        let front = stack.front;
        let members: Vec<(NodeId, String)> = stack
            .ids()
            .filter_map(|id| self.scene.all_nodes().find(|n| n.id == id))
            .map(|n| (n.id, if n.title.trim().is_empty() { n.url.clone() } else { n.title.clone() }))
            .collect();
        let refs: Vec<(NodeId, &str)> = members.iter().map(|(id, t)| (*id, t.as_str())).collect();
        let items = menu::outline(&refs, front);
        let Some(badge) = self.scene.node(front).and_then(|n| self.stack_badge(n)) else { return };
        let anchor = Vec2::new(badge.x, badge.bottom() + 4.0 * self.ui());
        let m = self.layout_menu(front, items, anchor);
        log::info!("outline of stack {}: {:?}", front.0, menu_labels(&m.items));
        self.menu = Some(m);
        self.menu_pressed = false;
    }

    pub(crate) fn activate_menu_item(&mut self, i: usize) {
        let Some(m) = self.menu.take() else { return };
        let id = m.page;
        let (action, label) = match m.items.get(i).cloned() {
            Some(Entry::Page(MenuItem::Action { action, enabled: true, label })) => (action, label),
            Some(Entry::Member { id: member, label, .. }) => {
                log::info!("outline: {label}");
                if self.scene.show(member) {
                    let stack = self.scene.stack_of(member).expect("shown in a stack");
                    log::info!("show: card {} ({} of {})", member.0, stack.front_index() + 1, stack.members.len());
                    self.after_fold();
                    self.dirty = true;
                }
                return;
            }
            Some(Entry::Unfold) => {
                log::info!("outline: unfold");
                self.unfold_stack(id);
                return;
            }
            Some(Entry::Command { cmd, label, .. }) => {
                log::info!("menu: {label}");
                // A card's own rows act on that card.
                if matches!(cmd, Command::JoinTrail | Command::JoinTrailKeep | Command::LinkCard | Command::LeaveTrail) && id != menu::NO_CARD && self.focused != Some(id) {
                    if !self.selection.contains(id) {
                        self.selection.clear();
                    }
                    self.focus(Some(id));
                }
                self.run_command(cmd);
                return;
            }
            Some(Entry::Space { id: space, label, .. }) => {
                log::info!("menu: space {label}");
                if space != self.spaces.active {
                    self.switch_space(space);
                }
                return;
            }
            _ => return,
        };
        log::info!("menu: {label}");
        // Edits act on the focused page; the right-click focused this card.
        if self.focused != Some(id) {
            self.focus(Some(id));
        }
        self.keys_in_page = true;
        let open_beside = |app: &mut App, url: &str| {
            if let Some(new) = app.open_card(Rect::default(), url, Some(id)) {
                app.ensure_visible(new);
            }
        };
        match action {
            MenuAction::OpenBeside(url) => open_beside(self, &url),
            MenuAction::Download(url) => {
                if let Some(c) = self.cards.get(&id) {
                    log::info!("menu: download {url}");
                    c.page.download(&url);
                }
            }
            MenuAction::Search(q) => {
                let url = omnibox::search_url(&q, self.settings.search);
                open_beside(self, &url)
            }
            MenuAction::AddToNote(text) => self.add_to_note(id, &text),
            MenuAction::SaveMarkdown => self.save_markdown(Some(id)),
            MenuAction::ToggleSticky => self.toggle_sticky(id),
            MenuAction::ToggleBlocking => self.toggle_blocking(Some(id)),
            MenuAction::ToggleJavaScript => self.toggle_site_content(id, scene::sitesettings::Content::JavaScript),
            MenuAction::ToggleImages => self.toggle_site_content(id, scene::sitesettings::Content::Images),
            MenuAction::ReplaceMisspelling(word) => {
                if let Some(c) = self.cards.get(&id) {
                    c.page.replace_misspelling(&word);
                }
            }
            MenuAction::AddToDictionary(word) => {
                if let Some(c) = self.cards.get(&id) {
                    c.page.add_to_dictionary(&word);
                }
            }
            MenuAction::CopyText(t) => self.clipboard.set_text(&t),
            MenuAction::Copy => self.page_copy(false, None),
            MenuAction::Cut => self.page_copy(true, None),
            MenuAction::Paste => self.page_paste(None),
            MenuAction::Undo | MenuAction::Redo | MenuAction::SelectAll => {
                if let Some(p) = self.focused_page() {
                    p.edit(match action {
                        MenuAction::Undo => EditCommand::Undo,
                        MenuAction::Redo => EditCommand::Redo,
                        _ => EditCommand::SelectAll,
                    });
                }
            }
            MenuAction::Back | MenuAction::Forward | MenuAction::Reload => {
                if let Some(p) = self.focused_page() {
                    match action {
                        MenuAction::Back => p.back(),
                        MenuAction::Forward => p.forward(),
                        _ => p.reload(),
                    }
                }
            }
            MenuAction::Inspect(x, y) => self.inspect_at(id, (x as f32, y as f32)),
        }
    }
}
