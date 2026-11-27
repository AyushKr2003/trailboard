//! File chooser.

use super::*;

impl App {
    /// A page opened a file chooser: the path is typed in a field
    /// (`Target::Files`), starting in the folder last chosen from.
    pub(super) fn files_asked(&mut self, page: u64, id: u64, mode: web::dialog::FileMode, default: &str, accept: Vec<String>) {
        if self.node_of(page).is_none_or(|n| !self.cards.contains_key(&n)) {
            web::dialog::choose(id, &[]);
            return;
        }
        if let Some(old) = self.choosing.take() {
            web::dialog::choose(old.id, &[]);
        }
        let home = home_path();
        let suggested = std::path::Path::new(default);
        let name = suggested.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let dir = self.files_dir.clone().filter(|d| d.is_dir()).unwrap_or_else(|| home.clone());
        let start = if suggested.is_absolute() && mode == web::dialog::FileMode::Save {
            crate::pathfield::shorten(suggested, &home)
        } else {
            let mut t = crate::pathfield::shorten(&dir, &home);
            if !t.ends_with('/') {
                t.push('/');
            }
            if mode == web::dialog::FileMode::Save {
                t.push_str(&name);
            }
            t
        };
        log::info!("file chooser {id}: {mode:?}, starting at {start}");
        self.choosing = Some(Choosing { id, mode, accept, name });
        self.menu = None;
        let mut ob = Omnibox::new(&start, Target::Files(id));
        ob.select_all = false;
        self.omnibox = Some(ob);
    }

    /// Enter in the file chooser's field: the path (or, with `*`, the
    /// paths) go to the page if they fit what it asked for; else the field
    /// stays open and a toast says why.
    pub(super) fn choose_files(&mut self, id: u64, text: &str) {
        use web::dialog::FileMode;
        let Some(c) = self.choosing.as_ref().filter(|c| c.id == id) else { return };
        let (mode, name) = (c.mode, c.name.clone());
        let home = home_path();
        let again = |this: &mut Self, msg: String| {
            this.toast(msg);
            let mut ob = Omnibox::new(text, Target::Files(id));
            ob.select_all = false;
            this.omnibox = Some(ob);
        };
        let Some(path) = crate::pathfield::expand(text, &home) else {
            return again(self, "Give a full path, like ~/Documents/file.pdf".to_string());
        };
        let shown = crate::pathfield::shorten(&path, &home);
        let paths: Vec<std::path::PathBuf> = match mode {
            FileMode::Open | FileMode::OpenMany => {
                let picked = crate::pathfield::pick(&path, crate::pathfield::read_dir);
                if picked.is_empty() {
                    return again(self, format!("No files fit {shown}"));
                }
                if picked.len() > 1 && mode == FileMode::Open {
                    return again(self, format!("{} files fit · this page takes one", picked.len()));
                }
                if picked.len() == 1 && picked[0].is_dir() {
                    return again(self, format!("{shown} is a folder · Tab lists what is in it"));
                }
                if let Some(missing) = picked.iter().find(|p| !p.is_file()) {
                    return again(self, format!("No file {}", crate::pathfield::shorten(missing, &home)));
                }
                picked
            }
            FileMode::Folder if path.is_dir() => vec![path],
            FileMode::Folder => return again(self, format!("No folder {shown}")),
            FileMode::Save => {
                let p = if path.is_dir() { path.join(if name.is_empty() { "download" } else { &name }) } else { path };
                if !p.parent().is_some_and(|d| d.is_dir()) {
                    return again(self, format!("No folder for {}", crate::pathfield::shorten(&p, &home)));
                }
                vec![p]
            }
        };
        self.choosing = None;
        let waited = web::dialog::choose(id, &paths);
        log::info!("file chooser {id}: chose {paths:?} (was waiting: {waited})");
        self.files_dir = paths[0].parent().map(|d| if mode == FileMode::Folder { paths[0].clone() } else { d.to_path_buf() });
        match paths.as_slice() {
            [one] => self.toast(format!("Chose {}", one.file_name().map_or_else(|| shown.clone(), |n| n.to_string_lossy().into_owned()))),
            many => self.toast(format!("Chose {} files", many.len())),
        }
    }
}
