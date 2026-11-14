# Changelog

All notable changes to Trailboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/) once it has releases.

## [Unreleased]

### Added
- **Multi-select and group move**: Shift+drag on the board draws a selection band
  (cards it touches are added), Shift+click a title bar adds or removes a card, and
  Ctrl+A (no card focused) selects all. Dragging a selected card's title bar moves
  the whole selection; Delete or Backspace closes it. A click on the board or Esc
  clears the selection. Group moves and closes undo in one step, and closed cards
  get their trail edges back.
- **Shortcut sheet**: press `?` with no card focused to see keyboard and mouse
  shortcuts; Esc, `?` or a click hides it. The empty board mentions it.
- Script commands `sdrag` and `sclick` (Shift held); `cards` logs whether each card is selected.
- **Page context menu**: right-click in a page for Open link beside, Copy link
  address, Open/Copy image address, Undo/Redo/Cut/Copy/Paste/Select all in text
  fields, Copy and Search for selected text, or Back/Forward/Reload. Arrow keys
  and Enter work; Esc or a click outside closes it.
- **Desktop clipboard**: Ctrl+C / Ctrl+X in a page now reach other apps, and
  Ctrl+V pastes text copied elsewhere. The address bar supports Ctrl+V, and
  Ctrl+C / Ctrl+X after Ctrl+A.
- **Input methods**: IME composition (e.g. fcitx5 Pinyin) in pages, with the
  candidate window at the text caret; committed text in the address bar.
- Script commands `pclick`/`prclick` (click at page coordinates), `key`, `type`,
  `ime-preedit`, `ime-commit` and `menuclick`.
- **Dropdowns and popups in pages**: `<select>` lists and other popup widgets are
  drawn over the page and can be clicked.
- **Page cursors**: the pointer shows the page's cursor (hand over links, text
  beam, resize arrows, hidden for `cursor: none`).
- Script command `cards` also logs each card's title.
- **Live-browser cap**: at most 6 cards (set with `TRAILBOARD_MAX_LIVE`) keep a
  live browser: the focused card, then cards on screen, then the most recently
  used. The others sleep and show their last frame, dimmed, and wake when clicked
  or brought into view. On start, pages load only as they come on screen.
- **Undo/redo for board edits**: Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y undo and redo
  opening, closing, moving and resizing cards. A restored card gets its live page
  and trail edges back. After a click inside a page, Ctrl+Z goes to the page.
- Script commands `drag`, `undo`, `redo` and `cards` for end-to-end runs.
- **Reverse-engineering notes** for Marble 0.1.28 (macOS), gathered with REA:
  architecture, module map, 1,549 recovered function names, SQLite page-text
  schema, board JSON structures, menus/shortcuts and the Explain API contract
  (`docs/re-findings.md`, `docs/marble-function-names.txt`).
- **Cargo workspace** with crates `scene`, `storage`, `web`, `compositor` and `app` (binary `trailboard`).
- **Board model** (`scene`): cards, trail edges, z-order, hit testing, bounds;
  camera with pan, zoom-about-point and fit; placement beside a source card and
  free placement near a point.
- **Chromium integration** (`web`): CEF 156 off-screen rendering, subprocess
  bootstrap, per-card browsers, paint buffers, mouse/wheel/keyboard forwarding,
  title/URL/loading events, and "open links beside the page" (link clicks,
  popups, Ctrl/middle-click).
- **Compositor** (`compositor`): wgpu renderer with dot-grid background, rounded
  and textured cards, soft shadows, focus ring, bezier trail edges and layered
  glyphon text.
- **App** (`trailboard`): winit window, pan/zoom, focus, move/resize/close/reopen
  cards, double-click to fill the view, omnibox with URL/search detection,
  layout-aware keyboard shortcuts, zoom-aware page resolution and pausing
  off-screen pages.
- **Persistence** (`storage`): board saved atomically to
  `$XDG_DATA_HOME/trailboard/board.json`, autosaved every 3 s and on exit, and
  restored on start. An unreadable board file is moved aside, not overwritten.
- **Scripted end-to-end runs** via `TRAILBOARD_SCRIPT` (click, move, wheel, fit, open, wait, log, quit).
- `scripts/install-cef.sh` installs the CEF build that matches the `cef` crate
  (works around `export-cef-dir` failing on CEF's >10 MB build index).
- README, plan (`docs/PLAN.md`), future plans (`docs/FUTURE.md`) and `CLAUDE.md`.

### Changed
- Ctrl+Shift+T now restores the closed card's trail edges and skips cards
  already brought back with undo.

### Fixed
- Cards' bottom corners are rounded like the rest of the card; the page image
  used to square them off (new `Shape::RoundBottom` in the compositor).
- Bold text (card titles) used a fallback serif face with wide gaps between
  words and digits; it now uses the sans font's bold face.
- Zoomed-in pages are sharp again: pages now render at the zoom's resolution
  (CEF ignored the device scale we reported, so pages always rendered at 1×).
- Shortcuts now follow the logical key instead of the physical keycode, so they
  work on non-QWERTY layouts and with virtual keyboards (a remapped key was
  being read as Escape).

### Known issues
- Copying uses the page's plain selected text, so pages that change what is
  copied, and "copy" buttons using `navigator.clipboard`, don't reach the desktop
  clipboard. Desktop text is pasted as plain text.
- Text copied from Trailboard leaves the clipboard when Trailboard quits.
- Pages see `devicePixelRatio` equal to the board's raster scale (0.5–2), and a page may lay out once at a stale zoom right after navigating to a new site.
- Custom image cursors (`cursor: url(…)`) show the default arrow.
- A card that wakes from sleep reloads its page, losing scroll position and form input.
- Typing is tested through scripted key events, not real keyboard input.
