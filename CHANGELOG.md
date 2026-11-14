# Changelog

All notable changes to Trailboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/) once it has releases.

## [Unreleased]

### Added
- **Trail names and colours**: press R with no page focused to name the trail
  in the middle of the view (an empty name clears it), and C to give it the
  next colour of the desktop theme (back to the accent after red). The name
  shows on a tag in the trail's colour above its first card (or its stack's
  front), the trail's edges take its colour, and stepping along it with
  Alt+PgDn shows the name in the toast. Links opened from a named trail join
  it with its name and colour, closing any of its cards keeps them, and both
  are undoable and saved with the board.
- Board files are now version 4 (adds trail names and colours); older boards
  are upgraded when read. Script `cards` logs each card's trail mark and the
  trail labels.
- **Notes**: press N with no page focused for a note card. Notes are written
  in Markdown: type in the note, then Esc or click away to see it rendered
  (headings, emphasis, links, lists, task lists, quotes, code). Click it to edit
  again, at the line you clicked; tick a task's box without editing. Enter
  continues a list. Links in a note open beside it. Notes follow the theme,
  are saved with the board, and take their title from their first line.
- **Add to note**: right-click selected text in a page and choose *Add to
  note* to quote it, with a link back to the page, at the end of the note you
  used last; with no note yet, a new note opens beside the page on its trail.
- Board files are now version 3 (adds notes); version 1 and 2 boards are
  upgraded when read.
- Script `cards` logs each note's text; `type` lines may contain `#`.
- **Next/Previous on Trail**: Alt+PgDn / Alt+PgUp go to the next / previous
  page of the trail you are on, in reading order, and focus it (also from
  inside a page and in Read Mode, where the page reached is read). Pages
  folded into a stack come to its front in place. A toast shows `2/4 · Title`,
  or the start or end of the trail.
- **Stack outline**: click a stack's count badge, or press O with no page
  focused, to list the pages in the stack. Pick one (click, or arrows and
  Enter) to bring it to the front, or choose *Unfold trail*. Script command
  `badgeclick ID` clicks a stack's badge.
- **Fold**: press S with no page focused to fold the trail in the middle of the
  view into a stack: only the card you were looking at stays on the board, with
  the edges of the others behind it, `2/4 ·` in its title and a count badge on
  its corner. `[` and `]` peek at the previous and next page of the stack in
  its place; S again unfolds the trail around the stack, as it was. The other
  pages sleep while folded. Fold and unfold are undoable, and stacks are saved
  with the board. Script `cards` logs folded cards (`hidden=`) and stacks.
- Board files are now version 2 (adds stacks); version 1 boards are upgraded
  when read and newer ones are refused, left untouched.
- **Tidy**: press T with no page focused to lay the board out again: each trail
  reads left to right from its first page, one column per step, with nothing
  overlapping; cards without a trail line up in rows below. Card sizes are kept,
  the card in the middle of the view stays where it is on screen, and Ctrl+Z
  undoes the whole Tidy in one step.
- **Read Mode**: Tab makes the focused card (or the one in the middle of the
  view) fill the window, with its page laid out at the window's size under a
  slim title bar. Tab, Esc or a double-click on the bar returns to the board as
  it was. Links opened while reading are added beside on the board and read
  next. After clicking inside a page, Tab goes to the page (form fields); Esc
  still leaves. Script `cards` logs the Read Mode state.
- **Omarchy look**: Trailboard follows the current Omarchy theme live, with its
  palette for the board, cards, address bar, menus, labels and the shortcut
  sheet, light themes as well as dark. Pages get the theme's light or dark
  `prefers-color-scheme`. Corners follow Hyprland's window rounding (asked
  again whenever Hyprland reloads its config), and overlays follow the
  desktop's text size (`omarchy display text size`). Without Omarchy it uses a
  dark palette with Omarchy's yellow accent.
- F1 shows the shortcut sheet from anywhere (also inside a page); Ctrl+Q quits.
- The window's Wayland app id is `trailboard`.
- **Spaces**: several boards, each with its own cards, camera and background
  tint. Ctrl+Shift+N makes a space, Ctrl+PageDown / Ctrl+PageUp go to the
  next / previous one, F2 renames it, Ctrl+Shift+B steps its tint (a theme colour)
  and Ctrl+Shift+W deletes it (Enter confirms when it has cards; the last space
  can't be deleted). The current space shows in the top-left corner when there
  is more than one, and switching shows a short toast. Undo history and closed
  cards are kept per space while the app runs.
- Spaces are stored as `spaces.json` (version 2; version 1 files are upgraded)
  plus `spaces/<id>.json`. The old
  `board.json` becomes the first space; a missing or unreadable index is rebuilt
  from the board files; a deleted space's board is moved to `spaces/deleted/`.
- Script command `spaces` logs every space and which one is active.
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
- N makes a note instead of opening the address bar for a new card (Ctrl+T
  still does that). Ctrl+L on a note opens the address bar for a new card.
- T on the board now tidies it; Ctrl+T still opens a new card.
- Ctrl+Shift+T now restores the closed card's trail edges and skips cards
  already brought back with undo.

### Fixed
- Text was drawn darker than its colour (it was gamma-decoded twice); light
  greys and theme colours now come out as specified.
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
