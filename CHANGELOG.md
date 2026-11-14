# Changelog

All notable changes to Trailboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/) once it has releases.

## [Unreleased]

### Added
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
- Shortcuts now follow the logical key instead of the physical keycode, so they
  work on non-QWERTY layouts and with virtual keyboards (a remapped key was
  being read as Escape).

### Known issues
- `<select>` dropdowns and other popup widgets inside pages are not drawn yet.
- No limit on live browsers; many cards use a lot of memory.
- Manual keyboard typing into pages has not been verified by automated tests.
