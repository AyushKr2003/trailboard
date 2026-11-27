//! Link hints.

use super::*;

impl App {
    /// f: label the links on the current card's page (else the one in the
    /// middle of the view), which becomes current. The labels show once the
    /// page has answered (`got_links`).
    pub(super) fn show_link_hints(&mut self) {
        self.show_link_hints_to(false);
    }

    /// Link hints; `here`: a chosen link loads in the same card (Page mode's
    /// f) instead of opening beside it.
    pub(super) fn show_link_hints_to(&mut self, here: bool) {
        self.ask_link_hints(here, None);
    }

    /// Link hints, of `kind` if given (`;y`, `;d`…): only the targets that
    /// kind can use are labelled.
    pub(super) fn ask_link_hints(&mut self, here: bool, kind: Option<hint::Kind>) {
        let Some(id) = self.current() else {
            self.toast("No page here to follow links on");
            return;
        };
        self.selection.clear();
        self.focus(Some(id)); // wakes it if asleep
        self.ensure_visible(id);
        let Some(card) = self.cards.get(&id) else {
            self.toast("No page here to follow links on");
            return;
        };
        use web::links::Filter;
        let filter = match kind {
            None => Filter::All,
            Some(
                hint::Kind::Yank | hint::Kind::Background | hint::Kind::Many | hint::Kind::Run(_) | hint::Kind::Edit | hint::Kind::Primary,
            ) => Filter::Links,
            Some(hint::Kind::Download) => Filter::Downloads,
            Some(hint::Kind::Image | hint::Kind::ImageHere) => Filter::Images,
            Some(hint::Kind::Field) => Filter::Fields,
            Some(hint::Kind::Hover) => Filter::All,
            Some(hint::Kind::Click) => Filter::Clickable,
        };
        card.page.request_links(filter);
        self.link_hints = Some(LinkHints { id, here, kind, chosen: Vec::new(), line: None, links: Vec::new(), sections: Vec::new(), hints: None });
        log::info!("link hints asked of card {}{}", id.0, kind.map(|k| format!(" (;{})", k.key())).unwrap_or_default());
    }

    /// The page of card `id` answered with its links (`width`: its layout
    /// width in CSS px). Ignored unless hints were asked of that card.
    pub(super) fn got_links(&mut self, id: NodeId, width: f32, links: Vec<web::links::Link>) {
        // The page may be zoomed: its CSS px are not the view's.
        let Some((_, css)) = self.page_view(id) else { return };
        let Some(lh) = self.link_hints.as_mut().filter(|h| h.id == id && h.hints.is_none()) else {
            log::debug!("links from card {} not asked for", id.0);
            return;
        };
        let k = css.x / width.max(1.0);
        lh.links = links
            .into_iter()
            .map(|l| (Rect::new(l.rect[0] * k, l.rect[1] * k, l.rect[2] * k, l.rect[3] * k), l.action))
            .collect();
        if lh.links.is_empty() {
            let kind = lh.kind;
            self.link_hints = None;
            let loading = self.cards.get(&id).is_some_and(|c| c.loading);
            log::info!("link hints on card {}: no links in view", id.0);
            let what = match kind {
                None | Some(hint::Kind::Click | hint::Kind::Hover) => "Nothing to click",
                Some(
                    hint::Kind::Yank | hint::Kind::Background | hint::Kind::Many | hint::Kind::Run(_) | hint::Kind::Edit | hint::Kind::Primary,
                ) => "No links",
                Some(hint::Kind::Download) => "No links or images",
                Some(hint::Kind::Image | hint::Kind::ImageHere) => "No images",
                Some(hint::Kind::Field) => "No text fields",
            };
            self.toast(if loading { format!("{what} yet · the page is still loading") } else { format!("{what} in view") });
            return;
        }
        let h = hint::Hints::new(lh.links.len());
        log::info!(
            "link hints on card {}: {} links, {}",
            id.0,
            lh.links.len(),
            h.labels
                .iter()
                .zip(&lh.links)
                .take(8)
                .map(|(l, (_, a))| match a {
                    web::links::Action::Open(u) => format!("{l}={u}"),
                    web::links::Action::Image(u) => format!("{l}=image {u}"),
                    web::links::Action::Click => format!("{l}=click"),
                    web::links::Action::Type => format!("{l}=type"),
                })
                .collect::<Vec<_>>()
                .join(" ")
        );
        lh.hints = Some(h);
    }

