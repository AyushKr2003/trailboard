//! Media controls (M13 step 5): what `:media` and the `,` keys ask of a
//! card's video or audio, and how the page's answer is said. Our own design.
//!
//! [`parse`] reads a `:media` argument into an [`Op`]. The page's script
//! (`web::media`) acts on the media it picks and answers with a line that
//! [`Status::parse`] reads; [`Status::describe`] turns it into the toast.

/// The largest jump a key or command makes, in seconds (ten hours).
const MAX_JUMP: i64 = 36_000;

/// The playback rates `faster` / `slower` step through, in percent.
pub const RATES: &[u16] = &[25, 50, 75, 100, 125, 150, 175, 200, 250, 300, 400];

/// The slowest and fastest rates `speed` accepts, in percent.
pub const MIN_RATE: u16 = 10;
pub const MAX_RATE: u16 = 1600;

/// Where an absolute seek goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum At {
    Seconds(u32),
    /// Of the duration, 0–100.
    Percent(u8),
}

/// One thing to do to the card's media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    /// Play if paused, else pause.
    Toggle,
    Play,
    Pause,
    /// Jump by this many seconds (back if negative).
    Seek(i32),
    SeekTo(At),
    /// The next / previous rate in [`RATES`].
    Faster,
    Slower,
    /// This rate, in percent (100: normal).
    Speed(u16),
    /// The card's sound off or on (the card, not the element: `:mute`).
    Mute,
    /// Play again from the start when it ends, or not.
    Loop,
    /// The video full window (the page's full screen, which Solo shows).
    Full,
}

/// The words `:media` takes, as completion offers them.
pub const WORDS: &[&str] = &["toggle", "play", "pause", "forward", "back", "seek", "faster", "slower", "speed", "mute", "loop", "full"];

/// How `:media` is written in the command list.
pub const HINT: &str = "<toggle|play|pause|forward [s]|back [s]|seek <time|%>|faster|slower|speed [rate]|mute|loop|full>";

/// The seconds `forward` / `back` jump without a number.
pub const STEP: i32 = 10;

impl Op {
    /// The op `count` times over (a key's count): jumps go further; other
    /// ops don't repeat.
    pub fn times(self, count: u32) -> Op {
        match self {
            Op::Seek(s) => Op::Seek((i64::from(s) * i64::from(count.max(1))).clamp(-MAX_JUMP, MAX_JUMP) as i32),
            other => other,
        }
    }

    /// The name the page's script knows the op by, and its number.
    pub fn wire(self) -> (&'static str, f64) {
        match self {
            Op::Toggle => ("toggle", 0.0),
            Op::Play => ("play", 0.0),
            Op::Pause => ("pause", 0.0),
            Op::Seek(s) => ("seek", f64::from(s)),
            Op::SeekTo(At::Seconds(s)) => ("seekto", f64::from(s)),
            Op::SeekTo(At::Percent(p)) => ("seekpct", f64::from(p)),
            Op::Faster => ("rate", next_rate(true)),
            Op::Slower => ("rate", next_rate(false)),
            Op::Speed(r) => ("rate", f64::from(r) / 100.0),
            Op::Mute => ("mute", 0.0),
            Op::Loop => ("loop", 0.0),
            Op::Full => ("full", 0.0),
        }
    }
}

/// `faster` / `slower` don't know the current rate here; the script picks
/// the step from it. These sentinels say which way (`rate` with a negative
/// number: up for -1, down for -2).
fn next_rate(up: bool) -> f64 {
    if up { -1.0 } else { -2.0 }
}

/// The next rate in [`RATES`] above (or below) `now`, in percent.
pub fn step_rate(now: u16, up: bool) -> u16 {
    if up {
        RATES.iter().copied().find(|&r| r > now).unwrap_or(*RATES.last().expect("rates"))
    } else {
        RATES.iter().rev().copied().find(|&r| r < now).unwrap_or(RATES[0])
    }
}

