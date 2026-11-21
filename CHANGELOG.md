# Changelog

All notable changes to Trailboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/) once it has releases.

Until the first release this is a summary by area. The full entry-by-entry
log up to 2026-11-21 is in git: `git show f554118:CHANGELOG.md`.

## [Unreleased]

### Changed

- **Welcome, start and settings pages redesigned around motion** that
  uses the board's own movements (docs/DESIGN.md, "Motion on app pages").
  The welcome page is a pinned stage where scrolling moves the board's
  camera through six chapters: a link opening beside its source, stepping
  back, folding a trail, switching spaces and Solo. It is followed by a
  card you click to swap where the keys go, shortcut rows that light up
  when their key is pressed, and icons that draw themselves as they come
  in. On the start page the clock's digits turn over as they change, the
  field docks at the top while scrolling, recent cards lean toward the
  pointer, and menus and dialogs grow out of what opened them. The settings
  side column is a trail with a bead that rides it as you scroll; headings
  dock, a choice's thumb slides, a change says "Saved", and filtering
  moves rows to their new places. Nothing loops, and pages are still when
  the desktop asks for less motion (passed to pages as `still`).

- **Shortcut sheet redesigned** (F1, `?`). The sections are stops on a
  trail down the left: a bead rides it as you scroll, the section in view
  is lit, and a click glides to a section. The heading of the section in
  view docks under the header, rows rise into place when the sheet opens
  or the filter changes, the words a filter matched are marked in the
  accent, and each stop shows how many of its rows matched. The row
  under the pointer is lifted, and long lists of keys stay inside the
  panel.

- **Trails sidebar redesigned** (Ctrl+B). Each trail is drawn as it is on
  the board: the site icons are its stops, with lines running down from
  each page and curving into the pages it opened. The way from the
  trail's first page to the current card is lit in the accent, so the
  sidebar shows how you got here, and the current card's icon is ringed.
  The raised tab glides to the new current card, and rows come in one
  after another as the sidebar opens (all of it still when the desktop
  asks for less motion). The old faint guide lines are gone.

- **Trails turn at right angles.** A trail's line now runs straight out
  of its card's side, straight up or down halfway between the cards and
  straight into the next card, instead of an S-curve. Its turns are
  rounded like the cards' corners and follow the *Corners* setting
  (square corners, square turns). The sidebar's trails and the welcome
  page's drawn board turn the same way.

### Fixed

- **Smooth j / k scrolling.** Holding j or k scrolled in lurches: each
  key repeat restarted an eased scroll from a standstill. Scrolls now
  glide by our own frame loop, which chases a target each press moves on,
  so a held key flows at a steady speed and settles when released (the
  biggest one-frame jump went from 125 px to 35 px). A wheel or the page
  scrolling mid-glide takes over at once.

### Added

- **Page inspector.** F12, Ctrl+Shift+I or `:inspect` opens Chromium's
  inspector (elements, styles, console, sources, network) docked in the
  page's own card, on the right or at the bottom, with a divider to drag;
  or in a card of its own beside the page (`:inspect beside`, or the
  button above the card). Right-click → *Inspect* opens it on the clicked
  element. It runs off screen like a page and talks to the page
  in-process (`web::inspector`), with no debugging port. It takes every
  key while clicked (Passthrough), closes with its page, and is never saved.

- **Board and cards.** Web pages as live cards on an infinite board (CEF
  off-screen rendering, wgpu). Links open beside their source, joined by a
  trail edge; new cards are screen-shaped. Pan, zoom, fit, multi-select,
  group move, move and resize by keyboard, undo/redo. A live-browser cap
  (`TRAILBOARD_MAX_LIVE`) puts far cards to sleep.
- **Board tools.** Solo (Tab: the card fills the window), Tidy, Fold and
  stacks with an outline, trail steps (Alt+PgDn / Alt+PgUp), trail names
  and colours, notes and sticky notes, *Add to note*, Save as Markdown.
- **Spaces.** Several boards, each with its own cards, camera and
  background; private spaces (Ctrl+Shift+P); the board saved atomically
  with autosave and migrations.
- **Chrome.** Ctrl+K palette (cards, history, closed cards, commands, open
  beside with Shift+Enter), a trails sidebar and side panel, find in page,
  page zoom, page menu, desktop clipboard, IME, dropdowns and page cursors,
  site icons, downloads, a start page you can make your own, a welcome page
  on the first start, a shortcut sheet (F1), settings as a page (Ctrl+,)
  with the useful parts of Chromium's own.
