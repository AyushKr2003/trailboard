//! Board state and interaction: cards are live pages on an infinite canvas.
//!
//! Input model
//! - Drag empty board / middle-drag anywhere: pan. Wheel over board: pan. Ctrl+wheel: zoom.
//! - Click a card: focus it (keyboard and wheel go to the page). Drag its title bar to move,
//!   its bottom-right corner to resize; × closes; double-click the title bar to fill the view.
//! - Esc (with no omnibox) unfocuses the card. With no card focused: F fits all, N/T new card.
//! - Ctrl+L address bar, Ctrl+T new card, Ctrl+W close, Ctrl+Shift+T reopen,
//!   Ctrl+R reload, Alt+←/→ back/forward, Ctrl+0 fit all.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use compositor::{rgba, Background, Layer, Quad, Renderer, Shape, Text};
use scene::{placement, Camera, NodeId, Rect, Scene, Vec2};
use web::{MouseButton as WebButton, Page, SpawnBeside, WebEvent};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use crate::keymap;
use crate::omnibox::{self, Omnibox, Target};
use crate::script::{Cmd, Script};

const START_URL: &str = "https://en.wikipedia.org/wiki/Special:Random";
const CARD_W: f32 = 1024.0;
const CARD_H: f32 = 720.0;
const MIN_CARD: f32 = 240.0;
const RESIZE_GRIP: f32 = 18.0;
const CLOSE_W: f32 = 30.0;

#[derive(Debug, Clone, Copy)]
enum Drag {
    None,
    Pan { last: Vec2 },
    Move { id: NodeId, grab: Vec2 },
    Resize { id: NodeId, anchor: Vec2, start: Rect },
    /// Mouse is down inside a page; moves and release go to it.
    Page { id: NodeId, button: WebButton },
}

struct Card {
    page: Page,
    loading: bool,
    hidden: bool,
}

pub struct App {
    renderer: Option<Renderer>,
    scene: Scene,
    camera: Camera,
    /// Camera we're gliding toward (fit / read mode).
    camera_goal: Option<Camera>,
    cards: HashMap<NodeId, Card>,
    focused: Option<NodeId>,
    closed: Vec<(Rect, String)>,
    omnibox: Option<Omnibox>,
    spawn_beside: SpawnBeside,
    drag: Drag,
    cursor: Vec2,
    mods: ModifiersState,
    last_click: Option<(Instant, Vec2, MouseButton)>,
    click_count: i32,
    /// When the camera last changed (used to re-rasterize pages at the settled zoom).
    camera_changed: Instant,
    page_scale: f32,
    initial_url: Option<String>,
    started: Instant,
    script: Option<Script>,
    pub quit: bool,
    board_path: std::path::PathBuf,
    /// Board changed since the last save.
    dirty: bool,
    last_save: Instant,
}

impl App {
    pub fn new(initial_url: Option<String>, board_path: std::path::PathBuf) -> Self {
        Self {
            renderer: None,
            scene: Scene::new(),
            camera: Camera::default(),
            camera_goal: None,
            cards: HashMap::new(),
            focused: None,
            closed: Vec::new(),
            omnibox: None,
            spawn_beside: SpawnBeside::new(true),
            drag: Drag::None,
            cursor: Vec2::default(),
            mods: ModifiersState::empty(),
            last_click: None,
            click_count: 1,
            camera_changed: Instant::now(),
            page_scale: 1.0,
            initial_url,
            started: Instant::now(),
            script: Script::from_env(),
            quit: false,
            board_path,
            dirty: false,
            last_save: Instant::now(),
        }
    }

    pub fn shutdown(&mut self) {
        self.save();
        self.cards.clear(); // closes every browser before CEF shuts down
    }

    // ---------------------------------------------------------------- persistence

    fn save(&mut self) {
        if self.renderer.is_none() {
            return; // never initialized; don't clobber a saved board with an empty one
        }
        let board = storage::Board::new(self.scene.clone(), self.camera);
        match storage::save_board(&self.board_path, &board) {
            Ok(()) => self.dirty = false,
            Err(e) => log::error!("saving board to {}: {e}", self.board_path.display()),
        }
        self.last_save = Instant::now();
    }

