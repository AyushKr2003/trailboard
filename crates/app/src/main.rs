//! Trailboard: a canvas browser for Linux. Pages live as cards on an infinite
//! board; links open beside the page they came from.

mod app;
mod board;
mod builtin;
mod dev;
mod keyboard;
mod page;
mod system;
mod trails;
mod ui;

use dev::perf;
use system::{crash, launch};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;
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
    if let Some(warning) = launch::engine_mismatch(web::built_engine_version(), web::loaded_engine_version()) {
        log::warn!("{warning}");
    }
    if let Some(node) = web::render_node_override() {
        log::info!("video decoding on {node}: the first render node is NVIDIA's, which has no VA-API");
    }
    if let Some(args) = web::extra_cef_args() {
        log::info!("TRAILBOARD_CEF_ARGS added: {args}");
    }
    if engine.sandbox().is_on() {
        log::info!("{}", engine.sandbox().describe());
    } else {
        log::warn!("{}", engine.sandbox().describe());
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let settings = storage::settings::load_settings(&data_dir());
    let urls = args.iter().filter_map(|a| launch::target(a, &cwd, &settings)).collect();
    let mut event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    // CEF asks for pump work from its own threads: wake the loop for it.
    let proxy = event_loop.create_proxy();
    web::set_waker(Box::new(move || {
        let _ = proxy.send_event(());
    }));
    let mut app = app::App::new(urls, data_dir(), settings);
    // Draw every frame, as before frames were compared (to bisect a missed redraw).
    app.always_redraw = std::env::var("TRAILBOARD_REDRAW").is_ok_and(|v| v == "always");
    if app.always_redraw {
        log::info!("TRAILBOARD_REDRAW=always: drawing every frame");
    }
    app.perf = perf::PerfLog::from_env();
    if app.perf.is_some() {
        log::info!("TRAILBOARD_PERF: logging the cost of drawing once a second");
    }
    if storage::crash::take_unseen(&data_dir()) {
        app.note_crash(&storage::crash::log_path(&data_dir()));
    }
    app.note_sandbox(engine.sandbox().clone());

    // The loop sleeps until something needs doing: a window event, CEF's next
    // work, a page painting, or the app's next deadline. While something
    // moves it draws at the display's rate (paced by the compositor's frame
    // callbacks); a frame that would look the same isn't presented.
    let mut last = Instant::now();
    let mut seen_activity = web::activity();
    // The app's deadline the loop last slept towards (the first pass draws).
    let mut deadline = last;
    let code = loop {
        let event = app.take_event();
        let now = Instant::now();
        if event || engine.next_due() <= now {
            engine.pump();
        }
        // After a rest, the first step of an animation is one frame long.
        let moving = app.animating() || app.always_redraw;
        let dt = if moving { (now - last).as_secs_f32().min(0.1) } else { 1.0 / 60.0 };
        last = now;
        app.tick(dt);
        let activity = web::activity();
        let redraw = moving || event || now >= deadline || activity != seen_activity;
        seen_activity = activity;
        if redraw {
            // Delivered when the compositor is ready for a frame (winit waits
            // for its frame callback), which wakes the wait below.
            app.request_redraw();
            deadline = app.next_deadline(Instant::now());
        } else {
            // Only ever sooner: a timer started on a quiet pass (the idle
            // tick stays counted from the last draw).
            deadline = deadline.min(app.next_deadline(Instant::now()));
        }
        let timeout = engine.next_due().min(deadline).saturating_duration_since(Instant::now());
        if let PumpStatus::Exit(code) = event_loop.pump_app_events(Some(timeout), &mut app) {
            break code;
        }
        if app.quit {
            break 0;
        }
    };
    app.shutdown();
    drop(app);
    engine.shutdown();
    ExitCode::from(code as u8)
}
