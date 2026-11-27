//! User keys.

use scene::keys::{self, Action as KeyAction};
use scene::userkeys::{self, Mode as KeyMode};

use crate::app::{App, PageKeys};
use crate::keyboard::keymap::KeyIn;

impl App {
    /// The user's binding of this key in `m` (Insert, hints, the command
    /// line), if it has one: a single chord there.
    pub(crate) fn user_key(&self, m: KeyMode, ev: &KeyIn) -> Option<KeyAction> {
        let chord = ev.chord(self.mods)?;
        match self.keys.table(m).lookup(&[chord]) {
            keys::Lookup::Exact(a) | keys::Lookup::ExactAndPrefix(a) => Some(a),
            _ => None,
        }
    }

    /// Runs a key action from a line or a binding outside the board and
    /// page tables: in a page if a page has the keys and it is a page key,
    /// else on the board.
    pub(crate) fn run_key_action(&mut self, a: KeyAction, count: Option<u32>) {
        if keys::insert().has(a) {
            self.insert_action(a);
        } else if self.visual_mode().is_some() && keys::visual().has(a) {
            self.visual_action(a, count);
        } else if self.entered() && keys::page().has(a) {
            self.page_action(a, count);
        } else {
            self.board_action(a, count);
        }
    }

    /// A user binding's chain; a count runs it that many times.
    pub(crate) fn run_user_line(&mut self, n: u16, count: Option<u32>) {
        const MAX_REPEAT: u32 = 100;
        let Some((steps, text)) = self.keys.lines.get(n as usize).cloned() else { return };
        log::info!("keys: line {n}: {text}");
        for _ in 0..count.unwrap_or(1).clamp(1, MAX_REPEAT) {
            self.run_steps(&steps, None);
        }
    }

    /// A `:hint` line on the link chosen.
    pub(crate) fn run_hint_line(&mut self, line: &str, url: &str) {
        match userkeys::steps(line, &self.keys.aliases) {
            Ok(steps) => self.run_steps(&steps, Some(url)),
            Err(e) => self.toast(format!("hint: {e}")),
        }
    }

    /// Reads the key file (`storage::keyfile`) over the default keys;
    /// `:bind`s of this run go. Wrong lines are logged, and said when
    /// `announce` (`:reload-keys`) or when there are any.
    pub(crate) fn load_keys(&mut self, announce: bool) {
        let path = storage::keyfile::path();
        let read = match storage::keyfile::load(&path) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("keys: reading {}: {e}", path.display());
                self.toast(format!("Can't read the keys file: {e}"));
                return;
            }
        };
        let (keys, mut errors) = userkeys::Keys::build(&read.entries);
        errors.extend(read.errors);
        errors.sort_by_key(|(n, _)| *n);
        self.keys = keys;
        self.pending.clear();
        for (n, e) in &errors {
            log::warn!("keys: {}:{n}: {e}", path.display());
        }
        let entries = read.entries.len();
        log::info!("keys: {} entries from {}, {} wrong", entries, path.display(), errors.len());
        match errors.as_slice() {
            [] if announce => self.toast(format!("Keys file read · {entries} {}", if entries == 1 { "entry" } else { "entries" })),
            [] => {}
            [(n, e)] => self.toast(format!("Keys file, line {n}: {e}")),
            [(n, e), rest @ ..] => self.toast(format!(
                "Keys file, line {n}: {e} · {} more wrong {} in the log",
                rest.len(),
                if rest.len() == 1 { "line" } else { "lines" }
            )),
        }
    }

    /// `:keys-file`: the key file in a card (made with an example first if
    /// there is none).
    pub(crate) fn open_keys_file(&mut self) {
        let path = storage::keyfile::path();
        match storage::keyfile::ensure(&path) {
            Ok(made) => {
                if made {
                    log::info!("keys: made {}", path.display());
                }
                let url = format!("file://{}", path.display());
                if self.new_card_at_center(&url).is_some() {
                    self.give_keys(PageKeys::Page);
                }
                self.toast(format!("{} · edit it, then :reload-keys", path.display()));
            }
            Err(e) => {
                log::warn!("keys: making {}: {e}", path.display());
                self.toast(format!("Can't make the keys file: {e}"));
            }
        }
    }

    /// `:run`: starts `words[0]` with the rest as its arguments, never
    /// through a shell, in its own process group, with nothing to read and
    /// its output dropped. A thread waits for it so it leaves no zombie.
    pub(crate) fn run_program(&mut self, words: &[String]) {
        use std::os::unix::process::CommandExt;
        let Some((program, args)) = words.split_first() else {
            return self.toast("run needs a program");
        };
        let started = std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .process_group(0)
            .spawn();
        match started {
            Ok(mut child) => {
                log::info!("run: started {program:?} {args:?} (pid {})", child.id());
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                self.toast(format!("Started {program}"));
            }
            Err(e) => {
                log::warn!("run: {program:?}: {e}");
                self.toast(format!("Can't start {program}: {e}"));
            }
        }
    }
}
