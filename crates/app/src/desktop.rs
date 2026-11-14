//! Follows the desktop live, by events rather than polling:
//! - the Omarchy theme and text size, through inotify on the theme's files
//!   (re-armed after every change, since a theme switch replaces files and
//!   directories, and on the nearest existing ancestor, so installing Omarchy
//!   later works without a restart);
//! - Hyprland's window rounding, asked of `hyprctl` at start and again whenever
//!   Hyprland's event socket announces `configreloaded` (it reconnects with
//!   back-off if the socket drops).
//!
//! Watchers run off the main thread and report through a channel that the app
//! drains once per frame; they never touch app or CEF state.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::theme::{self, Theme};

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Something under a watched path changed.
    Files,
    /// Hyprland's `decoration:rounding`.
    Rounding(f32),
}

/// Theme files settle this long after the last change before they are read.
const DEBOUNCE: Duration = Duration::from_millis(80);

pub struct Desktop {
    state: PathBuf,
    user_shell: PathBuf,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    watcher: Option<RecommendedWatcher>,
    /// A file change waiting for the debounce to pass.
    pending: Option<Instant>,
}

impl Desktop {
    /// Starts watching; `start_hyprland` also follows Hyprland's rounding.
    pub fn new(state: PathBuf, user_shell: PathBuf, start_hyprland: bool) -> Self {
        let (tx, rx) = channel();
        let watcher = {
            let tx = tx.clone();
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if res.is_ok() {
                    let _ = tx.send(Event::Files);
                }
            })
            .map_err(|e| log::warn!("can't watch the Omarchy theme: {e}"))
            .ok()
        };
        let mut d = Desktop { state, user_shell, tx, rx, watcher, pending: None };
        d.arm();
        if start_hyprland
            && let Some(socket) = hyprland_socket()
        {
            let tx = d.tx.clone();
            std::thread::Builder::new()
                .name("hyprland-events".into())
                .spawn(move || follow_hyprland(tx, Path::new("hyprctl"), &socket))
                .map_err(|e| log::warn!("can't follow Hyprland: {e}"))
                .ok();
        }
        d
    }

    /// The look as the files say now (rounding stays as it was).
    pub fn read(&self, rounding: f32) -> Theme {
        Theme {
            palette: theme::read_palette(&self.state.join("theme")),
            rounding,
            text_scale: theme::read_base_size(&self.user_shell, &self.state) / 12.0,
        }
    }

    /// (Re)watches the theme paths that exist now.
    fn arm(&mut self) {
        let Some(w) = self.watcher.as_mut() else { return };
        let theme_dir = self.state.join("theme");
        let mut paths = vec![nearest_existing(&self.state), self.state.clone(), theme_dir.clone()];
        let shell_dir = self.user_shell.parent().map(nearest_existing);
        paths.extend(shell_dir);
        if let Some(ancestor) = paths[0].parent() {
            paths.push(ancestor.to_path_buf());
        }
        paths.sort();
        paths.dedup();
        for p in &paths {
            let _ = w.unwatch(p);
            if p.exists() {
                let _ = w.watch(p, RecursiveMode::NonRecursive);
            }
        }
    }

    /// New values since the last call: a new theme when files settled, or a new rounding.
    pub fn poll(&mut self, current: &Theme) -> Option<Theme> {
        let mut rounding = None;
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                Event::Files => self.pending = Some(Instant::now()),
                Event::Rounding(r) => rounding = Some(r),
            }
        }
        let mut next = None;
        if self.pending.is_some_and(|t| t.elapsed() >= DEBOUNCE) {
            self.pending = None;
            self.arm();
            next = Some(self.read(current.rounding));
        }
        if let Some(r) = rounding {
            next.get_or_insert_with(|| current.clone()).rounding = r;
        }
        next.filter(|t| t != current)
    }
}

fn nearest_existing(p: &Path) -> PathBuf {
    let mut p = p.to_path_buf();
    while !p.exists() {
        match p.parent() {
            Some(parent) => p = parent.to_path_buf(),
            None => break,
        }
    }
    p
}

fn hyprland_socket() -> Option<PathBuf> {
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").filter(|s| !s.is_empty())?;
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    Some(PathBuf::from(runtime).join("hypr").join(sig).join(".socket2.sock"))
}