- **History and search.** Every page's text in `page-text.db` (FTS5),
  thumbnails, a history view (Ctrl+H), forget a page, clear history.
- **Privacy.** Ad and tracker blocking with uBlock-style lists, cosmetic
  filtering, YouTube ads blocked, cookie banners answered with "reject",
  per-site toggles and allowed lists, a shield showing what was done,
  pages in Chromium's sandbox.
- **Keyboard first** (M12). Vim-like modes (Board, Page, Insert,
  Passthrough, Caret/Visual, Command) with a mode pill; counts; scroll
  sections (`s`, `]s`, `S`); link hints and `;` hint kinds, inside frames
  too; a `:` command line with completion, history, readline keys, `.`,
  `:repeat` and `:later`; cards as tabs (J/K, Alt+digits, Ctrl+^, gt, d/u,
  co, gC, `Ctrl+w` sequences); search keywords (`gh foo`); jumps
  (Ctrl+O / Ctrl+I), marks and saved pages (`b` / `B`); a user keys file
  (`~/.config/trailboard/keys`) with bindings, aliases, chains and `:run`;
  `:set` / `:toggle` / `:site` and `t` toggles; `]]` / `[[`, gu/gU,
  Ctrl+a/x and other address moves; caret and visual selection; page
  dialogs, permissions and a typed file chooser from the keyboard; Ctrl+e
  edits a field in `$EDITOR`; `?` finds backward; `:messages`.
- **Look.** Omarchy theme followed live; cards as sheets without title
  bars, quiet labels, corners (Rounded, Follow Hyprland, Custom), board
  backgrounds, motion, an empty-board welcome, a new app icon.
- **Packaging.** `scripts/bundle.sh`, an Arch PKGBUILD, a desktop entry,
  `--version` / `--help`, crash reports, opening pages in the running
  window, `scripts/install-cef.sh`.
- **Testing.** Scripted end-to-end runs via `TRAILBOARD_SCRIPT`.

### Changed

- **Sidebar.** With no trail on the board, the cards are listed straight
  under the space, without an "Other cards" header; the header comes back
  once a trail exists.
- The app icon no longer shows as a white tile in launchers that draw SVG
  without filters (Qt, e.g. the Omarchy shell): the grain layer's strength
  is its `fill-opacity` now, not the filter.
- **H.264 and AAC video.** `scripts/install-cef.sh --codecs` installs a CEF
  build with the patent-licensed codecs (same CEF version, third-party,
  pinned by SHA-256), and the Arch package uses it by default
  (`TRAILBOARD_CODECS=0` for the standard one). `scripts/bundle.sh` refuses
  to package it unless `ALLOW_CODECS=1`.
- `scripts/install-cef.sh` pins both engines by name and SHA-256 (no build
  index lookup), checks tools, disk space and a running Trailboard first,
  works on disk instead of `/tmp`, swaps the new copy in only once it's
  complete, skips a build that's already installed (`--force`), and writes
  `.cargo/config.toml` when it's missing; `--help` lists the options.
- Ctrl+W alone no longer closes a card; it starts `Ctrl+w` sequences.
- Read Mode is now called Solo; `t` starts toggles and Tidy is `T`; the
  trails sidebar is Ctrl+B and `b` / `B` open saved pages.
- Shortcuts follow the logical key, so they work on any layout.
- Version 0.3.0 (`Cargo.toml`, PKGBUILD).
- Docs: `docs/PLAN.md` and this file shortened; `docs/BUGS.md` and
  `docs/FUTURE.md` folded into the plan (§7 and §8).

### Known issues

- Copying uses the page's plain selected text, so pages that change what is
  copied, and "copy" buttons using `navigator.clipboard`, don't reach the
  desktop clipboard. Desktop text is pasted as plain text.
- Text copied from Trailboard leaves the clipboard when Trailboard quits.
- Pages see `devicePixelRatio` equal to the board's raster scale (0.5–2).
- Custom image cursors (`cursor: url(…)`) show the default arrow.
- A card that wakes from sleep reloads its page, losing scroll position and
  form input.
- H.264, AAC and HEVC video and DRM don't play with the standard CEF (M13).
- Typing is tested through scripted key events, not real keyboard input.
