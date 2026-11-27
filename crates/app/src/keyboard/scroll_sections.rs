//! Scroll sections.

use std::time::Instant;

use scene::{hint, NodeId, Rect, scrolltarget};

use crate::app::{App, LinkHints, PageKeys, SCROLL_FLASH};
use crate::keyboard::jumps::SectionAsk;

/// A brief outline around the section a key just chose or scrolled.
pub(crate) struct ScrollFlash {
    pub(crate) id: NodeId,
    pub(crate) section: u32,
    /// In the view's CSS px.
    pub(crate) rect: Rect,
    pub(crate) at: Instant,
}

impl App {
    /// Scrolls card `id`'s scroll target (else the automatic pick) `n` steps
    /// (positive: down / right), going on to the section around it at its
    /// edge. The page answers with what moved (`got_scroll`).
    pub(crate) fn scroll_section(&mut self, id: NodeId, axis: web::scroll::Axis, n: i32, unit: web::scroll::Unit) {
        let Some(card) = self.cards.get(&id) else { return };
        let target = self.scroll_targets.get(&id).copied();
        card.page.scroll_command(&web::scroll::by(target, axis, n, unit, !self.still));
        self.link_hints = None; // scrolling moves the targets
    }

    /// gg / G / 0 / $: the scroll target to an edge, at once.
    pub(crate) fn scroll_edge(&mut self, id: NodeId, edge: web::scroll::Edge) {
        let Some(card) = self.cards.get(&id) else { return };
        let target = self.scroll_targets.get(&id).copied();
        card.page.scroll_command(&web::scroll::to(target, edge));
        self.link_hints = None;
    }

    /// Ask card `id`'s page for its sections in view; `got_scroll` acts on
    /// the answer as `why` says.
    pub(crate) fn ask_sections(&mut self, id: NodeId, why: SectionAsk) {
        let Some(card) = self.cards.get(&id) else { return };
        card.page.scroll_command(&web::scroll::list());
        self.section_ask = Some((id, why));
        log::debug!("sections asked of card {} ({why:?})", id.0);
    }

    /// A click (or a hint's click) at `(x, y)` in card `id`'s page (CSS px
    /// of the view): the innermost section there becomes the scroll target.
    pub(crate) fn pick_section_at(&mut self, id: NodeId, x: i32, y: i32) {
        let Some((_, css)) = self.page_view(id) else { return };
        let Some(card) = self.cards.get(&id) else { return };
        card.page.scroll_command(&web::scroll::pick(x as f32, y as f32, css.x));
    }

    /// Choose section `section` as card `id`'s scroll target (the page's
    /// own number goes back to the automatic pick, as after a navigation,
    /// except when chosen on purpose with `keep_page`).
    pub(crate) fn set_scroll_target(&mut self, id: NodeId, section: u32, keep_page: bool) {
        if section == web::scroll::PAGE && !keep_page {
            self.scroll_targets.remove(&id);
        } else {
            self.scroll_targets.insert(id, section);
        }
        log::info!("scroll target of card {}: {} (#{section})", id.0, self.section_name(id, section));
        let rect = self.sections.get(&id).and_then(|v| v.iter().find(|s| s.id == section)).map(|s| s.rect);
        if let Some(rect) = rect {
            self.scroll_flash = Some(ScrollFlash { id, section, rect, at: Instant::now() });
        }
        self.scroll_note = Some((id, section, String::new()));
    }

    /// The pill's name for a section of card `id`: `page`, `section 2.1`.
    pub(crate) fn section_name(&self, id: NodeId, section: u32) -> String {
        if section == web::scroll::PAGE {
            return "page".into();
        }
        let named = self.sections.get(&id).and_then(|v| scrolltarget::names(v).remove(&section));
        match named {
            Some(n) if n != "page" => format!("section {n}"),
            _ => "section".into(),
        }
    }