    fn autosave(&mut self) {
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(3) && matches!(self.drag, Drag::None) {
            self.save();
        }
    }

    /// Load the saved board. Returns false when starting fresh.
    fn restore(&mut self) -> bool {
        match storage::load_board(&self.board_path) {
            Ok(board) => {
                self.scene = board.scene;
                self.camera = board.camera;
                let ids: Vec<NodeId> = self.scene.nodes().map(|n| n.id).collect();
                for id in ids {
                    self.create_page(id);
                }
                log::info!("restored {} cards from {}", self.scene.nodes().count(), self.board_path.display());
                !self.scene.is_empty()
            }
            Err(storage::LoadError::Missing) => false,
            Err(e) => {
                // Keep the unreadable file for the user instead of overwriting it.
                let aside = self.board_path.with_extension(format!("unreadable-{}.json", std::process::id()));
                log::error!("{e}; moving it to {}", aside.display());
                let _ = std::fs::rename(&self.board_path, &aside);
                false
            }
        }
    }

    // ---------------------------------------------------------------- geometry

    fn window(&self) -> Option<&Arc<Window>> {
        self.renderer.as_ref().map(|r| r.window())
    }

    fn scale(&self) -> f32 {
        self.window().map_or(1.0, |w| w.scale_factor() as f32)
    }

    fn viewport(&self) -> Vec2 {
        self.renderer.as_ref().map_or(Vec2::new(1.0, 1.0), |r| {
            let (w, h) = r.viewport();
            Vec2::new(w, h)
        })
    }

    fn to_world(&self, p: Vec2) -> Vec2 {
        self.camera.screen_to_world(p, self.viewport(), self.scale())
    }

    fn to_screen_rect(&self, r: Rect) -> Rect {
        self.camera.rect_to_screen(r, self.viewport(), self.scale())
    }

    fn page_point(&self, id: NodeId, screen: Vec2) -> Option<(i32, i32)> {
        let n = self.scene.node(id)?;
        let w = self.to_world(screen);
        Some(((w.x - n.rect.x).round() as i32, (w.y - n.rect.y).round() as i32))
    }

    fn camera_moved(&mut self) {
        self.camera_changed = Instant::now();
        self.dirty = true;
    }

    // ---------------------------------------------------------------- cards

    fn open_card(&mut self, rect: Rect, url: &str, from: Option<NodeId>) -> Option<NodeId> {
        let id = match from {
            Some(src) => self.scene.spawn_beside(src, url)?,
            None => self.scene.add_page(rect, url),
        };
        self.create_page(id);
        self.dirty = true;
        Some(id)
    }

    /// Start the live browser for an existing card.
    fn create_page(&mut self, id: NodeId) {
        let Some(n) = self.scene.node(id) else { return };
        let (url, r) = (n.url.clone(), n.rect);
        match Page::new(id.0, &url, (r.w as i32, r.h as i32), self.page_scale, &self.spawn_beside) {
            Some(page) => {
                self.cards.insert(id, Card { page, loading: true, hidden: false });
            }
            None => log::error!("failed to create browser for {url}"),
        }
    }

    fn new_card_at_center(&mut self, url: &str) -> Option<NodeId> {
        let center = self.camera.center;
        let rect = placement::free_near(&self.scene, center, CARD_W, CARD_H);
        let id = self.open_card(rect, url, None)?;
        self.focus(Some(id));
        self.ensure_visible(id);
        Some(id)
    }

