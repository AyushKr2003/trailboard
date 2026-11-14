//! Scripted input for end-to-end testing: `TRAILBOARD_SCRIPT=/path/file`.
//!
//! One command per line, run in order; `#` starts a comment. Coordinates are
//! window pixels. Commands feed the same handlers as real input.
//!
//! ```text
//! wait 3000            # milliseconds
//! move 400 300
//! click 400 300        # left press + release
//! wheel 0 -3           # line deltas
//! ctrlwheel 2
//! drag 10 10 300 200   # left press, move in steps, release
//! fit                  # same as Ctrl+0
//! undo                 # same as Ctrl+Z on the board
//! redo                 # same as Ctrl+Shift+Z
//! cards                # log every card (id, rect, live page, title, url) and edge
//! open https://…       # same as Ctrl+T + Enter
//! log some note        # print to the log (marks screenshot moments)
//! quit
//! ```

use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Cmd {
    Wait(u64),
    Move(f32, f32),
    Click(f32, f32),
    Wheel(f32, f32),
    CtrlWheel(f32),
    Drag(f32, f32, f32, f32),
    Fit,
    Undo,
    Redo,
    Cards,
    Open(String),
    Log(String),
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
    let line = line.split('#').next()?.trim();
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let nums: Vec<f32> = rest.split_whitespace().filter_map(|n| n.parse().ok()).collect();
    let num = |i: usize| nums.get(i).copied().unwrap_or(0.0);
    Some(match word {
        "" => return None,
        "wait" => Cmd::Wait(num(0) as u64),
        "move" => Cmd::Move(num(0), num(1)),
        "click" => Cmd::Click(num(0), num(1)),
        "wheel" => Cmd::Wheel(num(0), num(1)),
        "ctrlwheel" => Cmd::CtrlWheel(num(0)),
        "drag" => Cmd::Drag(num(0), num(1), num(2), num(3)),
        "fit" => Cmd::Fit,
        "undo" => Cmd::Undo,
        "redo" => Cmd::Redo,
        "cards" => Cmd::Cards,
        "open" => Cmd::Open(rest.trim().to_string()),
        "log" => Cmd::Log(rest.trim().to_string()),
        "quit" => Cmd::Quit,
        other => {
            log::warn!("script: unknown command {other:?}");
            return None;
        }
    })
}
