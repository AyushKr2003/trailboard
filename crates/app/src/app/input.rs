//! Input.

use std::time::{Duration, Instant};

use scene::dock;
use scene::keys::{self, Outcome};
use scene::userkeys::{self, Mode as KeyMode};
use scene::{Node, NodeId, Rect, selection, Vec2};
use web::MouseButton as WebButton;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase};
use winit::keyboard::{KeyCode, NamedKey};

use crate::keyboard::keymap::{self, KeyIn};
use crate::ui::omnibox::{Omnibox, Target};
use crate::ui::palette::{self, Item};
use super::{App, Confirm, cursor_icon, DOCK_GAP, Drag, Hit, home_path, MIN_CARD, PageKeys, RESIZE_GRIP, WHEEL_LINE};

impl App {
    pub(crate) fn hit(&self, screen: Vec2) -> Hit {
        if let Some(id) = self.read {
            return if self.read_layout().content.contains(screen) { self.content_hit(id, screen) } else { Hit::ReadBar(id) };
        }
        let w = self.to_world(screen);
        let top = self.scene.topmost_at(w);
        // Badges stick out past their card's corner, so they are hit first,
        // unless a card drawn above covers them.
        let top_z = top.and_then(|t| self.scene.node(t)).map(|n| n.z);
        let badge = self
            .scene
            .stacks()
            .iter()
            .filter_map(|s| self.scene.node(s.front))
            .filter(|n| top_z.is_none_or(|z| n.z >= z))
            .filter(|n| self.stack_badge(n).is_some_and(|b| b.contains(screen)))
            .max_by_key(|n| n.z);
        if let Some(n) = badge {
            return Hit::Badge(n.id);
        }
        // Trail tags stick out above their card the same way.
        let tag = self
            .tags
            .iter()
            .rfind(|(t, r)| r.contains(screen) && self.scene.node(*t).is_some_and(|n| top_z.is_none_or(|z| n.z >= z)));
        if let Some(&(id, _)) = tag {
            return Hit::Tag(id);
        }
        // A card's label sits above it, off the card: its buttons, then the
        // label itself (the card's handle). A card drawn above covers them.
        let shown = |id: NodeId| self.scene.node(id).is_some_and(|n| top_z.is_none_or(|z| n.z >= z));
        if let Some(&(hit, _)) = self.bar_buttons.iter().rfind(|(h, r)| r.contains(screen) && h.id().is_some_and(shown)) {
            return hit;
        }
        if let Some(&(id, _)) = self.shields.iter().rfind(|(id, r)| r.contains(screen) && shown(*id)) {
            return Hit::Shield(id);
        }
        if let Some(&(id, _)) = self.bars.iter().rfind(|(id, r)| r.contains(screen) && shown(*id)) {
            return Hit::Header(id);
        }
        let Some(id) = top else { return Hit::Board };
        let n = self.scene.node(id).expect("hit node exists");
        let k = self.camera.pixels_per_unit(self.scale());
        let grip = RESIZE_GRIP / k.min(1.0);
        if w.y < n.rect.y {
            // A sticky note's strip moves it.
            return Hit::Header(id);
        }
        if w.x > n.rect.right() - grip.min(n.rect.w / 4.0) && w.y > n.rect.bottom() - grip.min(n.rect.h / 4.0) {
            return Hit::Grip(id);
        }
        self.content_hit(id, screen)
    }

