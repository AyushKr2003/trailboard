//! Drawing.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use compositor::{Background, Layer, Quad, Renderer, Shape};
use scene::keys::Outcome;
use scene::settings::self as prefs;
use scene::{dock, Edge, motion, Node, NodeId, read, Rect, selection, userkeys, Vec2};

use crate::dev::script::Script;
use crate::keyboard::keymap;
use crate::system::desktop::Desktop;
use crate::ui::omnibox::Target;
use crate::ui::palette::{self, Command, Item};
use crate::ui::theme::{rgb, text_on};
use super::{anim_key, App, ARRANGE_HELP, BAR_BTN, BAR_CLOSE_MIN, BAR_GAP, BAR_H, BAR_NAV_MIN, Confirm, dock_key, DockParts, Drag, Hit, IDLE_DRAW, LABELS_ALL_BELOW, menu_layer, page_key, PageKeys, PREFIX_PANEL_AFTER, PREVIEW_KEY, ReadMorph, SCROLL_FLASH, stack_step_px};

impl App {
    pub(crate) fn draw(&mut self) {
        let scale = self.scale();
        let vp = self.viewport();
        let k = self.camera.pixels_per_unit(scale);

        // Upload new page frames.
        let epoch = self.epoch;
        if let Some(r) = self.renderer.as_mut() {
            self.page_css.clear();
            for (id, card) in &self.cards {
                let f = card.page.frame();
                let key = page_key(epoch, *id);
                // Only what the page repainted since the last upload (a caret).
                let changed = r.page_serial(key).and_then(|s| f.changed_since(s)).map(|c| [c.x, c.y, c.w, c.h].map(|v| v.max(0) as u32));
                r.update_page_part(key, f.width, f.height, &f.pixels, f.serial, changed);
                self.page_css.insert(*id, f.css);
            }
            self.dock_css.clear();
            for (id, d) in &self.docks {
                let f = d.page.frame();
                let key = dock_key(epoch, *id);
                let changed = r.page_serial(key).and_then(|s| f.changed_since(s)).map(|c| [c.x, c.y, c.w, c.h].map(|v| v.max(0) as u32));
                r.update_page_part(key, f.width, f.height, &f.pixels, f.serial, changed);
                self.dock_css.insert(*id, f.css);
            }
            for s in self.icons.values() {
                r.update_page(s.key, s.icon.width, s.icon.height, &s.icon.pixels, s.serial);
            }
        }
        self.load_sleeping_thumbs();
        if self.read_morph.is_some_and(|m| m.done() || self.scene.node(m.id).is_none()) {
            self.read_morph = None;
        }
        // While a card grows into Solo the board still shows behind it.
        let board_hidden = self.read.is_some() && self.read_morph.is_none();
        let preview_url = self
            .omnibox
            .as_ref()
            .filter(|ob| ob.target.is_list())
            .and_then(|ob| self.list_rows(ob).get(ob.sel).and_then(|row| self.preview_url(row)));
        let preview = self.load_preview(preview_url);

        let origin = self.camera.world_to_screen(Vec2::default(), vp, scale);
        let th = self.theme.clone();
        let ui = self.ui();
        let tint = self.spaces.active_entry().tint.clone();
        // The grid keeps its dots 20 to 40 px apart at any zoom. Zooming in,
        // dots fade in halfway between them, and at 40 px those become the
        // grid: its density never jumps.
        let base = (32.0 * k).max(1e-3);
        let spacing = base * 2f32.powf((20.0 * scale / base).log2().ceil());
        // Behind the cards: the space's photo once it is read, else its
        // colour mood (a soft gradient with grain), else its flat colour.
        let photo = self.board_photo();
        let update = self.photo.update(photo.as_deref(), (vp.x.max(0.0) as u32, vp.y.max(0.0) as u32));
        if let (Some(t), Some(r)) = (update.upload, self.renderer.as_mut()) {
            r.update_page(crate::board::backdrop::TEXTURE, t.width, t.height, &t.pixels, self.photo.serial);
        }
        if let Some((p, e)) = update.failed {
            let name = p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().to_string());
            self.toast(format!("Can't show {name} as the background: {e}"));
        }
        let photo_on = photo.as_deref().is_some_and(|p| self.photo.ready(p)) && !board_hidden;
        let (near, far) = self.backdrop_colours();
        let (grain, dots) = match self.settings.backdrop {
            prefs::Backdrop::Plain => (0.0, 1.0),
            _ => (1.5, if photo_on { 0.0 } else { 0.7 }),
        };
        // Just switched space: the colours cross-fade from the old space's.
        let now = Instant::now();
        let space_t = match self.space_in {
            Some((at, ..)) if motion::progress(at, motion::SPACE, now) < 1.0 => Some(motion::ease_out(motion::progress(at, motion::SPACE, now))),
            _ => {
                self.space_in = None;
                None
            }
        };
        let mix = |old: [f32; 4], new: [f32; 4]| match space_t {
            Some(t) => std::array::from_fn(|i| old[i] + (new[i] - old[i]) * t),
            None => new,
        };
        let (old_near, old_far) = self.space_in.map_or((rgb(near, 1.0), rgb(far, 1.0)), |s| (s.2, s.3));
        let bg = Background {
            color: mix(old_near, rgb(near, 1.0)),
            color2: mix(old_far, rgb(far, 1.0)),
            grain,
            dot: rgb(th.dots(), dots),
            grid_origin: [origin.x, origin.y],
            grid_spacing: spacing,
            dot_radius: (1.0 * scale).max(0.75),
            fine: ((spacing / (20.0 * scale)) - 1.0).clamp(0.0, 1.0),
        };

