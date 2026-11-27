//! Arrange.

use scene::arrange::Arrange;
use scene::nav::Dir;
use scene::selection;
use winit::keyboard::KeyCode;

use crate::app::{App, ARRANGE_FINE, ARRANGE_STEP, MIN_CARD};

impl App {
    /// M: move and resize the current card (else the one in the middle of the
    /// view) with h/j/k/l until Enter, M or Esc. A selected card brings the
    /// rest of the selection along when it moves.
    pub(crate) fn start_arrange(&mut self) {
        if self.read.is_some() {
            return;
        }
        let Some(id) = self.current() else {
            self.toast("No card here to move");
            return;
        };
        if !self.selection.contains(id) {
            self.selection.clear();
        }
        self.focus(Some(id));
        let group = selection::drag_group(&self.scene, &self.selection, id);
        self.arrange = Arrange::new(&self.scene, id, group);
        self.ensure_visible(id);
        log::info!("arranging card {}", id.0);
    }

    /// Ends arranging: `keep` records the moves as one undo step, else every
    /// card goes back where it was.
    pub(crate) fn finish_arrange(&mut self, keep: bool) {
        let Some(a) = self.arrange.take() else { return };
        if keep {
            self.history.moved_many(&self.scene, &a.start);
        } else {
            a.cancel(&mut self.scene);
        }
        self.dirty = true;
        log::info!("arranged card {}: {}", a.id.0, if keep { "kept" } else { "cancelled" });
    }

    /// A key while arranging: h/j/k/l or arrows move, with Shift resize, with
    /// Ctrl take a fine step; Enter, Space or M keep, Esc cancels. Returns
    /// false for any other key, which keeps the arrangement and goes on as usual.
    pub(crate) fn arrange_key(&mut self, code: Option<KeyCode>) -> bool {
        let Some(a) = self.arrange.as_ref() else { return false };
        if a.is_stale(&self.scene) {
            self.arrange = None;
            return false;
        }
        let dir = match code {
            Some(KeyCode::KeyH | KeyCode::ArrowLeft) => Dir::Left,
            Some(KeyCode::KeyJ | KeyCode::ArrowDown) => Dir::Down,
            Some(KeyCode::KeyK | KeyCode::ArrowUp) => Dir::Up,
            Some(KeyCode::KeyL | KeyCode::ArrowRight) => Dir::Right,
            Some(KeyCode::Escape) => {
                self.finish_arrange(false);
                return true;
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space | KeyCode::KeyM) => {
                self.finish_arrange(true);
                return true;
            }
            _ => {
                self.finish_arrange(true);
                return false;
            }
        };
        let px = if self.mods.control_key() { ARRANGE_FINE } else { ARRANGE_STEP };
        let step = px / self.camera.zoom.max(1e-3);
        if self.mods.shift_key() {
            a.resize(&mut self.scene, dir, step, MIN_CARD);
        } else {
            a.nudge(&mut self.scene, dir, step);
        }
        let id = a.id;
        self.dirty = true;
        self.ensure_visible(id);
        true
    }
}
