//! Scripted input for end-to-end testing: `TRAILBOARD_SCRIPT=/path/file`.
//!
//! One command per line, run in order; `#` starts a comment. Coordinates are
//! window pixels. Commands feed the same handlers as real input.
//!
//! ```text
//! wait 3000            # milliseconds
//! move 400 300
//! click 400 300        # left press + release
//! pclick 1 20 40       # left-click card 1's page at CSS px (20, 40)
//! prclick 1 20 40      # same, right button
//! iclick 1 20 40       # left-click the inspector docked in card 1 at CSS px (20, 40) (irclick: right)
//! wheel 0 -3           # line deltas (mouse wheel notches)
//! touchpad 0 -8        # touchpad pixel deltas; `touchpad 0 0 end` lifts the fingers
//! ctrlwheel 2
//! drag 10 10 300 200   # left press, move in steps, release
//! sdrag 10 10 300 200  # same with Shift held (selection band on the board)
//! adrag 10 10 300 200  # same with Alt held (a label let go on a card joins its trail)
//! asdrag 10 10 300 200 # same with Alt and Shift held (joins keeping its colour)
//! press 10 10          # left press only (then `move`s, `release`): to look mid-drag
//! apress 10 10         # the same with Alt held for the press (an Alt+drag)
//! release 300 200      # left release
//! sclick 400 300       # Shift+click (add/remove a card from the selection)
//! rclick 400 300       # right-click (on the empty board: its menu)
//! key ctrl+shift+z     # press + release a key combo (US layout names, see keymap::KeyIn::from_name)
//! type hello world     (no comment: the rest of the line is typed, `#` included, one key press per character)
//! ime-preedit 你好       # IME composition text (empty cancels it)
//! ime-commit 你好        # IME final text
//! menuclick Copy       # click the open context menu's item with this label
//! awakeclick 3        # click the awake button on card 3's title bar (shown while it is hovered or current)
//! badgeclick 3         # click the count badge of the stack card 3 fronts (opens its outline)
//! fit                  # same as Shift+F (fit everything)
//! undo                 # same as Ctrl+Z on the board
//! redo                 # same as Ctrl+Shift+Z
//! cards                # log every card (id, rect, live page, selected, requests blocked, elements hidden, title, url, its trail and parent), edge and trail label
//! spaces               # log every space (id, name, background) and which is active
//! sidebar              # log the trails sidebar: open/keys, highlight, scroll, each row and where to click it
//! zooms                # log each live card's page zoom (wanted, Chromium's, and Chromium's default)
//! find                 # log the find bar: text, card, matches
//! history rust         # log the history index's size, its newest pages, and what the words find
//! palette              # log the Ctrl+K palette: query, highlight, each result row and where to click it
//! open https://…       # same as Ctrl+T + Enter
//! log some note        # print to the log (marks screenshot moments)
//! panic               # panic on purpose (tests the crash report)
//! quit
//! ```

use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Cmd {
    Wait(u64),
    Move(f32, f32),
    Click(f32, f32, bool),
    /// Right press + release at a point.
    RightClick(f32, f32),
    /// Click inside card `id`'s page at CSS px; `right` uses the right button.
    PageClick { id: u64, x: f32, y: f32, right: bool },
    DockClick { id: u64, x: f32, y: f32, right: bool },
    Wheel(f32, f32),
    /// Touchpad pixel deltas; `end` is the fingers lifting.
    Touchpad { dx: f32, dy: f32, end: bool },
    CtrlWheel(f32),
    /// Left drag; `shift` holds Shift throughout.
    /// `drag`, `sdrag` (Shift held), `adrag` (Alt held: join a trail),
    /// `asdrag` (both: join keeping the colour).
    Drag { from: (f32, f32), to: (f32, f32), shift: bool, alt: bool },
    /// Left press or release at a point, alone.
    /// `press`, `release`; `apress` presses with Alt held (an Alt+drag).
    Button { at: (f32, f32), pressed: bool, alt: bool },
    Key(String),
    Type(String),
    ImePreedit(String),
    ImeCommit(String),
    MenuClick(String),
    /// Click the count badge of the stack whose front is card `id`.
    BadgeClick(u64),
    /// Click the blocked-request shield on card `id`'s title bar.
    ShieldClick(u64),
    /// Click the trail tag drawn above card `id`.
    TagClick(u64),
    /// Click the awake button on card `id`'s title bar.
    AwakeClick(u64),
    /// Log the history index, and a search of it for these words.
    History(String),
    Fit,
    Undo,
    Redo,
    Cards,
    Spaces,
    Sidebar,
    Palette,
    /// Log each live card's site zoom and the zoom Chromium has.
    Zooms,
    /// Log each card's icon site and whether its icon is known.
    Icons,
    /// Log the find bar: its text, card and result.
    Find,
    Open(String),
    Log(String),
    Panic,
    Quit,
}

