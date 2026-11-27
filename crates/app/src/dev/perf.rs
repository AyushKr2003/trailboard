//! `TRAILBOARD_PERF=1`: once a second (drawing or not), log what one
//! second of drawing cost: page paints from CEF, bytes copied out of CEF's
//! buffers and uploaded to the GPU, frames presented (and how many were
//! partial), and time spent in the renderer. A debug knob for M16 step 6.

use std::time::{Duration, Instant};

/// Running totals, as the counters in `web` and `compositor` keep them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub paints: u64,
    pub copied: u64,
    pub uploaded: u64,
    pub presented: u64,
    pub partial: u64,
    pub skipped: u64,
    pub render: Duration,
}

pub struct PerfLog {
    since: Instant,
    last: Totals,
}

impl PerfLog {
    pub fn from_env() -> Option<Self> {
        std::env::var("TRAILBOARD_PERF").is_ok_and(|v| !v.is_empty() && v != "0").then(|| Self::new(Instant::now()))
    }

    fn new(now: Instant) -> Self {
        Self { since: now, last: Totals::default() }
    }

    /// When the next line is due.
    pub fn due(&self) -> Instant {
        self.since + Duration::from_secs(1)
    }

    /// The line for the second (or more) that ended at `now`, if one has;
    /// the totals are then counted from here.
    pub fn tick(&mut self, now: Instant, t: Totals) -> Option<String> {
        let span = now.saturating_duration_since(self.since);
        if span < Duration::from_secs(1) {
            return None;
        }
        let line = line(span, &self.last, &t);
        self.since = now;
        self.last = t;
        Some(line)
    }
}

fn line(span: Duration, a: &Totals, b: &Totals) -> String {
    let s = span.as_secs_f64();
    let per = |x: u64, y: u64| y.saturating_sub(x) as f64 / s;
    let mb = |x: u64, y: u64| per(x, y) / 1e6;
    let presented = b.presented.saturating_sub(a.presented);
    let render = b.render.saturating_sub(a.render);
    let avg_ms = if presented > 0 { render.as_secs_f64() * 1e3 / presented as f64 } else { 0.0 };
    format!(
        "perf: paints {:.1}/s copied {:.1} MB/s uploaded {:.1} MB/s presented {:.1}/s (partial {:.1}/s) skipped {:.1}/s render {:.1} ms/frame, {:.0} % of the time",
        per(a.paints, b.paints),
        mb(a.copied, b.copied),
        mb(a.uploaded, b.uploaded),
        per(a.presented, b.presented),
        per(a.partial, b.partial),
        per(a.skipped, b.skipped),
        avg_ms,
        render.as_secs_f64() / s * 100.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_due_a_second_after_the_last_line() {
        let t0 = Instant::now();
        let mut p = PerfLog::new(t0);
        assert_eq!(p.due(), t0 + Duration::from_secs(1));
        let later = t0 + Duration::from_millis(1500);
        assert!(p.tick(later, Totals::default()).is_some());
        assert_eq!(p.due(), later + Duration::from_secs(1));
    }

    #[test]
    fn logs_once_a_second_with_rates() {
        let t0 = Instant::now();
        let mut p = PerfLog::new(t0);
        let t = Totals { paints: 60, copied: 60 * 8_000_000, uploaded: 30 * 8_000_000, presented: 30, partial: 10, skipped: 5, render: Duration::from_millis(300) };
        assert_eq!(p.tick(t0 + Duration::from_millis(500), t), None);
        let l = p.tick(t0 + Duration::from_secs(2), t).unwrap();
        assert!(l.contains("paints 30.0/s"), "{l}");
        assert!(l.contains("copied 240.0 MB/s"), "{l}");
        assert!(l.contains("uploaded 120.0 MB/s"), "{l}");
        assert!(l.contains("presented 15.0/s (partial 5.0/s)"), "{l}");
        assert!(l.contains("render 10.0 ms/frame, 15 %"), "{l}");
        // Counted from the last line on: nothing new, all zero.
        let l = p.tick(t0 + Duration::from_secs(3), t).unwrap();
        assert!(l.contains("paints 0.0/s") && l.contains("render 0.0 ms/frame"), "{l}");
    }
}
