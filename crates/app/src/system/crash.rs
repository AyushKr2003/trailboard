//! Panic hook: every panic (in the browser process or a CEF helper running
//! this binary) is written to `$DATA/crash.log` after the usual stderr
//! message. Crashes inside Chromium's own code (signals) are not caught here.

use std::backtrace::Backtrace;
use std::path::{Path, PathBuf};

pub fn install(data_dir: PathBuf, cef_dir: &Path) {
    let version = crate::system::launch::version_line(cef_dir);
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let thread = std::thread::current();
        let when = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z").to_string();
        let backtrace = Backtrace::force_capture().to_string();
        let report = storage::crash::Report {
            when: &when,
            version: &version,
            thread: thread.name().unwrap_or("unnamed"),
            message: &message,
            location: location.as_deref(),
            backtrace: &backtrace,
        };
        match storage::crash::record(&data_dir, &report) {
            Ok(()) => eprintln!("trailboard: crash report written to {}", storage::crash::log_path(&data_dir).display()),
            Err(e) => eprintln!("trailboard: could not write the crash report: {e}"),
        }
    }));
}
