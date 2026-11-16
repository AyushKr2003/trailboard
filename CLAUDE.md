# Trailboard: agent instructions

A Linux canvas browser (Rust + winit + wgpu + CEF). Pages are live cards on an
infinite board; links open beside their source. Read [README.md](README.md) for
use, [docs/PLAN.md](docs/PLAN.md) for milestones and status,
[docs/FUTURE.md](docs/FUTURE.md) for later ideas, and
[docs/re-findings.md](docs/re-findings.md) for what we learned about Marble.
Interface rules (colour levels, text strengths, type scale, icons) are in
[docs/DESIGN.md](docs/DESIGN.md): use `theme.rs` tokens and `chrome.rs`
helpers, never ad-hoc colours or sizes.

## Clean-room rule

Trailboard is modelled on Marble's *behaviour*, not its code. Never copy Marble
code, decompiled pseudocode, art, icons, names, prompts or server APIs into this
repo. Recovered function names and data shapes in `docs/` are reference for
designing our own versions only.

## Layout and boundaries

| Crate | Owns | Must not depend on |
|---|---|---|
| `scene` | board model, camera, placement | GPU, CEF, winit |
| `storage` | on-disk formats (atomic writes) | GPU, CEF, winit |
| `web` | CEF lifecycle, pages, input forwarding, `WebEvent`s | winit, wgpu |
| `compositor` | wgpu rendering, text | CEF |
| `app` | window, input mapping, UI state, main loop | — |

- CEF runs on the main thread with `external_message_pump`; use `Rc`/`RefCell`, not locks.
  The exception is request blocking (`web::shield`), called on Chromium's IO
  thread: it uses locks and atomics, and must never touch a page's `Rc` state.
- CEF → app communication goes through `web::drain_events()`; don't call app code from CEF callbacks.
- Upload page textures only when `PaintFrame::serial` changes.
- Persisted formats carry a `version`; add migrations instead of breaking old files.
- Put pure logic (layout, placement, Tidy, search ranking) in `scene`/`storage` with unit tests.

## Build, run, test

Prereqs: CEF at `~/.local/share/cef` (`scripts/install-cef.sh`) and a local,
uncommitted `.cargo/config.toml` that sets `CEF_PATH` and an rpath (see README).

```sh
cargo build                     # debug
cargo run -p trailboard -- URL  # run
cargo test                      # unit tests
cargo clippy                    # must be warning-free
```

End-to-end: `TRAILBOARD_SCRIPT=file XDG_DATA_HOME=/tmp/x cargo run -p trailboard -- URL`.
The script commands live in `crates/app/src/script.rs`. Use an isolated `XDG_DATA_HOME`
so tests never touch the user's real board.

On this Hyprland 0.57 machine, `ydotool` doesn't reach windows and `wtype` remaps
keycodes. Prefer the script runner. Move the pointer with
`hyprctl dispatch 'hl.dsp.cursor.move({x=…, y=…})'`; screenshot a window with
`grim -g "X,Y WxH"` using geometry from `hyprctl clients -j`.

Kill the app with `pkill -x trailboard`, never `pkill -f`, which matches your own shell.

## Machine constraints

- 7.6 GB RAM, Intel iGPU, and `/tmp` is a RAM-backed tmpfs. Keep large files
  (builds, CEF, analysis data) on disk under `~/.cache` or the repo's `target/`.
- Each card is a full Chromium renderer; keep test boards small.
- Use `pkexec` for root, never `sudo`.

## Workflow

- Done means: `cargo test` green, `cargo clippy` clean, a scripted e2e run for
  behaviour changes, and a screenshot check for visual changes.
- Update `CHANGELOG.md` (Unreleased) and the status marks in `docs/PLAN.md` with every change.
- Conventional Commit subjects. Commit or push only when the user asks.
- Never add Claude as co-author or any "Generated with" line to commits or PRs.