    fn close_card(&mut self, id: NodeId) {
        self.dirty = true;
        if let Some(n) = self.scene.remove(id) {
            self.closed.push((n.rect, n.url));
        }
        self.cards.remove(&id);
        if let Some(r) = self.renderer.as_mut() {
            r.remove_page(id.0);
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
    }

    fn reopen_closed(&mut self) {
        if let Some((rect, url)) = self.closed.pop() {
            let id = self.open_card(rect, &url, None);
            self.focus(id);
        }
    }

    fn focus(&mut self, id: Option<NodeId>) {
        if self.focused == id {
            return;
        }
        if let Some(old) = self.focused.and_then(|o| self.cards.get(&o)) {
            old.page.set_focus(false);
        }
        self.focused = id;
        if let Some(id) = id {
            self.scene.bring_to_front(id);
            self.dirty = true;
            if let Some(c) = self.cards.get(&id) {
                c.page.set_focus(true);
            }
        }
    }

    fn focused_page(&self) -> Option<&Page> {
        self.focused.and_then(|id| self.cards.get(&id)).map(|c| &c.page)
    }

    // ---------------------------------------------------------------- camera

    fn fit_all(&mut self) {
        if let Some(b) = self.scene.bounds() {
            self.camera_goal = Some(Camera::fit(b, self.viewport(), self.scale(), 48.0 * self.scale()));
        }
    }

    fn fill_view(&mut self, id: NodeId) {
        if let Some(n) = self.scene.node(id) {
            let mut goal = Camera::fit(n.frame(), self.viewport(), self.scale(), 8.0 * self.scale());
            goal.zoom = goal.zoom.min(2.0);
            self.camera_goal = Some(goal);
        }
    }

    /// Glide just enough to bring card `id` on screen (fitting it with the focused card if possible).
    fn ensure_visible(&mut self, id: NodeId) {
        let Some(frame) = self.scene.node(id).map(|n| n.frame()) else { return };
        let vp = self.viewport();
        let on_screen = self.to_screen_rect(frame);
        if on_screen.x >= 0.0 && on_screen.y >= 0.0 && on_screen.right() <= vp.x && on_screen.bottom() <= vp.y {
            return;
        }
        let mut goal = self.camera;
        let need = Camera::fit(frame, vp, self.scale(), 32.0 * self.scale());
        if need.zoom < goal.zoom {
            goal.zoom = need.zoom;
        }
        goal.center = frame.center();
        self.camera_goal = Some(goal);
    }

    fn step_camera(&mut self, dt: f32) {
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
    fn sync_pages(&mut self) {
        let settled = self.camera_changed.elapsed() > Duration::from_millis(250) && self.camera_goal.is_none();
        if settled {
            let want = ((self.camera.zoom * self.scale()) * 4.0).round().clamp(2.0, 8.0) / 4.0;
            if (want - self.page_scale).abs() > f32::EPSILON {
                self.page_scale = want;
            }
        }
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let page_scale = self.page_scale;
        let mut updates = Vec::new();
        for n in self.scene.nodes() {
            let on_screen = self.to_screen_rect(n.frame()).intersects(&screen);
            updates.push((n.id, on_screen, (n.rect.w.round() as i32, n.rect.h.round() as i32)));
        }
        for (id, visible, size) in updates {
            if let Some(card) = self.cards.get_mut(&id) {
                if card.hidden == visible {
                    card.hidden = !visible;
                    card.page.set_hidden(!visible);
                }
                card.page.resize(size, page_scale);
            }
        }
    }

    // ---------------------------------------------------------------- events from pages

    fn handle_web_events(&mut self) {
        for ev in web::drain_events() {
            match ev {
                WebEvent::OpenBeside { source, url } => {
                    let src = NodeId(source);
                    if self.scene.node(src).is_some()
                        && let Some(id) = self.open_card(Rect::default(), &url, Some(src)) {
                            self.ensure_visible(id);
                        }
                }
                WebEvent::TitleChanged { page, title } => {
                    if let Some(n) = self.scene.node_mut(NodeId(page)) {
                        n.title = title;
                        self.dirty = true;
                    }
                }
                WebEvent::UrlChanged { page, url } => {
                    if let Some(n) = self.scene.node_mut(NodeId(page)) {
                        n.url = url;
                        self.dirty = true;
                    }
                }
                WebEvent::LoadingChanged { page, loading } => {
                    if let Some(c) = self.cards.get_mut(&NodeId(page)) {
                        c.loading = loading;
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- input

    fn hit(&self, screen: Vec2) -> Hit {
        let w = self.to_world(screen);
        let Some(id) = self.scene.topmost_at(w) else { return Hit::Board };
        let n = self.scene.node(id).expect("hit node exists");
        let k = self.camera.pixels_per_unit(self.scale());
        let grip = RESIZE_GRIP / k.min(1.0);
        if w.y < n.rect.y {
            if w.x > n.rect.right() - CLOSE_W {
                return Hit::Close(id);
            }
            return Hit::Header(id);
        }
        if w.x > n.rect.right() - grip.min(n.rect.w / 4.0) && w.y > n.rect.bottom() - grip.min(n.rect.h / 4.0) {
            return Hit::Grip(id);
        }
        Hit::Content(id)
    }

    fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        let p = self.cursor;
        let mods = keymap::modifiers(self.mods);
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
                self.drag = Drag::Pan { last: p };
                return;
            }
            let web_button = match button {
                MouseButton::Left => WebButton::Left,
                MouseButton::Right => WebButton::Right,
                _ => return,
            };
            match self.hit(p) {
                Hit::Board => {
                    self.focus(None);
                    self.drag = Drag::Pan { last: p };
                }
                Hit::Close(id) => self.close_card(id),
                Hit::Header(id) => {
                    self.focus(Some(id));
                    if self.click_count >= 2 {
                        self.fill_view(id);
                        self.drag = Drag::None;
                    } else {
                        let n = self.scene.node(id).expect("hit node");
                        let w = self.to_world(p);
                        self.drag = Drag::Move { id, grab: w - Vec2::new(n.rect.x, n.rect.y) };
                    }
                }
                Hit::Grip(id) => {
                    self.focus(Some(id));
                    let start = self.scene.node(id).expect("hit node").rect;
                    self.drag = Drag::Resize { id, anchor: self.to_world(p), start };
                }
                Hit::Content(id) => {
                    self.focus(Some(id));
                    if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_button(x, y, mods, web_button, false, self.click_count.min(3));
                    }
                    self.drag = Drag::Page { id, button: web_button };
                }
            }
        } else {
            if let Drag::Page { id, button: b } = self.drag
                && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                    card.page.mouse_button(x, y, mods, b, true, self.click_count.min(3));
                }
            self.drag = Drag::None;
        }
    }

    fn cursor_moved(&mut self, p: Vec2) {
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
                if let Some(n) = self.scene.node_mut(id) {
                    n.rect.x = w.x - grab.x;
                    n.rect.y = w.y - grab.y;
                }
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
            Drag::None => {
                let hit = self.hit(p);
                let prev_hit = self.hit(prev);
                if let Hit::Content(id) = hit
                    && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, p)) {
                        card.page.mouse_move(x, y, mods, false);
                    }
                if let (Hit::Content(old), false) = (prev_hit, matches!(hit, Hit::Content(n) if Some(n) == prev_hit.id()))
                    && let (Some(card), Some((x, y))) = (self.cards.get(&old), self.page_point(old, p)) {
                        card.page.mouse_move(x, y, mods, true);
                    }
                let icon = match hit {
                    Hit::Header(_) => CursorIcon::Grab,
                    Hit::Grip(_) => CursorIcon::SeResize,
                    Hit::Close(_) => CursorIcon::Pointer,
                    _ => CursorIcon::Default,
                };
                if let Some(w) = self.window() {
                    w.set_cursor(icon);
                }
            }
        }
    }

    fn wheel(&mut self, delta: MouseScrollDelta) {
        let (dx, dy, lines) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x, y, true),
            MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32, false),
        };
        self.camera_goal = None;
        if self.mods.control_key() {
            let steps = if lines { dy } else { dy / 60.0 };
            self.camera.zoom_about(1.15f32.powf(steps), self.cursor, self.viewport(), self.scale());
            self.camera_moved();
            return;
        }
        if let Hit::Content(id) | Hit::Grip(id) = self.hit(self.cursor)
            && Some(id) == self.focused {
                if let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, self.cursor)) {
                    let (px, py) = if lines { (dx * 100.0, dy * 100.0) } else { (dx, dy) };
                    card.page.mouse_wheel(x, y, keymap::modifiers(self.mods), px as i32, py as i32);
                }
                return;
            }
        let (px, py) = if lines { (dx * 60.0, dy * 60.0) } else { (dx, dy) };
        self.camera.pan_pixels(Vec2::new(px, py), self.scale());
        self.camera_moved();
    }

    fn key(&mut self, ev: winit::event::KeyEvent) {
        let pressed = ev.state == ElementState::Pressed;
        let ctrl = self.mods.control_key();
        let shift = self.mods.shift_key();
        let alt = self.mods.alt_key();
        // Shortcuts follow the logical key (layout-aware), like browsers do.
        let code = keymap::shortcut_code(&ev);

        if let Some(ob) = self.omnibox.as_mut() {
            if !pressed {
                return;
            }
            match &ev.logical_key {
                Key::Named(NamedKey::Escape) => self.omnibox = None,
                Key::Named(NamedKey::Enter) => {
                    let ob = self.omnibox.take().expect("omnibox open");
                    if let Some(url) = omnibox::resolve(&ob.text) {
                        match (ob.target, self.focused_page()) {
                            (Target::Focused, Some(page)) => page.load(&url),
                            _ => {
                                self.new_card_at_center(&url);
                            }
                        }
                    }
                }
                Key::Named(NamedKey::Backspace) => ob.backspace(),
                Key::Named(NamedKey::Delete) => ob.delete(),
                Key::Named(NamedKey::ArrowLeft) => ob.left(),
                Key::Named(NamedKey::ArrowRight) => ob.right(),
                Key::Named(NamedKey::Home) => ob.home(),
                Key::Named(NamedKey::End) => ob.end(),
                _ if ctrl && code == Some(KeyCode::KeyA) => ob.select_all = true,
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
            return;
        }

        if pressed {
            // App shortcuts first.
            let handled = match code {
                Some(KeyCode::KeyL) if ctrl => {
                    let url = self.focused.and_then(|id| self.scene.node(id)).map(|n| n.url.clone()).unwrap_or_default();
                    let target = if self.focused.is_some() { Target::Focused } else { Target::NewCard };
                    self.omnibox = Some(Omnibox::new(&url, target));
                    true
                }
                Some(KeyCode::KeyT) if ctrl && shift => {
                    self.reopen_closed();
                    true
                }
                Some(KeyCode::KeyT) if ctrl => {
                    self.omnibox = Some(Omnibox::new("", Target::NewCard));
                    true
                }
                Some(KeyCode::KeyW) if ctrl => {
                    if let Some(id) = self.focused {
                        self.close_card(id);
                    }
                    true
                }
                Some(KeyCode::KeyR) if ctrl => {
                    if let Some(p) = self.focused_page() {
                        p.reload();
                    }
                    true
                }
                Some(KeyCode::Digit0) if ctrl => {
                    self.fit_all();
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
                Some(KeyCode::Escape) if self.focused.is_some() && self.drag_is_idle() => {
                    // Let the page see Esc too (closes its menus), then leave the page.
                    if let Some(p) = self.focused_page() {
                        p.send_key(&keymap::to_web(&ev, self.mods));
                    }
                    self.focus(None);
                    true
                }
                // Board shortcuts when no page has focus.
                Some(KeyCode::KeyF) if self.focused.is_none() && !ctrl => {
                    self.fit_all();
                    true
                }
                Some(KeyCode::KeyN | KeyCode::KeyT) if self.focused.is_none() && !ctrl => {
                    self.omnibox = Some(Omnibox::new("", Target::NewCard));
                    true
                }
                _ => false,
            };
            if handled {
                return;
            }
        }
        if let Some(p) = self.focused_page() {
            p.send_key(&keymap::to_web(&ev, self.mods));
        }
    }

    fn drag_is_idle(&self) -> bool {
        matches!(self.drag, Drag::None)
    }

    // ---------------------------------------------------------------- drawing

    fn draw(&mut self) {
        let scale = self.scale();
        let vp = self.viewport();
        let k = self.camera.pixels_per_unit(scale);

        // Upload new page frames.
        if let Some(r) = self.renderer.as_mut() {
            for (id, card) in &self.cards {
                let f = card.page.frame();
                r.update_page(id.0, f.width, f.height, &f.pixels, f.serial);
            }
        }

        let origin = self.camera.world_to_screen(Vec2::default(), vp, scale);
        let spacing = 32.0 * k;
        let bg = Background {
            color: rgba(22, 22, 26, 1.0),
            dot: rgba(70, 70, 80, (spacing / 24.0).clamp(0.0, 1.0)),
            grid_origin: [origin.x, origin.y],
            grid_spacing: spacing,
            dot_radius: (1.1 * scale).max(0.8),
        };

        let mut layers: Vec<Layer> = Vec::new();

        // Trail edges under everything: a gentle S-curve from source's right edge to target's left edge.
        let mut edges = Layer::default();
        for e in self.scene.edges() {
            let (Some(a), Some(b)) = (self.scene.node(e.from), self.scene.node(e.to)) else { continue };
            let fa = self.to_screen_rect(a.frame());
            let fb = self.to_screen_rect(b.frame());
            let (p0, p3) = if fb.x >= fa.right() {
                ([fa.right(), fa.y + 18.0 * k], [fb.x, fb.y + 18.0 * k])
            } else {
                ([fa.x, fa.y + 18.0 * k], [fb.right(), fb.y + 18.0 * k])
            };
            let dx = (p3[0] - p0[0]) * 0.5;
            let (p1, p2) = ([p0[0] + dx, p0[1]], [p3[0] - dx, p3[1]]);
            let mut prev = p0;
            for i in 1..=24 {
                let t = i as f32 / 24.0;
                let u = 1.0 - t;
                let x = u * u * u * p0[0] + 3.0 * u * u * t * p1[0] + 3.0 * u * t * t * p2[0] + t * t * t * p3[0];
                let y = u * u * u * p0[1] + 3.0 * u * u * t * p1[1] + 3.0 * u * t * t * p2[1] + t * t * t * p3[1];
                edges.quads.push(Quad {
                    shape: Shape::Segment { from: prev, to: [x, y], width: (2.0 * k).clamp(1.0, 4.0) },
                    color: rgba(120, 140, 255, 0.75),
                    texture: None,
                });
                prev = [x, y];
            }
        }
        layers.push(edges);

        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        for n in self.scene.draw_order() {
            let frame = self.to_screen_rect(n.frame());
            if !frame.inflate(40.0).intersects(&screen) {
                continue;
            }
            let content = self.to_screen_rect(n.rect);
            let header = self.to_screen_rect(n.header());
            let radius = (10.0 * k).min(14.0);
            let focused = self.focused == Some(n.id);
            let card = self.cards.get(&n.id);
            let mut l = Layer::default();
            l.quads.push(Quad {
                shape: Shape::Shadow { rect: frame.translate(Vec2::new(0.0, 6.0 * k)), radius, blur: (22.0 * k).max(2.0) },
                color: [0.0, 0.0, 0.0, if focused { 0.55 } else { 0.4 }],
                texture: None,
            });
            if focused {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: frame.inflate(2.0), radius: radius + 2.0 },
                    color: rgba(110, 130, 255, 1.0),
                    texture: None,
                });
            }
            l.quads.push(Quad {
                shape: Shape::Rect { rect: frame, radius },
                color: if focused { rgba(46, 46, 54, 1.0) } else { rgba(36, 36, 42, 1.0) },
                texture: None,
            });
            let has_tex = self.renderer.as_ref().is_some_and(|r| r.has_page(n.id.0));
            l.quads.push(Quad {
                shape: Shape::Rect { rect: content, radius: 0.0 },
                color: if has_tex { [1.0; 4] } else { rgba(250, 250, 250, 1.0) },
                texture: has_tex.then_some(n.id.0),
            });
            // Loading bar along the bottom of the title bar.
            if card.is_some_and(|c| c.loading) {
                let t = (self.started.elapsed().as_secs_f32() * 0.8).fract();
                let w = header.w * 0.25;
                let x = header.x + (header.w - w) * t;
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(x, header.bottom() - 2.0, w, 2.0), radius: 1.0 },
                    color: rgba(110, 130, 255, 1.0),
                    texture: None,
                });
            }
            let font = 13.0 * k;
            if font >= 6.0 {
                let pad = 10.0 * k;
                let title = if n.title.is_empty() { n.url.as_str() } else { n.title.as_str() };
                let clip = Rect::new(header.x + pad, header.y, (header.w - pad * 2.0 - CLOSE_W * k).max(1.0), header.h);
                l.texts.push(Text {
                    text: title.to_string(),
                    pos: [header.x + pad, header.y + (header.h - font * 1.3) / 2.0],
                    size: font,
                    color: if focused { rgba(240, 240, 245, 1.0) } else { rgba(190, 190, 200, 1.0) },
                    clip,
                    bold: true,
                });
                let close_size = 16.0 * k;
                l.texts.push(Text {
                    text: "×".into(),
                    pos: [header.right() - CLOSE_W * k + (CLOSE_W * k - close_size * 0.6) / 2.0, header.y + (header.h - close_size * 1.3) / 2.0],
                    size: close_size,
                    color: rgba(170, 170, 180, 1.0),
                    clip: header,
                    bold: false,
                });
            }
            layers.push(l);
        }

        // Empty-board hint.
        if self.scene.is_empty() && self.omnibox.is_none() {
            let size = 16.0 * scale;
            let msg = "Press N or Ctrl+T to open a page";
            let w = self.renderer.as_mut().map_or(300.0, |r| r.measure(msg, size, false));
            layers.push(Layer {
                quads: vec![],
                texts: vec![Text {
                    text: msg.into(),
                    pos: [(vp.x - w) / 2.0, vp.y / 2.0 - size],
                    size,
                    color: rgba(150, 150, 160, 1.0),
                    clip: screen,
                    bold: false,
                }],
            });
        }

        if let Some(ob) = &self.omnibox {
            let w = (720.0 * scale).min(vp.x - 32.0 * scale);
            let h = 48.0 * scale;
            let r = Rect::new((vp.x - w) / 2.0, 18.0 * scale, w, h);
            let size = 17.0 * scale;
            let pad = 16.0 * scale;
            let (shown, color) = if ob.text.is_empty() {
                ("Search or enter address".to_string(), rgba(130, 130, 140, 1.0))
            } else {
                (ob.text.clone(), rgba(240, 240, 245, 1.0))
            };
            let caret_x = self.renderer.as_mut().map_or(0.0, |rd| rd.measure(ob.before_caret(), size, false));
            let text_w = self.renderer.as_mut().map_or(0.0, |rd| rd.measure(&ob.text, size, false));
            let mut l = Layer::default();
            l.quads.push(Quad {
                shape: Shape::Shadow { rect: r.translate(Vec2::new(0.0, 8.0 * scale)), radius: 12.0 * scale, blur: 28.0 * scale },
                color: [0.0, 0.0, 0.0, 0.6],
                texture: None,
            });
            l.quads.push(Quad { shape: Shape::Rect { rect: r, radius: 12.0 * scale }, color: rgba(40, 40, 48, 1.0), texture: None });
            if ob.select_all && !ob.text.is_empty() {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(r.x + pad, r.y + 12.0 * scale, text_w, h - 24.0 * scale), radius: 3.0 },
                    color: rgba(80, 100, 220, 0.6),
                    texture: None,
                });
            } else {
                l.quads.push(Quad {
                    shape: Shape::Rect { rect: Rect::new(r.x + pad + caret_x, r.y + 12.0 * scale, 2.0 * scale.max(1.0), h - 24.0 * scale), radius: 0.0 },
                    color: rgba(150, 165, 255, 1.0),
                    texture: None,
                });
            }
            l.texts.push(Text {
                text: shown,
                pos: [r.x + pad, r.y + (h - size * 1.3) / 2.0],
                size,
                color,
                clip: r,
                bold: false,
            });
            layers.push(l);
        }

        if let Some(r) = self.renderer.as_mut() {
            r.render(bg, &layers);
        }
    }

    /// One iteration of app work outside winit callbacks.
    pub fn tick(&mut self, dt: f32) {
        self.run_script();
        self.handle_web_events();
        self.autosave();
        self.step_camera(dt);
        self.sync_pages();
        if let Some(w) = self.window() {
            w.request_redraw();
        }
    }
}

