//! Trailboard: a canvas browser for Linux. Pages live as cards on an infinite
//! board; links open beside the page they came from.

mod app;
mod backdrop;
mod blocking;
mod chrome;
mod clipboard;
mod crash;
mod desktop;
mod editor;
mod engines;
mod hints;
mod hud;
mod icons;
mod keymap;
mod launch;
mod menu;
mod note;
mod omnibox;
mod palette;
mod pathfield;
mod script;
mod settings_page;
mod sidebar;
mod start;
mod theme;
mod welcome;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};

fn cef_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("CEF_PATH") {
        return PathBuf::from(p);
    }
    // Bundled layout: CEF files next to the executable.
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from));
    if let Some(dir) = exe_dir.filter(|d| d.join("libcef.so").exists()) {
        return dir;
    }
    dirs_home().join(".local/share/cef")
}

fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs_home().join(".local/share"))
        .join("trailboard")
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn main() -> ExitCode {
    // Before CEF: a running Trailboard would otherwise be handed `--version`.
    let args: Vec<String> = std::env::args().skip(1).collect();
    match launch::command(&args) {
        launch::Command::Version => {
            println!("{}", launch::version_line(&cef_dir()));
            return ExitCode::SUCCESS;
        }
        launch::Command::Help => {
            print!("{}", launch::USAGE);
            return ExitCode::SUCCESS;
        }
        launch::Command::Run => {}
    }
    crash::install(data_dir(), &cef_dir());
    // CEF helper subprocesses re-run this binary; handle them before anything else.
    // The languages pages ask for: Chromium's network stack reads them only at start.
    let languages = || {
        let s = storage::settings::load_settings(&data_dir());
        scene::settings::accept_languages(&scene::settings::language_tag(s.language, &app::system_locale()))
    };
    let engine = match web::start(&cef_dir(), &data_dir().join("browser"), languages) {
        Ok(web::Startup::Subprocess(code)) => return ExitCode::from(code as u8),
        Ok(web::Startup::Browser(engine)) => engine,
        Ok(web::Startup::Handed) => {
            // A launcher (or a link from another app) while we already run:
            // the running window opens the URLs.
            eprintln!("trailboard: opened in the running Trailboard");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("trailboard: could not start Chromium: {e}");
            return ExitCode::FAILURE;
        }
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"))
        .init();
    log::info!("{}", launch::version_line(&cef_dir()));
    if engine.sandbox().is_on() {
        log::info!("{}", engine.sandbox().describe());
    } else {
        log::warn!("{}", engine.sandbox().describe());
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let settings = storage::settings::load_settings(&data_dir());
    let urls = args.iter().filter_map(|a| launch::target(a, &cwd, &settings)).collect();
    let mut event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = app::App::new(urls, data_dir(), settings);
    if storage::crash::take_unseen(&data_dir()) {
        app.note_crash(&storage::crash::log_path(&data_dir()));
    }
    app.note_sandbox(engine.sandbox().clone());

    let mut last = Instant::now();
    let code = loop {
        engine.pump();
        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.1);
        last = now;
        app.tick(dt);
        // Rendering waits for vsync inside the redraw, which paces this loop.
        if let PumpStatus::Exit(code) = event_loop.pump_app_events(Some(Duration::ZERO), &mut app) {
            break code;
        }
        if app.quit {
            break 0;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    app.shutdown();
    drop(app);
    engine.shutdown();
    ExitCode::from(code as u8)
}