        let mut layers: Vec<Layer> = Vec::new();
        if photo_on {
            // The photo fills the window and stays put while the board moves,
            // a little dimmed so cards and labels stand off it.
            let dim = if th.light() { 0.94 } else { 0.78 };
            let mut l = Layer::default();
            l.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(0.0, 0.0, vp.x, vp.y), radius: 0.0 },
                color: [dim, dim, dim, 1.0],
                texture: Some(crate::board::backdrop::TEXTURE),
            });
            layers.push(l);
        }
        let line = crate::ui::chrome::hair(scale);
        // What the pointer is over lights up, unless something else has the pointer.
        let hover = if matches!(self.drag, Drag::None) && self.menu.is_none() && self.omnibox.is_none() && !self.hints && self.pointer_in {
            self.hover
        } else {
            Hit::Board
        };

        // Trail edges under everything: from the source's side to the
        // target's, near their tops, turning at right angles halfway across;
        // the turns are rounded like the cards' corners at this zoom
        // (`scene::route`), square when the cards are. A trail is drawn in its own colour, or a
        // quiet neutral if it has none; the current card's trail is brought
        // out, so its path can be followed through a busy board.
        // Everything from here to the cards' last layer is the board, which
        // slides in on a space switch.
        let board_from = layers.len();
        // A card on its way somewhere is drawn where it is now (`glides`).
        let glides = &self.glides;
        let shown_frame = |n: &Node| match glides.at(n.id, n.rect, now) {
            Some((r, _)) => Rect::new(r.x, r.y - n.header_h(), r.w, r.h + n.header_h()),
            None => n.frame(),
        };
        let mut edges = Layer::default();
        let labels: HashMap<NodeId, scene::TrailMark> =
            if board_hidden { HashMap::new() } else { self.scene.trail_labels().into_iter().collect() };
        // A line takes the colour of the card it runs into, so a part of
        // another colour that joined a trail is that colour up to it.
        let edge_colour = |id: NodeId| self.scene.node(id).and_then(|n| n.trail.colour.as_deref()).map(|c| th.color(c));
        let current_trail: HashSet<NodeId> = self.focused.map(|f| self.scene.trail_of(f).into_iter().collect()).unwrap_or_default();
        // Each line with how far it has drawn in (a join's) and how strongly
        // it shows (a leave's, fading out), then those cut lines themselves.
        let now = Instant::now();
        self.edges_in.retain(|(_, at)| motion::progress(*at, motion::FOLD, now) < 1.0);
        self.edges_out.retain(|(_, at)| motion::progress(*at, motion::FOLD, now) < 1.0);
        let board_edges: Vec<(Edge, f32, f32)> = if board_hidden {
            Vec::new()
        } else {
            let grown = |e: &Edge| self.edges_in.iter().find(|(x, _)| x == e).map_or(1.0, |(_, at)| motion::ease_out(motion::progress(*at, motion::FOLD, now)));
            let exists = |e: &Edge| if e.link { self.scene.links().contains(e) } else { self.scene.edges().contains(e) };
            let fading = self.edges_out.iter().filter(|(e, _)| !exists(e) && !self.scene.is_hidden(e.from) && !self.scene.is_hidden(e.to));
            // Links (`gl`) after the trail lines, drawn dashed.
            self.scene
                .shown_edges()
                .into_iter()
                .chain(self.scene.shown_links())
                .map(|e| (e, grown(&e), 1.0))
                .chain(fading.map(|(e, at)| (*e, 1.0, 1.0 - motion::ease_out(motion::progress(*at, motion::FOLD, now)))))
                .collect()
        };
        for &(ref e, grow, fade) in &board_edges {
            let (Some(a), Some(b)) = (self.scene.node(e.from), self.scene.node(e.to)) else { continue };
            let fa = self.to_screen_rect(shown_frame(a));
            let fb = self.to_screen_rect(shown_frame(b));
            // Near the top of each card, where its name is.
            let (ya, yb) = (fa.y + (28.0 * k).min(fa.h / 2.0), fb.y + (28.0 * k).min(fb.h / 2.0));
            let (p0, p3) = if fb.x >= fa.right() { ([fa.right(), ya], [fb.x, yb]) } else { ([fa.x, ya], [fb.right(), yb]) };
            // A link between cards one above the other runs straight down
            // (or up) where they overlap, rather than out of their sides.
            let (p0, p3) = match (e.link, fa.x.max(fb.x), fa.right().min(fb.right())) {
                (true, l, r) if l < r && fa.bottom() <= fb.y => ([(l + r) / 2.0, fa.bottom()], [(l + r) / 2.0, fb.y]),
                (true, l, r) if l < r && fb.bottom() <= fa.y => ([(l + r) / 2.0, fa.y], [(l + r) / 2.0, fb.bottom()]),
                _ => (p0, p3),
            };
            let hot = current_trail.contains(&e.from) && current_trail.contains(&e.to);
            // A trail line takes the colour of the card it runs into; a link
            // that of the card it was made from.
            let marked = edge_colour(if e.link { e.from } else { e.to });
            let (colour, alpha) = match (marked, hot) {
                (Some(c), true) => (c, 1.0),
                (Some(c), false) => (c, 0.7),
                (None, true) => (th.accent(), 0.9),
                (None, false) => (th.line_strong(), 1.0),
            };
            let width = ((if hot { 2.0 } else { 1.5 }) * k).clamp(scale.max(1.0), 3.0 * scale);
            let turn = th.card_corner(k, scale, f32::MAX, f32::MAX);
            let path = scene::route::part(&scene::route::elbow(Vec2::new(p0[0], p0[1]), Vec2::new(p3[0], p3[1]), turn), grow);
            let pieces: Vec<(Vec2, Vec2)> =
                if e.link { scene::route::dashes(&path, width * 3.0, width * 2.5) } else { path.windows(2).map(|w| (w[0], w[1])).collect() };
            for (a, b) in pieces {
                edges.quads.push(Quad { shape: Shape::Segment { from: [a.x, a.y], to: [b.x, b.y], width }, color: rgb(colour, alpha * fade), texture: None });
            }
            // A dot where the line leaves a card and where it arrives (once
            // a line drawing in gets there).
            let d = (7.0 * k).clamp(3.0 * scale, 8.0 * scale);
            let ends: &[[f32; 2]] = if grow < 1.0 { &[p0] } else { &[p0, p3] };
            for p in ends {
                crate::ui::chrome::fill(&mut edges, Rect::new(p[0] - d / 2.0, p[1] - d / 2.0, d, d), d / 2.0, rgb(colour, fade));
            }
        }
        layers.push(edges);

        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let band = self.band();
        let in_band = band.map(|b| selection::in_band(&self.scene, b)).unwrap_or_default();
        let typing = self.entered();
        let board_cards = if board_hidden { Vec::new() } else { self.scene.draw_order() };
        let morphing = self.read_morph.map(|m| m.id);
        self.shields.clear();
        self.tags.clear();
        self.bars.clear();
        self.bar_buttons.clear();
        let mut in_view = false;
        let now = Instant::now();
        let crowd = k / scale < LABELS_ALL_BELOW;
        // A trail folding: each card slides onto its sheet behind the
        // stack's front, under it, its page fading as it arrives, so it lands
        // as that blank sheet. One unfolded again (or closed) since is drawn
        // as usual instead.
        self.gathers.retain(|g| {
            motion::progress(g.start, motion::FOLD, now) < 1.0 && !board_hidden && self.scene.is_hidden(g.id) && self.scene.stack_of(g.front).is_some()
        });
        let step = stack_step_px(k, scale) / k.max(1e-6);
        let mut gathers = self.gathers.clone();
        gathers.sort_by_key(|g| std::cmp::Reverse(g.slot));
        for g in &gathers {
            let Some(front) = self.scene.node(g.front).map(|n| n.frame()) else { continue };
            let sheet = motion::stack_slot(front, g.slot, step);
            let t = motion::progress(g.start, motion::FOLD, now);
            let (frame, _) = motion::folding(g.frame, sheet, t);
            let (content, page) = motion::folding(g.content, sheet, t);
            let (frame, content) = (self.to_screen_rect(frame), self.to_screen_rect(content));
            if !frame.intersects(&screen) {
                continue;
            }
            let radius = th.card_corner(k, scale, frame.w, frame.h);
            let key = self.page_key(g.id);
            let has_tex = self.renderer.as_ref().is_some_and(|rd| rd.has_page(key));
            let mut l = Layer::default();
            l.quads.push(Quad { shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (20.0 * k).max(2.0) }, color: th.shadow(0.24), texture: None });
            crate::ui::chrome::fill(&mut l, frame, radius, rgb(th.card(false), 1.0));
            crate::ui::chrome::outline(&mut l, frame, radius, line, rgb(th.line_strong(), 1.0));
            if page > 0.0 {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: content, radius },
                    color: if has_tex { [1.0, 1.0, 1.0, page] } else { rgb(th.card(false), page) },
                    texture: has_tex.then_some(key),
                });
            }
            layers.push(l);
        }
        for n in board_cards {
            // The card on its way to or from Solo is drawn by `read_morph_layer`.
            if morphing == Some(n.id) {
                in_view = true;
                continue;
            }
            // Gliding or growing in: drawn where it is on the way, fading in if new.
            let glide = self.glides.at(n.id, n.rect, now);
            let moved;
            let n = match glide {
                Some((r, _)) => {
                    moved = Node { rect: r, ..n.clone() };
                    &moved
                }
                None => n,
            };
            let frame = self.to_screen_rect(n.frame());
            in_view |= frame.intersects(&screen);
            if !frame.inflate(64.0 * scale).intersects(&screen) {
                continue;
            }
            let content = self.to_screen_rect(n.rect);
            // Cards are sheets: the corner radius in use, scaled with the
            // zoom but kept visible when zoomed far out.
            let radius = th.card_corner(k, scale, frame.w, frame.h);
            let focused = self.focused == Some(n.id);
            let hovered = hover.id() == Some(n.id);
            let selected = self.selection.contains(n.id) || in_band.contains(&n.id);
            let card = self.cards.get(&n.id);
            let asleep = card.is_none();
            let sticky = n.sticky.as_deref().filter(|_| n.is_sticky());
            // Coming out of a stack: a blank sheet, its page showing on it on the way.
            if self.glides.unfolding(n.id, now) {
                let mut sheet = Layer::default();
                sheet.quads.push(Quad { shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (20.0 * k).max(2.0) }, color: th.shadow(0.24), texture: None });
                crate::ui::chrome::fill(&mut sheet, frame, radius, rgb(th.card(false), 1.0));
                crate::ui::chrome::outline(&mut sheet, frame, radius, line, rgb(th.line_strong(), 1.0));
                layers.push(sheet);
            }
            let mut l = Layer::default();

            // A stack shows the edges of up to three cards behind its front, down and to the right.
            let stack = self.scene.stack_of(n.id).map(|s| (s.front_index() + 1, s.members.len()));
            if let Some((_, len)) = stack {
                // At least a few px apart, so a stack still reads when zoomed far out.
                let step = stack_step_px(k, scale);
                let behind = len.min(motion::STACK_SHOWN + 1) - 1;
                // A sheet a folding card is still on its way to is that card (drawn above).
                let arriving: HashSet<usize> = self.gathers.iter().filter(|g| g.front == n.id).map(|g| g.slot).collect();
                let last = frame.translate(Vec2::new(step * behind as f32, step * behind as f32));
                if arriving.is_empty() {
                    l.quads.push(Quad {
                        shape: Shape::Shadow { rect: last.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (20.0 * k).max(2.0) },
                        color: th.shadow(0.24),
                        texture: None,
                    });
                }
                for i in (1..=behind).rev().filter(|i| !arriving.contains(i)) {
                    let back = frame.translate(Vec2::new(step * i as f32, step * i as f32));
                    crate::ui::chrome::fill(&mut l, back, radius, rgb(th.card(false), 1.0));
                    crate::ui::chrome::outline(&mut l, back, radius, line, rgb(th.line_strong(), 1.0));
                }
            }
            // One light from above: a wide, soft shadow around the card and a
            // tight one under its edge. The current card floats a little
            // higher, the one under the pointer a touch. Both keep some size
            // when zoomed out, so cards still float over the board.
            let (ambient, key) = if focused {
                ((12.0, 44.0, 0.40), (2.0, 6.0, 0.30))
            } else if hovered {
                ((9.0, 34.0, 0.32), (1.5, 4.0, 0.26))
            } else {
                ((6.0, 26.0, 0.26), (1.0, 3.0, 0.22))
            };
            for (dy, blur, a) in [ambient, key] {
                let lo = 0.35 * scale;
                l.quads.push(Quad {
                    shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, (dy * k).max(dy * lo))), radius, blur: (blur * k).max(blur * lo).max(1.5) },
                    color: th.shadow(a),
                    texture: None,
                });
            }
            // A sticky note's frame is its strip, in its colour.
            crate::ui::chrome::fill(&mut l, frame, radius, rgb(sticky.map_or_else(|| th.card(focused), |c| th.sticky_strip(c, focused)), 1.0));
            let key = page_key(epoch, n.id);
            let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
            // With a docked inspector the page takes its part of the card.
            let parts = self.dock_split(n.id, content, self.card_css(n));
            let (page_rect, page_css) = parts.as_ref().map_or((content, self.card_css(n)), |p| p.page);
            // A live page's last frame at its own size: right after a resize
            // (leaving Solo, resizing the card) it is cropped or filled
            // out, never squashed, until the page paints at the new size.
            let extent = match (card, self.page_css.get(&n.id)) {
                (Some(_), Some(&css)) => read::extent(page_css, css),
                _ => [1.0, 1.0],
            };
            let side = self.docks.get(&n.id).map(|d| d.side);
            let square = match (sticky, side) {
                (Some(_), _) => compositor::corners::TOP,
                (None, Some(dock::Side::Right)) => compositor::corners::RIGHT,
                (None, Some(dock::Side::Bottom)) => compositor::corners::BOTTOM,
                (None, None) => 0,
            };
            l.quads.push(Quad {
                // The page is the card: clipped to its rounded shape. (A
                // sticky note's page sits under its strip: rounded below only.)
                shape: Shape::Page { rect: page_rect, radius, square, extent },
                color: match (has_tex, asleep) {
                    (true, false) => [1.0; 4],
                    (true, true) => [0.72, 0.72, 0.74, 1.0], // a sleeping card's last frame, dimmed
                    // First paint pending, or asleep and never painted.
                    (false, _) => rgb(sticky.map_or_else(|| th.placeholder(), |c| th.sticky_paper(c)), 1.0),
                },
                texture: has_tex.then_some(key),
            });
            if let (Some(parts), Some(side)) = (&parts, side) {
                let hot = hover == Hit::Divider(n.id) || matches!(self.drag, Drag::Divider { id } if id == n.id);
                self.dock_layer(&mut l, n.id, parts, side, radius, focused, hot);
            }
            if asleep && !has_tex && sticky.is_none() {
                // Never painted: the site's icon and the title, centred, so
                // the card still says what it is.
                let s = (32.0 * k).clamp(12.0 * scale, 32.0 * scale);
                // Readable when the card has room for it, else left out.
                let size = (14.0 * k).max(11.0 * scale).min(14.0 * scale);
                let size = if content.w >= 120.0 * scale && content.h >= 60.0 * scale { size } else { 0.0 };
                let gap = (10.0 * k).max(6.0 * scale);
                let block = s + if size >= 9.0 { gap + size } else { 0.0 };
                let top = content.center().y - block / 2.0;
                if let Some(key) = self.icon_key(&n.url) {
                    l.quads.push(Quad {
                        shape: Shape::Rect { rect: Rect::new((content.center().x - s / 2.0).round(), top.round(), s, s), radius: 0.0 },
                        color: [0.85, 0.85, 0.87, 1.0],
                        texture: Some(key),
                    });
                } else if let Some(r) = self.renderer.as_mut() {
                    crate::ui::icons::draw(&mut l, r, crate::ui::icons::Icon::Moon, content.center().x, top + s / 2.0, s * 0.7, rgb(th.text3(), 1.0));
                }
                if size >= 9.0 {
                    let title = if n.title.trim().is_empty() { palette::host(&n.url).to_string() } else { n.title.trim().to_string() };
                    let room = content.w - 24.0 * k;
                    let w = self.renderer.as_mut().map_or(room, |r| r.measure(&title, size, false)).min(room);
                    let clip = Rect::new(content.center().x - w / 2.0, top + s + gap - size, w, size * 2.0);
                    crate::ui::chrome::text(&mut l, title, clip.x, top + s + gap + size / 2.0, size, rgb(th.text2(), 1.0), clip, false);
                }
            }
            if sticky.is_none() {
                // The hairline that keeps a card crisp against the board, over
                // the page's own edge too (a white page on a light board);
                // a step stronger under the pointer.
                let edge = if hovered && !focused { th.line_strong() } else { th.line() };
                crate::ui::chrome::outline(&mut l, frame, radius, line, rgb(edge, 1.0));
            }
            // Loading: a bar gliding along the card's top edge.
            if sticky.is_none() && card.is_some_and(|c| c.loading) {
                let t = (self.started.elapsed().as_secs_f32() * 0.7).fract();
                // Eased, so it gathers and lets go rather than sliding at one speed.
                let t = t * t * (3.0 - 2.0 * t);
                let w = frame.w * 0.3;
                let x = frame.x - w + (frame.w + w) * t;
                let (x0, x1) = (x.max(frame.x + radius), (x + w).min(frame.right() - radius));
                if x1 > x0 {
                    crate::ui::chrome::fill(&mut l, Rect::new(x0, frame.y, x1 - x0, 2.0 * scale.max(1.0)), 0.0, rgb(th.accent(), 1.0));
                }
            }
            // The corner that resizes shows itself under the pointer.
            if hover == Hit::Grip(n.id) {
                let s = 14.0 * scale;
                let at = Rect::new(content.right() - s - 3.0 * scale, content.bottom() - s - 3.0 * scale, s, s);
                crate::ui::chrome::fill(&mut l, at.inflate(2.0 * scale), th.small_radius(scale), rgb(th.panel(), 0.92));
                if let Some(r) = self.renderer.as_mut() {
                    crate::ui::icons::draw_scaled(&mut l, r, crate::ui::icons::Icon::Grip, at, rgb(th.text(), 1.0));
                }
            }
            // Rings sit outside the frame: the selection's, then the current
            // card's. The page with the keys (or a card being moved by
            // keyboard) gets a solid ring; a card that is only current a
            // thin, calm one.
            let arranging = self.arrange.as_ref().is_some_and(|a| a.id == n.id);
            let ring_w = if !focused { 0.0 } else if typing || arranging { 2.5 * scale.max(1.0) } else { 1.5 * scale.max(1.0) };
            if focused {
                let alpha = if typing || arranging { 1.0 } else { 0.8 };
                // An entered card's ring says where its keys go.
                let colour = if typing && !arranging { th.keys_ring(self.ring_keys()) } else { th.accent() };
                crate::ui::chrome::outline(&mut l, frame.inflate(ring_w), if radius > 0.0 { radius + ring_w } else { 0.0 }, ring_w, rgb(colour, alpha));
            }
            if selected {
                let w = 2.0 * scale.max(1.0);
                let m = ring_w + w + scale.max(1.0);
                crate::ui::chrome::outline(&mut l, frame.inflate(m), if radius > 0.0 { radius + m } else { 0.0 }, w, rgb(th.marked(), 1.0));
            }

            // The label above the card, left to right: a stack's size, a
            // marked trail's tag, the site's icon and the title; at its right
            // end, on the card under the pointer or the current one, what the
            // blocker stopped and the buttons.
            let bar_h = BAR_H * ui;
            let bar = Rect::new(frame.x, frame.y - BAR_GAP * ui - bar_h, frame.w, bar_h);
            let mid = bar.center().y;
            let small = th.small_radius(ui / th.text_scale);
            let mut lead = 0.0;
            // A stack's size on its top-left corner, readable at any zoom.
            if let (Some((_, len)), Some(pill)) = (stack, self.stack_badge(n)) {
                let size = 11.0 * ui;
                let text = len.to_string();
                let tw = self.renderer.as_mut().map_or(size * 0.6, |r| r.measure(&text, size, true));
                let over = hover == Hit::Badge(n.id);
                crate::ui::chrome::fill(&mut l, pill, if th.rounding > 0.0 { pill.h / 2.0 } else { 0.0 }, rgb(if over { th.accent() } else { th.text() }, 1.0));
                crate::ui::chrome::text(&mut l, text, pill.x + (pill.w - tw) / 2.0, pill.center().y, size, rgb(if over { th.accent_foreground() } else { th.panel() }, 1.0), pill, true);
                lead = pill.right() + 6.0 * ui - frame.x;
            }
            // A marked trail's name before its first card's own, in the
            // trail's colour (just a dot for a colour without a name).
            if let Some(mark) = labels.get(&n.id) {
                let size = 11.0 * ui;
                let pad = 8.0 * ui;
                let h = 20.0 * ui;
                let colour = th.trail(mark.colour.as_deref());
                let name = mark.name.clone().unwrap_or_default();
                let tw = if name.is_empty() { 0.0 } else { self.renderer.as_mut().map_or(100.0, |r| r.measure(&name, size, true)) };
                let w = if name.is_empty() { h } else { (tw + 2.0 * pad).min((frame.w - lead).max(h)) };
                let tag = Rect::new(frame.x + lead, (mid - h / 2.0).round(), w, h);
                let round = if th.rounding > 0.0 || name.is_empty() { h / 2.0 } else { 0.0 };
                let face = if hover == Hit::Tag(n.id) { crate::ui::theme::blend(colour, text_on(colour), 0.12) } else { colour };
                crate::ui::chrome::fill(&mut l, if name.is_empty() { tag.inflate(-4.0 * ui) } else { tag }, round, rgb(face, 1.0));
                self.tags.push((n.id, tag));
                if !name.is_empty() {
                    crate::ui::chrome::text(&mut l, name, tag.x + pad, tag.center().y, size, rgb(text_on(colour), 1.0), tag.inflate(-pad * 0.5), true);
                }
                lead += w + 8.0 * ui;
            }
            // Quiet labels: the title shows on the current card, the one
            // under the pointer, selected ones and stacks, and on every card
            // when zoomed far out; it fades in and out. The label's place
            // stays the card's handle either way.
            let label_on = focused || hovered || selected || stack.is_some() || crowd;
            let label_alpha = self.label_fades.alpha(n.id.0, label_on, now);
            let label_from = (l.quads.len(), l.texts.len());
            if sticky.is_none() && frame.w - lead >= 40.0 * ui {
                self.bars.push((n.id, bar));
                let mut right = bar.right();
                let controls = hovered || focused;
                if controls && frame.w >= BAR_CLOSE_MIN * ui {
                    // Close always; back, forward and reload on a live web
                    // page whose card is wide enough on screen.
                    let btn = BAR_BTN * ui;
                    let mut buttons: Vec<(Hit, crate::ui::icons::Icon, bool)> = vec![(Hit::Close(n.id), crate::ui::icons::Icon::Close, true)];
                    if let Some(c) = card.filter(|c| c.page.kind() == web::PageKind::Web && frame.w >= BAR_NAV_MIN * ui) {
                        buttons.push((Hit::Reload(n.id), crate::ui::icons::Icon::Reload, true));
                        buttons.push((Hit::Forward(n.id), crate::ui::icons::Icon::Forward, c.can_forward));
                        buttons.push((Hit::Back(n.id), crate::ui::icons::Icon::Back, c.can_back));
                    }
                    // The card's awake mode, a button that cycles it, and
                    // while it plays media a speaker that mutes it.
                    // Web pages only: notes and the inspector cost little.
                    let awake_button = !n.is_note() && !n.is_inspector() && !self.inspectors.contains_key(&n.id);
                    if awake_button && frame.w >= BAR_NAV_MIN * ui {
                        buttons.push((Hit::Awake(n.id), Self::awake_icon(n.awake), true));
                        if let Some(c) = card.filter(|c| self.media.get(&n.id).is_some_and(|m| m.playing) || c.page.muted()) {
                            let icon = if c.page.muted() { crate::ui::icons::Icon::SpeakerOff } else { crate::ui::icons::Icon::Speaker };
                            buttons.push((Hit::Mute(n.id), icon, true));
                        }
                    }
                    // A docked inspector: to the other side, or to a card
                    // of its own; an inspector card: back into its page's.
                    if self.docks.contains_key(&n.id) && frame.w >= BAR_NAV_MIN * ui {
                        buttons.push((Hit::Undock(n.id), crate::ui::icons::Icon::Trail, true));
                        buttons.push((Hit::DockSide(n.id), crate::ui::icons::Icon::Sidebar, true));
                    }
                    if self.inspectors.contains_key(&n.id) {
                        buttons.push((Hit::Redock(n.id), crate::ui::icons::Icon::Sidebar, true));
                    }
                    for (hit, icon, enabled) in buttons {
                        let cell = Rect::new(right - btn, bar.y, btn, bar.h);
                        right -= btn;
                        let hot = hover == hit && enabled;
                        if hot {
                            let face = if hit == Hit::Close(n.id) { crate::ui::theme::blend(th.panel(), th.danger(), 0.32) } else { th.active() };
                            crate::ui::chrome::fill(&mut l, cell.inflate(-ui), small, rgb(face, 1.0));
                        }
                        let ink = match (enabled, hot) {
                            (false, _) => rgb(th.text3(), 0.55),
                            (true, true) => rgb(th.text(), 1.0),
                            (true, false) => rgb(th.text2(), 1.0),
                        };
                        // A card kept awake says so in the accent.
                        let ink = match Self::awake_colour(&th, n.awake).filter(|_| hit == Hit::Awake(n.id)) {
                            Some(c) => rgb(c, 1.0),
                            None => ink,
                        };
                        if enabled {
                            self.bar_buttons.push((hit, cell));
                        }
                        if let Some(r) = self.renderer.as_mut() {
                            crate::ui::icons::draw(&mut l, r, icon, cell.center().x, mid, 14.0 * ui, ink);
                        }
                    }
                    right -= 4.0 * ui;
                    // What the blocker stopped on this page (requests blocked
                    // and elements hidden): a shield with the count. Not on
                    // an allowed site.
                    let badge = self
                        .blocking_state(n.id)
                        .filter(|b| b.on)
                        .and_then(|b| scene::privacy::blocked_badge(b.blocked.saturating_add(b.hidden)));
                    if let Some(count) = badge {
                        let size = 11.0 * ui;
                        let tw = self.renderer.as_mut().map_or(size * 1.2, |r| r.measure(&count, size, true));
                        let s = 13.0 * ui;
                        let w = 5.0 * ui + s + 3.0 * ui + tw + 5.0 * ui;
                        // Only while the title keeps some room.
                        if right - w - (frame.x + lead) >= 96.0 * ui {
                            let cell = Rect::new(right - w, bar.y, w, bar.h);
                            self.shields.push((n.id, cell));
                            let over = hover == Hit::Shield(n.id);
                            if over {
                                crate::ui::chrome::fill(&mut l, cell.inflate(-ui), small, rgb(th.active(), 1.0));
                            }
                            let ink = rgb(if over { th.text() } else { th.text2() }, 1.0);
                            if let Some(r) = self.renderer.as_mut() {
                                crate::ui::icons::draw(&mut l, r, crate::ui::icons::Icon::Shield, cell.x + 5.0 * ui + s / 2.0, mid, s, ink);
                            }
                            crate::ui::chrome::text(&mut l, count, cell.x + 5.0 * ui + s + 3.0 * ui, mid, size, ink, cell, true);
                            right = cell.x - 4.0 * ui;
                        }
                    }
                } else if frame.w - lead >= 72.0 * ui {
                    // At the end of the label: a card kept awake shows its
                    // mode in the accent, one playing media a speaker, and a
                    // sleeping card a moon.
                    let mut marks = Vec::new();
                    if let Some(c) = card.filter(|c| self.media.get(&n.id).is_some_and(|m| m.playing) || c.page.muted()) {
                        let icon = if c.page.muted() { crate::ui::icons::Icon::SpeakerOff } else { crate::ui::icons::Icon::Speaker };
                        marks.push((icon, rgb(th.text2(), 1.0)));
                    }
                    if !n.awake.is_auto() {
                        let c = Self::awake_colour(&th, n.awake).unwrap_or_else(|| th.text2());
                        marks.push((Self::awake_icon(n.awake), rgb(c, 1.0)));
                    } else if asleep {
                        marks.push((crate::ui::icons::Icon::Moon, rgb(th.text3(), 1.0)));
                    }
                    for (icon, ink) in marks {
                        if let Some(r) = self.renderer.as_mut() {
                            crate::ui::icons::draw(&mut l, r, icon, right - 8.0 * ui, mid, 13.0 * ui, ink);
                        }
                        right -= 20.0 * ui;
                    }
                    if right < bar.right() {
                        right -= 2.0 * ui;
                    }
                }
                let mut x = frame.x + lead;
                let label_x0 = x;
                let size = 12.0 * ui;
                let icon = self.icon_key(&n.url);
                if let Some(key) = icon {
                    let s = 14.0 * ui;
                    l.quads.push(Quad {
                        shape: Shape::Rect { rect: Rect::new(x.round(), (mid - s / 2.0).round(), s, s), radius: 0.0 },
                        color: if asleep { [0.72, 0.72, 0.74, 1.0] } else { [1.0; 4] },
                        texture: Some(key),
                    });
                    x += s + 6.0 * ui;
                }
                let title = if n.title.trim().is_empty() { n.url.clone() } else { n.title.trim().to_string() };
                let title = match stack {
                    Some((i, len)) => format!("{i}/{len} · {title}"),
                    None => title,
                };
                let ink = match (focused || hovered, asleep) {
                    (true, _) => th.text(),
                    (false, false) => th.text2(),
                    (false, true) => th.text3(),
                };
                let room = (right - 6.0 * ui - x).max(1.0);
                let tw = self.renderer.as_mut().map_or(room, |r| r.measure(&title, size, focused)).min(room);
                crate::ui::chrome::text(&mut l, title, x, mid, size, rgb(ink, 1.0), Rect::new(x, bar.y, room, bar.h), focused);
                // Where the page is from, quietly, after the title of the card
                // in hand: a title can say anything, the address can't.
                let host = palette::host(&n.url);
                if controls && !n.is_note() && scene::icon::site(&n.url).is_some() && !host.is_empty() {
                    let hs = 11.0 * ui;
                    let hw = self.renderer.as_mut().map_or(0.0, |r| r.measure(host, hs, false));
                    let hx = x + tw + 10.0 * ui;
                    if hx + hw <= right - 6.0 * ui {
                        crate::ui::chrome::text(&mut l, host, hx, mid, hs, rgb(th.text3(), 1.0), bar, false);
                    }
                }
                if photo_on {
                    // Over a photo the label sits on a small sheet of the
                    // panel colour, so it reads whatever the picture is.
                    let wide = controls || asleep;
                    let end = if wide { bar.right() } else { (x + tw + 9.0 * ui).min(bar.right()) };
                    let sheet = Rect::new(label_x0 - 7.0 * ui, bar.y + 2.0 * ui, end - label_x0 + 7.0 * ui, bar.h - 4.0 * ui);
                    let round = if th.rounding > 0.0 { sheet.h / 2.0 } else { 0.0 };
                    let mut back = Layer::default();
                    crate::ui::chrome::fill(&mut back, sheet, round, rgb(th.panel(), 0.82));
                    l.quads.splice(label_from.0..label_from.0, back.quads);
                }
            }
            if label_alpha < 1.0 {
                let ease = crate::ui::chrome::ease_out(label_alpha);
                l.quads[label_from.0..].iter_mut().for_each(|q| q.color[3] *= ease);
                l.texts[label_from.1..].iter_mut().for_each(|t| t.color[3] *= ease);
            }
            if let Some((_, a)) = glide.filter(|g| g.1 < 1.0) {
                crate::ui::chrome::fade(&mut l, a, Vec2::new(0.0, 0.0));
            }
            layers.push(l);
        }
        let scene = &self.scene;
        self.label_fades.retain(|id| scene.node(NodeId(id)).is_some());
        self.glides.settle(now);

        // Closed cards shrink away, then their pictures go.
        let mut done = Vec::new();
        for (n, key, at) in &self.ghosts {
            let t = motion::progress(*at, motion::CLOSE, now);
            if t >= 1.0 || board_hidden {
                done.push(*key);
                continue;
            }
            if self.scene.node(n.id).is_some() {
                continue; // back already (undo)
            }
            let (r, a) = motion::closing(self.to_screen_rect(n.rect), t);
            let radius = th.card_corner(k, scale, r.w, r.h);
            let has_tex = self.renderer.as_ref().is_some_and(|rd| rd.has_page(*key));
            let mut l = Layer::default();
            l.quads.push(Quad { shape: Shape::Shadow { rect: r.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (26.0 * k).max(2.0) }, color: th.shadow(0.26 * a), texture: None });
            l.quads.push(Quad {
                shape: Shape::Rect { rect: r, radius },
                color: if has_tex { [1.0, 1.0, 1.0, a] } else { rgb(th.card(false), a) },
                texture: has_tex.then_some(*key),
            });
            layers.push(l);
        }
        if !done.is_empty() {
            let gone: Vec<u64> = self.ghosts.iter().filter(|g| done.contains(&g.1) && self.scene.node(g.0.id).is_none()).map(|g| g.1).collect();
            self.ghosts.retain(|g| !done.contains(&g.1));
            if let Some(rd) = self.renderer.as_mut() {
                for key in gone {
                    rd.remove_page(key);
                }
            }
        }
        // A new space's board slides in from its side as the tint changes.
        if let (Some(t), Some((_, dir, ..))) = (space_t, self.space_in) {
            let shift = Vec2::new(dir * motion::SPACE_SLIDE * ui * (1.0 - t), 0.0);
            for l in &mut layers[board_from..] {
                crate::ui::chrome::fade(l, t, shift);
            }
        }

        if let Some(m) = self.read_morph {
            if let Some(l) = self.read_morph_layer(m) {
                layers.push(l);
            }
        } else if let Some(id) = self.read {
            let l = self.read_layer(id);
            layers.push(l);
        }

        // Over the board's cards, or over the page in Solo.
        if let Some(l) = self.scroll_flash_layer() {
            layers.push(l);
        }
        if let Some(l) = self.link_hint_layer() {
            layers.push(l);
        }
        if let Some(l) = self.card_pick_layer() {
            layers.push(l);
        }
        if let Some(l) = self.join_drag_layer() {
            layers.push(l);
        }

        if let Some(b) = band {
            let r = self.to_screen_rect(b);
            let mut l = Layer::default();
            crate::ui::chrome::fill(&mut l, r, 0.0, rgb(th.marked(), 0.10));
            crate::ui::chrome::outline(&mut l, r, 0.0, line, rgb(th.marked(), 0.9));
            layers.push(l);
        }

        let sidebar_w = if self.sidebar_shown() {
            let (rows, panel) = self.sidebar_layout();
            self.sidebar.scroll = panel.keep_visible(None, self.sidebar.scroll);
            let keys = self.sidebar_has_keys();
            // Icons are looked up before the renderer is borrowed to draw.
            let icons: HashMap<NodeId, u64> = rows
                .iter()
                .filter_map(|r| match r {
                    scene::sidebar::Row::Card { id, .. } => Some(*id),
                    _ => None,
                })
                .filter_map(|id| Some((id, self.icon_key(&self.scene.node(id)?.url)?)))
                .collect();
            let icon = |n: &scene::Node| icons.get(&n.id).copied();
            let chips: Vec<crate::trails::panel::SpaceChip> = self
                .spaces
                .spaces
                .iter()
                .map(|e| crate::trails::panel::SpaceChip { name: e.name.clone(), colour: e.tint.as_deref().map(|t| th.color(t)), private: e.private })
                .collect();
            let space = self.spaces.active_entry().name.clone();
            // What the head shows of the current card.
            let head = self
                .focused
                .and_then(|id| Some((self.scene.node(id)?, self.cards.get(&id))))
                .map(|(n, c)| {
                    let web = c.is_some_and(|c| c.page.kind() == web::PageKind::Web);
                    let addressed = !n.is_note() && n.url != scene::note::START_URL;
                    crate::trails::panel::Current {
                        web,
                        can_back: c.is_some_and(|c| c.can_back),
                        can_forward: c.is_some_and(|c| c.can_forward),
                        host: if addressed { crate::trails::panel::pill_host(&n.url) } else { String::new() },
                        secure: n.url.starts_with("https:"),
                    }
                })
                .unwrap_or_default();
            let marks = crate::trails::panel::View {
                scroll: self.sidebar.scroll,
                sel: if keys { self.sidebar.index(&rows) } else { None },
                hover: self.sidebar_hover,
                spot: self.sidebar_spot,
                head,
                kept: !self.sidebar_peek,
                viewport: vp,
                current: self.focused,
                drop_line: self.sidebar_drop_line(&rows, &panel),
                space: &space,
                spaces: &chips,
                active_space: self.spaces.position(self.spaces.active).unwrap_or(0),
                icon: &icon,
            };
            let came = self.anim.sidebar.update(Some(0), Instant::now());
            if let Some(r) = self.renderer.as_mut() {
                self.sidebar_motion.still = self.still;
                let mut l = crate::trails::panel::layer(r, &panel, &rows, &marks, &self.scene, &th, ui, &mut self.sidebar_motion);
                if !self.still {
                    self.sidebar_last = Some((l.clone(), panel.rect.right()));
                }
                // Slides in from past the window's left edge, settling quickly.
                crate::ui::chrome::fade(&mut l, 0.5 + 0.5 * came, Vec2::new(-panel.rect.right() * 0.6 * (1.0 - came), 0.0));
                layers.push(l);
            }
            self.sidebar_out = None;
            panel.rect.right()
        } else {
            self.anim.sidebar.update(None, Instant::now());
            // Hidden: it slides back out past the edge as it came (not into Solo).
            if self.read.is_some() {
                self.sidebar_last = None;
            }
            if let Some((l, right)) = &self.sidebar_last {
                let at = *self.sidebar_out.get_or_insert(now);
                let t = motion::progress(at, motion::SIDEBAR_OUT, now);
                if t < 1.0 {
                    let e = motion::ease_out(t);
                    let mut l = l.clone();
                    crate::ui::chrome::fade(&mut l, 1.0 - e, Vec2::new(-right * 0.6 * e, 0.0));
                    layers.push(l);
                } else {
                    self.sidebar_last = None;
                    self.sidebar_out = None;
                }
            }
            0.0
        };

        // The board's own control: the sidebar's button and the space. Solo
        // is the page alone, with a way back that shows when the pointer
        // nears the top of the window.
        let chrome_shown = self.read.is_none() && self.read_morph.is_none();
        self.nudge.clear();
        let mut tip_shown = None;
        if chrome_shown {
            let entry = self.spaces.active_entry();
            let (space, private, swatch) = (entry.name.clone(), entry.private, th.swatch(tint.as_deref()));
            let space_w = self.renderer.as_mut().map_or(80.0, |r| r.measure(&space, crate::ui::chrome::type_::BODY * ui, true));
            // The panel can be turned off in the settings (*Show the space panel*).
            // The open sidebar carries the space's name and its own buttons.
            self.hud = if self.settings.space_panel && !self.sidebar_shown() { crate::ui::hud::layout(vp, ui, sidebar_w, space_w) } else { crate::ui::hud::Hud::default() };
            let free = self.menu.is_none() && self.omnibox.is_none() && !self.hints && self.confirm.is_none();
            let hud_hover = self.hud_hover.filter(|_| free && self.pointer_in);
            let view = crate::ui::hud::View {
                space: &space,
                swatch,
                private,
                sidebar_open: self.sidebar_open && !self.sidebar_peek,
                hover: hud_hover.map(|(c, _)| c),
                down: self.hud_down,
                open: self.hud_open.filter(|_| self.menu.is_some()),
            };
            let mid_x = sidebar_w + (vp.x - sidebar_w) / 2.0;
            if let Some(r) = self.renderer.as_mut() {
                layers.push(crate::ui::hud::layer(r, &self.hud, &view, &th, ui));
                // Guidance where the board has nothing to show: an empty
                // board says how to start; a view with no card in it (the
                // board is somewhere else) offers the way back.
                let hot = |rect: Rect| free && rect.contains(self.cursor);
                let empty = self.scene.is_empty() && self.omnibox.is_none() && self.ghosts.is_empty();
                let came = self.anim.welcome.update(empty.then(|| anim_key(entry.id.0)), Instant::now());
                if empty {
                    let wv = crate::ui::hud::WelcomeView {
                        space: &space,
                        swatch,
                        private,
                        on_photo: photo_on,
                        pointer: (free && self.pointer_in && matches!(self.drag, Drag::None)).then_some(self.cursor),
                        down: self.hud_down,
                    };
                    let area = Rect::new(sidebar_w, 0.0, (vp.x - sidebar_w).max(0.0), vp.y);
                    let (mut l, buttons) = crate::ui::hud::welcome(r, &wv, area, &th, ui);
                    crate::ui::chrome::arrive(&mut l, came, 8.0 * ui);
                    layers.push(l);
                    self.nudge = buttons;
                } else if !self.scene.is_empty() && !in_view && !board_hidden && self.camera_goal.is_none() && matches!(self.drag, Drag::None) {
                    let bottom = vp.y - 16.0 * ui;
                    let probe = crate::ui::hud::nudge(r, "No cards in view.", "Fit everything", "Shift+F", mid_x, bottom, false, &th, ui).1;
                    let (l, button) = crate::ui::hud::nudge(r, "No cards in view.", "Fit everything", "Shift+F", mid_x, bottom, hot(probe), &th, ui);
                    layers.push(l);
                    self.nudge = vec![(button, crate::ui::hud::Go::Command(Command::Fit))];
                }
            }
            tip_shown = hud_hover.filter(|(_, since)| !self.hud_down && since.elapsed() >= crate::ui::hud::TIP_DWELL).map(|(c, _)| c);
        }

        let shown = self.omnibox.as_ref().map(|ob| anim_key(std::mem::discriminant(&ob.target)));
        let came = self.anim.omnibox.update(shown, Instant::now());
        if let Some(ob) = &self.omnibox {
            // The lists are centred sheets over everything; a lone box (a
            // name, an address, find) sits clear of an open sidebar.
            let r = if ob.target.is_list() { palette::field_rect(vp, ui) } else { palette::field_rect_from(vp, ui, sidebar_w) };
            use crate::ui::icons::Icon;
            let (icon, placeholder, hint): (Icon, &str, &[(&str, &str)]) = match ob.target {
                Target::RenameSpace => (Icon::Board, "Space name", &[("Enter", "Rename")]),
                Target::NameTrail(_) => (Icon::Trail, "Trail name (empty to clear)", &[("Enter", "Name")]),
                Target::Palette if palette::is_command(&ob.text) => (
                    Icon::Search,
                    "",
                    &[("↑", ""), ("↓", "Earlier"), ("Tab", "Next"), ("Enter", "Run"), ("Ctrl+Enter", "Run, stay"), ("Esc", "Close")],
                ),
                Target::Palette => (
                    Icon::Search,
                    "Find a card, space or command · : for a command line",
                    &[("↑", ""), ("↓", "Move"), ("Enter", "Open"), ("Shift+Enter", "Open beside"), ("Esc", "Close")],
                ),
                Target::Find(_) if self.find_back => (Icon::Find, "Find up the page", &[("Enter", "Previous"), ("Shift+Enter", "Next")]),
                Target::Find(_) => (Icon::Find, "Find in page", &[("Enter", "Next"), ("Shift+Enter", "Previous")]),
                Target::History => (
                    Icon::History,
                    "Search history",
                    &[("Enter", "Open"), ("Shift+Enter", "Open beside"), ("Shift+Del", "Forget"), ("Esc", "Close")],
                ),
                Target::Downloads => (
                    Icon::Download,
                    "Search downloads",
                    &[("Enter", "Open file"), ("Ctrl+Enter", "Show folder"), ("Shift+Del", "Cancel or remove"), ("Esc", "Close")],
                ),
                Target::AllowedSites => (Icon::Shield, "Sites with ads allowed", &[("Enter", "Block ads again"), ("Esc", "Close")]),
                Target::Messages => (
                    Icon::Note,
                    "Search messages",
                    &[("Enter", "Copy · run now"), ("Shift+Del", "Forget · cancel"), ("Esc", "Close")],
                ),
                Target::Photo => (Icon::Picture, "Find a picture or type its path", &[("Enter", "Use as background"), ("Esc", "Close")]),
                Target::SaveAs(_) => (Icon::Download, "Save download as", &[("Enter", "Save"), ("Esc", "Cancel")]),
                Target::Prompt(_) => (Icon::Note, "Your answer", &[("Enter", "OK"), ("Esc", "Cancel")]),
                Target::Files(_) => {
                    use web::dialog::FileMode;
                    let (icon, words) = match self.choosing.as_ref().map(|c| c.mode) {
                        Some(FileMode::Save) => (Icon::Download, "Where to save it"),
                        Some(FileMode::Folder) => (Icon::Board, "Path of a folder"),
                        Some(FileMode::OpenMany) => (Icon::Plus, "Path of a file · * picks several"),
                        _ => (Icon::Plus, "Path of a file"),
                    };
                    (icon, words, &[("Tab", "Complete"), ("Enter", "Choose"), ("Esc", "Cancel")])
                }
                Target::Beside => (Icon::Globe, "Search or enter address", &[("Enter", "Open beside")]),
                Target::Behind => (Icon::Globe, "Search or enter address", &[("Enter", "Open beside, stay here")]),
                _ => (Icon::Globe, "Search or enter address", &[("Enter", "Open"), ("Shift+Enter", "Open beside")]),
            };
            // The list (if this field has one) hangs from the field: one sheet.
            let list = ob.target.is_list().then(|| {
                let rows = self.list_rows(ob);
                let panel = self.list_panel(ob, &rows);
                let scroll = ob.scroll.min(panel.max_scroll());
                (rows, panel, scroll)
            });
            // Find in page: "3 of 17" (or "No matches") at the right end.
            let note = match (&ob.target, self.found) {
                (Target::Find(_), Some((count, active))) if !ob.text.is_empty() => Some(match (count, active) {
                    // Chromium picks an active match only once Enter steps to one.
                    (0, _) => "No matches".to_string(),
                    (1, 0) => "1 match".to_string(),
                    (n, 0) => format!("{n} matches"),
                    (n, a) => format!("{a} of {n}"),
                }),
                // A file chooser says what the page takes.
                (Target::Files(_), _) => self.choosing.as_ref().filter(|c| !c.accept.is_empty()).map(|c| c.accept.join(" ")),
                // An address bar: a keyword names the site it searches (`gh foo`).
                (Target::Focused | Target::NewCard | Target::Beside | Target::Behind, _) => match self.settings.resolve(&ob.text) {
                    // Short: the query is in the field already.
                    Some(scene::search::Resolved::Search { keyword: Some(_), site, .. }) => Some(format!("Search {site}")),
                    _ => None,
                },
                _ => None,
            };
            let field = palette::Field {
                icon,
                text: &ob.text,
                before_caret: ob.before_caret(),
                placeholder,
                selected: ob.select_all && !ob.text.is_empty(),
                note,
                // A lone box says what Enter does at its end; a list says it in its foot.
                hint: if list.is_some() { &[] } else { hint },
            };
            // Lists are modal: the board dims behind them.
            let mut fading: Vec<Layer> = Vec::new();
            if list.is_some() {
                // The veil only fades; the sheet also slides into place.
                layers.push(crate::ui::chrome::scrim(screen, &th, came));
            }
            let mut l = Layer::default();
            let radius = th.radius(scale);
            let sheet = match &list {
                Some((_, panel, _)) => Rect::new(r.x, r.y, r.w, panel.foot.bottom() - r.y),
                None => r,
            };
            crate::ui::chrome::panel(&mut l, sheet, radius, &th, ui, crate::ui::chrome::Lift::High, th.panel());
            if list.is_some() {
                // A hairline between the field and its results.
                crate::ui::chrome::fill(&mut l, Rect::new(r.x + line, r.bottom() - line, r.w - 2.0 * line, line), 0.0, rgb(th.line(), 1.0));
            }
            fading.push(l);
            if let Some(rd) = self.renderer.as_mut() {
                fading.push(palette::field_layer(rd, r, &field, &th, ui));
            }
            if let Some((rows, panel, scroll)) = list {
                let command_empty = palette::is_command(ob.query()).then(|| palette::command_empty(ob.query(), &self.keys.aliases));
                let empty = match (&ob.target, ob.text.trim().is_empty(), &self.page_index) {
                    _ if command_empty.is_some() => command_empty.as_deref().unwrap_or_default(),
                    (Target::History, _, None) => "History is off. Turn it on in the settings.",
                    (Target::History, true, _) => "No pages in history yet",
                    (Target::Downloads, true, _) => "No downloads yet",
                    (Target::AllowedSites, true, _) => "Ads are blocked on every site",
                    (Target::Messages, true, _) => "No messages yet",
                    (Target::Photo, true, _) => "No pictures in Pictures or the theme's backgrounds · type a picture's path",
                    _ => "Nothing found",
                };
                // Site icons are looked up before the renderer is borrowed to draw.
                let icons: Vec<Option<u64>> = rows
                    .iter()
                    .map(|row| {
                        let url = match &row.item {
                            Item::Card(id) => self.scene.all_nodes().find(|n| n.id == *id).filter(|n| !n.is_note()).map(|n| n.url.clone()),
                            Item::Open(url) => Some(url.clone()),
                            _ => None,
                        };
                        url.and_then(|u| self.icon_key(&u))
                    })
                    .collect();
                if let Some(rd) = self.renderer.as_mut() {
                    fading.push(palette::layer(rd, &panel, &rows, &icons, ob.sel, scroll, empty, &th, ui));
                    fading.push(palette::foot_layer(rd, panel.foot, hint, rows.len(), &th, ui));
                }
                if let Some(size) = preview
                    && let Some(at) = panel.preview_rect(vp, size, ui)
                {
                    fading.push(palette::preview_layer(at, PREVIEW_KEY, &th, ui));
                }
            }
            for mut l in fading {
                crate::ui::chrome::arrive(&mut l, came, 6.0 * ui);
                layers.push(l);
            }
        }

        if self.menu.is_none() {
            self.hud_open = None;
        }
        let came = self.anim.menu.update(self.menu.as_ref().map(|m| anim_key((m.rect.x as i32, m.rect.y as i32))), Instant::now());
        if let (Some(m), Some(r)) = (&self.menu, self.renderer.as_mut()) {
            let mut l = menu_layer(r, m, ui, &th);
            crate::ui::chrome::arrive(&mut l, came, 4.0 * ui);
            layers.push(l);
        }

        // Messages sit at the foot of the board, in its middle: a key guide
        // while arranging by keyboard, then whatever was just said.
        let mid_x = sidebar_w + (vp.x - sidebar_w) / 2.0;
        let mut bottom = vp.y - 16.0 * ui;
        if self.nudge.iter().any(|(_, g)| *g == crate::ui::hud::Go::Command(Command::Fit)) {
            bottom -= 52.0 * ui;
        }
        if self.arrange.is_some() && self.omnibox.is_none() {
            let l = self.message(ARRANGE_HELP, mid_x, bottom, false);
            bottom -= 48.0 * ui;
            layers.push(l);
        }
        // A half-typed count or key sequence on the board (`5`), and what
        // can follow it.
        if !self.pending.is_empty() && self.omnibox.is_none() && !self.entered() {
            let typed = self.pending.display();
            let l = self.message(&typed, mid_x, bottom, false);
            bottom -= 48.0 * ui;
            layers.push(l);
            let rows = self.prefix_rows();
            if !rows.is_empty() {
                let (l, h) = self.prefix_panel(&rows, None, mid_x, bottom);
                bottom -= h + 8.0 * ui;
                layers.push(l);
            }
        }
        // An entered card's mode, and what is half-typed in it, in the
        // corner, or (Ring, the default) nothing over the page: its ring and
        // the window title say it (`App::title_text`).
        let chip = self.settings.mode_label == prefs::ModeLabel::Chip;
        if self.entered() && self.omnibox.is_none() && self.link_hints.is_none() {
            if chip && let Some(text) = self.mode_text() {
                let l = self.message(&text, 0.0, vp.y - 16.0 * ui, false);
                layers.push(l);
            }
            // What can follow: by itself with the chip, else on Ctrl+G.
            let asked = self.prefix_asked.is_some() && self.prefix_asked == self.pending.since();
            let rows = if chip || asked { self.prefix_rows() } else { Vec::new() };
            if !rows.is_empty() {
                let bottom = if chip { vp.y - 64.0 * ui } else { vp.y - 16.0 * ui };
                let (l, _) = self.prefix_panel(&rows, Some(16.0 * ui), 0.0, bottom);
                layers.push(l);
            }
        }
        // `;` hints say what a label does, in the same corner.
        if chip && let Some(text) = self.hints_text() {
            let l = self.message(&text, 0.0, vp.y - 16.0 * ui, false);
            layers.push(l);
        }
        // A page's dialog: a sheet where the field goes (under it, for a prompt).
        let asking = self.dialogs.first().filter(|d| {
            d.shown && (self.confirm == Some(Confirm::Dialog) || self.omnibox.as_ref().is_some_and(|ob| ob.target == Target::Prompt(d.id)))
        });
        let came = self.anim.question.update(asking.map(|d| anim_key(d.id)), Instant::now());
        if let Some(d) = asking {
            let (title, message, hints) = Self::dialog_words(d);
            let field = palette::field_rect_from(vp, ui, sidebar_w);
            let top = if d.kind == web::dialog::Kind::Prompt { Rect::new(field.x, field.bottom() + 8.0 * ui, field.w, 0.0) } else { field };
            let radius = th.radius(scale);
            if let Some(rd) = self.renderer.as_mut() {
                let (mut l, _) = palette::question_layer(rd, top, &title, &message, hints, &th, ui, radius);
                crate::ui::chrome::arrive(&mut l, came, 6.0 * ui);
                layers.push(l);
            }
        }
        if let Some((msg, at)) = &self.toast {
            let asks = self.confirm.is_some_and(|c| c != Confirm::Dialog);
            if asks || Instant::now() < *at {
                let msg = msg.clone();
                let warn = asks;
                let came = self.anim.toast.update(Some(anim_key(&msg)), Instant::now());
                let mut l = self.message(&msg, mid_x, bottom, warn);
                // It rises into place.
                crate::ui::chrome::arrive(&mut l, came, -8.0 * ui);
                layers.push(l);
            } else {
                self.toast = None;
            }
        }
        if self.toast.is_none() {
            self.anim.toast.update(None, Instant::now());
        }

        let came = self.anim.sheet.update(self.hints.then_some(0), Instant::now());
        if self.hints
            && let Some(r) = self.renderer.as_mut()
        {
            for mut l in crate::keyboard::key_sheet::layers(r, ui, screen, &th, &mut self.sheet, &self.sheet_sections, self.cursor) {
                crate::ui::chrome::fade(&mut l, came, Vec2::new(0.0, 0.0));
                layers.push(l);
            }
        }

        // The name and key of the control the pointer rests on.
        let came = self.anim.tip.update(tip_shown.map(anim_key), Instant::now());
        if let (Some(c), Some(r)) = (tip_shown, self.renderer.as_mut())
            && let Some(mut l) = crate::ui::hud::tip(r, &self.hud, c, vp, &th, ui)
        {
            crate::ui::chrome::fade(&mut l, came, Vec2::new(0.0, 0.0));
            layers.push(l);
        }

        if let Some(r) = self.renderer.as_mut() {
            r.always_present = self.always_redraw;
            self.frame_changed = r.render(bg, &layers) && r.relaid;
        }
    }

    /// A card growing from its place on the board into Solo, or
    /// shrinking back: the board dims behind it and its corners square off
    /// as it fills the window. The page's pixels keep
    /// their own scale (`read::morph`), so nothing stretches on the way.
    pub(crate) fn read_morph_layer(&mut self, m: ReadMorph) -> Option<Layer> {
        let n = self.scene.node(m.id)?;
        let (header, content) = (self.to_screen_rect(n.header()), self.to_screen_rect(n.rect));
        let card_css = self.card_css(n);
        let p = m.progress();
        let lay = self.read_layout();
        let mo = read::morph(header, content, card_css, &lay, p);
        let th = self.theme.clone();
        let scale = self.scale();
        let vp = self.viewport();
        let k = self.camera.pixels_per_unit(scale);
        let radius = th.card_corner(k, scale, content.w, content.h) * (1.0 - p);
        let tint = self.spaces.active_entry().tint.clone();
        let key = self.page_key(m.id);
        let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
        let extent = self.page_css.get(&m.id).map_or([1.0, 1.0], |&css| read::extent(mo.css, css));
        let frame = Rect::new(mo.bar.x, mo.bar.y, mo.bar.w, mo.content.bottom() - mo.bar.y);
        let mut l = Layer::default();
        // The board recedes behind the page.
        l.quads.push(Quad {
            shape: Shape::Rect { rect: Rect::new(0.0, 0.0, vp.x, vp.y), radius: 0.0 },
            color: rgb(th.board(tint.as_deref()), 0.75 * p),
            texture: None,
        });
        l.quads.push(Quad {
            shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 12.0 * scale)), radius, blur: 36.0 * scale },
            color: th.shadow(0.5 * (1.0 - p * 0.5)),
            texture: None,
        });
        l.quads.push(Quad { shape: Shape::Rect { rect: frame, radius }, color: rgb(th.card(true), 1.0), texture: None });
        l.quads.push(Quad {
            shape: Shape::Page { rect: mo.content, radius, square: compositor::corners::TOP, extent },
            color: if has_tex { [1.0; 4] } else { rgb(th.placeholder(), 1.0) },
            texture: has_tex.then_some(key),
        });
        Some(l)
    }

    /// Card `id`'s docked inspector and the divider before it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dock_layer(&self, l: &mut Layer, id: NodeId, parts: &DockParts, side: dock::Side, radius: f32, focused: bool, hot: bool) {
        let th = &self.theme;
        let scale = self.scale();
        let key = dock_key(self.epoch, id);
        let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
        let (rect, css) = parts.inspector;
        let extent = self.dock_css.get(&id).map_or([1.0, 1.0], |&c| read::extent(css, c));
        let square = match side {
            dock::Side::Right => compositor::corners::LEFT,
            dock::Side::Bottom => compositor::corners::TOP,
        };
        l.quads.push(Quad {
            shape: Shape::Page { rect, radius, square, extent },
            color: if has_tex { [1.0; 4] } else { rgb(th.placeholder(), 1.0) },
            texture: has_tex.then_some(key),
        });
        // The divider: the card's own colour with a line down its middle,
        // the accent while it is under the pointer or dragged.
        let d = parts.divider;
        crate::ui::chrome::fill(l, d, 0.0, rgb(th.card(focused), 1.0));
        let w = if hot { 2.0 * scale } else { scale.max(1.0) };
        let line = match side {
            dock::Side::Right => Rect::new(d.center().x - w / 2.0, d.y, w, d.h),
            dock::Side::Bottom => Rect::new(d.x, d.center().y - w / 2.0, d.w, w),
        };
        crate::ui::chrome::fill(l, line, 0.0, rgb(if hot { th.accent() } else { th.line_strong() }, 1.0));
    }

    /// Solo: card `id`'s page across the whole window.
    pub(crate) fn read_layer(&mut self, id: NodeId) -> Layer {
        let th = self.theme.clone();
        let scale = self.scale();
        let lay = self.read_layout();
        let key = self.page_key(id);
        let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(key));
        let loading = self.cards.get(&id).is_some_and(|c| c.loading);
        let mut l = Layer::default();
        let full = Vec2::new(lay.css.0 as f32, lay.css.1 as f32);
        let parts = self.dock_split(id, lay.content, full);
        let (rect, css) = parts.as_ref().map_or((lay.content, full), |p| p.page);
        let extent = self.page_css.get(&id).map_or([1.0, 1.0], |&c| read::extent(css, c));
        l.quads.push(Quad {
            shape: Shape::Page { rect, radius: 0.0, square: 0, extent },
            color: if has_tex { [1.0; 4] } else { rgb(th.placeholder(), 1.0) },
            texture: has_tex.then_some(key),
        });
        if let (Some(parts), Some(side)) = (&parts, self.docks.get(&id).map(|d| d.side)) {
            let hot = self.hover == Hit::Divider(id) || matches!(self.drag, Drag::Divider { id: d } if d == id);
            self.dock_layer(&mut l, id, parts, side, 0.0, true, hot);
        }
        // No title bar: the page fills the window. A loading page shows a
        // thin moving line across its top edge.
        if loading {
            let c = lay.content;
            let t = (self.started.elapsed().as_secs_f32() * 0.8).fract();
            let w = c.w * 0.25;
            l.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(c.x + (c.w - w) * t, c.y, w, 2.0 * scale.max(1.0)), radius: 0.0 },
                color: rgb(th.accent(), 1.0),
                texture: None,
            });
        }
        l
    }

    /// A message centred on `mid_x` with its bottom at `bottom` (screen px):
    /// a toast, or a question. `warn` gives it a red edge: answering it
    /// loses something.
    /// What can follow a half-typed sequence (`g`, `;`, `m`), once it has
    /// waited [`PREFIX_PANEL_AFTER`]: the next keys and what they do.
    pub(crate) fn prefix_rows(&self) -> Vec<(String, String)> {
        let Some(at) = self.pending.since() else { return Vec::new() };
        if at.elapsed() < PREFIX_PANEL_AFTER && self.prefix_asked != Some(at) {
            return Vec::new();
        }
        let table = if self.visual_mode().is_some() {
            &self.keys.visual
        } else if self.page_mode() {
            &self.keys.page
        } else {
            &self.keys.board
        };
        userkeys::sheet_rows(&table.after(self.pending.keys()), &|a| self.keys.about(a))
    }

    /// A panel of `rows` (keys, what they do) with its bottom at `bottom`:
    /// its left edge at `left`, else centred on `mid_x`.
    pub(crate) fn prefix_panel(&mut self, rows: &[(String, String)], left: Option<f32>, mid_x: f32, bottom: f32) -> (Layer, f32) {
        const MAX_ROWS: usize = 14;
        let th = &self.theme;
        let ui = self.scale() * th.text_scale;
        let size = crate::ui::chrome::type_::BODY * ui;
        let (pad, line, gap) = (14.0 * ui, size * 1.9, 18.0 * ui);
        let vp = self.viewport();
        let more = rows.len().saturating_sub(MAX_ROWS);
        let shown = &rows[..rows.len().min(MAX_ROWS)];
        let measure = |r: &mut Option<Renderer>, t: &str, bold: bool| r.as_mut().map_or(t.chars().count() as f32 * size * 0.6, |r| r.measure(t, size, bold));
        let kw = shown.iter().map(|(k, _)| measure(&mut self.renderer, k, true)).fold(0.0, f32::max);
        let more_text = (more > 0).then(|| format!("and {more} more · ? lists every key"));
        let mut dw = shown.iter().map(|(_, d)| measure(&mut self.renderer, d, false)).fold(0.0, f32::max);
        if let Some(t) = more_text.as_deref() {
            dw = dw.max(measure(&mut self.renderer, t, false));
        }
        let n = shown.len() + usize::from(more > 0);
        let w = (2.0 * pad + kw + gap + dw).min(vp.x - 32.0 * ui).max(80.0);
        let h = 2.0 * pad * 0.6 + line * n as f32;
        let x = left.unwrap_or(mid_x - w / 2.0).clamp(16.0 * ui, (vp.x - w - 16.0 * ui).max(16.0 * ui));
        let r = Rect::new(x.round(), (bottom - h).round(), w.round(), h.round());
        let th = &self.theme;
        let mut l = Layer::default();
        crate::ui::chrome::panel_edged(&mut l, r, th.radius(self.scale()), th, ui, crate::ui::chrome::Lift::High, th.panel(), crate::ui::chrome::edge(th), crate::ui::chrome::hair(ui));
        let top = r.y + pad * 0.6;
        let dx = r.x + pad + kw + gap;
        let dclip = Rect::new(dx, r.y, (r.right() - pad - dx).max(1.0), r.h);
        for (i, (k, d)) in shown.iter().enumerate() {
            let mid = top + line * (i as f32 + 0.5);
            crate::ui::chrome::text(&mut l, k.clone(), r.x + pad, mid, size, rgb(th.text(), 1.0), Rect::new(r.x + pad, r.y, kw + 1.0, r.h), true);
            crate::ui::chrome::text(&mut l, d.clone(), dx, mid, size, rgb(th.text2(), 1.0), dclip, false);
        }
        if let Some(t) = more_text {
            crate::ui::chrome::text(&mut l, t, r.x + pad, top + line * (shown.len() as f32 + 0.5), size, rgb(th.text3(), 1.0), r, false);
        }
        (l, r.h)
    }

    /// An entered card's mode and what is half-typed in it (*Page · section
    /// 2.1 · 40% · 5*), for the corner chip or the window title.
    pub(crate) fn mode_text(&self) -> Option<String> {
        if !self.entered() || self.link_hints.is_some() {
            return None;
        }
        let typed = self.pending.display();
        let mut text = self.page_keys.label().to_string();
        if self.page_keys == PageKeys::Page
            && let Some((id, section, note)) = self.scroll_note.as_ref().filter(|(n, ..)| Some(*n) == self.focused)
        {
            text = format!("{text} · {}", self.section_name(*id, *section));
            if !note.is_empty() {
                text = format!("{text} · {note}");
            }
        }
        if !typed.is_empty() {
            text = format!("{text} · {typed}");
        }
        Some(text)
    }

    /// What `;` hints' labels do (*Hints · copy a link*), while they are up.
    pub(crate) fn hints_text(&self) -> Option<String> {
        let lh = self.link_hints.as_ref().filter(|h| h.hints.is_some())?;
        let kind = lh.kind?;
        if self.omnibox.is_some() {
            return None;
        }
        let mut text = format!("Hints · {}", kind.label());
        if kind.keeps() {
            text = format!("{text} · {} opened · Esc to stop", lh.chosen.len());
        }
        Some(text)
    }

    /// The window's title: the mode (or what hints do) and the entered
    /// card's title, else just the app's name. With nothing drawn over the
    /// page (`mode-label` Ring), this is where the mode is read, in Solo too.
    pub(crate) fn title_text(&self) -> String {
        let card = self.focused.filter(|_| self.entered()).and_then(|id| self.scene.node(id)).map(|n| n.title.trim()).filter(|t| !t.is_empty());
        match (self.hints_text().or_else(|| self.mode_text()), card) {
            (Some(m), Some(t)) => format!("{m} — {t}"),
            (Some(m), None) => format!("{m} — Trailboard"),
            (None, _) => "Trailboard".to_string(),
        }
    }

    /// Sets the window title when it has changed (`App::title_text`).
    pub(crate) fn update_title(&mut self) {
        let want = self.title_text();
        if want != self.title_shown
            && let Some(w) = self.window()
        {
            w.set_title(&want);
            self.title_shown = want;
        }
    }

    pub(crate) fn message(&mut self, text: &str, mid_x: f32, bottom: f32, warn: bool) -> Layer {
        let th = &self.theme;
        let ui = self.scale() * th.text_scale;
        let size = crate::ui::chrome::type_::BODY * ui;
        // The palette's and menus' sheet: same fill, edge, corners and a
        // high lift, so it reads over cards as well as over the board.
        let (pad, h) = (16.0 * ui, 40.0 * ui);
        let vp = self.viewport();
        let max_w = (vp.x - 32.0 * ui).max(80.0);
        let tw = self.renderer.as_mut().map_or(200.0, |r| r.measure(text, size, false));
        let w = (tw + 2.0 * pad).min(max_w);
        let r = Rect::new((mid_x - w / 2.0).clamp(16.0 * ui, (vp.x - w - 16.0 * ui).max(16.0 * ui)).round(), (bottom - h).round(), w.round(), h);
        let mut l = Layer::default();
        let (rim, bw) = if warn { (th.danger(), (1.5 * ui).max(1.0)) } else { (crate::ui::chrome::edge(th), crate::ui::chrome::hair(ui)) };
        crate::ui::chrome::panel_edged(&mut l, r, th.radius(self.scale()), th, ui, crate::ui::chrome::Lift::High, th.panel(), rim, bw);
        let clip = Rect::new(r.x + pad, r.y, (r.w - 2.0 * pad).max(1.0), r.h);
        let shown = self.renderer.as_mut().map_or_else(|| text.to_string(), |rd| rd.ellipsize(text, size, false, clip.w));
        crate::ui::chrome::text(&mut l, shown, r.x + pad, r.center().y, size, rgb(th.text(), 1.0), clip, false);
        l
    }

    /// Sends this frame's part of a touchpad fling to its page.
    pub(crate) fn step_page_scroll(&mut self, dt: f32) {
        let Some(((id, x, y), d)) = self.page_scroll.step(dt) else { return };
        match self.cards.get(&id) {
            Some(card) => card.page.mouse_wheel(x, y, keymap::modifiers(self.mods), d.x as i32, d.y as i32, true),
            // The card closed or fell asleep.
            None => self.page_scroll.stop(),
        }
    }

    /// One iteration of app work outside winit callbacks.
    pub fn tick(&mut self, dt: f32) {
        self.run_script();
        // In tick, not draw: a covered window still says its mode.
        self.update_title();
        self.follow_desktop();
        self.handle_web_events();
        self.check_inspectors();
        // The save field closed (Esc, or something else opened): the download is off.
        if self.save_as.as_ref().is_some_and(|s| self.omnibox.as_ref().is_none_or(|ob| ob.target != Target::SaveAs(s.id))) {
            let s = self.save_as.take().expect("checked");
            if web::download::refuse(s.id) {
                log::info!("download {}: cancelled (no path given)", s.id);
                self.toast("Download cancelled");
            }
            if s.blank {
                self.close_blank_card(s.page);
            }
        }
        // A question dropped by another (a space switch, say) comes back.
        self.tend_dialogs();
        self.tend_editor();
        if let Some(msg) = self.blocking.tick(Instant::now(), self.settings.block_ads) {
            self.toast(msg);
        }
        self.read_due_texts();
        self.run_due_later();
        // Nothing entered: the next card entered starts afresh.
        if !self.entered() && self.page_keys != PageKeys::Page {
            self.page_keys = PageKeys::Page;
            self.end_caret();
        }
        // Caret mode belongs to its card: another card current ends it.
        if self.caret.is_some_and(|c| Some(c.id) != self.focused) && matches!(self.page_keys, PageKeys::Visual(_)) {
            self.set_page_keys(PageKeys::Page);
        }
        // A key sequence left half-typed runs (or is dropped) after a pause.
        let page = self.page_mode();
        let visual = self.visual_mode().is_some();
        let table = if visual { &self.keys.visual } else if page { &self.keys.page } else { &self.keys.board };
        if let Some(Outcome::Run { action, count }) = self.pending.expire(table, Instant::now()) {
            if visual {
                self.visual_action(action, count)
            } else if page {
                self.page_action(action, count)
            } else {
                self.board_action(action, count)
            }
        }
        self.sidebar_peek_tick();
        self.sync_find();
        self.autosave();
        self.step_camera(dt);
        self.step_page_scroll(dt);
        self.step_board_pan(dt);
        if self.hints {
            self.sheet.step(dt);
        }
        self.sync_pages();
        self.sync_ime();
        self.log_perf();
    }

    /// `TRAILBOARD_PERF`: the line for the last second, from `tick` so a
    /// quiet app still logs without drawing.
    fn log_perf(&mut self) {
        let (Some(p), Some(r)) = (self.perf.as_mut(), self.renderer.as_ref()) else { return };
        let (paints, copied) = web::paint_stats();
        let t = crate::dev::perf::Totals {
            paints,
            copied,
            uploaded: r.uploaded,
            presented: r.presented,
            partial: r.partial,
            skipped: r.skipped,
            render: r.render_time,
            copied_px: r.copied_px,
        };
        if let Some(line) = p.tick(Instant::now(), t) {
            log::info!("{line}");
        }
    }

    /// The next time [`App::tick`] has something to look at without being
    /// woken: a field out in an editor, a change to the user's filter list
    /// settling, a stopped video's grace running out, the desktop wallpaper,
    /// the perf line. The loop wakes by then, never later than
    /// [`IDLE_TICK`](super::IDLE_TICK); that alone doesn't draw.
    pub fn next_poll(&self, now: Instant) -> Instant {
        let editing = self.editing.as_ref().map(|_| now + Duration::from_millis(250));
        let media = self.media.values().filter_map(|m| m.stopped).min().map(|t| t + scene::liveness::MEDIA_GRACE + Duration::from_millis(1));
        let candidates = [editing, media, self.wallpaper_due(), Some(self.blocking.deadline()), self.perf.as_ref().map(|p| p.due())];
        // Only times to come: one past (a grace long run out) would spin.
        candidates.into_iter().flatten().filter(|t| *t > now).fold(now + super::IDLE_TICK, Instant::min)
    }

    /// Whether the last frame drawn was laid out anew (something is moving).
    pub fn animating(&self) -> bool {
        self.frame_changed
    }

    /// Whether something asked for a draw outside an event since last asked.
    pub fn take_draw_wanted(&mut self) -> bool {
        std::mem::take(&mut self.draw_wanted)
    }

    /// Whether a window event (input, resize, focus…) came since last asked.
    pub fn take_event(&mut self) -> bool {
        std::mem::take(&mut self.event_seen)
    }

    pub fn request_redraw(&self) {
        if let Some(w) = self.window() {
            w.request_redraw();
        }
    }

    /// The next time something changes without input or a page painting: a
    /// timer that shows or hides something, or one [`App::tick`] polls. The
    /// loop draws by then at most, and never later than [`IDLE_DRAW`]
    /// (it still wakes every [`IDLE_TICK`](super::IDLE_TICK) for the timers `tick` polls).
    pub fn next_deadline(&self, now: Instant) -> Instant {
        let saving = (self.dirty || self.places_dirty).then(|| self.last_save + Duration::from_secs(3) + Duration::from_millis(1));
        let candidates = [
            self.toast.as_ref().map(|(_, at)| *at),
            self.hud_hover.map(|(_, since)| since + crate::ui::hud::TIP_DWELL),
            self.pending.deadline(),
            self.pending.since().map(|at| at + PREFIX_PANEL_AFTER),
            self.peek.deadline(),
            self.desktop.as_ref().and_then(Desktop::deadline),
            self.script.as_ref().and_then(Script::deadline),
            self.text_due.values().min().copied(),
            self.later.iter().map(|l| l.at).min(),
            self.scroll_flash.as_ref().map(|f| f.at + SCROLL_FLASH),
            // The pages are re-rastered sharp once the camera has rested.
            Some(self.camera_changed + Duration::from_millis(251)),
            saving,
        ];
        candidates.into_iter().flatten().filter(|t| *t > now).fold(now + IDLE_DRAW, Instant::min)
    }
}
