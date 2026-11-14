# Trailboard

A canvas browser for Linux: web pages live as cards on an infinite board, and a
link opens as a new card beside the page it came from, joined by a trail edge.

It's an independent implementation inspired by Marble (a macOS-only app). The
behaviour and architecture were studied with REA (see
[docs/re-findings.md](docs/re-findings.md)); no Marble code, art or server is used.

## Architecture

| Crate | Role |
|---|---|
| `scene` | Board model: cards, trail edges, camera, placement. Pure Rust, unit-tested. |
| `storage` | Board persistence (atomic JSON). |
| `web` | Chromium via CEF in off-screen-rendering mode; one windowless browser per card. |
| `compositor` | `wgpu` renderer: dot grid, rounded cards with page textures, shadows, edges, `glyphon` text. |
| `app` (`trailboard`) | `winit` window, input, omnibox, drawing, main loop. |

## Build and run

Requirements: Rust (stable), CMake, Ninja, a Vulkan driver, and CEF's runtime
libraries (nss, atk, cups, gbm, xkbcommon, gtk3; present on most desktops).

```sh
./scripts/install-cef.sh                      # once: CEF 156 into ~/.local/share/cef
mkdir -p .cargo && cat > .cargo/config.toml <<EOF
[env]
CEF_PATH = "$HOME/.local/share/cef"
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-Wl,-rpath,$HOME/.local/share/cef"]
EOF
cargo run --release -p trailboard -- https://en.wikipedia.org/wiki/Turbofan
```

Each space's board is saved to `$XDG_DATA_HOME/trailboard/spaces/<id>.json`
(default `~/.local/share/trailboard/`), listed by `spaces.json`, and restored on
the next start; the browser profile (cookies, cache) lives next to it. A
`board.json` from an earlier version becomes the first space. A deleted space's
board is moved to `spaces/deleted/`, not erased.

Only the 6 most relevant cards keep a live browser (focused first, then on screen,
then most recently used). The rest sleep and show their last frame, dimmed, until
you click them or bring them into view. Set `TRAILBOARD_MAX_LIVE=N` to change the cap.

## Using it

| Action | Input |
|---|---|
| Pan | drag empty board, middle-drag, or scroll over the board |
| Zoom | Ctrl + scroll |
| Focus a page | click it (keyboard and scroll then go to the page) |
| Move / resize a card | drag its title bar / bottom-right corner |
| Select cards | Shift+drag on the board, Shift+click title bars, Ctrl+A (Esc clears) |
| Move / close selected cards | drag a selected title bar / Delete |
| Fill the view with a card | double-click its title bar |
| Read Mode (page fills the window) | Tab (Tab, Esc or double-click its bar to go back) |
| Close card / reopen closed | × or Ctrl+W / Ctrl+Shift+T |
| Undo / redo board edits | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y (after clicking the board or a title bar) |
| Address bar / new card | Ctrl+L / Ctrl+T (or N with no page focused) |
| Fit everything | Ctrl+0 (or F with no page focused) |
| Back / forward / reload | Alt+← / Alt+→ / Ctrl+R |
| Page context menu | right-click in a page (open link beside, copy link, edit, search selection) |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V (shared with the desktop clipboard) |
| Leave the page | Esc |
| New space / next / previous | Ctrl+Shift+N / Ctrl+PageDown / Ctrl+PageUp |
| Rename space / next tint | F2 / Ctrl+Shift+B |
| Delete space | Ctrl+Shift+W (asks first if it has cards) |
| Show shortcuts | ? (with no page focused) or F1 |
| Quit | Ctrl+Q |

Input methods (e.g. fcitx5) work in pages and in the address bar.

On [Omarchy](https://omarchy.org/), Trailboard follows the current theme live
(colours, light or dark, including the pages' `prefers-color-scheme`),
Hyprland's window rounding and the desktop text size. Elsewhere it uses a dark
palette with a yellow accent and square corners.

## Tests

```sh
cargo test                       # scene, storage, omnibox unit tests
TRAILBOARD_SCRIPT=path cargo run -p trailboard -- URL   # scripted end-to-end run
```

`TRAILBOARD_SCRIPT` drives the real input handlers (`click x y`, `pclick card x y`,
`key ctrl+c`, `type text`, `ime-commit text`, `menuclick label`, `wheel`, `fit`,
`open URL`, `wait ms`, `log`, `quit`); see `crates/app/src/script.rs`.

## Status

Done: live page cards, open-links-beside with trail edges, pan/zoom, move,
resize, close/reopen, multi-select and group move, undo/redo, omnibox, zoom-aware page resolution, off-screen
pages paused, live-browser cap, page context menu, desktop clipboard, IME input,
board persistence, spaces (several boards with their own camera and background).

Next: Tidy and Fold, Read Mode, notes, trails sidebar, Ctrl+K palette,
history with full-text search, ad blocking, then optional semantic search and
an LLM-backed "Explain" panel.

- Full plan and milestone status: [docs/PLAN.md](docs/PLAN.md)
- Later ideas: [docs/FUTURE.md](docs/FUTURE.md)
- Changes: [CHANGELOG.md](CHANGELOG.md)
- Instructions for coding agents: [CLAUDE.md](CLAUDE.md)
