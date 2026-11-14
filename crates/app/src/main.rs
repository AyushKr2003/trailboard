//! Trailboard: a canvas browser for Linux. Pages live as cards on an infinite
//! board; links open beside the page they came from.

mod app;
mod clipboard;
mod desktop;
mod hints;
mod keymap;
mod menu;
mod note;
mod omnibox;
mod palette;
mod script;
mod sidebar;
mod theme;

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
    // CEF helper subprocesses re-run this binary; handle them before anything else.
    let engine = match web::start(&cef_dir(), &data_dir().join("browser")) {
        Ok(web::Startup::Subprocess(code)) => return ExitCode::from(code as u8),
        Ok(web::Startup::Browser(engine)) => engine,
        Err(e) => {
            eprintln!("trailboard: could not start Chromium: {e}");
            return ExitCode::FAILURE;
        }
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"))
        .init();

    let initial_url = std::env::args().skip(1).find(|a| !a.starts_with('-'));
    let mut event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = app::App::new(initial_url, data_dir());

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