    /// A page forgets its sections when it navigates (or closes): its
    /// scroll target goes back to the automatic pick.
    pub(crate) fn forget_sections(&mut self, id: NodeId) {
        self.scroll_targets.remove(&id);
        self.sections.remove(&id);
        if self.scroll_note.as_ref().is_some_and(|(n, ..)| *n == id) {
            self.scroll_note = None;
        }
        if self.scroll_flash.as_ref().is_some_and(|f| f.id == id) {
            self.scroll_flash = None;
        }
        if self.section_ask.is_some_and(|(n, _)| n == id) {
            self.section_ask = None;
        }
    }

    /// An answer from card `id`'s page about its scroll sections.
    pub(crate) fn got_scroll(&mut self, id: NodeId, report: web::scroll::Report) {
        use web::scroll::{Note, Report};
        if let Report::Spot { path, x, y } = report {
            self.got_spot(id, path, x, y);
            return;
        }
        let Some((_, css)) = self.page_view(id) else { return };
        let scaled = |width: f32, r: [f32; 4]| {
            let k = css.x / width.max(1.0);
            Rect::new(r[0] * k, r[1] * k, r[2] * k, r[3] * k)
        };
        match report {
            Report::List { width, auto, sections } => {
                let Some((_, why)) = self.section_ask.filter(|(n, _)| *n == id) else {
                    log::debug!("sections from card {} not asked for", id.0);
                    return;
                };
                self.section_ask = None;
                let list: Vec<scrolltarget::Section> = sections
                    .iter()
                    .map(|s| scrolltarget::Section { id: s.id, parent: s.parent, rect: scaled(width, s.rect) })
                    .collect();
                log::info!(
                    "sections of card {}: {} ({})",
                    id.0,
                    list.len(),
                    sections.iter().map(|s| format!("#{}<{}", s.id, s.parent.map_or(-1, i64::from))).collect::<Vec<_>>().join(" ")
                );
                self.sections.insert(id, list.clone());
                // Where stepping starts: the chosen target if it is still in
                // view, else what the automatic pick would scroll now.
                let current = self.scroll_targets.get(&id).copied().filter(|t| list.iter().any(|s| s.id == *t)).unwrap_or(auto);
                match why {
                    SectionAsk::Label => self.label_sections(id, &list),
                    SectionAsk::Step(forward) => {
                        if list.len() < 2 {
                            self.toast("Only the page scrolls here");
                        }
                        if let Some(next) = scrolltarget::step(&list, Some(current), forward) {
                            self.set_scroll_target(id, next, true);
                        }
                    }
                    SectionAsk::Out => match scrolltarget::outer(&list, current) {
                        Some(o) => self.set_scroll_target(id, o, true),
                        None => {
                            self.set_scroll_target(id, current, true);
                            self.toast("This is the whole page");
                        }
                    },
                    // The pill names sections from the list just kept.
                    SectionAsk::Name => {}
                }
            }
            Report::Spot { .. } => {}
            Report::Did { width, scrolled, rect, pct, note, reset } => {
                if self.focused != Some(id) {
                    return;
                }
                if reset {
                    log::info!("scroll target of card {} is gone; automatic again", id.0);
                    self.scroll_targets.remove(&id);
                }
                let rect = scaled(width, rect);
                let known = scrolled == web::scroll::PAGE || self.sections.get(&id).is_some_and(|v| v.iter().any(|s| s.id == scrolled));
                let text = match note {
                    Note::NoSideways => {
                        self.toast("Nothing here scrolls sideways");
                        String::new()
                    }
                    Note::Shown => "back in view".to_string(),
                    Note::Moved | Note::Edge => format!("{pct}%"),
                };
                log::info!("scrolled card {}: {} · {text} ({note:?})", id.0, self.section_name(id, scrolled));
                // Outline what moved when it isn't what moved last (the
                // target changed, or the scroll went on to an outer section).
                let fresh = self.scroll_flash.as_ref().is_none_or(|f| f.id != id || f.section != scrolled || f.at.elapsed() > SCROLL_FLASH);
                let changed = self.scroll_flash.as_ref().is_none_or(|f| f.id != id || f.section != scrolled);
                if changed || (fresh && note == Note::Shown) {
                    self.scroll_flash = Some(ScrollFlash { id, section: scrolled, rect, at: Instant::now() });
                } else if let Some(f) = self.scroll_flash.as_mut() {
                    f.rect = rect;
                }
                self.scroll_note = Some((id, scrolled, text));
                if !known && self.section_ask.is_none() {
                    self.ask_sections(id, SectionAsk::Name);
                }
            }
            Report::Wheel { width, at, delta, jump } => {
                // Only the card whose keys asked; and a wheel can only scroll
                // the page that asked for it.
                if self.focused != Some(id) {
                    return;
                }
                let k = css.x / width.max(1.0);
                let Some(card) = self.cards.get(&id) else { return };
                let (x, y) = ((at.0 * k).round() as i32, (at.1 * k).round() as i32);
                let (dx, dy) = ((delta.0 as f32 * k).round() as i32, (delta.1 as f32 * k).round() as i32);
                log::info!("scrolled card {} by wheel at {x},{y} ({dx},{dy})", id.0);
                card.page.mouse_wheel(x, y, 0, dx, dy, jump);
            }
            Report::Picked { id: section } => {
                if self.focused != Some(id) {
                    return;
                }
                if section == web::scroll::PAGE {
                    self.scroll_targets.remove(&id);
                } else {
                    self.scroll_targets.insert(id, section);
                }
                log::info!("scroll target of card {} by click: #{section}", id.0);
                if self.scroll_note.as_ref().is_some_and(|(n, ..)| *n == id) {
                    self.scroll_note = None;
                }
            }
        }
    }

