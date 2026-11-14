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

The board is saved to `$XDG_DATA_HOME/trailboard/board.json` (default
`~/.local/share/trailboard/`) and restored on the next start; the browser
profile (cookies, cache) lives next to it.

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
| Fill the view with a card | double-click its title bar |
| Close card / reopen closed | × or Ctrl+W / Ctrl+Shift+T |
| Undo / redo board edits | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y (after clicking the board or a title bar) |
| Address bar / new card | Ctrl+L / Ctrl+T (or N with no page focused) |
| Fit everything | Ctrl+0 (or F with no page focused) |
| Back / forward / reload | Alt+← / Alt+→ / Ctrl+R |
| Leave the page | Esc |

## Tests

```sh
cargo test                       # scene, storage, omnibox unit tests
TRAILBOARD_SCRIPT=path cargo run -p trailboard -- URL   # scripted end-to-end run
```

`TRAILBOARD_SCRIPT` drives the real input handlers (`click x y`, `wheel`, `fit`,
`open URL`, `wait ms`, `log`, `quit`); see `crates/app/src/script.rs`.

## Status

Done: live page cards, open-links-beside with trail edges, pan/zoom, move,
resize, close/reopen, omnibox, zoom-aware page resolution, off-screen pages
paused, board persistence.

Next: Spaces, Tidy and Fold, Read Mode, notes, trails sidebar, Ctrl+K palette,
history with full-text search, ad blocking, then optional semantic search and
an LLM-backed "Explain" panel.

- Full plan and milestone status: [docs/PLAN.md](docs/PLAN.md)
- Later ideas: [docs/FUTURE.md](docs/FUTURE.md)
- Changes: [CHANGELOG.md](CHANGELOG.md)
- Instructions for coding agents: [CLAUDE.md](CLAUDE.md)
