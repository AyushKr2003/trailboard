//! Inspector.

use std::time::Instant;

use scene::Vec2;
use scene::dock;
use scene::{motion, NodeId, Rect};
use web::Page;

use crate::app::{App, Card, DOCK_GAP, dock_key, Drag, FULL_FPS, PageKeys};

/// The page inspector docked in a card, beside or below its page (`App::docks`).
pub(crate) struct Dock {
    pub(crate) page: Page,
    pub(crate) side: dock::Side,
    /// The inspector's share of the card (`scene::dock::split`).
    pub(crate) share: f32,
    pub(crate) hidden: bool,
    /// Cursor the inspector asked for, shown while the pointer is over it.
    pub(crate) cursor: web::Cursor,
}

/// A docked card's parts on screen: the page and the inspector (screen
/// rect, CSS size each) and the divider between them.
pub(crate) struct DockParts {
    pub(crate) page: (Rect, Vec2),
    pub(crate) divider: Rect,
    pub(crate) inspector: (Rect, Vec2),
}

impl App {
    /// F12 / Ctrl+Shift+i / `:inspect`: the current card's inspector beside
    /// it, or closed if it has one (or is one).
    pub(crate) fn toggle_inspector(&mut self, at: Option<(f32, f32)>) {
        let Some(id) = self.current() else {
            self.toast("No card to inspect");
            return;
        };
        if self.inspectors.contains_key(&id) {
            self.close_inspector(id);
        } else if self.docks.contains_key(&id) {
            self.close_dock(id);
        } else if let Some(insp) = self.inspector_of(id) {
            self.close_inspector(insp);
        } else {
            self.open_dock(id, self.dock_side, at);
        }
    }

    /// `:inspect [right|bottom|beside]`: no argument toggles it; `right` /
    /// `bottom` dock it there (moving it from its own card, or to the other
    /// side); `beside` gives it a card of its own.
    pub(crate) fn inspect_command(&mut self, arg: &str) {
        let Some(id) = self.current() else {
            self.toast("No card to inspect");
            return;
        };
        // The card inspected, whether `id` is it or its inspector card.
        let src = self.inspectors.get(&id).copied().unwrap_or(id);
        if let Some(side) = dock::Side::parse(arg) {
            if let Some(insp) = self.inspector_of(src) {
                self.close_inspector(insp);
            }
            self.dock_to(src, side);
        } else if arg == "beside" {
            if self.inspector_of(src).is_none() {
                self.undock(src);
            }
        } else {
            self.toggle_inspector(None);
        }
    }

    /// Card `id`'s inspector docked on `side`: moved there, or opened.
    pub(crate) fn dock_to(&mut self, id: NodeId, side: dock::Side) {
        self.dock_side = side;
        match self.docks.get_mut(&id) {
            Some(d) => {
                d.side = side;
                log::info!("inspector of card {} docked {side:?}", id.0);
            }
            None => self.open_dock(id, side, None),
        }
    }

    /// Card `id`'s docked inspector moved to a card of its own beside it.
    pub(crate) fn undock(&mut self, id: NodeId) {
        self.close_dock(id);
        self.open_inspector(id, None);
    }

    /// The page menu's *Inspect*: the inspector of card `id`, on the element
    /// at `at` (CSS px in the page), opened if it isn't.
    pub(crate) fn inspect_at(&mut self, id: NodeId, at: (f32, f32)) {
        if let (Some(c), Some(d)) = (self.cards.get(&id), self.docks.get(&id)) {
            c.page.inspect_at(&d.page, at);
            self.focus(Some(id));
            self.set_dock_keys(true);
            self.give_keys(PageKeys::Passthrough);
            self.ensure_visible(id);
            return;
        }
        match self.inspector_of(id) {
            Some(insp) => {
                if let (Some(c), Some(i)) = (self.cards.get(&id), self.cards.get(&insp)) {
                    c.page.inspect_at(&i.page, at);
                }
                self.focus(Some(insp));
                self.ensure_visible(insp);
            }
            None => self.open_inspector(id, Some(at)),
        }
    }

    /// The focused card is an inspector.
    pub(crate) fn on_inspector(&self) -> bool {
        self.focused.is_some_and(|id| self.inspectors.contains_key(&id) || (self.dock_keys && self.docks.contains_key(&id)))
    }

    /// The inspector card of card `src`, if it has one open.
    pub(crate) fn inspector_of(&self, src: NodeId) -> Option<NodeId> {
        self.inspectors.iter().find(|&(_, &s)| s == src).map(|(&i, _)| i)
    }

