//! Starting and stopping CEF: the process handlers, command-line switches, engine versions.

use crate::*;

wrap_app! {
    struct TrailApp;

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefStringUtf16>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(cl) = command_line else { return };
            let browser = process_type.is_none_or(|t| t.to_string().is_empty());
            // Video decoding on the iGPU when NVIDIA's driver holds the first
            // render node (`render_node`); the GPU process inherits it.
            let node_switch = CefString::from("render-node-override");
            if browser && cl.has_switch(Some(&node_switch)) != 1 {
                let nodes = render_node::render_nodes(Path::new("/sys/class/drm"));
                if let Some(node) = render_node::override_for(&nodes) {
                    cl.append_switch_with_value(Some(&node_switch), Some(&CefString::from(node.as_str())));
                    let _ = RENDER_NODE.set(node);
                }
            }
            cl.append_switch(Some(&"no-startup-window".into()));
            cl.append_switch(Some(&"noerrdialogs".into()));
            cl.append_switch(Some(&"hide-crash-restore-bubble".into()));
            cl.append_switch(Some(&"use-mock-keychain".into()));
            cl.append_switch(Some(&"password-store=basic".into()));
            // Thin overlay scrollbars that show while a page scrolls (or the
            // pointer is near them) and fade when idle, instead of the classic
            // grey bar with arrow buttons on every card. They take no width
            // from the page. Pages that style their own scrollbars keep them.
            cl.append_switch_with_value(Some(&"enable-features".into()), Some(&"OverlayScrollbar".into()));
            // A debug knob for trying engine switches without a rebuild
            // (e.g. `--disable-gpu-compositing`); child processes get the
            // ones they need from the browser process.
            if browser {
                let extra = std::env::var("TRAILBOARD_CEF_ARGS").unwrap_or_default();
                let mut added = Vec::new();
                for (name, value) in extra_switches(&extra) {
                    match &value {
                        Some(v) => cl.append_switch_with_value(Some(&name.as_str().into()), Some(&v.as_str().into())),
                        None => cl.append_switch(Some(&name.as_str().into())),
                    }
                    added.push(format!("--{name}{}", value.map(|v| format!("={v}")).unwrap_or_default()));
                }
                if !added.is_empty() {
                    let _ = EXTRA_ARGS.set(added.join(" "));
                }
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(TrailProcess::new())
        }
    }
}

wrap_browser_process_handler! {
    struct TrailProcess;

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            log::info!("chromium context ready");
            prefs::context_ready();
        }

        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            schedule_pump(delay_ms);
        }

        fn on_already_running_app_relaunch(
            &self,
            command_line: Option<&mut CommandLine>,
            current_directory: Option<&CefString>,
        ) -> ::std::os::raw::c_int {
            let Some(cl) = command_line else { return 0 };
            let mut list = CefStringList::new();
            cl.arguments(Some(&mut list));
            let cwd = current_directory.map(|d| d.to_string()).unwrap_or_default();
            emit(WebEvent::Relaunch { args: list.into_iter().collect(), cwd });
            // Handled: we open the URLs ourselves instead of a Chrome window.
            1
        }
    }
}

/// Outcome of [`start`].
pub enum Startup {
    /// This process is a CEF helper (renderer, GPU, …) that has finished; exit with this code.
    Subprocess(i32),
    /// This is the browser process and CEF is initialized.
    Browser(Engine),
    /// Another Trailboard already runs on this profile; Chromium handed it our
    /// command line (it sees [`WebEvent::Relaunch`]). Exit successfully.
    Handed,
}

/// The CEF build this was compiled against, e.g.
/// `156.0.26+g8ea0c73+chromium-156.0.8078.88`. Needs no running CEF.
pub fn engine_version() -> &'static str {
    std::ffi::CStr::from_bytes_with_nul(sys::CEF_VERSION).ok().and_then(|c| c.to_str().ok()).unwrap_or("unknown")
}

pub(crate) static RENDER_NODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
pub(crate) static EXTRA_ARGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The switches `TRAILBOARD_CEF_ARGS` added, if any; for the start log.
pub fn extra_cef_args() -> Option<&'static str> {
    EXTRA_ARGS.get().map(String::as_str)
}

/// The render node Trailboard pointed Chromium's video decoding at, when it
/// did (see `render_node`); for the start log.
pub fn render_node_override() -> Option<&'static str> {
    RENDER_NODE.get().map(String::as_str)
}

/// A CEF version as (major, minor, patch), e.g. (156, 0, 26).
pub type EngineVersion = (i32, i32, i32);

/// The CEF version this was compiled against (the one `install-cef.sh` pins).
pub fn built_engine_version() -> EngineVersion {
    (sys::CEF_VERSION_MAJOR, sys::CEF_VERSION_MINOR, sys::CEF_VERSION_PATCH)
}

// `libcef.so` exports this; the `cef` crate doesn't wrap it.
unsafe extern "C" {
    fn cef_version_info(entry: std::os::raw::c_int) -> std::os::raw::c_int;
}