/// Reads `:media`'s argument. A unique start of a word is enough (`fo 30`).
pub fn parse(arg: &str) -> Result<Op, String> {
    let mut words = arg.split_whitespace();
    let Some(first) = words.next() else { return Err(format!("media needs {HINT}")) };
    let rest: Vec<&str> = words.collect();
    let word = pick(first)?;
    let value = match rest.as_slice() {
        [] => None,
        [v] => Some(*v),
        _ => return Err(format!("media {word}: too many words")),
    };
    let no_value = |op: Op| match value {
        None => Ok(op),
        Some(v) => Err(format!("media {word} takes nothing after it (“{v}”)")),
    };
    match word {
        "toggle" => no_value(Op::Toggle),
        "play" => no_value(Op::Play),
        "pause" => no_value(Op::Pause),
        "faster" => no_value(Op::Faster),
        "slower" => no_value(Op::Slower),
        "mute" => no_value(Op::Mute),
        "loop" => no_value(Op::Loop),
        "full" => no_value(Op::Full),
        "forward" | "back" => {
            let s = match value {
                None => STEP,
                Some(v) => seconds(v).filter(|&s| s > 0).ok_or_else(|| format!("media {word}: “{v}” isn't a time (30, 1:30)"))? as i32,
            };
            Ok(Op::Seek(if word == "back" { -s } else { s }))
        }
        "seek" => {
            let v = value.ok_or_else(|| "media seek needs a time (1:30, 90, 50%, +10, -10)".to_string())?;
            seek(v).ok_or_else(|| format!("media seek: “{v}” isn't a time (1:30, 90, 50%, +10, -10)"))
        }
        "speed" => match value {
            None => Ok(Op::Speed(100)),
            Some(v) => rate(v).map(Op::Speed).ok_or_else(|| format!("media speed: “{v}” isn't a rate from 0.1 to 16 (1.5, 150%)")),
        },
        _ => unreachable!("pick gives a word of WORDS"),
    }
}

/// The word of [`WORDS`] that `typed` is, or starts uniquely.
fn pick(typed: &str) -> Result<&'static str, String> {
    let typed = typed.to_lowercase();
    if let Some(w) = WORDS.iter().find(|w| **w == typed) {
        return Ok(w);
    }
    let fits: Vec<&'static str> = WORDS.iter().copied().filter(|w| w.starts_with(&typed)).collect();
    match fits.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("media: no “{typed}” ({})", WORDS.join(", "))),
        many => Err(format!("media: could be {}", many.join(", "))),
    }
}

/// A time: `90`, `1:30`, `1:02:03`.
fn seconds(v: &str) -> Option<u32> {
    let parts: Vec<&str> = v.split(':').collect();
    if parts.len() > 3 || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) || p.len() > 6) {
        return None;
    }
    let mut total: u64 = 0;
    for (i, p) in parts.iter().enumerate() {
        let n: u64 = p.parse().ok()?;
        // Minutes and seconds after the first part stay under 60.
        if i > 0 && n >= 60 {
            return None;
        }
        total = total * 60 + n;
    }
    u32::try_from(total).ok().filter(|&t| i64::from(t) <= MAX_JUMP * 10)
}

/// A seek's value: absolute (`1:30`, `50%`) or relative (`+10`, `-1:00`).
fn seek(v: &str) -> Option<Op> {
    if let Some(p) = v.strip_suffix('%') {
        let p: u8 = p.parse().ok()?;
        return (p <= 100).then_some(Op::SeekTo(At::Percent(p)));
    }
    if let Some(r) = v.strip_prefix('+') {
        return seconds(r).map(|s| Op::Seek(s.min(MAX_JUMP as u32) as i32));
    }
    if let Some(r) = v.strip_prefix('-') {
        return seconds(r).map(|s| Op::Seek(-(s.min(MAX_JUMP as u32) as i32)));
    }
    seconds(v).map(|s| Op::SeekTo(At::Seconds(s)))
}

/// A rate: `1.5`, `1.5x`, `150%`; in percent.
fn rate(v: &str) -> Option<u16> {
    let v = v.trim_end_matches(['x', '×']);
    let pct = if let Some(p) = v.strip_suffix('%') {
        p.parse::<f64>().ok()?
    } else {
        v.parse::<f64>().ok()? * 100.0
    };
    if !pct.is_finite() {
        return None;
    }
    let pct = pct.round();
    (f64::from(MIN_RATE)..=f64::from(MAX_RATE)).contains(&pct).then_some(pct as u16)
}

/// What the page answered after an op.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The frame had no media to act on.
    None,
    /// The page refused to play: it wants a click on the page first.
    Blocked,
    /// The page has no full screen for it (an embed without permission).
    NoFull,
    Media {
        /// The op the script ran (its wire name).
        op: String,
        playing: bool,
        /// Seconds.
        time: f64,
        /// Seconds; `None` for a live stream or an unknown length.
        duration: Option<f64>,
        /// 1.0 is normal.
        rate: f64,
        looped: bool,
        full: bool,
    },
}