    pub(crate) fn open_inspector(&mut self, src: NodeId, at: Option<(f32, f32)>) {
        let Some(n) = self.scene.node(src) else { return };
        if n.is_inspector() {
            return;
        }
        self.close_dock(src);
        self.create_page(src);
        if !self.cards.contains_key(&src) {
            self.toast("This card's page isn't running");
            return;
        }
        let Some(id) = self.scene.add_inspector(src, self.card_size) else { return };
        let css = self.card_css(self.scene.node(id).expect("just added"));
        let key = self.page_key(id);
        let made = self.cards[&src].page.open_inspector(key, (css.x as i32, css.y as i32), self.page_scale, at, &self.spawn_beside);
        let Some(page) = made else {
            self.scene.remove(id);
            self.toast("Couldn't open the inspector");
            return;
        };
        log::info!("inspector {} for card {}", id.0, src.0);
        self.cards.insert(id, Card { page, loading: true, can_back: false, can_forward: false, hidden: false, cursor: web::Cursor::Default, fps: FULL_FPS, held: false });
        self.inspectors.insert(id, src);
        self.touch(id);
        self.dirty = true;
        if !self.still
            && let (Some(from), Some(to)) = (self.scene.node(src).map(|n| n.rect), self.scene.node(id).map(|n| n.rect))
        {
            self.glides.start(id, motion::open_from(from, to), true, Instant::now());
        }
        self.focus(Some(id));
        self.ensure_visible(id);
        self.give_keys(PageKeys::Page);
    }

    /// Card `id`'s inspector docked in it on `side`; `at`: an element to
    /// open on (CSS px in the page).
    pub(crate) fn open_dock(&mut self, id: NodeId, side: dock::Side, at: Option<(f32, f32)>) {
        let Some(n) = self.scene.node(id) else { return };
        if n.is_inspector() || self.docks.contains_key(&id) {
            return;
        }
        self.create_page(id);
        if !self.cards.contains_key(&id) {
            self.toast("This card's page isn't running");
            return;
        }
        let css = self.card_css(self.scene.node(id).expect("checked"));
        let s = dock::split(Rect::new(0.0, 0.0, css.x, css.y), side, dock::DEFAULT_SHARE, DOCK_GAP);
        let size = (s.inspector.w as i32, s.inspector.h as i32);
        let key = dock_key(self.epoch, id);
        let Some(page) = self.cards[&id].page.open_inspector(key, size, self.page_scale, at, &self.spawn_beside) else {
            self.toast("Couldn't open the inspector");
            return;
        };
        log::info!("inspector docked {side:?} in card {}", id.0);
        self.dock_side = side;
        self.docks.insert(id, Dock { page, side, share: dock::DEFAULT_SHARE, hidden: false, cursor: web::Cursor::Default });
        self.focus(Some(id));
        self.set_dock_keys(true);
        self.give_keys(PageKeys::Passthrough);
        self.ensure_visible(id);
    }

    /// Takes card `id`'s docked inspector away; the page has the card again.
    pub(crate) fn close_dock(&mut self, id: NodeId) {
        if self.docks.remove(&id).is_none() {
            return;
        }
        log::info!("inspector in card {} closed", id.0);
        self.dock_css.remove(&id);
        let key = dock_key(self.epoch, id);
        if let Some(r) = self.renderer.as_mut() {
            r.remove_page(key);
        }
        if self.focused == Some(id) && self.dock_keys {
            self.dock_keys = false;
            if let Some(c) = self.cards.get(&id) {
                c.page.set_focus(true);
            }
            if self.page_keys == PageKeys::Passthrough {
                self.set_page_keys(self.site_keys(Some(id)));
            }
        }
        if matches!(self.drag, Drag::Dock { id: d, .. } | Drag::Divider { id: d } if d == id) {
            self.drag = Drag::None;
        }
    }

    /// The keys go to the focused card's docked inspector (`on`) or its page.
    pub(crate) fn set_dock_keys(&mut self, on: bool) {
        let Some(id) = self.focused else { return };
        let on = on && self.docks.contains_key(&id);
        if self.dock_keys == on {
            return;
        }
        self.dock_keys = on;
        if let Some(c) = self.cards.get(&id) {
            c.page.set_focus(!on);
        }
        if let Some(d) = self.docks.get(&id) {
            d.page.set_focus(on);
        }
    }

    /// Takes inspector card `id` off the board and stops its page (which
    /// lets go of the page it inspected).
    pub(crate) fn close_inspector(&mut self, id: NodeId) {
        self.inspectors.remove(&id);
        if self.scene.remove(id).is_some() {
            log::info!("inspector {} closed", id.0);
        }
        self.drop_page(id);
    }

    /// Every frame: an inspector goes when its page does (closed, asleep, or
    /// a new browser in its place), and a card left over without one.
    pub(crate) fn check_inspectors(&mut self) {
        let mut gone: Vec<NodeId> = self
            .inspectors
            .iter()
            .filter(|&(i, s)| match (self.cards.get(i), self.cards.get(s)) {
                (Some(i), Some(s)) => !i.page.inspects(&s.page),
                _ => true,
            })
            .map(|(&i, _)| i)
            .collect();
        gone.extend(self.scene.all_nodes().filter(|n| n.is_inspector() && !self.inspectors.contains_key(&n.id)).map(|n| n.id));
        for id in gone {
            self.close_inspector(id);
        }
        let stale: Vec<NodeId> = self
            .docks
            .iter()
            .filter(|&(id, d)| !self.cards.get(id).is_some_and(|c| d.page.inspects(&c.page)))
            .map(|(&id, _)| id)
            .collect();
        for id in stale {
            self.close_dock(id);
        }
    }
}
