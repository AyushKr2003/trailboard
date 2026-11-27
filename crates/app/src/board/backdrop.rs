//! The board's photo (*Board background: Photo*): which picture a space
//! shows, and reading it off the main thread. A wallpaper can be several
//! megapixels of WebP; decoding it in a frame would stall the board, so a
//! worker reads the file once (`storage::photo::load`), keeps it, and cuts
//! it to the window's shape (`storage::photo::cover`) whenever the window
//! changes size. Until the picture is ready the board shows its mood.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{channel, Receiver, Sender};

use storage::thumbs::Thumb;

/// Texture key of the photo in the renderer (apart from pages, icons and previews).
pub const TEXTURE: u64 = 1 << 60;

/// The desktop's wallpaper as Omarchy has it now (a link under its state
/// folder), if there is one this module can read.
pub fn desktop_wallpaper() -> Option<PathBuf> {
    let p = std::fs::canonicalize(crate::ui::theme::state_dir().join("background")).ok()?;
    (p.is_file() && storage::photo::is_picture(&p)).then_some(p)
}

/// Folders the photo list offers pictures from: the desktop theme's
/// backgrounds, then the user's pictures (and a wallpapers folder in them).
pub fn picture_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let pictures = std::env::var_os("XDG_PICTURES_DIR").map(PathBuf::from).unwrap_or_else(|| home.join("Pictures"));
    vec![
        crate::ui::theme::state_dir().join("theme/backgrounds"),
        home.join(".config/omarchy/backgrounds"),
        pictures.join("Wallpapers"),
        pictures.join("wallpapers"),
        pictures,
    ]
}

/// What the worker sends back: the file it read and, for the window size
/// asked, the cut picture or why it couldn't.
type Done = (PathBuf, (u32, u32), Result<(Arc<Thumb>, Thumb), String>);

/// The photo in hand and the one wanted.
pub struct Photo {
    /// The file read last and its pixels, kept to cut again on a resize.
    read: Option<(PathBuf, Arc<Thumb>)>,
    /// What the texture holds now: file and window size.
    shown: Option<(PathBuf, (u32, u32))>,
    /// What the worker is doing.
    pending: Option<(PathBuf, (u32, u32))>,
    /// A file that couldn't be read (not tried again until it changes).
    failed: Option<PathBuf>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    /// Bumped with each new picture, for the renderer's upload.
    pub serial: u64,
}

impl Default for Photo {
    fn default() -> Self {
        let (tx, rx) = channel();
        Photo { read: None, shown: None, pending: None, failed: None, tx, rx, serial: 0 }
    }
}

/// What one [`Photo::update`] brings.
#[derive(Default)]
pub struct Update {
    /// A new picture to upload as [`TEXTURE`].
    pub upload: Option<Thumb>,
    /// A file that couldn't be read, and why (once per file).
    pub failed: Option<(PathBuf, String)>,
}

impl Photo {
    /// Whether the texture shows `want` (perhaps cut for another window size
    /// while a new cut is made): the board can draw it.
    pub fn ready(&self, want: &Path) -> bool {
        self.shown.as_ref().is_some_and(|(p, _)| p == want)
    }

    /// Works toward showing `want` at `size` (window px): takes what the
    /// worker finished and starts it on what is still missing.
    pub fn update(&mut self, want: Option<&Path>, size: (u32, u32)) -> Update {
        let mut out = Update::default();
        while let Ok((path, at, result)) = self.rx.try_recv() {
            if self.pending.as_ref().is_some_and(|(p, s)| *p == path && *s == at) {
                self.pending = None;
            }
            match result {
                Ok((full, cut)) => {
                    self.read = Some((path.clone(), full));
                    if want == Some(path.as_path()) {
                        self.shown = Some((path, at));
                        self.serial += 1;
                        out.upload = Some(cut);
                    }
                }
                Err(e) => {
                    log::warn!("board photo {}: {e}", path.display());
                    self.failed = Some(path.clone());
                    out.failed = Some((path, e));
                }
            }
        }
        let Some(want) = want else { return out };
        if size.0 == 0 || size.1 == 0 || self.pending.is_some() || self.failed.as_deref() == Some(want) {
            return out;
        }
        if self.shown.as_ref().is_some_and(|(p, s)| p == want && *s == size) {
            return out;
        }
        let job = (want.to_path_buf(), size);
        self.pending = Some(job.clone());
        let kept = self.read.as_ref().filter(|(p, _)| p == want).map(|(_, t)| t.clone());
        let tx = self.tx.clone();
        let spawned = std::thread::Builder::new().name("board-photo".into()).spawn(move || {
            let started = std::time::Instant::now();
            let full = match kept {
                Some(t) => Ok(t),
                None => storage::photo::load(&job.0).map(Arc::new),
            };
            let result = full.map(|f| {
                let cut = storage::photo::cover(&f, job.1.0, job.1.1);
                (f, cut)
            });
            if let Ok((f, c)) = &result {
                log::info!(
                    "board photo {}: {}×{} → {}×{} in {} ms",
                    job.0.display(),
                    f.width,
                    f.height,
                    c.width,
                    c.height,
                    started.elapsed().as_millis()
                );
            }
            let _ = tx.send((job.0, job.1, result));
            web::wake();
        });
        if let Err(e) = spawned {
            log::warn!("board photo: no worker: {e}");
            self.pending = None;
        }
        out
    }

    /// Forgets a file that failed, so it is tried again (it was chosen anew).
    pub fn retry(&mut self) {
        self.failed = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn picture(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
        let t = Thumb::new(w, h, vec![200; (w * h * 4) as usize]).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, t.encode().unwrap()).unwrap();
        p
    }

    /// Updates until something arrives (or a second passes).
    fn settle(ph: &mut Photo, want: Option<&Path>, size: (u32, u32)) -> Update {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let u = ph.update(want, size);
            if u.upload.is_some() || u.failed.is_some() || Instant::now() > until {
                return u;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn reads_once_and_cuts_again_on_resize() {
        let dir = tempfile::tempdir().unwrap();
        let p = picture(dir.path(), "a.jpg", 400, 200);
        let mut ph = Photo::default();
        assert!(!ph.ready(&p));
        let u = settle(&mut ph, Some(&p), (100, 100));
        let t = u.upload.expect("a picture");
        assert_eq!((t.width, t.height), (100, 100));
        assert!(ph.ready(&p));
        assert!(ph.update(Some(&p), (100, 100)).upload.is_none(), "nothing new at the same size");
        // A resize cuts the kept picture again, even with the file gone.
        std::fs::remove_file(&p).unwrap();
        let t = settle(&mut ph, Some(&p), (200, 50)).upload.expect("cut again");
        assert_eq!((t.width, t.height), (200, 50));
        assert_eq!(ph.serial, 2);
    }

    #[test]
    fn a_bad_file_is_reported_once() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.png");
        std::fs::write(&bad, b"nope").unwrap();
        let mut ph = Photo::default();
        let u = settle(&mut ph, Some(&bad), (10, 10));
        assert_eq!(u.failed.map(|(p, _)| p), Some(bad.clone()));
        std::thread::sleep(Duration::from_millis(50));
        let again = ph.update(Some(&bad), (10, 10));
        assert!(again.failed.is_none() && again.upload.is_none());
        assert!(!ph.ready(&bad));
    }
}
