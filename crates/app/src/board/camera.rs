//! Camera.

use std::collections::HashMap;
use std::time::Instant;

use scene::{Camera, motion, Node, NodeId, read, Rect, tidy, TrailStep, Vec2};

use crate::app::App;
use crate::ui::menu;
use crate::ui::omnibox::{Omnibox, Target};

/// A card of a trail being folded, on its way to its sheet behind the
/// stack's front. The model has it in the stack (hidden) already; only the
/// drawing moves.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Gather {
    pub(crate) id: NodeId,
    /// The stack's front card.
    pub(crate) front: NodeId,
    /// Which sheet behind the front it becomes (`motion::slot_behind`).
    pub(crate) slot: usize,
    /// Its frame and content rect where it was (world units).
    pub(crate) frame: Rect,
    pub(crate) content: Rect,
    pub(crate) start: Instant,
}

impl App {
    pub(crate) fn fit_all(&mut self) {
        self.leave_read();
        if let Some(b) = self.scene.bounds() {
            self.camera_goal = Some(Camera::fit(b, self.viewport(), self.scale(), scene::FIT_MARGIN * self.scale()));
        }
    }

    /// T: lay every trail out left to right with nothing overlapping
    /// (`scene::tidy`), as one undo step. The card in the middle of the view
    /// keeps its place on screen, so the board doesn't jump away.
    pub(crate) fn tidy_board(&mut self) {
        let anchor = read::pick(&self.scene, self.focused, self.camera.center)
            .and_then(|id| self.scene.node(id).map(|n| (id, n.rect)));
        let mut before = Vec::new();
        for (id, rect) in tidy::tidy(&self.scene) {
            if let Some(n) = self.scene.node_mut(id)
                && n.rect != rect
            {
                before.push((id, std::mem::replace(&mut n.rect, rect)));
            }
        }
        log::info!("tidy: moved {} card(s)", before.len());
        if before.is_empty() {
            self.toast("Already tidy");
            return;
        }
        self.history.moved_many(&self.scene, &before);
        self.dirty = true;
        if !self.still {
            // Cards glide to their places rather than jumping.
            let now = Instant::now();
            for &(id, old) in &before {
                self.glides.start(id, old, false, now);
            }
        }
        if let Some((id, old)) = anchor
            && let Some(new) = self.scene.node(id).map(|n| n.rect)
        {
            let mut goal = self.camera_goal.unwrap_or(self.camera);
            goal.center = goal.center + Vec2::new(new.x - old.x, new.y - old.y);
            self.camera_goal = Some(goal);
        }
        self.toast(format!("Tidied {} card{}", before.len(), if before.len() == 1 { "" } else { "s" }));
    }