impl App {
    fn run_script(&mut self) {
        if self.renderer.is_none() {
            return;
        }
        while let Some(cmd) = self.script.as_mut().and_then(Script::next) {
            log::info!("script: {cmd:?}");
            match cmd {
                Cmd::Wait(_) => {}
                Cmd::Move(x, y) => self.cursor_moved(Vec2::new(x, y)),
                Cmd::Click(x, y) => {
                    self.cursor_moved(Vec2::new(x, y));
                    self.mouse_input(ElementState::Pressed, MouseButton::Left);
                    self.mouse_input(ElementState::Released, MouseButton::Left);
                }
                Cmd::Wheel(x, y) => self.wheel(MouseScrollDelta::LineDelta(x, y)),
                Cmd::CtrlWheel(y) => {
                    let saved = self.mods;
                    self.mods = ModifiersState::CONTROL;
                    self.wheel(MouseScrollDelta::LineDelta(0.0, y));
                    self.mods = saved;
                }
                Cmd::Fit => self.fit_all(),
                Cmd::Open(url) => {
                    self.new_card_at_center(&url);
                }
                Cmd::Log(msg) => log::info!("script log: {msg} (cards={}, edges={})", self.scene.nodes().count(), self.scene.edges().len()),
                Cmd::Quit => self.quit = true,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Board,
    Header(NodeId),
    Close(NodeId),
    Grip(NodeId),
    Content(NodeId),
}

impl Hit {
    fn id(&self) -> Option<NodeId> {
        match *self {
            Hit::Board => None,
            Hit::Header(id) | Hit::Close(id) | Hit::Grip(id) | Hit::Content(id) => Some(id),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Trailboard")
            .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let display = Box::new(event_loop.owned_display_handle());
        match pollster::block_on(Renderer::new(window, display)) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                log::error!("GPU init failed: {e}");
                event_loop.exit();
                return;
            }
        }
        self.page_scale = self.scale();
        let restored = self.restore();
        let url = match self.initial_url.take() {
            Some(u) => Some(u),
            None if !restored => Some(START_URL.to_string()),
            None => None,
        };
        if let Some(url) = url
            && let Some(id) = self.new_card_at_center(&url)
                && !restored {
                    self.fill_view(id);
                    if let Some(goal) = self.camera_goal.take() {
                        self.camera = goal;
                    }
                }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => self.camera_moved(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => self.cursor_moved(Vec2::new(position.x as f32, position.y as f32)),
            WindowEvent::CursorLeft { .. } => {
                if let Some(id) = self.hit(self.cursor).id()
                    && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, self.cursor)) {
                        card.page.mouse_move(x, y, 0, true);
                    }
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_input(state, button),
            WindowEvent::MouseWheel { delta, .. } => self.wheel(delta),
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => self.key(event),
            WindowEvent::Focused(false) => self.mods = ModifiersState::empty(),
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
}