    /// A key while link hints are up. `chord`: Ctrl or Alt is held.
    pub(super) fn link_hint_key(&mut self, ev: &KeyIn, chord: bool) {
        let Some(lh) = self.link_hints.as_mut() else { return };
        let letter = ev.text.as_deref().and_then(|t| {
            let mut cs = t.chars();
            cs.next().filter(|c| cs.next().is_none() && c.is_alphabetic())
        });
        match (ev.named, letter, lh.hints.as_mut()) {
            (Some(NamedKey::Backspace), _, Some(h)) if !chord => {
                h.backspace();
            }
            (_, Some(c), Some(h)) if !chord => match h.type_char(c) {
                hint::Typed::Follow(i) if !lh.sections.is_empty() => {
                    let (id, (section, _)) = (lh.id, lh.sections[i]);
                    self.link_hints = None;
                    self.set_scroll_target(id, section, true);
                }
                hint::Typed::Follow(i) if lh.kind.is_some_and(hint::Kind::keeps) => {
                    h.restart();
                    if !lh.chosen.contains(&i) {
                        lh.chosen.push(i);
                    }
                    let (id, action, line) = (lh.id, lh.links[i].1.clone(), lh.line.clone());
                    if let web::links::Action::Open(url) | web::links::Action::Image(url) = action {
                        match line {
                            Some(line) => self.run_hint_line(&line, &url),
                            None => self.open_behind(id, &url),
                        }
                    }
                }
                hint::Typed::Follow(i) => {
                    let (id, here, kind, (rect, action), line) = (lh.id, lh.here, lh.kind, lh.links[i].clone(), lh.line.clone());
                    self.link_hints = None;
                    match kind {
                        Some(hint::Kind::Run(_)) => {
                            if let (Some(line), web::links::Action::Open(url) | web::links::Action::Image(url)) = (line, action) {
                                self.run_hint_line(&line, &url);
                            }
                        }
                        Some(k) => self.use_hint(id, k, rect, action),
                        None => match action {
                            web::links::Action::Open(url) if here => self.follow_here(id, &url),
                            web::links::Action::Open(url) | web::links::Action::Image(url) => self.follow_link(id, &url),
                            web::links::Action::Click => self.click_target(id, rect, false),
                            web::links::Action::Type => self.click_target(id, rect, true),
                        },
                    }
                }
                hint::Typed::Narrowed => {}
                hint::Typed::NoMatch => log::debug!("no link hint starts with {c:?}"),
            },
            // Still waiting: letters wait too, anything else gives up.
            (_, Some(_), None) if !chord => {}
            _ => {
                self.link_hints = None;
                log::info!("link hints cancelled");
            }
        }
    }

    /// A label of a `;` hint kind was chosen on card `id`'s page.
    pub(super) fn use_hint(&mut self, id: NodeId, kind: hint::Kind, rect: Rect, action: web::links::Action) {
        use web::links::Action as A;
        let url = match &action {
            A::Open(u) | A::Image(u) => Some(u.clone()),
            A::Click | A::Type => None,
        };
        match (kind, url) {
            (hint::Kind::Yank, Some(url)) => {
                self.clipboard.set_text(&url);
                log::info!("hint: copied {url}");
                self.toast(if matches!(action, A::Image(_)) { "Copied the image's address" } else { "Copied the link's address" });
            }
            (hint::Kind::Download, Some(url)) => {
                log::info!("hint: download {url}");
                if let Some(c) = self.cards.get(&id) {
                    c.page.download(&url);
                }
            }
            (hint::Kind::Image, Some(url)) => self.follow_link(id, &url),
            (hint::Kind::Background | hint::Kind::Many, Some(url)) => {
                self.open_behind(id, &url);
                self.toast("Opened beside · still here");
            }
            (hint::Kind::Field, _) => self.click_target(id, rect, true),
            (hint::Kind::Primary, Some(url)) => {
                self.clipboard.set_primary(&url);
                log::info!("hint: {url} to the primary selection");
                self.toast("Put the link's address in the primary selection");
            }
            (hint::Kind::Edit, Some(url)) => {
                log::info!("hint: editing {url} in :open");
                self.open_palette();
                self.fill_command_line(&format!("open {url}"));
            }
            (hint::Kind::ImageHere, Some(url)) => self.follow_here(id, &url),
            (hint::Kind::Hover, _) => self.hover_target(id, rect),
            (hint::Kind::Click, _) => self.click_target(id, rect, matches!(action, A::Type)),
            (hint::Kind::Run(_), _) => log::debug!("hint: a :hint line runs from link_hint_key"),
            (_, None) => log::debug!("hint kind {kind:?} on a target without an address"),
        }
    }

    /// Open `url` beside card `from` without going there: the current card,
    /// the view and where the keys are stay as they were.
    pub(super) fn open_behind(&mut self, from: NodeId, url: &str) {
        if self.scene.node(from).is_none() {
            return;
        }
        log::info!("opened a link hint from card {} beside it, staying: {url}", from.0);
        self.open_card(Rect::default(), url, Some(from));
    }