    /// S: fold the trail of the card in the middle of the view into a stack
    /// with that card in front, or unfold it if it is a stack. One undo step.
    pub(crate) fn toggle_fold(&mut self) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        if self.scene.stack_of(id).is_some() {
            return self.unfold_stack(id);
        }
        // Where the trail's cards on screen are now, to slide them into the stack.
        let places: HashMap<NodeId, (Rect, Rect)> = if self.still {
            HashMap::new()
        } else {
            self.scene.trail_of(id).into_iter().filter(|&m| m != id && self.card_on_screen(m)).filter_map(|m| Some((m, self.scene.node(m).map(|n| (n.frame(), n.rect))?))).collect()
        };
        if let Some(f) = self.scene.fold(id) {
            log::info!("fold: {:?} in front of {}", f.members.iter().map(|m| m.0).collect::<Vec<_>>(), id.0);
            // Each card becomes a sheet of the stack: the next one the
            // sheet just behind the front, and so on (as [ ] go through them).
            let now = Instant::now();
            for (j, m) in self.stack_behind(id).into_iter().enumerate() {
                let Some(&(frame, content)) = places.get(&m) else { continue };
                self.glides.forget(m);
                self.gathers.retain(|g| g.id != m);
                self.gathers.push(Gather { id: m, front: id, slot: motion::slot_behind(j), frame, content, start: now });
            }
            self.history.folded(&f);
            self.after_fold();
            self.toast(format!("Folded {} pages · S unfolds, [ ] peek", f.members.len()));
        } else {
            self.toast("No trail to fold");
            return;
        }
        self.dirty = true;
    }

    pub(crate) fn unfold_stack(&mut self, id: NodeId) {
        if let Some(stack) = self.scene.unfold(id) {
            log::info!("unfold: {} card(s) around {}", stack.members.len(), id.0);
            // Each sheet behind the front slides out to its card's place,
            // the card's page showing on it on the way.
            if !self.still
                && let Some(front) = self.scene.node(stack.front).map(|n| n.frame())
            {
                let now = Instant::now();
                let step = self.stack_step();
                let len = stack.members.len();
                let behind = (1..len).map(|j| stack.members[(stack.front_index() + j) % len].id);
                for (j, m) in behind.enumerate() {
                    self.gathers.retain(|g| g.id != m);
                    if let Some(n) = self.scene.node(m) {
                        let sheet = motion::stack_slot(front, motion::slot_behind(j), step);
                        let hh = n.header_h();
                        self.glides.unfold(m, Rect::new(sheet.x, sheet.y + hh, sheet.w, (sheet.h - hh).max(1.0)), now);
                    }
                }
            }
            self.history.unfolded(&stack);
            self.toast(format!("Unfolded {} pages", stack.members.len()));
            self.dirty = true;
        }
    }

    /// O: the outline of the stack in the middle of the view.
    pub(crate) fn outline_in_view(&mut self) {
        match read::pick(&self.scene, self.focused, self.camera.center) {
            Some(id) if self.scene.stack_of(id).is_some() => self.open_outline(id),
            _ => self.toast("No stack here · S folds a trail"),
        }
    }

    /// [ / ]: show the previous / next card of the stack in the middle of the view.
    pub(crate) fn peek_stack(&mut self, step: isize) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        let Some(front) = self.scene.peek(id, step) else { return };
        let stack = self.scene.stack_of(front).expect("peeked a stack");
        log::info!("peek: card {} ({} of {})", front.0, stack.front_index() + 1, stack.members.len());
        self.after_fold();
        self.dirty = true;
    }

    /// Alt+PgDn / Alt+PgUp: focus the next / previous card along the trail of
    /// the focused card (or the one in the middle of the view), showing it at
    /// its stack's front if it is folded away, and bring it on screen (in Read
    /// Mode, read it). Stops at the ends of the trail.
    pub(crate) fn step_trail(&mut self, step: isize) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        match self.scene.step_trail(id, step) {
            TrailStep::Moved { to, index, len } => {
                log::info!("trail: card {} ({} of {})", to.0, index + 1, len);
                self.selection.clear();
                self.after_fold();
                self.focus(Some(to));
                self.ensure_visible(to);
                let title = self.scene.node(to).map(|n| n.title.clone()).filter(|t| !t.is_empty());
                let title = title.unwrap_or_else(|| self.scene.node(to).map(|n| n.url.clone()).unwrap_or_default());
                let step = format!("{}/{} · {}", index + 1, len, menu::cut(&title, menu::OUTLINE_CHARS));
                self.toast(match self.scene.trail_mark(to).name {
                    Some(name) => format!("{name} · {step}"),
                    None => step,
                });
                self.dirty = true;
            }
            TrailStep::End { first, .. } => {
                // Focus the card we stepped from, so the next step starts there.
                self.focus(Some(id));
                self.toast(if first { "Start of the trail" } else { "End of the trail" });
            }
            TrailStep::Lone => self.toast("Not on a trail"),
        }
    }

    /// R: the address bar, as a name box for the trail in the middle of the view.
    pub(crate) fn ask_trail_name(&mut self) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        self.ask_trail_name_of(id);
    }

    /// The name box for the trail through card `id` (from R, the sidebar or
    /// a click on the trail's tag), holding its current name.
    pub(crate) fn ask_trail_name_of(&mut self, id: NodeId) {
        self.menu = None;
        self.link_hints = None;
        let name = self.scene.trail_mark(id).name.unwrap_or_default();
        self.omnibox = Some(Omnibox::new(&name, Target::NameTrail(id)));
        log::info!("name box for the trail of card {}: {name:?}", id.0);
    }

    pub(crate) fn name_trail(&mut self, id: NodeId, name: &str) {
        let old = self.scene.name_trail(id, name);
        if old.is_empty() {
            return;
        }
        self.history.marked(old);
        self.dirty = true;
        let mark = self.scene.trail_mark(id);
        log::info!("trail of card {} named {:?}", id.0, mark.name);
        self.toast(match mark.name {
            Some(n) => format!("Trail named {n}"),
            None => "Trail name cleared".to_string(),
        });
    }

    /// C: the trail in the middle of the view takes its next colour.
    /// C: the next colour of the trail of the current card (or the one in the
    /// middle of the view), or of that card itself if it is a sticky note.
    pub(crate) fn cycle_trail_colour(&mut self) {
        let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) else { return };
        if self.scene.node(id).is_some_and(Node::is_sticky) {
            self.cycle_sticky_colour(id);
            return;
        }
        let old = self.scene.cycle_trail_colour(id);
        self.history.marked(old);
        self.dirty = true;
        let colour = self.scene.trail_mark(id).colour;
        log::info!("trail of card {} colour {:?}", id.0, colour);
        self.toast(match colour {
            Some(c) => format!("Trail colour: {c}"),
            None => "Trail colour: accent".to_string(),
        });
    }
}