/// The version of the `libcef.so` actually loaded, which can differ from
/// [`built_engine_version`] when the installed engine wasn't updated with
/// the binary. Asks the library itself, so it needs no running CEF.
pub fn loaded_engine_version() -> EngineVersion {
    // SAFETY: a plain query of constants compiled into libcef; entries 0–2
    // are the CEF major, minor and patch numbers.
    unsafe { (cef_version_info(0), cef_version_info(1), cef_version_info(2)) }
}

/// Must run first in `main`. Handles CEF helper subprocesses (which re-execute
/// this same binary) and initializes CEF in the browser process, where
/// `accept_language` gives the languages pages ask for (`hi-IN,hi,en`; empty
/// for Chromium's own), which Chromium's network stack reads only now.
pub fn start(cef_dir: &Path, cache_dir: &Path, accept_language: impl FnOnce() -> String) -> Result<Startup, String> {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = Args::new();
    let cmd = args.as_cmd_line().ok_or("no command line")?;
    let type_switch = CefString::from("type");
    let is_browser = cmd.has_switch(Some(&type_switch)) != 1;

    let mut app = TrailApp::new();
    let code = execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut());
    if !is_browser {
        return Ok(Startup::Subprocess(code.max(0)));
    }
    if code != -1 {
        return Err(format!("execute_process returned {code} in the browser process"));
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(cache_dir).map_err(|e| e.to_string())?;
    // A running Trailboard owns the profile (this one will hand over to it).
    if !profile_in_use(cache_dir) {
        prefs::forget_saved_zooms(&cache_dir.join("Default"));
    }
    // Chromium looks for `chrome-sandbox` beside the real executable (a
    // `/usr/bin` symlink resolves to `/usr/lib/trailboard`).
    let sandbox = sandbox::detect(exe.parent().unwrap_or(Path::new("/")));
    let settings = Settings {
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        no_sandbox: (!sandbox.is_on()) as i32,
        browser_subprocess_path: CefString::from(exe.to_string_lossy().as_ref()),
        resources_dir_path: CefString::from(cef_dir.to_string_lossy().as_ref()),
        locales_dir_path: CefString::from(cef_dir.join("locales").to_string_lossy().as_ref()),
        root_cache_path: CefString::from(cache_dir.to_string_lossy().as_ref()),
        cache_path: CefString::from(cache_dir.join("Default").to_string_lossy().as_ref()),
        persist_session_cookies: 1,
        accept_language_list: CefString::from(accept_language().as_str()),
        log_severity: LogSeverity::WARNING,
        ..Default::default()
    };
    if initialize(Some(args.as_main_args()), Some(&settings), Some(&mut app), std::ptr::null_mut()) != 1 {
        if get_exit_code() == sys::cef_resultcode_t::CEF_RESULT_CODE_NORMAL_EXIT_PROCESS_NOTIFIED as i32 {
            return Ok(Startup::Handed);
        }
        return Err(format!("CEF initialize failed (exit code {})", get_exit_code()));
    }
    Ok(Startup::Browser(Engine { sandbox }))
}

/// Whether a live Chromium holds the profile in `cache_dir`: its
/// `SingletonLock` is a link to `host-pid`, and that process exists.
pub(crate) fn profile_in_use(cache_dir: &Path) -> bool {
    let Ok(target) = std::fs::read_link(cache_dir.join("SingletonLock")) else { return false };
    let target = target.to_string_lossy();
    let pid = target.rsplit('-').next().and_then(|p| p.parse::<u32>().ok());
    pid.is_none_or(|pid| Path::new(&format!("/proc/{pid}")).exists())
}

/// Handle to an initialized CEF. Call [`Engine::shutdown`] after every page is dropped.
pub struct Engine {
    sandbox: sandbox::Sandbox,
}

impl Engine {
    /// How Chromium's child processes are confined.
    pub fn sandbox(&self) -> &sandbox::Sandbox {
        &self.sandbox
    }

    /// Run pending CEF work. Call when [`Engine::next_due`] has come, and
    /// after input was sent to a page.
    pub fn pump(&self) {
        // Work done now may ask for more (it lowers this again); otherwise
        // the next pump is the fallback one.
        PUMP_DUE.store(nanos_at(std::time::Instant::now() + MAX_PUMP_DELAY), Ordering::Release);
        do_message_loop_work();
    }

    /// When CEF next wants [`Engine::pump`] (in the past: now).
    pub fn next_due(&self) -> std::time::Instant {
        epoch() + std::time::Duration::from_nanos(PUMP_DUE.load(Ordering::Acquire))
    }

    pub fn shutdown(self) {
        // Let close requests settle before tearing down.
        for _ in 0..20 {
            do_message_loop_work();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        cef::shutdown();
    }
}

/// The switches in `TRAILBOARD_CEF_ARGS`: whitespace-separated `--name` or
/// `--name=value` (the dashes are optional), as (name, value).
pub(crate) fn extra_switches(s: &str) -> Vec<(String, Option<String>)> {
    s.split_whitespace()
        .filter_map(|w| {
            let w = w.trim_start_matches('-');
            let (name, value) = match w.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (w, None),
            };
            (!name.is_empty()).then(|| (name.to_string(), value))
        })
        .collect()
}
