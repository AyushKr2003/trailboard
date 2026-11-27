//! Custom trails (M15).

use super::*;

impl App {
    /// The cards a join or leave acts on: the selection when the current
    /// card is in it (or there is none), else the current card.
    pub(super) fn relink_cards(&self) -> Vec<NodeId> {
        match read::pick(&self.scene, self.focused, self.camera.center) {
            Some(id) if !self.selection.contains(id) => vec![id],
            _ if !self.selection.is_empty() => self.selection.ids(),
            Some(id) => vec![id],
            None => Vec::new(),
        }
    }

    /// A card's title for a message, cut short.
    pub(super) fn card_title(&self, id: NodeId) -> String {
        let n = self.scene.node(id);
        let title = n.map(|n| n.title.trim()).filter(|t| !t.is_empty()).or(n.map(|n| n.url.as_str())).unwrap_or_default();
        menu::cut(title, 40)
    }

    /// Joins `cards` to the trail of `target` (`scene::join`), as one undo
    /// step: they glide to their new places and a toast names the trail.
    /// `keep`: a coloured joining trail keeps its colour (else it takes the
    /// target trail's: a merge).
    pub(super) fn join_cards(&mut self, cards: &[NodeId], target: NodeId, keep: bool) {
        use scene::RelinkError as E;
        if cards.is_empty() {
            return self.toast("No card here to join");
        }
        if cards.iter().all(|&c| c == target) {
            return self.toast("A card can't join itself · pick another card");
        }
        match self.scene.join_many(cards, target, keep) {
            Ok(r) => {
                log::info!("join: {:?} to card {} (moved {})", r.cards.iter().map(|c| c.0).collect::<Vec<_>>(), target.0, r.moved.len());
                self.history.relinked(&r);
                self.after_relink(&r);
                let what = match r.cards.len() {
                    1 => "Joined".to_string(),
                    n => format!("{n} cards joined"),
                };
                let trail = match self.scene.trail_mark(target).name {
                    Some(name) => name,
                    None => format!("the trail of {}", self.card_title(target)),
                };
                let colours = if keep && self.scene.colour_part(target).len() < self.scene.trail_of(target).len() { ", keeping its colour" } else { "" };
                self.toast(format!("{what} {trail}{colours}"));
            }
            Err(E::Loop) => self.toast("Can't join a card to its own trail below it"),
            Err(E::Unchanged) => self.toast("Already joined there"),
            Err(E::Missing) => self.toast("No card to join"),
        }
    }

    /// Shift+A, `:leave`: cuts the current card (or the selection) out of
    /// its trail, with the cards opened from it. One undo step.
    /// Shift+A, `:leave`: cuts one of the current card's trail lines
    /// (owner, 2026-11-25). With one line, that one; with more, letters on
    /// the cards at their other ends pick which. A selection of several
    /// cards leaves as before ([`App::leave_cards`]).
    pub(super) fn leave_trail(&mut self) {
        let cards = self.relink_cards();
        if cards.len() > 1 {
            return self.leave_cards(&cards);
        }
        let Some(&card) = cards.first() else { return self.toast("No card here") };
        let mut ends = self.scene.neighbours(card);
        for e in self.scene.link_ends(card) {
            if !ends.contains(&e) {
                ends.push(e);
            }
        }
        match ends.as_slice() {
            [] => self.toast("Not on a trail"),
            [one] => self.cut_line(card, *one),
            _ => {
                self.menu = None;
                self.link_hints = None;
                let unseen = ends.iter().filter(|&&e| !self.card_on_screen(self.shown_card(e))).count();
                log::info!("cut: picking among {:?} for card {}", ends.iter().map(|e| e.0).collect::<Vec<_>>(), card.0);
                self.card_pick = Some(CardPick { keep: false, cut: true, link: false, hints: hint::Hints::new(ends.len()), cards: vec![card], targets: ends });
                self.toast(if unseen > 0 {
                    format!("Cut which line? Type the letters on the card to cut it from ({unseen} out of view: zoom out) · Esc cancels")
                } else {
                    "Cut which line? Type the letters on the card to cut it from · Esc cancels".to_string()
                });
                self.dirty = true;
            }
        }
    }

    /// Cuts the line between `card` and `other`; the side away from the
    /// trail's start leaves with the cards after it (`Scene::cut`).
    pub(super) fn cut_line(&mut self, card: NodeId, other: NodeId) {
        let trail_line = self.scene.edges().iter().any(|e| (e.from == card && e.to == other) || (e.from == other && e.to == card));
        if !trail_line && self.scene.is_linked(card, other) {
            return self.unlink_cards(card, other);
        }
        match self.scene.cut(card, other) {
            Ok(r) => {
                log::info!("cut: line {} – {} ({:?} left)", card.0, other.0, r.cards.iter().map(|c| c.0).collect::<Vec<_>>());
                self.history.relinked(&r);
                self.after_relink(&r);
                self.toast(format!("Cut {} from {}", self.card_title(card), self.card_title(other)));
            }
            Err(_) => self.toast("No line there to cut"),
        }
    }

