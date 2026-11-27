//! Palette.

use super::*;

impl App {
    /// Ctrl+K: the palette, over everything but Solo's page.
    pub(super) fn open_palette(&mut self) {
        self.menu = None;
        self.link_hints = None;
        self.finish_arrange(true);
        self.omnibox = Some(Omnibox::new("", Target::Palette));
        log::info!("palette open");
    }

    /// `:`: the palette as a command line (`scene::command`).
    /// `:` (a count before it starts the line: `3:` → `:3 `).
    pub(super) fn open_command_line(&mut self, count: Option<u32>) {
        self.open_palette();
        let mut ob = Omnibox::new("", Target::Palette);
        ob.set(&count.map_or_else(|| ":".to_string(), |n| format!(":{n} ")));
        self.omnibox = Some(ob);
        log::info!("command line open");
    }

    /// `.`: the last command line again; a count replaces the one it had.
    pub(super) fn repeat_line(&mut self, count: Option<u32>) {
        let Some(line) = self.last_line.clone().or_else(|| self.recent_commands.lines().first().cloned()) else {
            return self.toast("No command line run yet");
        };
        let line = match count {
            Some(n) => format!("{n} {}", scene::command::split_count(&line).1),
            None => line,
        };
        log::info!("command line: repeat {line:?}");
        self.run_line(&line);
    }

    /// `:later` lines whose time has come.
    pub(super) fn run_due_later(&mut self) {
        let now = Instant::now();
        if !self.later.iter().any(|l| l.at <= now) {
            return;
        }
        let (due, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.later).into_iter().partition(|l| l.at <= now);
        self.later = wait;
        for l in due {
            self.run_later(l);
        }
    }

    pub(super) fn run_later(&mut self, l: LaterLine) {
        log::info!("command line: later {:?}", l.line);
        match userkeys::steps(&l.line, &self.keys.aliases) {
            Ok(steps) => self.run_steps(&steps, l.hint_url.as_deref()),
            Err(e) => self.toast(format!("later: {e}")),
        }
    }
}
