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
| Go to the next card left / down / up / right | h / j / k / l or the arrow keys (the card becomes current, with a thin ring) |
| Type in the current card's page | Enter or Space, or click in the page (a thick ring shows it has the keys) |
| Back to the board / let go of the card | Esc / Esc again |
| Pan / zoom with the keyboard | Shift + h/j/k/l or Shift + arrows / + and - |
| Move / resize a card with the keyboard | M, then h/j/k/l (Shift resizes, Ctrl takes fine steps); Enter keeps, Esc puts it back |
| Follow a link by keyboard | f labels the links in the current card's page; type a label to open it beside (Esc cancels) |
| Close the current card / quit | Delete / Q (Ctrl+Q works everywhere) |
| Focus a page | click it (keyboard and scroll then go to the page) |
| Move / resize a card | drag its title bar / bottom-right corner |
| Select cards | Shift+drag on the board, Shift+click title bars, Ctrl+A (Esc clears) |
| Move / close selected cards | drag a selected title bar / Delete |
| Fill the view with a card | double-click its title bar |
| Read Mode (page fills the window) | Tab (Tab, Esc or double-click its bar to go back) |
| Close card / reopen closed | × or Ctrl+W / Ctrl+Shift+T |
| Undo / redo board edits | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y (after clicking the board or a title bar) |
| Address bar / new card | Ctrl+L / Ctrl+T |
| Palette: find a card (folded ones too, or by words in its page), a card you closed, a page from history, another space or a command | Ctrl+K, type a few letters, ↑/↓ (or Ctrl+N/P) and Enter, or click a row; the last row searches the web or opens what you typed |
| New note (Markdown) | N (while no page has the keys); click a note to edit, Esc or click away to see it rendered |
| New sticky note / its next colour | Shift+N / C on it (while no page has the keys); drag its coloured strip to move it; *Make sticky note* / *Make note card* in a note's right-click menu |
| Quote selected text in a note | right-click the selection → *Add to note* (the note last used, or a new one beside the page) |
| Save as Markdown | Ctrl+S: the focused note, else the whole board (its trails as nested lists, notes in full); also *Save as Markdown* in a note's right-click menu. Files go to `Documents/Trailboard/` |
| Fit everything | Shift+F (while no page has the keys), or Ctrl+0 with no card current |
| Trails sidebar: every trail as a tree, then the other cards | B (while no page has the keys): j/k go to a card, h/l collapse / expand a trail, Enter types in the card's page, Esc or B closes; or click a row |
| Tidy: lay trails out left to right, no overlaps | T (while no page has the keys; Ctrl+Z undoes it) |
| Fold the current trail (or the one in the middle of the view) into a stack / unfold it | S (while no page has the keys; Ctrl+Z undoes it) |
| Peek at the previous / next card of a stack | [ / ] (while no page has the keys) |
| List a stack's pages, bring one to the front or unfold | Click the count badge, or O (while no page has the keys) |
| Go to the next / previous card on the trail (folded or not) | Alt+PgDn / Alt+PgUp |
| Name the current trail / give it the next colour | R / C (while no page has the keys; Ctrl+Z undoes it) |
| Back / forward / reload | Alt+← / Alt+→ / Ctrl+R |
| Click a link, button, checkbox or field by keyboard | f, then type its label (links open beside; a field gets the keys) |
| Select / deselect the current card | v (h/j/k/l keep the selection) |
| Fill the view with the current card | z |
| Scroll the current page without entering it | d / u |
| Page menu by keyboard | Menu key or Shift+F10; j/k or ↑/↓, Enter, Esc |
| History: pages you opened, newest first | Ctrl+H, type to filter by words in them, Enter opens, Shift+Delete forgets the page; *Clear history* in Ctrl+K forgets all |
| Find in the current page | Ctrl+F, type (matches show as you type); Enter / Shift+Enter next / previous; Esc closes |
| Zoom the current page in / out / back to 100% | Ctrl++ / Ctrl+- / Ctrl+0: every card on that site follows, and the zoom is remembered per site |
| Page context menu | right-click in a page (open link beside, copy link, edit, search selection) |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V (shared with the desktop clipboard) |
| Leave the page | Esc |
| New space / next / previous | Ctrl+Shift+N / Ctrl+PageDown / Ctrl+PageUp |
| Rename space / next tint | F2 / Ctrl+Shift+B |
| Delete space | Ctrl+Shift+W (asks first if it has cards) |
| Private space on / off | Ctrl+Shift+P: pages there aren't kept in history, their site icons and zoom aren't saved, and cookies live in memory until you quit (the board itself is still saved) |
| Show shortcuts | ? (while no page has the keys) or F1 |
| Quit | Ctrl+Q |

Input methods (e.g. fcitx5) work in pages and in the address bar.

Each card's title bar shows its site's icon (kept per site under
`icons/` in the data folder, so sleeping cards and restored boards keep theirs)
and a moving bar while the page loads. Zoomed out too far to read titles, the
card under the pointer (or else the current card) shows its icon and title in
a label below it.

Pages you open are remembered with their text in `page-text.db` in the data
folder (the newest 5000), so Ctrl+K can find them by any word on them.
Ctrl+H lists them newest first, with when you saw each; type to keep the
pages with those words, Enter opens one (or goes to the card showing it), and
Shift+Delete forgets the highlighted page (in the Ctrl+K palette too). The
palette command *Clear history* forgets every page after asking.

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
`key ctrl+c`, `type text`, `ime-commit text`, `menuclick label`, `badgeclick id`, `wheel`, `fit`,
`open URL`, `wait ms`, `log`, `quit`); see `crates/app/src/script.rs`.

## Status

Done: live page cards, open-links-beside with trail edges, pan/zoom, move,
resize, close/reopen, multi-select and group move, undo/redo, omnibox, zoom-aware page resolution, off-screen
pages paused, live-browser cap, page context menu, desktop clipboard, IME input,
board persistence, spaces (several boards with their own camera and background),
Read Mode, Tidy, Fold, stack outline, next/previous on a trail, Markdown notes
with *Add to note*, trail names and colours, save a note or board as Markdown,
the trails sidebar, the Ctrl+K palette, find in page and per-site page zoom.

Next: history with full-text search, ad blocking, then optional semantic search and
an LLM-backed "Explain" panel.

- Full plan and milestone status: [docs/PLAN.md](docs/PLAN.md)
- Later ideas: [docs/FUTURE.md](docs/FUTURE.md)
- Changes: [CHANGELOG.md](CHANGELOG.md)
- Instructions for coding agents: [CLAUDE.md](CLAUDE.md)