    /// A card that stands for `id` on the board: itself, or its stack's
    /// front if it is folded away.
    pub(super) fn shown_card(&self, id: NodeId) -> NodeId {
        self.scene.stack_of(id).filter(|s| s.front != id).map_or(id, |s| s.front)
    }

    /// Several selected cards leave their trails at once, each cut from the
    /// card it was opened from.
    pub(super) fn leave_cards(&mut self, cards: &[NodeId]) {
        let names: Vec<Option<String>> = cards.iter().map(|&c| self.scene.trail_mark(c).name).collect();
        match self.scene.leave_many(cards) {
            Ok(r) => {
                log::info!("leave: {:?}", r.cards.iter().map(|c| c.0).collect::<Vec<_>>());
                self.history.relinked(&r);
                self.after_relink(&r);
                let name = cards.iter().zip(&names).find(|(c, _)| r.cards.contains(c)).and_then(|(_, n)| n.clone());
                self.toast(match (r.cards.len(), name) {
                    (1, Some(n)) => format!("Left {n}"),
                    (1, None) => "Left the trail".to_string(),
                    (k, Some(n)) => format!("{k} cards left {n}"),
                    (k, None) => format!("{k} cards left their trails"),
                });
            }
            Err(_) if cards.iter().all(|&c| self.scene.trail_of(c).len() < 2) => self.toast("Not on a trail"),
            Err(_) => self.toast("The first cards of their trails have no line in to cut · Shift+A on one card picks a line"),
        }
    }

    /// After a join or leave: cards that moved glide there, new trail lines
    /// draw in from their source and cut ones fade out.
    pub(super) fn after_relink(&mut self, r: &scene::Relinked) {
        if !self.still {
            let now = Instant::now();
            for &(id, old) in &r.moved {
                self.glides.start(id, old, false, now);
            }
            let made = self.scene.edges().iter().filter(|e| !r.edges.contains(e)).map(|e| (*e, now));
            let cut = r.edges.iter().filter(|e| !self.scene.edges().contains(e)).map(|e| (*e, now));
            self.edges_in.extend(made.collect::<Vec<_>>());
            self.edges_out.extend(cut.collect::<Vec<_>>());
        }
        self.after_fold();
        self.dirty = true;
    }