    /// `s`: labels on card `id`'s sections in view; typing one picks it.
    pub(crate) fn label_sections(&mut self, id: NodeId, list: &[scrolltarget::Section]) {
        let order = scrolltarget::order(list);
        let sections: Vec<(u32, Rect)> = order
            .iter()
            .filter_map(|o| list.iter().find(|s| s.id == *o))
            .map(|s| (s.id, s.rect))
            .collect();
        if sections.len() < 2 {
            self.toast("Only the page scrolls here");
            if let Some(&(only, _)) = sections.first() {
                self.set_scroll_target(id, only, false);
            }
            return;
        }
        let hints = hint::Hints::new(sections.len());
        log::info!(
            "section labels on card {}: {}",
            id.0,
            hints.labels.iter().zip(&sections).map(|(l, (s, _))| format!("{l}=#{s}")).collect::<Vec<_>>().join(" ")
        );
        self.link_hints = Some(LinkHints { id, here: true, kind: None, chosen: Vec::new(), line: None, links: Vec::new(), sections, hints: Some(hints) });
    }

    /// Where the entered page's focus went: a field the user put the caret
    /// in switches to Insert; leaving a field (or a click outside one)
    /// switches back to Page. A page focusing a field on its own as it loads
    /// changes nothing. Only the entered card is listened to.
    pub(crate) fn page_focus(&mut self, id: NodeId, report: web::focus::Report) {
        if self.focused != Some(id) || !self.entered() || self.page_keys == PageKeys::Passthrough {
            return;
        }
        match report {
            web::focus::Report::Field { editable: true, user: true } if matches!(self.page_keys, PageKeys::Page | PageKeys::Visual(_)) => {
                self.set_page_keys(PageKeys::Insert);
            }
            web::focus::Report::Field { editable: false, .. } if self.page_keys == PageKeys::Insert => {
                // A note or the start page types without a field we can see.
                if self.entry_mode(id) == PageKeys::Page {
                    self.set_page_keys(PageKeys::Page);
                }
            }
            web::focus::Report::NoField => self.toast("No text field here"),
            _ => {}
        }
    }
}