/// Asks `hyprctl` for the rounding; `None` if it fails or says nothing usable.
fn ask_rounding(hyprctl: &Path) -> Option<f32> {
    let out = Command::new(hyprctl).args(["-j", "getoption", "decoration:rounding"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    theme::parse_rounding(&String::from_utf8_lossy(&out.stdout))
}

/// Runs until the app goes away: reports the rounding on every connection to
/// the event socket and after every `configreloaded`.
fn follow_hyprland(tx: Sender<Event>, hyprctl: &Path, socket: &Path) {
    let report = |tx: &Sender<Event>| match ask_rounding(hyprctl) {
        Some(r) => tx.send(Event::Rounding(r)).is_ok(),
        None => true,
    };
    let mut backoff = Duration::from_secs(1);
    let mut asked = false;
    loop {
        if let Ok(stream) = UnixStream::connect(socket) {
            backoff = Duration::from_secs(1);
            asked = true;
            // Asked on every connection, since a reload may have come between.
            if !report(&tx) {
                return;
            }
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                if line.starts_with("configreloaded>>") && !report(&tx) {
                    return;
                }
            }
        } else if !asked {
            // Ask once anyway, so the rounding is right even without the socket.
            asked = true;
            if !report(&tx) {
                return;
            }
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(Duration::from_secs(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    fn fake_hyprctl(dir: &Path, rounding: i32) -> PathBuf {
        let p = dir.join("hyprctl");
        let tmp = dir.join("hyprctl.tmp");
        std::fs::write(&tmp, format!("#!/bin/sh\necho '{{\"option\": \"decoration:rounding\", \"int\": {rounding}, \"set\": true}}'\n")).unwrap();
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::rename(&tmp, &p).unwrap();
        p
    }

    fn next_rounding(rx: &Receiver<Event>) -> f32 {
        match rx.recv_timeout(Duration::from_secs(5)).expect("an event") {
            Event::Rounding(r) => r,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rounding_follows_configreloaded() {
        let dir = tempfile::tempdir().unwrap();
        let hyprctl = fake_hyprctl(dir.path(), 8);
        let sock = dir.path().join("events.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        let (tx, rx) = channel();
        let (h, s) = (hyprctl.clone(), sock.clone());
        std::thread::spawn(move || follow_hyprland(tx, &h, &s));

        let (mut conn, _) = listener.accept().unwrap();
        assert_eq!(next_rounding(&rx), 8.0, "asked on connect");
        // Other events are ignored; a reload asks again.
        fake_hyprctl(dir.path(), 12);
        conn.write_all(b"workspace>>2\nconfigreloaded>>\n").unwrap();
        assert_eq!(next_rounding(&rx), 12.0);

        // A dropped socket is reconnected and asked again.
        drop(conn);
        fake_hyprctl(dir.path(), 3);
        let (_conn, _) = listener.accept().unwrap();
        assert_eq!(next_rounding(&rx), 3.0);
    }

    #[test]
    fn theme_change_is_picked_up_after_debounce() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("current");
        std::fs::create_dir_all(state.join("theme")).unwrap();
        std::fs::write(state.join("theme/colors.toml"), "background = \"#101010\"\n").unwrap();
        let mut d = Desktop::new(state.clone(), dir.path().join("cfg/shell.toml"), false);
        let first = d.read(0.0);
        assert_eq!(first.background(), [0x10, 0x10, 0x10]);

        // A theme switch replaces the directory.
        std::fs::remove_dir_all(state.join("theme")).unwrap();
        std::fs::create_dir_all(state.join("theme")).unwrap();
        std::fs::write(state.join("theme/colors.toml"), "background = \"#fafafa\"\n").unwrap();
        let got = wait_for(&mut d, &first);
        assert_eq!(got.background(), [0xfa, 0xfa, 0xfa]);
        assert!(got.light());

        // The new directory is watched too: editing the file again is seen.
        std::fs::write(state.join("theme/colors.toml"), "background = \"#202020\"\n").unwrap();
        let again = wait_for(&mut d, &got);
        assert_eq!(again.background(), [0x20, 0x20, 0x20]);

        // Text size from a shell.toml that didn't exist when watching started.
        std::fs::create_dir_all(dir.path().join("cfg")).unwrap();
        // The new directory must be watched first: its parent saw it appear.
        let _ = wait_for_any(&mut d, &again);
        std::fs::write(dir.path().join("cfg/shell.toml"), "[font]\nbase-size = 18\n").unwrap();
        let bigger = wait_for(&mut d, &again);
        assert_eq!(bigger.text_scale, 1.5);
    }

    fn wait_for(d: &mut Desktop, current: &Theme) -> Theme {
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            if let Some(t) = d.poll(current) {
                return t;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("no theme change seen");
    }

    /// Lets pending file events settle (re-arming the watches) whether or not the look changed.
    fn wait_for_any(d: &mut Desktop, current: &Theme) -> Option<Theme> {
        let until = Instant::now() + Duration::from_millis(400);
        let mut got = None;
        while Instant::now() < until {
            got = d.poll(current).or(got);
            std::thread::sleep(Duration::from_millis(20));
        }
        got
    }
}