pub struct Script {
    cmds: VecDeque<Cmd>,
    resume_at: Instant,
}

impl Script {
    pub fn from_env() -> Option<Script> {
        let path = std::env::var_os("TRAILBOARD_SCRIPT")?;
        let text = std::fs::read_to_string(&path)
            .map_err(|e| log::error!("TRAILBOARD_SCRIPT {path:?}: {e}"))
            .ok()?;
        let cmds = text.lines().filter_map(parse).collect();
        Some(Script { cmds, resume_at: Instant::now() })
    }

    /// Next command due now, if any.
    pub fn next(&mut self) -> Option<Cmd> {
        if Instant::now() < self.resume_at {
            return None;
        }
        let cmd = self.cmds.pop_front()?;
        if let Cmd::Wait(ms) = cmd {
            self.resume_at = Instant::now() + std::time::Duration::from_millis(ms);
            return None;
        }
        Some(cmd)
    }
}

fn parse(line: &str) -> Option<Cmd> {
    // `type` takes the rest of the line as is, `#` included.
    if let Some(text) = line.trim_start().strip_prefix("type ") {
        return Some(Cmd::Type(text.to_string()));
    }
    let line = line.split('#').next()?.trim();
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let nums: Vec<f32> = rest.split_whitespace().filter_map(|n| n.parse().ok()).collect();
    let num = |i: usize| nums.get(i).copied().unwrap_or(0.0);
    Some(match word {
        "" => return None,
        "wait" => Cmd::Wait(num(0) as u64),
        "move" => Cmd::Move(num(0), num(1)),
        "click" | "sclick" => Cmd::Click(num(0), num(1), word == "sclick"),
        "rclick" => Cmd::RightClick(num(0), num(1)),
        "pclick" | "prclick" => Cmd::PageClick { id: num(0) as u64, x: num(1), y: num(2), right: word == "prclick" },
        "iclick" | "irclick" => Cmd::DockClick { id: num(0) as u64, x: num(1), y: num(2), right: word == "irclick" },
        "wheel" => Cmd::Wheel(num(0), num(1)),
        "touchpad" => Cmd::Touchpad { dx: num(0), dy: num(1), end: rest.split_whitespace().nth(2) == Some("end") },
        "ctrlwheel" => Cmd::CtrlWheel(num(0)),
        "press" | "apress" | "release" => Cmd::Button { at: (num(0), num(1)), pressed: word != "release", alt: word == "apress" },
        "drag" | "sdrag" | "adrag" | "asdrag" => {
            Cmd::Drag { from: (num(0), num(1)), to: (num(2), num(3)), shift: word.contains('s'), alt: word.starts_with('a') }
        }
        "key" => Cmd::Key(rest.trim().to_string()),
        // Keep the text's own spacing; only the separator after the word is dropped.
        "type" => Cmd::Type(rest.to_string()),
        "ime-preedit" => Cmd::ImePreedit(rest.trim().to_string()),
        "ime-commit" => Cmd::ImeCommit(rest.trim().to_string()),
        "menuclick" => Cmd::MenuClick(rest.trim().to_string()),
        "badgeclick" => Cmd::BadgeClick(num(0) as u64),
        "shieldclick" => Cmd::ShieldClick(num(0) as u64),
        "tagclick" => Cmd::TagClick(num(0) as u64),
        "awakeclick" => Cmd::AwakeClick(num(0) as u64),
        "fit" => Cmd::Fit,
        "undo" => Cmd::Undo,
        "redo" => Cmd::Redo,
        "cards" => Cmd::Cards,
        "spaces" => Cmd::Spaces,
        "sidebar" => Cmd::Sidebar,
        "palette" => Cmd::Palette,
        "zooms" => Cmd::Zooms,
        "icons" => Cmd::Icons,
        "history" => Cmd::History(rest.trim().to_string()),
        "find" => Cmd::Find,
        "open" => Cmd::Open(rest.trim().to_string()),
        "log" => Cmd::Log(rest.trim().to_string()),
        "panic" => Cmd::Panic,
        "quit" => Cmd::Quit,
        other => {
            log::warn!("script: unknown command {other:?}");
            return None;
        }
    })
}