    /// A hint on a button, field or other control: click its middle, as the
    /// mouse would. A field (`typing`) gets the keys, so you can type in it at once.
    pub(super) fn click_target(&mut self, id: NodeId, rect: Rect, typing: bool) {
        if typing {
            self.focus(Some(id));
            self.give_keys(PageKeys::Insert);
        }
        let Some(card) = self.cards.get(&id) else { return };
        if typing {
            card.page.set_focus(true);
        }
        let c = rect.center();
        let (x, y) = (c.x.round() as i32, c.y.round() as i32);
        card.page.mouse_move(x, y, 0, false);
        card.page.mouse_button(x, y, 0, WebButton::Left, false, 1);
        card.page.mouse_button(x, y, 0, WebButton::Left, true, 1);
        log::info!("clicked a hint on card {} at {x},{y}{}", id.0, if typing { ", keys to the page" } else { "" });
        // What was clicked is where the reader is: scroll there next.
        self.pick_section_at(id, x, y);
    }

    /// `;h`: the pointer over a target's middle, as the mouse would (menus
    /// that open on hover); nothing is clicked. It stays until the real
    /// pointer moves over the page.
    pub(super) fn hover_target(&mut self, id: NodeId, rect: Rect) {
        let Some(card) = self.cards.get(&id) else { return };
        let c = rect.center();
        let (x, y) = (c.x.round() as i32, c.y.round() as i32);
        card.page.mouse_move(x, y, 0, false);
        log::info!("hovered a hint on card {} at {x},{y}", id.0);
    }

    /// v: add the current card to the selection, or take it out. Going to
    /// another card keeps the selection, so several can be picked by keyboard.
    pub(super) fn toggle_mark(&mut self) {
        let Some(id) = self.current() else { return };
        self.focus(Some(id));
        self.ensure_visible(id);
        self.selection.toggle(id);
        let n = self.selection.ids().len();
        log::info!("selection: {n} card(s)");
        self.toast(match n {
            0 => "Selection cleared".to_string(),
            1 => "1 card selected".to_string(),
            n => format!("{n} cards selected"),
        });
    }

    /// z: the current card fills the view, as a double-click on its title bar does.
    pub(super) fn fill_current(&mut self) {
        if let Some(id) = self.current() {
            self.focus(Some(id));
            self.fill_view(id);
        }
    }

    /// d / u: scroll the current card's page half its height, as the wheel
    /// would over its middle, without giving it the keys.
    pub(super) fn scroll_current(&mut self, down: bool) {
        let Some(id) = self.current() else { return };
        self.focus(Some(id));
        self.ensure_visible(id);
        self.scroll_section(id, web::scroll::Axis::Y, if down { 1 } else { -1 }, web::scroll::Unit::Half);
    }

    /// Menu key or Shift+F10: the page's own menu, as a right-click gives, at
    /// the page's selection or focused field. Chromium works out where and
    /// what (link, selection, field) and asks for our menu as usual.
    pub(super) fn page_menu_by_key(&mut self, ev: &KeyIn) {
        let Some(id) = self.current() else {
            self.toast("No page here for a menu");
            return;
        };
        self.focus(Some(id));
        self.ensure_visible(id);
        let Some(card) = self.cards.get(&id) else { return };
        card.page.send_key(&ev.to_web(self.mods));
        self.menu_by_key = Some(Instant::now());
        log::info!("page menu asked of card {} by key", id.0);
    }

    /// Open `url` beside card `from`, as a click on the link would, and make
    /// the new card current, so the next f follows on from it.
    /// Page mode's f: a web page loads the link itself; a note or one of our
    /// own pages never navigates, so the link opens beside it as usual.
    pub(super) fn follow_here(&mut self, id: NodeId, url: &str) {
        match self.cards.get(&id).map(|c| &c.page) {
            Some(p) if p.kind() == web::PageKind::Web => {
                log::info!("followed a link hint in card {} to {url}", id.0);
                p.load(url);
            }
            _ => self.follow_link(id, url),
        }
    }

    pub(super) fn follow_link(&mut self, from: NodeId, url: &str) {
        if self.scene.node(from).is_none() {
            return;
        }
        log::info!("followed a link hint on card {} to {url}", from.0);
        if let Some(id) = self.open_card(Rect::default(), url, Some(from)) {
            self.focus(Some(id));
            self.ensure_visible(id);
        }
    }

