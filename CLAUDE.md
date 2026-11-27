# Trailboard: agent instructions

A Linux canvas browser (Rust + winit + wgpu + CEF). Pages are live cards on an
infinite board; links open beside their source. Read [README.md](README.md) and [docs/GUIDE.md](docs/GUIDE.md) for
use, [docs/PLAN.md](docs/PLAN.md) for milestones and status, and
its §7 (known bugs) and §8 (later ideas).
Interface rules (colour levels, text strengths, type scale, icons) are in
[docs/DESIGN.md](docs/DESIGN.md): use `ui/theme.rs` tokens and `ui/chrome.rs`
helpers, never ad-hoc colours or sizes.

## Clean-room rule

Trailboard is our own design and code. Never copy another app's code, art,
icons, names, prompts or server APIs into this repo, and don't name other
browsers in the docs.

## Layout and boundaries

| Crate | Owns | Must not depend on |
|---|---|---|
| `scene` | board model, camera, placement | GPU, CEF, winit |
| `storage` | on-disk formats (atomic writes) | GPU, CEF, winit |
| `web` | CEF lifecycle, pages, input forwarding, `WebEvent`s | winit, wgpu |
| `compositor` | wgpu rendering, text | CEF |
| `app` | window, input mapping, UI state, main loop | — |

Inside the crates (put new code where its neighbours are):

- `app/src/app/`: the `App` struct (`mod.rs`) and one file per area, each an
  `impl App` block (`cards.rs`, `input.rs`, `drawing.rs`, `camera.rs`,
  `trails.rs`…); methods shared between them are `pub(super)`. Beside it:
  `ui/` (what the app draws), `pages/` (its own HTML pages), `system/`
  (desktop, clipboard, launch), `dev/` (script runner, perf, memstat).
- `scene/src/`: `board/`, `trails/`, `input/`, `pages/`, `prefs/`; `lib.rs`
  re-exports every module, so paths stay `scene::keys`, `scene::tidy`….
- `web/src/`: `events`, `runtime` (pump, wake), `engine` (CEF start),
  `frame`, `shared`, `handlers` (CEF callbacks), `page` (`Page`), plus the
  feature modules; `lib.rs` re-exports, so paths stay `web::Page`….

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
The script commands live in `crates/app/src/dev/script.rs`. Use an isolated `XDG_DATA_HOME`
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

## Starting a session

1. Read `docs/PLAN.md` (section 6 "Handoff" and the status marks) and the
   Unreleased part of `CHANGELOG.md`; run `git status`, `cargo build`, `cargo test`.
   If something is broken, fixing it is the task.
2. Take the Handoff's "Next up", else the first ⬜/🟡 item in milestone order.
   Do one coherent item per session; decide open questions sensibly and record
   them in `docs/PLAN.md`. Ask only when it's genuinely the owner's call.
3. When done, rewrite the Handoff (date, done, next up, notes) and end with a
   short summary: what changed, how it was verified, what wasn't, what's next.

## Workflow

- Done means: `cargo test` green, `cargo clippy` clean, a scripted e2e run for
  behaviour changes, and a screenshot check for visual changes.
- Update `CHANGELOG.md` (Unreleased) and the status marks in `docs/PLAN.md` with every change.
- Conventional Commit subjects. Commit or push only when the user asks.
- Never add Claude as co-author or any "Generated with" line to commits or PRs.
