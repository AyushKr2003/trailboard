//! Geometry.

use std::sync::Arc;
use std::time::Instant;

use scene::{Camera, dock, Node, NodeId, read, Rect, Vec2};
use winit::window::Window;

use super::{App, DOCK_GAP, DockParts, Hit, READ_BAR};

impl App {
    pub(crate) fn window(&self) -> Option<&Arc<Window>> {
        self.renderer.as_ref().map(|r| r.window())
    }

    pub(crate) fn scale(&self) -> f32 {
        self.window().map_or(1.0, |w| w.scale_factor() as f32)
    }

    pub(crate) fn viewport(&self) -> Vec2 {
        self.renderer.as_ref().map_or(Vec2::new(1.0, 1.0), |r| {
            let (w, h) = r.viewport();
            Vec2::new(w, h)
        })
    }

    pub(crate) fn to_world(&self, p: Vec2) -> Vec2 {
        self.camera.screen_to_world(p, self.viewport(), self.scale())
    }

    pub(crate) fn to_screen_rect(&self, r: Rect) -> Rect {
        self.camera.rect_to_screen(r, self.viewport(), self.scale())
    }

    pub(crate) fn read_layout(&self) -> read::ReadLayout {
        read::layout(self.viewport(), self.scale(), READ_BAR * self.ui())
    }

    /// Card `n`'s page size on the board (CSS px). Pages lay out at Read
    /// Mode's width and the card shows them scaled (`read::card_css`), so a
    /// card always looks like the page in a full window; notes fill their card.
    pub(crate) fn card_css(&self, n: &Node) -> Vec2 {
        let card = Vec2::new(n.rect.w, n.rect.h);
        if n.note.is_some() {
            return read::card_css(card, 0);
        }
        read::card_css(card, self.read_layout().css.0)
    }

    /// Card `id`'s content area (screen px) and its size in CSS px: the
    /// page's, or the page's and its docked inspector's together.
    pub(crate) fn content_view(&self, id: NodeId) -> Option<(Rect, Vec2)> {
        if self.read == Some(id) {
            let l = self.read_layout();
            return Some((l.content, Vec2::new(l.css.0 as f32, l.css.1 as f32)));
        }
        let n = self.scene.node(id)?;
        Some((self.to_screen_rect(n.rect), self.card_css(n)))
    }

    /// Where card `id`'s page is drawn (screen px) and its size in CSS px.
    pub(crate) fn page_view(&self, id: NodeId) -> Option<(Rect, Vec2)> {
        let (r, css) = self.content_view(id)?;
        Some(self.dock_split(id, r, css).map_or((r, css), |s| s.page))
    }

    /// Card `id`'s content at `r` (screen px), `css` CSS px, split around
    /// its docked inspector; `None` without one.
    pub(crate) fn dock_split(&self, id: NodeId, r: Rect, css: Vec2) -> Option<DockParts> {
        let d = self.docks.get(&id)?;
        let s = dock::split(Rect::new(0.0, 0.0, css.x, css.y), d.side, d.share, DOCK_GAP);
        let k = r.w / css.x.max(1e-3);
        let to = |c: Rect| Rect::new(r.x + c.x * k, r.y + c.y * k, c.w * k, c.h * k);
        Some(DockParts {
            page: (to(s.page), Vec2::new(s.page.w, s.page.h)),
            divider: to(s.divider),
            inspector: (to(s.inspector), Vec2::new(s.inspector.w, s.inspector.h)),
        })
    }

    /// Screen point → CSS px in card `id`'s docked inspector.
    pub(crate) fn dock_point(&self, id: NodeId, screen: Vec2) -> Option<(i32, i32)> {
        let (r, css) = self.content_view(id)?;
        let (r, css) = self.dock_split(id, r, css)?.inspector;
        let k = css.x / r.w.max(1e-3);
        Some((((screen.x - r.x) * k).round() as i32, ((screen.y - r.y) * k).round() as i32))
    }

    /// What a point in card `id`'s content is over: its page, its docked
    /// inspector, or the divider between them.
    pub(crate) fn content_hit(&self, id: NodeId, screen: Vec2) -> Hit {
        if let Some((r, css)) = self.content_view(id)
            && let Some(parts) = self.dock_split(id, r, css)
        {
            // A little wider than it looks, to be easy to catch.
            if parts.divider.inflate(2.0 * self.scale()).contains(screen) {
                return Hit::Divider(id);
            }
            if parts.inspector.0.contains(screen) {
                return Hit::Inspector(id);
            }
        }
        Hit::Content(id)
    }

    /// Screen point → CSS px in card `id`'s page.
    pub(crate) fn page_point(&self, id: NodeId, screen: Vec2) -> Option<(i32, i32)> {
        let (r, css) = self.page_view(id)?;
        let k = css.x / r.w.max(1e-3);
        Some((((screen.x - r.x) * k).round() as i32, ((screen.y - r.y) * k).round() as i32))
    }

    /// A rect in card `id`'s page (CSS px) → screen px.
    pub(crate) fn page_rect_to_screen(&self, id: NodeId, p: Rect) -> Option<Rect> {
        let (r, css) = self.page_view(id)?;
        let k = r.w / css.x.max(1e-3);
        Some(Rect::new(r.x + p.x * k, r.y + p.y * k, p.w * k, p.h * k))
    }

    /// The camera that shows `r` (the opening cards) in the window.
    pub(crate) fn opening_camera(&self, r: Rect) -> Camera {
        let mut goal = Camera::fit(r, self.viewport(), self.scale(), 8.0 * self.scale());
        goal.zoom = goal.zoom.min(2.0);
        goal
    }

    /// After a resize, fit the opening cards to the window's real size, as
    /// long as nothing has moved the camera since they were first fitted.
    pub(crate) fn refit_opening(&mut self) {
        let Some((r, fitted)) = self.opening_fit else { return };
        if self.camera != fitted || self.camera_goal.is_some() || self.read.is_some() {
            self.opening_fit = None;
            return;
        }
        let goal = self.opening_camera(r);
        if goal != fitted {
            log::info!("opening fit redone for a {:.0}×{:.0} window", self.viewport().x, self.viewport().y);
            self.camera = goal;
            self.opening_fit = Some((r, goal));
            self.camera_moved();
        }
    }

    pub(crate) fn camera_moved(&mut self) {
        self.camera_changed = Instant::now();
        self.dirty = true;
        // The menu belongs to a spot on a page; it shouldn't float away from it.
        self.menu = None;
    }
}
