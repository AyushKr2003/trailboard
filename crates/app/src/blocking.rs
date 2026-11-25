//! Ad and tracker blocking, the app's side: keeps the filter lists fresh
//! (fetching stale ones in the background, with the parts they `!#include`,
//! see `scene::privacy`), hands them to `web::shield`, and keeps the sites
//! where the user turned blocking off.
//! `filters/user.txt` is watched (inotify, off the main thread), so edits to
//! the user's own rules take effect without a restart.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};

use notify::event::{AccessKind, AccessMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use scene::privacy::{self, AllowedSites, FilterList, LISTS, USER_LIST};

/// Tag prefix of the fetches made here.
const TAG: &str = "filters:";
/// How often the lists' ages are looked at while running.
const CHECK_EVERY: Duration = Duration::from_secs(600);
/// The first look waits a little, so the first pages load first.
const FIRST_CHECK: Duration = Duration::from_secs(3);
/// `filters/user.txt` is read again this long after its last change, so a
/// save in several writes is read once, whole.
const USER_SETTLE: Duration = Duration::from_millis(300);

pub struct Blocking {
    data_dir: PathBuf,
    /// Read by `web::shield` on Chromium's IO thread.
    allowed: Arc<RwLock<AllowedSites>>,
    /// Sites allowed from a private space: in force until quitting, never saved.
    unsaved: HashSet<String>,
    next_check: Instant,
    /// Lists and parts being fetched, by file id.
    fetching: HashSet<String>,
    failed: HashMap<String, Instant>,
    /// A list or part was saved; the rules are rebuilt once no fetch is left.
    saved: bool,
    /// Watches the filters folder; dropped, it stops.
    _watcher: Option<RecommendedWatcher>,
    /// A note per change to `filters/user.txt`.
    user_changes: Option<Receiver<()>>,
    /// The last change to `filters/user.txt`, not yet read.
    user_pending: Option<Instant>,
    /// `filters/user.txt` as last given to the rules (`None`: no file).
    user_text: Option<String>,
}

impl Blocking {
    /// Loads the allowed sites and the saved lists and starts blocking (if `on`).
    pub fn start(data_dir: PathBuf, on: bool) -> Self {
        let allowed = Arc::new(RwLock::new(storage::privacy::load_allowed(&data_dir)));
        let read = allowed.clone();
        web::shield::set_allowed(move |url| read.read().unwrap_or_else(|e| e.into_inner()).allows(url));
        web::shield::set_enabled(on);
        let (watcher, user_changes) = watch_user_list(&data_dir).unzip();
        let mut b = Blocking {
            data_dir,
            allowed,
            unsaved: HashSet::new(),
            next_check: Instant::now() + FIRST_CHECK,
            fetching: HashSet::new(),
            failed: HashMap::new(),
            saved: false,
            _watcher: watcher,
            user_changes,
            user_pending: None,
            user_text: None,
        };
        b.reload_rules();
        b
    }

    fn reload_rules(&mut self) {
        let lists = storage::privacy::load_lists(&self.data_dir);
        if lists.is_empty() {
            log::info!("blocking: no filter lists yet");
        }
        self.user_text = lists.iter().find(|(id, _)| id == USER_LIST).map(|(_, t)| t.clone());
        web::shield::load_rules(lists);
    }

    /// Fetches the lists that are missing or old (looked at every few
    /// minutes), and reads `filters/user.txt` again once a change to it has
    /// settled. Returns what to tell the user when their rules were reloaded;
    /// `on` is the *Block ads and trackers* setting.
    pub fn tick(&mut self, now: Instant, on: bool) -> Option<String> {
        let told = self.follow_user_list(now, on);
        if now >= self.next_check {
            self.fetch_stale(now);
        }
        told
    }

    fn follow_user_list(&mut self, now: Instant, on: bool) -> Option<String> {
        if let Some(rx) = &self.user_changes {
            while rx.try_recv().is_ok() {
                self.user_pending = Some(now);
            }
        }
        if self.user_pending.is_none_or(|t| now < t + USER_SETTLE) {
            return None;
        }
        self.user_pending = None;
        let text = storage::privacy::load_user_list(&self.data_dir);
        if text == self.user_text {
            return None; // touched, or saved unchanged
        }
        let rules = text.as_deref().map(privacy::count_rules);
        log::info!("blocking: filters/user.txt changed ({}); rebuilding rules", rules.map_or("removed".into(), |n| format!("{n} rule(s)")));
        self.reload_rules();
        Some(privacy::user_rules_message(rules, on))
    }

    fn fetch_stale(&mut self, now: Instant) {
        self.next_check = now + CHECK_EVERY;
        for list in LISTS {
            self.fetch_if_stale(now, list.id, list.url, list.refresh);
            self.fetch_parts(now, &list);
        }
    }

    /// Fetches the parts list `list` (as saved) includes that are missing or old.
    fn fetch_parts(&mut self, now: Instant, list: &FilterList) {
        let Some(text) = storage::privacy::load_list(&self.data_dir, list.id) else {
            return;
        };
        for name in privacy::includes(&text) {
            self.fetch_if_stale(
                now,
                &privacy::include_id(list.id, &name),
                &privacy::include_url(list.url, &name),
                list.refresh,
            );
        }
    }

    fn fetch_if_stale(&mut self, now: Instant, id: &str, url: &str, refresh: u64) {
        let retry_at = self
            .failed
            .get(id)
            .map(|&t| t + Duration::from_secs(privacy::RETRY_SECS));
        if self.fetching.contains(id) || retry_at.is_some_and(|t| now < t) {
            return;
        }
        let age = storage::privacy::list_age(&self.data_dir, id, SystemTime::now());
        if privacy::is_stale(age, refresh) && web::shield::fetch(url, &format!("{TAG}{id}")) {
            self.fetching.insert(id.to_string());
        }
    }

    /// Handles a finished fetch if it was one of ours; returns whether it was.
    pub fn fetched(&mut self, tag: &str, ok: bool, body: &[u8]) -> bool {
        let Some(id) = tag.strip_prefix(TAG) else {
            return false;
        };
        // A list, or a part of one (`list--part`).
        let list = LISTS.iter().find(|l| l.id == id);
        let part_of = LISTS.iter().find(|l| {
            id.strip_prefix(l.id)
                .is_some_and(|rest| rest.starts_with("--"))
        });
        if list.is_none() && part_of.is_none() {
            return false;
        }
        self.fetching.remove(id);
        let text = String::from_utf8_lossy(body);
        let looks_right = if list.is_some() {
            privacy::looks_like_list(&text)
        } else {
            privacy::looks_like_part(&text)
        };
        if !ok || !looks_right {
            log::warn!(
                "blocking: couldn't fetch {id} ({}); trying again later",
                if ok { "not a filter list" } else { "network" }
            );
            self.failed.insert(id.to_string(), Instant::now());
        } else {
            self.failed.remove(id);
            match storage::privacy::save_list(&self.data_dir, id, &text) {
                Ok(()) => self.saved = true,
                Err(e) => log::warn!("blocking: saving {id}: {e}"),
            }
            // A list just fetched: its parts too.
            if let Some(list) = list {
                self.fetch_parts(Instant::now(), list);
            }
        }
        // One rebuild once the whole batch is in, not one per list.
        if self.saved && self.fetching.is_empty() {
            self.saved = false;
            self.reload_rules();
        }
        true
    }

    /// Blocking is off on `url`'s site.
    pub fn allows(&self, url: &str) -> bool {
        self.allowed.read().unwrap_or_else(|e| e.into_inner()).allows(url)
    }

    /// Turns blocking off on `url`'s site, or back on. From a `private`
    /// space, a site allowed is not saved. Returns the site and whether
    /// blocking is now on there; `None` if `url` isn't a web page.
    pub fn toggle(&mut self, url: &str, private: bool) -> Option<(String, bool)> {
        let (site, on) = self.allowed.write().unwrap_or_else(|e| e.into_inner()).toggle(url)?;
        if on {
            self.unsaved.remove(&site);
        } else if private {
            self.unsaved.insert(site.clone());
        }
        self.save_allowed();
        log::info!("blocking {} {site}", if on { "back on for" } else { "off for" });
        Some((site, on))
    }

    /// The sites where blocking is off, sorted, each with whether it is
    /// only until quitting (allowed from a private space).
    pub fn allowed_sites(&self) -> Vec<(String, bool)> {
        let all = self.allowed.read().unwrap_or_else(|e| e.into_inner());
        all.sites.iter().map(|s| (s.clone(), self.unsaved.contains(s))).collect()
    }

    /// Turns blocking back on for `site` (an entry of `allowed_sites`).
    /// Returns whether it was off there.
    pub fn block_again(&mut self, site: &str) -> bool {
        if !self.allowed.write().unwrap_or_else(|e| e.into_inner()).remove(site) {
            return false;
        }
        self.unsaved.remove(site);
        self.save_allowed();
        log::info!("blocking back on for {site}");
        true
    }

    /// Saves the allowed sites, leaving out those allowed from a private space.
    fn save_allowed(&self) {
        let saved = {
            let all = self.allowed.read().unwrap_or_else(|e| e.into_inner());
            AllowedSites { sites: all.sites.iter().filter(|s| !self.unsaved.contains(*s)).cloned().collect() }
        };
        if let Err(e) = storage::privacy::save_allowed(&self.data_dir, &saved) {
            log::warn!("blocking: saving allowed sites: {e}");
        }
    }
}

/// Watches the filters folder (made if missing, so it can be watched) for
/// changes to `filters/user.txt`. Reads of the file are not changes, or
/// reading it again would wake the watcher again.
fn watch_user_list(data_dir: &std::path::Path) -> Option<(RecommendedWatcher, Receiver<()>)> {
    let dir = storage::privacy::filters_dir(data_dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("blocking: can't make {}: {e}", dir.display());
        return None;
    }
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(ev) = res else { return };
        let read = matches!(ev.kind, EventKind::Access(a) if a != AccessKind::Close(AccessMode::Write));
        if !read && ev.paths.iter().any(|p| storage::privacy::is_user_list(p)) {
            let _ = tx.send(());
            web::wake();
        }
    })
    .map_err(|e| log::warn!("blocking: can't watch {}: {e}", dir.display()))
    .ok()?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| log::warn!("blocking: can't watch {}: {e}", dir.display()))
        .ok()?;
    Some((watcher, rx))
}