    /// The labels of the link hints, over their links on screen.
    pub(super) fn link_hint_layer(&mut self) -> Option<Layer> {
        let lh = self.link_hints.as_ref()?;
        let h = lh.hints.as_ref()?;
        let (view, _) = self.page_view(lh.id)?;
        let th = &self.theme;
        let ui = self.ui();
        let scale = self.scale();
        let size = 11.5 * ui;
        let (pad, bh) = (size * 0.4, size * 1.45);
        let typed_n = h.typed.chars().count();
        let (bg, fg, done) = (rgb(th.accent(), 1.0), rgb(th.accent_foreground(), 1.0), rgb(th.accent_foreground(), 0.45));
        let border = rgb(th.background(), 0.9);
        let radius = th.small_radius(scale);
        let mut items = Vec::new();
        let mut l = Layer::default();
        // Section labels (`s`) sit at their sections' top-left corners, each
        // section outlined; link labels at the middle of their link's start.
        let spots = if lh.sections.is_empty() {
            None
        } else {
            let rects: Vec<Rect> = lh.sections.iter().filter_map(|(_, r)| self.page_rect_to_screen(lh.id, *r)).collect();
            let longest = h.labels.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
            let label = Vec2::new(longest * size * 0.75 + pad * 2.0, bh);
            for (i, r) in rects.iter().enumerate() {
                if h.shown(i) {
                    crate::ui::chrome::outline(&mut l, r.inflate(-scale), radius, (1.5 * ui).max(1.0), rgb(th.accent(), 0.55));
                }
            }
            (rects.len() == lh.sections.len()).then(|| scrolltarget::label_spots(&rects, label, view))
        };
        let count = if lh.sections.is_empty() { lh.links.len() } else { lh.sections.len() };
        for i in 0..count {
            if !h.shown(i) {
                continue;
            }
            let at = match &spots {
                Some(s) => Rect::new(s[i].x, s[i].y, 0.0, bh),
                None => {
                    let rect = if lh.sections.is_empty() { lh.links[i].0 } else { continue };
                    let Some(at) = self.page_rect_to_screen(lh.id, rect) else { continue };
                    at
                }
            };
            let label = h.labels[i].to_uppercase();
            let (head, tail): (String, String) = (label.chars().take(typed_n).collect(), label.chars().skip(typed_n).collect());
            items.push((at, head, tail, lh.chosen.contains(&i)));
        }
        let r = self.renderer.as_mut()?;
        for (at, head, tail, chosen) in items {
            // A label already chosen (`;m`) is quieter, but can be chosen again.
            let (bg, fg) = if chosen { (rgb(th.accent(), 0.45), rgb(th.accent_foreground(), 0.7)) } else { (bg, fg) };
            let head_w = r.measure(&head, size, true);
            let w = head_w + r.measure(&tail, size, true) + pad * 2.0;
            // At the link's start, kept inside the page.
            let x = at.x.max(view.x).min(view.right() - w).max(view.x);
            let y = (at.y + (at.h - bh) / 2.0).max(view.y).min(view.bottom() - bh).max(view.y);
            let b = Rect::new(x, y, w, bh);
            l.quads.push(Quad { shape: Shape::Rect { rect: b.inflate(scale.max(1.0)), radius }, color: border, texture: None });
            l.quads.push(Quad { shape: Shape::Rect { rect: b, radius }, color: bg, texture: None });
            let ty = y + (bh - size * 1.3) / 2.0;
            if !head.is_empty() {
                l.texts.push(Text { text: head, pos: [x + pad, ty], size, color: done, clip: view, bold: true });
            }
            l.texts.push(Text { text: tail, pos: [x + pad + head_w, ty], size, color: fg, clip: view, bold: true });
        }
        Some(l)
    }

    /// The brief outline around a newly chosen or scrolled section, fading.
    pub(super) fn scroll_flash_layer(&mut self) -> Option<Layer> {
        let f = self.scroll_flash.as_ref()?;
        let age = f.at.elapsed();
        if age > SCROLL_FLASH || self.focused != Some(f.id) {
            return None;
        }
        let (view, _) = self.page_view(f.id)?;
        let at = self.page_rect_to_screen(f.id, f.rect)?;
        // Inside the page's view, so the page's own outline stays visible.
        let x0 = at.x.max(view.x);
        let y0 = at.y.max(view.y);
        let rect = Rect::new(x0, y0, at.right().min(view.right()) - x0, at.bottom().min(view.bottom()) - y0);
        if rect.w < 4.0 || rect.h < 4.0 {
            return None;
        }
        let t = age.as_secs_f32() / SCROLL_FLASH.as_secs_f32();
        let alpha = if self.still { 0.9 } else { 0.9 * (1.0 - t * t) };
        let ui = self.ui();
        let mut l = Layer::default();
        let radius = self.theme.small_radius(self.scale());
        crate::ui::chrome::outline(&mut l, rect, radius, (2.0 * ui).max(1.5), rgb(self.theme.accent(), alpha));
        Some(l)
    }
}
