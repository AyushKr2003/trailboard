//! Crash reports: when Trailboard panics, a report (time, version, message,
//! place, backtrace) is appended to `$DATA/crash.log`, and `crash-unseen`
//! marks it so the next start can say so once. The log keeps the newest
//! reports within [`MAX_LOG_BYTES`].

use std::fs;
use std::path::{Path, PathBuf};

/// File the reports go to, in the data folder.
pub const LOG_NAME: &str = "crash.log";
const UNSEEN_NAME: &str = "crash-unseen";
/// The log is trimmed to its newest reports within this size.
pub const MAX_LOG_BYTES: usize = 256 * 1024;
/// First line of every report; the log is split on it when trimming.
const HEADER: &str = "=== Trailboard crash ";
/// A backtrace longer than this is cut, so one report can't fill the log.
const MAX_BACKTRACE_BYTES: usize = 48 * 1024;

pub fn log_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LOG_NAME)
}

/// One panic, as written to the log.
pub struct Report<'a> {
    /// Local time, already formatted.
    pub when: &'a str,
    /// `trailboard 0.2.0 (CEF …)`.
    pub version: &'a str,
    /// Thread name, or "unnamed".
    pub thread: &'a str,
    pub message: &'a str,
    /// `file:line:column`, when the panic has one.
    pub location: Option<&'a str>,
    pub backtrace: &'a str,
}

impl Report<'_> {
    pub fn format(&self) -> String {
        let mut bt = self.backtrace.trim_end();
        if bt.len() > MAX_BACKTRACE_BYTES {
            let mut cut = MAX_BACKTRACE_BYTES;
            while !bt.is_char_boundary(cut) {
                cut -= 1;
            }
            bt = &bt[..cut];
        }
        let mut out = format!("{HEADER}{} ===\n", self.when);
        out += &format!("version: {}\n", self.version);
        out += &format!("thread: {}\n", self.thread);
        if let Some(loc) = self.location {
            out += &format!("at: {loc}\n");
        }
        out += &format!("panic: {}\n", self.message.trim_end());
        if !bt.is_empty() {
            out += "backtrace:\n";
            out += bt;
            out += "\n";
        }
        out += "\n";
        out
    }
}

/// `old` with `report` appended, dropping the oldest reports until it fits in
/// `max` bytes. A single report larger than `max` is kept on its own.
pub fn appended(old: &str, report: &str, max: usize) -> String {
    let mut reports: Vec<&str> = split_reports(old);
    reports.push(report);
    let mut total: usize = reports.iter().map(|r| r.len()).sum();
    let mut first = 0;
    while total > max && first + 1 < reports.len() {
        total -= reports[first].len();
        first += 1;
    }
    reports[first..].concat()
}

/// The reports in a log, each with its header line. Text before the first
/// header (a hand-edited or foreign file) is dropped.
fn split_reports(log: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = log.match_indices(HEADER).map(|(i, _)| i).filter(|&i| i == 0 || log.as_bytes()[i - 1] == b'\n').collect();
    starts.push(log.len());
    starts.windows(2).map(|w| &log[w[0]..w[1]]).collect()
}

/// Append `report` to the crash log in `data_dir` and mark it unseen. Called
/// from the panic hook, so it never panics; errors are returned for stderr.
pub fn record(data_dir: &Path, report: &Report) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let path = log_path(data_dir);
    let old = fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let new = appended(&old, &report.format(), MAX_LOG_BYTES);
    crate::write_atomic(&path, new.as_bytes())?;
    fs::write(data_dir.join(UNSEEN_NAME), b"")
}

/// Whether a crash was recorded since the last call; clears the mark.
pub fn take_unseen(data_dir: &Path) -> bool {
    fs::remove_file(data_dir.join(UNSEEN_NAME)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(msg: &str) -> String {
        Report { when: "2026-11-14 10:00:00", version: "trailboard 0.1.0", thread: "main", message: msg, location: Some("a.rs:1:2"), backtrace: "  0: f\n  1: g\n" }
            .format()
    }

    #[test]
    fn report_has_every_part() {
        let r = report("boom");
        assert!(r.starts_with("=== Trailboard crash 2026-11-14 10:00:00 ===\n"));
        assert!(r.contains("version: trailboard 0.1.0\nthread: main\nat: a.rs:1:2\npanic: boom\nbacktrace:\n  0: f\n  1: g\n\n"));
        let r = Report { when: "t", version: "v", thread: "x", message: "m", location: None, backtrace: "" }.format();
        assert_eq!(r, "=== Trailboard crash t ===\nversion: v\nthread: x\npanic: m\n\n");
    }

    #[test]
    fn long_backtrace_is_cut_on_a_char_boundary() {
        let bt = "é".repeat(MAX_BACKTRACE_BYTES);
        let r = Report { when: "t", version: "v", thread: "x", message: "m", location: None, backtrace: &bt }.format();
        assert!(r.len() < MAX_BACKTRACE_BYTES + 200);
    }

    #[test]
    fn append_keeps_newest_within_limit() {
        let (a, b, c) = (report("a"), report("b"), report("c"));
        let log = appended("", &a, 10_000);
        assert_eq!(log, a);
        let log = appended(&log, &b, 10_000);
        assert_eq!(log, format!("{a}{b}"));
        // Room for two: the oldest goes.
        let log = appended(&log, &c, a.len() * 2 + 1);
        assert_eq!(log, format!("{b}{c}"));
        // A report bigger than the limit is still kept.
        assert_eq!(appended(&log, &c, 5), c);
        // Junk before the first header is dropped.
        assert_eq!(appended("hello\n", &a, 10_000), a);
    }

    #[test]
    fn record_and_take_unseen() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("trailboard");
        assert!(!take_unseen(&data));
        let r = Report { when: "t", version: "v", thread: "x", message: "first", location: None, backtrace: "" };
        record(&data, &r).unwrap();
        record(&data, &Report { message: "second", ..r }).unwrap();
        let log = fs::read_to_string(log_path(&data)).unwrap();
        assert!(log.contains("panic: first") && log.contains("panic: second"));
        assert!(take_unseen(&data));
        assert!(!take_unseen(&data));
    }
}