impl Status {
    /// Reads the script's answer: `none`, `blocked`, `nofull`, or
    /// `<op> <p|r> <time> <duration|inf> <rate> <0|1 loop> <0|1 full>`.
    pub fn parse(body: &str) -> Option<Status> {
        let w: Vec<&str> = body.split_whitespace().collect();
        match w.as_slice() {
            ["none"] => Some(Status::None),
            ["blocked"] => Some(Status::Blocked),
            ["nofull"] => Some(Status::NoFull),
            [op, state, time, duration, rate, looped, full] => {
                if op.len() > 12 || !op.bytes().all(|b| b.is_ascii_lowercase()) {
                    return None;
                }
                let num = |s: &str| s.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0);
                let flag = |s: &str| match s {
                    "1" => Some(true),
                    "0" => Some(false),
                    _ => None,
                };
                Some(Status::Media {
                    op: (*op).to_string(),
                    playing: match *state {
                        "p" => true,
                        "r" => false,
                        _ => return None,
                    },
                    time: num(time)?,
                    duration: num(duration).filter(|d| *d > 0.0),
                    rate: num(rate).filter(|r| *r > 0.0)?,
                    looped: flag(looped)?,
                    full: flag(full)?,
                })
            }
            _ => None,
        }
    }

    /// The toast for this answer.
    pub fn describe(&self) -> String {
        match self {
            Status::None => "No video or audio here".to_string(),
            Status::Blocked => "The page wants a click on its player before it plays".to_string(),
            Status::NoFull => "This player can't go full window".to_string(),
            Status::Media { op, playing, time, duration, rate, looped, full } => {
                let when = match duration {
                    Some(d) => format!("{} / {}", clock(*time), clock(*d)),
                    None => format!("{} · live", clock(*time)),
                };
                match op.as_str() {
                    "rate" => format!("Speed {}", speed(*rate)),
                    "loop" => if *looped { "Loop on".to_string() } else { "Loop off".to_string() },
                    "full" => if *full { "Full window (Esc leaves)".to_string() } else { "Out of full window".to_string() },
                    "seek" | "seekto" | "seekpct" => when,
                    _ => {
                        let state = if *playing { "Playing" } else { "Paused" };
                        if (*rate - 1.0).abs() > 0.001 {
                            format!("{state} · {when} · {}", speed(*rate))
                        } else {
                            format!("{state} · {when}")
                        }
                    }
                }
            }
        }
    }
}

/// `1.5×`, `0.25×`, `2×`.
fn speed(rate: f64) -> String {
    let s = format!("{rate:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}×")
}