    /// gl, Ctrl+K *Link to a card…*, `:link`: letters on the cards in view
    /// of other trails; typing one links the current card (or the
    /// selection) to it, both trails staying as they are (`scene::link`).
    pub(super) fn start_link_pick(&mut self) {
        let cards = self.relink_cards();
        if cards.is_empty() {
            return self.toast("No card here to link");
        }
        self.menu = None;
        self.link_hints = None;
        let own: HashSet<NodeId> = cards.iter().flat_map(|&c| self.scene.trail_of(c)).collect();
        let mut targets: Vec<NodeId> = self
            .scene
            .nodes()
            .map(|n| n.id)
            .filter(|id| !own.contains(id) && self.card_on_screen(*id) && !cards.iter().all(|&c| self.scene.is_linked(c, *id)))
            .collect();
        targets.sort_by(|a, b| {
            let (a, b) = (self.scene.node(*a).expect("on board").rect, self.scene.node(*b).expect("on board").rect);
            a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x))
        });
        if targets.is_empty() {
            return self.toast("No card of another trail in view · zoom out, or :link a card by name");
        }
        log::info!("link: picking among {} card(s) for {:?}", targets.len(), cards.iter().map(|c| c.0).collect::<Vec<_>>());
        self.card_pick = Some(CardPick { keep: false, cut: false, link: true, hints: hint::Hints::new(targets.len()), cards, targets });
        self.toast("Link to which card? Type its letters · Esc cancels");
        self.dirty = true;
    }

    /// Links `cards` to `target`, as one undo step; the new lines draw in.
    pub(super) fn link_cards(&mut self, cards: &[NodeId], target: NodeId) {
        use scene::LinkError as E;
        match self.scene.link_many(cards, target) {
            Ok(old) => {
                let made: Vec<Edge> = self.scene.links().iter().filter(|e| !old.contains(e)).copied().collect();
                log::info!("link: {:?}", made.iter().map(|e| (e.from.0, e.to.0)).collect::<Vec<_>>());
                self.history.linked(old);
                if !self.still {
                    let now = Instant::now();
                    self.edges_in.extend(made.iter().map(|e| (*e, now)));
                }
                self.toast(match made.len() {
                    1 => format!("Linked {} to {}", self.card_title(made[0].to), self.card_title(target)),
                    n => format!("Linked {n} cards to {}", self.card_title(target)),
                });
            }
            Err(E::SameTrail) => self.toast("Already on that trail · its line joins them"),
            Err(E::Unchanged) => self.toast("Already linked"),
            Err(E::Missing) => self.toast("No card to link"),
        }
        self.dirty = true;
    }

    /// Removes the link between `a` and `b`; its line fades out.
    pub(super) fn unlink_cards(&mut self, a: NodeId, b: NodeId) {
        let gone: Vec<Edge> = self.scene.links().iter().filter(|e| (e.from == a && e.to == b) || (e.from == b && e.to == a)).copied().collect();
        match self.scene.unlink(a, b) {
            Ok(old) => {
                log::info!("unlink: {} – {}", a.0, b.0);
                self.history.linked(old);
                if !self.still {
                    let now = Instant::now();
                    self.edges_out.extend(gone.into_iter().map(|e| (e, now)));
                }
                self.toast(format!("Unlinked {} from {}", self.card_title(a), self.card_title(b)));
            }
            Err(_) => self.toast("No link there"),
        }
        self.dirty = true;
    }

    /// A, Ctrl+K *Join a trail…*, `:join`: letters on the other cards in
    /// view; typing one joins the current card (or the selection) to that
    /// card's trail.
    pub(super) fn start_card_pick(&mut self, keep: bool) {
        let cards = self.relink_cards();
        if cards.is_empty() {
            return self.toast("No card here to join");
        }
        self.menu = None;
        self.link_hints = None;
        let below: HashSet<NodeId> = cards.iter().flat_map(|&c| self.scene.followers(c)).collect();
        let mut targets: Vec<NodeId> = self.scene.nodes().map(|n| n.id).filter(|id| !below.contains(id) && self.card_on_screen(*id)).collect();
        targets.sort_by(|a, b| {
            let (a, b) = (self.scene.node(*a).expect("on board").rect, self.scene.node(*b).expect("on board").rect);
            a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x))
        });
        if targets.is_empty() {
            return self.toast("No other card in view · zoom out, or :join a card by name");
        }
        log::info!("join: picking among {} card(s) for {:?}", targets.len(), cards.iter().map(|c| c.0).collect::<Vec<_>>());
        self.card_pick = Some(CardPick { keep, cut: false, link: false, hints: hint::Hints::new(targets.len()), cards, targets });
        self.toast(if keep {
            "Join which trail, keeping this colour? Type a card's letters · Esc cancels"
        } else {
            "Join which trail? Type a card's letters (Shift keeps this colour) · Esc cancels"
        });
        self.dirty = true;
    }

    /// A key while card letters are up: letters pick, Backspace takes one
    /// back, anything else cancels.
    pub(super) fn card_pick_key(&mut self, ev: &KeyIn, chord: bool) {
        let Some(cp) = self.card_pick.as_mut() else { return };
        let letter = ev.text.as_deref().and_then(|t| {
            let mut cs = t.chars();
            cs.next().filter(|c| cs.next().is_none() && c.is_alphabetic())
        });
        match (ev.named, letter) {
            (Some(NamedKey::Backspace), _) if !chord => {
                cp.hints.backspace();
            }
            // Shift on a letter (it comes upper case) keeps the colour.
            (_, Some(c)) if !chord => match cp.hints.type_char(c.to_ascii_lowercase()) {
                hint::Typed::Follow(i) => {
                    let cp = self.card_pick.take().expect("picking");
                    if cp.cut {
                        self.cut_line(cp.cards[0], cp.targets[i]);
                    } else if cp.link {
                        self.link_cards(&cp.cards, cp.targets[i]);
                    } else {
                        self.join_cards(&cp.cards, cp.targets[i], cp.keep || c.is_uppercase());
                    }
                }
                hint::Typed::Narrowed => {}
                hint::Typed::NoMatch => log::debug!("no card label starts with {c:?}"),
            },
            _ => {
                let what = self.card_pick.take().map_or("Join", |cp| if cp.cut { "Cut" } else if cp.link { "Link" } else { "Join" });
                log::info!("card letters cancelled");
                self.toast(format!("{what} cancelled"));
            }
        }
        self.dirty = true;
    }

    /// The card letters: a label in the middle of each target card, and the
    /// joining cards outlined.
    pub(super) fn card_pick_layer(&mut self) -> Option<Layer> {
        let cp = self.card_pick.as_ref()?;
        let th = &self.theme;
        let ui = self.ui();
        let scale = self.scale();
        let size = crate::ui::chrome::type_::TITLE * ui;
        let (pad, bh) = (size * 0.45, size * 1.6);
        let typed_n = cp.hints.typed.chars().count();
        let radius = th.small_radius(scale);
        let mut l = Layer::default();
        for &c in &cp.cards {
            if let Some(n) = self.scene.node(c).filter(|_| !self.scene.is_hidden(c)) {
                crate::ui::chrome::outline(&mut l, self.to_screen_rect(n.frame()), radius, (2.0 * ui).max(1.5), rgb(th.marked(), 0.9));
            }
        }
        let mut items = Vec::new();
        for (i, &t) in cp.targets.iter().enumerate() {
            if !cp.hints.shown(i) {
                continue;
            }
            let Some(n) = self.scene.node(self.shown_card(t)) else { continue };
            let label = cp.hints.labels[i].to_uppercase();
            let (head, tail): (String, String) = (label.chars().take(typed_n).collect(), label.chars().skip(typed_n).collect());
            items.push((self.to_screen_rect(n.frame()), head, tail));
        }
        let (bg, fg, done) = (rgb(th.accent(), 1.0), rgb(th.accent_foreground(), 1.0), rgb(th.accent_foreground(), 0.45));
        let border = rgb(th.background(), 0.9);
        let screen = Rect::new(0.0, 0.0, self.viewport().x, self.viewport().y);
        let r = self.renderer.as_mut()?;
        for (frame, head, tail) in items {
            crate::ui::chrome::outline(&mut l, frame, radius, (1.5 * ui).max(1.0), rgb(th.accent(), 0.55));
            let head_w = r.measure(&head, size, true);
            let w = head_w + r.measure(&tail, size, true) + pad * 2.0;
            let c = frame.center();
            let b = Rect::new(c.x - w / 2.0, c.y - bh / 2.0, w, bh);
            l.quads.push(Quad { shape: Shape::Rect { rect: b.inflate(scale.max(1.0)), radius }, color: border, texture: None });
            l.quads.push(Quad { shape: Shape::Rect { rect: b, radius }, color: bg, texture: None });
            let ty = b.y + (bh - size * 1.3) / 2.0;
            if !head.is_empty() {
                l.texts.push(Text { text: head, pos: [b.x + pad, ty], size, color: done, clip: screen, bold: true });
            }
            l.texts.push(Text { text: tail, pos: [b.x + pad + head_w, ty], size, color: fg, clip: screen, bold: true });
        }
        Some(l)
    }

    /// The card under screen point `p` that an Alt+drag would join: any card
    /// but the ones being dragged.
    pub(super) fn join_drop_target(&self, p: Vec2) -> Option<NodeId> {
        self.hit(p).id().filter(|id| !self.group.iter().any(|(g, _)| g == id) && self.scene.node(*id).is_some())
    }

    /// Alt+drag of a label: the dragged cards outlined and a trail line
    /// drawn as the board draws trails (rounded right-angle turns, a dot at
    /// each end). Over a card, that card is outlined and a faint card shows
    /// where the grabbed one will land beside it, the line running into it,
    /// all in that trail's colour; elsewhere the line runs from the grabbed
    /// card to the pointer.
    pub(super) fn join_drag_layer(&self) -> Option<Layer> {
        let Drag::Join { id, keep, link } = self.drag else { return None };
        let th = &self.theme;
        let scale = self.scale();
        let k = self.camera.pixels_per_unit(scale);
        let radius = th.small_radius(scale);
        let mut l = Layer::default();
        let outline = ((2.0 * self.ui()).max(1.5), rgb(th.marked(), 0.9));
        for &(g, _) in &self.group {
            if let Some(n) = self.scene.node(g) {
                crate::ui::chrome::outline(&mut l, self.to_screen_rect(n.frame()), radius, outline.0, outline.1);
            }
        }
        let card = self.to_screen_rect(self.scene.node(id)?.frame());
        // Where a trail meets a card: its side, near the top, where its name is.
        let row = |f: Rect| f.y + (28.0 * k).min(f.h / 2.0);
        let target = self.join_drop_target(self.cursor).and_then(|t| self.scene.node(t));
        let (from, to, colour) = match target {
            Some(t) => {
                let tf = self.to_screen_rect(t.frame());
                let colour = self.scene.trail_mark(t.id).colour.map_or(th.accent(), |c| th.color(&c));
                crate::ui::chrome::outline(&mut l, tf, radius, outline.0 * 1.5, rgb(colour, 0.95));
                if link {
                    // A link leaves the card where it is: a dashed line to it.
                    let (a, b) = if card.x >= tf.right() {
                        (Vec2::new(tf.right(), row(tf)), Vec2::new(card.x, row(card)))
                    } else {
                        (Vec2::new(tf.x, row(tf)), Vec2::new(card.right(), row(card)))
                    };
                    return Some(self.line_layer(l, a, b, colour, true));
                }
                // Where the card will land (`scene::join` puts it there): a
                // faint card in the trail's colour, the line running into it
                // as the board draws an edge.
                let skip: std::collections::BTreeSet<NodeId> = self.group.iter().flat_map(|&(g, _)| self.scene.followers(g)).collect();
                let size = self.scene.node(id)?.rect;
                let land = scene::placement::beside_skipping(&self.scene, t, size.w, size.h, &skip);
                let lf = self.to_screen_rect(land);
                // With Alt+Shift a coloured trail of two or more keeps its colour (`scene::join`).
                let own = self.scene.node(id).and_then(|n| n.trail.colour.clone()).filter(|_| keep && self.scene.followers(id).len() > 1);
                let colour = own.map_or(colour, |c| th.color(&c));
                let corner = th.card_corner(k, scale, lf.w, lf.h);
                crate::ui::chrome::fill(&mut l, lf, corner, rgb(colour, 0.14));
                crate::ui::chrome::outline(&mut l, lf, corner, outline.0, rgb(colour, 0.8));
                (Vec2::new(tf.right(), row(tf)), Vec2::new(lf.x, row(lf)), colour)
            }
            None => {
                let p = self.cursor;
                let side = if p.x >= card.center().x { card.right() } else { card.x };
                (Vec2::new(side, row(card)), p, th.accent())
            }
        };
        Some(self.line_layer(l, from, to, colour, link))
    }

    /// Adds to `l` a trail line from `from` to `to` (screen px) as the board
    /// draws one, dashed for a link, with a dot at each end.
    pub(super) fn line_layer(&self, mut l: Layer, from: Vec2, to: Vec2, colour: [u8; 3], dashed: bool) -> Layer {
        let scale = self.scale();
        let k = self.camera.pixels_per_unit(scale);
        let th = &self.theme;
        let width = (2.0 * k).clamp(scale.max(1.0), 3.0 * scale);
        let turn = th.card_corner(k, scale, f32::MAX, f32::MAX);
        let path = scene::route::elbow(from, to, turn);
        let pieces: Vec<(Vec2, Vec2)> = if dashed { scene::route::dashes(&path, width * 3.0, width * 2.5) } else { path.windows(2).map(|w| (w[0], w[1])).collect() };
        for (a, b) in pieces {
            l.quads.push(Quad { shape: Shape::Segment { from: [a.x, a.y], to: [b.x, b.y], width }, color: rgb(colour, 0.95), texture: None });
        }
        let d = (7.0 * k).clamp(3.0 * scale, 8.0 * scale);
        for p in [from, to] {
            crate::ui::chrome::fill(&mut l, Rect::new(p.x - d / 2.0, p.y - d / 2.0, d, d), d / 2.0, rgb(colour, 1.0));
        }
        l
    }

    /// The cards behind stack front `front`, nearest first (as ] shows them).
    pub(super) fn stack_behind(&self, front: NodeId) -> Vec<NodeId> {
        let Some(s) = self.scene.stack_of(front) else { return Vec::new() };
        let len = s.members.len();
        (1..len).map(|j| s.members[(s.front_index() + j) % len].id).collect()
    }

    /// The gap between a stack's sheets now, in world units.
    pub(super) fn stack_step(&self) -> f32 {
        let scale = self.scale();
        let k = self.camera.pixels_per_unit(scale);
        stack_step_px(k, scale) / k.max(1e-6)
    }

    /// Cards folded away lose focus and selection; their pages sleep in `sync_liveness`.
    pub(super) fn after_fold(&mut self) {
        if self.focused.is_some_and(|f| self.scene.is_hidden(f)) {
            self.focus(None);
        }
        let hidden: Vec<NodeId> = self.selection.ids().into_iter().filter(|id| self.scene.is_hidden(*id)).collect();
        for id in hidden {
            self.selection.remove(id);
        }
    }

    pub(super) fn fill_view(&mut self, id: NodeId) {
        if let Some(n) = self.scene.node(id) {
            // The margin Shift+F leaves, so a lone card looks the same after either.
            let mut goal = Camera::fit(n.frame(), self.viewport(), self.scale(), scene::FIT_MARGIN * self.scale());
            goal.zoom = goal.zoom.min(2.0);
            self.camera_goal = Some(goal);
        }
    }

    /// Read card `id`: it fills the window and gets focus. Keys stay with the
    /// board (Tab returns) until the page is clicked; other keys reach the page.
    pub(super) fn read_card(&mut self, id: NodeId) {
        if self.scene.node(id).is_none() {
            return;
        }
        if self.read != Some(id) {
            log::info!("solo: card {}", id.0);
        }
        // From the board, the card grows out of its place (if it is on screen).
        if self.read.is_none() {
            self.read_morph = (self.card_on_screen(id) && !self.still).then(|| ReadMorph { id, entering: true, start: Instant::now() });
        }
        self.read = Some(id);
        self.selection.clear();
        self.menu = None;
        self.camera_goal = None;
        self.drag = Drag::None;
        self.focus(Some(id));
        self.keys_in_page = false;
        self.set_page_keys(PageKeys::Page);
    }

    /// Tab: read the focused card (or the one in the middle of the view), or go back to the board.
    pub(super) fn toggle_read(&mut self) {
        self.link_hints = None;
        if self.read.is_some() {
            self.leave_read();
        } else if let Some(id) = read::pick(&self.scene, self.focused, self.camera.center) {
            self.read_card(id);
        }
    }

    /// Back to the board as it was, with the card that was read focused and on screen.
    pub(super) fn leave_read(&mut self) {
        let Some(id) = self.read.take() else { return };
        log::info!("solo: back to the board");
        // A page in full screen comes out of it with Solo.
        if self.page_full.take_if(|(full, _)| *full == id).is_some()
            && let Some(c) = self.cards.get(&id)
            && c.page.fullscreen()
        {
            c.page.exit_fullscreen();
        }
        self.menu = None;
        self.keys_in_page = false;
        self.camera_moved();
        self.ensure_visible(id);
        // Pages go straight back to the board's raster scale (where the camera
        // is heading), so they lay out once instead of at 1:1 and again once it rests.
        let zoom = self.camera_goal.map_or(self.camera.zoom, |g| g.zoom);
        self.page_scale = self.board_page_scale(zoom);
        // It shrinks back into its place (following the camera if that glides).
        self.read_morph = (!self.still).then(|| ReadMorph { id, entering: false, start: Instant::now() });
    }

    /// Whether any of card `id` is in the window.
    pub(super) fn card_on_screen(&self, id: NodeId) -> bool {
        let vp = self.viewport();
        self.scene.node(id).is_some_and(|n| self.to_screen_rect(n.frame()).intersects(&Rect::new(0.0, 0.0, vp.x, vp.y)))
    }

    /// Glide just enough to bring card `id` on screen (fitting it with the focused card if possible).
    /// In Solo, bringing a card on screen means reading it.
    pub(super) fn ensure_visible(&mut self, id: NodeId) {
        if self.read.is_some() {
            self.read_card(id);
            return;
        }
        let Some(frame) = self.scene.node(id).map(|n| n.frame()) else { return };
        let vp = self.viewport();
        // The open sidebar covers the left of the window: fit the card in the rest.
        let left = if self.sidebar_shown() { self.sidebar_rect().right() } else { 0.0 };
        let on_screen = self.to_screen_rect(frame);
        if on_screen.x >= left && on_screen.y >= 0.0 && on_screen.right() <= vp.x && on_screen.bottom() <= vp.y {
            return;
        }
        let mut goal = self.camera;
        let scale = self.scale();
        let need = Camera::fit(frame, Vec2::new(vp.x - left, vp.y), scale, 32.0 * scale);
        if need.zoom < goal.zoom {
            goal.zoom = need.zoom;
        }
        goal.center = frame.center();
        goal.center.x -= left / 2.0 / goal.pixels_per_unit(scale);
        self.camera_goal = Some(goal);
    }

    pub(super) fn step_camera(&mut self, dt: f32) {
        let Some(goal) = self.camera_goal else { return };
        let t = 1.0 - (-dt * 14.0).exp();
        let c = &mut self.camera;
        c.center = c.center + (goal.center - c.center) * t;
        // Interpolate zoom in log space so it feels even.
        c.zoom = (c.zoom.ln() + (goal.zoom.ln() - c.zoom.ln()) * t).exp();
        let close = (c.center.x - goal.center.x).abs() < 0.5
            && (c.center.y - goal.center.y).abs() < 0.5
            && (c.zoom - goal.zoom).abs() < 0.001;
        if close {
            *c = goal;
            self.camera_goal = None;
        }
        self.camera_moved();
    }

    /// Re-rasterize pages at a resolution matching the settled zoom, and hide
    /// pages that are off screen so they stop painting.
    pub(super) fn sync_pages(&mut self) {
        // A read page is shown 1:1, so it rasterizes as if the zoom were 1.
        let reading = self.read;
        let settled =
            reading.is_some() || (self.camera_changed.elapsed() > Duration::from_millis(250) && self.camera_goal.is_none());
        // The scale first, so pages that wake below start at it rather than
        // at the old one and then lay out again.
        if settled {
            self.page_scale = self.settled_page_scale();
        }
        self.sync_liveness(settled && !matches!(self.drag, Drag::Pan { .. }));
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let page_scale = self.page_scale;
        let mut updates = Vec::new();
        let read_css = self.read_layout().css;
        for n in self.scene.nodes() {
            let css = self.card_css(n);
            let board = (css.x as i32, css.y as i32);
            let (on_screen, size) = match reading {
                Some(r) if r == n.id => (true, read_css),
                Some(_) => (false, board),
                None => (self.to_screen_rect(n.frame()).intersects(&screen), board),
            };
            updates.push((n.id, on_screen, size));
        }
        for (id, visible, size) in updates {
            // A docked inspector takes its share of the card.
            let (size, insp) = match self.docks.get(&id) {
                Some(d) => {
                    let s = dock::split(Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32), d.side, d.share, DOCK_GAP);
                    ((s.page.w as i32, s.page.h as i32), Some((s.inspector.w as i32, s.inspector.h as i32)))
                }
                None => (size, None),
            };
            let (run, focused) = (self.keeps_running(id), self.focused == Some(id));
            if let Some(card) = self.cards.get_mut(&id) {
                Self::show_card(id, card, visible, run, focused);
                card.page.resize(size, page_scale);
            }
            if let (Some(d), Some(insp)) = (self.docks.get_mut(&id), insp) {
                if d.hidden == visible {
                    d.hidden = !visible;
                    d.page.set_hidden(!visible);
                }
                d.page.resize(insp, page_scale);
            }
        }
        // Cards folded in a stack that stay awake are never on screen.
        let folded: Vec<NodeId> = self.cards.keys().copied().filter(|id| self.scene.is_hidden(*id)).collect();
        for id in folded {
            let (run, focused) = (self.keeps_running(id), self.focused == Some(id));
            if let Some(card) = self.cards.get_mut(&id) {
                Self::show_card(id, card, false, run, focused);
            }
        }
    }

    /// Tells card `id`'s page whether it is hidden, how fast to paint and
    /// whether it holds its focus. A page off screen is hidden (it stops
    /// painting and Chromium slows it) unless it keeps running (`run`: see
    /// [`App::keeps_running`]); then it paints slowly, or at full speed if
    /// the card is *Always active*.
    pub(super) fn show_card(id: NodeId, card: &mut Card, visible: bool, run: Option<Awake>, focused: bool) {
        let hidden = !visible && run.is_none();
        if card.hidden != hidden {
            card.hidden = hidden;
            card.page.set_hidden(hidden);
            if !visible && !hidden {
                log::info!("card {} keeps running off screen ({})", id.0, run.map_or("", Awake::name));
            }
        }
        // A hidden page doesn't paint; only one running unseen paints slowly.
        let fps = if hidden || visible || run == Some(Awake::Active) { FULL_FPS } else { UNSEEN_FPS };
        if card.fps != fps {
            card.fps = fps;
            card.page.set_frame_rate(fps);
        }
        let held = run == Some(Awake::Active);
        if card.held != held {
            card.held = held;
            card.page.hold_focus(held);
            if !held && !focused {
                card.page.set_focus(false);
            }
            log::info!("card {} {} its focus", id.0, if held { "holds" } else { "no longer holds" });
        }
    }

    /// Whether card `id` keeps running off screen, and why: *Always active*,
    /// or media playing now (`Auto` / `Keep`). `None`: it may be hidden.
    pub(super) fn keeps_running(&self, id: NodeId) -> Option<Awake> {
        let awake = self.scene.node(id).map_or(Awake::Auto, |n| n.awake);
        if awake == Awake::Active {
            return Some(Awake::Active);
        }
        self.media.get(&id).filter(|m| m.playing).map(|_| awake)
    }

    /// Whether card `id` sits outside the live cap: kept awake by its mode,
    /// or by media playing (or paused a short while ago).
    pub(super) fn exempt(&self, id: NodeId) -> bool {
        if self.scene.node(id).is_some_and(|n| !n.awake.is_auto()) {
            return true;
        }
        self.media.get(&id).is_some_and(|m| liveness::media_keeps(m.playing, m.stopped.map(|t| t.elapsed())))
    }

    /// Takes a page's report that it started or stopped playing media.
    pub(super) fn got_media(&mut self, id: NodeId, playing: bool) {
        if !self.cards.contains_key(&id) {
            return;
        }
        let stopped = if playing { None } else { Some(Instant::now()) };
        self.media.insert(id, Media { playing, stopped });
        log::info!("card {} {} media", id.0, if playing { "plays" } else { "stopped its" });
    }

    /// The icon of an awake mode: a ring for *Auto* and *Keep awake*, a
    /// solid disc for *Always active* (docs/DESIGN.md, "Awake mark").
    pub(super) fn awake_icon(mode: Awake) -> crate::ui::icons::Icon {
        match mode {
            Awake::Auto | Awake::Keep => crate::ui::icons::Icon::AwakeRing,
            Awake::Active => crate::ui::icons::Icon::AwakeDisc,
        }
    }

    /// The colour of an awake mode's mark; `None` for *Auto*, drawn like the
    /// other buttons.
    pub(super) fn awake_colour(th: &crate::ui::theme::Theme, mode: Awake) -> Option<crate::ui::theme::Rgb> {
        match mode {
            Awake::Auto => None,
            Awake::Keep => Some(th.awake_keep()),
            Awake::Active => Some(th.awake_active()),
        }
    }

    /// Mutes card `id`'s sound, or turns it back on.
    pub(super) fn toggle_mute(&mut self, id: NodeId) {
        let Some(c) = self.cards.get(&id) else { return };
        let muted = !c.page.muted();
        c.page.set_muted(muted);
        log::info!("card {} {}", id.0, if muted { "muted" } else { "unmuted" });
        self.toast(if muted { "Muted" } else { "Sound on" });
    }

    /// `:media` and the `,` keys: `op` on the current card's video or audio.
    pub(super) fn media_op(&mut self, op: scene::media::Op) {
        let Some(id) = self.current() else { return self.toast("No card") };
        if op == scene::media::Op::Mute {
            return self.toggle_mute(id);
        }
        let Some(c) = self.cards.get(&id) else { return self.toast("The card is asleep") };
        if c.page.kind() != web::PageKind::Web {
            return self.toast(scene::media::Status::None.describe());
        }
        let (name, value) = op.wire();
        log::info!("media {name} {value} in card {}", id.0);
        if c.page.media_control(name, value, scene::media::RATES) {
            self.media_asked = Some((id, Instant::now()));
        } else {
            self.toast(scene::media::Status::None.describe());
        }
    }

    /// A page's answer to [`App::media_op`]: a toast, if it was asked of
    /// that card lately.
    pub(super) fn media_done(&mut self, id: NodeId, answer: &str) {
        let asked = self.media_asked.is_some_and(|(at, when)| at == id && when.elapsed() < Duration::from_secs(5));
        if !asked {
            return;
        }
        let Some(status) = scene::media::Status::parse(answer) else { return };
        log::info!("media answer in card {}: {answer}", id.0);
        self.toast(status.describe());
    }

    /// A video in card `id` can't play: say why, once per page and kind.
    pub(super) fn media_trouble(&mut self, id: NodeId, kind: &str) {
        let Some(kind) = scene::media::Trouble::parse(kind) else { return };
        let url = self.scene.node(id).map(|n| n.url.clone()).unwrap_or_default();
        if !self.media_told.insert((id, kind, url.clone())) {
            return;
        }
        let site = scene::address::site(&url).unwrap_or_default().to_string();
        log::info!("media trouble in card {}: {kind:?} on {url}", id.0);
        self.toast_for(kind.describe(&site), Duration::from_secs(6));
    }

    /// Page `id` went into full screen, or out: the card goes Solo (it
    /// fills the window), and back to where it was after.
    pub(super) fn page_fullscreen(&mut self, id: NodeId, on: bool) {
        log::info!("card {} page full screen {}", id.0, if on { "on" } else { "off" });
        if on {
            if self.page_full.is_none_or(|(full, _)| full != id) {
                self.page_full = Some((id, self.read));
            }
            if self.read != Some(id) {
                self.read_card(id);
            }
        } else if let Some((full, before)) = self.page_full.take_if(|(full, _)| *full == id) {
            match before {
                Some(b) if b != full && self.scene.node(b).is_some() => self.read_card(b),
                Some(_) => {}
                None if self.read == Some(full) => self.leave_read(),
                None => {}
            }
        }
    }

    /// Sets card `id`'s awake mode (`None`: the next one, as the button cycles).
    pub(super) fn set_awake(&mut self, id: NodeId, mode: Option<Awake>) {
        let Some(n) = self.scene.node_mut(id) else { return };
        let mode = mode.unwrap_or(n.awake.next());
        if n.awake == mode {
            return;
        }
        n.awake = mode;
        self.dirty = true;
        log::info!("card {} awake: {}", id.0, mode.name());
        self.toast(match mode {
            Awake::Auto => "Auto: this card sleeps when the live limit needs room",
            Awake::Keep => "Keep awake: this card never sleeps",
            Awake::Active => "Always active: this card never sleeps and runs as if in view",
        });
    }

    /// The raster scale for pages once the camera rests: the zoom's
    /// resolution in quarter steps (0.5–2), 1:1 in Solo.
    pub(super) fn settled_page_scale(&self) -> f32 {
        if self.read.is_some() {
            return Self::page_scale_for(1.0, self.scale());
        }
        self.board_page_scale(self.camera.zoom)
    }

    /// The raster scale for pages on the board at camera `zoom`. Pages lay
    /// out at Solo's width (`card_css`), so a card of the default width
    /// shows `CARD_W / read width` world units per CSS px. One scale for all
    /// cards: Chromium shares page zoom (our raster scale) across a site.
    pub(super) fn board_page_scale(&self, zoom: f32) -> f32 {
        let css = read::card_css(Vec2::new(scene::CARD_W, scene::CARD_H), self.read_layout().css.0);
        Self::page_scale_for(zoom * scene::CARD_W / css.x, self.scale())
    }

    /// The raster scale for camera `zoom` on a window of `scale`.
    pub(super) fn page_scale_for(zoom: f32, scale: f32) -> f32 {
        ((zoom * scale) * 4.0).round().clamp(2.0, 8.0) / 4.0
    }

    /// Keep at most `max_live` live browsers. Sleeping cards on screen wake only
    /// when `allow_wake` (the camera has settled), so panning doesn't start pages.
    pub(super) fn sync_liveness(&mut self, allow_wake: bool) {
        // Cards folded away in a stack don't keep a live browser, unless
        // they are kept awake (by their mode or by media).
        let folded: Vec<NodeId> = self.cards.keys().copied().filter(|id| self.scene.is_hidden(*id) && !self.exempt(*id)).collect();
        for id in folded {
            log::info!("sleeping card {} (folded)", id.0);
            self.sleep_page(id);
        }
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        // An inspector and the card it inspects stay live together.
        let paired = |id: NodeId| self.inspectors.contains_key(&id) || self.inspectors.values().any(|&s| s == id) || self.docks.contains_key(&id);
        let candidates: Vec<liveness::Candidate> = self
            .scene
            .all_nodes()
            .filter(|n| !paired(n.id) && (!self.scene.is_hidden(n.id) || self.exempt(n.id)))
            .map(|n| liveness::Candidate {
                id: n.id,
                live: self.cards.contains_key(&n.id),
                visible: match self.read {
                    Some(r) => r == n.id,
                    None => !self.scene.is_hidden(n.id) && self.to_screen_rect(n.frame()).intersects(&screen),
                },
                focused: self.focused == Some(n.id),
                last_used: self.used.get(&n.id).copied().unwrap_or(0),
                exempt: self.exempt(n.id),
            })
            .collect();
        let plan = liveness::plan(&candidates, self.max_live, allow_wake);
        for id in plan.sleep {
            self.sleep_page(id);
        }
        for id in plan.wake {
            let why = match self.scene.node(id).map(|n| n.awake) {
                Some(Awake::Keep) => "kept awake",
                Some(Awake::Active) => "always active",
                _ => "on screen",
            };
            log::info!("waking card {} ({why})", id.0);
            let used = self.used.get(&id).copied();
            self.create_page(id);
            // Coming into view isn't use; keep its old recency.
            match used {
                Some(u) => self.used.insert(id, u),
                None => self.used.remove(&id),
            };
        }
    }
}