    pub(crate) fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        let p = self.cursor;
        let mods = keymap::modifiers(self.mods);
        if state == ElementState::Pressed {
            self.board_pan.stop(); // a grab holds the board where it is
            self.pending.clear(); // a click ends a half-typed count
        }
        // A click outside the shortcut sheet closes it; none does anything else.
        if self.hints {
            if state == ElementState::Pressed && !self.sheet.panel.contains(p) {
                self.hints = false;
                log::info!("hints hidden");
                self.menu_pressed = true; // swallow the release too
            } else if state == ElementState::Pressed
                && let Some(i) = self.sheet.stop_at(p)
            {
                // A section's stop: glide to it.
                self.sheet.go_to_stop(i);
                log::info!("hints: to section {i}");
            }
            return;
        }
        // A click while asking to delete the space keeps it, and does nothing else.
        if let Some(confirm) = self.confirm {
            if state == ElementState::Pressed {
                self.confirm = None;
                self.decline(confirm);
                self.menu_pressed = true;
            }
            return;
        }
        // While card letters are up, a click on a labelled card picks it;
        // anywhere else cancels. Either way it does nothing else.
        if let Some(cp) = self.card_pick.as_ref() {
            if state == ElementState::Pressed {
                let hit = self.hit(p).id();
                let target = cp.targets.iter().copied().find(|&t| Some(self.shown_card(t)) == hit);
                let cp = self.card_pick.take().expect("picking");
                match target {
                    Some(t) if cp.cut => self.cut_line(cp.cards[0], t),
                    Some(t) if cp.link => self.link_cards(&cp.cards, t),
                    Some(t) => self.join_cards(&cp.cards, t, cp.keep || self.mods.shift_key()),
                    None => self.toast(if cp.cut { "Cut cancelled" } else if cp.link { "Link cancelled" } else { "Join cancelled" }),
                }
                self.menu_pressed = true; // swallow the release
                self.dirty = true;
            }
            return;
        }
        // A press ends link hints, then does what it always does.
        if state == ElementState::Pressed && self.link_hints.take().is_some() {
            log::info!("link hints cancelled by a click");
        }
        // A press keeps a keyboard arrangement, then does what it always does.
        if state == ElementState::Pressed {
            self.finish_arrange(true);
        }
        // The context menu takes clicks first. A click outside closes it; a right
        // click outside still goes through so it can open a new menu.
        if let Some(m) = &self.menu {
            if state == ElementState::Pressed {
                if m.rect.contains(p) {
                    self.menu_pressed = true;
                    return;
                }
                self.menu = None;
                if button != MouseButton::Right {
                    return;
                }
            } else if std::mem::take(&mut self.menu_pressed) {
                if let Some(i) = m.item_at(p) {
                    self.activate_menu_item(i);
                }
                return;
            }
        }
        if state == ElementState::Released && self.sidebar_press.is_some() && button == MouseButton::Left {
            self.menu_pressed = false;
            self.sidebar_release();
            return;
        }
        if state == ElementState::Released && std::mem::take(&mut self.menu_pressed) {
            return;
        }
        // The palette: a click on a result runs it, a click in the field or
        // the list keeps it open, a click elsewhere closes it and goes through.
        if state == ElementState::Pressed
            && let Some(ob) = self.omnibox.as_ref().filter(|ob| ob.target.is_list())
        {
            let rows = self.list_rows(ob);
            let panel = self.list_panel(ob, &rows);
            let scroll = ob.scroll.min(panel.max_scroll());
            if let Some(i) = panel.row_at(p, scroll) {
                self.menu_pressed = true; // swallow the release
                // An allowed site is blocked again; the list stays.
                if let Item::AllowedSite(site) = &rows[i].item {
                    if let Some(ob) = self.omnibox.as_mut() {
                        ob.sel = i;
                    }
                    if button == MouseButton::Left {
                        self.block_site_again(site);
                    }
                    return;
                }
                self.omnibox = None;
                if button == MouseButton::Left {
                    self.run_palette(rows[i].clone(), self.mods.shift_key());
                }
                return;
            }
            if panel.contains(p) {
                self.menu_pressed = true;
                return;
            }
        }
        // The sidebar covers the board's left edge and takes its clicks; a
        // press anywhere else takes the keys from it (it stays open).
        if state == ElementState::Pressed && self.sidebar_shown() {
            if self.sidebar_rect().contains(p) {
                self.sidebar_click(p, button);
                self.menu_pressed = true; // swallow the release
                return;
            }
            self.sidebar_keys = false;
        }
        // The board's own controls take clicks before the board does: a
        // press runs one; a press between them does nothing.
        if self.read.is_none() && button == MouseButton::Left {
            if state == ElementState::Released && std::mem::take(&mut self.hud_down) {
                return;
            }
            if state == ElementState::Pressed {
                if let Some(&(_, go)) = self.nudge.iter().find(|(r, _)| r.contains(p)) {
                    self.hud_down = true;
                    self.omnibox = None;
                    match go {
                        crate::ui::hud::Go::Command(cmd) => self.run_command(cmd),
                        crate::ui::hud::Go::Palette => self.open_palette(),
                    }
                    return;
                }
                if let Some(c) = self.hud.at(p) {
                    self.hud_down = true;
                    self.omnibox = None;
                    self.hud_press(c);
                    return;
                }
                if self.hud.covers(p) {
                    self.hud_down = true;
                    return;
                }
            }
        }
        log::debug!("mouse {state:?} {button:?} at {p:?} hit={:?}", self.hit(p));
        if state == ElementState::Pressed {
            // Track multi-clicks for the page and for double-click on title bars.
            let now = Instant::now();
            self.click_count = match self.last_click {
                Some((t, at, b))
                    if b == button && now - t < Duration::from_millis(400) && (at.x - p.x).abs() < 5.0 && (at.y - p.y).abs() < 5.0 =>
                {
                    self.click_count + 1
                }
                _ => 1,
            };
            self.last_click = Some((now, p, button));
            self.omnibox = None;
            self.camera_goal = None;

            if button == MouseButton::Middle {
                if let Hit::Content(id) = self.hit(p) {
                    // Middle-click in a page (open link in new card) goes to the page.
                    if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_button(x, y, mods, WebButton::Middle, false, 1);
                        self.drag = Drag::Page { id, button: WebButton::Middle };
                        return;
                    }
                }
                if self.read.is_none() {
                    self.drag = Drag::Pan { last: p };
                }
                return;
            }
            let web_button = match button {
                MouseButton::Left => WebButton::Left,
                MouseButton::Right => WebButton::Right,
                _ => return,
            };
            let shift = self.mods.shift_key();
            match self.hit(p) {
                Hit::Board if shift && button == MouseButton::Left => {
                    // Band adds to the current selection.
                    self.keys_in_page = false;
                    self.focus(None);
                    self.drag = Drag::Band { anchor: self.to_world(p) };
                }
                Hit::Board if button == MouseButton::Right => {
                    // What can be done on the board, where the pointer is.
                    self.keys_in_page = false;
                    self.focus(None);
                    self.open_board_menu(p);
                }
                Hit::Board => {
                    self.keys_in_page = false;
                    self.focus(None);
                    self.selection.clear();
                    self.drag = Drag::Pan { last: p };
                }
                Hit::ReadBar(_) => {
                    self.keys_in_page = false;
                    if self.click_count >= 2 && button == MouseButton::Left {
                        self.leave_read();
                    }
                }
                Hit::Close(id) => self.close_card(id),
                Hit::Back(id) | Hit::Forward(id) | Hit::Reload(id) => {
                    self.keys_in_page = false;
                    self.focus(Some(id));
                    if let Some(c) = self.cards.get(&id) {
                        match self.hit(p) {
                            Hit::Back(_) => c.page.back(),
                            Hit::Forward(_) => c.page.forward(),
                            _ => c.page.reload(),
                        }
                    }
                    self.menu_pressed = true; // swallow the release
                }
                Hit::Shield(id) => {
                    self.keys_in_page = false;
                    self.focus(Some(id));
                    self.open_shield_menu(id);
                    self.menu_pressed = true;
                }
                Hit::Awake(id) => {
                    self.keys_in_page = false;
                    self.set_awake(id, None);
                    self.menu_pressed = true;
                }
                Hit::Mute(id) => {
                    self.keys_in_page = false;
                    self.toggle_mute(id);
                    self.menu_pressed = true;
                }
                Hit::Tag(id) => {
                    self.keys_in_page = false;
                    self.ask_trail_name_of(id);
                    self.menu_pressed = true; // swallow the release
                }
                Hit::Badge(id) => {
                    self.keys_in_page = false;
                    self.open_outline(id);
                    // Swallow the release; releasing over a row picks it, as in a native menu.
                    self.menu_pressed = true;
                }
                Hit::Header(id) => {
                    if shift && !self.mods.alt_key() {
                        self.selection.toggle(id);
                    } else if !self.selection.contains(id) {
                        self.selection.clear();
                    }
                    self.focus(Some(id));
                    self.keys_in_page = false;
                    if self.mods.alt_key() {
                        // Alt+drag: let go on another card to join its trail
                        // (Alt+Shift: keeping its colour).
                        self.group = selection::drag_group(&self.scene, &self.selection, id);
                        // Ctrl+Alt: link instead (`scene::link`).
                        self.drag = Drag::Join { id, keep: shift, link: self.mods.control_key() };
                        self.set_cursor(web::Cursor::Grabbing);
                    } else if self.click_count >= 2 && !shift {
                        self.fill_view(id);
                        self.drag = Drag::None;
                    } else {
                        let n = self.scene.node(id).expect("hit node");
                        let w = self.to_world(p);
                        self.group = selection::drag_group(&self.scene, &self.selection, id);
                        self.drag = Drag::Move { id, grab: w - Vec2::new(n.rect.x, n.rect.y) };
                    }
                }
                Hit::Grip(id) => {
                    self.selection.clear();
                    self.focus(Some(id));
                    self.keys_in_page = false;
                    let start = self.scene.node(id).expect("hit node").rect;
                    self.drag = Drag::Resize { id, anchor: self.to_world(p), start };
                }
                Hit::Inspector(id) => {
                    self.selection.clear();
                    self.focus(Some(id));
                    self.set_dock_keys(true);
                    self.give_keys(PageKeys::Passthrough);
                    if let (Some(d), Some((x, y))) = (self.docks.get(&id), self.dock_point(id, p)) {
                        d.page.mouse_button(x, y, mods, web_button, false, self.click_count.min(3));
                    }
                    self.drag = Drag::Dock { id, button: web_button };
                }
                Hit::Divider(id) => {
                    self.selection.clear();
                    self.focus(Some(id));
                    self.drag = Drag::Divider { id };
                }
                Hit::DockSide(id) => {
                    self.keys_in_page = false;
                    self.focus(Some(id));
                    let side = self.docks.get(&id).map_or(self.dock_side, |d| d.side.flip());
                    self.dock_to(id, side);
                    self.menu_pressed = true;
                }
                Hit::Undock(id) => {
                    self.undock(id);
                    self.menu_pressed = true;
                }
                Hit::Redock(id) => {
                    if let Some(&src) = self.inspectors.get(&id) {
                        self.close_inspector(id);
                        self.open_dock(src, self.dock_side, None);
                    }
                    self.menu_pressed = true;
                }
                Hit::Content(id) => {
                    self.selection.clear();
                    self.focus(Some(id));
                    self.set_dock_keys(false);
                    // Insert until the page says the click wasn't in a field
                    // (`WebEvent::Focus`), so typing right after a click lands.
                    self.give_keys(PageKeys::Insert);
                    if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_button(x, y, mods, web_button, false, self.click_count.min(3));
                        // The innermost section clicked in becomes the scroll target.
                        if web_button == WebButton::Left {
                            self.pick_section_at(id, x, y);
                        }
                    }
                    self.drag = Drag::Page { id, button: web_button };
                }
            }
        } else {
            if let Drag::Page { id, button: b } = self.drag
                && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                    card.page.mouse_button(x, y, mods, b, true, self.click_count.min(3));
                }
            if let Drag::Dock { id, button: b } = self.drag
                && let (Some(d), Some((x, y))) = (self.docks.get(&id), self.dock_point(id, p)) {
                    d.page.mouse_button(x, y, mods, b, true, self.click_count.min(3));
                }
            match self.drag {
                Drag::Resize { id, start, .. } => self.history.moved(&self.scene, id, start),
                Drag::Move { .. } => self.history.moved_many(&self.scene, &std::mem::take(&mut self.group)),
                Drag::Join { keep, link, .. } => {
                    let target = self.join_drop_target(p);
                    let cards: Vec<NodeId> = std::mem::take(&mut self.group).into_iter().map(|(g, _)| g).collect();
                    self.dirty = true;
                    match target {
                        Some(t) if link => self.link_cards(&cards, t),
                        Some(t) => self.join_cards(&cards, t, keep),
                        None => log::info!("join: let go on no card"),
                    }
                }
                Drag::Band { .. } => {
                    let band = self.band().expect("band drag");
                    self.selection.extend(selection::in_band(&self.scene, band));
                    log::info!("selected {} card(s)", self.selection.len());
                }
                _ => {}
            }
            self.drag = Drag::None;
        }
    }

    pub(crate) fn cursor_moved(&mut self, p: Vec2) {
        self.pointer_in = true;
        if let Some(m) = self.menu.as_mut()
            && m.rect.contains(p)
            && matches!(self.drag, Drag::None)
        {
            m.hover = m.item_at(p);
            self.cursor = p;
            self.set_cursor(web::Cursor::Default);
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            m.hover = None;
        }
        let list = self.omnibox.as_ref().filter(|ob| ob.target.is_list()).map(|ob| self.list_panel(ob, &self.list_rows(ob)));
        if let (Some(panel), Some(ob)) = (list, self.omnibox.as_mut())
            && panel.contains(p)
        {
            if let Some(i) = panel.row_at(p, ob.scroll.min(panel.max_scroll())) {
                ob.sel = i;
            }
            self.cursor = p;
            self.set_cursor(web::Cursor::Default);
            return;
        }
        self.sidebar_hover = None;
        self.sidebar_spot = None;
        // Over one of the board's own controls: it lights up, and after a
        // moment says its name and key.
        let over = if matches!(self.drag, Drag::None) && self.read.is_none() && self.sidebar_press.is_none() && self.omnibox.as_ref().is_none_or(|ob| !ob.target.is_list()) {
            self.hud.at(p)
        } else {
            None
        };
        let on_chrome = over.is_some()
            || (matches!(self.drag, Drag::None) && self.read.is_none() && (self.hud.covers(p) || self.nudge.iter().any(|(r, _)| r.contains(p))));
        match (over, self.hud_hover) {
            (Some(c), Some((old, _))) if c == old => {}
            (Some(c), _) => self.hud_hover = Some((c, Instant::now())),
            (None, _) => self.hud_hover = None,
        }
        if on_chrome {
            self.hover = Hit::Board;
            self.cursor = p;
            let button = over.is_some() || self.nudge.iter().any(|(r, _)| r.contains(p));
            self.set_cursor(if button { web::Cursor::Pointer } else { web::Cursor::Default });
            return;
        }
        if self.sidebar_press.is_some() {
            self.sidebar_press_moved(p);
            self.cursor = p;
            self.set_cursor(web::Cursor::Default);
            return;
        }
        if self.sidebar_shown() && matches!(self.drag, Drag::None) && self.sidebar_rect().contains(p) {
            let (rows, panel) = self.sidebar_layout();
            let closable = |i: usize| matches!(rows.get(i), Some(scene::sidebar::Row::Card { .. }));
            self.sidebar_hover = panel.row_at(p, self.sidebar.scroll);
            self.sidebar_spot = panel.spot_at(p, self.spaces.spaces.len(), self.sidebar.scroll, &closable);
            self.cursor = p;
            // The head's controls, the close buttons and the foot are buttons.
            self.set_cursor(if self.sidebar_spot.is_some() { web::Cursor::Pointer } else { web::Cursor::Default });
            return;
        }
        let prev = self.cursor;
        self.cursor = p;
        let mods = keymap::modifiers(self.mods);
        match self.drag {
            Drag::Pan { last } => {
                self.camera.pan_pixels(p - last, self.scale());
                self.drag = Drag::Pan { last: p };
                self.camera_moved();
            }
            Drag::Move { id, grab } => {
                self.dirty = true;
                let w = self.to_world(p);
                if let Some(&(_, start)) = self.group.iter().find(|(g, _)| *g == id) {
                    let d = Vec2::new(w.x - grab.x - start.x, w.y - grab.y - start.y);
                    selection::move_group(&mut self.scene, &self.group, d);
                }
            }
            Drag::Band { .. } => self.set_cursor(web::Cursor::Crosshair),
            Drag::Join { .. } => {
                self.dirty = true;
                self.set_cursor(web::Cursor::Grabbing);
            }
            Drag::Resize { id, anchor, start } => {
                self.dirty = true;
                let d = self.to_world(p) - anchor;
                if let Some(n) = self.scene.node_mut(id) {
                    n.rect.w = (start.w + d.x).max(MIN_CARD);
                    n.rect.h = (start.h + d.y).max(MIN_CARD);
                }
            }
            Drag::Page { id, button } => {
                let held = match button {
                    WebButton::Left => web::flags::LEFT_MOUSE,
                    WebButton::Middle => web::flags::MIDDLE_MOUSE,
                    WebButton::Right => web::flags::RIGHT_MOUSE,
                };
                if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                    card.page.mouse_move(x, y, mods | held, false);
                }
            }
            Drag::Dock { id, button } => {
                let held = match button {
                    WebButton::Left => web::flags::LEFT_MOUSE,
                    WebButton::Middle => web::flags::MIDDLE_MOUSE,
                    WebButton::Right => web::flags::RIGHT_MOUSE,
                };
                if let (Some(d), Some((x, y))) = (self.docks.get(&id), self.dock_point(id, p)) {
                    d.page.mouse_move(x, y, mods | held, false);
                }
            }
            Drag::Divider { id } => {
                if let Some((r, css)) = self.content_view(id)
                    && let Some(d) = self.docks.get_mut(&id)
                {
                    let gap = DOCK_GAP * r.w / css.x.max(1e-3);
                    d.share = dock::share_at(r, d.side, p.x, p.y, gap);
                }
            }
            Drag::None => {
                let hit = self.hit(p);
                let prev_hit = self.hit(prev);
                if let Hit::Inspector(id) = hit
                    && let (Some(d), Some((x, y))) = (self.docks.get(&id), self.dock_point(id, p)) {
                        d.page.mouse_move(x, y, mods, false);
                    }
                if let (Hit::Inspector(old), false) = (prev_hit, hit == prev_hit)
                    && let (Some(d), Some((x, y))) = (self.docks.get(&old), self.dock_point(old, p)) {
                        d.page.mouse_move(x, y, mods, true);
                    }
                if let Hit::Content(id) = hit
                    && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_move(x, y, mods, false);
                    }
                if let (Hit::Content(old), false) = (prev_hit, matches!(hit, Hit::Content(n) if Some(n) == prev_hit.id()))
                    && let (Some(card), Some((x, y))) = (self.cards.get(&old), self.page_point(old, p)) {
                        card.page.mouse_move(x, y, mods, true);
                    }
                self.hover = hit;
                let cursor = match hit {
                    Hit::Header(_) => web::Cursor::Grab,
                    Hit::Grip(_) => web::Cursor::ResizeSE,
                    Hit::Close(_) | Hit::Back(_) | Hit::Forward(_) | Hit::Reload(_) | Hit::Badge(_) | Hit::Shield(_) | Hit::Tag(_) | Hit::Awake(_) | Hit::Mute(_) => web::Cursor::Pointer,
                    Hit::DockSide(_) | Hit::Undock(_) | Hit::Redock(_) => web::Cursor::Pointer,
                    Hit::Content(id) => self.cards.get(&id).map_or(web::Cursor::Default, |c| c.cursor),
                    Hit::Inspector(id) => self.docks.get(&id).map_or(web::Cursor::Default, |d| d.cursor),
                    Hit::Divider(id) => match self.docks.get(&id).map(|d| d.side) {
                        Some(dock::Side::Bottom) => web::Cursor::ResizeNS,
                        _ => web::Cursor::ResizeEW,
                    },
                    Hit::Board | Hit::ReadBar(_) => web::Cursor::Default,
                };
                self.set_cursor(cursor);
            }
        }
    }

    pub(crate) fn set_cursor(&self, cursor: web::Cursor) {
        let Some(w) = self.window() else { return };
        match cursor_icon(cursor) {
            Some(icon) => {
                w.set_cursor_visible(true);
                w.set_cursor(icon);
            }
            None => w.set_cursor_visible(false),
        }
    }

    pub(crate) fn wheel(&mut self, delta: MouseScrollDelta, phase: TouchPhase) {
        let (dx, dy, lines) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x, y, true),
            MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32, false),
        };
        // The shortcut sheet is in front of everything: the wheel scrolls it.
        if self.hints {
            let speed = self.settings.scroll_speed.factor();
            if lines {
                self.sheet.scroll_by(-dy * self.sheet.line * 2.0 * speed);
            } else {
                self.sheet.touch(dy, speed, matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled));
            }
            return;
        }
        self.camera_goal = None;
        self.menu = None;
        self.link_hints = None; // scrolling a page moves its links
        if let Some(ob) = self.omnibox.as_ref().filter(|ob| ob.target.is_list()) {
            let panel = self.list_panel(ob, &self.list_rows(ob));
            if panel.rect.contains(self.cursor) {
                let rows = if lines { -dy * 3.0 } else { -dy / panel.row_h };
                let ob = self.omnibox.as_mut().expect("palette open");
                ob.scroll = panel.scroll_by(ob.scroll, rows.round() as isize);
                return;
            }
        }
        if self.sidebar_shown() && self.sidebar_rect().contains(self.cursor) {
            let (_, panel) = self.sidebar_layout();
            let rows = if lines {
                -dy * 3.0
            } else {
                -dy / (crate::trails::panel::ROW_H * self.ui())
            };
            self.sidebar.scroll = panel.scroll_by(self.sidebar.scroll, rows.round() as isize);
            return;
        }
        if self.mods.control_key() {
            if self.read.is_some() {
                return; // the board isn't on screen
            }
            let steps = if lines { dy } else { dy / 60.0 };
            self.camera.zoom_about(
                1.15f32.powf(steps),
                self.cursor,
                self.viewport(),
                self.scale(),
            );
            self.camera_moved();
            return;
        }
        if let Hit::Inspector(id) = self.hit(self.cursor)
            && Some(id) == self.focused
        {
            // The inspector scrolls as Chromium does it, without our glide.
            if let (Some(d), Some((x, y))) = (self.docks.get(&id), self.dock_point(id, self.cursor)) {
                let speed = self.settings.scroll_speed.factor();
                let (dx, dy) = if lines { (dx * WHEEL_LINE * speed, dy * WHEEL_LINE * speed) } else { (dx, dy) };
                d.page.mouse_wheel(x, y, keymap::modifiers(self.mods), dx as i32, dy as i32, !lines);
            }
            self.board_pan.stop();
            return;
        }
        if let Hit::Content(id) | Hit::Grip(id) = self.hit(self.cursor)
            && Some(id) == self.focused
        {
            if let (Some(card), Some((x, y))) =
                (self.cards.get(&id), self.page_point(id, self.cursor))
            {
                // A wheel notch goes at once and Chromium glides it;
                // touchpad movement goes at once, precise, and may
                // fling after (`tick`). See `scene::scroll`.
                let speed = self.settings.scroll_speed.factor();
                let d = Vec2::new(dx, dy);
                let now = if lines {
                    self.page_scroll.wheel((id, x, y), d, speed)
                } else {
                    let ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
                    self.page_scroll.touch((id, x, y), d, speed, ended)
                };
                if now != Vec2::default() {
                    card.page.mouse_wheel(
                        x,
                        y,
                        keymap::modifiers(self.mods),
                        now.x as i32,
                        now.y as i32,
                        !lines,
                    );
                }
            }
            self.board_pan.stop();
            return;
        }
        if self.read.is_some() {
            return;
        }
        // The board pans like a page scrolls: a notch glides (`step_board_pan`),
        // the touchpad follows the fingers and flings.
        let speed = self.settings.scroll_speed.factor();
        let d = Vec2::new(dx, dy);
        if lines {
            self.board_pan.wheel(d, speed);
        } else {
            let ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
            let now = self.board_pan.touch(d, speed, ended);
            if now != Vec2::default() {
                self.camera.pan_pixels(now, self.scale());
                self.camera_moved();
            }
        }
    }

    /// This frame's part of a wheel glide or touchpad fling on the board.
    /// Runs every frame, busy or not: the touchpad's fling speed is timed by it.
    pub(crate) fn step_board_pan(&mut self, dt: f32) {
        // The camera is going somewhere else, or the board is off screen.
        if self.board_pan.busy() && (self.camera_goal.is_some() || self.read.is_some()) {
            self.board_pan.stop();
        }
        let d = self.board_pan.step(dt);
        if d != Vec2::default() {
            self.camera.pan_pixels(d, self.scale());
            self.camera_moved();
        }
    }

    /// Shows the shortcut sheet, from the top with no filter.
    pub(crate) fn show_hints(&mut self) {
        self.hints = true;
        self.sheet = crate::keyboard::key_sheet::Sheet::open(self.still);
        self.sheet_sections = crate::keyboard::key_sheet::sections(&self.keys);
        log::info!("hints shown");
    }

    /// A key while the shortcut sheet shows: typing filters it, the arrows,
    /// Page keys and Home/End scroll it, Esc clears the filter and then
    /// closes it, as do ? and F1. Other keys do nothing.
    pub(crate) fn sheet_key(&mut self, ev: &KeyIn, chord: bool) {
        let sh = &mut self.sheet;
        let close = |app: &mut Self| {
            app.hints = false;
            log::info!("hints hidden");
        };
        match ev.named {
            Some(NamedKey::Escape) if !sh.filter.is_empty() => sh.clear_filter(),
            Some(NamedKey::Escape | NamedKey::F1) => close(self),
            Some(NamedKey::ArrowDown) => sh.scroll_by(sh.line),
            Some(NamedKey::ArrowUp) => sh.scroll_by(-sh.line),
            Some(NamedKey::PageDown) => sh.scroll_by(sh.page),
            Some(NamedKey::PageUp) => sh.scroll_by(-sh.page),
            Some(NamedKey::Home) => sh.scroll_to(0.0),
            Some(NamedKey::End) => sh.scroll_to(sh.max),
            Some(NamedKey::Backspace) if chord => sh.clear_filter(),
            Some(NamedKey::Backspace) => sh.backspace(),
            _ if chord => {}
            // ? closes the sheet it opened, unless it is being typed into a filter.
            _ if ev.text.as_deref() == Some("?") && sh.filter.is_empty() => close(self),
            _ => {
                if let Some(t) = ev.text.as_deref().filter(|t| !t.trim().is_empty() || !sh.filter.is_empty()) {
                    sh.type_text(t);
                }
            }
        }
    }

    pub(crate) fn key(&mut self, ev: KeyIn) {
        let pressed = ev.pressed;
        let ctrl = self.mods.control_key();
        let shift = self.mods.shift_key();
        let alt = self.mods.alt_key();
        // Shortcuts follow the logical key (layout-aware), like browsers do.
        let code = ev.code;

        let is_question = ev.text.as_deref() == Some("?") || (shift && code == Some(KeyCode::Slash));
        // A half-typed count or sequence ends when something else takes the keys.
        if pressed
            && (self.hints
                || self.confirm.is_some()
                || self.link_hints.is_some()
                || self.arrange.is_some()
                || self.menu.is_some()
                || self.omnibox.is_some()
                || self.typing())
        {
            self.pending.clear();
        }
        if self.hints {
            if pressed {
                self.sheet_key(&ev, ctrl || alt);
            }
            return;
        }
        if let Some(confirm) = self.confirm {
            // Enter deletes the space; any other key (but a modifier) keeps it and is dropped.
            if pressed && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)) {
                self.confirm = None;
                // Enter or y says yes; n, Esc and the rest say no.
                let yes = ev.named == Some(NamedKey::Enter) || (!ctrl && !alt && ev.text.as_deref().is_some_and(|t| t.eq_ignore_ascii_case("y")));
                match (yes, confirm) {
                    (true, Confirm::DeleteSpace) => self.delete_space(),
                    (true, Confirm::CloseOthers(keep)) => self.close_others(keep),
                    (true, Confirm::ClearHistory) => self.clear_history(),
                    (true, Confirm::ClearCookies) => self.clear_cookies(),
                    (true, Confirm::Permission) => self.answer_ask(true),
                    (true, Confirm::Dialog) => self.answer_dialog(true, ""),
                    _ => self.decline(confirm),
                }
            }
            return;
        }
        if self.link_hints.is_some() {
            // The user's hints keys (one chord with Ctrl or Alt, or an F key).
            if pressed && let Some(a) = self.user_key(KeyMode::Hints, &ev) {
                self.link_hints = None;
                return self.run_key_action(a, None);
            }
            // Letters type a label; Backspace takes one back; Esc or any other
            // key (but a modifier) ends the hints and is dropped.
            if pressed && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)) {
                self.link_hint_key(&ev, ctrl || alt);
            }
            return;
        }
        if self.card_pick.is_some() {
            if pressed && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)) {
                self.card_pick_key(&ev, ctrl || alt);
            }
            return;
        }
        if self.arrange.is_some()
            && pressed
            && !matches!(ev.named, Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super))
            && self.arrange_key(code)
        {
            return;
        }
        let f1 = ev.named == Some(NamedKey::F1) && !ctrl && !alt;
        // ? shows them on the board only: in a page it searches backward.
        if pressed && (f1 || (is_question && !ctrl && !alt && !self.entered() && self.omnibox.is_none() && self.menu.is_none())) {
            self.menu = None;
            self.pending.clear();
            self.show_hints();
            return;
        }

        if pressed && self.menu.is_some() {
            match ev.named {
                Some(NamedKey::Escape) => {
                    self.menu = None;
                    return;
                }
                Some(NamedKey::ArrowDown) => return self.menu.as_mut().expect("menu").step(1),
                _ if !ctrl && !alt && code == Some(KeyCode::KeyJ) => return self.menu.as_mut().expect("menu").step(1),
                _ if !ctrl && !alt && code == Some(KeyCode::KeyK) => return self.menu.as_mut().expect("menu").step(-1),
                Some(NamedKey::ArrowUp) => return self.menu.as_mut().expect("menu").step(-1),
                Some(NamedKey::Enter) => {
                    if let Some(i) = self.menu.as_ref().and_then(|m| m.hover) {
                        self.activate_menu_item(i);
                    }
                    return;
                }
                // Modifiers alone keep the menu (e.g. reaching for a shortcut).
                Some(NamedKey::Shift | NamedKey::Control | NamedKey::Alt) => {}
                _ => self.menu = None,
            }
        }

        // The palette's list: its length and how many rows fit, for the keys below.
        let palette_len = self
            .omnibox
            .as_ref()
            .filter(|ob| ob.target.is_list())
            .map(|ob| self.list_panel(ob, &self.list_rows(ob)));
        // The allowed-sites list's highlighted site.
        // The user's command-line keys.
        if pressed
            && self.omnibox.as_ref().is_some_and(|ob| ob.target == Target::Palette && palette::is_command(&ob.text))
            && let Some(a) = self.user_key(KeyMode::Command, &ev)
        {
            self.omnibox = None;
            return self.run_key_action(a, None);
        }
        let setting_sel = self
            .omnibox
            .as_ref()
            .filter(|ob| matches!(ob.target, Target::AllowedSites | Target::Messages))
            .and_then(|ob| self.list_rows(ob).into_iter().nth(ob.sel).map(|r| r.item));
        if let Some(ob) = self.omnibox.as_mut() {
            if !pressed {
                return;
            }
            let before = ob.text.clone();
            if ctrl && code == Some(KeyCode::KeyJ) && ob.target == Target::Downloads {
                self.omnibox = None;
                return;
            }
            // A file chooser's path: Tab completes the last part.
            if matches!(ob.target, Target::Files(_)) && !ctrl && !alt && ev.named == Some(NamedKey::Tab) {
                match crate::page::pathfield::complete(&ob.text, &home_path(), crate::page::pathfield::read_dir) {
                    Some(t) => ob.set(&t),
                    None => log::debug!("files: nothing to complete for {:?}", ob.text),
                }
                return;
            }
            let command_line = ob.target == Target::Palette && palette::is_command(&ob.text);
            // Command line: Up / Down walk the lines run before that start
            // with what is typed (Ctrl+N / Ctrl+P still move in the list).
            if command_line && !ctrl && !alt && matches!(ev.named, Some(NamedKey::ArrowUp | NamedKey::ArrowDown)) {
                let walk = ob.walk.get_or_insert_with(|| scene::command::Walk::new(&ob.text));
                if let Some(line) = walk.step(&self.recent_commands, ev.named == Some(NamedKey::ArrowUp)) {
                    ob.set(&format!(":{line}"));
                    ob.list_for = None;
                    ob.sel = 0;
                    ob.scroll = 0;
                }
                return;
            }
            // Tab / Shift+Tab walk the list, the field showing the row's
            // line; the list stays the one for what was typed.
            if command_line && !ctrl && !alt && ev.named == Some(NamedKey::Tab) {
                let mut ob = self.omnibox.take().expect("command line open");
                let rows = self.list_rows(&ob);
                let line_of = |i: usize| match rows.get(i).map(|r| &r.item) {
                    Some(Item::Run(l) | Item::Recent(l) | Item::Complete(l)) => Some(format!(":{l}")),
                    _ => None,
                };
                let len = rows.len();
                if len > 0 {
                    let to = if shift {
                        (ob.sel + len - 1) % len
                    } else if line_of(ob.sel).as_deref() == Some(ob.text.as_str()) {
                        (ob.sel + 1) % len
                    } else {
                        ob.sel
                    };
                    if ob.list_for.is_none() {
                        ob.list_for = Some(ob.text.clone());
                    }
                    ob.sel = to;
                    if let Some(panel) = &palette_len {
                        ob.scroll = panel.keep_visible(to, ob.scroll);
                    }
                    if let Some(l) = line_of(to) {
                        ob.set(&l);
                    }
                    ob.walk = None;
                }
                self.omnibox = Some(ob);
                return;
            }
            // Palette: Up/Down (Ctrl+K/J, Ctrl+P/N), PgUp/PgDn move the highlight.
            let step = palette_len.as_ref().and_then(|panel| match ev.named {
                Some(NamedKey::ArrowDown) => Some(1),
                Some(NamedKey::ArrowUp) => Some(-1),
                Some(NamedKey::PageDown) => Some(panel.fits(ob.scroll) as isize),
                Some(NamedKey::PageUp) => Some(-(panel.fits(ob.scroll) as isize)),
                _ if ctrl && matches!(code, Some(KeyCode::KeyJ | KeyCode::KeyN)) => Some(1),
                _ if ctrl && code == Some(KeyCode::KeyP) => Some(-1),
                _ => None,
            });
            if let (Some(step), Some(panel)) = (step, palette_len) {
                let len = panel.len();
                if len > 0 {
                    ob.sel = (ob.sel as isize + step).clamp(0, len as isize - 1) as usize;
                    ob.scroll = panel.keep_visible(ob.sel, ob.scroll);
                }
                return;
            }
            match ev.named {
                Some(NamedKey::Escape) => self.omnibox = None,
                // Readline keys, in every field. Ctrl+A and Ctrl+K are the
                // line's start and end only in the command line: elsewhere
                // Ctrl+A selects all, and Ctrl+K closes the palette.
                _ if ctrl && !alt && code == Some(KeyCode::KeyW) => ob.kill_big_word_back(),
                _ if ctrl && !alt && code == Some(KeyCode::KeyU) => ob.kill_to_start(),
                _ if ctrl && !alt && code == Some(KeyCode::KeyE) => ob.end(),
                _ if ctrl && !alt && code == Some(KeyCode::KeyY) => ob.yank(),
                _ if ctrl && !alt && code == Some(KeyCode::KeyA) && command_line => ob.home(),
                _ if ctrl && !alt && code == Some(KeyCode::KeyK) && (command_line || ob.target != Target::Palette) => ob.kill_to_end(),
                _ if alt && !ctrl && code == Some(KeyCode::KeyB) => ob.word_left(),
                _ if alt && !ctrl && code == Some(KeyCode::KeyF) => ob.word_right(),
                _ if alt && !ctrl && code == Some(KeyCode::KeyD) => ob.kill_word_forward(),
                Some(NamedKey::Backspace) if ctrl || alt => ob.kill_word_back(),
                Some(NamedKey::ArrowLeft) if ctrl || alt => ob.word_left(),
                Some(NamedKey::ArrowRight) if ctrl || alt => ob.word_right(),
                // Ctrl+Delete removes a list's row (as Shift+Delete does),
                // else the word after the caret.
                Some(NamedKey::Delete) if ctrl && !shift && ob.target == Target::Downloads => self.drop_selected_download(),
                Some(NamedKey::Delete) if ctrl && !shift && ob.target == Target::AllowedSites => {
                    if let Some(Item::AllowedSite(site)) = &setting_sel {
                        self.block_site_again(site);
                    }
                }
                Some(NamedKey::Delete) if (ctrl || shift) && ob.target == Target::Messages => {
                    if let Some(item) = &setting_sel {
                        self.drop_message_row(item);
                    }
                }
                Some(NamedKey::Delete) if ctrl && !shift && ob.target.is_list() => self.forget_selected(),
                Some(NamedKey::Delete) if ctrl && !shift => ob.kill_word_forward(),
                _ if ctrl && code == Some(KeyCode::KeyK) && ob.target == Target::Palette => self.omnibox = None,
                _ if ctrl && code == Some(KeyCode::KeyF) && matches!(ob.target, Target::Find(_)) => ob.select_all = true,
                Some(NamedKey::Enter) if matches!(ob.target, Target::Find(_)) => {
                    if let Target::Find(id) = ob.target
                        && let Some(c) = self.cards.get(&id)
                    {
                        // Up the page after ?, so Shift+Enter goes down.
                        c.page.find(&ob.text, shift == self.find_back, true);
                    }
                }
                _ if ctrl && code == Some(KeyCode::KeyH) && ob.target == Target::History => {
                    self.omnibox = None
                }
                Some(NamedKey::Delete) if shift && ob.target == Target::Downloads => {
                    self.drop_selected_download()
                }
                // Allowed sites: Enter or Shift+Delete blocks ads there again; the list stays.
                Some(NamedKey::Enter | NamedKey::Delete) if ob.target == Target::AllowedSites => {
                    if (ev.named == Some(NamedKey::Enter) || shift)
                        && let Some(Item::AllowedSite(site)) = &setting_sel
                    {
                        self.block_site_again(site);
                    }
                }
                Some(NamedKey::Delete) if shift && ob.target.is_list() => self.forget_selected(),
                // Downloads: Ctrl+Enter shows the file in its folder.
                Some(NamedKey::Enter) if ctrl && ob.target == Target::Downloads => {
                    let ob = self.omnibox.take().expect("downloads open");
                    if let Some(palette::Item::Download(key)) = self.list_rows(&ob).into_iter().nth(ob.sel).map(|r| r.item) {
                        self.show_download_folder(key);
                    }
                }
                // Command line: Ctrl+Enter runs and keeps the line open.
                Some(NamedKey::Enter) if ctrl && command_line => {
                    let ob = self.omnibox.take().expect("command line open");
                    if let Some(row) = self.list_rows(&ob).into_iter().nth(ob.sel) {
                        let runs = match &row.item {
                            Item::Run(_) | Item::Recent(_) => true,
                            Item::Complete(l) => userkeys::steps(l, &self.keys.aliases).is_ok(),
                            _ => false,
                        };
                        self.run_palette(row, false);
                        if runs && self.omnibox.is_none() {
                            self.fill_command_line(ob.text.trim_start_matches(':'));
                        }
                    }
                }
                Some(NamedKey::Enter) if ob.target.is_list() => {
                    let ob = self.omnibox.take().expect("palette open");
                    let rows = self.list_rows(&ob);
                    if let Some(row) = rows.into_iter().nth(ob.sel) {
                        self.run_palette(row, shift);
                    }
                }
                Some(NamedKey::Enter) => {
                    let ob = self.omnibox.take().expect("omnibox open");
                    if ob.target == Target::RenameSpace {
                        self.rename_space(&ob.text);
                    } else if let Target::SaveAs(id) = ob.target {
                        self.save_download_as(id, &ob.text);
                    } else if let Target::Prompt(id) = ob.target {
                        if self.dialogs.first().is_some_and(|d| d.id == id) {
                            self.answer_dialog(true, &ob.text);
                        }
                    } else if let Target::Files(id) = ob.target {
                        self.choose_files(id, &ob.text);
                    } else if let Target::NameTrail(id) = ob.target {
                        self.name_trail(id, &ob.text);
                    } else if let Some(url) = self.settings.resolve_url(&ob.text) {
                        match ob.target {
                            // Shift+Enter: beside the current card instead.
                            _ if shift => self.open_beside_current(&url),
                            Target::Beside => self.open_beside_current(&url),
                            Target::Behind => match self.current().filter(|&id| self.scene.node(id).is_some()) {
                                Some(src) => {
                                    self.open_behind(src, &url);
                                    self.toast("Opened beside · still here");
                                }
                                None => {
                                    self.new_card_at_center(&url);
                                }
                            },
                            Target::Focused if self.focused_page().is_some() => self.load_here(&url),
                            _ => {
                                // Typed an address: the new page is what you use next.
                                if self.new_card_at_center(&url).is_some() {
                                    self.give_keys(PageKeys::Page);
                                }
                            }
                        }
                    }
                }
                Some(NamedKey::Backspace) => ob.backspace(),
                Some(NamedKey::Delete) => ob.delete(),
                Some(NamedKey::ArrowLeft) => ob.left(),
                Some(NamedKey::ArrowRight) => ob.right(),
                Some(NamedKey::Home) => ob.home(),
                Some(NamedKey::End) => ob.end(),
                _ if ctrl && code == Some(KeyCode::KeyA) => ob.select_all = true,
                _ if ctrl && matches!(code, Some(KeyCode::KeyC | KeyCode::KeyX)) => {
                    if ob.select_all && !ob.text.is_empty() {
                        let text = ob.text.clone();
                        if code == Some(KeyCode::KeyX) {
                            ob.insert("");
                        }
                        self.clipboard.set_text(&text);
                    }
                }
                _ if ctrl && code == Some(KeyCode::KeyV) => {
                    if let Some(t) = self.clipboard.text() {
                        let line = t.split_whitespace().collect::<Vec<_>>().join(" ");
                        self.omnibox.as_mut().expect("omnibox open").insert(&line);
                    }
                }
                _ => {
                    if !ctrl && !alt
                        && let Some(t) = ev.text.as_ref() {
                            let t: String = t.chars().filter(|c| !c.is_control()).collect();
                            if !t.is_empty() {
                                ob.insert(&t);
                            }
                        }
                }
            }
            // Find as you type.
            if let Some(ob) = self.omnibox.as_ref()
                && let Target::Find(id) = ob.target
                && ob.text != before
            {
                self.last_find = ob.text.clone();
                self.found = None;
                if let Some(c) = self.cards.get(&id) {
                    c.page.find(&ob.text, !self.find_back, false);
                }
            }
            // New text, new results: back to the best one, and the walks
            // through earlier lines and the list start again.
            if let Some(ob) = self.omnibox.as_mut()
                && ob.target.is_list()
                && ob.text != before
            {
                ob.sel = 0;
                ob.scroll = 0;
                ob.walk = None;
                ob.list_for = None;
            }
            return;
        }

        // The trails sidebar, when it has the keys, takes its own (j/k, h/l,
        // Enter, Esc, B); the rest are board keys as usual.
        if pressed && !ctrl && !alt && self.drag_is_idle() && self.sidebar_has_keys() && self.sidebar_key(code, shift) {
            return;
        }

        // Board keys (`scene::keys`), while no card is entered. A key that
        // isn't one goes on to the app's shortcuts below.
        if pressed
            && !self.entered()
            && let Some(chord) = ev.chord(self.mods)
        {
            match self.pending.feed(&self.keys.board, chord, Instant::now(), true) {
                Outcome::Run { action, count } => return self.board_action(action, count),
                Outcome::Wait | Outcome::Cancelled | Outcome::Unbound { swallow: true } => return,
                Outcome::Unbound { swallow: false } => {}
            }
        }
        // Page mode: our page keys first. A character that isn't one does
        // nothing; other keys (Enter, Tab, arrows, Space, shortcuts) go on to
        // the app's shortcuts and then to the page.
        let mut to_page = self.typing();
        // Ctrl+G mid-sequence in a page: show what can follow (with the
        // chip that shows by itself). Ctrl+G alone goes on to the page.
        if pressed
            && self.entered()
            && (self.page_mode() || self.visual_mode().is_some())
            && !self.pending.is_empty()
            && ev.chord(self.mods) == Some(keys::Chord::new(keys::Key::Char('g'), true, false, false))
        {
            self.prefix_asked = self.pending.since();
            return;
        }
        // Caret / Visual mode: its keys; nothing goes to the page, and a
        // key that isn't one does nothing unless it is an app shortcut.
        if pressed
            && self.visual_mode().is_some()
            && let Some(chord) = ev.chord(self.mods)
        {
            match self.pending.feed(&self.keys.visual, chord, Instant::now(), true) {
                Outcome::Run { action, count } => return self.visual_action(action, count),
                Outcome::Wait | Outcome::Cancelled | Outcome::Unbound { swallow: true } => return,
                Outcome::Unbound { swallow: false } => {}
            }
            if chord.is_bare() && matches!(chord.key, keys::Key::Char(_)) {
                return;
            }
        }
        if self.page_mode() {
            if pressed {
                let chord = ev.chord(self.mods);
                if let Some(chord) = chord {
                    match self.pending.feed(&self.keys.page, chord, Instant::now(), true) {
                        Outcome::Run { action, count } => return self.page_action(action, count),
                        Outcome::Wait | Outcome::Cancelled | Outcome::Unbound { swallow: true } => return,
                        Outcome::Unbound { swallow: false } => {}
                    }
                }
                if chord.is_some_and(|c| c.is_bare() && matches!(c.key, keys::Key::Char(_))) {
                    return;
                }
                to_page = true;
                self.forwarded.insert(ev.native);
            } else {
                to_page = self.forwarded.remove(&ev.native);
            }
        }
        // Insert mode: the user's insert keys come before the page.
        if pressed
            && self.entered()
            && self.page_keys == PageKeys::Insert
            && let Some(a) = self.user_key(KeyMode::Insert, &ev)
        {
            return self.run_key_action(a, None);
        }

        // Ctrl keys while typing in a page (`keys::typing_key`): Ctrl+W,
        // Ctrl+U and Ctrl+H edit the field in Insert, and Ctrl+R (and in
        // Passthrough every Ctrl+letter but a few) is the page's, instead of
        // closing or reloading the card mid-sentence.
        if pressed
            && self.typing()
            && let Some(chord) = ev.chord(self.mods)
        {
            match keys::typing_key(chord, self.page_keys == PageKeys::Passthrough) {
                keys::TypingKey::Edit(edit) => {
                    self.edit_page_field(edit);
                    return;
                }
                keys::TypingKey::ToPage => {
                    if let Some(p) = self.focused_page() {
                        p.send_key(&ev.to_web(self.mods));
                    }
                    return;
                }
                keys::TypingKey::App => {}
            }
        }

        if pressed {
            // App shortcuts first.
            let handled = match code {
                // Board and Page mode have F11 in their tables; this is Insert's.
                Some(KeyCode::F11) if !ctrl && !shift && !alt && self.page_keys != PageKeys::Passthrough => {
                    self.toggle_fullscreen();
                    true
                }
                // And F12 / Ctrl+Shift+i, the inspector: on the inspector
                // itself even in Passthrough, so the key that opened it closes it.
                Some(KeyCode::F12) if !ctrl && !shift && !alt && (self.page_keys != PageKeys::Passthrough || self.on_inspector()) => {
                    self.toggle_inspector(None);
                    true
                }
                Some(KeyCode::KeyI) if ctrl && shift && !alt && (self.page_keys != PageKeys::Passthrough || self.on_inspector()) => {
                    self.toggle_inspector(None);
                    true
                }
                Some(KeyCode::KeyQ) if ctrl && !shift && !alt => {
                    log::info!("quit (Ctrl+Q)");
                    self.quit = true;
                    true
                }
                // Spaces.
                Some(KeyCode::KeyN) if ctrl && shift && !alt => {
                    self.new_space(false);
                    true
                }
                Some(KeyCode::PageDown | KeyCode::PageUp) if alt && !ctrl && !shift && self.drag_is_idle() => {
                    self.step_trail(if code == Some(KeyCode::PageDown) { 1 } else { -1 });
                    true
                }
                Some(KeyCode::PageDown | KeyCode::PageUp) if ctrl && !shift && !alt => {
                    let step = if code == Some(KeyCode::PageDown) { 1 } else { -1 };
                    self.switch_space(self.spaces.neighbour(self.spaces.active, step));
                    true
                }
                Some(KeyCode::F2) if !ctrl && !shift && !alt => {
                    let name = self.spaces.active_entry().name.clone();
                    self.omnibox = Some(Omnibox::new(&name, Target::RenameSpace));
                    true
                }
                Some(KeyCode::KeyB) if ctrl && shift && !alt => {
                    self.cycle_tint();
                    true
                }
                Some(KeyCode::KeyP) if ctrl && shift && !alt => {
                    self.toggle_private();
                    true
                }
                Some(KeyCode::KeyW) if ctrl && shift && !alt => {
                    self.ask_delete_space();
                    true
                }
                Some(KeyCode::KeyK) if ctrl && !shift && !alt => {
                    self.open_palette();
                    true
                }
                Some(KeyCode::KeyH) if ctrl && !shift && !alt => {
                    self.open_history();
                    true
                }
                Some(KeyCode::Comma) if ctrl && !shift && !alt => {
                    self.open_settings();
                    true
                }
                Some(KeyCode::KeyJ) if ctrl && !shift && !alt => {
                    self.open_downloads();
                    true
                }
                Some(KeyCode::KeyL) if ctrl => {
                    self.open_address_bar();
                    true
                }
                Some(KeyCode::KeyC | KeyCode::KeyX) if ctrl && !alt && self.focused_page().is_some() => {
                    let cut = code == Some(KeyCode::KeyX);
                    self.page_copy(cut, Some(&ev));
                    true
                }
                Some(KeyCode::KeyV) if ctrl && !alt && self.focused_page().is_some() => {
                    self.page_paste(Some(&ev));
                    true
                }
                Some(KeyCode::KeyT) if ctrl && shift => {
                    self.reopen_closed();
                    true
                }
                Some(KeyCode::KeyT) if ctrl => {
                    self.new_card();
                    true
                }
                // Ctrl+W alone closes nothing (bug report 2026-11-21): the
                // board and page tables wait for its second key, and while
                // typing it deletes a word. Here only Caret / Visual mode
                // reaches it, where it does nothing.
                Some(KeyCode::KeyW) if ctrl && !alt => true,
                Some(KeyCode::KeyS) if ctrl && !shift && !alt => {
                    let note = self.focused.filter(|&id| self.scene.node(id).is_some_and(Node::is_note));
                    self.save_markdown(note);
                    true
                }
                Some(KeyCode::KeyR) if ctrl => {
                    if let Some(p) = self.focused_page().filter(|p| p.kind() == web::PageKind::Web) {
                        p.reload();
                    }
                    true
                }
                Some(KeyCode::KeyF) if ctrl && !alt && !shift => {
                    self.open_find(false);
                    true
                }
                Some(KeyCode::ContextMenu) if !ctrl && !alt => {
                    self.page_menu_by_key(&ev);
                    true
                }
                Some(KeyCode::F10) if shift && !ctrl && !alt => {
                    self.page_menu_by_key(&ev);
                    true
                }
                Some(
                    KeyCode::Equal
                    | KeyCode::NumpadAdd
                    | KeyCode::Minus
                    | KeyCode::NumpadSubtract
                    | KeyCode::Digit0
                    | KeyCode::Numpad0,
                ) if ctrl && !alt => {
                    let dir = match code {
                        Some(KeyCode::Equal | KeyCode::NumpadAdd) => 1,
                        Some(KeyCode::Minus | KeyCode::NumpadSubtract) => -1,
                        _ => 0,
                    };
                    self.zoom_page(dir);
                    true
                }
                Some(KeyCode::ArrowLeft) if alt => {
                    if let Some(p) = self.focused_page() {
                        p.back();
                    }
                    true
                }
                Some(KeyCode::ArrowRight) if alt => {
                    if let Some(p) = self.focused_page() {
                        p.forward();
                    }
                    true
                }
                Some(KeyCode::Tab) if !ctrl && !alt && !shift && !self.typing() => {
                    self.toggle_read();
                    true
                }
                // Passthrough: Shift+Esc gives our keys back; a plain Esc is the page's.
                Some(KeyCode::Escape) if self.typing() && self.page_keys == PageKeys::Passthrough && shift => {
                    self.set_page_keys(PageKeys::Page);
                    true
                }
                Some(KeyCode::Escape) if self.typing() && self.page_keys == PageKeys::Passthrough => false,
                // A page in full screen comes out of it (and Solo with it).
                Some(KeyCode::Escape) if self.focused_page().is_some_and(|p| p.fullscreen()) && self.drag_is_idle() => {
                    if let Some(p) = self.focused_page() {
                        p.exit_fullscreen();
                    }
                    true
                }
                // Insert: the page sees Esc (closes its menus), the field lets
                // go of the caret, and our page keys are back.
                Some(KeyCode::Escape) if self.typing() && self.drag_is_idle() => {
                    if let Some(p) = self.focused_page() {
                        p.send_key(&ev.to_web(self.mods));
                        p.blur_field();
                    }
                    self.set_page_keys(PageKeys::Page);
                    true
                }
                // Find marks left in the current card's page go first.
                Some(KeyCode::Escape) if self.find_marks.is_some() && self.find_marks == self.focused && self.drag_is_idle() => {
                    if let Some(p) = self.focused_page() {
                        p.clear_find();
                    }
                    self.find_marks = None;
                    log::info!("find marks cleared");
                    true
                }
                Some(KeyCode::Escape) if self.read.is_some() && self.drag_is_idle() => {
                    // The page sees Esc too (closes its menus); the card stays focused.
                    if let Some(p) = self.focused_page() {
                        p.send_key(&ev.to_web(self.mods));
                    }
                    self.leave_read();
                    true
                }
                Some(KeyCode::Escape) if self.focused.is_none() && !self.selection.is_empty() => {
                    self.selection.clear();
                    true
                }
                Some(KeyCode::Escape) if self.page_mode() && self.drag_is_idle() => {
                    // Let the page see Esc too (closes its menus), then give the
                    // keys back to the board; the card stays the current one.
                    if let Some(p) = self.focused_page() {
                        p.send_key(&ev.to_web(self.mods));
                    }
                    self.leave_page();
                    true
                }
                Some(KeyCode::Escape) if self.focused.is_some() && self.drag_is_idle() => {
                    self.selection.clear();
                    self.focus(None);
                    true
                }
                _ => false,
            };
            if handled {
                return;
            }
        }
        if to_page
            && let Some(p) = self.focused_page()
        {
            p.send_key(&ev.to_web(self.mods));
        }
    }

    pub(crate) fn drag_is_idle(&self) -> bool {
        matches!(self.drag, Drag::None)
    }

    /// The selection band being drawn, in world units.
    pub(crate) fn band(&self) -> Option<Rect> {
        match self.drag {
            Drag::Band { anchor } => Some(Rect::from_corners(anchor, self.to_world(self.cursor))),
            _ => None,
        }
    }
}