/// `0:07`, `4:56`, `1:02:03`.
pub fn clock(seconds: f64) -> String {
    let s = if seconds.is_finite() && seconds > 0.0 { seconds.floor() as u64 } else { 0 };
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Why a page's media can't play, as its script reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trouble {
    /// The format isn't one this engine decodes.
    Format,
    /// It needs a DRM module (Widevine), which isn't there.
    Drm,
}

impl Trouble {
    pub fn parse(s: &str) -> Option<Trouble> {
        match s {
            "format" => Some(Trouble::Format),
            "drm" => Some(Trouble::Drm),
            _ => None,
        }
    }

    /// The message, naming the site (`example.com`) when there is one.
    pub fn describe(self, site: &str) -> String {
        let on = if site.is_empty() { String::new() } else { format!(" on {site}") };
        match self {
            Trouble::Format => format!("A video{on} can't play: its format isn't one Trailboard plays"),
            Trouble::Drm => format!("A video{on} can't play: it's protected (DRM), which Trailboard doesn't support yet"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_their_starts() {
        assert_eq!(parse("toggle"), Ok(Op::Toggle));
        assert_eq!(parse("pl"), Ok(Op::Play));
        assert_eq!(parse("PAUSE"), Ok(Op::Pause));
        assert_eq!(parse("fa"), Ok(Op::Faster));
        assert_eq!(parse("mu"), Ok(Op::Mute));
        assert_eq!(parse("fu"), Ok(Op::Full));
        assert!(parse("f").unwrap_err().contains("could be"));
        assert!(parse("s").unwrap_err().contains("could be"));
        assert!(parse("rewind").unwrap_err().contains("no “rewind”"));
        assert!(parse("").is_err());
        assert!(parse("play now").unwrap_err().contains("takes nothing"));
        assert!(parse("seek 1 2").unwrap_err().contains("too many"));
    }

    #[test]
    fn jumps() {
        assert_eq!(parse("forward"), Ok(Op::Seek(10)));
        assert_eq!(parse("back"), Ok(Op::Seek(-10)));
        assert_eq!(parse("fo 30"), Ok(Op::Seek(30)));
        assert_eq!(parse("back 1:30"), Ok(Op::Seek(-90)));
        assert!(parse("back 0").is_err());
        assert!(parse("forward x").is_err());
        assert_eq!(Op::Seek(10).times(3), Op::Seek(30));
        assert_eq!(Op::Seek(-10).times(0), Op::Seek(-10));
        assert_eq!(Op::Seek(10).times(u32::MAX), Op::Seek(MAX_JUMP as i32));
        assert_eq!(Op::Toggle.times(5), Op::Toggle);
    }

    #[test]
    fn seeks() {
        assert_eq!(parse("seek 90"), Ok(Op::SeekTo(At::Seconds(90))));
        assert_eq!(parse("seek 1:30"), Ok(Op::SeekTo(At::Seconds(90))));
        assert_eq!(parse("seek 1:02:03"), Ok(Op::SeekTo(At::Seconds(3723))));
        assert_eq!(parse("seek 0"), Ok(Op::SeekTo(At::Seconds(0))));
        assert_eq!(parse("seek 50%"), Ok(Op::SeekTo(At::Percent(50))));
        assert_eq!(parse("seek +10"), Ok(Op::Seek(10)));
        assert_eq!(parse("seek -1:00"), Ok(Op::Seek(-60)));
        assert!(parse("seek").is_err());
        assert!(parse("seek 101%").is_err());
        assert!(parse("seek 1:60").is_err());
        assert!(parse("seek 1::0").is_err());
        assert!(parse("seek 1:2:3:4").is_err());
        assert!(parse("seek abc").is_err());
    }

    #[test]
    fn rates() {
        assert_eq!(parse("speed"), Ok(Op::Speed(100)));
        assert_eq!(parse("speed 1.5"), Ok(Op::Speed(150)));
        assert_eq!(parse("speed 2x"), Ok(Op::Speed(200)));
        assert_eq!(parse("speed 75%"), Ok(Op::Speed(75)));
        assert!(parse("speed 0").is_err());
        assert!(parse("speed 20").is_err());
        assert!(parse("speed nan").is_err());
        assert_eq!(step_rate(100, true), 125);
        assert_eq!(step_rate(100, false), 75);
        assert_eq!(step_rate(110, true), 125, "between steps: the next one up");
        assert_eq!(step_rate(110, false), 100);
        assert_eq!(step_rate(400, true), 400);
        assert_eq!(step_rate(25, false), 25);
        assert_eq!(step_rate(1000, false), 400);
    }

    #[test]
    fn wire_names() {
        assert_eq!(Op::Seek(-10).wire(), ("seek", -10.0));
        assert_eq!(Op::SeekTo(At::Percent(50)).wire(), ("seekpct", 50.0));
        assert_eq!(Op::Speed(150).wire(), ("rate", 1.5));
        assert_eq!(Op::Faster.wire(), ("rate", -1.0));
        assert_eq!(Op::Slower.wire(), ("rate", -2.0));
    }

    #[test]
    fn answers() {
        assert_eq!(Status::parse("none"), Some(Status::None));
        assert_eq!(Status::parse("blocked"), Some(Status::Blocked));
        let s = Status::parse("toggle r 83.4 296 1 0 0").unwrap();
        assert_eq!(s.describe(), "Paused · 1:23 / 4:56");
        let s = Status::parse("play p 5 inf 1.5 0 0").unwrap();
        assert_eq!(s.describe(), "Playing · 0:05 · live · 1.5×");
        assert_eq!(Status::parse("rate p 5 10 0.25 0 0").unwrap().describe(), "Speed 0.25×");
        assert_eq!(Status::parse("rate p 5 10 2 0 0").unwrap().describe(), "Speed 2×");
        assert_eq!(Status::parse("seek p 3723 7200 1 0 0").unwrap().describe(), "1:02:03 / 2:00:00");
        assert_eq!(Status::parse("loop p 0 1 1 1 0").unwrap().describe(), "Loop on");
        assert_eq!(Status::parse("full p 0 1 1 0 1").unwrap().describe(), "Full window (Esc leaves)");
        assert_eq!(Status::parse("toggle x 1 1 1 0 0"), None);
        assert_eq!(Status::parse("toggle p -1 1 1 0 0"), None);
        assert_eq!(Status::parse("toggle p 1 1 0 0 0"), None, "a rate of 0");
        assert_eq!(Status::parse("Bad! p 1 1 1 0 0"), None);
        assert_eq!(Status::parse("toggle p 1 1 1 0"), None);
        assert_eq!(Status::parse(""), None);
    }

    #[test]
    fn clocks() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(59.9), "0:59");
        assert_eq!(clock(600.0), "10:00");
        assert_eq!(clock(3600.0), "1:00:00");
        assert_eq!(clock(f64::NAN), "0:00");
        assert_eq!(clock(-5.0), "0:00");
    }

    #[test]
    fn troubles() {
        assert_eq!(Trouble::parse("drm"), Some(Trouble::Drm));
        assert_eq!(Trouble::parse("x"), None);
        assert!(Trouble::Drm.describe("example.com").contains("on example.com"));
        assert!(!Trouble::Format.describe("").contains(" on "));
    }
}
