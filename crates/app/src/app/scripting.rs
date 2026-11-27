//! The end-to-end script runner (`TRAILBOARD_SCRIPT`).

use super::*;

impl App {
    pub(super) fn run_script(&mut self) {
        if self.renderer.is_none() {
            return;
        }
        while let Some(cmd) = self.script.as_mut().and_then(Script::next) {
            log::info!("script: {cmd:?}");
            match cmd {
                Cmd::Wait(_) => {}
                Cmd::Move(x, y) => self.cursor_moved(Vec2::new(x, y)),
                Cmd::Click(x, y, shift) => {
                    let saved = self.mods;
                    if shift {
                        self.mods = ModifiersState::SHIFT;
                    }
                    self.cursor_moved(Vec2::new(x, y));
                    self.mouse_input(ElementState::Pressed, MouseButton::Left);
                    self.mouse_input(ElementState::Released, MouseButton::Left);
                    self.mods = saved;
                }
                Cmd::RightClick(x, y) => {
                    self.cursor_moved(Vec2::new(x, y));
                    self.mouse_input(ElementState::Pressed, MouseButton::Right);
                    self.mouse_input(ElementState::Released, MouseButton::Right);
                }
                Cmd::PageClick { id, x, y, right } => {
                    let Some(at) = self.page_rect_to_screen(NodeId(id), Rect::new(x, y, 0.0, 0.0)) else {
                        log::warn!("script: no card {id}");
                        continue;
                    };
                    let p = Vec2::new(at.x, at.y);
                    let button = if right { MouseButton::Right } else { MouseButton::Left };
                    self.cursor_moved(p);
                    self.mouse_input(ElementState::Pressed, button);
                    self.mouse_input(ElementState::Released, button);
                }
                Cmd::DockClick { id, x, y, right } => {
                    let id = NodeId(id);
                    let at = self.content_view(id).and_then(|(r, css)| self.dock_split(id, r, css)).map(|p| {
                        let (r, css) = p.inspector;
                        let k = r.w / css.x.max(1e-3);
                        Vec2::new(r.x + x * k, r.y + y * k)
                    });
                    let Some(p) = at else {
                        log::warn!("script: card {} has no docked inspector", id.0);
                        continue;
                    };
                    let button = if right { MouseButton::Right } else { MouseButton::Left };
                    self.cursor_moved(p);
                    self.mouse_input(ElementState::Pressed, button);
                    self.mouse_input(ElementState::Released, button);
                }
                Cmd::Wheel(x, y) => self.wheel(MouseScrollDelta::LineDelta(x, y), TouchPhase::Moved),
                Cmd::Touchpad { dx, dy, end } => {
                    let phase = if end { TouchPhase::Ended } else { TouchPhase::Moved };
                    self.wheel(MouseScrollDelta::PixelDelta(PhysicalPosition::new(dx as f64, dy as f64)), phase);
                }
                Cmd::CtrlWheel(y) => {
                    let saved = self.mods;
                    self.mods = ModifiersState::CONTROL;
                    self.wheel(MouseScrollDelta::LineDelta(0.0, y), TouchPhase::Moved);
                    self.mods = saved;
                }
                Cmd::Fit => self.fit_all(),
                Cmd::ImePreedit(t) => {
                    let n = t.len();
                    self.ime(Ime::Preedit(t, Some((n, n))));
                }
                Cmd::ImeCommit(t) => self.ime(Ime::Commit(t)),
                Cmd::MenuClick(label) => {
                    let row = self.menu.as_ref().and_then(|m| {
                        let i = menu_labels(&m.items).iter().position(|l| *l == label)?;
                        Some(m.rows[i])
                    });
                    match row {
                        Some(r) => {
                            self.cursor_moved(r.center());
                            self.mouse_input(ElementState::Pressed, MouseButton::Left);
                            self.mouse_input(ElementState::Released, MouseButton::Left);
                        }
                        None => log::warn!("script: no open menu item {label:?}"),
                    }
                }
                Cmd::BadgeClick(id) => match self.scene.node(NodeId(id)).and_then(|n| self.stack_badge(n)) {
                    Some(b) => {
                        self.cursor_moved(b.center());
                        self.mouse_input(ElementState::Pressed, MouseButton::Left);
                        self.mouse_input(ElementState::Released, MouseButton::Left);
                    }
                    None => log::warn!("script: card {id} is no stack's front"),
                },
                Cmd::ShieldClick(id) => match self.shields.iter().find(|(s, _)| s.0 == id).map(|(_, r)| *r) {
                    Some(r) => {
                        self.cursor_moved(r.center());
                        self.mouse_input(ElementState::Pressed, MouseButton::Left);
                        self.mouse_input(ElementState::Released, MouseButton::Left);
                    }
                    None => log::warn!("script: card {id} shows no shield"),
                },
                Cmd::AwakeClick(id) => match self.bar_buttons.iter().find(|(h, _)| *h == Hit::Awake(NodeId(id))).map(|(_, r)| *r) {
                    Some(r) => {
                        self.cursor_moved(r.center());
                        self.mouse_input(ElementState::Pressed, MouseButton::Left);
                        self.mouse_input(ElementState::Released, MouseButton::Left);
                    }
                    None => log::warn!("script: card {id} shows no awake button (hover it first)"),
                },
                Cmd::TagClick(id) => match self.tags.iter().find(|(t, _)| t.0 == id).map(|(_, r)| *r) {
                    Some(r) => {
                        self.cursor_moved(r.center());
                        self.mouse_input(ElementState::Pressed, MouseButton::Left);
                        self.mouse_input(ElementState::Released, MouseButton::Left);
                    }
                    None => log::warn!("script: no trail tag on card {id}"),
                },
                Cmd::Key(combo) => match keymap::parse_combo(&combo) {
                    Some((mods, name)) => self.script_key(name, mods),
                    None => log::warn!("script: bad key combo {combo:?}"),
                },
                Cmd::Type(text) => {
                    for c in text.chars() {
                        let shift = c.is_ascii_uppercase();
                        let mods = if shift { ModifiersState::SHIFT } else { ModifiersState::empty() };
                        self.script_key(&c.to_string(), mods);
                    }
                }
                Cmd::Drag { from: (x0, y0), to: (x1, y1), shift, alt, ctrl } => {
                    let saved = self.mods;
                    self.mods = ModifiersState::empty();
                    if shift {
                        self.mods |= ModifiersState::SHIFT;
                    }
                    if alt {
                        self.mods |= ModifiersState::ALT;
                    }
                    if ctrl {
                        self.mods |= ModifiersState::CONTROL;
                    }
                    self.cursor_moved(Vec2::new(x0, y0));
                    self.mouse_input(ElementState::Pressed, MouseButton::Left);
                    for i in 1..=8 {
                        let t = i as f32 / 8.0;
                        self.cursor_moved(Vec2::new(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t));
                    }
                    self.mouse_input(ElementState::Released, MouseButton::Left);
                    self.mods = saved;
                }
                Cmd::Button { at: (x, y), pressed, alt } => {
                    self.cursor_moved(Vec2::new(x, y));
                    let state = if pressed { ElementState::Pressed } else { ElementState::Released };
                    let saved = self.mods;
                    if alt {
                        self.mods = ModifiersState::ALT;
                    }
                    self.mouse_input(state, MouseButton::Left);
                    self.mods = saved;
                }
                Cmd::Undo => self.undo(false),
                Cmd::Redo => self.undo(true),
                Cmd::Cards => {
                    for e in self.scene.links() {
                        log::info!("script link {} -> {}", e.from.0, e.to.0);
                    }
                    for n in self.scene.all_nodes() {
                        let (r, f) = (n.rect, self.to_screen_rect(n.frame()));
                        log::info!(
                            "script card {}: world ({:.0},{:.0} {:.0}x{:.0}) screen ({:.0},{:.0} {:.0}x{:.0}) live={} awake={} media={} page_hidden={} fps={} sel={} hidden={} blocked={} elements_hidden={} title={:?} {}",
                            n.id.0, r.x, r.y, r.w, r.h, f.x, f.y, f.w, f.h, self.cards.contains_key(&n.id),
                            n.awake.name(),
                            self.media.get(&n.id).map_or("-", |m| if m.playing { "playing" } else { "stopped" }),
                            self.cards.get(&n.id).is_some_and(|c| c.hidden),
                            self.cards.get(&n.id).map_or(0, |c| c.fps),
                            self.selection.contains(n.id), self.scene.is_hidden(n.id),
                            self.cards.get(&n.id).map_or(0, |c| c.page.blocked()),
                            self.cards.get(&n.id).map_or(0, |c| c.page.hidden()), n.title, n.url
                        );
                        let m = self.scene.trail_mark(n.id);
                        let bar = self.bars.iter().find(|(b, _)| *b == n.id).map(|(_, r)| format!("({:.0},{:.0} {:.0}x{:.0})", r.x, r.y, r.w, r.h));
                        log::info!(
                            "script relink {}: parent={} trail={:?} name={:?} colour={:?} label={}",
                            n.id.0,
                            self.scene.parent(n.id).map_or(0, |p| p.0),
                            self.scene.trail_of(n.id).iter().map(|t| t.0).collect::<Vec<_>>(),
                            m.name,
                            m.colour,
                            bar.as_deref().unwrap_or("-")
                        );
                        if let Some(text) = &n.note {
                            log::info!("script note {}: {:?}", n.id.0, text);
                        }
                        if let Some(d) = self.docks.get(&n.id) {
                            let div = self.content_view(n.id).and_then(|(r, css)| self.dock_split(n.id, r, css)).map(|p| p.divider).unwrap_or_default();
                            log::info!(
                                "script dock {}: {:?} share={:.2} keys={} divider screen ({:.0},{:.0} {:.0}x{:.0})",
                                n.id.0, d.side, d.share, self.dock_keys && self.focused == Some(n.id), div.x, div.y, div.w, div.h
                            );
                        }
                        if let Some(c) = n.sticky.as_ref().filter(|_| n.is_sticky()) {
                            log::info!("script sticky {}: colour={c}", n.id.0);
                        }
                        if !n.trail.is_empty() {
                            log::info!("script trail {}: name={:?} colour={:?}", n.id.0, n.trail.name, n.trail.colour);
                        }
                    }
                    for st in self.scene.stacks() {
                        log::info!(
                            "script stack: {:?} front={} anchor=({:.0},{:.0})",
                            st.ids().map(|i| i.0).collect::<Vec<_>>(), st.front.0, st.anchor.x, st.anchor.y
                        );
                    }
                    for e in self.scene.edges() {
                        log::info!("script edge {} -> {}", e.from.0, e.to.0);
                    }
                    for (head, mark) in self.scene.trail_labels() {
                        log::info!("script label on {}: name={:?} colour={:?}", head.0, mark.name, mark.colour);
                    }
                    log::info!(
                        "script focus: card {:?} entered={} mode={:?} solo={:?} order={:?}",
                        self.focused.map(|f| f.0), self.entered(), self.page_keys, self.read.map(|r| r.0),
                        scene::tidy::order(&self.scene).iter().map(|i| i.0).collect::<Vec<_>>()
                    );
                    if let Some(sp) = self.places.spaces.get(&self.spaces.active.0) {
                        log::info!(
                            "script jumps: {:?} at={} marks={:?} board_marks={:?} before={:?}",
                            sp.jumps.cards().iter().map(|i| i.0).collect::<Vec<_>>(), sp.jumps.at(),
                            sp.marks.iter().map(|(id, m)| (id.0, m.keys().collect::<String>())).collect::<Vec<_>>(),
                            self.places.board.keys().collect::<String>(), self.before
                        );
                    }
                    let l = self.read_layout();
                    log::info!(
                        "script read: {:?} css={:?} page_scale={} focused={:?} keys_in_page={} mode={} camera=({:.0},{:.0} z{:.3})",
                        self.read.map(|r| r.0), l.css, self.page_scale, self.focused.map(|f| f.0), self.keys_in_page,
                        if self.entered() { format!("{:?}", self.page_keys) } else { "Board".into() },
                        self.camera.center.x, self.camera.center.y, self.camera.zoom
                    );
                }
                Cmd::Spaces => {
                    for e in &self.spaces.spaces {
                        log::info!(
                            "script space {}: {:?} tint={:?} board={:?} private={}{}",
                            e.id.0, e.name, e.tint, self.theme.board(e.tint.as_deref()), e.private,
                            if e.id == self.spaces.active { " active" } else { "" }
                        );
                    }
                }
                Cmd::Sidebar => {
                    let (rows, panel) = self.sidebar_layout();
                    log::info!(
                        "script sidebar: open={} peek={} keys={} sel={:?} scroll={} shown={} width={:.0} rows={}",
                        self.sidebar_open, self.sidebar_peek, self.sidebar_has_keys(), self.sidebar.index(&rows), self.sidebar.scroll,
                        panel.fits(self.sidebar.scroll), panel.rect.w, rows.len()
                    );
                    for (i, row) in rows.iter().enumerate() {
                        let at = panel.row_rect(i, self.sidebar.scroll).map(|r| r.center());
                        let at = at.map_or_else(|| "off".to_string(), |c| format!("({:.0},{:.0})", c.x, c.y));
                        let what = match row {
                            scene::sidebar::Row::Header { label, count, collapsed, mark, .. } => format!(
                                "header {label:?} count={count}{}{} place={:?}",
                                if *collapsed { " collapsed" } else { "" },
                                if mark.pinned { " pinned" } else { "" },
                                mark.place
                            ),
                            scene::sidebar::Row::Card { id, depth, folded, .. } => {
                                let title = self.scene.node(*id).map(|n| n.title.clone()).unwrap_or_default();
                                format!("card {} depth={depth}{} {title:?}", id.0, if *folded { " folded" } else { "" })
                            }
                        };
                        log::info!("script sidebar row {i} at {at}: {what}");
                    }
                }
                Cmd::History(query) => match &self.page_index {
                    None => log::info!("script history: off"),
                    Some(index) => {
                        let len = index.len().unwrap_or(0);
                        log::info!("script history: {len} pages");
                        for v in index.recent(5).unwrap_or_default() {
                            log::info!("script history recent: {} {:?}", v.url, v.title);
                        }
                        for v in index.search(&query, 10).unwrap_or_default() {
                            log::info!("script history found {query:?}: {} {:?}", v.url, v.title);
                        }
                    }
                },
                Cmd::Icons => {
                    let mut sites: Vec<_> = self.icons.iter().map(|(s, i)| format!("{s}={}x{}", i.icon.width, i.icon.height)).collect();
                    sites.sort();
                    log::info!("script icons: sites={sites:?} named={:?}", self.focused.map(|id| id.0));
                    for n in self.scene.nodes() {
                        let site = scene::icon::site(&n.url);
                        let drawn = self.icon_key(&n.url).is_some();
                        log::info!("script icon card {}: site {site:?} drawn {drawn}", n.id.0);
                    }
                }
                Cmd::Frames => {
                    let (presented, partial, skipped) =
                        self.renderer.as_ref().map_or((0, 0, 0), |r| (r.presented, r.partial, r.skipped));
                    log::info!("script frames: presented {presented} (partial {partial}) skipped {skipped}");
                }
                Cmd::Mem => {
                    let (frames, frame_bytes) = self.cards.values().fold((0, 0u64), |(n, b), c| {
                        let f = c.page.frame();
                        (n + usize::from(!f.pixels.is_empty()), b + f.pixels.len() as u64)
                    });
                    let (textures, texture_bytes) = self.renderer.as_ref().map_or((0, 0), |r| r.page_texture_bytes());
                    let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
                    log::info!(
                        "script mem: {} live {} frames {} {:.1} MiB textures {} {:.1} MiB",
                        crate::dev::memstat::summary(), self.cards.len(), frames, mib(frame_bytes), textures, mib(texture_bytes)
                    );
                }
                Cmd::Zooms => {
                    log::info!("script zooms: sites={:?}", self.zooms.borrow().sites);
                    let mut ids: Vec<_> = self.cards.keys().copied().collect();
                    ids.sort();
                    for id in ids {
                        let url = self.scene.node(id).map(|n| n.url.clone()).unwrap_or_default();
                        let page = self.cards[&id].page.zoom_factor();
                        let default = self.cards[&id].page.default_zoom_factor();
                        log::info!(
                            "script zoom card {}: site {:?} want {} chromium/scale {page:.3} default/scale {default:.3} {url:?}",
                            id.0, pagezoom::site_of(&url), pagezoom::label(self.zooms.borrow().get(&url))
                        );
                    }
                }
                Cmd::Find => match self.omnibox.as_ref().filter(|ob| matches!(ob.target, Target::Find(_))) {
                    None => log::info!("script find: closed marks={:?}", self.find_marks.map(|id| id.0)),
                    Some(ob) => log::info!("script find: {:?} on {:?} found={:?} back={}", ob.text, ob.target, self.found, self.find_back),
                },
                Cmd::Palette => match self.omnibox.as_ref().filter(|ob| ob.target.is_list()) {
                    None => log::info!("script palette: closed"),
                    Some(ob) => {
                        let rows = self.list_rows(ob);
                        let panel = self.list_panel(ob, &rows);
                        let scroll = ob.scroll.min(panel.max_scroll());
                        log::info!(
                            "script palette: query={:?} caret={} list_for={:?} sel={} scroll={} shown={} rows={}",
                            ob.text, ob.caret, ob.list_for, ob.sel, scroll, panel.fits(scroll), rows.len()
                        );
                        for (i, row) in rows.iter().enumerate() {
                            let at = panel.row_rect(i, scroll).map(|r| r.center());
                            let at = at.map_or_else(|| "off".to_string(), |c| format!("({:.0},{:.0})", c.x, c.y));
                            let head = panel.label_rect(i, scroll).map_or("", |(_, h)| h);
                            log::info!(
                                "script palette row {i} at {at}: {:?} {:?} detail={:?} marks={:?} section={head:?}",
                                row.item, row.label, row.detail, row.positions
                            );
                        }
                    }
                },
                Cmd::Open(url) => {
                    self.new_card_at_center(&url);
                }
                Cmd::Log(msg) => log::info!("script log: {msg} (cards={}, edges={})", self.scene.nodes().count(), self.scene.edges().len()),
                Cmd::Panic => panic!("script: panic on purpose"),
                Cmd::Quit => self.quit = true,
            }
        }
    }

    /// Press and release a named key with `mods` held, through the normal key path.
    pub(super) fn script_key(&mut self, name: &str, mods: ModifiersState) {
        let shift = mods.shift_key();
        let (Some(down), Some(up)) = (KeyIn::from_name(name, true, shift), KeyIn::from_name(name, false, shift)) else {
            // Not on the US layout: deliver it as text only.
            let text = KeyIn { pressed: true, code: None, named: None, text: Some(name.to_string()), native: 0 };
            let saved = std::mem::replace(&mut self.mods, mods);
            self.key(text.clone());
            self.key(KeyIn { pressed: false, text: None, ..text });
            self.mods = saved;
            return;
        };
        let saved = std::mem::replace(&mut self.mods, mods);
        self.key(down);
        self.key(up);
        self.mods = saved;
    }
}
