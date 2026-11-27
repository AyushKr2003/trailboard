//! The window's event loop: winit calls in here.

use super::*;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Trailboard")
            .with_name("trailboard", "trailboard")
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
        self.start_desktop();
        self.update_card_size();
        let first_run = spaces::is_first_run(&self.data_dir);
        let restored = self.restore();
        // At the restored camera's resolution, so pages don't lay out twice.
        self.page_scale = self.settled_page_scale();
        let urls = std::mem::take(&mut self.initial_urls);
        let mut last = None;
        for url in &urls {
            last = self.new_card_at_center(url).or(last);
        }
        if urls.is_empty() && !restored {
            last = Some(self.add_start());
        }
        let guide = (first_run && self.persist).then(|| self.add_guide(last));
        if !restored {
            let shown = [last, guide].into_iter().flatten().filter_map(|id| self.scene.node(id)).map(|n| n.frame()).reduce(|a, b| a.union(&b));
            if let Some(r) = shown {
                self.camera = self.opening_camera(r);
                self.opening_fit = Some((r, self.camera));
                // `ensure_visible` on the new page left a goal of its own.
                self.camera_goal = None;
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.event_seen = true;
        }
        if matches!(event, WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::KeyboardInput { .. }) {
            self.opening_fit = None;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
                // The window may have moved to another monitor.
                self.update_card_size();
                self.refit_opening();
            }
            WindowEvent::ScaleFactorChanged { .. } => self.camera_moved(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => self.cursor_moved(Vec2::new(position.x as f32, position.y as f32)),
            WindowEvent::CursorLeft { .. } => {
                self.pointer_in = false;
                self.hud_hover = None;
                self.hover = Hit::Board;
                if let Some(id) = self.hit(self.cursor).id()
                    && let (Some(card), Some((x, y))) = (self.cards.get(&id), self.page_point(id, self.cursor)) {
                        card.page.mouse_move(x, y, 0, true);
                    }
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_input(state, button),
            WindowEvent::MouseWheel { delta, phase, .. } => self.wheel(delta, phase),
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => self.key(KeyIn::from_winit(&event)),
            WindowEvent::Ime(ime) => self.ime(ime),
            WindowEvent::Focused(false) => self.mods = ModifiersState::empty(),
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
}
